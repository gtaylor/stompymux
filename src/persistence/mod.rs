//! Schema-32 SQLite persistence: one relational database, written as a diff against the
//! world it last stored.
//!
//! Reads use short-lived read-only connections. Writes go through a [`Database`], which
//! the server keeps open for its whole run: the schema is validated and the write-ahead
//! log enabled once, prepared statements stay cached, and SQLite folds the log back into
//! the database at its normal checkpoint interval instead of after every save.
mod btech;
mod btech_artillery;
mod btech_autopilot;
mod btech_building_repair;
mod btech_building_routes;
mod btech_cargo_bay;
mod btech_character;
mod btech_deadlines;
mod btech_decorations;
mod btech_dice;
mod btech_entrances;
mod btech_inventory;
mod btech_landing_exclusions;
mod btech_map_lifecycle;
mod btech_map_links;
mod btech_map_random;
mod btech_minefields;
mod btech_object_order;
mod btech_part_costs;
mod btech_player_configuration;
mod btech_points_of_interest;
mod btech_reactor;
mod btech_recovery;
mod btech_regions;
mod btech_static_decorations;
mod btech_terrain;
mod btech_tows;
mod btech_turn_clock;
mod btech_unit_configuration;
pub(crate) mod btech_unit_rows;
mod btech_units;
mod btech_values;
mod btech_vehicles;
mod btech_view_preferences;
mod btech_wrecks;
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
use std::path::Path;
pub use write::Saved;

/// The Rust-owned tables, created by [`initialize`] and required by every later open.
const TABLES: &[&str] = &[
    "btech_simulation_clock",
    "btech_units",
    "btech_unit_timers",
    "btech_vehicles",
    "btech_vehicle_timers",
    "btech_map_terrain",
    "btech_map_terrain_codes",
    "btech_mine_order",
    "btech_landing_order",
    "btech_map_decorations",
    "btech_map_points_of_interest",
    "btech_map_regions",
    "btech_map_region_corners",
    "btech_map_random",
    "btech_building_repair",
    "btech_artillery",
    "btech_tows",
    "btech_wrecks",
    "btech_character_recovery",
    "btech_reactor_clock",
    "btech_autopilot_controllers",
    "btech_autopilot_controller_orders",
    "btech_autopilot_controller_waypoints",
    "btech_autopilot_controller_feedback",
];

/// Open a connection without altering journal or foreign-key policy.
///
/// Every connection uses full sync, so a commit is on disk once it returns, even across
/// a power loss.
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

/// Close a connection, preserving the operation's own error over a close error.
async fn finish<T>(connection: SqliteConnection, result: Result<T>) -> Result<T> {
    let closed = connection.close().await;
    match result {
        Err(error) => Err(error),
        Ok(value) => {
            if let Err(error) = closed {
                tracing::error!(error = %error, "SQLite close failed after completed operation");
            }
            Ok(value)
        }
    }
}

