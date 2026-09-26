//! Wizard coordinate edits preserve live controls, clear stale observations and replay airborne state.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Both interfaces retain crew, heading, speed, map slot and outgoing selection across every chassis.
#[tokio::test]
async fn setxy_native_lua_geometry_guards_and_restart() {
    for source in firing::templates() {
        let (_dir, config, mut world, unit, other, _) =
            firing::fixture_with_target(&source, None, &source).await;
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(other);
        assign_battle_pilot(&mut world, other, ObjectId(2)).unwrap();
        refresh_battle_contacts(&mut world, &[other]).unwrap();
        select_battle_target(&mut world, other, ObjectId(2), Some(unit)).unwrap();
        let fixed =
            world.btech.vehicles().get(&unit).is_some_and(|unit| {
                unit.definition().movement == BattleVehicleMovement::Stationary
            });
        firing::edit(&mut world, unit, |state| {
            state["motion"]["heading"] = 15.0.into();
            state["motion"]["desired_heading"] = if fixed { 15.0 } else { 45.0 }.into();
            state["motion"]["speed"] = if fixed { 0.0 } else { 1.0 }.into();
            state["motion"]["desired_speed"] = if fixed { 0.0 } else { 2.0 }.into();
        });
        world.validate(&config).unwrap();
        for elevation in [None, Some(30), Some(-5), Some(i32::MAX), Some(i32::MIN)] {
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let value = elevation.map_or("nil".into(), |value| value.to_string());
            let report: mlua::Table = lua
                .eval_callback(&format!(
                    "return btech.unit.setxy(1, {}, 0, 9, {value})",
                    unit.0
                ))
                .unwrap();
            let report = serde_json::to_value(report).unwrap();
            let expected = elevation
                .unwrap_or(0)
                .clamp(i32::from(i16::MIN), i32::from(i16::MAX));
            assert_eq!(report["elevation"], expected);
            let suffix = elevation.map_or(String::new(), |value| format!(" {value}"));
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("setxy 0 9{suffix}"),
            );
            assert!(
                text.contains(&format!("Pos changed to 0,9,{expected}")),
                "{text}"
            );
            assert!(text.contains("lock has been lost"), "{text}");
            assert_eq!(native.world().btech, lua.world().btech);
            let before = serde_json::to_value(&world.btech).unwrap();
            let after = serde_json::to_value(&native.world().btech).unwrap();
            let group = if world.btech.vehicles().contains_key(&unit) {
                "vehicles"
            } else {
                "constructed"
            };
            for field in [
                "pilot",
                "power",
                "map_slot",
                "target_lock",
                "dice",
                "sections",
            ] {
                assert_eq!(
                    before[group][unit.0.to_string()][field],
                    after[group][unit.0.to_string()][field],
                    "{field}"
                );
            }
            for field in ["heading", "desired_heading", "speed", "desired_speed"] {
                assert_eq!(
                    before[group][unit.0.to_string()]["motion"][field],
                    after[group][unit.0.to_string()]["motion"][field],
                    "{field}"
                );
            }
            assert!(after[group][other.0.to_string()]["target_lock"].is_null());
            assert!(
                after[group][unit.0.to_string()]["contacts"]
                    .as_object()
                    .unwrap()
                    .is_empty()
            );
            let saved = native.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        for args in [
            "",
            "0",
            "0 9 0 extra",
            "-1 9",
            "1 9",
            "0 12",
            "0 9 false",
            "2147483648 9",
        ] {
            let text =
                support::run_text(&scripts, &config, ObjectId(1), 1, &format!("setxy {args}"));
            assert!(!text.is_empty());
            assert_eq!(scripts.world().btech, world.btech);
        }
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.setxy(1, {}, 0, 9, 20); error('abort')",
                    unit.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, world.btech);
        assert!(scripts.drain_outbox().is_empty());
        let visitor = scripts
            .world_mut()
            .create(&config, "Visitor".into(), Kind::Player);
        assert!(
            set_battle_coordinates_action(
                &scripts,
                &config,
                visitor,
                unit,
                BattleScenarioPosition {
                    coordinate: BattleHexCoordinate { x: 0, y: 9 },
                    elevation: None,
                }
            )
            .is_err()
        );
    }
}

