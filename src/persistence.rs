//! Async SQLx SQLite snapshots; accepted mutations commit in one transaction.
use crate::{config::Config, world::*};
use anyhow::{Context, Result, ensure};
use sqlx::{Column, Connection, Row, SqliteConnection, sqlite::SqliteConnectOptions};
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
/// Load Rust storage using the centralized default timeout.
pub async fn load(path: &Path) -> Result<World> {
    load_with_timeout(
        path,
        crate::config::DatabaseConfig::default().busy_timeout_ms,
    )
    .await
}
/// Read and validate a snapshot without creating or modifying storage.
pub async fn load_with_timeout(path: &Path, busy_timeout_ms: u64) -> Result<World> {
    let mut c = connect(path, busy_timeout_ms, true, false).await?;
    let result=async {
        let legacy:i64=sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE type='table' AND name='snapshot'").fetch_one(&mut c).await?;
        ensure!(legacy==0,"database.game_database points to a legacy database; set the live Rust path and use import-legacy --source explicitly");
        let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&mut c).await?;
        ensure!(version==1,"unsupported Rust database version");
        let json:String=sqlx::query_scalar("SELECT document FROM world WHERE id=1").fetch_one(&mut c).await?;
    let value: serde_json::Value = serde_json::from_str(&json)?;
    if let Some(objects) = value.get("objects").and_then(|v| v.as_object()) {
        for (id, object) in objects {
            if let Some(powers) = object.get("powers") {
                serde_json::from_value::<crate::powers::PowerSet>(powers.clone())
                    .with_context(|| format!("object #{id}: invalid stored powers"))?;
            }
            if let Some(flags) = object.get("flags") {
                serde_json::from_value::<crate::flags::FlagSet>(flags.clone())
                    .with_context(|| format!("object #{id}: invalid stored flags"))?;
            }
        }
    }
    let mut w: World = serde_json::from_value(value)?;
    for o in w.objects.values_mut() {
        o.flags.remove(crate::flags::Flag::Connected);
    }
    Ok(w)
    }.await;
    finish(c, result).await
}
/// Save using the centralized timeout.
pub async fn save(path: &Path, w: &World) -> Result<()> {
    save_with_timeout(
        path,
        w,
        crate::config::DatabaseConfig::default().busy_timeout_ms,
    )
    .await
}
/// Persist durable fields atomically, excluding session-owned connection flags.
pub async fn save_with_timeout(path: &Path, w: &World, busy_timeout_ms: u64) -> Result<()> {
    let mut durable = w.clone();
    for o in durable.objects.values_mut() {
        o.flags.remove(crate::flags::Flag::Connected);
    }
    let document = serde_json::to_string(&durable)?;
    let mut c = connect(path, busy_timeout_ms, false, true).await?;
    let result=async {
        let version:i64=sqlx::query_scalar("PRAGMA user_version").fetch_one(&mut c).await?;
        ensure!(version==0 || version==1,"unsupported Rust database version");
        let mut tx=c.begin().await?;
        sqlx::raw_sql("CREATE TABLE IF NOT EXISTS world (id INTEGER PRIMARY KEY CHECK(id=1), document TEXT NOT NULL); PRAGMA user_version=1;").execute(&mut *tx).await?;
        sqlx::query("INSERT INTO world VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET document=excluded.document").bind(document).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(())
    }.await;
    finish(c, result).await
}
/// Persist an owned world snapshot without sending Lua values to a worker.
pub async fn persist(path: PathBuf, w: World, busy_timeout_ms: u64) -> Result<()> {
    save_with_timeout(&path, &w, busy_timeout_ms).await
}
/// Initialize storage using the centralized timeout.
pub async fn initialize(path: &Path, w: &World) -> Result<()> {
    initialize_with_timeout(
        path,
        w,
        crate::config::DatabaseConfig::default().busy_timeout_ms,
    )
    .await
}
/// Exclusively create restricted storage and remove it after failed initialization.
pub async fn initialize_with_timeout(path: &Path, w: &World, busy_timeout_ms: u64) -> Result<()> {
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
    if let Err(error) = save_with_timeout(path, w, busy_timeout_ms).await {
        let _ = tokio::fs::remove_file(path).await;
        return Err(error);
    }
    Ok(())
}
/// Convert the legacy negative-reference sentinel.
fn id(value: i64) -> Option<ObjectId> {
    (value >= 0).then_some(ObjectId(value))
}
/// Read schema 32 without modifying the source archive.
pub async fn read_legacy(source: &Path, cfg: &Config) -> Result<World> {
    let mut c = connect(source, cfg.database.busy_timeout_ms, true, false).await?;
    let result = read_legacy_connection(&mut c, cfg).await;
    finish(c, result).await
}
/// Decode legacy rows with checked SQLite storage classes for Lua scalar values.
async fn read_legacy_connection(c: &mut SqliteConnection, cfg: &Config) -> Result<World> {
    let (version, next, record): (i64, i64, i64) =
        sqlx::query_as("SELECT schema_version,db_top,record_players FROM snapshot WHERE id=1")
            .fetch_one(&mut *c)
            .await?;
    ensure!(
        version == 32,
        "unsupported legacy schema {version}; expected 32"
    );
    let mut w = World {
        next_id: next,
        record_players: usize::try_from(record).context("invalid record_players")?,
        initialized: true,
        ..Default::default()
    };
    for r in sqlx::query("SELECT * FROM objects")
        .fetch_all(&mut *c)
        .await?
    {
        let columns: Vec<&str> = r.columns().iter().map(|c| c.name()).collect();
        let object_id = ObjectId(r.try_get("dbref")?);
        let kind = Kind::from_code(r.try_get("type")?)?;
        let link = id(r.try_get("link")?);
        let mut flags = crate::flags::FlagSet::default();
        let mut powers = crate::powers::PowerSet::default();
        for col in &columns {
            if col.starts_with("has_")
                && (col.ends_with("_flag") || col.ends_with("_power"))
                && r.try_get::<i64, _>(*col)? == 1
            {
                if let Some(s) = col
                    .strip_prefix("has_")
                    .and_then(|s| s.strip_suffix("_flag"))
                    && s != "connected"
                {
                    flags.insert(
                        crate::flags::Flag::parse(s)
                            .with_context(|| format!("object #{}", object_id.0))?,
                    );
                }
                if let Some(s) = col
                    .strip_prefix("has_")
                    .and_then(|s| s.strip_suffix("_power"))
                {
                    powers.insert(
                        crate::powers::Power::parse(s)
                            .with_context(|| format!("object #{}", object_id.0))?,
                    );
                }
            }
        }
        w.objects.insert(
            object_id,
            Object {
                id: object_id,
                name: r.try_get("name")?,
                kind,
                location: id(r.try_get(if kind == Kind::Exit {
                    "exits"
                } else {
                    "location"
                })?),
                zone: id(r.try_get("zone")?),
                affiliation: id(r.try_get("affiliation")?),
                home: if kind != Kind::Exit { link } else { None },
                destination: if kind == Kind::Exit {
                    id(r.try_get("location")?)
                } else {
                    None
                },
                description: r.try_get("description")?,
                internal_description: r.try_get("internal_description")?,
                lua_parent: r.try_get("lua_parent")?,
                flags,
                powers,
                state: Default::default(),
            },
        );
    }
    for r in sqlx::query("SELECT * FROM player_state")
        .fetch_all(&mut *c)
        .await?
    {
        w.accounts.insert(
            ObjectId(r.try_get("object_dbref")?),
            Account {
                hash: r.try_get("password_hash")?,
                alias: r.try_get("alias")?,
                last_login: r.try_get("last_login")?,
                last_site: r.try_get("last_site")?,
                successes: r.try_get("successful_login_count")?,
                failures: r.try_get("failed_login_count")?,
                unreported_failures: r.try_get("unreported_failed_login_count")?,
                history: Vec::new(),
            },
        );
    }
    for r in
        sqlx::query("SELECT * FROM player_login_history ORDER BY player_dbref,outcome,position")
            .fetch_all(&mut *c)
            .await?
    {
        w.accounts
            .get_mut(&ObjectId(r.try_get("player_dbref")?))
            .context("login history account missing")?
            .history
            .push(Login {
                success: r.try_get::<i64, _>("outcome")? == 0,
                at: r.try_get("occurred_at")?,
                host: r.try_get("host")?,
            });
    }
    for r in sqlx::query("SELECT *, typeof(value) AS storage_type FROM object_state")
        .fetch_all(&mut *c)
        .await?
    {
        let kind: i64 = r.try_get("value_type")?;
        let storage: String = r.try_get("storage_type")?;
        let value = match (kind, storage.as_str()) {
            (3, "integer") => Scalar::Integer(r.try_get("value")?),
            (2, "integer") => Scalar::Boolean(r.try_get::<i64, _>("value")? != 0),
            (4, "real") => Scalar::Number(r.try_get("value")?),
            (4, "integer") => Scalar::Number(r.try_get::<i64, _>("value")? as f64),
            (1, "text") => Scalar::String(r.try_get("value")?),
            (1, "blob") => Scalar::String(String::from_utf8(r.try_get("value")?)?),
            _ => anyhow::bail!("unsupported legacy state encoding {kind} ({storage})"),
        };
        w.objects
            .get_mut(&ObjectId(r.try_get("object_dbref")?))
            .context("state object missing")?
            .state
            .entry(r.try_get("namespace")?)
            .or_default()
            .insert(r.try_get("key")?, value);
    }
    for r in sqlx::query("SELECT * FROM comsys_channels")
        .fetch_all(&mut *c)
        .await?
    {
        let name: String = r.try_get("name")?;
        w.channels.insert(
            name.clone(),
            Channel {
                name,
                object: id(r.try_get("chan_obj")?),
                flags: r.try_get("type")?,
                messages: r.try_get("num_messages")?,
            },
        );
    }
    w.next_id = w
        .next_id
        .max(w.objects.keys().next_back().map_or(0, |id| id.0 + 1));
    w.validate(cfg)?;
    Ok(w)
}
/// Import explicitly, refusing to overwrite initialized destinations.
pub async fn import(source: &Path, cfg: &Config) -> Result<String> {
    let w = read_legacy(source, cfg).await?;
    initialize_with_timeout(&cfg.database(), &w, cfg.database.busy_timeout_ms).await?;
    Ok(format!(
        "Imported schema 32: {} objects, {} accounts, {} channels. Object/account/login/Lua state imported; remaining tables retained in source archive {}. No legacy data was modified.",
        w.objects.len(),
        w.accounts.len(),
        w.channels.len(),
        source.display()
    ))
}