/// Validate the storage contract without altering tables, pragmas or metadata.
async fn validate(c: &mut SqliteConnection) -> Result<()> {
    let tables: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type='table'")
            .fetch_all(&mut *c)
            .await?;
    ensure!(
        tables.iter().any(|name| name == "snapshot"),
        "expected schema-32 relational storage; database.game_database must point to stompymux.db, not a Rust JSON snapshot or empty database; no automatic conversion is performed"
    );
    let version: i64 = sqlx::query_scalar("SELECT schema_version FROM snapshot WHERE id=1")
        .fetch_one(&mut *c)
        .await?;
    ensure!(
        version == 32,
        "unsupported database schema {version}; expected 32"
    );
    let missing: Vec<&str> = TABLES
        .iter()
        .copied()
        .filter(|table| !tables.iter().any(|name| name == table))
        .collect();
    ensure!(
        missing.is_empty(),
        "database lacks the tables {}; create a fresh database instead of reusing one from an older build",
        missing.join(", ")
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

/// The open write connection to a validated database.
///
/// The server keeps one for its whole run and drops it after a failed write, so the next
/// save opens a fresh one. Closing it as the last connection folds the write-ahead log
/// back into the database file, so a stopped server leaves no log behind.
pub struct Database {
    connection: SqliteConnection,
}

impl Database {
    /// Open, validate and switch to write-ahead logging. Missing destinations are never
    /// created here.
    ///
    /// CONNECTED is session state. Zero is stored for it on every save, and any stale
    /// value an unclean stop left behind is cleared here so the stored rows match the
    /// world the server saves against.
    pub async fn open(path: &Path, timeout: u64) -> Result<Self> {
        let mut connection = connect(path, timeout, false, false).await?;
        let result = async {
            validate(&mut connection).await?;
            use_write_ahead_log(&mut connection).await?;
            sqlx::query("UPDATE objects SET has_connected_flag=0 WHERE has_connected_flag<>0")
                .execute(&mut connection)
                .await?;
            Ok(())
        }
        .await;
        if let Err(error) = result {
            let _ = connection.close().await;
            return Err(error);
        }
        Ok(Self { connection })
    }

    /// Close the connection.
    pub async fn close(self) -> Result<()> {
        Ok(self.connection.close().await?)
    }

    /// Store every change of `world` against the stored world, including the clock.
    pub async fn save(&mut self, world: &World) -> Result<Saved> {
        self.save_changes(None, world, 1).await
    }

    /// Write `world` as a diff against `baseline`, the world this database last stored.
    ///
    /// The simulation clock is stored alongside any other change, or alone once it is
    /// `clock_interval` seconds ahead of the stored clock; zero never stores it alone.
    /// Explicit saves and shutdown pass one, so they always store the current clock.
    ///
    /// A baseline spares reading the whole database back before diffing, and lets the
    /// diff skip every entry the two worlds still share. Pass `None` unless the database
    /// is known to hold exactly `baseline`; the stored world is then read instead.
    pub async fn save_changes(
        &mut self,
        baseline: Option<&World>,
        world: &World,
        clock_interval: u64,
    ) -> Result<Saved> {
        let mut tx = self.connection.begin_with("BEGIN IMMEDIATE").await?;
        let stored;
        let before = match baseline {
            Some(baseline) => baseline,
            None => {
                stored = load::read(&mut tx).await?;
                &stored
            }
        };
        let saved = write::apply(&mut tx, before, world, clock_interval).await?;
        tx.commit().await?;
        world.macros.committed();
        // Unregister sanctions served their purpose once the teardown is durable.
        world.btech.retire_sanctions.borrow_mut().clear();
        Ok(saved)
    }

    /// Persist a callback's approved maintenance effects with all ordinary mutations.
    ///
    /// Without maintenance, `baseline` is passed on to [`Self::save_changes`];
    /// maintenance always reads the stored world.
    pub async fn persist_effects(
        &mut self,
        world: &World,
        report: Option<crate::dbck::DbCheckReport>,
        baseline: Option<&World>,
        clock_interval: u64,
    ) -> Result<Saved> {
        let Some(mut report) = report else {
            return self.save_changes(baseline, world, clock_interval).await;
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
        let world = world.clone();
        let links = self
            .repair(|raw| {
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
            .await?
            .1;
        Ok(Saved {
            changed: true,
            links,
        })
    }

    /// Check, plan, run world callbacks and persist maintenance under one transaction.
    /// The caller owns restoring its in-memory snapshot if any stage fails. Returns the
    /// report and the containment links the database holds afterwards.
    pub async fn repair<F>(
        &mut self,
        apply: F,
    ) -> Result<(crate::dbck::DbCheckReport, std::sync::Arc<Links>)>
    where
        F: FnOnce(&Links) -> Result<(World, crate::dbck::DbCheckReport)>,
    {
        let mut tx = self.connection.begin_with("BEGIN IMMEDIATE").await?;
        let integrity: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_all(&mut *tx)
            .await?;
        ensure!(
            integrity == ["ok"],
            "SQLite integrity check failed: {}",
            integrity.join("; ")
        );
        let before = load::read(&mut tx).await?;
        let (after, report) = apply(&before.links)?;
        let changes_before: i64 = sqlx::query_scalar("SELECT total_changes()")
            .fetch_one(&mut *tx)
            .await?;
        let saved = write::apply_changes(&mut tx, &before, &after, Some(&report.plan), 1).await?;
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
        Ok((report, saved.links))
    }
}

/// Store every change of `world` through a connection opened for this save alone.
pub async fn save(path: &Path, world: &World) -> Result<()> {
    save_with_timeout(
        path,
        world,
        crate::config::DatabaseConfig::default().busy_timeout_ms,
    )
    .await
}

/// [`save`] with an explicit busy timeout.
pub async fn save_with_timeout(path: &Path, world: &World, timeout: u64) -> Result<()> {
    let mut database = Database::open(path, timeout).await?;
    let result = database.save(world).await;
    finish(database.connection, result).await.map(|_| ())
}

/// [`Database::persist_effects`] through a connection opened for this save alone, for
/// writers that run before or outside the server's own connection.
pub async fn persist_effects(
    path: &Path,
    world: &World,
    timeout: u64,
    report: Option<crate::dbck::DbCheckReport>,
    clock_interval: u64,
) -> Result<Saved> {
    let mut database = Database::open(path, timeout).await?;
    let result = database
        .persist_effects(world, report, None, clock_interval)
        .await;
    finish(database.connection, result).await
}

/// [`Database::repair`] through a connection opened for this repair alone.
pub async fn repair<F>(path: &Path, timeout: u64, apply: F) -> Result<crate::dbck::DbCheckReport>
where
    F: FnOnce(&Links) -> Result<(World, crate::dbck::DbCheckReport)>,
{
    let mut database = Database::open(path, timeout).await?;
    let result = database.repair(apply).await;
    finish(database.connection, result)
        .await
        .map(|(report, _)| report)
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
            sqlx::raw_sql(include_str!("btech_schema.sql"))
                .execute(&mut *tx)
                .await?;
            sqlx::query("INSERT INTO snapshot VALUES(1,32,1,32,0,0,0,0,0)")
                .execute(&mut *tx)
                .await?;
            write::apply(&mut tx, &World::default(), world, 1).await?;
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Table definitions of a database, by name, with whitespace normalized.
    async fn tables(c: &mut SqliteConnection) -> std::collections::BTreeMap<String, String> {
        sqlx::query_as::<_, (String, String)>(
            "SELECT name,sql FROM sqlite_master WHERE type='table' ORDER BY name",
        )
        .fetch_all(c)
        .await
        .unwrap()
        .into_iter()
        .map(|(name, sql)| (name, sql.split_whitespace().collect::<Vec<_>>().join(" ")))
        .collect()
    }

    /// The test fixture database holds exactly the tables the schema files create, so a
    /// schema change that is not carried into the fixture fails here rather than in
    /// whichever integration test first touches the difference.
    #[tokio::test]
    async fn fixture_database_matches_the_schema_files() {
        let mut fresh = SqliteConnection::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql(include_str!("schema32.sql"))
            .execute(&mut fresh)
            .await
            .unwrap();
        sqlx::raw_sql(include_str!("btech_schema.sql"))
            .execute(&mut fresh)
            .await
            .unwrap();
        let expected = tables(&mut fresh).await;
        let fixture = Path::new(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/fixtures/game/data/stompymux.db"
        ));
        let mut stored = connect(fixture, 1000, true, false).await.unwrap();
        let actual = tables(&mut stored).await;
        stored.close().await.unwrap();
        assert_eq!(
            actual, expected,
            "tests/fixtures/game/data/stompymux.db differs from the schema files"
        );
    }

    /// The required table list names exactly the tables the schema file creates.
    #[test]
    fn required_tables_match_the_schema_file() {
        let created: Vec<&str> = include_str!("btech_schema.sql")
            .lines()
            .filter_map(|line| line.strip_prefix("CREATE TABLE "))
            .map(|rest| rest.split_whitespace().next().unwrap())
            .collect();
        assert_eq!(created, TABLES);
    }
}