/// Jump relocation preserves the planned route and elapsed distance, then resumes on the next tick.
#[tokio::test]
async fn setxy_preserves_jump_progress_and_forced_descent_clock() {
    let source = &firing::templates()[0];
    let (_dir, config, mut world, unit, _, _) =
        firing::fixture_with_target(source, None, source).await;
    launch_battle_jump(&mut world, unit, ObjectId(1), 0, 3.0).unwrap();
    let before = world.btech.constructed_units()[&unit].flight().unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    set_battle_coordinates_action(
        &scripts,
        &config,
        ObjectId(1),
        unit,
        BattleScenarioPosition {
            coordinate: BattleHexCoordinate { x: 0, y: 9 },
            elevation: Some(20),
        },
    )
    .unwrap();
    let flight = scripts.world().btech.constructed_units()[&unit]
        .flight()
        .unwrap();
    assert_eq!(flight.path(), before.path());
    assert_eq!(flight.travelled(), before.travelled());
    assert_eq!(
        flight.sample().point,
        BattleHexCoordinate { x: 0, y: 9 }.center()
    );
    assert_eq!(flight.sample().elevation, 20.0);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, saved.btech);
    let mut original = saved.clone();
    advance_battle_jumps(&mut original, BattleMovementRules::STANDARD).unwrap();
    advance_battle_jumps(&mut restored, BattleMovementRules::STANDARD).unwrap();
    assert_eq!(original.btech, restored.btech);
    stop_battle_unit(
        &mut restored,
        unit,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let before = restored.btech.constructed_units()[&unit]
        .free_fall()
        .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
    set_battle_coordinates_action(
        &scripts,
        &config,
        ObjectId(1),
        unit,
        BattleScenarioPosition {
            coordinate: BattleHexCoordinate { x: 0, y: 7 },
            elevation: Some(25),
        },
    )
    .unwrap();
    let after = scripts.world().btech.constructed_units()[&unit]
        .free_fall()
        .unwrap();
    assert_eq!(after.elevation(), 25);
    let before = serde_json::to_value(before).unwrap();
    let after = serde_json::to_value(after).unwrap();
    assert_eq!(before["speed"], after["speed"]);
    assert_eq!(before["remaining"], after["remaining"]);
}

