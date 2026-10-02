//! Shared map-object selection, typed cleanup, native/Lua atomicity and restart.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

use crate::support::btech_map_objects as map_objects;
use map_objects::fixture;

#[tokio::test]
async fn native_lua_selectors_replay_and_rollback_all_owned_kinds() {
    for (args, lua_args, count) in [
        ("FIRE", "'FIRE'", 1),
        ("smoke 2 2", "'smoke',2,2", 1),
        ("DECO", "'DECO'", 1),
        ("MINE 1 1", "'MINE',1,1", 2),
        ("b", "'b'", 1),
        ("LEAVE 1 1", "'LEAVE',1,1", 1),
        ("ENTRA", "'ENTRA'", 1),
        ("LINKED 1 1", "'LINKED',1,1", 1),
        ("BLZ", "'BLZ'", 2),
        ("1 1", "nil,1,1", 10),
        ("-1 99", "nil,-1,99", 0),
    ] {
        let (_dir, config, world, map, interior) = fixture().await;
        let before = world.btech.clone();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let call = format!("btech.map.delete_objects(1,{},{lua_args})", map.0);
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(lua.drain_outbox().is_empty());
        let actual: usize = lua.eval_callback(&format!("return {call}")).unwrap();
        assert_eq!(actual, count, "{args}");
        let output = support::run_text(&native, &config, ObjectId(1), 1, &format!("delobj {args}"));
        assert!(output.contains(&format!("{count} ")), "{output}");
        assert_eq!(native.world().btech, lua.world().btech, "{args}");
        let saved = native.world().clone();
        if args == "1 1" || args == "b" {
            assert!(saved.btech.maps()[&interior].building_exits().is_empty());
        }
        if args == "1 1" || args == "LINKED 1 1" {
            assert!(saved.btech.maps()[&map].wrapping());
            assert_eq!(saved.btech.maps()[&map].linked_markers().len(), 1);
        }
        if args == "1 1" {
            assert_eq!(
                saved.btech.maps()[&map].hex(1, 1).unwrap().terrain(),
                Terrain::Water
            );
            assert!(!map_fire_pending(&saved));
        }
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

#[tokio::test]
async fn invalid_selectors_leave_definitions_intact() {
    let (_dir, config, world, map, _) = fixture().await;
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for input in [
        "delobj",
        "delobj invalid",
        "delobj MINE bad 1",
        "delobj MINE 1 1 extra",
        "delobj/no MINE",
    ] {
        support::run_text(&scripts, &config, ObjectId(1), 1, input);
        assert_eq!(scripts.world().btech, before, "{input}");
    }
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.map.delete_objects(1,{},nil,1)", map.0))
            .is_err()
    );
    assert!(
        delete_battle_map_objects_action(
            &scripts,
            &config,
            ObjectId(2),
            map,
            Some(BattleMapObjectKind::Mine),
            None
        )
        .is_err()
    );
    assert!(
        delete_battle_map_objects_action(&scripts, &config, ObjectId(1), map, None, None).is_err()
    );
    assert_eq!(scripts.world().btech, before);
}

/// Failed publication rolls back terrain restoration and reciprocal cleanup as well as selected records.
#[tokio::test]
async fn deletion_confirmation_failure_restores_every_kind() {
    let (dir, _, world, map, _) = fixture().await;
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("runtime")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_message_limit".into(), 1.into());
    std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(
        delete_battle_map_objects_action(
            &scripts,
            &config,
            ObjectId(1),
            map,
            None,
            Some(BattleHexCoordinate { x: 1, y: 1 })
        )
        .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
}
