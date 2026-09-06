//! Rust-owned, versioned snapshots. Every accepted mutation is one SQLite transaction.
use crate::{config::Config, world::*};
use anyhow::{Context, Result, ensure};
use rusqlite::{Connection, OpenFlags, types::ValueRef};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};
fn readonly(path: &Path) -> Result<Connection> {
    Ok(Connection::open_with_flags(
        path,
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?)
}
pub fn load(path: &Path) -> Result<World> {
    let c = readonly(path)?;
    ensure!(
        c.pragma_query_value(None, "user_version", |r| r.get::<_, i64>(0))? == 1,
        "unsupported Rust database version"
    );
    let json: String = c.query_row("SELECT document FROM world WHERE id=1", [], |r| r.get(0))?;
    let mut w: World = serde_json::from_str(&json)?;
    for o in w.objects.values_mut() {
        o.flags.remove("CONNECTED");
    }
    Ok(w)
}
pub fn save(path: &Path, w: &World) -> Result<()> {
    let mut c = Connection::open(path)?;
    c.busy_timeout(std::time::Duration::from_secs(5))?;
    let version: i64 = c.pragma_query_value(None, "user_version", |r| r.get(0))?;
    ensure!(
        version == 0 || version == 1,
        "unsupported Rust database version"
    );
    let tx = c.transaction()?;
    tx.execute_batch("CREATE TABLE IF NOT EXISTS world (id INTEGER PRIMARY KEY CHECK(id=1), document TEXT NOT NULL); PRAGMA user_version=1;")?;
    tx.execute(
        "INSERT INTO world VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET document=excluded.document",
        [serde_json::to_string(w)?],
    )?;
    tx.commit()?;
    Ok(())
}
pub async fn persist(path: PathBuf, w: World) -> Result<()> {
    tokio::task::spawn_blocking(move || save(&path, &w)).await?
}
pub fn initialize(path: &Path, w: &World) -> Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .context("destination already exists or cannot be created")?;
    drop(file);
    if let Err(e) = save(path, w) {
        let _ = std::fs::remove_file(path);
        return Err(e);
    }
    Ok(())
}
fn id(v: i64) -> Option<ObjectId> {
    (v >= 0).then_some(ObjectId(v))
}
pub fn read_legacy(source: &Path, cfg: &Config) -> Result<World> {
    let c = readonly(source)?;
    let (version, next, record): (i64, i64, usize) = c.query_row(
        "SELECT schema_version,db_top,record_players FROM snapshot WHERE id=1",
        [],
        |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
    )?;
    ensure!(
        version == 32,
        "unsupported legacy schema {version}; expected 32"
    );
    let mut w = World {
        next_id: next,
        record_players: record,
        initialized: true,
        ..Default::default()
    };
    let mut stmt = c.prepare("SELECT * FROM objects")?;
    let columns: Vec<String> = stmt.column_names().iter().map(|s| s.to_string()).collect();
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let object_id = ObjectId(r.get("dbref")?);
        let kind = Kind::from_code(r.get("type")?)?;
        let link = id(r.get("link")?);
        let mut flags = BTreeSet::new();
        let mut powers = BTreeSet::new();
        for col in &columns {
            if r.get::<_, i64>(col.as_str()).unwrap_or(0) == 1 {
                if let Some(s) = col
                    .strip_prefix("has_")
                    .and_then(|s| s.strip_suffix("_flag"))
                    && s != "connected"
                {
                    flags.insert(s.to_uppercase());
                }
                if let Some(s) = col
                    .strip_prefix("has_")
                    .and_then(|s| s.strip_suffix("_power"))
                {
                    powers.insert(s.to_uppercase());
                }
            }
        }
        w.objects.insert(
            object_id,
            Object {
                id: object_id,
                name: r.get("name")?,
                kind,
                location: id(r.get(if kind == Kind::Exit {
                    "exits"
                } else {
                    "location"
                })?),
                zone: id(r.get("zone")?),
                affiliation: id(r.get("affiliation")?),
                home: if kind != Kind::Exit { link } else { None },
                destination: if kind == Kind::Exit {
                    id(r.get("location")?)
                } else {
                    None
                },
                description: r.get("description")?,
                internal_description: r.get("internal_description")?,
                lua_parent: r.get("lua_parent")?,
                flags,
                powers,
                state: Default::default(),
            },
        );
    }
    let mut stmt = c.prepare("SELECT * FROM player_state")?;
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        w.accounts.insert(
            ObjectId(r.get("object_dbref")?),
            Account {
                hash: r.get("password_hash")?,
                alias: r.get("alias")?,
                last_login: r.get("last_login")?,
                last_site: r.get("last_site")?,
                successes: r.get("successful_login_count")?,
                failures: r.get("failed_login_count")?,
                unreported_failures: r.get("unreported_failed_login_count")?,
                history: Vec::new(),
            },
        );
    }
    let mut stmt =
        c.prepare("SELECT * FROM player_login_history ORDER BY player_dbref,outcome,position")?;
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        w.accounts
            .get_mut(&ObjectId(r.get("player_dbref")?))
            .context("login history account missing")?
            .history
            .push(Login {
                success: r.get::<_, i64>("outcome")? == 0,
                at: r.get("occurred_at")?,
                host: r.get("host")?,
            });
    }
    let mut stmt = c.prepare("SELECT * FROM object_state")?;
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let kind: i64 = r.get("value_type")?;
        let value = match (kind, r.get_ref("value")?) {
            (3, ValueRef::Integer(n)) => Scalar::Integer(n),
            (2, ValueRef::Integer(n)) => Scalar::Boolean(n != 0),
            (4, ValueRef::Real(n)) => Scalar::Number(n),
            (4, ValueRef::Integer(n)) => Scalar::Number(n as f64),
            (1, ValueRef::Text(s) | ValueRef::Blob(s)) => {
                Scalar::String(String::from_utf8(s.to_vec())?)
            }
            _ => anyhow::bail!("unsupported legacy state encoding {kind}"),
        };
        w.objects
            .get_mut(&ObjectId(r.get("object_dbref")?))
            .context("state object missing")?
            .state
            .entry(r.get("namespace")?)
            .or_default()
            .insert(r.get("key")?, value);
    }
    let mut stmt = c.prepare("SELECT * FROM comsys_channels")?;
    let mut rows = stmt.query([])?;
    while let Some(r) = rows.next()? {
        let name: String = r.get("name")?;
        w.channels.insert(
            name.clone(),
            Channel {
                name,
                object: id(r.get("chan_obj")?),
                flags: r.get("type")?,
                messages: r.get("num_messages")?,
            },
        );
    }
    w.next_id = w
        .next_id
        .max(w.objects.keys().next_back().map_or(0, |id| id.0 + 1));
    w.validate(cfg)?;
    Ok(w)
}
pub fn import(source: &Path, cfg: &Config) -> Result<String> {
    let w = read_legacy(source, cfg)?;
    initialize(&cfg.database(), &w)?;
    Ok(format!(
        "Imported schema 32: {} objects, {} accounts, {} channels. Object/account/login/Lua state imported; remaining tables retained in source archive {}. No legacy data was modified.",
        w.objects.len(),
        w.accounts.len(),
        w.channels.len(),
        source.display()
    ))
}
