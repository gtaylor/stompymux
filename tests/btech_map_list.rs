//! Wizard map listings share unit order, object selection and atomic read-only publication.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
#[path = "support/btech_map_objects.rs"]
mod map_objects;
use crate::support;

/// New effects prepend their kind's list, including replacements, independent of tile coordinates.
#[tokio::test]
async fn object_table_keeps_effect_creation_order_across_restart_and_rollback() {
    for (kind, stored, name, lua_name) in [
        (
            BattleDecorationKind::Fire,
            BattleStaticDecorationKind::Fire,
            "FIRE",
            "add_fire",
        ),
        (
            BattleDecorationKind::Smoke,
            BattleStaticDecorationKind::Smoke,
            "SMOKE",
            "add_smoke",
        ),
    ] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let map = world.create(&config, "Ordering".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "plain",
            BattleMapAsset::parse("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
        )
        .unwrap();
        set_battle_static_decoration(
            &mut world,
            map,
            stored,
            0,
            Some(BattleStaticDecoration {
                coordinate: BattleHexCoordinate { x: 2, y: 2 },
                restored_terrain: Terrain::Grassland,
                object: ObjectId(1),
                duration: 99,
                scalar: 3,
            }),
        )
        .unwrap();
        for x in [2, 0, 1] {
            set_map_decoration(
                &mut world,
                map,
                BattleHexCoordinate { x, y: 0 },
                Some(BattleDecoration::new(kind, 20, None)),
            )
            .unwrap();
        }
        for (replacement, expected) in [
            (false, vec![(1, 0), (0, 0), (2, 0), (2, 2)]),
            (true, vec![(2, 0), (1, 0), (0, 0), (2, 2)]),
        ] {
            if replacement {
                set_map_decoration(
                    &mut world,
                    map,
                    BattleHexCoordinate { x: 2, y: 0 },
                    Some(BattleDecoration::new(kind, 30, None)),
                )
                .unwrap();
            }
            let before = world.btech.clone();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            list_battle_map_action(&scripts, &config, ObjectId(1), map, true).unwrap();
            let rows = output(&scripts);
            let coordinates: Vec<(i32, i32)> = rows
                .iter()
                .filter_map(|row| {
                    let fields: Vec<_> = row.split_whitespace().collect();
                    (fields.get(2) == Some(&name))
                        .then(|| (fields[0].parse().unwrap(), fields[1].parse().unwrap()))
                })
                .collect();
            assert_eq!(coordinates, expected);
            assert!(
                scripts
                    .eval_callback::<()>(&format!(
                        "btech.map.{lua_name}(1,{},0,0,10); error('abort')",
                        map.0
                    ))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, before);
            assert!(output(&scripts).is_empty());
            persistence::save(&config.database(), &world).await.unwrap();
            world = persistence::load(&config.database()).await.unwrap();
            let replay = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            list_battle_map_action(&replay, &config, ObjectId(1), map, true).unwrap();
            assert_eq!(output(&replay), rows);
        }
    }
}

/// Imported payloads and smoke's retained duration remain visible after a tick and restart.
#[tokio::test]
async fn object_table_reports_saved_payloads_and_retained_duration() {
    use sqlx::Connection;
    let (_dir, config, _world, map, interior) = map_objects::fixture().await;
    let mut sql = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("UPDATE btech_map_objects SET data_char=87,data_short=-23,data_int=91 WHERE map_dbref=? AND object_type IN (4,7)")
        .bind(map.0).execute(&mut sql).await.unwrap();
    sqlx::query(
        "UPDATE btech_map_objects SET data_short=-32768 WHERE map_dbref=? AND object_type=9",
    )
    .bind(map.0)
    .execute(&mut sql)
    .await
    .unwrap();
    let mut world = persistence::load(&config.database()).await.unwrap();
    advance_map_smoke(&mut world);
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    list_battle_map_action(&scripts, &config, ObjectId(1), map, true).unwrap();
    let rows = output(&scripts);
    let building = rows.iter().find(|row| row.contains("BUILDING")).unwrap();
    assert_eq!(
        building.split_whitespace().collect::<Vec<_>>(),
        vec![
            "1",
            "1",
            "BUILDING",
            &interior.0.to_string(),
            "87",
            "-23",
            "91"
        ]
    );
    assert!(rows.contains(&"1   1   LINKED 0     87   -23    91".into()));
    assert!(rows.contains(&"1   1   BLZ   1     0    -32768 1".into()));
    assert!(rows.contains(&"2   2   SMOKE 0     32   20     0".into()));
    assert_eq!(scripts.world().btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    let replay = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
    list_battle_map_action(&replay, &config, ObjectId(1), map, true).unwrap();
    assert_eq!(output(&replay), rows);
}

fn output(scripts: &Scripts) -> Vec<String> {
    scripts
        .drain_outbox()
        .into_iter()
        .map(|(_, text)| text.source().to_owned())
        .collect()
}

#[tokio::test]
async fn mixed_unit_lists_follow_slots_without_mutating_simulation() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, target, _) =
            firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D")).await;
        let map = world.btech.units()[&id].map.unwrap();
        firing::edit(&mut world, id, |unit| unit["map_slot"] = 5.into());
        firing::edit(&mut world, target, |unit| unit["map_slot"] = 2.into());
        let actor = world.create(&config, "Map lister".into(), Kind::Player);
        world
            .objects
            .get_mut(&actor)
            .unwrap()
            .flags
            .insert(Flag::Wizard);
        world.objects.get_mut(&actor).unwrap().location = Some(map);
        let before = world.btech.clone();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let text = support::run_text(&native, &config, actor, 1, "list mEcHs ignored");
        assert!(
            text.find(&format!("Mech DB Number: {} : [AC]", target.0))
                .unwrap()
                < text
                    .find(&format!("Mech DB Number: {} : [AF]", id.0))
                    .unwrap()
        );
        assert!(text.contains("2 Mechs On Map"));
        assert!(text.contains("248 positions open"));
        assert!(
            lua.eval_callback::<bool>(&format!(
                "return btech.map.list({},{},'MECHS')",
                actor.0, map.0
            ))
            .unwrap()
        );
        let lines = output(&lua);
        assert_eq!(lines.len(), 6);
        assert!(lines.contains(&"6 is first free slot, according to db.".into()));
        for line in lines {
            assert!(text.contains(&line), "{text:?} missing {line:?}");
        }
        assert_eq!(native.world().btech, before);
        assert_eq!(lua.world().btech, before);
    }
}

