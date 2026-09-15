//! Selective persistence for the common external tow relationship table.
use super::write::{Cell, fields, row};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

/// Detect optional authored tow state without modifying a database during loading.
async fn installed(c: &mut SqliteConnection) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_tows'",
    )
    .fetch_one(c)
    .await?
        == 1)
}

/// Read relationships; enclosing world validation checks participants and pair disjointness.
pub(super) async fn load(c: &mut SqliteConnection) -> Result<BTreeMap<ObjectId, ObjectId>> {
    let mut tows = BTreeMap::new();
    if !installed(c).await? {
        return Ok(tows);
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM btech_tows")
        .fetch_one(&mut *c)
        .await?;
    ensure!(count <= 1_000_000, "Too many tow relationships");
    for entry in
        sqlx::query("SELECT carrier_dbref,target_dbref FROM btech_tows ORDER BY carrier_dbref")
            .fetch_all(c)
            .await?
    {
        tows.insert(
            ObjectId(entry.try_get("carrier_dbref")?),
            ObjectId(entry.try_get("target_dbref")?),
        );
    }
    Ok(tows)
}

/// Delete detached pairs before inserting new ownership, preserving unrelated row columns.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    if before.btech.tows() == after.btech.tows() {
        return Ok(false);
    }
    if !installed(c).await? {
        sqlx::raw_sql(include_str!("btech_tows.sql"))
            .execute(&mut *c)
            .await?;
    }
    for &carrier in before.btech.tows().keys() {
        if !after.btech.tows().contains_key(&carrier) {
            sqlx::query("DELETE FROM btech_tows WHERE carrier_dbref=?")
                .bind(carrier.0)
                .execute(&mut *c)
                .await?;
        }
    }
    for (&carrier, &target) in after.btech.tows() {
        if before.btech.tows().get(&carrier) == Some(&target) {
            continue;
        }
        let old = before
            .btech
            .tows()
            .get(&carrier)
            .map(|target| fields([("target_dbref", Cell::Integer(target.0))]));
        row(
            c,
            "btech_tows",
            fields([("carrier_dbref", Cell::Integer(carrier.0))]),
            old.as_ref(),
            &fields([("target_dbref", Cell::Integer(target.0))]),
        )
        .await?;
    }
    Ok(true)
}

/// Remove either endpoint before object identities are purged.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    if !installed(c).await? {
        return Ok(());
    }
    for id in ids {
        sqlx::query("DELETE FROM btech_tows WHERE carrier_dbref=? OR target_dbref=?")
            .bind(id.0)
            .bind(id.0)
            .execute(&mut *c)
            .await?;
    }
    Ok(())
}
