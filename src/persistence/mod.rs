//! Direct asynchronous schema-32 persistence with selective, atomic updates.
mod btech;
mod btech_autopilot;
mod btech_character;
mod btech_decorations;
mod btech_dice;
mod btech_map_lifecycle;
mod btech_map_random;
mod btech_reactor;
mod btech_recovery;
mod btech_terrain;
mod btech_unit_configuration;
mod btech_unit_rows;
mod btech_units;
mod btech_vehicles;
mod communication;
mod load;
mod macros;
mod maintenance;
mod write;
use crate::{accounts::Login, world::*};
use anyhow::{Context, Result, ensure};
use sqlx::{
    Connection, SqliteConnection,
    sqlite::{SqliteConnectOptions, SqliteSynchronous},
};
use std::path::{Path, PathBuf};
/// Open an operation-scoped connection without altering journal or foreign-key policy.
///
/// Every connection uses full sync, so a commit is on disk once it returns, even across
/// a power loss. Writers switch a validated database to write-ahead-log mode with
/// [`use_write_ahead_log`].
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
            .synchronous(SqliteSynchronous::Full)
            .busy_timeout(std::time::Duration::from_millis(timeout)),
    )
    .await?)
}
/// Put a validated database in write-ahead-log mode, which SQLite keeps in the file.
///
/// A commit then costs one sync of the log instead of the rollback journal's several.
/// This must run outside a transaction, and only after validation, so that an empty or
/// foreign file is never rewritten.
async fn use_write_ahead_log(c: &mut SqliteConnection) -> Result<()> {
    let mode: String = sqlx::query_scalar("PRAGMA journal_mode=WAL")
        .fetch_one(&mut *c)
        .await?;
    ensure!(
        mode.eq_ignore_ascii_case("wal"),
        "SQLite kept journal mode {mode} instead of write-ahead logging"
    );
    Ok(())
}
/// An idle connection that keeps the database open between saves.
///
/// Each save opens and closes its own connection. In write-ahead-log mode SQLite copies
/// the log back into the database whenever the last connection closes, which would cost
/// every save a second round of syncs. While this connection is held, that copy happens
/// only at SQLite's normal checkpoint interval.
pub struct DatabaseAnchor {
    connection: SqliteConnection,
}

impl DatabaseAnchor {
    /// Open the anchor for a database that a save has already validated. Reading the
    /// schema joins the write-ahead log, which is what keeps the log open.
    pub async fn open(path: &Path, timeout: u64) -> Result<Self> {
        let mut connection = connect(path, timeout, false, false).await?;
        validate(&mut connection).await?;
        Ok(Self { connection })
    }

