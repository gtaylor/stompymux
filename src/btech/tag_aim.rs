//! The to-hit number of a TAG designation, which Total Warfare p. 142 resolves as a standard
//! weapon attack: gunnery, TAG's own 5/9/15 range brackets, attacker and target movement,
//! heat, sensor and control damage, the multiple-targets rule, cover and light. TAG has no
//! ammunition, mounting-section or targeting-computer terms.
use super::{AimModifiers, AimRules, RangeBracket, StealthRange, System, WeaponRange};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// TAG's short, medium and long range in hexes (TW p. 142); it has no minimum range.
pub const TAG_RANGES: [u8; 3] = [5, 9, 15];

/// TAG's range bracket at `distance`, or `None` beyond fifteen hexes. Reach is the raw
/// distance and the brackets use the same rounding as weapon ranges.
pub(super) fn tag_range(distance: f64) -> Result<Option<WeaponRange>> {
    ensure!(distance.is_finite() && distance >= 0.0, "Invalid TAG range");
    let [short, medium, long] = TAG_RANGES;
    if distance > f64::from(long) {
        return Ok(None);
    }
    let rounded = (distance + 0.95).floor() as u8;
    let (bracket, modifier) = if rounded > medium {
        (RangeBracket::Long, 4)
    } else if rounded > short {
        (RangeBracket::Medium, 2)
    } else {
        (RangeBracket::Short, 0)
    };
    Ok(Some(WeaponRange { bracket, modifier }))
}

/// Inspect a TAG designation's to-hit contributions without drawing dice or designating.
pub fn tag_aim_modifiers(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
    gunnery: i16,
    rules: AimRules,
) -> Result<AimModifiers> {
    for id in [shooter, target] {
        ensure!(
            world
                .objects
                .get(&id)
                .is_some_and(|object| !object.flags.contains(Flag::Going)),
            "Unit is unavailable"
        );
    }
    let source = super::fire_target::TargetSource::from(shooter);
    let distance = super::unit_range(world, shooter, target)?.spatial;
    let defender = super::scanner::scanner_unit(world, target).context("Target is unavailable")?;
    let movement = super::aim::unit_target_movement(world, target, distance)?;
    let (attacker_movement, heat, sensors, control_damage, haywire) =
        if let Some(vehicle) = world.btech.vehicles().get(&shooter) {
            (
                vehicle.attacker_movement_modifier(),
                0,
                0,
                vehicle.gunnery_damage(),
                vehicle.has_beacon(super::BeaconKind::Haywire),
            )
        } else {
            let mech = world
                .btech
                .constructed_units()
                .get(&shooter)
                .context("Shooter construction is unavailable")?;
            (
                mech.attacker_movement_modifier(),
                mech.heat().to_hit_modifier(),
                match mech.system_hits(System::Sensors) {
                    0 => 0,
                    1 => 2,
                    _ => 75,
                },
                0,
                mech.has_beacon(super::BeaconKind::Haywire),
            )
        };
    Ok(AimModifiers {
        indirect: None,
        gunnery,
        distance,
        network_range: None,
        range: tag_range(distance)?.map(|range| range.against_stealth(defender.concealed)),
        attacker_movement,
        spotting: u8::from(super::spotter::spotting_for_others(world, shooter)),
        target_movement: movement.total(),
        bogged_down: super::bog_down::target_modifier(world, target),
        orbital_drop: super::orbital_drop_state::current(world, target)
            .map_or(0, super::OrbitalDrop::target_modifier),
        light: super::light_aim::modifier(world, target, false)?,
        environment: Some(0),
        heat,
        sensors,
        control_damage,
        mounting_section: 0,
        weapon_accuracy: 0,
        // A Haywire pod on the designator makes TAG harder too (TW p. 141).
        beacon_accuracy: i8::from(haywire),
        ammunition_accuracy: 0,
        targeting_computer: 0,
        aimed_section: 0,
        target_lock: super::aim::lock_modifier(world, source, target, rules.override_weapon_arcs)?,
        perception: super::aim::perception_aim(world, shooter, target, false)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Brackets follow the 5/9/15 table with weapon-style rounding; reach is the raw distance.
    #[test]
    fn tag_brackets_and_reach() {
        for (distance, expected) in [
            (0.0, Some((RangeBracket::Short, 0))),
            (5.0, Some((RangeBracket::Short, 0))),
            (5.04, Some((RangeBracket::Short, 0))),
            (5.05, Some((RangeBracket::Medium, 2))),
            (9.0, Some((RangeBracket::Medium, 2))),
            (9.1, Some((RangeBracket::Long, 4))),
            (15.0, Some((RangeBracket::Long, 4))),
            (15.01, None),
        ] {
            let range = tag_range(distance).unwrap();
            assert_eq!(
                range.map(|range| (range.bracket, range.modifier)),
                expected,
                "{distance}"
            );
        }
        assert!(tag_range(-1.0).is_err());
        assert!(tag_range(f64::NAN).is_err());
    }
}
