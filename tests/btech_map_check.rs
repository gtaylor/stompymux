//! FIXMAP checks derived membership without unit lifecycle side effects.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

#[tokio::test]
async fn map_check_preserves_each_chassis_and_matches_lua_after_restart() {
    for template in firing::templates() {
        let (_dir, config, mut world, source, target, _) =
            firing::fixture_with_target(&template, None, include_str!("../game/mechs/AS7-D.toml"))
                .await;
        let map = world.btech.units()[&source].map.unwrap();
        let actor = world.create(&config, "Map checker".into(), Kind::Player);
        world
            .objects
            .get_mut(&actor)
            .unwrap()
            .flags
            .insert(Flag::Wizard);
        world.objects.get_mut(&actor).unwrap().location = Some(map);
        let before = world.btech.clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let report = check_battle_map_action(&scripts, &config, actor, map).unwrap();
        assert_eq!(report.units, vec![source, target]);
        scripts.drain_outbox();
        assert_eq!(
            support::run_text(&scripts, &config, actor, 1, "fixmap ignored"),
            "Checking 2 entries..\nDone."
        );
        let lua: String = scripts
            .eval_callback(&format!(
                "local r=btech.map.check({},{}); return r.map..':'..r.units[1]..':'..r.units[2]",
                actor.0, map.0
            ))
            .unwrap();
        assert_eq!(lua, format!("{}:{}:{}", map.0, source.0, target.0));
        assert_eq!(scripts.drain_outbox().len(), 2);
        assert_eq!(scripts.world().btech, before);
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, before);
        let restart = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
        assert_eq!(
            check_battle_map_action(&restart, &config, actor, map).unwrap(),
            report
        );
    }
}

#[tokio::test]
async fn invalid_membership_rejects_without_success_or_destructive_repair() {
    for template in firing::templates() {
        let (_dir, config, world, source, target, _) =
            firing::fixture_with_target(&template, None, include_str!("../game/mechs/AS7-D.toml"))
                .await;
        let map = world.btech.units()[&source].map.unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let valid = scripts.world().clone();
        for fault in ["missing", "duplicate", "location", "identity"] {
            *scripts.world_mut() = valid.clone();
            if fault == "location" {
                scripts
                    .world_mut()
                    .objects
                    .get_mut(&source)
                    .unwrap()
                    .location = Some(ObjectId(0));
            } else {
                let mut state = serde_json::to_value(&valid.btech).unwrap();
                let family = if valid.btech.vehicles().contains_key(&source) {
                    "vehicles"
                } else {
                    "constructed"
                };
                match fault {
                    "missing" => {
                        state[family][source.0.to_string()]["map_slot"] = serde_json::Value::Null
                    }
                    "duplicate" => {
                        state[family][source.0.to_string()]["map_slot"] = serde_json::to_value(
                            valid.btech.constructed_units()[&target].map_slot(),
                        )
                        .unwrap()
                    }
                    _ => state["units"][source.0.to_string()]["map"] = serde_json::Value::Null,
                }
                let decoded = serde_json::from_value(state);
                if family == "vehicles" && fault == "missing" {
                    assert!(decoded.is_err());
                    assert_eq!(scripts.world().btech, valid.btech);
                    assert!(scripts.drain_outbox().is_empty());
                    continue;
                }
                scripts.world_mut().btech = decoded.unwrap();
            }
            let broken = scripts.world().clone();
            assert!(
                check_battle_map_action(&scripts, &config, ObjectId(1), map).is_err(),
                "{fault}"
            );
            assert_eq!(scripts.world().btech, broken.btech);
            assert_eq!(
                scripts.world().objects[&source].location,
                broken.objects[&source].location
            );
            assert!(scripts.drain_outbox().is_empty());
        }
    }
}

#[tokio::test]
async fn empty_map_authority_and_publication_rollback() {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Empty map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "empty",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(check_battle_map_action(&scripts, &config, map, map).is_err());
    assert!(check_battle_map_action(&scripts, &config, ObjectId(1), ObjectId(-1)).is_err());
    assert!(
        check_battle_map_action(&scripts, &config, ObjectId(1), map)
            .unwrap()
            .units
            .is_empty()
    );
    assert_eq!(scripts.drain_outbox().len(), 2);
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.map.check(1,{});error('abort')", map.0))
            .is_err()
    );
    assert!(scripts.drain_outbox().is_empty());
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
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(scripts.world().clone()))).unwrap();
    let before = scripts.world().btech.clone();
    assert!(check_battle_map_action(&scripts, &config, ObjectId(1), map).is_err());
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(scripts.world().btech, before);
}

/// Allocation history remains observable after middle and tail removals, including an empty allocated span.
#[tokio::test]
async fn membership_span_survives_holes_reuse_and_restart() {
    for template in [
        firing::templates()[0].clone(),
        firing::templates()[2].clone(),
    ] {
        let (_dir, config, mut world, first, middle, _) =
            firing::fixture_with_target(&template, None, &template).await;
        let map = world.btech.units()[&first].map.unwrap();
        for id in [first, middle] {
            firing::edit(&mut world, id, |unit| {
                unit["power"] = serde_json::to_value(BattlePower::Off).unwrap();
                unit["target_lock"] = serde_json::Value::Null;
            });
        }
        let last = world.create(&config, "Last member".into(), Kind::Thing);
        BattleUnitTemplate::parse("test", &template)
            .unwrap()
            .create(&mut world, last)
            .unwrap();
        support::seed_object_dice(&mut world, last, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, last, map, 0, 0).unwrap();
        release_battle_pilot(&mut world, first, ObjectId(1)).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for (remove, span, count) in [
            (Some(middle), 3, 2),
            (Some(last), 2, 1),
            (Some(first), 2, 0),
        ] {
            remove_battle_unit(
                &mut scripts.world_mut(),
                remove.unwrap(),
                ObjectId(config.home()),
            )
            .unwrap();
            let before = scripts.world().btech.clone();
            let check = support::run_text(&scripts, &config, ObjectId(1), 1, "FIXMAP");
            assert_eq!(check, format!("Checking {span} entries..\nDone."));
            let list = support::run_text(&scripts, &config, ObjectId(1), 1, "LIST MECHS");
            assert!(list.contains(&format!("{count} Mechs On Map")));
            assert!(list.contains(&format!("{span} is first free slot, according to db.")));
            assert_eq!(scripts.world().btech, before);
            let world_snapshot = scripts.world().clone();
            persistence::save(&config.database(), &world_snapshot)
                .await
                .unwrap();
            let loaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(loaded.btech, before);
            let replay = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
            check_battle_map_action(&replay, &config, ObjectId(1), map).unwrap();
            let output: Vec<_> = replay
                .drain_outbox()
                .into_iter()
                .map(|(_, text)| text.source().to_owned())
                .collect();
            assert_eq!(
                output,
                [format!("Checking {span} entries.."), "Done.".into()]
            );
        }
        for id in [first, middle] {
            place_battle_unit(&mut scripts.world_mut(), id, map, 0, 0).unwrap();
        }
        let filled = support::run_text(&scripts, &config, ObjectId(1), 1, "LIST MECHS");
        assert!(!filled.contains("is first free slot"));
        for id in [middle, first] {
            remove_battle_unit(&mut scripts.world_mut(), id, ObjectId(config.home())).unwrap();
        }
        assert_eq!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "FIXMAP"),
            "Checking 0 entries..\nDone."
        );
    }
}
