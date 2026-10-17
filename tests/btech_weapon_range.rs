//! Weapon range is the hex count to the target (Total Warfare p. 109): height above or below
//! the shooter never adds to it, for range brackets, minimum range or maximum reach.
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Ordinary aim policy, with weapon arcs enforced.
fn rules() -> AimRules {
    AimRules {
        override_weapon_arcs: false,
    }
}

/// Put a VTOL in the air at `altitude` levels above the ground.
fn fly(world: &mut World, vtol: ObjectId, altitude: f64) {
    firing::edit(world, vtol, |state| {
        state["vtol_flight"]["phase"] = serde_json::json!({"kind": "airborne"});
        state["vtol_flight"]["altitude"] = serde_json::json!(altitude);
    });
}

/// A shooter armed with `weapon` and a target `range` hexes away down one clear column.
async fn lane(
    shooter: &str,
    weapon: Weapon,
    target: &str,
    range: u16,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    let (dir, config, world) = crate::support::isolated_world().await;
    let (world, shooter, target, index) = firing::lane_fixture_on(
        world,
        &config,
        (shooter, Some(weapon), false, None),
        target,
        range,
    );
    (dir, config, world, shooter, target, index)
}

/// A VTOL ten levels above an adjacent Mech is at range 1. Measured through the air the
/// range would be 2.24: a small laser would fall to long range and a PPC would lose most
/// of its minimum-range penalty.
#[tokio::test]
async fn a_vtol_above_an_adjacent_unit_fires_at_range_one() {
    let kestrel = include_str!("../game/units/Kestrel.toml");
    let jenner = include_str!("../game/units/JR7-D.toml");
    for (weapon, bracket, modifier) in [
        (Weapon::SmallLaser, RangeBracket::Short, 0),
        (Weapon::Ppc, RangeBracket::Minimum, 3),
    ] {
        let (_dir, config, mut world, shooter, target, index) =
            lane(kestrel, weapon, jenner, 1).await;
        fly(&mut world, shooter, 10.0);
        refresh_battle_contacts(&mut world, &[shooter]).unwrap();
        let measured = battle_unit_range(&world, shooter, target).unwrap();
        assert_eq!(measured.hex_distance, 1);
        assert!((measured.spatial - 5.0_f64.sqrt()).abs() < 1e-9);
        assert_ne!(
            weapon.range_modifier(measured.spatial).unwrap(),
            Some(WeaponRange { bracket, modifier }),
            "{weapon:?}: the case must tell hexes from spatial distance"
        );
        let aim = battle_aim_modifiers(&world, shooter, target, index, 4, rules()).unwrap();
        assert_eq!(aim.distance, 1.0);
        assert_eq!(
            aim.range,
            Some(WeaponRange { bracket, modifier }),
            "{weapon:?}"
        );
        world.validate(&config).unwrap();
    }
}

/// An LRM-20 keeps its minimum range at six hexes and its reach at twenty-one however high
/// the target flies (TW pp. 109, 113): thirty levels up at six hexes is still the +1 minimum
/// range, and five levels up at twenty-one hexes is still long range, not out of range.
#[tokio::test]
async fn height_never_pushes_an_lrm_target_out_of_its_range() {
    let jenner = include_str!("../game/units/JR7-D.toml");
    let kestrel = include_str!("../game/units/Kestrel.toml");
    for (range, altitude, bracket, modifier) in [
        (6, 30.0, RangeBracket::Minimum, 1),
        (21, 5.0, RangeBracket::Long, 4),
    ] {
        let (_dir, config, mut world, shooter, target, index) =
            lane(jenner, Weapon::Lrm20, kestrel, range).await;
        fly(&mut world, target, altitude);
        refresh_battle_contacts(&mut world, &[shooter]).unwrap();
        let measured = battle_unit_range(&world, shooter, target).unwrap();
        assert_eq!(measured.hex_distance, u64::from(range));
        assert_ne!(
            Weapon::Lrm20.range_modifier(measured.spatial).unwrap(),
            Some(WeaponRange { bracket, modifier }),
            "{range} hexes: the case must tell hexes from spatial distance"
        );
        let aim = battle_aim_modifiers(&world, shooter, target, index, 4, rules()).unwrap();
        assert_eq!(aim.distance, f64::from(range));
        assert_eq!(
            aim.range,
            Some(WeaponRange { bracket, modifier }),
            "{range}"
        );
        world.validate(&config).unwrap();
    }
}
