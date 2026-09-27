//! Durable shared wreck timers and selective removal of retired simulation identities.
use super::write::purge_rows;
use crate::{ObjectId, World};
use anyhow::{Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

/// Load bounded countdowns; domain validation checks the associated unit's condition.
pub(super) async fn load(c: &mut SqliteConnection) -> Result<BTreeMap<ObjectId, u8>> {
    let mut timers = BTreeMap::new();
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM btech_wrecks")
        .fetch_one(&mut *c)
        .await?;
    ensure!(count <= 1_000_000, "Too many wreck timers");
    for row in sqlx::query("SELECT dbref,remaining FROM btech_wrecks ORDER BY dbref")
        .fetch_all(c)
        .await?
    {
        timers.insert(ObjectId(row.try_get("dbref")?), row.try_get("remaining")?);
    }
    Ok(timers)
}

/// Retire native records and timers inside the existing atomic world save.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let retired = crate::btech::wreck_cleanup::retired(
        before,
        after,
        &crate::btech::unit_lifecycle::unregistered(before, after),
    )?;
    if !retired.is_empty() {
        super::btech_units::purge(c, &retired).await?;
        super::btech_vehicles::purge(c, &retired).await?;
        purge_rows(c, "btech_special_registrations", "dbref", &retired).await?;
    }
    if before.btech.wrecks == after.btech.wrecks {
        return Ok(!retired.is_empty());
    }
    let removed = before
        .btech
        .wrecks
        .keys()
        .filter(|id| !after.btech.wrecks.contains_key(id))
        .copied()
        .collect();
    purge(c, &removed).await?;
    for (&id, &remaining) in after.btech.wrecks.iter() {
        if before.btech.wrecks.get(&id) == Some(&remaining) {
            continue;
        }
        sqlx::query("INSERT INTO btech_wrecks VALUES(?,?) ON CONFLICT(dbref) DO UPDATE SET remaining=excluded.remaining").bind(id.0).bind(remaining).execute(&mut *c).await?;
    }
    Ok(true)
}

/// Remove timers before ordinary object purges, including maps' unrelated occupants only when deleted.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    purge_rows(c, "btech_wrecks", "dbref", ids).await
}
