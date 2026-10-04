//! Native linked-map control shares Lua wrapping state and durable marker ownership.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// All chassis retain their exact state while the map acquires one shared wrapping marker.
#[tokio::test]
async fn setlinked_shares_wrapping_and_saved_state() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D.toml"))
                .await;
        let map = world.btech.units()[&id].map.unwrap();
        let actor = world.create(&config, "Map linker".into(), Kind::Player);
        world
            .objects
            .get_mut(&actor)
            .unwrap()
            .flags
            .insert(Flag::Wizard);
        world.objects.get_mut(&actor).unwrap().location = Some(map);
        let before = world.btech.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        lua.eval_callback::<()>(&format!("btech.map.wrapping({},true)", map.0))
            .unwrap();
        let marker: (i32, i32) = lua
            .eval_callback(&format!(
                "local p=btech.map.inspect({}).linked_markers[0].coordinate; return p.x,p.y",
                map.0
            ))
            .unwrap();
        assert_eq!(marker, (0, 0));
        for count in 1..=2 {
            let output = support::run_text(&native, &config, actor, 1, "setlinked ignored");
            assert!(output.contains("Map set to linked."), "{output}");
            if count == 1 {
                assert_eq!(native.world().btech, lua.world().btech);
            }
            assert_eq!(
                native.world().btech.maps()[&map].linked_markers().len(),
                count
            );
            let saved = native.world().clone();
            assert_eq!(saved.btech.constructed_units(), before.constructed_units());
            assert_eq!(saved.btech.vehicles(), before.vehicles());
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
    }
}

/// A failed confirmation cannot silently enable wrapping.
#[tokio::test]
async fn linked_confirmation_failure_restores_state() {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "map",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
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
    let output = support::run_text(&scripts, &config, ObjectId(1), 1, "setlinked");
    assert!(output.contains("limit"), "{output}");
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
}
