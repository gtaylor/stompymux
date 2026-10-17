//! Hex targets are aimed at like units (Total Warfare pp. 106-107, 110): woods between the
//! attacker and the hex add their to-hit modifiers, the hex's own terrain adds nothing, and an
//! empty hex is an immobile target at -4.
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Hexes from the shooter to the targeted hex: medium range for a medium laser.
const RANGE: i32 = 5;

/// Ordinary shot rules with the pilot's own gunnery.
fn shot_rules() -> ShotRules {
    ShotRules {
        tsm_tow_bonus: true,
        aim: AimRules {
            override_weapon_arcs: false,
        },
        vehicle_impact: VehicleImpactRules::STANDARD,
        extended_gunnery: false,
        extended_piloting: true,
        target_toughness: false,
    }
}

/// A Jenner with a medium laser at the south end of a clear one-column lane, its locked
/// target three hexes past the empty hex it will shoot at, and that hex.
async fn lane() -> (
    tempfile::TempDir,
    Config,
    World,
    ObjectId,
    usize,
    HexCoordinate,
) {
    let jenner = include_str!("../game/units/JR7-D.toml");
    let (dir, config, world) = crate::support::isolated_world().await;
    let (world, shooter, _target, index) = firing::lane_fixture_on(
        world,
        &config,
        (jenner, Some(Weapon::MediumLaser), false, None),
        jenner,
        8,
    );
    let row = i32::from(world.btech.unit(shooter).unwrap().position().unwrap().y);
    let hex = HexCoordinate {
        x: 0,
        y: row - RANGE,
    };
    (dir, config, world, shooter, index, hex)
}

/// Plant light woods in the given rows of the shooter's lane.
fn plant(world: &mut World, shooter: ObjectId, rows: &[i32]) {
    let map = world.btech.unit(shooter).unwrap().position().unwrap().map;
    world
        .btech
        .rewrite_map_record(map, |record| {
            for &row in rows {
                record["terrain"][usize::try_from(row).unwrap()] =
                    serde_json::to_value(Hex::new(Terrain::LightWoods, 0)).unwrap();
            }
        })
        .unwrap();
}

/// A shot at an empty clear hex takes -4 for an immobile target, even under a plain unit
/// lock: gunnery 4, medium range +2, immobile -4.
#[tokio::test]
async fn an_empty_clear_hex_is_an_immobile_target() {
    let (_dir, config, world, shooter, index, hex) = lane().await;
    let aim = battle_hex_aim_modifiers(&world, shooter, hex, index, 4).unwrap();
    assert_eq!(aim.mode, HexTargetMode::UnitAtHex);
    assert_eq!(aim.modifiers.distance, f64::from(RANGE));
    assert_eq!(aim.modifiers.range.unwrap().modifier, 2);
    assert_eq!(aim.terrain, 0);
    assert_eq!(aim.hex_bonus, -4);
    assert_eq!(aim.subtotal(), Some(2));
    world.validate(&config).unwrap();
}

/// Two hexes of light woods between the shooter and the hex add +2 (TW p. 107), and the shot
/// resolves at that number. Light woods in the target hex itself add nothing: a hex does not
/// occupy itself.
#[tokio::test]
async fn a_hex_shot_pays_for_intervening_woods_only() {
    let (_dir, config, mut world, shooter, index, hex) = lane().await;
    let clear = battle_hex_aim_modifiers(&world, shooter, hex, index, 4)
        .unwrap()
        .subtotal()
        .unwrap();
    let mut wooded_target = world.clone();
    plant(&mut wooded_target, shooter, &[hex.y]);
    let aim = battle_hex_aim_modifiers(&wooded_target, shooter, hex, index, 4).unwrap();
    assert!(aim.visible);
    assert_eq!(aim.terrain, 0);
    assert_eq!(aim.subtotal(), Some(clear));
    plant(&mut world, shooter, &[hex.y + 2, hex.y + 3]);
    let aim = battle_hex_aim_modifiers(&world, shooter, hex, index, 4).unwrap();
    assert!(aim.visible);
    assert_eq!(aim.terrain, 2);
    assert_eq!(aim.subtotal(), Some(clear + 2));
    let expected = battle_pilot_hex_aim_modifiers(&world, shooter, hex, index, false)
        .unwrap()
        .subtotal();
    let shot = resolve_battle_hex_shot(&mut world, shooter, ObjectId(1), hex, index, shot_rules())
        .unwrap();
    assert_eq!(shot.aim.terrain, 2);
    assert_eq!(shot.target_number, expected);
    world.validate(&config).unwrap();
}
