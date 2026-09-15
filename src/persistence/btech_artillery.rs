//! Durable map-owned artillery queues, saved with impact effects and map randomness.
use super::write::{Cell, fields, row};
use crate::{BattleArtilleryShot, ObjectId, StoredBattleMap, World};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

/// Detect saved queues without modifying databases on read.
async fn installed(c: &mut SqliteConnection) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_artillery'",
    )
    .fetch_one(c)
    .await?
        == 1)
}

/// Decode bounded queues and reject orphan records.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredBattleMap>,
) -> Result<()> {
    if !installed(c).await? {
        return Ok(());
    }
    use futures_util::TryStreamExt;
    let mut rows = sqlx::query("SELECT map_dbref,length(CAST(shots AS BLOB)) AS bytes,CASE WHEN length(CAST(shots AS BLOB))<=1048576 THEN shots ELSE NULL END AS shots FROM btech_artillery").fetch(c);
    while let Some(row) = rows.try_next().await? {
        let map = maps
            .get_mut(&ObjectId(row.try_get("map_dbref")?))
            .context("Artillery queue references missing map")?;
        ensure!(
            row.try_get::<i64, _>("bytes")? <= 1048576,
            "Artillery queue exceeds size limit"
        );
        let encoded: String = row.try_get("shots")?;
        map.artillery_shots =
            serde_json::from_str::<BTreeMap<u32, BattleArtilleryShot>>(&encoded)?.into();
    }
    Ok(())
}

/// Save queue changes in the same transaction as every arrival consequence.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = before.btech.maps().get(&id).map(|map| &map.artillery_shots);
        if old.is_some_and(|old| old == &map.artillery_shots)
            || (old.is_none() && map.artillery_shots.is_empty())
        {
            continue;
        }
        if !installed(c).await? {
            sqlx::raw_sql(include_str!("btech_artillery.sql"))
                .execute(&mut *c)
                .await?;
        }
        if map.artillery_shots.is_empty() {
            sqlx::query("DELETE FROM btech_artillery WHERE map_dbref=?")
                .bind(id.0)
                .execute(&mut *c)
                .await?;
            changed = true;
            continue;
        }
        let encoded = serde_json::to_string(&map.artillery_shots)?;
        ensure!(
            encoded.len() <= 1048576,
            "Artillery queue exceeds size limit"
        );
        let previous: Option<String> =
            sqlx::query_scalar("SELECT shots FROM btech_artillery WHERE map_dbref=?")
                .bind(id.0)
                .fetch_optional(&mut *c)
                .await?;
        row(
            c,
            "btech_artillery",
            fields([("map_dbref", Cell::Integer(id.0))]),
            previous
                .map(|shots| fields([("shots", Cell::Text(shots))]))
                .as_ref(),
            &fields([("shots", Cell::Text(encoded))]),
        )
        .await?;
        changed = true;
    }
    Ok(changed)
}

/// Remove queued shots before map identities are purged.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    if !installed(c).await? {
        return Ok(());
    }
    for id in ids {
        sqlx::query("DELETE FROM btech_artillery WHERE map_dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
    }
    Ok(())
}
