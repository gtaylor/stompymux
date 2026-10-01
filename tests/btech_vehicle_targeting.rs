//! Vehicle target selections own countdowns and survive transactional scan and persistence paths.
use crate::support;
use stompymux_rs::*;

/// Two Mechs and two vehicles at opposite ends of a north/south lane.
async fn fixture(
    tiles: &str,
    vehicle: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 4]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Sight lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "sight",
        BattleMapAsset::parse(&format!("1 5\n{tiles}")).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for index in 0..4 {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if index < 2 {
            let mut definition =
                BattleTemplate::parse("JR7-D",include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
            definition
                .attributes
                .insert("specials".into(), "FlipArms Searchlight".into());
            create_battle_unit(&mut world, id, definition).unwrap();
        } else {
            create_battle_vehicle(
                &mut world,
                id,
                BattleVehicleTemplate::parse("test",vehicle).unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut world, id, map, 0, if index % 2 == 0 { 4 } else { 0 }).unwrap();
        ids.push(id);
    }
    (dir, config, world, map, ids.try_into().unwrap())
}

/// Assign scenario power without introducing crew actions into sensor tests.
fn power(world: &mut World, ids: &[ObjectId], value: BattlePower) {
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    for id in ids {
        let class = if world.btech.vehicles().contains_key(id) {
            "vehicles"
        } else {
            "constructed"
        };
        saved[class][id.0.to_string()]["power"] = serde_json::to_value(value).unwrap();
    }
    world.btech = serde_json::from_value(saved).unwrap();
}

/// A close mixed formation guarantees contact acquisition without a random fixture dependency.
async fn formation() -> (tempfile::TempDir, Config, World, [ObjectId; 4]) {
    let (dir, config, mut world, map, ids) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/mechs/Demolisher.toml"),
    )
    .await;
    for id in ids {
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    power(&mut world, &ids, BattlePower::Running);
    (dir, config, world, ids)
}

#[tokio::test]
async fn vehicle_unit_and_coordinate_selections_settle_replay_and_drive_scans() {
    let (_dir, config, initial, [mech, _, observer, vehicle]) = formation().await;
    for target in [mech, vehicle] {
        let mut world = initial.clone();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
        assign_battle_pilot(&mut world, observer, ObjectId(1)).unwrap();
        refresh_battle_contacts(&mut world, &[observer]).unwrap();
        let before = world.btech.clone();
        assert!(select_battle_target(&mut world, observer, ObjectId(2), Some(target)).is_err());
        assert!(select_battle_target(&mut world, observer, ObjectId(1), Some(observer)).is_err());
        assert_eq!(world.btech, before);
        select_battle_target(&mut world, observer, ObjectId(1), Some(target)).unwrap();
        assert_eq!(
            world.btech.vehicles()[&observer]
                .target_lock()
                .unwrap()
                .remaining,
            8
        );
        for _ in 0..3 {
            assert!(advance_battle_target_locks(&mut world).is_empty());
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for remaining in (0..5).rev() {
            let notices = advance_battle_target_locks(&mut world);
            assert_eq!(notices, advance_battle_target_locks(&mut restored));
            assert_eq!(notices.len(), usize::from(remaining == 0));
            assert_eq!(
                world.btech.vehicles()[&observer]
                    .target_lock()
                    .unwrap()
                    .remaining,
                remaining
            );
        }
        assert_eq!(world.btech, restored.btech);
        assert!(advance_battle_target_locks(&mut world).is_empty());
        let expected = scan_battle_unit(&world, observer, ObjectId(1), target, "").unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        assert!(support::run_text(&scripts, &config, ObjectId(1), 1, "scan").contains(&expected));
        let report = report_battle_unit(&scripts.world(), observer, ObjectId(1), target).unwrap();
        assert!(support::run_text(&scripts, &config, ObjectId(1), 1, "report").contains(&report));
        let before = scripts.world().btech.clone();
        scripts.drain_outbox();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.lock({},1,nil); error('abort')",
                    observer.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
        let text = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("lock #{}", target.0),
        );
        assert!(text.contains("Target set"), "{text}");
        let remaining: u8 = scripts
            .eval_callback(&format!(
                "return btech.unit.state({}).target_lock.remaining",
                observer.0
            ))
            .unwrap();
        assert_eq!(remaining, 8);
        support::run_text(&scripts, &config, ObjectId(1), 1, "lock 0 0");
        assert!(
            scripts.world().btech.vehicles()[&observer]
                .target_lock()
                .is_none()
        );
        assert!(
            scripts.world().btech.vehicles()[&observer]
                .hex_lock()
                .is_some()
        );
        let scan = support::run_text(&scripts, &config, ObjectId(1), 1, "scan");
        assert!(scan.contains("Type: MECH"), "{scan}");
        for mode in ["H", "B", "I", "C"] {
            let text = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("lock 0 1 {mode}"),
            );
            assert!(text.contains("Target coordinates set"), "{text}");
            let scan = support::run_text(&scripts, &config, ObjectId(1), 1, "scan");
            assert!(!scan.contains("not constructed"), "{scan}");
        }
        support::run_text(&scripts, &config, ObjectId(1), 1, "lock -");
        assert!(
            scripts.world().btech.vehicles()[&observer]
                .target_selection()
                .is_none()
        );
    }
}

