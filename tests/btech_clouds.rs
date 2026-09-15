//! Cloud boundaries share optical policy across every supported unit pairing.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// Raise only the target's terrain hex, keeping both units valid at their natural ground heights.
fn raised_target(world: &mut World, map: ObjectId, target: ObjectId) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["terrain"][10]["elevation"] = 1.into();
    let key = if world.btech.vehicles().contains_key(&target) {
        "vehicles"
    } else {
        "constructed"
    };
    let unit = &mut state[key][target.0.to_string()];
    unit["ground_elevation"] = serde_json::Value::Null;
    if !unit["vtol_flight"].is_null() {
        unit["vtol_flight"]["altitude"] = 1.into();
    }
    world.btech = serde_json::from_value(state).unwrap();
}

/// Optical contact is obstructed across the boundary, including equality, independently of chassis.
#[tokio::test]
async fn cloud_boundary_filters_all_supported_unit_pairs() {
    for source in firing::templates() {
        for target in firing::templates() {
            let (_dir, config, mut world, observer, target, _) =
                firing::fixture_with_target(&source, None, &target).await;
            let map = world.btech.units()[&observer].map.unwrap();
            raised_target(&mut world, map, target);
            for sensor in [
                BattleSensorMode::Visual,
                BattleSensorMode::LightAmplification,
                BattleSensorMode::Infrared,
            ] {
                set_battle_map_cloud_base(&mut world, ObjectId(1), map, 0).unwrap();
                let ordinary =
                    battle_map_optical_contact(&world, observer, target, sensor, false, false)
                        .unwrap();
                assert!(ordinary.eligible, "{sensor:?}");
                set_battle_map_cloud_base(&mut world, ObjectId(1), map, 1).unwrap();
                for (a, b) in [(observer, target), (target, observer)] {
                    let blocked =
                        battle_map_optical_contact(&world, a, b, sensor, false, false).unwrap();
                    assert!(!blocked.eligible);
                    assert_eq!(blocked.acquisition_factor, 0);
                    assert!(
                        !battle_optical_contact(
                            &world,
                            a,
                            b,
                            sensor,
                            BattleSensorConditions {
                                light: BattleLight::Day,
                                visibility: 30,
                                disabled: false,
                                target_lit: false
                            }
                        )
                        .unwrap()
                        .eligible
                    );
                }
                for base in [-1, 0, 2, 200] {
                    set_battle_map_cloud_base(&mut world, ObjectId(1), map, base).unwrap();
                    assert_eq!(
                        battle_map_optical_contact(&world, observer, target, sensor, false, false)
                            .unwrap(),
                        ordinary
                    );
                }
            }
            for sensor in [BattleSensorMode::Radar, BattleSensorMode::Electromagnetic] {
                set_battle_map_cloud_base(&mut world, ObjectId(1), map, 0).unwrap();
                let before =
                    battle_map_optical_contact(&world, observer, target, sensor, false, false)
                        .unwrap();
                set_battle_map_cloud_base(&mut world, ObjectId(1), map, 1).unwrap();
                assert_eq!(
                    battle_map_optical_contact(&world, observer, target, sensor, false, false)
                        .unwrap(),
                    before
                );
            }
            world.validate(&config).unwrap();
        }
    }
}

/// Native and Lua controls share authority, callback rollback, signed limits and database ownership.
#[tokio::test]
async fn cloud_controls_persist_and_roll_back() {
    let (_dir, config, world, id, _, _) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        None,
        include_str!("../game/mechs/Demolisher"),
    )
    .await;
    let map = world.btech.units()[&id].map.unwrap();
    assert_eq!(world.btech.maps()[&map].cloud_base, 200);
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for altitude in [-32768, 0, 1, 32767] {
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.map.cloud_base(1,{},{}); error('abort cloud')",
                map.0, altitude
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before);
        let value: i16 = lua
            .eval_callback(&format!(
                "return btech.map.cloud_base(1,{},{})",
                map.0, altitude
            ))
            .unwrap();
        assert_eq!(value, altitude);
        let output = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("@btech map-cloud #{}={altitude}", map.0),
        );
        assert!(output.contains("cloud base saved"), "{output}");
        assert_eq!(native.world().btech, lua.world().btech);
        let saved = lua.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
    for bad in ["32768", "-32769", "'invalid'"] {
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!("btech.map.cloud_base(1,{}, {bad})", map.0))
                .is_err()
        );
        assert_eq!(lua.world().btech, before);
    }
    lua.world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let before = lua.world().btech.clone();
    assert!(
        lua.eval_callback::<()>(&format!("btech.map.cloud_base(2,{},0)", map.0))
            .is_err()
    );
    assert_eq!(lua.world().btech, before);
}

