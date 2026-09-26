//! Station navigation uses shared chassis geometry, independent selections and guarded occupants.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
use crate::support;

/// A station with the same settled hex selection as its parent, on any supported chassis.
async fn fixture(
    template: &str,
) -> (
    tempfile::TempDir,
    Config,
    World,
    ObjectId,
    ObjectId,
    ObjectId,
    ObjectId,
) {
    let (dir, config, mut world, parent, target, _) =
        firing::fixture_with_target(template, Some(BattleWeapon::MediumLaser), template).await;
    let station = world.create(&config, "Station".into(), Kind::Thing);
    let gunner = world.create(&config, "Gunner".into(), Kind::Player);
    world.objects.get_mut(&gunner).unwrap().location = Some(station);
    register_gunner_station(&mut world, ObjectId(1), station, parent, 0).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    gunner_station_action(&scripts, station, gunner, true).unwrap();
    for (owner, actor) in [(parent, ObjectId(1)), (station, gunner)] {
        select_battle_hex_target(
            &mut scripts.world_mut(),
            owner,
            actor,
            BattleHexCoordinate { x: 0, y: 9 },
            BattleHexTargetMode::Hex,
        )
        .unwrap();
    }
    let world = scripts.world().clone();
    (dir, config, world, parent, target, station, gunner)
}

/// Native, Rust and Lua measurements use the same live geometry and never alter combat state.
#[tokio::test]
async fn station_measurements_share_all_chassis_geometry_and_restart() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, _target, station, gunner) = fixture(&template).await;
        for speed in [0.0, 10.75, -21.5] {
            if speed != 0.0
                && world.btech.vehicles().get(&parent).is_some_and(|unit| {
                    unit.definition().movement == BattleVehicleMovement::Stationary
                })
            {
                continue;
            }
            // ETA uses actual signed speed, without running a movement tick or route calculation.
            firing::edit(&mut world, parent, |state| {
                state["motion"]["speed"] = speed.into()
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            for (command, method) in [
                ("bearing", "bearing"),
                ("range", "range_report"),
                ("vector", "vector"),
                ("eta", "eta"),
                ("findcenter", "findcenter"),
            ] {
                for arguments in ["", "0 8"] {
                    let expected: mlua::Table = scripts
                        .eval_callback(&format!(
                            "return btech.unit.{method}({},1,'{arguments}')",
                            parent.0
                        ))
                        .unwrap();
                    let actual: mlua::Table = scripts
                        .eval_callback(&format!(
                            "return btech.gunner.{method}({},{},'{arguments}')",
                            station.0, gunner.0
                        ))
                        .unwrap();
                    assert_eq!(
                        serde_json::to_value(actual).unwrap(),
                        serde_json::to_value(expected).unwrap(),
                        "{command}"
                    );
                    scripts.drain_outbox();
                    assert_eq!(
                        support::run_text_for_player(
                            &scripts,
                            &config,
                            gunner,
                            1,
                            &format!("{command} {arguments}")
                        ),
                        support::run_text_for_player(
                            &scripts,
                            &config,
                            ObjectId(1),
                            1,
                            &format!("{command} {arguments}")
                        )
                    );
                    assert_eq!(scripts.world().btech, world.btech);
                }
            }
        }
        // Stationary platforms cannot persist a fabricated moving speed.
        firing::edit(&mut world, parent, |state| {
            state["motion"]["speed"] = 0.0.into()
        });
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            battle_bearing(&restored, station, gunner, "").unwrap(),
            battle_bearing(&world, station, gunner, "").unwrap()
        );
        assert_eq!(
            battle_eta(&restored, station, gunner, "").unwrap(),
            battle_eta(&world, station, gunner, "").unwrap()
        );
    }
}

/// Retargeting and releasing a station never substitutes a parent lock or grants piloting rights.
#[tokio::test]
async fn station_measurements_preserve_selection_and_authority() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, _target, station, gunner) = fixture(&template).await;
        select_battle_hex_target(
            &mut world,
            parent,
            ObjectId(1),
            BattleHexCoordinate { x: 0, y: 11 },
            BattleHexTargetMode::Hex,
        )
        .unwrap();
        let before = world.btech.clone();
        assert_ne!(
            battle_bearing(&world, parent, ObjectId(1), "")
                .unwrap()
                .bearing,
            battle_bearing(&world, station, gunner, "").unwrap().bearing
        );
        assert_eq!(
            battle_bearing(&world, parent, gunner, "").unwrap(),
            battle_bearing(&world, station, gunner, "").unwrap()
        );
        assert_eq!(
            battle_eta(&world, station, gunner, "").unwrap().coordinate,
            BattleHexCoordinate { x: 0, y: 9 }
        );
        assert!(battle_bearing(&world, station, ObjectId(1), "").is_err());
        assert!(find_battle_hex_center(&world, parent, ObjectId(2)).is_err());
        assert_eq!(world.btech, before);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        gunner_station_action(&scripts, station, gunner, false).unwrap();
        for method in ["bearing", "range_report", "vector", "eta", "findcenter"] {
            assert!(
                scripts
                    .eval_callback::<()>(&format!(
                        "btech.gunner.{method}({},{})",
                        station.0, gunner.0
                    ))
                    .is_err()
            );
        }
        let text = support::run_text(&scripts, &config, gunner, 1, "shutdown");
        assert!(!text.contains("shutting down"), "{text}");
    }
}

