//! Persistent motive damage limits live vehicle speed and survives restarts without changing construction.
use crate::support;
use stompymux_rs::*;

/// A running wheeled vehicle in a long, level corridor.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Road".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "road",
        MapAsset::from_cells(&format!(
            "20 3\n{}\n{}\n{}\n",
            ".0".repeat(20),
            ".0".repeat(20),
            ".0".repeat(20)
        ))
        .unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Truck".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        VehicleTemplate::parse(
            "Flatbed_Truck",
            include_str!("../game/units/Flatbed_Truck.toml"),
        )
        .unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 2, 1).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id, map)
}

#[tokio::test]
async fn motive_speed_loss_clamps_controls_and_replays_damaged_movement() {
    for reverse in [false, true] {
        let (_dir, config, mut world, id, _) = fixture().await;
        set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
        for _ in 0..10 {
            advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
        }
        let desired = if reverse { -86.0 * 2.0 / 3.0 } else { 86.0 };
        set_battle_speed(&mut world, id, ObjectId(1), desired).unwrap();
        for _ in 0..20 {
            advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
        }
        let original = world.btech.vehicles()[&id].clone();
        damage_battle_vehicle_motive(
            &mut world,
            id,
            VehicleMotiveHit::SpeedLoss { movement_points: 2 },
        )
        .unwrap();
        let vehicle = &world.btech.vehicles()[&id];
        assert_eq!(vehicle.definition(), original.definition());
        assert_eq!(vehicle.sections(), original.sections());
        assert_eq!(vehicle.motive_speed_loss(), 21.5);
        assert_eq!(vehicle.maximum_speed(), 64.5);
        assert_eq!(
            vehicle.motion().unwrap().speed,
            if reverse { -43.0 } else { 64.5 }
        );
        assert_eq!(
            vehicle.motion().unwrap().desired_speed,
            if reverse { -43.0 } else { 64.5 }
        );
        let before = world.btech.clone();
        assert!(set_battle_speed(&mut world, id, ObjectId(1), desired).is_err());
        assert_eq!(world.btech, before);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        for _ in 0..5 {
            assert_eq!(
                advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap(),
                advance_battle_motion(&mut loaded, MovementRules::STANDARD).unwrap()
            );
            assert_eq!(loaded.btech, world.btech);
        }
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        scripts
            .eval_callback::<()>(&format!("btech.unit.speed({},1,'flank')", id.0))
            .unwrap();
        assert_eq!(
            scripts.world().btech.vehicles()[&id]
                .motion()
                .unwrap()
                .desired_speed,
            64.5
        );
        let state: (f64,f64,bool) = scripts.eval_callback(&format!("local u=btech.unit.state({}); return u.maximum_speed,u.motive_speed_loss,u.immobilized",id.0)).unwrap();
        assert_eq!(state, (64.5, 21.5, false));
    }
}

#[tokio::test]
async fn immobilization_and_exhausted_speed_stop_translation_and_turning_after_restart() {
    for hit in [
        VehicleMotiveHit::Immobilize,
        VehicleMotiveHit::SpeedLoss {
            movement_points: 255,
        },
    ] {
        let (_dir, config, mut world, id, _) = fixture().await;
        set_battle_speed(&mut world, id, ObjectId(1), 86.0).unwrap();
        set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
        advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
        damage_battle_vehicle_motive(&mut world, id, hit).unwrap();
        assert_eq!(world.btech.vehicles()[&id].maximum_speed(), 0.0);
        assert_eq!(
            world.btech.vehicles()[&id].immobilized(),
            matches!(hit, VehicleMotiveHit::Immobilize)
        );
        assert!(!world.btech.vehicles()[&id].motion().unwrap().active());
        assert!(set_battle_speed(&mut world, id, ObjectId(1), 1.0).is_err());
        assert!(set_battle_heading(&mut world, id, ObjectId(1), 180.0).is_err());
        let before = world.btech.clone();
        advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
        assert_eq!(before, world.btech);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, world.btech);
        assert!(set_battle_heading(&mut loaded, id, ObjectId(1), 180.0).is_err());
        let before = loaded.btech.clone();
        loaded
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::Going);
        assert!(damage_battle_vehicle_motive(&mut loaded, id, hit).is_err());
        assert_eq!(before, loaded.btech);
    }
}

#[tokio::test]
async fn motive_snapshots_reject_impossible_limits_and_immobile_motion() {
    let (_dir, _config, mut world, id, _) = fixture().await;
    set_battle_speed(&mut world, id, ObjectId(1), 86.0).unwrap();
    let original = serde_json::to_value(&world.btech.vehicles()[&id]).unwrap();
    for loss in [-1.0, 87.0] {
        let mut bad = original.clone();
        bad["motive_speed_loss"] = loss.into();
        assert!(serde_json::from_value::<Vehicle>(bad).is_err());
    }
    let mut bad = original.clone();
    bad["immobilized"] = true.into();
    assert!(serde_json::from_value::<Vehicle>(bad).is_err());
    let mut bad = original;
    bad["motive_speed_loss"] = 10.75.into();
    // A saved throttle can retain the prior environmental ceiling until the next tick.
    let retained = serde_json::from_value::<Vehicle>(bad.clone()).unwrap();
    assert_eq!(retained.motion().unwrap().desired_speed, 86.0);
    bad["motion"]["desired_speed"] = 10_000.0.into();
    assert!(serde_json::from_value::<Vehicle>(bad).is_err());
    let before = world.btech.clone();
    damage_battle_vehicle_motive(
        &mut world,
        id,
        VehicleMotiveHit::SpeedLoss { movement_points: 0 },
    )
    .unwrap();
    assert_eq!(world.btech, before);
}
