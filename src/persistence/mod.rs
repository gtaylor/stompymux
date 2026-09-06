//! Direct asynchronous schema-32 persistence with selective, atomic updates.
mod load;
mod write;
use crate::world::*;
use anyhow::{Context, Result, ensure};
use sqlx::{Connection, SqliteConnection, sqlite::SqliteConnectOptions};
use std::path::{Path, PathBuf};
/// Open an operation-scoped connection without altering journal or foreign-key policy.
async fn connect(
    path: &Path,
    timeout: u64,
    readonly: bool,
    create: bool,
) -> Result<SqliteConnection> {
    Ok(SqliteConnection::connect_with(
        &SqliteConnectOptions::new()
            .filename(path)
            .read_only(readonly)
            .create_if_missing(create)
            .foreign_keys(false)
            .busy_timeout(std::time::Duration::from_millis(timeout)),
    )
    .await?)
}
/// Drain the worker before returning, preserving the original operation error.
async fn finish<T>(connection: SqliteConnection, result: Result<T>) -> Result<T> {
    let closed = connection.close().await;
    match result {
        Err(error) => Err(error),
        Ok(value) => {
            if let Err(error) = closed {
                eprintln!("SQLite close failed after completed operation: {error}");
            }
            Ok(value)
        }
    }
}

/// Validate the storage contract without altering tables, pragmas or metadata.
async fn validate(c: &mut SqliteConnection) -> Result<()> {
    let snapshot: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='snapshot'",
    )
    .fetch_one(&mut *c)
    .await?;
    ensure!(
        snapshot == 1,
        "expected schema-32 relational storage; database.game_database must point to stompymux.db, not a Rust JSON snapshot or empty database; no automatic conversion is performed"
    );
    let version: i64 = sqlx::query_scalar("SELECT schema_version FROM snapshot WHERE id=1")
        .fetch_one(&mut *c)
        .await?;
    ensure!(
        version == 32,
        "unsupported database schema {version}; expected 32"
    );
    Ok(())
}
/// Load the live relational database using the centralized timeout.
pub async fn load(path: &Path) -> Result<World> {
    load_with_timeout(
        path,
        crate::config::DatabaseConfig::default().busy_timeout_ms,
    )
    .await
}
/// Load one consistent read-only snapshot without creating missing storage.
pub async fn load_with_timeout(path: &Path, timeout: u64) -> Result<World> {
    let mut c = connect(path, timeout, true, false).await?;
    let result = async {
        let mut tx = c.begin().await?;
        validate(&mut tx).await?;
        let world = load::read(&mut tx).await?;
        tx.commit().await?;
        Ok(world)
    }
    .await;
    finish(c, result).await
}
/// Persist only supported changes against the durable relational baseline.
pub async fn save(path: &Path, world: &World) -> Result<()> {
    save_with_timeout(
        path,
        world,
        crate::config::DatabaseConfig::default().busy_timeout_ms,
    )
    .await
}
/// Compare and write within one transaction; missing destinations are never created here.
pub async fn save_with_timeout(path: &Path, world: &World, timeout: u64) -> Result<()> {
    let mut c = connect(path, timeout, false, false).await?;
    let result = async {
        let mut tx = c.begin_with("BEGIN IMMEDIATE").await?;
        validate(&mut tx).await?;
        let before = load::read(&mut tx).await?;
        write::apply(&mut tx, &before, world).await?;
        tx.commit().await?;
        Ok(())
    }
    .await;
    finish(c, result).await
}
/// Persist an owned world after releasing world-thread RefCell borrows.
pub async fn persist(path: PathBuf, world: World, timeout: u64) -> Result<()> {
    save_with_timeout(&path, &world, timeout).await
}
/// Exclusively create a fresh schema-32 destination with centralized timeout.
pub async fn initialize(path: &Path, world: &World) -> Result<()> {
    initialize_with_timeout(
        path,
        world,
        crate::config::DatabaseConfig::default().busy_timeout_ms,
    )
    .await
}
/// Create schema and bootstrap state atomically; close SQLite before failed-file cleanup.
pub async fn initialize_with_timeout(path: &Path, world: &World, timeout: u64) -> Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    let file = tokio::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .await
        .context("destination already exists or cannot be created")?;
    drop(file);
    let result = async {
        let mut c = connect(path, timeout, false, false).await?;
        let result = async {
            let mut tx = c.begin_with("BEGIN IMMEDIATE").await?;
            sqlx::raw_sql(include_str!("schema32.sql"))
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO snapshot VALUES(1,32,1,32,0,0,0,0,0)")
                .execute(&mut *tx)
                .await?;
            write::apply(&mut tx, &World::default(), world).await?;
            tx.commit().await?;
            Ok(())
        }
        .await;
        finish(c, result).await
    }
    .await;
    if result.is_err() {
        let _ = tokio::fs::remove_file(path).await;
    }
    result
}
/// Keep the newest history entries within schema capacity and the configured total limit.
pub fn trim_history(history: &mut Vec<Login>, limit: usize) {
    let (mut successes, mut failures, mut total) = (0, 0, 0);
    history.reverse();
    history.retain(|entry| {
        let (count, capacity) = if entry.success {
            (&mut successes, 4)
        } else {
            (&mut failures, 3)
        };
        if *count >= capacity || total >= limit {
            return false;
        }
        *count += 1;
        total += 1;
        true
    });
    history.reverse();
}
