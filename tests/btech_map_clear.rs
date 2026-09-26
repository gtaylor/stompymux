//! Bulk map clearing shares placement cleanup, shutdown rules and atomic host consequences.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Keep the operator in the map room while cockpit pilots remain assigned.
fn operator(world: &mut World, config: &Config, map: ObjectId) -> ObjectId {
    let actor = world.create(config, "Map operator".into(), Kind::Player);
    world
        .objects
        .get_mut(&actor)
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    world.objects.get_mut(&actor).unwrap().location = Some(map);
    actor
}

/// All chassis, including unpiloted running members, share native/Lua removal and durable slot reuse.
#[tokio::test]
async fn clear_map_units_shares_chassis_shutdown_and_restart() {
    for (chassis, source) in firing::templates().into_iter().enumerate() {
        for mode in ["off", "starting", "running", "moving", "airborne"] {
            if mode == "airborne" && ![0, 2, 4, 6].contains(&chassis) {
                continue;
            }
            if mode == "moving" && chassis == 5 {
                continue;
            }
            let (_dir, config, mut world, id, target, _) =
                firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D"))
                    .await;
            let map = world.btech.units()[&id].map.unwrap();
            let actor = operator(&mut world, &config, map);
            firing::edit(&mut world, target, |unit| unit["map_slot"] = 0.into());
            firing::edit(&mut world, id, |unit| {
                unit["map_slot"] = 1.into();
                unit["target_lock"] = serde_json::Value::Null;
                unit["power"] = serde_json::to_value(match mode {
                    "off" => BattlePower::Off,
                    "starting" => BattlePower::Starting { remaining: 10 },
                    _ => BattlePower::Running,
                })
                .unwrap();
                if mode == "moving" {
                    unit["motion"]["speed"] = 21.5.into();
                }
                if mode == "airborne" && chassis != 0 {
                    if chassis == 6 {
                        unit["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
                        unit["vtol_flight"]["altitude"] = 3.into();
                    } else {
                        unit["ground_elevation"] = 3.into();
                    }
                }
            });
            if mode == "airborne" && chassis == 0 {
                launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
            }
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            let before = world.btech.clone();
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let mut remote_world = world.clone();
            remote_world.objects.get_mut(&actor).unwrap().location = Some(ObjectId(0));
            let remote = Scripts::new(&config, Rc::new(RefCell::new(remote_world))).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let call = format!("btech.map.clear_units({},{})", actor.0, map.0);
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort clear')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            let ids = lua
                .eval_callback::<Vec<i64>>(&format!("return {call}"))
                .unwrap();
            assert_eq!(ids, vec![target.0, id.0]);
            let output = support::run_text(&native, &config, actor, 1, "clearmechs ignored");
            assert!(output.contains("Map Cleared"), "{chassis} {mode}: {output}");
            assert_eq!(native.world().btech, lua.world().btech);
            let remote_output = support::run_text(
                &remote,
                &config,
                actor,
                1,
                &format!("shutdown {} ignored", map.0),
            );
            assert!(remote_output.contains("Map Cleared"), "{remote_output}");
            assert_eq!(remote.world().btech, native.world().btech);
            assert_eq!(remote.world().objects[&actor].location, Some(ObjectId(0)));

            let saved = native.world().clone();
            let mut expected_map = serde_json::to_value(&before.maps()[&map]).unwrap();
            expected_map["membership_extent"] = 0.into();
            assert_eq!(
                serde_json::to_value(&saved.btech.maps()[&map]).unwrap(),
                expected_map
            );
            for unit in [id, target] {
                assert_eq!(saved.objects[&unit].location, Some(map));
                assert_eq!(saved.btech.units()[&unit].map, None);
                let key = if saved.btech.vehicles().contains_key(&unit) {
                    "vehicles"
                } else {
                    "constructed"
                };
                let state = serde_json::to_value(&saved.btech).unwrap();
                let state = &state[key][unit.0.to_string()];
                assert!(state["position"].is_null() && state["map_slot"].is_null());
                assert!(state["pilot"].is_null() && state["motion"].is_null());
                assert_eq!(state["power"]["state"], "off");
            }
            assert_eq!(saved.objects[&ObjectId(1)].location, Some(id));
            saved.validate(&config).unwrap();
            persistence::save(&config.database(), &saved).await.unwrap();
            let mut reloaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(reloaded.btech, saved.btech);
            place_battle_unit(&mut reloaded, id, map, 0, 11).unwrap();
            let slot = reloaded
                .btech
                .vehicles()
                .get(&id)
                .map(|u| u.map_slot())
                .unwrap_or_else(|| reloaded.btech.constructed_units()[&id].map_slot());
            assert_eq!(slot, Some(0));
        }
    }
}

/// A failure after earlier membership changes restores the entire pass and its queued output.
#[tokio::test]
async fn clear_map_units_authority_empty_map_and_output_rollback() {
    let (dir, config, mut world, id, target, _) = firing::fixture_with_target(
        &firing::templates()[0],
        None,
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    let map = world.btech.units()[&id].map.unwrap();
    let actor = operator(&mut world, &config, map);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    // Off units produce only operator reports, so the second report exceeds the limit.
    for unit in [id, target] {
        firing::edit(&mut world, unit, |unit| {
            unit["power"] = serde_json::to_value(BattlePower::Off).unwrap();
            unit["target_lock"] = serde_json::Value::Null;
        });
    }
    let original_config = config.clone();
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
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(clear_battle_map_units_action(&scripts, &config, ObjectId(2), map).is_err());
    assert!(clear_battle_map_units_action(&scripts, &config, actor, map).is_err());
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    let config = original_config;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(scripts.world().clone()))).unwrap();
    clear_battle_map_units_action(&scripts, &config, actor, map).unwrap();
    scripts.drain_outbox();
    let before = scripts.world().btech.clone();
    assert!(
        clear_battle_map_units_action(&scripts, &config, actor, map)
            .unwrap()
            .is_empty()
    );
    assert_eq!(scripts.world().btech, before);
    assert_eq!(scripts.drain_outbox().len(), 1);

    // An empty map can retain allocation after individual sparse removals.
    // Bulk clear must release it even when there are no units to visit.
    let mut encoded = serde_json::to_value(&scripts.world().btech).unwrap();
    encoded["maps"][map.0.to_string()]["membership_extent"] = 7.into();
    scripts.world_mut().btech = serde_json::from_value(encoded).unwrap();
    scripts.world().validate(&config).unwrap();
    let allocated = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.map.clear_units({},{}); error('abort empty clear')",
                actor.0, map.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, allocated);
    assert!(scripts.drain_outbox().is_empty());
    assert!(
        clear_battle_map_units_action(&scripts, &config, actor, map)
            .unwrap()
            .is_empty()
    );
    assert_eq!(scripts.world().btech, before);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}