/// Losing every perception channel, placement, removal, shutdown and destruction clear vehicle
/// locks; light changes alone do not.
#[tokio::test]
async fn vehicle_locks_clear_on_visibility_sensor_placement_and_power_changes() {
    let (_dir, config, mut initial, [mech, _, observer, vehicle]) = formation().await;
    let map = initial.btech.vehicles()[&observer].position().unwrap().map;
    // Sight still reaches a target sharing the observer's hex, so hold this one four hexes off.
    power(&mut initial, &[vehicle], BattlePower::Off);
    place_battle_unit(&mut initial, vehicle, map, 0, 4).unwrap();
    power(&mut initial, &[vehicle], BattlePower::Running);
    initial.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
    assign_battle_pilot(&mut initial, observer, ObjectId(1)).unwrap();
    refresh_battle_contacts(&mut initial, &[observer]).unwrap();
    select_battle_target(&mut initial, observer, ObjectId(1), Some(vehicle)).unwrap();
    let mut world = initial.clone();
    set_battle_map_perception(&mut world, map, BattleMapPerceptionFlag::Sensors, false).unwrap();
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 0).unwrap();
    let events = refresh_battle_contacts(&mut world, &[observer]).unwrap();
    assert!(
        events
            .iter()
            .any(|event| event.target == vehicle && event.lock_lost)
    );
    assert!(
        world.btech.vehicles()[&observer]
            .target_selection()
            .is_none()
    );
    let mut world = initial.clone();
    power(&mut world, &[vehicle], BattlePower::Off);
    place_battle_unit(&mut world, vehicle, map, 0, 1).unwrap();
    assert!(
        world.btech.vehicles()[&observer]
            .target_selection()
            .is_none()
    );
    let mut world = initial.clone();
    select_battle_target(&mut world, observer, ObjectId(1), Some(mech)).unwrap();
    power(&mut world, &[mech], BattlePower::Off);
    remove_battle_unit(&mut world, mech, ObjectId(config.home())).unwrap();
    assert!(
        world.btech.vehicles()[&observer]
            .target_selection()
            .is_none()
    );
    let mut world = initial.clone();
    stop_battle_unit(
        &mut world,
        observer,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert!(
        world.btech.vehicles()[&observer]
            .target_selection()
            .is_none()
    );
    let mut world = initial.clone();
    // Light changes wait for the next scan, where the sensor band still reaches the target.
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 30).unwrap();
    assert!(world.btech.vehicles()[&observer].target_lock().is_some());
    refresh_battle_contacts(&mut world, &[observer]).unwrap();
    assert!(world.btech.vehicles()[&observer].target_lock().is_some());
    select_battle_hex_target(
        &mut world,
        observer,
        ObjectId(1),
        BattleHexCoordinate { x: 0, y: 1 },
        BattleHexTargetMode::Building,
    )
    .unwrap();
    let coordinate_target = world.btech.vehicles()[&observer].target_selection();
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 30).unwrap();
    refresh_battle_contacts(&mut world, &[observer]).unwrap();
    assert_eq!(
        world.btech.vehicles()[&observer].target_selection(),
        coordinate_target
    );
    let mut world = initial.clone();
    world
        .objects
        .get_mut(&vehicle)
        .unwrap()
        .flags
        .insert(Flag::Going);
    persistence::save(&config.database(), &world).await.unwrap();
    persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
        dbck::plan(&world, raw, &config)
    })
    .await
    .unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(
        restored.btech.vehicles()[&observer]
            .target_selection()
            .is_none()
    );
}

