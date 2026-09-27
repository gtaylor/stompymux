//! Sparse durable building repair clocks, independent of wall time and terrain.
//!
//! Each clock is stored as the simulation second of its next repair step, so a running
//! clock causes no writes between steps.
use super::btech_deadlines::Clock;
use super::write::{Rows, fields, sync_rows};
use crate::{ObjectId, StoredBattleMap, World};
use anyhow::{Context, Result};
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

/// Longest repair countdown, in seconds.
const MAX_REMAINING: i64 = 120;

/// Read the actual owned rows; inferred load-time clocks need not yet have a database row.
async fn records(c: &mut SqliteConnection, clock: Clock) -> Result<BTreeMap<ObjectId, u16>> {
    let mut clocks = BTreeMap::new();
    if !installed(c).await? {
        return Ok(clocks);
    }
    for row in
        sqlx::query("SELECT map_dbref,repairs_at FROM btech_building_repair ORDER BY map_dbref")
            .fetch_all(c)
            .await?
    {
        let id = ObjectId(row.try_get("map_dbref")?);
        let remaining = clock
            .remaining(row.try_get("repairs_at")?, MAX_REMAINING)
            .context("Invalid building repair countdown")?;
        clocks.insert(id, remaining);
    }
    Ok(clocks)
}

/// Preserve committed clocks and resume missing intervals for referenced, damaged interiors.
/// Reading a database never writes a clock or consumes elapsed offline time.
pub(super) async fn load(
    c: &mut SqliteConnection,
    maps: &mut BTreeMap<ObjectId, StoredBattleMap>,
    clock: Clock,
) -> Result<()> {
    for (id, remaining) in records(c, clock).await? {
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

/// Save countdown changes in the enclosing world transaction. The stored rows are read
/// back because load-time clocks may not have a row yet; running clocks keep their rows.
pub(super) async fn save(c: &mut SqliteConnection, after: &World) -> Result<bool> {
    let now = Clock::of(after);
    let desired: Rows = after
        .btech
        .maps()
        .iter()
        .filter_map(|(&id, map)| {
            let remaining = map.building_repair?;
            Some((
                vec![id.0],
                fields([("repairs_at", now.deadline(remaining))]),
            ))
        })
        .collect();
    if desired.is_empty() && !installed(c).await? {
        return Ok(false);
    }
    if !installed(c).await? {
        sqlx::raw_sql(include_str!("btech_building_repair.sql"))
            .execute(&mut *c)
            .await?;
    }
    sync_rows(
        c,
        "btech_building_repair",
        &[],
        &["map_dbref"],
        &["repairs_at"],
        &desired,
    )
    .await
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