#[tokio::test]
async fn object_listing_shares_native_lua_order_and_owned_details() {
    let (_dir, config, world, map, _) = map_objects::fixture().await;
    let before = world.btech.clone();
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let call = format!("btech.map.list(1,{},'oBjS')", map.0);
    assert!(
        lua.eval_callback::<()>(&format!("{call}; error('abort')"))
            .is_err()
    );
    assert!(output(&lua).is_empty());
    assert!(
        lua.eval_callback::<bool>(&format!("return {call}"))
            .unwrap()
    );
    let lines = output(&lua);
    assert_eq!(lines.len(), 16);
    assert_eq!(lines[0], "X   Y   Type  obj   dc   ds     di");
    assert_eq!(lines[1], "--------------------------------------------");
    assert_eq!(lines[15], lines[1]);
    assert_eq!(lines[12], "--- MAP/HANGAR INFORMATION OBJECT ---");
    let kinds: Vec<_> = lines[2..15]
        .iter()
        .filter(|line| !line.contains("MAP/HANGAR INFORMATION"))
        .map(|line| line.split_whitespace().nth(2).unwrap())
        .collect();
    assert_eq!(
        kinds,
        vec![
            "FIRE", "SMOKE", "DECO", "MINE", "MINE", "BUILDING", "LEAVE", "ENTRA", "LINKED",
            "LINKED", "BLZ", "BLZ"
        ]
    );
    assert_eq!(lines[2], "1   1   FIRE  0     32   0      0");
    assert_eq!(lines[3], "2   2   SMOKE 0     32   20     0");
    assert_eq!(lines[4], "1   1   DECO  0     126  0      0");
    assert_eq!(lines[14], "1   1   BLZ   1     0    0      1");
    let text = support::run_text(&native, &config, ObjectId(1), 1, "list objs");
    for line in lines {
        assert!(text.contains(&line), "Missing {line:?}");
    }
    assert_eq!(native.world().btech, before);
    assert_eq!(lua.world().btech, before);
    assert!(list_battle_map_action(&native, &config, ObjectId(2), map, true).is_err());
    for input in ["list", "list invalid", "list/no objs", "list objs extra"] {
        support::run_text(&native, &config, ObjectId(1), 1, input);
        assert_eq!(native.world().btech, before);
    }
}

#[tokio::test]
async fn a_partial_listing_is_discarded_when_output_limits_fail() {
    let (dir, _, world, map, _) = map_objects::fixture().await;
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("lua")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_entry_limit".into(), 2.into());
    std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(list_battle_map_action(&scripts, &config, ObjectId(1), map, true).is_err());
    assert!(output(&scripts).is_empty());
    assert_eq!(scripts.world().btech, before);
}

/// Target names are complete words; native parsing consumes only the first token.
#[tokio::test]
async fn list_target_admission_matches_reference_replies() {
    let (_dir, config, world, map, _) = map_objects::fixture().await;
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for argument in [
        "m",
        "mech",
        "o",
        "obj",
        "iNvAlId",
        "objs\u{a0}suffix",
        "mechs\u{2003}suffix",
    ] {
        let expected = format!("Invalid argument ({argument})!");
        let text = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("list {argument} ignored"),
        );
        assert!(text.contains(&expected), "{text:?}");
        let error = scripts
            .eval_callback::<bool>(&format!("return btech.map.list(1,{},'{argument}')", map.0,))
            .unwrap_err();
        assert!(format!("{error:#}").contains(&expected));
        assert!(output(&scripts).is_empty());
    }
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "list");
    assert!(text.contains("Supply target type too!"));
    for target in ["MeChS", "ObJs"] {
        let plain = support::run_text(&scripts, &config, ObjectId(1), 1, &format!("list {target}"));
        let extra = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("list {target} ignored arguments"),
        );
        assert_eq!(plain, extra);
        let tabs = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("list \t{target}\tignored arguments"),
        );
        assert_eq!(plain, tabs);
    }
    assert_eq!(scripts.world().btech, before);
}
