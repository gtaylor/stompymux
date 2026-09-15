//! Durable shared wreck timers and selective removal of retired simulation identities.
use crate::{ObjectId, World};
use anyhow::{Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

/// Detect the optional extension without writing during load.
async fn installed(c: &mut SqliteConnection) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_wrecks'",
    )
    .fetch_one(c)
    .await?
        == 1)
}

/// Load bounded countdowns; domain validation checks the associated unit's condition.
pub(super) async fn load(c: &mut SqliteConnection) -> Result<BTreeMap<ObjectId, u8>> {
    let mut timers = BTreeMap::new();
    if !installed(c).await? {
        return Ok(timers);
    }
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
    let retired = crate::btech::wreck_cleanup::retired(before, after)?;
    if !retired.is_empty() {
        super::btech_units::purge(c, &retired).await?;
        super::btech_vehicles::purge(c, &retired).await?;
    }
    for id in &retired {
        sqlx::query("DELETE FROM btech_special_registrations WHERE dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
    }
    if before.btech.wrecks == after.btech.wrecks {
        return Ok(!retired.is_empty());
    }
    if !installed(c).await? {
        sqlx::query("CREATE TABLE btech_wrecks (dbref INTEGER PRIMARY KEY REFERENCES objects(dbref), remaining INTEGER NOT NULL CHECK(remaining BETWEEN 1 AND 10))").execute(&mut *c).await?;
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
    if ids.is_empty() || !installed(c).await? {
        return Ok(());
    }
    for id in ids {
        sqlx::query("DELETE FROM btech_wrecks WHERE dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
    }
    Ok(())
}
