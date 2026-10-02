//! Deterministic startup cadence, durable countdowns and server tick save-failure recovery.
use crate::support;
use sqlx::{Connection, SqliteConnection};
use stompymux_rs::{
    BattleMapAsset, BattlePower, BattleTemplate, Config, Kind, ObjectId, Scripts, ShutdownRequest,
    World, advance_battle_units, assign_battle_pilot, create_battle_map, create_battle_unit,
    persistence, place_battle_unit, remove_battle_unit, start_battle_unit, stop_battle_unit,
};

/// A piloted, placed Jenner in an isolated database, ready for a power transition.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    world.accounts.get_mut(&ObjectId(1)).unwrap().hash =
        Some(stompymux_rs::accounts::hash("secret", &config).unwrap());
    let map = world.create(&config, "Battlefield".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test.map",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Engine lab".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    (dir, config, world, id)
}

#[tokio::test]
async fn startup_emits_six_stages_and_resumes_only_committed_seconds() {
    let (_dir, config, mut world, id) = fixture().await;
    assert!(start_battle_unit(&mut world, id, ObjectId(2), false).is_err());
    start_battle_unit(&mut world, id, ObjectId(1), false).unwrap();
    assert!(start_battle_unit(&mut world, id, ObjectId(1), false).is_err());
    let mut messages = Vec::new();
    for second in 1..=30 {
        let notices = advance_battle_units(&mut world, 0);
        assert_eq!(notices.len(), usize::from(second % 5 == 0));
        messages.extend(notices.into_iter().map(|notice| notice.text));
        if second == 12 {
            persistence::save(&config.database(), &world).await.unwrap();
            world = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                world.btech.constructed_units()[&id].power(),
                BattlePower::Starting { remaining: 18 }
            );
        }
    }
    assert_eq!(
        messages,
        [
            "Main reactor is now online.",
            "Gyros are now stable.",
            "Main computer system is now online.",
            "Scanners are now operational.",
            "Targeting system is now operational.",
            "All systems operational!"
        ]
    );
    assert_eq!(
        world.btech.constructed_units()[&id].power(),
        BattlePower::Running
    );
    assert!(advance_battle_units(&mut world, 0).is_empty());
    assert!(remove_battle_unit(&mut world, id, ObjectId(config.start())).is_err());
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id].power(),
        BattlePower::Off
    );
    assert_eq!(world.btech.constructed_units()[&id].pilot(), None);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..4 {
        assert!(advance_battle_units(&mut world, 0).is_empty());
    }
    assert_eq!(
        advance_battle_units(&mut world, 0)[0].text,
        "All systems operational!"
    );
}

#[tokio::test]
async fn abort_and_lua_rollback_cancel_pending_startup_and_output() {
    let (_dir, config, world, id) = fixture().await;
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.start({},1); error('abort')", id.0))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before.btech);
    assert!(scripts.drain_outbox().is_empty());
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "startup");
    assert!(text.contains("Startup Cycle"), "{text}");
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "shutdown");
    assert!(text.contains("aborted"), "{text}");
    for _ in 0..31 {
        assert!(advance_battle_units(&mut scripts.world_mut(), 0).is_empty());
    }
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].power(),
        BattlePower::Off
    );
    assert_eq!(scripts.world().btech.constructed_units()[&id].pilot(), None);
}

