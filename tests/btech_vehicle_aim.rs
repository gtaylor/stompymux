//! Vehicle attack movement and shared target modifiers follow saved tactical state.
use crate::support;
use stompymux_rs::*;

/// A running vehicle with a present pilot, initially disconnected from a session.
async fn fixture(template: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Test field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("test", template).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

/// Install a valid instantaneous speed without advancing into neighboring terrain.
fn motion(world: &mut World, id: ObjectId, speed: f64, turning: bool) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let motion = &mut state["vehicles"][id.0.to_string()]["motion"];
    motion["speed"] = serde_json::json!(speed);
    motion["desired_speed"] = serde_json::json!(speed.clamp(-86.0 * 2.0 / 3.0, 86.0));
    motion["desired_heading"] = serde_json::json!(if turning { 90.0 } else { 0.0 });
    world.btech = serde_json::from_value(state).unwrap();
}

#[tokio::test]
async fn vehicle_attack_and_target_movement_follow_speed_turning_and_replay() {
    let (_dir, config, mut world, id) =
        fixture(include_str!("../game/mechs/Flatbed_Truck.toml")).await;
    for (speed, target, attack) in [
        (0.0, 0, 0),
        (21.5, 0, 1),
        (21.501, 1, 1),
        (43.0, 1, 1),
        (43.001, 2, 1),
        (86.0 * 2.0 / 3.0 + 0.1, 2, 1),
        (57.434, 2, 2),
        (64.5, 2, 2),
        (64.501, 3, 2),
        (86.0, 3, 2),
        (96.75, 3, 2),
        (-43.001, 2, 1),
        (-86.0 * 2.0 / 3.0, 2, 1),
    ] {
        for turning in [false, true] {
            motion(&mut world, id, speed, turning);
            let checkpoint = world.btech.clone();
            let vehicle = &world.btech.vehicles()[&id];
            assert_eq!(vehicle.attacker_movement_modifier(false), attack);
            assert_eq!(
                vehicle.attacker_movement_modifier(true),
                if attack == 2 {
                    2
                } else {
                    attack + u8::from(turning)
                }
            );
            for extended in [false, true] {
                assert_eq!(
                    battle_unit_target_movement_modifier(&world, id, 1.0, extended).unwrap(),
                    target
                );
            }
            assert_eq!(world.btech, checkpoint);
        }
    }
    motion(&mut world, id, 50.0, false);
    damage_battle_vehicle_motive(
        &mut world,
        id,
        BattleVehicleMotiveHit::SpeedLoss { movement_points: 3 },
    )
    .unwrap();
    // Damage lowers the throttle limit, while the firing threshold remains construction-based.
    assert_eq!(world.btech.vehicles()[&id].maximum_speed(), 53.75);
    assert_eq!(
        world.btech.vehicles()[&id].attacker_movement_modifier(false),
        1
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(
        battle_unit_target_movement_modifier(&loaded, id, 5.0, true).unwrap(),
        2
    );
}

#[tokio::test]
async fn vehicle_immobility_stacks_with_speed_without_treating_stopped_as_disabled() {
    let (_dir, _config, mut world, id) =
        fixture(include_str!("../game/mechs/Flatbed_Truck.toml")).await;
    assert_eq!(
        battle_unit_target_movement_modifier(&world, id, 0.0, false).unwrap(),
        0
    );
    motion(&mut world, id, 43.0, false);
    let moving = world.clone();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["recoveries"]["1"] = serde_json::json!({
        "mode":{"kind":"tactical","injuries":4},"remaining":30,
        "pain_resistance":false,"toughness":false,"dice":BattleDice::seeded([0;32])
    });
    world.btech = serde_json::from_value(state).unwrap();
    assert_eq!(
        battle_unit_target_movement_modifier(&world, id, 10.0, false).unwrap(),
        -3
    );
    world = moving.clone();
    damage_battle_vehicle_motive(&mut world, id, BattleVehicleMotiveHit::Immobilize).unwrap();
    assert_eq!(
        battle_unit_target_movement_modifier(&world, id, 10.0, false).unwrap(),
        -4
    );
    world = moving.clone();
    damage_battle_vehicle_motive(
        &mut world,
        id,
        BattleVehicleMotiveHit::SpeedLoss {
            movement_points: 255,
        },
    )
    .unwrap();
    assert_eq!(
        battle_unit_target_movement_modifier(&world, id, 10.0, false).unwrap(),
        0
    );
    world = moving;
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert_eq!(
        battle_unit_target_movement_modifier(&world, id, 10.0, false).unwrap(),
        -4
    );
    let checkpoint = world.btech.clone();
    for distance in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(battle_unit_target_movement_modifier(&world, id, distance, false).is_err());
    }
    assert!(battle_unit_target_movement_modifier(&world, ObjectId(-1), 1.0, false).is_err());
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert!(battle_unit_target_movement_modifier(&world, id, 1.0, false).is_err());
    assert_eq!(world.btech, checkpoint);
}

#[tokio::test]
async fn stationary_construction_and_turret_rotation_use_distinct_modifiers() {
    let (_dir, config, mut world, id) =
        fixture(include_str!("../game/mechs/RadioTower.toml")).await;
    assert_eq!(world.btech.vehicles()[&id].power(), BattlePower::Running);
    assert_eq!(
        battle_unit_target_movement_modifier(&world, id, 2.0, false).unwrap(),
        -4
    );
    assert_eq!(
        world.btech.vehicles()[&id].attacker_movement_modifier(true),
        0
    );
    let checkpoint = world.btech.clone();
    assert!(set_battle_speed(&mut world, id, ObjectId(1), 1.0).is_err());
    assert!(set_battle_heading(&mut world, id, ObjectId(1), 90.0).is_err());
    assert_eq!(world.btech, checkpoint);
    set_battle_turret(&mut world, id, ObjectId(1), 90.0).unwrap();
    let cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(loaded.btech.vehicles()[&id].turret_heading(), Some(90.0));
    for _ in 0..cycle.weapon.profile().recycle_seconds {
        assert_eq!(
            advance_battle_recycle(&mut world),
            advance_battle_recycle(&mut loaded)
        );
        assert_eq!(loaded.btech, world.btech);
    }
    assert!(
        loaded.btech.vehicles()[&id]
            .weapon_readiness(0)
            .unwrap()
            .ready
    );
    let (_dir, _config, mut world, id) =
        fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    set_battle_turret(&mut world, id, ObjectId(1), 90.0).unwrap();
    assert_eq!(
        world.btech.vehicles()[&id].attacker_movement_modifier(true),
        0
    );
    set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
    assert_eq!(
        world.btech.vehicles()[&id].attacker_movement_modifier(true),
        1
    );
}
