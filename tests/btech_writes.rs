//! Idle heartbeats write nothing: running countdowns, counting and wrapping clocks and the
//! simulation clock itself are stored so that a tick with no other change needs no
//! database write.
use crate::support::{self, btech_firing as firing};
use stompymux_rs::*;

/// SQLite's count of commits made by other connections to this database.
async fn data_version(db: &mut sqlx::SqliteConnection) -> i64 {
    sqlx::query_scalar("PRAGMA data_version")
        .fetch_one(db)
        .await
        .unwrap()
}

/// Running units standing still, one settling a target lock and one recycling a weapon,
/// reach a steady state in which heartbeats commit without touching the database, then
/// reload exactly. The only writes left are turn-boundary ticks, whose periodic checks
/// can roll dice, and the periodic simulation clock save.
#[tokio::test(flavor = "current_thread")]
async fn running_units_standing_still_write_nothing_per_tick() {
    let templates = firing::templates();
    for template in [&templates[0], &templates[2]] {
        let (dir, _, mut world, shooter, _, index) =
            firing::fixture_with_target(template, None, template).await;
        // A weapon recycling for longer than the test runs holds its timer row still.
        firing::edit(&mut world, shooter, |record| {
            record["weapon_recycle"] = serde_json::json!({ index.to_string(): 120 });
        });
        // The shipped default; the test fixture saves the clock every second.
        let config = support::with_clock_save_interval(dir.path(), 60);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
        )
        .await
        .unwrap();
        let mut harness = HeartbeatHarness::new(config.clone(), world).unwrap();
        // The lock settles and every running count takes its clock form, including a
        // wrap of the thirty-second overheat and turn phases.
        for second in 1..=35 {
            assert!(harness.step(second).await.committed);
        }
        let mut clock_save: Option<i64> = None;
        for second in 36..=100 {
            let before = data_version(&mut db).await;
            assert!(harness.step(second).await.committed);
            if data_version(&mut db).await == before {
                continue;
            }
            let phase = serde_json::to_value(&harness.world().btech).unwrap()["turn_clock"]
                .as_i64()
                .unwrap();
            if phase == 29 || phase == 0 {
                continue;
            }
            assert!(
                clock_save.is_none_or(|previous| second - previous >= 60),
                "heartbeat at second {second} (turn phase {phase}) wrote to the database"
            );
            clock_save = Some(second);
        }
        // A forced save stores the current clock, and loading rebuilds every count.
        let current = harness.world();
        persistence::save(&config.database(), &current)
            .await
            .unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, current.btech);
        sqlx::Connection::close(db).await.unwrap();
    }
}

/// Every harness heartbeat publishes its attempt; only a successful
/// commit counts as committed, so a refused save leaves the committed count alone.
#[tokio::test(flavor = "current_thread")]
async fn heartbeat_progress_counts_attempts_and_commits() {
    let templates = firing::templates();
    // The fixture stores the clock every tick, so the refused tick reaches the database.
    let (_dir, config, world, _, _, _) =
        firing::fixture_with_target(&templates[0], None, &templates[0]).await;
    persistence::save(&config.database(), &world).await.unwrap();
    let mut harness = HeartbeatHarness::new(config.clone(), world).unwrap();
    let progress = harness.scripts().progress();
    assert_eq!(*progress.borrow(), RuntimeProgress::default());
    assert!(harness.step(1).await.committed);
    let first = *progress.borrow();
    assert_eq!(
        (first.heartbeats_attempted, first.heartbeats_committed),
        (1, 1)
    );
    let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER deny_progress BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(ABORT,'progress failure'); END;")
        .execute(&mut db)
        .await
        .unwrap();
    assert!(!harness.step(2).await.committed);
    let refused = *progress.borrow();
    assert_eq!(
        (refused.heartbeats_attempted, refused.heartbeats_committed),
        (2, 1)
    );
    assert_eq!(refused.maintenance_ticks, 0);
    sqlx::Connection::close(db).await.unwrap();
}
