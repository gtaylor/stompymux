//! Gunner selections share targeting rules while retaining independent state and timers.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
use crate::support;

/// Create an independently occupied station without disturbing the parent's assigned pilot.
fn station(
    world: &mut World,
    config: &Config,
    parent: ObjectId,
    name: &str,
) -> (ObjectId, ObjectId) {
    let room = world.create(config, name.into(), Kind::Thing);
    let gunner = world.create(config, format!("{name} gunner"), Kind::Player);
    register_gunner_station(world, ObjectId(1), room, parent, 0).unwrap();
    world.objects.get_mut(&gunner).unwrap().location = Some(room);
    (room, gunner)
}

#[tokio::test]
async fn independent_native_lua_locks_settle_and_restart_for_every_chassis() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, target, _) =
            firing::fixture_with_target(&template, None, &template).await;
        let (first, gunner) = station(&mut world, &config, parent, "First");
        let (second, other) = station(&mut world, &config, parent, "Second");
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        gunner_station_action(&scripts, first, gunner, true).unwrap();
        gunner_station_action(&scripts, second, other, true).unwrap();
        scripts.drain_outbox();
        let parent_before = scripts.world().btech.clone();
        let output =
            support::run_text(&scripts, &config, gunner, 4, &format!("lock #{}", target.0));
        assert!(output.contains("Target set"), "{output}");
        scripts
            .eval_callback::<()>(&format!(
                "btech.gunner.lock_hex({},{},0,0,'H')",
                second.0, other.0
            ))
            .unwrap();
        assert_eq!(
            scripts.world().btech.constructed_units(),
            parent_before.constructed_units()
        );
        assert_eq!(scripts.world().btech.vehicles(), parent_before.vehicles());
        for _ in 0..3 {
            advance_battle_target_locks(&mut scripts.world_mut());
        }
        let saved = scripts.world().clone();
        assert_eq!(saved.btech.gunner_stations()[&first].lock_remaining, 5);
        persistence::save(&config.database(), &saved).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            loaded.btech.gunner_stations(),
            saved.btech.gunner_stations()
        );
        for _ in 0..4 {
            assert!(
                !advance_battle_target_locks(&mut loaded)
                    .iter()
                    .any(|n| n.unit == first || n.unit == second)
            );
        }
        let notices = advance_battle_target_locks(&mut loaded);
        assert!(
            notices
                .iter()
                .any(|n| n.unit == first && n.text.contains("stable lock"))
        );
        assert!(
            notices
                .iter()
                .any(|n| n.unit == second && n.text.contains("(0,0)"))
        );
        assert_eq!(loaded.btech.gunner_stations()[&first].lock_remaining, 0);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
        let before = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.gunner.lock({},{},nil);error('abort')",
                    first.0, gunner.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
        scripts
            .eval_callback::<()>(&format!("btech.gunner.lock({},{},nil)", first.0, gunner.0))
            .unwrap();
        assert!(
            scripts.world().btech.gunner_stations()[&first]
                .target_selection()
                .is_none()
        );
        assert_eq!(
            scripts.world().btech.gunner_stations()[&second],
            before.gunner_stations()[&second]
        );
        for mode in ["B", "I", "C", "unit_at_hex"] {
            scripts
                .eval_callback::<()>(&format!(
                    "btech.gunner.lock_hex({},{},0,0,'{mode}')",
                    first.0, gunner.0
                ))
                .unwrap();
            let Some(BattleTargetSelection::Hex(lock)) =
                scripts.world().btech.gunner_stations()[&first].target_selection()
            else {
                panic!("missing coordinate lock")
            };
            assert_eq!(lock.mode, mode.parse().unwrap());
        }
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database())
                .await
                .unwrap()
                .btech
                .gunner_stations(),
            saved.btech.gunner_stations()
        );
    }
}