/// Darkness uses parent altitude, contact locks use parent sensors, and unavailable controls reject reads.
#[tokio::test]
async fn station_measurements_use_parent_sensors_and_health() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, target, station, gunner) = fixture(&template).await;
        let map = world.btech.units()[&parent].map.unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["maps"][map.0.to_string()]["flags"] = 32.into();
        world.btech = serde_json::from_value(state.clone()).unwrap();
        assert_eq!(
            battle_range_report(&world, station, gunner, "0 8").unwrap(),
            battle_range_report(&world, parent, ObjectId(1), "0 8").unwrap()
        );
        state["maps"][map.0.to_string()]["flags"] = 0.into();
        world.btech = serde_json::from_value(state).unwrap();
        select_battle_target(&mut world, station, gunner, Some(target)).unwrap();
        assert!(battle_bearing(&world, station, gunner, "").is_ok());
        assert!(battle_range_report(&world, station, gunner, "").is_ok());
        assert!(battle_eta(&world, station, gunner, "").is_err());
        let baseline = world.clone();
        for condition in ["unconscious", "parent_removed"] {
            let mut world = baseline.clone();
            match condition {
                "unconscious" => {
                    let mut state = serde_json::to_value(&world.btech).unwrap();
                    state["recoveries"][gunner.0.to_string()] = serde_json::json!({"remaining":1,"pain_resistance":false,"toughness":false,"dice":BattleDice::seeded([34;32])});
                    world.btech = serde_json::from_value(state).unwrap();
                }
                _ => {
                    world
                        .objects
                        .get_mut(&parent)
                        .unwrap()
                        .flags
                        .insert(Flag::Going);
                }
            }
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            for method in ["bearing", "range_report", "vector", "eta", "findcenter"] {
                assert!(
                    scripts
                        .eval_callback::<()>(&format!(
                            "btech.gunner.{method}({},{},'0 8')",
                            station.0, gunner.0
                        ))
                        .is_err(),
                    "{condition}: {method}"
                );
                assert_eq!(scripts.world().btech, world.btech);
                assert!(scripts.drain_outbox().is_empty());
            }
        }
    }
}

/// ETA output joins the host transaction and cannot leave partial notices after rejection.
#[tokio::test]
async fn station_eta_publication_failure_is_atomic() {
    let (dir, _config, world, _parent, _target, station, gunner) =
        fixture(&firing::templates()[0]).await;
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
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let query = format!("btech.gunner.eta({},{})", station.0, gunner.0);
    assert!(
        scripts
            .eval_callback::<()>(&format!("{query}; {query}"))
            .is_err()
    );
    assert_eq!(scripts.world().btech, world.btech);
    assert!(scripts.drain_outbox().is_empty());
}

/// All three renderers share physical-unit geometry and exact native/Lua output across chassis and modes.
#[tokio::test]
async fn station_maps_share_renderers_without_changing_targets_or_dice() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, target, station, gunner) = fixture(&template).await;
        select_battle_target(&mut world, parent, ObjectId(1), None).unwrap();
        for ansi in [false, true] {
            for actor in [ObjectId(1), gunner] {
                let flags = &mut world.objects.get_mut(&actor).unwrap().flags;
                if ansi {
                    flags.insert(Flag::Ansi);
                } else {
                    flags.remove(Flag::Ansi);
                }
            }
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            for center in [
                String::new(),
                format!("#{}", target.0),
                "0 1".into(),
                "180 -1".into(),
            ] {
                for (method, mode) in [
                    ("navigate", ""),
                    ("tactical", ""),
                    ("tactical", "L"),
                    ("tactical", "U"),
                    ("tactical", "C"),
                    ("tactical", "T"),
                    ("tactical", "B"),
                    ("tactical", "M"),
                    ("lrsmap", "T"),
                    ("lrsmap", "E"),
                    ("lrsmap", "C"),
                    ("lrsmap", "M"),
                    ("lrsmap", "L"),
                    ("lrsmap", "H"),
                    ("lrsmap", "S"),
                ] {
                    let args = if method == "lrsmap" {
                        format!("'{mode}','{center}'")
                    } else {
                        format!("'{}'", format!("{mode} {center}").trim())
                    };
                    let expected: mlua::Table = scripts
                        .eval_callback(&format!(
                            "return btech.unit.{method}({},1,{args})",
                            parent.0
                        ))
                        .unwrap();
                    let query = format!("btech.gunner.{method}({},{},{args})", station.0, gunner.0);
                    let actual: mlua::Table =
                        scripts.eval_callback(&format!("return {query}")).unwrap();
                    assert_eq!(
                        serde_json::to_value(actual).unwrap(),
                        serde_json::to_value(expected).unwrap(),
                        "{method} {mode} {center}"
                    );
                    let command = format!("{method} {mode} {center}");
                    assert_eq!(
                        support::run_text(&scripts, &config, gunner, 1, &command),
                        support::run_text(&scripts, &config, ObjectId(1), 1, &command)
                    );
                    assert_eq!(scripts.world().btech, world.btech);
                    assert!(
                        scripts
                            .eval_callback::<()>(&format!("{query}; error('abort')"))
                            .is_err()
                    );
                    assert_eq!(scripts.world().btech, world.btech);
                    assert!(scripts.drain_outbox().is_empty());
                }
            }
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            battle_navigate(&restored, station, gunner, "").unwrap(),
            battle_navigate(&world, station, gunner, "").unwrap()
        );
    }
}