/// Explicit altitude preserves VTOL flight; selecting the surface lands without spending dice.
#[tokio::test]
async fn setxy_vtol_flight_and_atomic_notification_failure() {
    let source = &firing::templates()[6];
    let (dir, config, mut world, unit, other, _) =
        firing::fixture_with_target(source, None, source).await;
    let _ = begin_battle_vtol_takeoff(&mut world, unit, ObjectId(1), 0, false).unwrap();
    advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
    assert_eq!(
        world.btech.vehicles()[&unit].vtol_flight().unwrap().phase,
        BattleVtolFlightPhase::Airborne
    );
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    set_battle_coordinates_action(
        &scripts,
        &config,
        ObjectId(1),
        unit,
        BattleScenarioPosition {
            coordinate: BattleHexCoordinate { x: 0, y: 9 },
            elevation: Some(30),
        },
    )
    .unwrap();
    let flight = scripts.world().btech.vehicles()[&unit]
        .vtol_flight()
        .unwrap();
    assert_eq!(flight.phase, BattleVtolFlightPhase::Airborne);
    assert_eq!(flight.altitude, 30.0);
    let mut falling = scripts.world().clone();
    stop_battle_unit(
        &mut falling,
        unit,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let before_fall = falling.btech.vehicles()[&unit]
        .vtol_flight()
        .unwrap()
        .fall
        .unwrap();
    let falling = Scripts::new(&config, Rc::new(RefCell::new(falling))).unwrap();
    set_battle_coordinates_action(
        &falling,
        &config,
        ObjectId(1),
        unit,
        BattleScenarioPosition {
            coordinate: BattleHexCoordinate { x: 0, y: 8 },
            elevation: Some(40),
        },
    )
    .unwrap();
    let flight = falling.world().btech.vehicles()[&unit]
        .vtol_flight()
        .unwrap();
    assert_eq!(flight.phase, BattleVtolFlightPhase::Falling);
    assert_eq!(flight.altitude, 40.0);
    assert_eq!(flight.fall.unwrap().elevation(), 40);
    assert_eq!(
        serde_json::to_value(before_fall).unwrap()["remaining"],
        serde_json::to_value(flight.fall.unwrap()).unwrap()["remaining"]
    );
    set_battle_coordinates_action(
        &scripts,
        &config,
        ObjectId(1),
        unit,
        BattleScenarioPosition {
            coordinate: BattleHexCoordinate { x: 0, y: 8 },
            elevation: None,
        },
    )
    .unwrap();
    let flight = scripts.world().btech.vehicles()[&unit]
        .vtol_flight()
        .unwrap();
    assert_eq!(flight.phase, BattleVtolFlightPhase::Landed);
    assert_eq!(flight.altitude, 0.0);
    // Create an incoming lock so its publication succeeds before the confirmation fails.
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(other);
    assign_battle_pilot(&mut world, other, ObjectId(2)).unwrap();
    refresh_battle_contacts(&mut world, &[other]).unwrap();
    select_battle_target(&mut world, other, ObjectId(2), Some(unit)).unwrap();
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("lua")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_entry_limit".into(), 2.into());
    std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    // Without a contact the observer hears nothing, so only the confirmation is published.
    let mut contacts = serde_json::Value::Null;
    firing::edit(&mut scripts.world_mut(), other, |state| {
        contacts = state["contacts"].clone();
        state["contacts"] = serde_json::json!({});
    });
    battle_losemit_action(&scripts, ObjectId(1), unit, "first").unwrap();
    firing::edit(&mut scripts.world_mut(), other, |state| {
        state["contacts"] = contacts
    });
    let before = scripts.world().btech.clone();
    let error = set_battle_coordinates_action(
        &scripts,
        &config,
        ObjectId(1),
        unit,
        BattleScenarioPosition {
            coordinate: BattleHexCoordinate { x: 0, y: 9 },
            elevation: Some(30),
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("output limit"), "{error:#}");
    assert_eq!(scripts.world().btech, before);
    assert_eq!(scripts.drain_outbox().len(), 1);
}

/// Either member can be named while attached units move together across mixed anatomies.
#[tokio::test]
async fn setxy_moves_complete_tow_pairs() {
    let templates = firing::templates();
    for carrier in &templates {
        for target in &templates {
            let (_dir, config, mut world) = support::isolated_world().await;
            let map = world.create(&config, "Yard".into(), Kind::Room);
            create_battle_map(
                &mut world,
                map,
                "yard",
                BattleMapAsset::parse("2 2\n.0.0\n.0.0\n").unwrap(),
            )
            .unwrap();
            let a = world.create(&config, "Carrier".into(), Kind::Thing);
            let b = world.create(&config, "Tow".into(), Kind::Thing);
            for (id, source) in [(a, carrier), (b, target)] {
                world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
                BattleUnitTemplate::parse(source)
                    .unwrap()
                    .create(&mut world, id)
                    .unwrap();
                place_battle_unit(&mut world, id, map, 0, 0).unwrap();
            }
            set_battle_tow(&mut world, a, Some(b)).unwrap();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            for id in [a, b] {
                set_battle_coordinates_action(
                    &scripts,
                    &config,
                    ObjectId(1),
                    id,
                    BattleScenarioPosition {
                        coordinate: BattleHexCoordinate { x: 1, y: 1 },
                        elevation: Some(20),
                    },
                )
                .unwrap();
                assert_eq!(scripts.world().btech.tows().get(&a), Some(&b));
                for id in [a, b] {
                    assert_eq!(
                        battle_unit_elevation(&scripts.world(), id).unwrap(),
                        Some(20)
                    );
                }
                scripts.world().validate(&config).unwrap();
            }
        }
    }
}
