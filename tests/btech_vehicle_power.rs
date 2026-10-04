//! Ground-vehicle cockpit power transitions, durable countdowns and server save-failure recovery.
use crate::support;
use sqlx::{Connection, SqliteConnection};
use stompymux_rs::{
    Config, Kind, MapAsset, ObjectId, Power, Scripts, ShutdownRequest, VehicleTemplate, World,
    advance_battle_units, assign_battle_pilot, create_battle_map, create_battle_vehicle,
    persistence, place_battle_unit, remove_battle_unit, start_battle_unit, stop_battle_unit,
};

/// A piloted, placed tracked vehicle in an isolated database, ready for a power transition.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    world.accounts.get_mut(&ObjectId(1)).unwrap().hash =
        Some(stompymux_rs::accounts::hash("secret", &config).unwrap());
    let map = world.create(&config, "Battlefield".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test.map",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, "Engine lab".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        VehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
            .unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
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
                world.btech.vehicles()[&id].power(),
                Power::Starting { remaining: 18 }
            );
        }
    }
    assert_eq!(
        messages,
        [
            "Powerplant initialized and online.",
            "Auto-aligning drive wheels.",
            "Adjusting track tension.",
            "Scanners are now operational.",
            "Targeting system is now operational.",
            "All systems operational!"
        ]
    );
    assert_eq!(world.btech.vehicles()[&id].power(), Power::Running);
    assert!(advance_battle_units(&mut world, 0).is_empty());
    assert!(remove_battle_unit(&mut world, id, ObjectId(config.start())).is_err());
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::MovementRules::STANDARD.fall,
    )
    .unwrap();
    assert_eq!(world.btech.vehicles()[&id].power(), Power::Off);
    assert_eq!(world.btech.vehicles()[&id].pilot(), None);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
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
    assert_eq!(scripts.world().btech.vehicles()[&id].power(), Power::Off);
    assert_eq!(scripts.world().btech.vehicles()[&id].pilot(), None);
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
    support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let text = support::run_text(&scripts, &config, ObjectId(2), 2, "startup override");
    assert!(text.contains("Insufficient access"), "{text}");
    assert_eq!(scripts.world().btech.vehicles()[&id].power(), Power::Off);
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
    let mut unit = support::unit_record(&config.database(), "btech_vehicles", id).await;
    unit["power"]["remaining"] = 0.into();
    support::store_unit_record(&mut sql, "btech_vehicles", id, &unit).await;
    assert!(
        format!(
            "{:#}",
            persistence::load(&config.database()).await.unwrap_err()
        )
        .contains("Invalid vehicle startup countdown")
    );
}

#[tokio::test(flavor = "current_thread")]
async fn server_tick_retries_failed_countdowns_without_publishing_completion() {
    tokio::task::LocalSet::new().run_until(async {
        let (_dir,config,_world,id)=fixture().await;
        let (addr,shutdown,task,_lua,mut heartbeats)=support::start(&config,std::rc::Rc::new(std::cell::Cell::new(1))).await;
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
        let initial=persistence::load(&config.database()).await.unwrap().btech.vehicles()[&id].power();
        heartbeats.attempt().await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.vehicles()[&id].power(),initial);
        sqlx::query("DROP TRIGGER deny_tick").execute(&mut sql).await.unwrap();
        client.until_heartbeats("All systems operational!", &mut heartbeats, 20).await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.vehicles()[&id].power(),Power::Running);
        let before_point = persistence::load(&config.database()).await.unwrap().btech.vehicles()[&id].motion().unwrap().point;
        client.send("speed 10.75").await;
        client.until("Desired speed changed to 10 KPH.").await;
        heartbeats.until_saved(&config, 20, |saved| saved.btech.vehicles()[&id].motion().unwrap().point != before_point).await;
        client.send("shutdown").await;
        client.until("All systems shut down.").await;
        let loaded=persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech.vehicles()[&id].power(),Power::Off);
        assert_eq!(loaded.btech.vehicles()[&id].pilot(),None);
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn native_vehicle_pilot_departure_and_hull_destruction_reconcile_state() {
    let (_dir, config, mut world, id) = fixture().await;
    stompymux_rs::release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "pilot");
    assert!(text.contains("take the cockpit"), "{text}");
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "leave");
    assert!(
        scripts.world().btech.vehicles()[&id].pilot().is_none(),
        "{text}"
    );
    let mut world = scripts.world().clone();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    stompymux_rs::damage_battle_vehicle_phase(
        &mut world,
        id,
        stompymux_rs::VehicleSection::Front,
        8,
        stompymux_rs::DamagePhase::Internal,
    )
    .unwrap();
    assert_eq!(world.btech.vehicles()[&id].power(), Power::Off);
    assert!(start_battle_unit(&mut world, id, ObjectId(1), true).is_err());
    persistence::save(&config.database(), &world).await.unwrap();
}
