//! Sparse durable building repair clocks, independent of wall time and terrain.
use super::write::{Cell, fields, row};
use crate::{ObjectId, StoredBattleMap, World};
use anyhow::{Context, Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

/// Detect optional owned storage without changing a read-only database.
async fn installed(c: &mut SqliteConnection) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_building_repair'",
    )
    .fetch_one(c)
    .await?
        == 1)
}

/// Read the actual owned rows; inferred load-time clocks need not yet have a database row.
async fn records(c: &mut SqliteConnection) -> Result<BTreeMap<ObjectId, u16>> {
    let mut clocks = BTreeMap::new();
    if !installed(c).await? {
        return Ok(clocks);
    }
    use futures_util::TryStreamExt;
    let mut rows =
        sqlx::query("SELECT map_dbref,remaining FROM btech_building_repair ORDER BY map_dbref")
            .fetch(c);
    while let Some(row) = rows.try_next().await? {
        let id = ObjectId(row.try_get("map_dbref")?);
        let remaining = u16::try_from(row.try_get::<i64, _>("remaining")?)?;
        ensure!(
            (1..=120).contains(&remaining),
            "Invalid building repair countdown"
        );
        clocks.insert(id, remaining);
    }
    Ok(clocks)
}

/// Preserve committed clocks and resume missing intervals for referenced, damaged interiors.
/// Reading a database never writes a clock or consumes elapsed offline time.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredBattleMap>,
) -> Result<()> {
    for (id, remaining) in records(c).await? {
        maps.get_mut(&id)
            .context("Repair references missing map")?
            .building_repair = Some(remaining);
    }
    let interiors: BTreeSet<_> = maps
        .values()
        .flat_map(|map| {
            map.building_entrances
                .values()
                .map(|entrance| entrance.interior)
        })
        .collect();
    for id in interiors {
        // An entrance can retain its object target after that object relinquishes its MAP role.
        let Some(map) = maps.get_mut(&id) else {
            continue;
        };
        if map.building_repair.is_none() {
            map.building_repair = map.building.repair_delay();
        }
    }
    Ok(())
}

/// Save countdown changes in the enclosing world transaction, preserving extension columns.
pub(super) async fn save(c: &mut SqliteConnection, after: &World) -> Result<bool> {
    let stored = records(c).await?;
    let mut changed = false;
    for (&id, map) in after.btech.maps() {
        let old = stored.get(&id).copied();
        if old == map.building_repair {
            continue;
        }
        if !installed(c).await? {
            sqlx::raw_sql(include_str!("btech_building_repair.sql"))
                .execute(&mut *c)
                .await?;
        }
        if let Some(remaining) = map.building_repair {
            row(
                c,
                "btech_building_repair",
                fields([("map_dbref", Cell::Integer(id.0))]),
                old.map(|value| fields([("remaining", Cell::Integer(i64::from(value)))]))
                    .as_ref(),
                &fields([("remaining", Cell::Integer(i64::from(remaining)))]),
            )
            .await?;
        } else {
            sqlx::query("DELETE FROM btech_building_repair WHERE map_dbref=?")
                .bind(id.0)
                .execute(&mut *c)
                .await?;
        }
        changed = true;
    }
    Ok(changed)
}

/// Remove countdowns before their maps are purged.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    if !installed(c).await? {
        return Ok(());
    }
    for id in ids {
        sqlx::query("DELETE FROM btech_building_repair WHERE map_dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
    }
    Ok(())
}
