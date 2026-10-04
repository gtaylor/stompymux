//! Live special-object HELP routing, persisted candidate order and general-help fallback.
use crate::support;
use sqlx::{Connection, SqliteConnection};
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Current chassis use their saved class, with uppercase special help and ordinary lowercase help.
#[tokio::test]
async fn cockpit_help_matches_catalogue_without_mutation() {
    for (template, class) in [
        (include_str!("../game/mechs/JR7-D.toml"), CommandClass::Mech),
        (
            include_str!("../game/mechs/GOL-1H.toml"),
            CommandClass::Mech,
        ),
        (
            include_str!("../game/mechs/Demolisher.toml"),
            CommandClass::Ground,
        ),
        (
            include_str!("../game/mechs/Kestrel.toml"),
            CommandClass::Vtol,
        ),
    ] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let unit = world.create(&config, "Help cockpit".into(), Kind::Thing);
        UnitTemplate::parse("test", template)
            .unwrap()
            .create(&mut world, unit)
            .unwrap();
        support::seed_object_dice(&mut world, unit, support::FIXTURE_DICE_SEED);
        let actor = world.create(&config, "Help reader".into(), Kind::Player);
        world.objects.get_mut(&actor).unwrap().location = Some(unit);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        for (input, topic) in [
            ("HELP", ""),
            ("HELP Movement", "Movement"),
            ("  HELP\tALL  ", "ALL"),
        ] {
            let actual = support::run_text(&scripts, &config, actor, 1, input);
            assert_eq!(actual, SpecialType::Unit.help(Some(class), false, topic));
        }
        for input in ["help", "Help", "help pilot"] {
            assert!(matches!(
                commands::run(&scripts, &config, actor, 1, input).unwrap(),
                CommandAction::Server(commands::ServerRequest::Help(_))
            ));
        }
        assert_eq!(scripts.world().btech, before);
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
        assert_eq!(
            support::run_text(&scripts, &config, actor, 1, "HELP"),
            SpecialType::Unit.help(Some(class), false, "")
        );
    }
}

/// Imported registrations expose help without inventing runtime domain state for deferred types.
#[tokio::test]
async fn actor_location_and_linked_inventory_order_survive_restart() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let actor = world.create(&config, "Registry reader".into(), Kind::Player);
    let map = world.create(&config, "Help map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "help",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    world.objects.get_mut(&actor).unwrap().location = Some(map);
    let first = world.create(&config, "Autopilot help".into(), Kind::Thing);
    let second = world.create(&config, "Debug help".into(), Kind::Thing);
    for id in [first, second] {
        world.objects.get_mut(&id).unwrap().location = Some(actor);
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    for (id, kind) in [(actor, "DEBUG"), (first, "AUTOPILOT"), (second, "DEBUG")] {
        sqlx::query("INSERT INTO btech_special_registrations(dbref,special_type) VALUES(?,?)")
            .bind(id.0)
            .bind(kind)
            .execute(&mut sql)
            .await
            .unwrap();
    }
    // Deliberately put the larger dbref first in the persisted contents chain.
    sqlx::query("UPDATE objects SET contents=? WHERE dbref=?")
        .bind(second.0)
        .bind(actor.0)
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("UPDATE objects SET next=? WHERE dbref=?")
        .bind(first.0)
        .bind(second.0)
        .execute(&mut sql)
        .await
        .unwrap();
    sqlx::query("UPDATE objects SET next=-1 WHERE dbref=?")
        .bind(first.0)
        .execute(&mut sql)
        .await
        .unwrap();
    sql.close().await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
    for (exclude, expected) in [
        (None, Some(SpecialType::Debug)),
        (Some(actor), Some(SpecialType::Map)),
        (Some(map), Some(SpecialType::Debug)),
        (Some(second), Some(SpecialType::Autopilot)),
        (Some(first), None),
    ] {
        if let Some(id) = exclude {
            scripts
                .world_mut()
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::Zombie);
        }
        let before = scripts.world().btech.clone();
        if let Some(kind) = expected {
            assert_eq!(
                support::run_text(&scripts, &config, actor, 1, "HELP"),
                kind.help(None, false, "")
            );
        } else {
            assert!(matches!(
                commands::run(&scripts, &config, actor, 1, "HELP").unwrap(),
                CommandAction::Server(commands::ServerRequest::Help(_))
            ));
        }
        assert_eq!(scripts.world().btech, before);
    }
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}

/// Without space compression, tabs do not delimit the special HELP word and trailing spaces matter.
#[tokio::test]
async fn help_respects_uncompressed_input() {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Raw help map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "help",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    let path = dir.path().join("stompymux.toml");
    let mut settings: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    settings
        .as_table_mut()
        .unwrap()
        .entry("mux")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("space_compress".into(), false.into());
    std::fs::write(path, toml::to_string(&settings).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(matches!(
        commands::run(&scripts, &config, ObjectId(1), 1, "HELP\tALL").unwrap(),
        CommandAction::Server(commands::ServerRequest::Help(_))
    ));
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "HELP   ALL"),
        SpecialType::Map.help(None, true, "ALL")
    );
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "HELP ALL ")
            .contains("doesn't have any other detailed help")
    );
}
