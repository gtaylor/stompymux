//! Selected DEBUG commands keep reference permissions without broadening operator APIs.
use crate::support;
use sqlx::{Connection, SqliteConnection};
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Imported DEBUG objects restrict every operator control to privileged actors.
#[tokio::test]
async fn debug_permissions_diagnostics_and_restart() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let actor = world.create(&config, "Debug user".into(), Kind::Player);
    let debug = world.create(&config, "Debug tool".into(), Kind::Thing);
    world.objects.get_mut(&debug).unwrap().location = Some(actor);
    world.objects.get_mut(&actor).unwrap().location = Some(ObjectId(0));
    // A same-name exit must not intercept an admitted special command.
    let exit = world.create(&config, "SHUTDOWN".into(), Kind::Exit);
    world.objects.get_mut(&exit).unwrap().location = Some(ObjectId(0));
    world.objects.get_mut(&exit).unwrap().destination = Some(ObjectId(2));
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("INSERT INTO btech_special_registrations(dbref,special_type) VALUES(?,'DEBUG')")
        .bind(debug.0)
        .execute(&mut sql)
        .await
        .unwrap();
    sql.close().await.unwrap();
    let world = persistence::load(&config.database()).await.unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for iteration in 0..2 {
        for input in [
            "SHUTDOWN",
            "sHuTdOwN 1",
            "SETXPLEVEL PilBip 1",
            "XPTOP PilBip",
        ] {
            let before = scripts.world().btech.clone();
            assert_eq!(
                support::run_text(&scripts, &config, actor, 1, input),
                "Sorry, that command is restricted!"
            );
            assert_eq!(scripts.world().btech, before);
        }
        assert_eq!(scripts.world().objects[&actor].location, Some(ObjectId(0)));
        if iteration == 0 {
            // The registration persists across a restart.
            let saved = scripts.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            *scripts.world_mut() = persistence::load(&config.database()).await.unwrap();
        }
    }
    // Moving the tool to GOD exercises the restricted DEBUG adapter, including exact errors.
    scripts
        .world_mut()
        .objects
        .get_mut(&debug)
        .unwrap()
        .location = Some(ObjectId(1));
    for (input, reply) in [
        ("SHUTDOWN", "Invalid map number!"),
        ("SHUTDOWN not-a-map", "Invalid map number!"),
        ("SHUTDOWN -1", ""),
    ] {
        assert_eq!(
            support::run_text(&scripts, &config, ObjectId(1), 1, input),
            reply
        );
    }
    for (input, reply) in [
        ("SETXPLEVEL", "Invalid arguments!"),
        ("SETXPLEVEL PilBip 1 extra", "Invalid arguments!"),
        ("SETXPLEVEL PilBip bad", "Invalid value!"),
        ("SETXPLEVEL PilBip 2147483648", "Invalid value!"),
        (
            "SETXPLEVEL Missing -1",
            "Threshold needs to be >=0 (0 = no gains possible)",
        ),
        ("SETXPLEVEL Missing 0", "That isn't any charvalue!"),
        ("SETXPLEVEL Refle 0", "That isn't any skill!"),
        ("SETXPLEVEL ShoHit 0", "That isn't any skill!"),
        ("XPTOP", "Invalid argument!"),
        ("XPTOP Missing", "Invalid value name!"),
        ("XPTOP Refle", "Only skills have XP (for now at least)"),
        ("XPTOP ShoHit", "Only skills have XP (for now at least)"),
    ] {
        let before = scripts.world().btech.clone();
        assert_eq!(
            support::run_text(&scripts, &config, ObjectId(1), 1, input),
            reply
        );
        assert_eq!(scripts.world().btech, before);
    }
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "SETXPLEVEL PilBip 0"),
        ""
    );
    let threshold: u32 = scripts
        .eval_callback("return btech.character.threshold('Piloting-Biped')")
        .unwrap();
    assert_eq!(threshold, 0);
    let map = scripts
        .world_mut()
        .create(&config, "Clearable map".into(), Kind::Room);
    create_battle_map(
        &mut scripts.world_mut(),
        map,
        "clearable",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut scripts.world_mut(), map, support::FIXTURE_DICE_SEED);
    assert_eq!(
        support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("SHUTDOWN {} ignored", map.0)
        ),
        "Map Cleared"
    );
    // Removal from candidate selection restores the existing global authority gate.
    scripts
        .world_mut()
        .objects
        .get_mut(&debug)
        .unwrap()
        .location = Some(actor);
    assert_eq!(
        support::run_text(&scripts, &config, actor, 1, "XPTOP PilBip"),
        "Sorry, that command is restricted!"
    );
    scripts
        .world_mut()
        .objects
        .get_mut(&debug)
        .unwrap()
        .flags
        .insert(Flag::Zombie);
    assert_ne!(
        support::run_text(&scripts, &config, actor, 1, "XPTOP PilBip"),
        "Sorry, that command is restricted!"
    );
}
