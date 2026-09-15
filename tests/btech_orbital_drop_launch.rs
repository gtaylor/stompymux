//! Native and Lua orbital insertion share authorization, placement, restart and aircraft startup.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// Native and Lua use independent checkpoints over the identical initial world.
fn scripts(config: &Config, world: &World) -> Scripts {
    Scripts::new(config, Rc::new(RefCell::new(world.clone()))).unwrap()
}

/// Unit snapshots expose common insertion invariants for both anatomy stores.
fn snapshot(world: &World, id: ObjectId) -> serde_json::Value {
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        serde_json::to_value(unit).unwrap()
    } else {
        serde_json::to_value(&world.btech.vehicles()[&id]).unwrap()
    }
}

#[tokio::test]
async fn native_lua_insertion_matches_for_all_supported_chassis_and_restarts() {
    for source in firing::templates() {
        let (_dir, config, world, unit, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        for height in [None, Some(20), Some(-4), Some(i32::MAX), Some(i32::MIN)] {
            let native = scripts(&config, &world);
            let lua = scripts(&config, &world);
            let suffix = height.map_or(String::new(), |z| format!(" {z}"));
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("@ood 0 9{suffix}"),
            );
            assert!(text.contains("OOD initiated."), "{text}");
            let report: mlua::Table = lua
                .eval_callback(&format!(
                    "return btech.unit.ood(1, {}, 0, 9, {})",
                    unit.0,
                    height.map_or("nil".into(), |z| z.to_string())
                ))
                .unwrap();
            let report = serde_json::to_value(report).unwrap();
            let elevation = height
                .unwrap_or(300)
                .clamp(i32::from(i16::MIN), i32::from(i16::MAX));
            assert_eq!(report["elevation"], elevation);
            assert_eq!(native.world().btech, lua.world().btech);
            let after = snapshot(&native.world(), unit);
            let before = snapshot(&world, unit);
            assert_eq!(
                battle_unit_elevation(&native.world(), unit).unwrap(),
                Some(elevation)
            );
            for key in [
                "pilot",
                "power",
                "sections",
                "ammunition",
                "dice",
                "map_slot",
                "target_lock",
            ] {
                assert_eq!(after[key], before[key], "{key}");
            }
            if let Some(vehicle) = world
                .btech
                .vehicles()
                .get(&unit)
                .filter(|unit| unit.definition().is_vtol())
            {
                assert!(report["drop"].is_null());
                assert_eq!(after["vtol_flight"]["phase"]["kind"], "airborne");
                assert_eq!(
                    after["motion"]["desired_speed"],
                    vehicle.maximum_speed() / 2.0
                );
            } else {
                let mass = world
                    .btech
                    .constructed_units()
                    .get(&unit)
                    .map(|unit| unit.mass().unwrap().total)
                    .unwrap_or_else(|| world.btech.vehicles()[&unit].mass().unwrap().total);
                assert_eq!(report["drop"]["protection"]["integrity"], mass / 5120 + 1);
                assert!(after["ground_elevation"].is_null());
            }
            let saved = native.world().clone();
            saved.validate(&config).unwrap();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
    }
}

#[tokio::test]
async fn launch_authority_invalid_arguments_repeat_and_callback_failure_are_atomic() {
    let source = &firing::templates()[0];
    let (_dir, config, mut world, unit, _, _) =
        firing::fixture_with_target(source, None, source).await;
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let scripts = scripts(&config, &world);
    for code in [
        format!("btech.unit.ood(2, {}, 0, 9)", unit.0),
        format!("btech.unit.ood(1, {}, -1, 9)", unit.0),
        format!("btech.unit.ood(1, {}, 0, 12)", unit.0),
        format!("btech.unit.ood(1, {}, 0, 9); error('abort')", unit.0),
    ] {
        assert!(scripts.eval_callback::<()>(&code).is_err());
        assert_eq!(scripts.world().btech, world.btech);
        assert!(scripts.drain_outbox().is_empty());
    }
    for command in [
        "@ood",
        "@ood 0",
        "@ood x 9",
        "@ood 0 y",
        "@ood 0 9 z",
        "@ood/bad 0 9",
    ] {
        let text = support::run_text(&scripts, &config, ObjectId(1), 1, command);
        assert!(!text.contains("OOD initiated."), "{text}");
        assert_eq!(scripts.world().btech, world.btech);
    }
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "@ood 0 9 20 ignored");
    assert!(text.contains("OOD initiated."), "{text}");
    let before = scripts.world().btech.clone();
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "@ood 0 8");
    assert!(text.contains("OOD already in progress!"), "{text}");
    assert_eq!(scripts.world().btech, before);
}

