//! Selected DEBUG commands keep reference permissions without broadening operator APIs.
use crate::support;
use sqlx::{Connection, SqliteConnection};
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Imported DEBUG objects expose public Battle Value edits while other controls remain restricted.
#[tokio::test]
async fn debug_weapon_permissions_diagnostics_and_restart() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let actor = world.create(&config, "Debug user".into(), Kind::Player);
    let debug = world.create(&config, "Debug tool".into(), Kind::Thing);
    world.objects.get_mut(&debug).unwrap().location = Some(actor);
    world.objects.get_mut(&actor).unwrap().location = Some(ObjectId(0));
    // A same-name exit must not intercept an admitted special command.
    let exit = world.create(&config, "SETWBV".into(), Kind::Exit);
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
        for (input, reply) in [
            ("SETWBV", "Invalid arguments!"),
            ("SETWBV IS.SmallLaser 1 extra", "Invalid arguments!"),
            ("SETWBV IS.SmallLaser nope", "Invalid value!"),
            ("SETWBV IS.SmallLaser 2147483648", "Invalid value!"),
            ("SETWBV nonsense -1", "BV needs to be >=0"),
            ("SETWBV SmallLaser 1", "That is no weapon!"),
            ("SETWBV *.SmallLaser 1", "That is no weapon!"),
            ("SETWBV Gyro 1", "That is no weapon!"),
            (
                "SETVRT IS.SmallLaser 1",
                "Sorry, that command is restricted!",
            ),
            ("SHUTDOWN", "Sorry, that command is restricted!"),
        ] {
            let before = scripts.world().btech.clone();
            assert_eq!(support::run_text(&scripts, &config, actor, 1, input), reply);
            assert_eq!(scripts.world().btech, before);
        }
        assert_eq!(
            support::run_text(
                &scripts,
                &config,
                actor,
                1,
                "sEtWbV magna.is.smalllaser 314"
            ),
            "BV for IS.SmallLaser set to 314."
        );
        assert_eq!(scripts.world().objects[&actor].location, Some(ObjectId(0)));
        let value: u32 = scripts
            .eval_callback("return btech.weapon.settings('IS.SmallLaser').battle_value")
            .unwrap();
        assert_eq!(value, 314);
        // The same unprivileged actor cannot use the general typed/Lua control.
        assert!(
            set_battle_weapon_battle_value(&mut scripts.world_mut(), actor, "IS.SmallLaser", 1)
                .is_err()
        );
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.weapon.set_battle_value({}, 'IS.SmallLaser', 1)",
                    actor.0
                ))
                .is_err()
        );
        if iteration == 0 {
            let saved = scripts.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            *scripts.world_mut() = persistence::load(&config.database()).await.unwrap();
            let value: u32 = scripts
                .eval_callback("return btech.weapon.settings('IS.SmallLaser').battle_value")
                .unwrap();
            assert_ne!(
                value, 314,
                "runtime overrides reset while registration persists"
            );
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
        ("SETVRT IS.SmallLaser 0", "VRT needs to be >0"),
        ("SETVRT IS.SmallLaser 128", "VRT can be at max 127"),
        ("SETVRT SmallLaser 1", "That is no weapon!"),
        (
            "SETVRT IS.SmallLaser 12",
            "VRT for IS.SmallLaser set to 12.",
        ),
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
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
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
        .flags
        .insert(Flag::Zombie);
    assert_ne!(
        support::run_text(&scripts, &config, actor, 1, "SETWBV IS.SmallLaser 42"),
        "BV for IS.SmallLaser set to 42."
    );
}
