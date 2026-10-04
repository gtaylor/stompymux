//! Located vehicle impacts commit hit-table effects, armor damage and critical cascades together.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Vehicle hit-table, critical and environmental fire policy; the caller supplies configuration.
#[derive(Debug, Clone, Copy)]
pub struct VehicleImpactRules {
    /// Inferno ignites section fires instead of rolling for a heat explosion.
    pub advanced_fire: bool,
    pub criticals: VehicleCriticalRules,
    pub hit: VehicleHitRules,
}

/// One damage group's table selection and all staged effects, before visibility publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish impact effects only with the enclosing attack transaction"]
pub struct VehicleImpact {
    /// Initial routing roll, plus the selected table's own roll for advanced or critical-proof routing.
    pub rolls: Vec<u8>,
    /// Combat-safe routing has no hit location or motive effects.
    pub hit: Option<VehicleHit>,
    pub damage: Option<VehicleArmorDamage>,
    pub notices: Vec<Notice>,
    /// Pilot-only control feedback indexed into the damage notice stream.
    pub pilot_notices: Vec<PilotNotice>,
    pub broadcasts: Vec<Notice>,
}

/// Resolve one already successful attack group from its incoming arc, preserving all-or-nothing dice and damage.
/// Weapon targeting and publication still belong to the enclosing attack.
pub fn resolve_vehicle_impact(
    world: &mut World,
    id: ObjectId,
    arc: HitArc,
    amount: u32,
    armor_piercing: Option<Weapon>,
    rules: VehicleImpactRules,
) -> Result<VehicleImpact> {
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    ensure!(!vehicle.is_destroyed(), "Vehicle is destroyed");
    world.attempt(|world| {
        let result = resolve_followup_in_candidate(
            world,
            id,
            arc,
            ImpactRequest {
                amount,
                armor_piercing,
                rear: false,
                attacker: None,
                class: DamageClass::Ordinary,
            },
            rules,
        )?;
        Ok(result)
    })
}

/// One admitted vehicle damage entry, independent of the attacker's chassis.
#[derive(Debug, Clone, Copy)]
pub(super) struct ImpactRequest {
    pub amount: u32,
    pub armor_piercing: Option<Weapon>,
    pub rear: bool,
    pub attacker: Option<ObjectId>,
    pub class: DamageClass,
}

/// Continue a previously admitted attack, retaining location and damage rolls after hull loss.
pub(super) fn resolve_followup_in_candidate(
    world: &mut World,
    id: ObjectId,
    arc: HitArc,
    request: ImpactRequest,
    rules: VehicleImpactRules,
) -> Result<VehicleImpact> {
    resolve_directed_followup(world, id, arc, request, rules, None)
}

/// Directed hits bypass random location effects while sharing material damage and critical resolution.
pub(super) fn resolve_directed_followup(
    world: &mut World,
    id: ObjectId,
    arc: HitArc,
    request: ImpactRequest,
    rules: VehicleImpactRules,
    forced: Option<VehicleHit>,
) -> Result<VehicleImpact> {
    let ImpactRequest {
        amount,
        armor_piercing,
        rear,
        attacker,
        class,
    } = request;
    ensure!(amount > 0, "Impact damage must be positive");
    if let Some(weapon) = armor_piercing {
        ensure!(
            AmmunitionMode::ArmorPiercing.supports(weapon),
            "Weapon cannot fire armor-piercing ammunition"
        );
    }
    let mut result = if let Some(hit) = forced {
        VehicleImpact {
            rolls: Vec::new(),
            hit: Some(hit),
            damage: None,
            notices: Vec::new(),
            pilot_notices: Vec::new(),
            broadcasts: Vec::new(),
        }
    } else {
        resolve_location_followup(world, id, arc, rules)?
    };
    if let Some(hit) = result.hit {
        let damage = super::vehicle_armor_damage::resolve_rear_followup_in_candidate(
            world,
            id,
            VehicleArmorHit {
                section: hit.section,
                amount,
                through_armor_critical: hit.through_armor_critical,
                armor_piercing,
                damage_class: class,
            },
            rear,
            rules.criticals,
            super::vehicle_internal_damage::DamageContext { attacker, depth: 0 },
        )?;
        super::piloting::append_feedback(
            &mut result.pilot_notices,
            damage.pilot_notices.iter().cloned(),
            result.notices.len(),
        );
        result.notices.extend(damage.notices.clone());
        result.broadcasts.extend(damage.broadcasts.clone());
        result.damage = Some(damage);
    } else {
        result
            .notices
            .extend(super::combat_safe::notice(attacker, id));
        // Safe material impacts still enter the damage stage and consume its diagnostic roll.
        world
            .btech
            .vehicles
            .get_mut(&id)
            .unwrap()
            .dice
            .generic_roll();
    }
    Ok(result)
}