    /// Close the anchor. As the last connection, it folds the log back into the
    /// database file, so a stopped server leaves no log behind.
    pub async fn close(self) -> Result<()> {
        Ok(self.connection.close().await?)
    }
}
/// Drain the worker before returning, preserving the original operation error.
async fn finish<T>(connection: SqliteConnection, result: Result<T>) -> Result<T> {
    let closed = connection.close().await;
    match result {
        Err(error) => Err(error),
        Ok(value) => {
            if let Err(error) = closed {
                crate::logging::fatal(&format!(
                    "SQLite close failed after completed operation: {error}"
                ));
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
    save_changes(path, None, world, timeout).await.map(|_| ())
}
/// Write `world` as a diff against `baseline`, the world this database last stored.
///
/// A baseline spares reading the whole database back before diffing, and lets the
/// diff skip every entry the two worlds still share. Pass `None` unless the database
/// is known to hold exactly `baseline`; the stored world is then read instead.
/// Returns whether any row changed.
pub(crate) async fn save_changes(
    path: &Path,
    baseline: Option<&World>,
    world: &World,
    timeout: u64,
) -> Result<bool> {
    let mut c = connect(path, timeout, false, false).await?;
    let result = async {
        validate(&mut c).await?;
        use_write_ahead_log(&mut c).await?;
        let mut tx = c.begin_with("BEGIN IMMEDIATE").await?;
        let stored;
        let before = match baseline {
            Some(baseline) => baseline,
            None => {
                stored = load::read(&mut tx).await?;
                &stored
            }
        };
        let changed = write::apply(&mut tx, before, world).await?;
        tx.commit().await?;
        world.macros.committed();
        // Unregister sanctions served their purpose once the teardown is durable.
        world.btech.retire_sanctions.borrow_mut().clear();
        Ok(changed)
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
            use_write_ahead_log(&mut c).await?;
            let mut tx = c.begin_with("BEGIN IMMEDIATE").await?;
            sqlx::raw_sql(include_str!("schema32.sql"))
                .execute(&mut *tx)
                .await?;
            sqlx::raw_sql(include_str!("btech_units.sql"))
                .execute(&mut *tx)
                .await?;
            sqlx::raw_sql(include_str!("btech_terrain.sql"))
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO snapshot VALUES(1,32,1,32,0,0,0,0,0)")
                .execute(&mut *tx)
                .await?;
            write::apply(&mut tx, &World::default(), world).await?;
            tx.commit().await?;
            world.macros.committed();
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

/// Check, plan, run world callbacks and persist maintenance under one SQLite transaction.
/// The caller owns restoring its in-memory snapshot if any stage fails.
pub async fn repair<F>(path: &Path, timeout: u64, apply: F) -> Result<crate::dbck::DbCheckReport>
where
    F: FnOnce(&crate::world::Links) -> Result<(World, crate::dbck::DbCheckReport)>,
{
    let mut c = connect(path, timeout, false, false).await?;
    let result = async {
        validate(&mut c).await?;
        use_write_ahead_log(&mut c).await?;
        let mut tx = c.begin_with("BEGIN IMMEDIATE").await?;
        let integrity: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_all(&mut *tx)
            .await?;
        ensure!(
            integrity == ["ok"],
            "SQLite integrity check failed: {}",
            integrity.join("; ")
        );
        let before = load::read(&mut tx).await?;
        let raw = maintenance::links(&mut tx).await?;
        let (after, report) = apply(&raw)?;
        let changes_before: i64 = sqlx::query_scalar("SELECT total_changes()")
            .fetch_one(&mut *tx)
            .await?;
        write::apply_changes(&mut tx, &before, &after, Some(&report.plan)).await?;
        maintenance::cleanup(&mut tx, &report.plan.purges).await?;
        ensure!(
            sqlx::query("PRAGMA foreign_key_check")
                .fetch_all(&mut *tx)
                .await?
                .is_empty(),
            "database repair blocked by an unresolved foreign-key dependency"
        );
        let changes_after: i64 = sqlx::query_scalar("SELECT total_changes()")
            .fetch_one(&mut *tx)
            .await?;
        if changes_after > changes_before {
            sqlx::query("UPDATE snapshot SET dump_time=? WHERE id=1")
                .bind(crate::clock::wall_time())
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        after.macros.committed();
        Ok(report)
    }
    .await;
    finish(c, result).await
}

/// Startup requires consistent legacy lists; repairing them requires explicit @dbck in an existing session.
pub async fn validate_lists(path: &Path, world: &World, timeout: u64) -> Result<()> {
    let mut c = connect(path, timeout, true, false).await?;
    let result = async {
        let raw = maintenance::links(&mut c).await?;
        let expected = crate::dbck::rebuild_links(world, &raw);
        for (id, links) in &expected {
            ensure!(
                raw.get(id) == Some(links),
                "inconsistent persisted containment list at #{}; startup requires a valid world",
                id.0
            );
        }
        Ok(())
    }
    .await;
    finish(c, result).await
}

/// Read one object's durable list slots without acquiring a write connection.
pub async fn inspect_links(
    path: &Path,
    object: ObjectId,
    timeout: u64,
) -> Result<crate::world::LinkSlots> {
    use sqlx::Row;
    let mut c = connect(path, timeout, true, false).await?;
    let result = async {
        let row = sqlx::query("SELECT contents,exits,next FROM objects WHERE dbref=?")
            .bind(object.0)
            .fetch_one(&mut c)
            .await
            .with_context(|| format!("reading persisted bookkeeping for #{}", object.0))?;
        Ok(crate::world::LinkSlots {
            contents: row.try_get("contents")?,
            exits: row.try_get("exits")?,
            next: row.try_get("next")?,
        })
    }
    .await;
    finish(c, result).await
}

/// Persist a callback's approved maintenance effects with all ordinary mutations.
///
/// Without maintenance, `baseline` is passed on to [`save_changes`]; maintenance always
/// reads the stored world. Returns whether any row may have changed.
pub(crate) async fn persist_effects(
    path: PathBuf,
    world: World,
    timeout: u64,
    report: Option<crate::dbck::DbCheckReport>,
    baseline: Option<&World>,
) -> Result<bool> {
    let Some(mut report) = report else {
        return save_changes(&path, baseline, &world, timeout).await;
    };
    for id in &report.plan.purges {
        let o = world
            .objects
            .get(id)
            .context("callback removed a tombstone")?;
        ensure!(
            o.kind == crate::world::Kind::Garbage
                && o.flags == [crate::flags::Flag::Going].into_iter().collect()
                && o.powers == Default::default()
                && o.state.is_empty()
                && !world.accounts.contains_key(id),
            "callback changed purged object #{}",
            id.0
        );
    }
    repair(&path, timeout, |raw| {
        report.plan.links = crate::dbck::rebuild_links(&world, &report.plan.links);
        report.plan.list_changes = report
            .plan
            .links
            .iter()
            .filter(|(id, links)| raw.get(id) != Some(*links))
            .map(|(id, _)| *id)
            .collect();
        Ok((world, report))
    })
    .await
    .map(|_| true)
}

mod btech_values;

mod btech_entrances;

mod btech_artillery;
mod btech_building_repair;

mod btech_minefields;

mod btech_landing_exclusions;
mod btech_map_bits;
mod btech_object_order;

mod btech_player_configuration;
mod btech_view_preferences;

mod btech_wrapping;

mod btech_building_routes;

mod btech_tows;

mod btech_wrecks;

mod btech_inventory;
mod btech_part_costs;

mod btech_cargo_bay;

mod btech_static_decorations;

mod btech_map_links;

mod btech_gunner_stations;

mod btech_turn_clock;

mod btech_sensor_recovery;
