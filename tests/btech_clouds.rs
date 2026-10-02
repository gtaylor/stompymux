//! Cloud boundaries cut sensors and sight across every supported unit pairing and terrain hex.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Raise only the target's terrain hex, keeping both units valid at their natural ground heights.
fn raised_target(world: &mut World, map: ObjectId, target: ObjectId) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    crate::support::set_hex_elevation(&mut state["maps"][map.0.to_string()]["terrain"][10], 1);
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

/// Sensors and sight are cut across the boundary, including equality, independently of chassis.
#[tokio::test]
async fn cloud_boundary_filters_all_supported_unit_pairs() {
    for source in firing::templates() {
        for target in firing::templates() {
            let (_dir, config, mut world, observer, target, _) =
                firing::fixture_with_target(&source, None, &target).await;
            let map = world.btech.units()[&observer].map.unwrap();
            raised_target(&mut world, map, target);
            // With the band switched off, sight carries the same clear-line rule.
            for sensors in [true, false] {
                set_battle_map_perception(
                    &mut world,
                    map,
                    BattleMapPerceptionFlag::Sensors,
                    sensors,
                )
                .unwrap();
                set_battle_map_cloud_base(&mut world, ObjectId(1), map, 0).unwrap();
                let ordinary = battle_perceive(&world, observer, target).unwrap();
                assert_eq!(
                    ordinary.map(|perception| perception.channel),
                    Some(if sensors {
                        BattleDetectionChannel::Sensors
                    } else {
                        BattleDetectionChannel::Sight
                    })
                );
                set_battle_map_cloud_base(&mut world, ObjectId(1), map, 1).unwrap();
                for (a, b) in [(observer, target), (target, observer)] {
                    assert_eq!(battle_perceive(&world, a, b).unwrap(), None);
                }
                for base in [-1, 0, 2, 200] {
                    set_battle_map_cloud_base(&mut world, ObjectId(1), map, base).unwrap();
                    assert_eq!(battle_perceive(&world, observer, target).unwrap(), ordinary);
                }
            }
            world.validate(&config).unwrap();
        }
    }
}

/// Native and Lua controls share authority, callback rollback, signed limits and database ownership.
#[tokio::test]
async fn cloud_controls_persist_and_roll_back() {
    let (_dir, config, world, id, _, _) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D.toml"),
        None,
        include_str!("../game/mechs/Demolisher.toml"),
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

/// Raise one empty terrain hex a level above the flat lane.
fn raised_hex(world: &mut World, map: ObjectId, index: usize) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    crate::support::set_hex_elevation(&mut state["maps"][map.0.to_string()]["terrain"][index], 1);
    world.btech = serde_json::from_value(state).unwrap();
}

/// Terrain visibility is cut only when the boundary separates the observer and hex levels.
#[tokio::test]
async fn terrain_clouds_follow_level_and_equality_rules_without_consuming_dice() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D.toml"))
                .await;
        let map = world.btech.units()[&id].map.unwrap();
        // The observer stands at level zero and looks at an empty hex raised to level one.
        raised_hex(&mut world, map, 9);
        let target = BattleHexCoordinate { x: 0, y: 9 };
        set_battle_map_cloud_base(&mut world, ObjectId(1), map, 0).unwrap();
        let ordinary = battle_hex_perception(&world, id, target).unwrap();
        assert_eq!(ordinary, Some(BattleDetectionChannel::Sensors));
        for base in [-1, 0, 1, 2, 3] {
            set_battle_map_cloud_base(&mut world, ObjectId(1), map, base).unwrap();
            let before = world.btech.clone();
            let blocked = base == 1;
            assert_eq!(battle_hex_visible(&world, id, target).unwrap(), !blocked);
            assert_eq!(
                battle_hex_perception(&world, id, target).unwrap(),
                if blocked { None } else { ordinary },
                "base {base}"
            );
            assert_eq!(world.btech, before);
        }
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            battle_hex_perception(&restored, id, target).unwrap(),
            ordinary
        );
    }
}

/// Terrain firing and read-only aim share cloud admission through both player interfaces.
#[tokio::test]
async fn terrain_cloud_admission_matches_native_lua_and_preserves_failed_shots() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, _, index) = firing::fixture_with_target(
            &source,
            Some(BattleWeapon::MediumLaser),
            include_str!("../game/mechs/AS7-D.toml"),
        )
        .await;
        let map = world.btech.units()[&id].map.unwrap();
        raised_hex(&mut world, map, 9);
        set_battle_map_cloud_base(&mut world, ObjectId(1), map, 1).unwrap();
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