#[tokio::test]
async fn override_requires_wizard_and_corrupt_countdowns_fail_loading() {
    let (_dir, config, mut world, id) = fixture().await;
    stompymux_rs::release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(stompymux_rs::Flag::Wizard);
    assign_battle_pilot(&mut world, id, ObjectId(2)).unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let text = support::run_text(&scripts, &config, ObjectId(2), 2, "startup override");
    assert!(text.contains("Insufficient access"), "{text}");
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].power(),
        BattlePower::Off
    );
    let text = support::run_text(&scripts, &config, ObjectId(2), 2, "startup");
    assert!(text.contains("Startup Cycle"), "{text}");
    let world = scripts.world().clone();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(config.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    let mut unit = support::unit_record(&config.database(), "btech_units", id).await;
    unit["power"]["remaining"] = 0.into();
    support::store_unit_record(&mut sql, "btech_units", id, &unit).await;
    assert!(
        format!(
            "{:#}",
            persistence::load(&config.database()).await.unwrap_err()
        )
        .contains("Invalid startup countdown")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn server_tick_retries_failed_countdowns_without_publishing_completion() {
    tokio::task::LocalSet::new().run_until(async {
        let (_dir,config,_world,id)=fixture().await;
        let (addr,shutdown,task,_lua)=support::start(&config,std::rc::Rc::new(std::cell::Cell::new(1))).await;
        let mut client=support::Client { socket:tokio::net::TcpStream::connect(addr).await.unwrap(),pending:Vec::new() };
        client.until("Who are you? ").await;
        client.send("#1").await;
        client.until("Password: ").await;
        client.send("secret").await;
        client.until("Engine lab").await;
        client.send("startup override").await;
        client.until("Startup Cycle commencing").await;
        let mut sql=SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()).foreign_keys(false)).await.unwrap();
        // A running countdown keeps its timer row still, so refuse the tick's commit where
        // every save leaves its mark: the snapshot stamp.
        sqlx::query("CREATE TRIGGER deny_tick BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(ABORT,'tick failure'); END").execute(&mut sql).await.unwrap();
        let initial=persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].power();
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].power(),initial);
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].last_startup(), 0);
        client.send(&format!("@btech inspect #{}",id.0)).await;
        let text=client.until("Power: Starting").await;
        assert!(!text.contains("All systems operational"));
        sqlx::query("DROP TRIGGER deny_tick").execute(&mut sql).await.unwrap();
        client.until("All systems operational!").await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].power(),BattlePower::Running);
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].last_startup(), 1);
        let before_motion=persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].motion().unwrap();
        client.send("speed 10.75").await;
        client.until("Desired speed changed to 10 KPH.").await;
        tokio::time::timeout(std::time::Duration::from_secs(6),async {
            loop {
                let motion=persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].motion().unwrap();
                if motion.point!=before_motion.point {break;}
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }).await.unwrap();
        client.send("speed stop").await;
        client.until("Desired speed changed to 0 KPH.").await;
        tokio::time::timeout(std::time::Duration::from_secs(6),async {
            loop {
                if persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].motion().unwrap().speed==0.0 {break;}
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }).await.unwrap();
        client.send("shutdown").await;
        client.until("All systems shut down.").await;
        let loaded=persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech.constructed_units()[&id].power(),BattlePower::Off);
        assert_eq!(loaded.btech.constructed_units()[&id].pilot(),None);
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn startup_heat_limit_is_strict_durable_and_cannot_be_overridden() {
    for fast in [false, true] {
        for excess in [30.0, 30.0001, 32.0] {
            let (_dir, config, mut world, id) = fixture().await;
            // Seed a precise saved heat boundary without depending on floating-point cooling drift.
            let mut snapshot = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap();
            snapshot["heat"] = serde_json::json!({"stored": excess + 10.0, "excess": excess});
            let mut sql = SqliteConnection::connect_with(
                &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
            )
            .await
            .unwrap();
            support::store_unit_record(&mut sql, "btech_units", id, &snapshot).await;
            world = persistence::load(&config.database()).await.unwrap();
            let before = world.btech.clone();
            let result = start_battle_unit(&mut world, id, ObjectId(1), fast);
            if excess > 30.0 {
                assert!(result.unwrap_err().to_string().contains("too hot"));
                assert_eq!(world.btech, before);
                assert!(advance_battle_units(&mut world, 0).is_empty());
                for _ in 0..12 {
                    stompymux_rs::advance_battle_heat(&mut world);
                }
                assert!(world.btech.constructed_units()[&id].heat().excess < 30.0);
                start_battle_unit(&mut world, id, ObjectId(1), fast).unwrap();
            } else {
                result.unwrap();
            }
            assert_eq!(
                world.btech.constructed_units()[&id].power(),
                BattlePower::Starting {
                    remaining: if fast { 5 } else { 30 }
                }
            );
            advance_battle_units(&mut world, 0);
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
        }
    }
}

