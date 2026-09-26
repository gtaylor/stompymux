//! Typed player recovery persistence, installed only by an explicit native write.
use super::write::{Cell, Fields, fields, row};
use crate::{BattleRecovery, BattleRecoveryMode, ObjectId, World};
use anyhow::{Context, Result, bail};
use sqlx::{Row, SqliteConnection, sqlite::SqliteRow};
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

/// Columns read back for one record, in the order [`decode`] expects.
fn select(filter: &str) -> String {
    format!(
        "SELECT player_dbref,mode,tactical_injuries,remaining,pain_resistance,toughness,{} \
         FROM btech_character_recovery{filter}",
        super::btech_dice::COLUMNS
    )
}

/// Rebuild one record from its typed columns.
fn decode(entry: &SqliteRow) -> Result<BattleRecovery> {
    let mode = match entry.try_get::<i64, _>("mode")? {
        0 => BattleRecoveryMode::Ready,
        1 => BattleRecoveryMode::Character,
        2 => BattleRecoveryMode::Tactical {
            injuries: u8::try_from(
                entry
                    .try_get::<Option<i64>, _>("tactical_injuries")?
                    .context("Tactical recovery lacks an injury count")?,
            )?,
        },
        other => bail!("Unknown recovery mode {other}"),
    };
    Ok(BattleRecovery::from_saved(
        mode,
        u8::try_from(entry.try_get::<i64, _>("remaining")?)?,
        entry.try_get("pain_resistance")?,
        entry.try_get("toughness")?,
        super::btech_dice::read(entry)?,
    ))
}

/// Owned column values for one record.
fn encode(recovery: &BattleRecovery) -> Fields {
    let (mode, injuries) = match recovery.mode {
        BattleRecoveryMode::Ready => (0, Cell::Null),
        BattleRecoveryMode::Character => (1, Cell::Null),
        BattleRecoveryMode::Tactical { injuries } => (2, Cell::Integer(i64::from(injuries))),
    };
    let mut values = fields([
        ("mode", Cell::Integer(mode)),
        ("tactical_injuries", injuries),
        ("remaining", Cell::Integer(i64::from(recovery.remaining))),
        (
            "pain_resistance",
            Cell::Integer(i64::from(recovery.pain_resistance)),
        ),
        ("toughness", Cell::Integer(i64::from(recovery.toughness))),
    ]);
    values.extend(fields(super::btech_dice::fields(recovery.dice())));
    values
}

/// Decode typed records without inventing a replacement random stream.
pub(super) async fn load(c: &mut SqliteConnection) -> Result<BTreeMap<ObjectId, BattleRecovery>> {
    let mut records = BTreeMap::new();
    if !installed(c).await? {
        return Ok(records);
    }
    for entry in sqlx::query(sqlx::AssertSqlSafe(select("")))
        .fetch_all(c)
        .await?
    {
        let recovery = decode(&entry)?;
        recovery.validate()?;
        records.insert(ObjectId(entry.try_get("player_dbref")?), recovery);
    }
    Ok(records)
}

/// Update only changed columns, so a ticking countdown rewrites a single value.
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
        let old = match sqlx::query(sqlx::AssertSqlSafe(select(" WHERE player_dbref=?")))
            .bind(id.0)
            .fetch_optional(&mut *c)
            .await?
        {
            Some(entry) => Some(encode(&decode(&entry)?)),
            None => None,
        };
        row(
            c,
            "btech_character_recovery",
            fields([("player_dbref", Cell::Integer(id.0))]),
            old.as_ref(),
            &encode(recovery),
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