/// Resolve table routing and its direct effects inside an unpublished attack candidate.
/// Pod hits stop here; ordinary impacts continue into armor and internal damage.
pub(super) fn resolve_location(
    world: &mut World,
    id: ObjectId,
    arc: HitArc,
    rules: VehicleImpactRules,
) -> Result<VehicleImpact> {
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    ensure!(!vehicle.is_destroyed(), "Vehicle is destroyed");
    resolve_location_followup(world, id, arc, rules)
}

/// Table routing also applies to later packets against a wreck's remaining sections.
fn resolve_location_followup(
    world: &mut World,
    id: ObjectId,
    arc: HitArc,
    rules: VehicleImpactRules,
) -> Result<VehicleImpact> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Vehicle is unavailable"
    );
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    let table = rules.criticals.table_for(vehicle);
    let proof = vehicle.definition().has_special("CritProof_Tech");
    let mut dice = vehicle.dice.clone();
    let mut rolls = vec![dice.generic_roll()];
    if table != VehicleCriticalTable::Standard || proof {
        rolls.push(dice.generic_roll());
    }
    let mut result = VehicleImpact {
        rolls,
        hit: None,
        damage: None,
        notices: Vec::new(),
        pilot_notices: Vec::new(),
        broadcasts: Vec::new(),
    };
    if rules.criticals.combat_safe || vehicle.combat_safe {
        world.btech.vehicles.get_mut(&id).unwrap().dice = dice;
        return Ok(result);
    }
    let roll = *result.rolls.last().unwrap();
    if vehicle.definition().is_vtol() {
        let selected = if table == VehicleCriticalTable::Advanced {
            vehicle.advanced_vtol_hit(
                arc,
                roll,
                rules.hit.critical_mode,
                rules.hit.critical_level,
                &mut dice,
            )?
        } else {
            vehicle.definition().vtol_hit(arc, roll)?
        };
        world.btech.vehicles.get_mut(&id).unwrap().dice = dice;
        if let Some(effect) = selected.rotor {
            let rotor = super::rotor_damage::apply_in_world(world, id, effect)?;
            result.notices.push(Notice {
                unit: id,
                text: rotor.message().into(),
            });
        }
        result.hit = Some(selected.hit);
        return Ok(result);
    }

    let turret = if table == VehicleCriticalTable::Advanced {
        vehicle
            .sections()
            .get(&VehicleSection::Turret)
            .is_some_and(|section| section.internal > 0)
    } else {
        vehicle
            .definition()
            .sections
            .get(&VehicleSection::Turret)
            .is_some_and(|section| section.internal > 0)
    };
    let exposed_turret = vehicle.dig_state().dug_in && turret && dice.die(100)? >= 42;
    let hit = if exposed_turret {
        super::VehicleHit {
            section: VehicleSection::Turret,
            through_armor_critical: false,
            motive: None,
            motive_roll: None,
            piloting_penalty: 0,
        }
    } else if table == VehicleCriticalTable::Advanced {
        vehicle.advanced_hit(
            arc,
            roll,
            rules.hit.critical_mode,
            rules.hit.critical_level,
            &mut dice,
        )?
    } else if proof {
        vehicle.critical_proof_hit(arc, roll)?
    } else {
        vehicle.standard_hit(arc, roll, rules.hit.critical_mode, &mut dice)?
    };
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    vehicle.dice = dice;
    let (notices, broadcasts) = super::vehicle_motive_effects::apply(
        vehicle,
        id,
        hit.motive,
        hit.piloting_penalty,
        hit.motive_roll,
    );
    result.notices.extend(notices);
    result.broadcasts.extend(broadcasts);
    result.hit = Some(hit);
    Ok(result)
}