/// Station map dimensions and ANSI belong to the gunner, while darkness and equipment belong to the parent.
#[tokio::test]
async fn station_maps_apply_viewer_preferences_and_parent_visibility() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, _target, station, gunner) = fixture(&template).await;
        let dimensions = BattleViewDimensions {
            tactical_width: 5,
            tactical_height: 5,
            long_range_height: 10,
        };
        set_battle_view_dimensions(&mut world, gunner, dimensions).unwrap();
        world
            .objects
            .get_mut(&gunner)
            .unwrap()
            .flags
            .insert(Flag::Ansi);
        let map = world.btech.units()[&parent].map.unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["maps"][map.0.to_string()]["flags"] = 32.into();
        world.btech = serde_json::from_value(state).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let expected = battle_tactical_map(&world, parent, gunner, "L", dimensions).unwrap();
        let actual: mlua::Table = scripts
            .eval_callback(&format!(
                "return btech.gunner.tactical({},{},'L')",
                station.0, gunner.0
            ))
            .unwrap();
        assert_eq!(
            serde_json::to_value(actual).unwrap(),
            serde_json::to_value(expected).unwrap()
        );
        assert_ne!(
            battle_view_dimensions(&world, ObjectId(1)).unwrap(),
            dimensions
        );
        for mode in ["B", "C", "T"] {
            assert!(
                scripts
                    .eval_callback::<()>(&format!(
                        "btech.gunner.tactical({},{},'{mode}')",
                        station.0, gunner.0
                    ))
                    .unwrap_err()
                    .to_string()
                    .contains("can't see that much")
            );
        }
        let map: mlua::Table = scripts
            .eval_callback(&format!(
                "return btech.gunner.lrsmap({},{},'S')",
                station.0, gunner.0
            ))
            .unwrap();
        assert_eq!(
            serde_json::to_value(map).unwrap(),
            serde_json::to_value(
                battle_long_range_map(
                    &world,
                    parent,
                    gunner,
                    BattleLongRangeMode::VisibleUnits,
                    "",
                    dimensions
                )
                .unwrap()
            )
            .unwrap()
        );
        assert_eq!(scripts.world().btech, world.btech);
    }
}

/// Failed scanner hardware blocks tactical/LRS displays but preserves the local navigation exception.
#[tokio::test]
async fn station_maps_preserve_hardware_and_control_guards() {
    let (_dir, config, mut world, parent, _target, station, gunner) =
        fixture(&firing::templates()[0]).await;
    firing::edit(
        &mut world,
        parent,
        |state| {
            state["lost_criticals"] =
                serde_json::json!([{"section":"Head","slot":1},{"section":"Head","slot":4}])
        },
    );
    assert_eq!(
        world.btech.constructed_units()[&parent]
            .sensor_ranges()
            .tactical,
        0
    );
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let navigation = format!("btech.gunner.navigate({},{})", station.0, gunner.0);
    assert!(
        scripts
            .eval_callback::<mlua::Table>(&format!("return {navigation}"))
            .is_ok()
    );
    for call in [
        format!("btech.gunner.tactical({},{})", station.0, gunner.0),
        format!("btech.gunner.lrsmap({},{},'T')", station.0, gunner.0),
    ] {
        assert!(
            scripts
                .eval_callback::<()>(&call)
                .unwrap_err()
                .to_string()
                .contains("inoperational")
        );
    }
    gunner_station_action(&scripts, station, gunner, false).unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&navigation)
            .unwrap_err()
            .to_string()
            .contains("initialized")
    );
}
