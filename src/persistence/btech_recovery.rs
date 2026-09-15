//! Versioned player recovery persistence, installed only by an explicit native write.
use super::write::{Cell, fields, row};
use crate::{BattleRecovery, ObjectId, World};
use anyhow::{Result, ensure};
use sqlx::{Row, SqliteConnection};
use std::collections::{BTreeMap, BTreeSet};

/// Read-only extension detection for existing game databases.
async fn installed(c: &mut SqliteConnection) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_character_recovery'",
    )
    .fetch_one(c)
    .await?
        == 1)
}

/// Decode bounded versioned records without inventing a replacement random stream.
pub(super) async fn load(c: &mut SqliteConnection) -> Result<BTreeMap<ObjectId, BattleRecovery>> {
    let mut records = BTreeMap::new();
    if !installed(c).await? {
        return Ok(records);
    }
    for entry in sqlx::query("SELECT player_dbref,state_version,length(CAST(recovery AS BLOB)) AS bytes,CASE WHEN length(CAST(recovery AS BLOB))<=16384 THEN recovery ELSE NULL END AS recovery FROM btech_character_recovery").fetch_all(c).await? {
        ensure!(entry.try_get::<i64,_>("state_version")? == 1, "Unsupported recovery state version");
        ensure!(entry.try_get::<i64,_>("bytes")? <= 16384, "Recovery state exceeds size limit");
        let encoded: String = entry.try_get("recovery")?;
        let recovery: BattleRecovery = serde_json::from_str(&encoded)?;
        recovery.validate()?;
        records.insert(ObjectId(entry.try_get("player_dbref")?), recovery);
    }
    Ok(records)
}

/// Update owned columns, retaining independent extensions on the same row.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    for (&id, recovery) in after.btech.recoveries() {
        let previous = before.btech.recoveries().get(&id);
        if previous == Some(recovery) {
            continue;
        }
        if !installed(c).await? {
            sqlx::raw_sql(include_str!("btech_recovery.sql"))
                .execute(&mut *c)
                .await?;
        }
        let old: Option<String> = sqlx::query_scalar(
            "SELECT recovery FROM btech_character_recovery WHERE player_dbref=?",
        )
        .bind(id.0)
        .fetch_optional(&mut *c)
        .await?;
        let old = old.map(|encoded| {
            fields([
                ("state_version", Cell::Integer(1)),
                ("recovery", Cell::Text(encoded)),
            ])
        });
        row(
            c,
            "btech_character_recovery",
            fields([("player_dbref", Cell::Integer(id.0))]),
            old.as_ref(),
            &fields([
                ("state_version", Cell::Integer(1)),
                ("recovery", Cell::Text(serde_json::to_string(recovery)?)),
            ]),
        )
        .await?;
        changed = true;
    }
    Ok(changed)
}

/// Explicit cleanup before object tombstones replace player identities.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    if !installed(c).await? {
        return Ok(());
    }
    for id in ids {
        sqlx::query("DELETE FROM btech_character_recovery WHERE player_dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
    }
    Ok(())
}
