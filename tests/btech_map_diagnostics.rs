//! Map files load whole or not at all, and rejected loads report why on the MapErrors channel.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// A map-error listener, a blank 2x2 map unless `create`, and a valid 2x2 map file.
async fn fixture(create: bool) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Terrain diagnostics".into(), Kind::Room);
    if !create {
        create_battle_map(
            &mut world,
            map,
            "blank",
            MapAsset::from_cells("2 2\n.0.0\n.0.0\n").unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let mut channel = Channel::new("MapErrors".into());
    channel.users.push(communication::Membership {
        who: ObjectId(1),
        listening: true,
    });
    world.channels.insert(channel.name.clone(), channel);
    support::write_map(
        &config.path(&config.database.map_database),
        "field.map",
        "2 2\n.1&2\n.3.4\n",
    );
    (dir, config, world, map)
}

/// The native command and Lua agree for every activation route; a file without a `flags` key
/// keeps the flags the live map already has, and an explicit list replaces them.
#[tokio::test]
async fn native_and_lua_map_activation_agree_and_inherit_unnamed_flags() {
    for (settings, expected) in [
        ("", None),
        (
            "gravity = 50\ntemperature = -40\nflags = []\n",
            Some((0, 50, -40)),
        ),
        ("flags = [\"special_rules\"]\n", Some((2, 100, 20))),
    ] {
        for operation in ["create", "reload", "load"] {
            let (_dir, config, mut world, map) = fixture(operation == "create").await;
            let inherited = if operation == "create" { 0 } else { 49 };
            if operation != "create" {
                world
                    .btech
                    .rewrite_map_record(map, |record| {
                        record["flags"] = inherited.into();
                    })
                    .unwrap();
            }
            let root = config.path(&config.database.map_database);
            let text = std::fs::read_to_string(root.join("field.map.toml")).unwrap();
            let body = text[text.find("terrain").unwrap()..].to_owned();
            std::fs::write(root.join("field.map.toml"), format!("{settings}{body}")).unwrap();
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let command = if operation == "load" {
                "loadmap field.map".into()
            } else {
                format!("@btech map-{operation} #{}=field.map", map.0)
            };
            let call = if operation == "load" {
                format!("btech.map.load_as(1, {}, 'field.map')", map.0)
            } else {
                format!("btech.map.{operation}({}, 'field.map')", map.0)
            };
            support::run_text(&native, &config, ObjectId(1), 1, &command);
            assert!(
                lua.eval_callback::<bool>(&format!("return {call}"))
                    .unwrap()
            );
            // Independent map creation seeds independent fire streams; all other state must agree.
            let mut native_state = serde_json::to_value(&native.world().btech).unwrap();
            let lua_state = serde_json::to_value(&lua.world().btech).unwrap();
            native_state["maps"][map.0.to_string()]["fire_dice"] =
                lua_state["maps"][map.0.to_string()]["fire_dice"].clone();
            assert_eq!(native_state, lua_state);
            let expected = expected.unwrap_or((inherited, 100, 20));
            for scripts in [&native, &lua] {
                let state = scripts.world();
                let field = &state.btech.maps()[&map];
                assert_eq!((field.flags, field.gravity, field.temperature), expected);
                assert_eq!(field.hex(1, 0).unwrap(), Hex::new(Terrain::Fire, 2));
                assert_eq!(field.hex(1, 1).unwrap(), Hex::new(Terrain::Grassland, 4));
                assert_eq!(state.channels["MapErrors"].messages, 0);
            }
            let saved = lua.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
    }
}

/// Loading works without a MapErrors channel; an invalid file is still rejected whole.
#[tokio::test]
async fn invalid_files_are_rejected_without_a_diagnostic_channel() {
    let (_dir, config, mut world, map) = fixture(false).await;
    world.channels.clear();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    load_battle_map_action(&scripts, &config, ObjectId(1), map, "field.map").unwrap();
    let before = scripts.world().btech.clone();
    std::fs::write(
        config
            .path(&config.database.map_database)
            .join("bad.map.toml"),
        "terrain = '''\n!.\n..\n'''\nlevel = '''\n10\n00\n'''\n",
    )
    .unwrap();
    assert!(load_battle_map_action(&scripts, &config, ObjectId(1), map, "bad.map").is_err());
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.world().channels.is_empty());
}

/// Rejected loads publish their reason without changing terrain or escaping callback rollback.
#[tokio::test]
async fn failed_load_diagnostics_commit_only_with_the_enclosing_callback() {
    for (name, source, diagnostic) in [
        (
            "ragged.map",
            Some("terrain = '''\n..\n.\n'''\nlevel = '''\n00\n0\n'''\n"),
            Some("ragged.map is not a valid map file: terrain grid row 1 has 1 hexes"),
        ),
        (
            "symbol.map",
            Some("terrain = 'X'\nlevel = '0'\n"),
            Some("symbol.map is not a valid map file: unknown terrain symbol 'X' at 0,0"),
        ),
        ("missing.map", None, None),
    ] {
        let (_dir, config, world, map) = fixture(false).await;
        if let Some(source) = source {
            std::fs::write(
                config
                    .path(&config.database.map_database)
                    .join(format!("{name}.toml")),
                source,
            )
            .unwrap();
        }
        let call = format!("btech.map.load_as(1,{},'{name}')", map.0);
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let output =
            support::run_text(&native, &config, ObjectId(1), 1, &format!("loadmap {name}"));
        assert!(output.contains("#-1 Map"));
        assert!(
            !lua.eval_callback::<bool>(&format!("return pcall(function() {call} end)"))
                .unwrap()
        );
        for scripts in [&native, &lua] {
            assert_eq!(scripts.world().btech, world.btech);
            let state = scripts.world();
            let channel = &state.channels["MapErrors"];
            assert_eq!(channel.messages, i64::from(diagnostic.is_some()));
            assert_eq!(channel.history.len(), usize::from(diagnostic.is_some()));
            if let Some(diagnostic) = diagnostic {
                assert!(
                    channel.history[0]
                        .message
                        .contains(&format!("Map #{}: {diagnostic}", map.0)),
                    "{}",
                    channel.history[0].message
                );
            }
        }
        let saved = lua.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, saved.btech);
        assert_eq!(
            serde_json::to_value(&restored.channels).unwrap(),
            serde_json::to_value(&saved.channels).unwrap()
        );
        let aborted = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        assert!(
            aborted
                .eval_callback::<()>(&format!("pcall(function() {call} end); error('abort')"))
                .is_err()
        );
        assert_eq!(aborted.world().btech, world.btech);
        assert_eq!(
            serde_json::to_value(&aborted.world().channels).unwrap(),
            serde_json::to_value(&world.channels).unwrap()
        );
        assert!(aborted.drain_outbox().is_empty());
        if diagnostic.is_some() {
            let mut limited = world.clone();
            limited.channels.get_mut("MapErrors").unwrap().messages = i64::MAX;
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(limited.clone()))).unwrap();
            support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("loadmap {name}"),
            );
            assert!(
                !scripts
                    .eval_callback::<bool>(&format!("return pcall(function() {call} end)"))
                    .unwrap()
            );
            assert_eq!(scripts.world().btech, limited.btech);
            assert_eq!(
                serde_json::to_value(&scripts.world().channels).unwrap(),
                serde_json::to_value(&limited.channels).unwrap()
            );
            assert!(scripts.drain_outbox().is_empty());
        }
    }
}
