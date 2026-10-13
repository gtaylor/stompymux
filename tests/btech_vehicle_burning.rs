//! Infernos against vehicles: one critical roll per missile at −2 (TW p. 142), the stationary
//! unit's jelly timer, and blast heat that leaves mobile vehicles alone.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// A placed, shutdown vehicle with an assigned operator and no active scanners.
async fn fixture(stationary: bool) -> (tempfile::TempDir, Config, World, ObjectId) {
    fixture_movement(if stationary {
        VehicleMovement::Stationary
    } else {
        VehicleMovement::Tracked
    })
    .await
}

/// Use the same protection for tracked and rotorcraft checks.
async fn fixture_movement(
    movement: VehicleMovement,
) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Fire test".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "fire",
        MapAsset::from_cells("1 2\n.0\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Burning vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    let mut text = if movement == VehicleMovement::Vtol {
        include_str!("../game/units/Kestrel.toml")
    } else {
        include_str!("../game/units/Demolisher.toml")
    }
    .to_owned();
    if movement == VehicleMovement::Stationary {
        text = text
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 5", "walk_mp = 0");
    }
    create_battle_vehicle(
        &mut world,
        id,
        VehicleTemplate::parse("test", &text).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, id)
}

/// Alter selected saved fields while retaining domain deserialization validation.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    world.btech.rewrite_unit_record(id, change).unwrap();
}

/// Find an independent stream with a selected leading result.
fn matching_seed(predicate: impl Fn(&mut Dice) -> bool) -> [u8; 32] {
    for number in 0u32..100000 {
        let mut seed = [0; 32];
        seed[..4].copy_from_slice(&number.to_le_bytes());
        if predicate(&mut Dice::seeded(seed)) {
            return seed;
        }
    }
    panic!("No matching stream");
}

