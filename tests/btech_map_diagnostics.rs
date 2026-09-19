//! Terrain substitutions share transactional diagnostics across map activation paths.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
mod support;

/// A map-error listener and an asset with two distinct substitutions in source order.
async fn fixture(create: bool) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Terrain diagnostics".into(), Kind::Room);
    if !create {
        create_battle_map(
            &mut world,
            map,
            "blank",
            BattleMapAsset::parse("2 2\n.0.0\n.0.0\n").unwrap(),
        )
        .unwrap();
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
    let root = config.path(&config.database.map_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("unknown.map"),
        "2 2\n!1&2ignored suffix\n.3$4.9\n",
    )
    .unwrap();
    (dir, config, world, map)
}

/// Each activation route preserves elevations, permanent fire and diagnostic order through restart.
#[tokio::test]
async fn native_and_lua_map_activation_share_substitutions_and_channels() {
    for (metadata, expected) in [
        ("", (8, 100, 20)),
        ("2: 50 nope\n", (8, 100, 20)),
        ("\n2: 50 -40\n", (8, 100, 20)),
        ("0: 300 -300\nignored\n", (0, 255, -128)),
        ("2: -20 300\n", (2, 0, 127)),
        ("\u{b}2\u{c}: \u{b}50\u{c} \u{c}-40\u{b}\n", (2, 50, -40)),
        ("2: 50\u{a0}-40\n", (8, 100, 20)),
    ] {
        for operation in ["create", "reload", "load"] {
            let (_dir, config, mut world, map) = fixture(operation == "create").await;
            let inherited_flags = if operation == "create" { 0 } else { 49 };
            if operation != "create" {
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["maps"][map.0.to_string()]["flags"] = inherited_flags.into();
                world.btech = serde_json::from_value(state).unwrap();
            }
            let expected = if expected.0 == 8 {
                (expected.0 | inherited_flags, expected.1, expected.2)
            } else {
                expected
            };
            std::fs::write(
                config
                    .path(&config.database.map_database)
                    .join("unknown.map"),
                {
                    let mut bytes = b"2 2\n!1&2\xffignored suffix\n.3$4\xfe\n".to_vec();
                    bytes.extend_from_slice(metadata.as_bytes());
                    bytes
                },
            )
            .unwrap();
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let command = if operation == "load" {
                "loadmap unknown.map".into()
            } else {
                format!("@btech map-{operation} #{}=unknown.map", map.0)
            };
            let call = if operation == "load" {
                format!("btech.map.load_as(1, {}, 'unknown.map')", map.0)
            } else {
                format!("btech.map.{operation}({}, 'unknown.map')", map.0)
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
            for scripts in [&native, &lua] {
                let state = scripts.world();
                let field = &state.btech.maps()[&map];
                assert_eq!((field.flags, field.gravity, field.temperature), expected);
                assert_eq!(
                    field.hex(0, 0).unwrap(),
                    BattleHex {
                        terrain: Terrain::Grassland,
                        elevation: 1
                    }
                );
                assert_eq!(
                    field.hex(1, 1).unwrap(),
                    BattleHex {
                        terrain: Terrain::Grassland,
                        elevation: 4
                    }
                );
                let channel = &state.channels["MapErrors"];
                assert_eq!(channel.messages, 2);
                assert_eq!(channel.history.len(), 2);
                for (message, suffix) in channel.history.iter().zip(["0,0: '!'", "1,1: '$'"]) {
                    assert!(
                        message
                            .message
                            .contains(&format!("Map #{}: Invalid terrain at {suffix}", map.0))
                    );
                }
            }
            let saved = lua.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(saved.btech, restored.btech);
            assert_eq!(
                serde_json::to_value(saved.channels).unwrap(),
                serde_json::to_value(restored.channels).unwrap()
            );
        }
    }
}

/// Late callback failure and channel capacity failure roll back terrain and every diagnostic.
#[tokio::test]
async fn terrain_diagnostics_obey_action_and_callback_rollback() {
    for operation in ["create", "reload", "load"] {
        let (_dir, config, world, map) = fixture(operation == "create").await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let call = if operation == "load" {
            format!("btech.map.load_as(1, {}, 'unknown.map')", map.0)
        } else {
            format!("btech.map.{operation}({}, 'unknown.map')", map.0)
        };
        assert!(
            scripts
                .eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(scripts.world().btech, world.btech);
        assert_eq!(
            serde_json::to_value(&scripts.world().channels).unwrap(),
            serde_json::to_value(&world.channels).unwrap()
        );
        assert!(scripts.drain_outbox().is_empty());
        let mut limited = world.clone();
        limited.channels.get_mut("MapErrors").unwrap().messages = i64::MAX - 1;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(limited.clone()))).unwrap();
        let command = if operation == "load" {
            "loadmap unknown.map".into()
        } else {
            format!("@btech map-{operation} #{}=unknown.map", map.0)
        };
        support::run_text(&scripts, &config, ObjectId(1), 1, &command);
        assert_eq!(scripts.world().btech, limited.btech);
        assert_eq!(
            serde_json::to_value(&scripts.world().channels).unwrap(),
            serde_json::to_value(&limited.channels).unwrap()
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

/// Missing diagnostic channels do not prevent loading; malformed elevations still reject the asset.
#[tokio::test]
async fn decoding_fallback_does_not_relax_structural_validation() {
    let (_dir, config, mut world, map) = fixture(false).await;
    world.channels.clear();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    load_battle_map_action(&scripts, &config, ObjectId(1), map, "unknown.map").unwrap();
    let before = scripts.world().btech.clone();
    std::fs::write(
        config.path(&config.database.map_database).join("bad.map"),
        "2 2\n!1.0\n.0!x\n",
    )
    .unwrap();
    assert!(load_battle_map_action(&scripts, &config, ObjectId(1), map, "bad.map").is_err());
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.world().channels.is_empty());
    assert!(Terrain::from_symbol('!').is_err());
}

/// Rejected loads publish preflight diagnostics without changing terrain or escaping callback rollback.
#[tokio::test]
async fn failed_load_diagnostics_commit_only_with_the_enclosing_callback() {
    for (name, source, diagnostic) in [
        (
            "dimensions.map",
            Some("0 2\n"),
            Some("Invalid height and or/width on dimensions.map"),
        ),
        (
            "rows.map",
            Some("2 2\n.0.0\n"),
            Some(
                "Mapfile possibly corrupt and/or height/width flipped. Height != what was read in rows.map",
            ),
        ),
        ("missing.map", None, None),
    ] {
        let (_dir, config, world, map) = fixture(false).await;
        if let Some(source) = source {
            std::fs::write(
                config.path(&config.database.map_database).join(name),
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
                        .contains(&format!("Map #{}: {diagnostic}", map.0))
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