#[tokio::test]
async fn idle_map_smoke_ticks_retry_failed_saves_and_expire() {
    use stompymux_rs::{
        BattleDecoration, BattleDecorationKind, BattleHexCoordinate, Terrain, set_map_decoration,
    };
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world) = support::isolated_world().await;
        let map = world.create(&config, "Smoke field".into(), Kind::Room);
        create_battle_map(&mut world, map, "smoke.map", BattleMapAsset::parse("1 1\n\"2\n").unwrap()).unwrap();
        let coordinate = BattleHexCoordinate { x: 0, y: 0 };
        let smoke = BattleDecoration::new(BattleDecorationKind::Smoke, 2, None);
        set_map_decoration(&mut world, map, coordinate, Some(smoke)).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut sql = SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()).foreign_keys(false)).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_smoke_update BEFORE UPDATE ON btech_map_decorations BEGIN SELECT RAISE(ABORT,'smoke update failure'); END; CREATE TRIGGER deny_smoke_delete BEFORE DELETE ON btech_map_decorations BEGIN SELECT RAISE(ABORT,'smoke delete failure'); END;").execute(&mut sql).await.unwrap();
        let (_addr, shutdown, task, _lua) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        tokio::time::sleep(std::time::Duration::from_millis(2200)).await;
        // The first tick only stores the clock, since the smoke's expiry deadline is fixed;
        // the expiry tick that must delete the row keeps failing.
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech.maps()[&map].decoration(coordinate).unwrap(), Some(BattleDecoration { remaining: 1, ..smoke }));
        sqlx::raw_sql("DROP TRIGGER deny_smoke_update; DROP TRIGGER deny_smoke_delete;").execute(&mut sql).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(6), async {
            loop {
                let loaded = persistence::load(&config.database()).await.unwrap();
                if loaded.btech.maps()[&map].decoration(coordinate).unwrap().is_none() {
                    assert_eq!(loaded.btech.maps()[&map].hex(0, 0).unwrap().terrain(), Terrain::HeavyForest);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }).await.unwrap();
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn idle_map_fire_burnout_retries_random_state_save_failure() {
    use stompymux_rs::{
        BattleDecoration, BattleDecorationKind, BattleHexCoordinate, Terrain, set_map_decoration,
    };
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world) = support::isolated_world().await;
        let map = world.create(&config, "Burnout field".into(), Kind::Room);
        create_battle_map(&mut world, map, "fire.map", BattleMapAsset::parse("1 1\n\"2\n").unwrap()).unwrap();
        let coordinate = BattleHexCoordinate { x: 0, y: 0 };
        set_map_decoration(&mut world, map, coordinate, Some(BattleDecoration::new(BattleDecorationKind::Fire, 60, None))).unwrap();
        // Resume the final burnout phase of an already spreading fire.
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["maps"][map.0.to_string()]["decorations"]["0"]["remaining"] = 1.into();
        encoded["maps"][map.0.to_string()]["decorations"]["0"]["next_spread"] = serde_json::Value::Null;
        world.btech = serde_json::from_value(encoded).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let expected = world.btech.clone();
        let mut sql = SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()).foreign_keys(false)).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_fire_random BEFORE UPDATE ON btech_map_random BEGIN SELECT RAISE(ABORT,'fire random failure'); END;").execute(&mut sql).await.unwrap();
        let (_addr, shutdown, task, _lua) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        tokio::time::sleep(std::time::Duration::from_millis(2200)).await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech, expected);
        sqlx::query("DROP TRIGGER deny_fire_random").execute(&mut sql).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(6), async {
            loop {
                let loaded = persistence::load(&config.database()).await.unwrap();
                if loaded.btech.maps()[&map].decoration(coordinate).unwrap().is_none() {
                    assert!(matches!(loaded.btech.maps()[&map].hex(0, 0).unwrap().terrain(), Terrain::Rough | Terrain::Grassland));
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }).await.unwrap();
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn idle_building_repair_retries_failed_world_save() {
    use stompymux_rs::{BattleBuildingState, set_building_state};
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world) = support::isolated_world().await;
        let map = world.create(&config, "Repairing hangar".into(), Kind::Room);
        create_battle_map(&mut world, map, "inside.map", BattleMapAsset::parse("1 1\n.0\n").unwrap()).unwrap();
        set_building_state(&mut world, map, BattleBuildingState { integrity: 9, maximum_integrity: 10, flags: 0, regeneration: 1 }).unwrap();
        // Resume one committed second before the final repair of an otherwise idle map.
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["maps"][map.0.to_string()]["building_repair"] = 1.into();
        world.btech = serde_json::from_value(encoded).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let expected = world.btech.clone();
        let mut sql = SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("CREATE TRIGGER deny_repair BEFORE DELETE ON btech_building_repair BEGIN SELECT RAISE(ABORT,'repair failure'); END").execute(&mut sql).await.unwrap();
        let (_addr, shutdown, task, _lua) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        tokio::time::sleep(std::time::Duration::from_millis(2200)).await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech, expected);
        sqlx::query("DROP TRIGGER deny_repair").execute(&mut sql).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(6), async {
            loop {
                let loaded = persistence::load(&config.database()).await.unwrap();
                if loaded.btech.maps()[&map].building_repair.is_none() {
                    assert_eq!(loaded.btech.maps()[&map].building.integrity, 10);
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }).await.unwrap();
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn shutdown_inferno_expiry_retries_failed_save() {
    use stompymux_rs::apply_inferno_burn;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir,config,mut world,id) = fixture().await;
        assert_eq!(world.btech.constructed_units()[&id].power(),BattlePower::Off);
        apply_inferno_burn(&mut world,id,1).unwrap();
        persistence::save(&config.database(),&world).await.unwrap();
        let expected = world.btech.clone();
        let mut sql = SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("CREATE TRIGGER deny_inferno BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'inferno failure'); END").execute(&mut sql).await.unwrap();
        let (_addr,shutdown,task,_lua) = support::start(&config,std::rc::Rc::new(std::cell::Cell::new(1))).await;
        tokio::time::sleep(std::time::Duration::from_millis(2200)).await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech,expected);
        sqlx::query("DROP TRIGGER deny_inferno").execute(&mut sql).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(6),async {
            loop {
                let loaded = persistence::load(&config.database()).await.unwrap();
                if loaded.btech.constructed_units()[&id].inferno_remaining()==0 { break; }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }).await.unwrap();
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

/// A shutdown radio keeps the server tick active until its saved XP gate expires.
#[tokio::test]
async fn shutdown_radio_xp_gate_retries_failed_save() {
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world, id) = fixture().await;
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["constructed"][id.0.to_string()]["radio_experience_remaining"] = 1.into();
        world.btech = serde_json::from_value(encoded).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let expected = world.btech.clone();
        let mut sql = SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        // The gate expiring removes a timer row; refuse the commit at the snapshot stamp.
        sqlx::query("CREATE TRIGGER deny_radio_xp BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(ABORT,'radio XP failure'); END").execute(&mut sql).await.unwrap();
        let (_addr, shutdown, task, _lua) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        tokio::time::sleep(std::time::Duration::from_millis(2200)).await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech, expected);
        sqlx::query("DROP TRIGGER deny_radio_xp").execute(&mut sql).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(6), async {
            loop {
                let loaded = persistence::load(&config.database()).await.unwrap();
                if loaded.btech.constructed_units()[&id].radio_experience_remaining() == 0 { break; }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }).await.unwrap();
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}