#[tokio::test]
async fn each_inferno_missile_rolls_a_critical_at_minus_two_on_the_struck_location() {
    let (_dir, config, base, id) = fixture(false).await;
    // Missile one: front (7), table roll 10 (Commander Hit at −2 is an 8, Stabilizer).
    // Missile two: turret (10), table roll 9 (a 7, Turret Jam).
    let seed = matching_seed(|dice| {
        dice.two_d6() == 7 && dice.two_d6() == 10 && dice.two_d6() == 10 && dice.two_d6() == 9
    });
    let mut world = base.clone();
    edit(&mut world, id, |unit| {
        unit["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap()
    });
    let before = world.btech.vehicles()[&id].clone();
    let mut replay = world.clone();
    let report =
        resolve_battle_vehicle_inferno_hit(&mut world, id, 2, VehicleImpactRules::STANDARD)
            .unwrap();
    assert_eq!(report.missiles, 2);
    assert_eq!(report.burn_seconds, 0);
    assert_eq!(report.criticals.len(), 2);
    let first = &report.criticals[0];
    assert_eq!(first.location_roll, 7);
    assert_eq!(first.section, VehicleSection::Front);
    assert_eq!(first.critical.selection.rolls, vec![10]);
    assert_eq!(first.critical.selection.modifier, INFERNO_CRITICAL_MODIFIER);
    assert_eq!(
        first.critical.selection.effect,
        Some(VehicleCriticalEffect::Stabilizer)
    );
    let second = &report.criticals[1];
    assert_eq!(second.location_roll, 10);
    assert_eq!(second.section, VehicleSection::Turret);
    assert_eq!(
        second.critical.selection.effect,
        Some(VehicleCriticalEffect::TurretJam)
    );
    let vehicle = &world.btech.vehicles()[&id];
    assert!(vehicle.lost_stabilizers().contains(&VehicleSection::Front));
    assert!(vehicle.turret_jammed());
    // Infernos deal no damage of their own.
    assert_eq!(vehicle.sections(), before.sections());
    assert!(!vehicle.is_destroyed());
    assert_eq!(
        resolve_battle_vehicle_inferno_hit(&mut replay, id, 2, VehicleImpactRules::STANDARD)
            .unwrap(),
        report
    );
    assert_eq!(replay.btech, world.btech);
    let mut dice = Dice::seeded(seed);
    for _ in 0..4 {
        dice.two_d6();
    }
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn inferno_criticals_stop_once_the_vehicle_is_destroyed_and_safe_targets_take_none() {
    let (_dir, config, base, id) = fixture(false).await;
    // Missile one: right side (5), table roll 12 (a 10 at −2: Engine Hit on a Demolisher's
    // side, moving down from Weapon Destroyed). Missile two: right side (5), table roll 12
    // again, now Fuel Tank with the engine gone, which explodes the vehicle.
    let seed = matching_seed(|dice| {
        dice.two_d6() == 5 && dice.two_d6() == 12 && dice.two_d6() == 5 && dice.two_d6() == 12
    });
    let mut world = base.clone();
    edit(&mut world, id, |unit| {
        unit["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap()
    });
    let report =
        resolve_battle_vehicle_inferno_hit(&mut world, id, 5, VehicleImpactRules::STANDARD)
            .unwrap();
    assert_eq!(report.criticals.len(), 2, "no rolls after destruction");
    assert_eq!(
        report.criticals[0].critical.selection.effect,
        Some(VehicleCriticalEffect::Engine)
    );
    assert_eq!(
        report.criticals[1].critical.selection.effect,
        Some(VehicleCriticalEffect::FuelTank)
    );
    assert!(report.criticals[1].critical.unit_destroyed);
    assert!(world.btech.vehicles()[&id].is_destroyed());
    world.validate(&config).unwrap();

    let mut world = base.clone();
    let before = world.btech.clone();
    let mut safe = VehicleImpactRules::STANDARD;
    safe.criticals.combat_safe = true;
    let report = resolve_battle_vehicle_inferno_hit(&mut world, id, 3, safe).unwrap();
    assert!(report.criticals.is_empty());
    assert_eq!(world.btech, before);
    assert!(resolve_battle_vehicle_inferno_hit(&mut world, id, 0, safe).is_err());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn stationary_jelly_and_mobile_heat_preserve_distinct_rules() {
    let (_dir, config, base, id) = fixture(true).await;
    let mut world = base.clone();
    let report =
        resolve_battle_vehicle_inferno_hit(&mut world, id, 3, VehicleImpactRules::STANDARD)
            .unwrap();
    assert_eq!(report.burn_seconds, 360);
    assert!(report.criticals.is_empty());
    let _ = resolve_battle_vehicle_inferno_hit(&mut world, id, 1, VehicleImpactRules::STANDARD)
        .unwrap();
    assert_eq!(world.btech.vehicles()[&id].inferno_remaining(), 540);
    let map = world.btech.vehicles()[&id].position().unwrap().map;
    assert!(battle_hex_illuminated(&world, map, HexCoordinate { x: 0, y: 1 }).unwrap());
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    for _ in 0..539 {
        assert!(
            advance_battle_vehicle_fires(&mut restored, &config)
                .unwrap()
                .notices
                .is_empty()
        );
    }
    let notices = advance_battle_vehicle_fires(&mut restored, &config).unwrap();
    assert!(
        notices
            .notices
            .iter()
            .any(|notice| notice.text.contains("fires finally die"))
    );
    assert_eq!(restored.btech, base.btech);
    assert!(!battle_hex_illuminated(&restored, map, HexCoordinate { x: 0, y: 1 }).unwrap());

    // Blast heat: jelly for a stationary unit, nothing for a mobile vehicle.
    for stationary in [false, true] {
        let (_dir, config, base, id) = fixture(stationary).await;
        for heat in [0, 2] {
            let mut world = base.clone();
            let before = world.clone();
            let result = resolve_battle_vehicle_heat_exposure(
                &mut world,
                id,
                heat,
                VehicleImpactRules::STANDARD,
            )
            .unwrap();
            assert_eq!(
                result.burn_seconds,
                if stationary { i64::from(heat) * 6 } else { 0 }
            );
            assert_eq!(
                world.btech.vehicles()[&id].inferno_remaining(),
                if stationary { heat as u32 * 6 } else { 0 }
            );
            assert_eq!(
                world.btech.vehicles()[&id].sections(),
                before.btech.vehicles()[&id].sections()
            );
            let mut replay = before;
            assert_eq!(
                resolve_battle_vehicle_heat_exposure(
                    &mut replay,
                    id,
                    heat,
                    VehicleImpactRules::STANDARD
                )
                .unwrap(),
                result
            );
            assert_eq!(world.btech, replay.btech);
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn vehicle_destruction_cancels_jelly() {
    let (_dir, config, mut world, id) = fixture(true).await;
    let _ = resolve_battle_vehicle_inferno_hit(&mut world, id, 3, VehicleImpactRules::STANDARD)
        .unwrap();
    assert_eq!(world.btech.vehicles()[&id].inferno_remaining(), 360);
    let _ = damage_battle_vehicle_phase(
        &mut world,
        id,
        VehicleSection::Front,
        u16::MAX,
        DamagePhase::Internal,
    )
    .unwrap();
    assert!(world.btech.vehicles()[&id].is_destroyed());
    assert_eq!(world.btech.vehicles()[&id].inferno_remaining(), 0);
    let before = world.btech.clone();
    assert!(
        resolve_battle_vehicle_inferno_hit(&mut world, id, 1, VehicleImpactRules::STANDARD)
            .is_err()
    );
    assert_eq!(world.btech, before);
    world.validate(&config).unwrap();
}

/// Inferno criticals reach the pilot's private feedback only through the host action.
#[tokio::test]
async fn inferno_actions_publish_critical_feedback_once() {
    let (_dir, config, mut base, id) = fixture_movement(VehicleMovement::Vtol).await;
    base.objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    // Front (7), table roll 8 (a 6 at −2: Co-Pilot Hit).
    let seed = matching_seed(|dice| dice.two_d6() == 7 && dice.two_d6() == 8);
    edit(&mut base, id, |unit| {
        unit["dice"] = serde_json::to_value(Dice::seeded(seed)).unwrap()
    });
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
    let report = resolve_battle_vehicle_inferno_hit_action(
        &scripts,
        &config,
        id,
        1,
        VehicleImpactRules::STANDARD,
    )
    .unwrap();
    assert_eq!(
        report.criticals[0].critical.selection.effect,
        Some(VehicleCriticalEffect::VtolCopilot)
    );
    let output = scripts.drain_outbox();
    assert_eq!(
        output
            .iter()
            .filter(|(who, message)| *who == ObjectId(1)
                && message.to_string().contains("co-pilot"))
            .count(),
        1
    );
    assert_eq!(scripts.world().btech.vehicles()[&id].gunnery_damage(), 1);
}
