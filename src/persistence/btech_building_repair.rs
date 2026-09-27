//! Sparse durable building repair clocks, independent of wall time and terrain.
//!
//! Each clock is stored as the simulation second of its next repair step, so a running
//! clock causes no writes between steps.
use super::btech_deadlines::Clock;
use super::write::{Fields, Rows, purge_rows, sync_rows};
use crate::{ObjectId, StoredBattleMap, World};
use anyhow::{Context, Result};
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

/// Longest repair countdown, in seconds.
const MAX_REMAINING: i64 = 120;

/// Read the actual owned rows; inferred load-time clocks need not yet have a database row.
async fn records(c: &mut SqliteConnection, clock: Clock) -> Result<BTreeMap<ObjectId, u16>> {
    let mut clocks = BTreeMap::new();
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

/// The rows a world's running clocks call for, relative to its clock.
fn rows(world: &World) -> Rows {
    let now = Clock::of(world);
    world
        .btech
        .maps()
        .iter()
        .filter_map(|(&id, map)| {
            let remaining = map.building_repair?;
            Some((
                vec![id.0],
                Fields::from([("repairs_at", now.deadline(remaining))]),
            ))
        })
        .collect()
}

/// Save countdown changes in the enclosing world transaction.
///
/// A loaded clock may be inferred from building damage without having a row yet, so
/// while any clock runs the stored rows are read back rather than compared with the
/// baseline; a world without running clocks, the common case, touches nothing.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let desired = rows(after);
    if desired.is_empty() && rows(before).is_empty() {
        return Ok(false);
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
    purge_rows(c, "btech_building_repair", "map_dbref", ids).await
}
