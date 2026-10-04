//! Predictive firing reuses ordinary launch state, validates boundaries and rolls back Lua failures.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Every supported shooter uses identical native/Lua prediction and queued artillery transactions.
#[tokio::test]
async fn predictive_fire_shares_launching_and_callback_rollback_across_chassis() {
    for source in firing::templates() {
        let target_source = include_str!("../game/mechs/JR7-D.toml");
        let (_dir, config, mut world, shooter, target, weapon) = firing::fixture_with_supply(
            &source,
            Some(Weapon::ThumperCannon),
            target_source,
            false,
            Some(""),
        )
        .await;
        firing::edit(&mut world, target, |unit| {
            unit["motion"]["speed"] = 107.5.into();
            unit["motion"]["desired_speed"] = 107.5.into();
        });
        let before = world.btech.clone();
        let prediction =
            predict_battle_artillery_target(&world, shooter, target, MovementRules::STANDARD)
                .unwrap();
        assert_eq!(prediction.seconds, 10);
        assert!(prediction.coordinate.y < 10);
        assert!(!prediction.stopped);
        assert_eq!(world.btech, before);
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let unlaunched = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.snipe({},1,{},'{weapon}'); error('abort launched shot')",
                shooter.0, target.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, unlaunched);
        let output = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("snipe #{} {weapon}", target.0),
        );
        assert!(output.contains("You fire ThumperCannon"), "{output}");
        lua.eval_callback::<()>(&format!(
            "btech.unit.snipe({},1,{},'{weapon}')",
            shooter.0, target.0
        ))
        .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        assert_eq!(
            lua.world()
                .btech
                .maps()
                .values()
                .map(|map| map.artillery_shots().len())
                .sum::<usize>(),
            1
        );
        let committed = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.snipe({},1,{},'{weapon}'); error('abort')",
                shooter.0, target.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, committed);
        assert!(
            battle_snipe_action(
                &lua,
                &config,
                shooter,
                ObjectId(2),
                target,
                &weapon.to_string()
            )
            .is_err()
        );
        assert_eq!(lua.world().btech, committed);
        let snapshot = lua.world().clone();
        persistence::save(&config.database(), &snapshot)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            committed
        );
    }
}

/// Forecasting cannot leave the map or mutate the live target while finding an interception point.
#[tokio::test]
async fn prediction_stops_at_map_edge_without_consuming_live_state() {
    let source = include_str!("../game/mechs/JR7-D.toml");
    let (_dir, _config, mut world, shooter, target, _) =
        firing::fixture_with_target(source, None, source).await;
    firing::edit(&mut world, target, |unit| {
        unit["motion"]["heading"] = 180.0.into();
        unit["motion"]["desired_heading"] = 180.0.into();
        unit["motion"]["speed"] = 107.5.into();
        unit["motion"]["desired_speed"] = 107.5.into();
    });
    let before = world.btech.clone();
    let prediction =
        predict_battle_artillery_target(&world, shooter, target, MovementRules::STANDARD).unwrap();
    assert!(prediction.stopped);
    assert_eq!(prediction.coordinate, HexCoordinate { x: 0, y: 11 });
    assert_eq!(
        prediction.point.containing_hex().unwrap(),
        prediction.coordinate
    );
    assert_eq!(world.btech, before);
    assert!(
        predict_battle_artillery_target(&world, shooter, ObjectId(-1), MovementRules::STANDARD)
            .is_err()
    );
}

/// All admitted target chassis use the same bounded terrain-stop policy, including fixed platforms.
#[tokio::test]
async fn prediction_handles_vehicle_targets_and_blocking_terrain() {
    let source = include_str!("../game/mechs/JR7-D.toml");
    for target_source in firing::templates() {
        let (_dir, config, mut world, shooter, target, _) =
            firing::fixture_with_target(source, None, &target_source).await;
        let stationary = world
            .btech
            .vehicles()
            .get(&target)
            .is_some_and(|v| v.definition().movement == VehicleMovement::Stationary);
        if !stationary {
            firing::edit(&mut world, target, |unit| {
                unit["motion"]["speed"] = 40.0.into();
                unit["motion"]["desired_speed"] = 40.0.into();
            });
        }
        let map = world.btech.units()[&shooter].map.unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_map_hex_action(
            &scripts,
            &config,
            ObjectId(1),
            map,
            HexCoordinate { x: 0, y: 9 },
            Hex::new(Terrain::Mountains, 9),
        )
        .unwrap();
        let world = scripts.world();
        let before = world.btech.clone();
        let prediction =
            predict_battle_artillery_target(&world, shooter, target, MovementRules::STANDARD)
                .unwrap();
        // A sheer cliff stops every ground target; aircraft fly over it.
        let vtol = world
            .btech
            .vehicles()
            .get(&target)
            .is_some_and(|v| v.definition().movement == VehicleMovement::Vtol);
        assert_eq!(prediction.stopped, !vtol);
        if !vtol {
            assert_eq!(prediction.coordinate.y, if stationary { 10 } else { 9 });
        }
        assert_eq!(world.btech, before);
    }
}

/// Interception beyond the minimum flight time agrees with committed constant-order movement.
#[tokio::test]
async fn distant_prediction_matches_live_motion_until_shell_catches_up() {
    let source = include_str!("../game/mechs/JR7-D.toml");
    let (_dir, config, mut world, shooter, target, _) =
        firing::fixture_with_target(source, None, source).await;
    let map = world.create(&config, "Long artillery lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "long",
        MapAsset::from_cells(&format!("1 100\n{}", ".0\n".repeat(100))).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    for id in [shooter, target] {
        firing::edit(&mut world, id, |unit| {
            unit["power"] = serde_json::to_value(Power::Off).unwrap()
        });
    }
    place_battle_unit(&mut world, shooter, map, 0, 99).unwrap();
    place_battle_unit(&mut world, target, map, 0, 10).unwrap();
    for id in [shooter, target] {
        firing::edit(&mut world, id, |unit| {
            unit["power"] = serde_json::to_value(Power::Running).unwrap()
        });
    }

    firing::edit(&mut world, target, |unit| {
        unit["motion"]["speed"] = 107.5.into();
        unit["motion"]["desired_speed"] = 107.5.into();
    });
    let prediction =
        predict_battle_artillery_target(&world, shooter, target, MovementRules::STANDARD).unwrap();
    assert_eq!(prediction.seconds, 18);
    assert!(!prediction.stopped);
    for _ in 0..prediction.seconds {
        let _ = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
    }
    assert_eq!(
        world.btech.constructed_units()[&target]
            .motion()
            .unwrap()
            .point,
        prediction.point
    );
}