#[tokio::test]
async fn admission_and_map_transfer_cannot_leak_or_retain_station_locks() {
    let template = &firing::templates()[0];
    let (_dir, config, mut world, parent, target, _) =
        firing::fixture_with_target(template, None, template).await;
    let target_pilot = world.create(&config, "Target pilot".into(), Kind::Player);
    world.objects.get_mut(&target_pilot).unwrap().location = Some(target);
    assign_battle_pilot(&mut world, target, target_pilot).unwrap();
    let (station, gunner) = station(&mut world, &config, parent, "Station");
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(select_battle_target(&mut scripts.world_mut(), station, gunner, Some(target)).is_err());
    gunner_station_action(&scripts, station, gunner, true).unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        select_battle_target(&mut scripts.world_mut(), station, ObjectId(1), Some(target)).is_err()
    );
    assert!(select_battle_target(&mut scripts.world_mut(), station, gunner, Some(parent)).is_err());
    assert!(
        select_battle_hex_target(
            &mut scripts.world_mut(),
            station,
            gunner,
            BattleHexCoordinate { x: -1, y: 0 },
            BattleHexTargetMode::Hex
        )
        .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    select_battle_target(&mut scripts.world_mut(), station, gunner, Some(target)).unwrap();
    stop_battle_unit(
        &mut scripts.world_mut(),
        target,
        target_pilot,
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    remove_battle_unit(&mut scripts.world_mut(), target, ObjectId(config.home())).unwrap();
    assert!(
        scripts.world().btech.gunner_stations()[&station]
            .target_selection()
            .is_none()
    );
    select_battle_hex_target(
        &mut scripts.world_mut(),
        station,
        gunner,
        BattleHexCoordinate { x: 0, y: 0 },
        BattleHexTargetMode::Hex,
    )
    .unwrap();
    stop_battle_unit(
        &mut scripts.world_mut(),
        parent,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    remove_battle_unit(&mut scripts.world_mut(), parent, ObjectId(config.home())).unwrap();
    assert!(
        scripts.world().btech.gunner_stations()[&station]
            .target_selection()
            .is_none()
    );
    assert!(select_battle_target(&mut scripts.world_mut(), station, gunner, None).is_err());
}

#[tokio::test]
async fn settling_writes_retry_and_parent_purge_discards_timer() {
    use sqlx::Connection;
    let template = &firing::templates()[0];
    let (_dir, config, mut world, parent, target, _) =
        firing::fixture_with_target(template, None, template).await;
    let (station, gunner) = station(&mut world, &config, parent, "Station");
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    gunner_station_action(&scripts, station, gunner, true).unwrap();
    select_battle_target(&mut scripts.world_mut(), station, gunner, Some(target)).unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.gunner.lock_hex({},1,0,0,'H')", parent.0))
            .is_err()
    );
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let mut db = sqlx::SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER block_gunner_clock BEFORE UPDATE ON btech_gunner_lock_timers BEGIN SELECT RAISE(FAIL,'clock write failure'); END").execute(&mut db).await.unwrap();
    let mut next = saved.clone();
    advance_battle_target_locks(&mut next);
    assert!(persistence::save(&config.database(), &next).await.is_err());
    assert_eq!(
        persistence::load(&config.database())
            .await
            .unwrap()
            .btech
            .gunner_stations()[&station]
            .lock_remaining,
        8
    );
    sqlx::query("DROP TRIGGER block_gunner_clock")
        .execute(&mut db)
        .await
        .unwrap();
    persistence::save(&config.database(), &next).await.unwrap();
    assert_eq!(
        persistence::load(&config.database())
            .await
            .unwrap()
            .btech
            .gunner_stations()[&station]
            .lock_remaining,
        7
    );
    next.objects
        .get_mut(&parent)
        .unwrap()
        .flags
        .insert(Flag::Going);
    persistence::save(&config.database(), &next).await.unwrap();
    persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
        dbck::plan(&next, raw, &config)
    })
    .await
    .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        loaded.btech.gunner_stations()[&station].parent,
        ObjectId(-1)
    );
    assert_eq!(loaded.btech.gunner_stations()[&station].lock_remaining, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM btech_gunner_lock_timers")
            .fetch_one(&mut db)
            .await
            .unwrap(),
        0
    );
    db.close().await.unwrap();
}

