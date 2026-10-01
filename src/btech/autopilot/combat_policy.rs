//! Pure target choice over the same acquired and network-relayed contacts available to Lua.

use super::observations::AutopilotObservation;
use crate::ObjectId;

/// Select a currently observed hostile target. The explicit assignment wins when visible,
/// including through a C3/C3i peer.
/// Targets the unit can engage itself are always preferred to network-relayed ones, which
/// are chosen only so the unit can close on and face an enemy it cannot yet see.
/// An existing target is retained until another candidate scores at least 20% higher.
pub fn choose_target(
    observation: &AutopilotObservation,
    explicit: Option<ObjectId>,
    current: Option<ObjectId>,
) -> Option<ObjectId> {
    let _measurement = crate::btech::autopilot::diagnostics::measure(
        crate::btech::autopilot::diagnostics::Category::Selection,
    );
    let hostile: Vec<_> = observation
        .contacts
        .iter()
        .filter(|contact| contact.identified && !contact.friendly && !contact.known_destroyed)
        .collect();
    if let Some(explicit) = explicit {
        return hostile
            .iter()
            .find(|contact| contact.unit == explicit)
            .map(|contact| contact.unit);
    }
    let hostile: Vec<_> = if hostile.iter().any(|contact| !contact.relayed) {
        hostile
            .into_iter()
            .filter(|contact| !contact.relayed)
            .collect()
    } else {
        hostile
    };
    // Only the controller's own ready weapons and acquired range may inform
    // this estimate. It does not inspect target armor, heat, ammunition or
    // any hidden capability. The actual shot path still does full admission.
    let profiles: Vec<_> = observation
        .own
        .weapons
        .iter()
        .filter(|ready| ready.ready)
        .filter(|ready| !ready.weapon.is_ams() && !ready.weapon.is_artillery())
        .map(|ready| ready.weapon.profile())
        .collect();
    // Physical distance decides reach and minimum range; the network's shared distance, when
    // present, decides the band, exactly as the shot path applies it.
    let score = |contact: &super::observations::AutopilotContact| {
        let range = contact.range.max(0.0);
        let band = contact.aiming_range().clamp(0.0, range);
        let effectiveness: f64 = profiles
            .iter()
            .filter(|profile| range <= f64::from(profile.long_range))
            .map(|profile| {
                let nominal = f64::from(profile.damage) * f64::from(profile.missiles.max(1));
                let assisted = range > f64::from(profile.minimum_range);
                let band = if assisted { band } else { range };
                let range_factor = if band <= f64::from(profile.short_range) {
                    1.0
                } else if band <= f64::from(profile.medium_range) {
                    0.66
                } else {
                    0.33
                };
                let minimum_factor = if range < f64::from(profile.minimum_range) {
                    0.5
                } else {
                    1.0
                };
                nominal * range_factor * minimum_factor
            })
            .sum();
        if effectiveness > 0.0 {
            effectiveness + 0.01 / (1.0 + range)
        } else {
            1.0 / (1.0 + range)
        }
    };
    let scored: Vec<_> = hostile
        .iter()
        .map(|contact| (*contact, score(contact)))
        .collect();
    let &(best, best_score) =
        scored
            .iter()
            .max_by(|(left, left_score), (right, right_score)| {
                left_score
                    .total_cmp(right_score)
                    .then_with(|| right.range.total_cmp(&left.range))
                    .then_with(|| right.unit.cmp(&left.unit))
            })?;
    if let Some(current) = current
        && let Some((old, old_score)) = scored.iter().find(|(contact, _)| contact.unit == current)
        && best_score < old_score * 1.2
    {
        return Some(old.unit);
    }
    Some(best.unit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::{BattlePosition, autopilot::observations::AutopilotContact};

    fn reference_choose_target(
        observation: &AutopilotObservation,
        explicit: Option<ObjectId>,
        current: Option<ObjectId>,
    ) -> Option<ObjectId> {
        let _measurement = crate::btech::autopilot::diagnostics::measure(
            crate::btech::autopilot::diagnostics::Category::Selection,
        );
        let hostile: Vec<_> = observation
            .contacts
            .iter()
            .filter(|contact| contact.identified && !contact.friendly && !contact.known_destroyed)
            .collect();
        if let Some(explicit) = explicit {
            return hostile
                .iter()
                .find(|contact| contact.unit == explicit)
                .map(|contact| contact.unit);
        }
        // Only the controller's own ready weapons and acquired range may inform
        // this estimate. It does not inspect target armor, heat, ammunition or
        // any hidden capability. The actual shot path still does full admission.
        let score = |distance: f64| {
            let range = distance.max(0.0);
            let effectiveness: f64 = observation
                .own
                .weapons
                .iter()
                .filter(|ready| ready.ready)
                .filter(|ready| !ready.weapon.is_ams() && !ready.weapon.is_artillery())
                .map(|ready| ready.weapon.profile())
                .filter(|profile| range <= f64::from(profile.long_range))
                .map(|profile| {
                    let nominal = f64::from(profile.damage) * f64::from(profile.missiles.max(1));
                    let range_factor = if range <= f64::from(profile.short_range) {
                        1.0
                    } else if range <= f64::from(profile.medium_range) {
                        0.66
                    } else {
                        0.33
                    };
                    let minimum_factor = if range < f64::from(profile.minimum_range) {
                        0.5
                    } else {
                        1.0
                    };
                    nominal * range_factor * minimum_factor
                })
                .sum();
            if effectiveness > 0.0 {
                effectiveness + 0.01 / (1.0 + range)
            } else {
                1.0 / (1.0 + range)
            }
        };
        let best = hostile.iter().max_by(|left, right| {
            score(left.range)
                .total_cmp(&score(right.range))
                .then_with(|| right.range.total_cmp(&left.range))
                .then_with(|| right.unit.cmp(&left.unit))
        })?;
        if let Some(current) = current
            && let Some(old) = hostile.iter().find(|contact| contact.unit == current)
            && score(best.range) < score(old.range) * 1.2
        {
            return Some(current);
        }
        Some(best.unit)
    }

    #[test]
    fn ignores_friendlies_and_retains_close_score() {
        let map = ObjectId(1);
        let at = |unit, range, friendly| AutopilotContact {
            unit: ObjectId(unit),
            position: BattlePosition {
                map,
                x: unit as u16,
                y: 0,
            },
            friendly,
            identified: true,
            known_destroyed: false,
            range,
            network_range: None,
            relayed: false,
            seen_at: 0,
        };
        let observation = AutopilotObservation {
            unit: ObjectId(10),
            time: 0,
            position: None,
            heading: None,
            speed: 0.0,
            own: crate::btech::autopilot::observations::AutopilotOwnReadiness {
                power: crate::btech::BattlePower::Off,
                maximum_speed: 0.0,
                heat: None,
                weapons: vec![],
            },
            contacts: vec![at(2, 11.0, false), at(3, 10.0, false), at(4, 1.0, true)],
            remembered: vec![],
        };
        let config = crate::Config::load("tests/fixtures/game").unwrap();
        let mut world = crate::World::default();
        let unit = world.create(&config, "Policy weapon fixture".into(), crate::Kind::Thing);
        crate::BattleUnitTemplate::parse("JR7-D", include_str!("../../../game/mechs/JR7-D.toml"))
            .unwrap()
            .create(&mut world, unit)
            .unwrap();
        let weapons = world.btech.constructed_units()[&unit]
            .weapon_readiness_batch()
            .unwrap();
        for sample in 0..160 {
            let mut generated = observation.clone();
            generated.own.weapons = weapons.clone();
            for (index, weapon) in generated.own.weapons.iter_mut().enumerate() {
                weapon.ready = (sample + index) % 3 != 0;
            }
            generated.contacts = (1..=12)
                .map(|id| {
                    let mut contact =
                        at(id, ((sample * id as usize) % 40) as f64 / 2.0, id % 5 == 0);
                    contact.identified = id % 7 != 0;
                    contact.known_destroyed = id % 11 == 0;
                    contact
                })
                .collect();
            for explicit in [
                None,
                Some(ObjectId(2)),
                Some(ObjectId(5)),
                Some(ObjectId(99)),
            ] {
                for current in [
                    None,
                    Some(ObjectId(1)),
                    Some(ObjectId(3)),
                    Some(ObjectId(99)),
                ] {
                    assert_eq!(
                        choose_target(&generated, explicit, current),
                        reference_choose_target(&generated, explicit, current)
                    );
                }
            }
        }
        assert_eq!(
            choose_target(&observation, None, Some(ObjectId(2))),
            Some(ObjectId(2))
        );
        assert_eq!(
            choose_target(&observation, Some(ObjectId(3)), None),
            Some(ObjectId(3))
        );
        assert_eq!(choose_target(&observation, Some(ObjectId(4)), None), None);
        let mut unknown = at(5, 0.5, false);
        unknown.identified = false;
        let mut observation = observation;
        observation.contacts.push(unknown);
        assert_eq!(choose_target(&observation, Some(ObjectId(5)), None), None);
        assert_eq!(choose_target(&observation, None, None), Some(ObjectId(3)));
    }

    /// Network sightings steer choice without ever outranking a target the unit can engage.
    #[test]
    fn network_range_improves_bands_and_relayed_targets_only_fill_gaps() {
        let map = ObjectId(1);
        let at = |unit: i64, range, network_range, relayed| AutopilotContact {
            unit: ObjectId(unit),
            position: BattlePosition {
                map,
                x: unit as u16,
                y: 0,
            },
            friendly: false,
            identified: true,
            known_destroyed: false,
            range,
            network_range,
            relayed,
            seen_at: 0,
        };
        let config = crate::Config::load("tests/fixtures/game").unwrap();
        let mut world = crate::World::default();
        let unit = world.create(&config, "Network policy fixture".into(), crate::Kind::Thing);
        crate::BattleUnitTemplate::parse("JR7-D", include_str!("../../../game/mechs/JR7-D.toml"))
            .unwrap()
            .create(&mut world, unit)
            .unwrap();
        let mut observation = AutopilotObservation {
            unit: ObjectId(10),
            time: 0,
            position: None,
            heading: None,
            speed: 0.0,
            own: crate::btech::autopilot::observations::AutopilotOwnReadiness {
                power: crate::btech::BattlePower::Running,
                maximum_speed: 0.0,
                heat: None,
                weapons: world.btech.constructed_units()[&unit]
                    .weapon_readiness_batch()
                    .unwrap(),
            },
            contacts: vec![at(2, 6.5, None, false), at(3, 8.0, None, false)],
            remembered: vec![],
        };
        for weapon in &mut observation.own.weapons {
            weapon.ready = true;
        }
        // Unassisted, the nearer target sits in the better band.
        assert_eq!(choose_target(&observation, None, None), Some(ObjectId(2)));
        // A peer two hexes from the farther target puts it in short range for this unit.
        observation.contacts[1].network_range = Some(2.0);
        assert_eq!(choose_target(&observation, None, None), Some(ObjectId(3)));
        // A relayed target never displaces one the unit can lock, however close the peer is.
        observation.contacts.push(at(4, 1.0, Some(1.0), true));
        assert_eq!(choose_target(&observation, None, None), Some(ObjectId(3)));
        // With nothing of its own in view, the unit turns to what its peers see.
        observation.contacts.retain(|contact| contact.relayed);
        observation.contacts.push(at(5, 30.0, Some(30.0), true));
        assert_eq!(choose_target(&observation, None, None), Some(ObjectId(4)));
        // An explicit attack order may name a target only a peer can see.
        assert_eq!(
            choose_target(&observation, Some(ObjectId(5)), None),
            Some(ObjectId(5))
        );
    }
}
