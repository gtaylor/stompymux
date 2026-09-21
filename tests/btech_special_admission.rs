//! Restricted special commands consume input before general dispatch and never borrow cause authority.
use crate::support;
use sqlx::{Connection, SqliteConnection};
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

const DENIED: &str = "Sorry, that command is restricted!";

/// Catalogue restrictions apply independently of command spelling and the underlying global handler.
#[tokio::test]
async fn map_restrictions_are_exact_read_only_and_durable() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Restricted map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "restricted",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let actor = world.create(&config, "Map reader".into(), Kind::Player);
    world.objects.get_mut(&actor).unwrap().location = Some(map);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(map);
    let exit = world.create(&config, "FIXMAP".into(), Kind::Exit);
    world.objects.get_mut(&exit).unwrap().location = Some(map);
    world.objects.get_mut(&exit).unwrap().destination = Some(ObjectId(0));
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    for input in [
        "setcond 50 -40",
        "ViEw 0 0",
        "@VIEWMAP",
        "LOADMAP missing",
        "FIXMAP",
        "LIST MECHS",
    ] {
        assert_eq!(
            support::run_text(&scripts, &config, actor, 1, input),
            DENIED
        );
        assert_eq!(scripts.world().btech, before);
        assert_eq!(scripts.world().objects[&actor].location, Some(map));
    }
    // Public STORES remains available, and GOD passes the catalogue gate to handler validation.
    assert_ne!(
        support::run_text(&scripts, &config, actor, 1, "STORES"),
        DENIED
    );
    assert_ne!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "SETCOND"),
        DENIED
    );
    assert_eq!(scripts.world().btech, before);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
    assert_eq!(
        support::run_text(&scripts, &config, actor, 1, "SETCOND 50 -40"),
        DENIED
    );
    assert_eq!(scripts.world().btech, before);
}

/// A public first match ends the restriction search; a later restricted object cannot override it.
#[tokio::test]
async fn candidate_order_and_queued_cause_do_not_change_restrictions() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let location = world.create(&config, "Debug location".into(), Kind::Thing);
    let actor = world.create(&config, "Autopilot actor".into(), Kind::Thing);
    let carried = world.create(&config, "Debug inventory".into(), Kind::Thing);
    world.objects.get_mut(&actor).unwrap().location = Some(location);
    world.objects.get_mut(&carried).unwrap().location = Some(actor);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    for (id, kind) in [
        (actor, "AUTOPILOT"),
        (location, "DEBUG"),
        (carried, "DEBUG"),
    ] {
        sqlx::query("INSERT INTO btech_special_registrations(dbref,special_type) VALUES(?,?)")
            .bind(id.0)
            .bind(kind)
            .execute(&mut sql)
            .await
            .unwrap();
    }
    sql.close().await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
    let execution = commands::ExecutionContext {
        executor: actor,
        cause: ObjectId(1),
        session: None,
        origin: commands::InputOrigin::Queued,
    };
    let before = scripts.world().btech.clone();
    // EVENTSTATS is public on AUTOPILOT but restricted on DEBUG. Its existing global
    // handler admission remains separate from this special-object restriction gate.
    let action = commands::execute(&scripts, &config, execution, "EVENTSTATS").unwrap();
    assert!(
        !matches!(action, CommandAction::Report(CommandReport::Reply(ref message)) if message == DENIED)
    );
    scripts.drain_outbox();
    for excluded in [actor, location] {
        scripts
            .world_mut()
            .objects
            .get_mut(&excluded)
            .unwrap()
            .flags
            .insert(Flag::Zombie);
        let action = commands::execute(&scripts, &config, execution, "EvEnTsTaTs").unwrap();
        assert!(
            matches!(action, CommandAction::Report(CommandReport::Reply(ref message)) if message == DENIED)
        );
        assert!(scripts.drain_outbox().is_empty());
        assert_eq!(scripts.world().btech, before);
    }
    scripts
        .world_mut()
        .objects
        .get_mut(&carried)
        .unwrap()
        .flags
        .insert(Flag::Zombie);
    let action = commands::execute(&scripts, &config, execution, "EVENTSTATS").unwrap();
    assert!(
        !matches!(action, CommandAction::Report(CommandReport::Reply(ref message)) if message == DENIED)
    );
    assert_eq!(scripts.world().btech, before);
}