/// Removing either end first releases the pair, including a falling aircraft's external load.
#[tokio::test]
async fn clear_map_tows_in_either_slot_order() {
    for source in [
        firing::templates()[0].clone(),
        firing::templates()[6].clone(),
    ] {
        for load_first in [false, true] {
            let (_dir, config, mut world, id, target, _) =
                firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D"))
                    .await;
            let map = world.btech.units()[&id].map.unwrap();
            let actor = operator(&mut world, &config, map);
            firing::edit(&mut world, target, |unit| {
                unit["power"] = serde_json::to_value(BattlePower::Off).unwrap();
                unit["target_lock"] = serde_json::Value::Null;
            });
            place_battle_unit(&mut world, target, map, 0, 11).unwrap();
            if world.btech.vehicles().contains_key(&id) {
                firing::edit(&mut world, id, |unit| {
                    unit["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
                    unit["vtol_flight"]["altitude"] = 5.into();
                });
            }
            set_battle_tow(&mut world, id, Some(target)).unwrap();
            if load_first {
                firing::edit(&mut world, target, |unit| unit["map_slot"] = 0.into());
                firing::edit(&mut world, id, |unit| unit["map_slot"] = 1.into());
            }
            world.validate(&config).unwrap();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let removed = clear_battle_map_units_action(&scripts, &config, actor, map).unwrap();
            assert_eq!(
                removed,
                if load_first {
                    vec![target, id]
                } else {
                    vec![id, target]
                }
            );
            let saved = scripts.world().clone();
            assert!(saved.btech.tows().is_empty());
            saved.validate(&config).unwrap();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
    }
}

/// Explicit map shutdown must neither bypass wizard authority nor change bare cockpit shutdown.
#[tokio::test]
async fn selected_map_shutdown_is_guarded_and_bare_shutdown_remains_local() {
    let (_dir, config, world, source, target, _) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        None,
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    let map = world.btech.units()[&source].map.unwrap();
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for input in [
        "shutdown nope".to_owned(),
        "shutdown -1".into(),
        "shutdown 9223372036854775808".into(),
    ] {
        assert!(
            !support::run_text(&scripts, &config, ObjectId(1), 1, &input).contains("Map Cleared")
        );
        assert_eq!(scripts.world().btech, before);
    }
    let denied = support::run_text(
        &scripts,
        &config,
        ObjectId(4),
        4,
        &format!("shutdown {}", map.0),
    );
    assert!(denied.contains("Permission denied"), "{denied}");
    assert_eq!(scripts.world().btech, before);
    let output = support::run_text(&scripts, &config, ObjectId(1), 1, "shutdown");
    assert!(!output.contains("Map Cleared"));
    assert_eq!(scripts.world().btech.units()[&source].map, Some(map));
    assert_eq!(scripts.world().btech.units()[&target].map, Some(map));
    assert_eq!(
        scripts.world().btech.constructed_units()[&source].power(),
        BattlePower::Off
    );
    assert_eq!(
        scripts.world().btech.constructed_units()[&target],
        before.constructed_units()[&target]
    );
}
