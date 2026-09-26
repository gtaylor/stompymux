//! Durable map-owned randomness, independent of unit lifetime and player sessions.
use super::write::{Cell, fields, row};
use crate::{ObjectId, StoredBattleMap, World};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

/// Detect saved streams without modifying databases on read.
async fn installed(c: &mut SqliteConnection) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_map_random'",
    )
    .fetch_one(c)
    .await?
        == 1)
}

/// Decode typed streams and reject orphan records.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredBattleMap>,
) -> Result<()> {
    if !installed(c).await? {
        return Ok(());
    }
    use futures_util::TryStreamExt;
    let query = format!(
        "SELECT map_dbref,{} FROM btech_map_random",
        super::btech_dice::COLUMNS
    );
    let mut rows = sqlx::query(sqlx::AssertSqlSafe(query)).fetch(c);
    while let Some(row) = rows.try_next().await? {
        let map = maps
            .get_mut(&ObjectId(row.try_get("map_dbref")?))
            .context("Random stream references missing map")?;
        map.fire_dice = Some(super::btech_dice::read(&row)?);
    }
    Ok(())
}

/// Preserve the stream even after the final fire disappears, updating only owned columns.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = before
            .btech
            .maps()
            .get(&id)
            .and_then(|map| map.fire_dice.as_ref());
        let Some(dice) = &map.fire_dice else {
            ensure!(old.is_none(), "Map random stream cannot be discarded");
            continue;
        };
        if old == Some(dice) {
            continue;
        }
        if !installed(c).await? {
            sqlx::raw_sql(include_str!("btech_map_random.sql"))
                .execute(&mut *c)
                .await?;
        }
        let query = format!(
            "SELECT {} FROM btech_map_random WHERE map_dbref=?",
            super::btech_dice::COLUMNS
        );
        let previous = match sqlx::query(sqlx::AssertSqlSafe(query))
            .bind(id.0)
            .fetch_optional(&mut *c)
            .await?
        {
            Some(stored) => Some(fields(super::btech_dice::fields(&super::btech_dice::read(
                &stored,
            )?))),
            None => None,
        };
        row(
            c,
            "btech_map_random",
            fields([("map_dbref", Cell::Integer(id.0))]),
            previous.as_ref(),
            &fields(super::btech_dice::fields(dice)),
        )
        .await?;
        changed = true;
    }
    Ok(changed)
}

/// Remove streams before map identities are purged.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    if !installed(c).await? {
        return Ok(());
    }
    for id in ids {
        sqlx::query("DELETE FROM btech_map_random WHERE map_dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
    }
    Ok(())
}