impl VehicleImpactRules {
    /// Standard tactical vehicle hit and critical policy.
    pub const STANDARD: Self = Self {
        advanced_fire: false,
        criticals: VehicleCriticalRules {
            rotor_damage_divisor: 0,
            extended_piloting: false,
            vtol_table: None,
            table: VehicleCriticalTable::Standard,
            enabled: true,
            combat_safe: false,
            toughness: false,
        },
        hit: VehicleHitRules {
            critical_mode: 2,
            critical_level: 60,
        },
    };

    /// Apply configured hit-table and critical policy consistently for either shooter class.
    pub(crate) fn configured(config: &crate::config::BattleTechConfig, toughness: bool) -> Self {
        Self {
            advanced_fire: config.fasaadvvhlfire != 0,
            criticals: VehicleCriticalRules {
                rotor_damage_divisor: config.divrotordamage.clamp(0, i64::from(u32::MAX)) as u32,
                extended_piloting: config.extended_piloting != 0,
                vtol_table: Some(VehicleCriticalTable::from_settings(
                    config.fasaadvvtolcrit != 0,
                )),
                table: VehicleCriticalTable::from_settings(config.fasaadvvhlcrit != 0),
                enabled: config.vcrit != 0,
                combat_safe: false,
                toughness,
            },
            hit: VehicleHitRules {
                critical_mode: config.vcrit,
                critical_level: config.critlevel,
            },
        }
    }
}

#[cfg(test)]
mod policy_tests {
    use super::*;

    /// Host settings propagate to exterior damage while nonpositive settings disable scaling.
    #[test]
    fn configured_rotor_divisor_is_bounded_and_retained() {
        for (setting, expected) in [(-1, 0), (0, 0), (1, 1), (5, 5), (i64::MAX, u32::MAX)] {
            let config = crate::config::BattleTechConfig {
                divrotordamage: setting,
                ..Default::default()
            };
            let rules = VehicleImpactRules::configured(&config, false);
            assert_eq!(rules.criticals.rotor_damage_divisor, expected);
        }
    }

    #[test]
    fn configured_critical_tables_are_selected_per_victim_class() {
        let ground = Vehicle::new(
            VehicleTemplate::parse(
                "Demolisher",
                include_str!("../../game/mechs/Demolisher.toml"),
            )
            .unwrap(),
        )
        .unwrap();
        let aircraft = Vehicle::new(
            VehicleTemplate::parse("Kestrel", include_str!("../../game/mechs/Kestrel.toml"))
                .unwrap(),
        )
        .unwrap();
        let observation = Vehicle::new(
            VehicleTemplate::parse(
                "ObservationVTOL",
                include_str!("../../game/mechs/ObservationVTOL.toml"),
            )
            .unwrap(),
        )
        .unwrap();
        for ground_advanced in [false, true] {
            for aircraft_advanced in [false, true] {
                let config = crate::config::BattleTechConfig {
                    fasaadvvhlcrit: i64::from(ground_advanced),
                    fasaadvvtolcrit: i64::from(aircraft_advanced),
                    ..Default::default()
                };
                let rules = VehicleImpactRules::configured(&config, true);
                for (unit, advanced) in [
                    (&ground, ground_advanced),
                    (&aircraft, aircraft_advanced),
                    (&observation, aircraft_advanced),
                ] {
                    let expected = if advanced {
                        VehicleCriticalTable::Advanced
                    } else {
                        VehicleCriticalTable::Standard
                    };
                    assert_eq!(rules.criticals.table_for(unit), expected);
                }
                assert!(rules.criticals.toughness);
                // Shared launcher policy updates must preserve both victim choices.
                let launch = VehicleCriticalRules {
                    toughness: false,
                    ..rules.criticals
                };
                assert_eq!(
                    launch.table_for(&aircraft),
                    rules.criticals.table_for(&aircraft)
                );
            }
        }
    }
}