#[tokio::test(flavor = "current_thread")]
async fn idle_server_settles_station_lock_after_parent_shutdown() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let template = &firing::templates()[0];
            let (_dir, config, mut world, parent, target, _) =
                firing::fixture_with_target(template, None, template).await;
            let (station, gunner) = station(&mut world, &config, parent, "Gunner station");
            let target_pilot = world.create(&config, "Target pilot".into(), Kind::Player);
            world.objects.get_mut(&target_pilot).unwrap().location = Some(target);
            assign_battle_pilot(&mut world, target, target_pilot).unwrap();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            gunner_station_action(&scripts, station, gunner, true).unwrap();
            select_battle_hex_target(
                &mut scripts.world_mut(),
                station,
                gunner,
                BattleHexCoordinate { x: 0, y: 0 },
                BattleHexTargetMode::Hex,
            )
            .unwrap();
            let mut world = scripts.world().clone();
            for (unit, pilot) in [(parent, ObjectId(1)), (target, target_pilot)] {
                stop_battle_unit(&mut world, unit, pilot, BattleMovementRules::STANDARD.fall)
                    .unwrap();
            }
            assert!(optical_scanner_observers(&world).is_empty());
            world.accounts.entry(gunner).or_default().hash =
                Some(accounts::hash("secret", &config).unwrap());
            persistence::save(&config.database(), &world).await.unwrap();
            let (address, shutdown, task, _) =
                support::start(&config, Rc::new(std::cell::Cell::new(1))).await;
            let mut client = support::Client {
                socket: tokio::net::TcpStream::connect(address).await.unwrap(),
                pending: vec![],
            };
            client.until("Who are you? ").await;
            client.send(&format!("#{}", gunner.0)).await;
            client.until("Password: ").await;
            client.send("secret").await;
            client
                .until("The sensors acquire a stable lock on (0,0).")
                .await;
            assert_eq!(
                persistence::load(&config.database())
                    .await
                    .unwrap()
                    .btech
                    .gunner_stations()[&station]
                    .lock_remaining,
                0
            );
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}

#[tokio::test]
async fn explicit_arc_stations_lock_immediately_without_changing_parent_timers() {
    let template = &firing::templates()[0];
    let (_dir, config, mut world, parent, target, _) =
        firing::fixture_with_target(template, None, template).await;
    let station = world.create(&config, "Arc station".into(), Kind::Thing);
    let gunner = world.create(&config, "Arc gunner".into(), Kind::Player);
    register_gunner_station(&mut world, ObjectId(1), station, parent, 5).unwrap();
    world.objects.get_mut(&gunner).unwrap().location = Some(station);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    gunner_station_action(&scripts, station, gunner, true).unwrap();
    scripts.drain_outbox();
    let before = scripts.world().btech.clone();
    assert_eq!(
        support::run_text(&scripts, &config, gunner, 1, &format!("lock #{}", target.0)),
        "Target set."
    );
    assert_eq!(
        scripts.world().btech.gunner_stations()[&station].lock_remaining,
        0
    );
    assert_eq!(
        scripts.world().btech.constructed_units(),
        before.constructed_units()
    );
    let map = scripts.world().btech.units()[&parent].map.unwrap();
    set_battle_map_hex_action(
        &scripts,
        &config,
        ObjectId(1),
        map,
        BattleHexCoordinate { x: 0, y: 1 },
        Terrain::Grassland,
        3,
    )
    .unwrap();
    scripts
        .eval_callback::<()>(&format!(
            "btech.gunner.lock_hex({},{},0,1)",
            station.0, gunner.0
        ))
        .unwrap();
    let saved = scripts.world().clone();
    let lock = &saved.btech.gunner_stations()[&station];
    assert_eq!(lock.lock_remaining, 0);
    assert_eq!(lock.lock_modes & 0xf8000, 0x8000);
    assert_eq!(lock.target_coordinates, [0, 1, 3]);
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database())
            .await
            .unwrap()
            .btech
            .gunner_stations(),
        saved.btech.gunner_stations()
    );
}
