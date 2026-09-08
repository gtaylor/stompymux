//! SQLx connection lifecycle, asynchronous contention and initialization guarantees.
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::{cell::Cell, time::Duration};
use stompymux_rs::{World, persistence};

/// Waiting for a database lock leaves the single-thread Tokio runtime responsive.
#[tokio::test(flavor = "current_thread")]
async fn sqlite_contention_yields_to_other_tasks() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.db");
    let world = World::default();
    persistence::initialize(&path, &world).await.unwrap();
    let mut lock = SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(&path)
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("BEGIN EXCLUSIVE")
        .execute(&mut lock)
        .await
        .unwrap();
    let finished = Cell::new(false);
    tokio::time::timeout(Duration::from_secs(5), async {
        tokio::join!(
            async {
                let result = persistence::save_with_timeout(&path, &world, 2000).await;
                finished.set(true);
                result.unwrap();
            },
            async {
                tokio::time::sleep(Duration::from_millis(50)).await;
                assert!(
                    !finished.get(),
                    "write should still be waiting for the held lock"
                );
                sqlx::raw_sql("ROLLBACK").execute(&mut lock).await.unwrap();
            }
        );
    })
    .await
    .unwrap();
    lock.close().await.unwrap();
    persistence::load(&path).await.unwrap();
}

/// Failed first writes clean up their exclusive destination after closing SQLite.
#[tokio::test(flavor = "current_thread")]
async fn failed_initialization_cleans_up_without_overwriting_existing_files() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("world.db");
    let journal = dir.path().join("world.db-journal");
    // A directory prevents SQLite from opening its rollback journal on first write.
    std::fs::create_dir(&journal).unwrap();
    assert!(
        persistence::initialize(&path, &World::default())
            .await
            .is_err()
    );
    assert!(!path.exists());
    assert!(journal.is_dir());
    std::fs::remove_dir(journal).unwrap();
    persistence::initialize(&path, &World::default())
        .await
        .unwrap();
    use std::os::unix::fs::PermissionsExt;
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let original = std::fs::read(&path).unwrap();
    assert!(
        persistence::initialize(&path, &World::default())
            .await
            .is_err()
    );
    assert_eq!(original, std::fs::read(&path).unwrap());
}

/// Read-only errors must not create missing database files or sidecars.
#[tokio::test(flavor = "current_thread")]
async fn readonly_load_does_not_create_files() {
    let dir = tempfile::tempdir().unwrap();
    assert!(
        persistence::load(&dir.path().join("missing.db"))
            .await
            .is_err()
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
}
