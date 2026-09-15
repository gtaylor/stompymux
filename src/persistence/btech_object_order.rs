//! Shared persistence for map-object traversal without changing stable record identities.
use super::write::{Cell, fields, row};
use crate::{ObjectId, StoredBattleMap, World};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

/// Only owned table identifiers can enter the shared SQL statements.
#[derive(Clone, Copy)]
pub(super) enum Kind {
    Mine,
    Landing,
}

impl Kind {
    /// Storage is independent of object payload columns and their extension metadata.
    fn table(self) -> &'static str {
        match self {
            Self::Mine => "btech_mine_order",
            Self::Landing => "btech_landing_order",
        }
    }

    /// Inspect the committed traversal for a single object kind.
    fn order(self, map: &StoredBattleMap) -> &Arc<Vec<u32>> {
        match self {
            Self::Mine => &map.minefield_order,
            Self::Landing => &map.landing_exclusion_order,
        }
    }

    /// Install loaded order before the map's membership validator runs.
    fn order_mut(self, map: &mut StoredBattleMap) -> &mut Arc<Vec<u32>> {
        match self {
            Self::Mine => &mut map.minefield_order,
            Self::Landing => &mut map.landing_exclusion_order,
        }
    }
}

/// Inspection of shared reference databases does not create Rust-owned tables.
async fn exists(c: &mut SqliteConnection, kind: Kind) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?",
    )
    .bind(kind.table())
    .fetch_one(c)
    .await?
        != 0)
}

/// Remove owned rows before dependency checks, tolerating absent optional tables.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    for kind in [Kind::Mine, Kind::Landing] {
        if !exists(c, kind).await? {
            continue;
        }
        let query = format!("DELETE FROM {} WHERE map_dbref=?", kind.table());
        for id in ids {
            sqlx::query(sqlx::AssertSqlSafe(query.as_str()))
                .bind(id.0)
                .execute(&mut *c)
                .await?;
        }
    }
    Ok(())
}

/// Override imported ordinal traversal only when explicit owned order is stored.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredBattleMap>,
    kind: Kind,
) -> Result<()> {
    use futures_util::TryStreamExt;
    if !exists(c, kind).await? {
        return Ok(());
    }
    let query = format!(
        "SELECT map_dbref,ordinals_json FROM {} ORDER BY map_dbref",
        kind.table()
    );
    let mut rows = sqlx::query(sqlx::AssertSqlSafe(query.as_str())).fetch(c);
    while let Some(row) = rows.try_next().await? {
        let map = maps
            .get_mut(&ObjectId(row.try_get("map_dbref")?))
            .context("Map object order references missing map")?;
        let encoded: String = row.try_get("ordinals_json")?;
        ensure!(encoded.len() <= 12_000_002, "Map object order is too large");
        *kind.order_mut(map) = Arc::new(serde_json::from_str(&encoded)?);
    }
    Ok(())
}

/// Save changed traversals alongside their objects in the enclosing transaction.
pub(super) async fn save(
    c: &mut SqliteConnection,
    before: &World,
    after: &World,
    kind: Kind,
) -> Result<bool> {
    let mut changed = false;
    let table = kind.table();
    for (&id, map) in after.btech.maps() {
        let old = before.btech.maps().get(&id);
        let current = kind.order(map);
        if old.is_some_and(|old| kind.order(old) == current)
            || (old.is_none() && current.is_empty())
        {
            continue;
        }
        if !exists(c, kind).await? {
            sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TABLE {table} (map_dbref INTEGER PRIMARY KEY REFERENCES objects(dbref) ON DELETE CASCADE, ordinals_json TEXT NOT NULL)")))
                .execute(&mut *c).await?;
        }
        if current.is_empty() {
            sqlx::query(sqlx::AssertSqlSafe(format!(
                "DELETE FROM {table} WHERE map_dbref=?"
            )))
            .bind(id.0)
            .execute(&mut *c)
            .await?;
            changed = true;
            continue;
        }
        let previous = sqlx::query_scalar::<_, String>(sqlx::AssertSqlSafe(format!(
            "SELECT ordinals_json FROM {table} WHERE map_dbref=?"
        )))
        .bind(id.0)
        .fetch_optional(&mut *c)
        .await?;
        row(
            c,
            table,
            fields([("map_dbref", Cell::Integer(id.0))]),
            previous
                .map(|value| fields([("ordinals_json", Cell::Text(value))]))
                .as_ref(),
            &fields([("ordinals_json", Cell::Text(serde_json::to_string(current)?))]),
        )
        .await?;
        changed = true;
    }
    Ok(changed)
}
