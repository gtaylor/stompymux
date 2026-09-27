//! Missing building repair clocks resume on load without resetting committed progress.
use crate::support;
use sqlx::{Connection, SqliteConnection};
use stompymux_rs::*;

/// One referenced interior and one unrelated map with equal construction state.
async fn fixture(integrity: i64) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let exterior = world.create(&config, "Exterior".into(), Kind::Room);
    let interior = world.create(&config, "Workshop".into(), Kind::Room);
    let unrelated = world.create(&config, "Unreferenced map".into(), Kind::Room);
    for id in [exterior, interior, unrelated] {
        create_battle_map(
            &mut world,
            id,
            "workshop",
            BattleMapAsset::parse("2 2\n.0.0\n.0.0\n").unwrap(),
        )
        .unwrap();
    }
    for id in [interior, unrelated] {
        set_building_state(
            &mut world,
            id,
            BattleBuildingState {
                integrity,
                maximum_integrity: 10,
                regeneration: 2,
                flags: 0,
            },
        )
        .unwrap();
    }
    for (ordinal, x) in [(0, 0), (1, 1)] {
        set_building_entrance(
            &mut world,
            exterior,
            ordinal,
            Some(BattleBuildingEntrance {
                coordinate: BattleHexCoordinate { x, y: 0 },
                interior,
                data_char: 0,
                data_short: 0,
                data_int: 0,
            }),
        )
        .unwrap();
    }
    persistence::save(&config.database(), &world).await.unwrap();
    (dir, config, world, interior, unrelated)
}

#[tokio::test]
async fn missing_timer_is_recovered_read_only_and_first_save_inserts_it() {
    let (_dir, config, _, interior, unrelated) = fixture(4).await;
    let mut sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    let mut world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech.maps()[&interior].building_repair, Some(120));
    assert_eq!(world.btech.maps()[&unrelated].building_repair, None);
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM btech_building_repair")
        .fetch_one(&mut sql)
        .await
        .unwrap();
    assert_eq!(rows, 0, "Loading must not mutate the database");
    // Even an unchanged save must materialize an inferred countdown.
    persistence::save(&config.database(), &world).await.unwrap();
    // The countdown is stored as its deadline on the (unstarted) simulation clock.
    let repairs_at: i64 =
        sqlx::query_scalar("SELECT repairs_at FROM btech_building_repair WHERE map_dbref=?")
            .bind(interior.0)
            .fetch_one(&mut sql)
            .await
            .unwrap();
    assert_eq!(repairs_at, world.btech.simulation_time() + 120);
    sqlx::query("ALTER TABLE btech_building_repair ADD COLUMN note TEXT DEFAULT 'keep'")
        .execute(&mut sql)
        .await
        .unwrap();
    for _ in 0..119 {
        advance_building_repairs(&mut world);
    }
    assert_eq!(world.btech.maps()[&interior].building.integrity, 4);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut replay = persistence::load(&config.database()).await.unwrap();
    assert_eq!(replay.btech.maps()[&interior].building_repair, Some(1));
    advance_building_repairs(&mut replay);
    assert_eq!(replay.btech.maps()[&interior].building.integrity, 6);
    assert_eq!(replay.btech.maps()[&interior].building_repair, Some(120));
    persistence::save(&config.database(), &replay)
        .await
        .unwrap();
    let note: String =
        sqlx::query_scalar("SELECT note FROM btech_building_repair WHERE map_dbref=?")
            .bind(interior.0)
            .fetch_one(&mut sql)
            .await
            .unwrap();
    assert_eq!(note, "keep");
    for _ in 0..240 {
        advance_building_repairs(&mut replay);
    }
    assert_eq!(replay.btech.maps()[&interior].building.integrity, 10);
    assert_eq!(replay.btech.maps()[&interior].building_repair, None);
    persistence::save(&config.database(), &replay)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        replay.btech
    );
}

#[tokio::test]
async fn first_recovered_tick_is_atomic_when_timer_storage_rejects_the_save() {
    let (_dir, config, _, interior, _) = fixture(4).await;
    let mut sql = SqliteConnection::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TRIGGER reject_timer BEFORE INSERT ON btech_building_repair BEGIN SELECT RAISE(ABORT,'clock failure'); END").execute(&mut sql).await.unwrap();
    let mut world = persistence::load(&config.database()).await.unwrap();
    advance_building_repairs(&mut world);
    assert_eq!(world.btech.maps()[&interior].building_repair, Some(119));
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert_eq!(
        persistence::load(&config.database())
            .await
            .unwrap()
            .btech
            .maps()[&interior]
            .building_repair,
        Some(120)
    );
    sqlx::query("DROP TRIGGER reject_timer")
        .execute(&mut sql)
        .await
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

/// Recovery inferred during startup activates an otherwise idle server's committed heartbeat.
#[tokio::test]
async fn startup_resumes_missing_repair_without_any_active_units() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_dir, config, world, interior, _) = fixture(4).await;
            assert!(!building_repair_pending(&world));
            let (_address, shutdown, task, _scripts) =
                support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
            tokio::time::timeout(std::time::Duration::from_secs(6), async {
                loop {
                    let loaded = persistence::load(&config.database()).await.unwrap();
                    let map = &loaded.btech.maps()[&interior];
                    if map.building_repair.is_some_and(|remaining| remaining < 120) {
                        assert_eq!(map.building.integrity, 4);
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                }
            })
            .await
            .unwrap();
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}
