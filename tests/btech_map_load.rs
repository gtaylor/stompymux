//! Map loading activates new terrain before shared shutdown and honors GOD's retained membership.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
use crate::support;

/// Both operator classes receive the same assets and metadata; only membership policy differs.
#[tokio::test]
async fn load_map_native_lua_and_restart_across_chassis() {
    for (chassis, source) in firing::templates().into_iter().enumerate() {
        for god in [false, true] {
            let (_dir, config, mut world, id, target, _) =
                firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D"))
                    .await;
            let map = world.btech.units()[&id].map.unwrap();
            let actor = if god {
                release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
                ObjectId(1)
            } else {
                let actor = world.create(&config, "Map loader".into(), Kind::Player);
                world
                    .objects
                    .get_mut(&actor)
                    .unwrap()
                    .flags
                    .insert(Flag::Wizard);
                actor
            };
            world.objects.get_mut(&actor).unwrap().location = Some(map);
            set_battle_map_cloud_base(&mut world, actor, map, 37).unwrap();
            if chassis != 5 {
                firing::edit(&mut world, id, |unit| unit["motion"]["speed"] = 21.5.into());
            }
            let root = config.path(&config.database.map_database);
            std::fs::create_dir_all(&root).unwrap();
            // Raised terrain distinguishes retained physical altitude from implicit ground height.
            std::fs::write(
                root.join("loaded.map"),
                format!("2 14\n{}2: 50 -40\n", "#2.0ignored suffix\n".repeat(14)),
            )
            .unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            let before = world.btech.clone();
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let call = format!("btech.map.load_as({},{},'loaded.map')", actor.0, map.0);
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort load')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            assert!(
                lua.eval_callback::<bool>(&format!("return {call}"))
                    .unwrap()
            );
            let output =
                support::run_text(&native, &config, actor, 1, "loadmap loaded.map ignored");
            assert!(
                output.contains("Loading loaded.map"),
                "{chassis} {god}: {output}"
            );
            assert_eq!(output.contains("Map Cleared"), !god);
            assert_eq!(native.world().btech, lua.world().btech);
            let saved = native.world().clone();
            let field = &saved.btech.maps()[&map];
            assert_eq!(
                (
                    field.width,
                    field.height,
                    field.gravity,
                    field.temperature,
                    field.cloud_base
                ),
                (2, 14, 50, -40, 37)
            );
            assert_eq!(field.name, "loaded.map");
            for unit in [id, target] {
                assert_eq!(saved.btech.units()[&unit].map, god.then_some(map));
                assert_eq!(saved.objects[&unit].location, Some(map));
                if god {
                    assert_eq!(battle_unit_altitude(&saved, unit).unwrap(), Some(0.0));
                }
            }
            saved.validate(&config).unwrap();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
    }
}

/// Missing/malformed assets and invalid crops cannot clear units or alter pending output.
#[tokio::test]
async fn invalid_loads_preserve_map_membership_and_messages() {
    let (_dir, config, mut world, id, _, _) = firing::fixture_with_target(
        &firing::templates()[0],
        None,
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    let map = world.btech.units()[&id].map.unwrap();
    let root = config.path(&config.database.map_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("short.map"), "1 1\n.0\n").unwrap();
    std::fs::write(root.join("bad.map"), "2 3\n.0\n").unwrap();
    std::fs::write(root.join("dimensions.map"), "0 2\n").unwrap();
    std::fs::write(root.join("eof.map"), "2 2\n.0.0\n").unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (name, expected) in [
        ("missing.map", "#-1 Map not found."),
        ("dimensions.map", "#-1 Map invalid - Bad Height/Width."),
        ("bad.map", "#-1 Map invalid - Height not loaded properly"),
        ("eof.map", "#-1 Map invalid - Height not loaded properly"),
    ] {
        let error = scripts
            .eval_callback::<bool>(&format!("return btech.map.load_as(1,{},'{name}')", map.0))
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
        assert_eq!(
            format!(
                "{:#}",
                load_battle_map_action(&scripts, &config, ObjectId(1), map, name).unwrap_err()
            ),
            expected
        );
        for suffix in ["", " ignored"] {
            assert_eq!(
                support::run_text(
                    &scripts,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("loadmap {name}{suffix}")
                ),
                format!("Loading {name}\r\n{expected}")
            );
            assert_eq!(scripts.world().btech, before);
            assert!(scripts.drain_outbox().is_empty());
        }
    }
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "loadmap"),
        "Invalid number of arguments!"
    );
    for name in ["missing.map", "bad.map", "short.map", "../outside"] {
        assert!(load_battle_map_action(&scripts, &config, ObjectId(1), map, name).is_err());
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
    }
    assert!(load_battle_map_action(&scripts, &config, ObjectId(2), map, "short.map").is_err());
    assert_eq!(scripts.world().btech, before);
}

/// Shutdown sees the replacement water surface, not the dry terrain that existed before loading.
#[tokio::test]
async fn loaded_terrain_precedes_shutdown_consequences() {
    let (_dir, config, mut world, id, _, _) = firing::fixture_with_target(
        &firing::templates()[2],
        None,
        include_str!("../game/mechs/AS7-D"),
    )
    .await;
    let map = world.btech.units()[&id].map.unwrap();
    let actor = world.create(&config, "Map loader".into(), Kind::Player);
    world
        .objects
        .get_mut(&actor)
        .unwrap()
        .flags
        .insert(Flag::Wizard);
    world.objects.get_mut(&actor).unwrap().location = Some(map);
    firing::edit(&mut world, id, |unit| unit["motion"]["speed"] = 21.5.into());
    let root = config.path(&config.database.map_database);
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("water.map"),
        format!("1 12\n{}", "~2\n".repeat(12)),
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let output = support::run_text(&scripts, &config, actor, 1, "loadmap water.map");
    assert!(output.contains("free-fall"), "{output}");
    assert!(!output.contains("mid-motion"), "{output}");
    assert!(output.contains("Map Cleared"));
    assert_eq!(scripts.world().btech.units()[&id].map, None);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}