#[tokio::test]
async fn vehicle_lock_snapshots_reject_invalid_countdowns_targets_and_coordinates() {
    let (_dir, config, mut world, [_, _, observer, target]) = formation().await;
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
    assign_battle_pilot(&mut world, observer, ObjectId(1)).unwrap();
    refresh_battle_contacts(&mut world, &[observer]).unwrap();
    select_battle_target(&mut world, observer, ObjectId(1), Some(target)).unwrap();
    for value in [
        serde_json::json!({"target": observer.0,"remaining":8}),
        serde_json::json!({"target":999999,"remaining":8}),
        serde_json::json!({"hex":{"x":-1,"y":0},"mode":"hex","remaining":8}),
    ] {
        let mut bad = world.clone();
        let mut saved = serde_json::to_value(&bad.btech).unwrap();
        saved["vehicles"][observer.0.to_string()]["target_lock"] = value;
        bad.btech = serde_json::from_value(saved).unwrap();
        assert!(bad.validate(&config).is_err());
    }
    for field in ["remaining", "power"] {
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        if field == "remaining" {
            saved["vehicles"][observer.0.to_string()]["target_lock"]["remaining"] =
                serde_json::json!(9);
        } else {
            saved["vehicles"][observer.0.to_string()]["power"] =
                serde_json::to_value(BattlePower::Off).unwrap();
        }
        assert!(serde_json::from_value::<BtechState>(saved).is_err());
    }
}

#[tokio::test]
async fn idle_vehicle_lock_countdown_retries_failed_server_commits() {
    tokio::task::LocalSet::new().run_until(async {
        use sqlx::Connection;
        let (_dir, config, mut world, [a, b, observer, other]) = formation().await;
        power(&mut world, &[a, b, other], BattlePower::Off);
        for id in [a, b, other] { remove_battle_unit(&mut world, id, ObjectId(config.home())).unwrap(); }
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
        assign_battle_pilot(&mut world, observer, ObjectId(1)).unwrap();
        select_battle_hex_target(&mut world, observer, ObjectId(1), BattleHexCoordinate { x: 0, y: 1 }, BattleHexTargetMode::Hex).unwrap();
        assert!(battle_contact_observers(&world).is_empty());
        persistence::save(&config.database(), &world).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        // A settling lock keeps its timer row still; refuse the commit at the snapshot stamp.
        sqlx::raw_sql("CREATE TRIGGER deny_lock BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(ABORT,'lock failure'); END;").execute(&mut sql).await.unwrap();
        let (_address, shutdown, server, _lua) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        let saved = persistence::load(&config.database()).await.unwrap();
        assert_eq!(saved.btech.vehicles()[&observer].hex_lock().unwrap().remaining, 8);
        sqlx::raw_sql("DROP TRIGGER deny_lock").execute(&mut sql).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let saved = persistence::load(&config.database()).await.unwrap();
                if saved.btech.vehicles()[&observer].hex_lock().unwrap().remaining < 8 { break; }
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }).await.unwrap();
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        server.await.unwrap().unwrap();
    }).await;
}