#[tokio::test]
async fn unpowered_vtol_insertion_waits_for_startup_with_saved_controls() {
    let source = &firing::templates()[6];
    let (_dir, config, mut world, unit, _, _) =
        firing::fixture_with_target(source, None, source).await;
    firing::edit(&mut world, unit, |state| {
        state["power"] = serde_json::to_value(BattlePower::Off).unwrap();
        state["target_lock"] = serde_json::Value::Null;
    });
    let scripts = scripts(&config, &world);
    let _: mlua::Table = scripts
        .eval_callback(&format!("return btech.unit.ood(1, {}, 0, 9, 20)", unit.0))
        .unwrap();
    let mut waiting = scripts.world().clone();
    let grounded = Scripts::new(&config, Rc::new(RefCell::new(waiting.clone()))).unwrap();
    let _: mlua::Table = grounded
        .eval_callback(&format!("return btech.unit.setxy(1, {}, 0, 8)", unit.0))
        .unwrap();
    grounded.world().validate(&config).unwrap();
    let (phase, desired_speed) = {
        let world = grounded.world();
        let landed = &world.btech.vehicles()[&unit];
        (
            landed.vtol_flight().unwrap().phase,
            landed.motion().unwrap().desired_speed,
        )
    };
    assert_eq!(phase, BattleVtolFlightPhase::Landed);
    assert_eq!(
        desired_speed,
        waiting.btech.vehicles()[&unit]
            .motion()
            .unwrap()
            .desired_speed
    );
    let before = waiting.btech.clone();
    for _ in 0..5 {
        advance_battle_motion(&mut waiting, BattleMovementRules::STANDARD).unwrap();
    }
    assert_eq!(waiting.btech, before);
    assert_eq!(battle_unit_elevation(&waiting, unit).unwrap(), Some(20));
    persistence::save(&config.database(), &waiting)
        .await
        .unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    for world in [&mut waiting, &mut restored] {
        start_battle_unit(world, unit, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(world, 0);
        }
        advance_battle_motion(world, BattleMovementRules::STANDARD).unwrap();
        world.validate(&config).unwrap();
        assert!(world.btech.vehicles()[&unit].motion().unwrap().speed > 0.0);
        assert!(world.btech.vehicles()[&unit].orbital_drop().is_none());
    }
    assert_eq!(waiting.btech, restored.btech);
}

#[tokio::test]
async fn insertion_replaces_jump_and_rejects_prone_or_digging_units_without_relocation() {
    let source = &firing::templates()[0];
    let (_dir, config, mut world, unit, _, _) =
        firing::fixture_with_target(source, None, source).await;
    launch_battle_jump(&mut world, unit, ObjectId(1), 0, 2.0).unwrap();
    let scripts = scripts(&config, &world);
    let _: mlua::Table = scripts
        .eval_callback(&format!("return btech.unit.ood(1, {}, 0, 9, 20)", unit.0))
        .unwrap();
    assert!(
        scripts.world().btech.constructed_units()[&unit]
            .flight()
            .is_none()
    );
    assert!(
        scripts.world().btech.constructed_units()[&unit]
            .orbital_drop()
            .is_some()
    );
    scripts.world().validate(&config).unwrap();
    for index in [0, 2] {
        let source = &firing::templates()[index];
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(source, None, source).await;
        firing::edit(&mut world, unit, |state| {
            if index == 0 {
                state["posture"] = serde_json::to_value(BattlePosture::Prone).unwrap();
            } else {
                state["dig"] = serde_json::to_value(BattleDigState::preparing(10)).unwrap();
            }
        });
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.unit.ood(1, {}, 0, 9)", unit.0))
                .is_err()
        );
        assert_eq!(scripts.world().btech, world.btech);
        assert!(scripts.drain_outbox().is_empty());
    }
}

#[tokio::test]
async fn insertion_output_failure_restores_prior_pose_and_existing_messages() {
    let source = &firing::templates()[0];
    let (dir, _, world, unit, _, _) = firing::fixture_with_target(source, None, source).await;
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("lua")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_entry_limit".into(), 1.into());
    std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let scripts = scripts(&config, &world);
    set_battle_coordinates_action(
        &scripts,
        &config,
        ObjectId(1),
        unit,
        BattleScenarioPosition {
            coordinate: BattleHexCoordinate { x: 0, y: 9 },
            elevation: Some(10),
        },
    )
    .unwrap();
    let before = scripts.world().btech.clone();
    let error = initiate_battle_orbital_drop_action(
        &scripts,
        &config,
        ObjectId(1),
        unit,
        BattleScenarioPosition {
            coordinate: BattleHexCoordinate { x: 0, y: 8 },
            elevation: None,
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("output limit"), "{error:#}");
    assert_eq!(scripts.world().btech, before);
    let output = scripts.drain_outbox();
    assert_eq!(output.len(), 1);
    assert!(output[0].1.source().contains("Pos changed to 0,9,10"));
}