/// Terrain visibility preserves the distinct mixed-sensor cloud rule for every supported chassis.
#[tokio::test]
async fn terrain_clouds_preserve_pair_and_equality_rules_without_consuming_dice() {
    for source in firing::templates() {
        let (_dir, config, mut initial, id, _, _) =
            firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D")).await;
        let map = initial.btech.units()[&id].map.unwrap();
        // Both units stand at elevation two, so target terrain introduces no hill obstruction.
        let mut state = serde_json::to_value(&initial.btech).unwrap();
        for tile in state["maps"][map.0.to_string()]["terrain"]
            .as_array_mut()
            .unwrap()
        {
            tile["elevation"] = 2.into();
        }
        for class in ["constructed", "vehicles"] {
            for unit in state[class].as_object_mut().unwrap().values_mut() {
                unit["ground_elevation"] = serde_json::Value::Null;
                if !unit["vtol_flight"].is_null() {
                    unit["vtol_flight"]["altitude"] = 2.into();
                }
            }
        }
        initial.btech = serde_json::from_value(state).unwrap();
        let target = BattleHexCoordinate { x: 0, y: 9 };
        for pair in [
            BattleSensorPair {
                primary: BattleSensorMode::Visual,
                secondary: BattleSensorMode::Visual,
            },
            BattleSensorPair {
                primary: BattleSensorMode::Visual,
                secondary: BattleSensorMode::Infrared,
            },
            BattleSensorPair {
                primary: BattleSensorMode::Electromagnetic,
                secondary: BattleSensorMode::Visual,
            },
        ] {
            let mut world = initial.clone();
            firing::edit(&mut world, id, |state| {
                state["sensor_selection"]["active"] = serde_json::to_value(pair).unwrap()
            });
            set_battle_map_cloud_base(&mut world, ObjectId(1), map, 0).unwrap();
            let ordinary = battle_hex_sensor_visibility(&world, id, target).unwrap();
            assert!(ordinary.primary || ordinary.secondary);
            for base in [-1, 0, 1, 2, 3] {
                set_battle_map_cloud_base(&mut world, ObjectId(1), map, base).unwrap();
                let before = world.btech.clone();
                let sensors = battle_hex_sensor_visibility(&world, id, target).unwrap();
                let blocked = base != 0 && base < 2 && pair.primary != pair.secondary;
                assert_eq!(battle_hex_visible(&world, id, target).unwrap(), !blocked);
                assert_eq!(
                    sensors,
                    if blocked {
                        BattleContactSensors::default()
                    } else {
                        ordinary
                    }
                );
                assert_eq!(world.btech, before);
            }
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                battle_hex_sensor_visibility(&restored, id, target).unwrap(),
                ordinary
            );
        }
    }
}

/// Terrain firing and read-only aim share cloud admission through both player interfaces.
#[tokio::test]
async fn terrain_cloud_admission_matches_native_lua_and_preserves_failed_shots() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, _, index) = firing::fixture_with_target(
            &source,
            Some(BattleWeapon::MediumLaser),
            include_str!("../game/mechs/AS7-D"),
        )
        .await;
        let map = world.btech.units()[&id].map.unwrap();
        firing::edit(&mut world, id, |state| {
            state["sensor_selection"]["active"] = serde_json::to_value(BattleSensorPair {
                primary: BattleSensorMode::Visual,
                secondary: BattleSensorMode::Infrared,
            })
            .unwrap()
        });
        set_battle_map_cloud_base(&mut world, ObjectId(1), map, -1).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let visible: bool = lua
            .eval_callback(&format!(
                "return btech.unit.aim_hex({},{},0,9).visible",
                id.0, index
            ))
            .unwrap();
        assert!(!visible);
        let call = format!("btech.unit.fire({},1,{},{{x=0,y=9}})", id.0, index);
        assert!(lua.eval_callback::<()>(&call).is_err());
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire {index} 0 9"),
        );
        assert!(text.contains("Target hex is not visible"), "{text}");
        assert_eq!(lua.world().btech, world.btech);
        assert_eq!(native.world().btech, world.btech);
        let mut privileged = world.clone();
        firing::edit(&mut privileged, id, |state| {
            state["visibility"]["clairvoyant"] = true.into()
        });
        assert!(battle_hex_visible(&privileged, id, BattleHexCoordinate { x: 0, y: 9 }).unwrap());
        set_battle_map_cloud_base(&mut world, ObjectId(1), map, 0).unwrap();
        *lua.world_mut() = world.clone();
        *native.world_mut() = world.clone();
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort shot')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        lua.eval_callback::<()>(&call).unwrap();
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire {index} 0 9"),
        );
        assert!(text.contains("You fire"), "{text}");
        assert_eq!(native.world().btech, lua.world().btech);
        lua.world().validate(&config).unwrap();
    }
}
