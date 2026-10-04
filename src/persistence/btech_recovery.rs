//! Typed player recovery persistence.
//!
//! The recovery countdown is stored as the simulation second of the next check, so an
//! unconscious player's row is rewritten only when a check rolls dice or reschedules.
use super::btech_deadlines::Clock;
use super::write::{Cell, Fields, purge_rows, sync_rows};
use crate::{ObjectId, Recovery, RecoveryMode, World};
use anyhow::{Context, Result, bail};
use sqlx::{Row, SqliteConnection, sqlite::SqliteRow};
use std::collections::{BTreeMap, BTreeSet};

/// Longest recovery countdown, in seconds.
const MAX_REMAINING: i64 = 30;

/// Stored columns besides the player.
const COLUMNS: &[&str] = &[
    "mode",
    "tactical_injuries",
    "recovers_at",
    "pain_resistance",
    "toughness",
    "dice_seed",
    "dice_stream",
    "dice_block",
    "dice_word",
];

/// Rebuild one record from its typed columns.
fn decode(entry: &SqliteRow, clock: Clock) -> Result<Recovery> {
    let mode = match entry.try_get::<i64, _>("mode")? {
        0 => RecoveryMode::Ready,
        1 => RecoveryMode::Character,
        2 => RecoveryMode::Tactical {
            injuries: u8::try_from(
                entry
                    .try_get::<Option<i64>, _>("tactical_injuries")?
                    .context("Tactical recovery lacks an injury count")?,
            )?,
        },
        other => bail!("Unknown recovery mode {other}"),
    };
    Ok(Recovery::from_saved(
        mode,
        clock.optional_remaining(entry.try_get("recovers_at")?, MAX_REMAINING)?,
        entry.try_get("pain_resistance")?,
        entry.try_get("toughness")?,
        super::btech_dice::read(entry)?,
    ))
}

/// Owned column values for one record, relative to the saved clock.
fn encode(recovery: &Recovery, clock: Clock) -> Fields {
    let (mode, injuries) = match recovery.mode {
        RecoveryMode::Ready => (0, Cell::Null),
        RecoveryMode::Character => (1, Cell::Null),
        RecoveryMode::Tactical { injuries } => (2, Cell::Integer(i64::from(injuries))),
    };
    let mut values = Fields::from([
        ("mode", Cell::Integer(mode)),
        ("tactical_injuries", injuries),
        ("recovers_at", clock.optional_deadline(recovery.remaining)),
        (
            "pain_resistance",
            Cell::Integer(i64::from(recovery.pain_resistance)),
        ),
        ("toughness", Cell::Integer(i64::from(recovery.toughness))),
    ]);
    values.extend(super::btech_dice::fields(recovery.dice()));
    values
}

/// Decode typed records without inventing a replacement random stream.
pub(super) async fn load(
    c: &mut SqliteConnection,
    clock: Clock,
) -> Result<BTreeMap<ObjectId, Recovery>> {
    let mut records = BTreeMap::new();
    let query = format!(
        "SELECT player_dbref,{} FROM btech_character_recovery",
        COLUMNS.join(",")
    );
    for entry in sqlx::query(sqlx::AssertSqlSafe(query)).fetch_all(c).await? {
        let recovery = decode(&entry, clock)?;
        recovery.validate()?;
        records.insert(ObjectId(entry.try_get("player_dbref")?), recovery);
    }
    Ok(records)
}

/// Write records whose stored form changed; a countdown in step with the clock does not.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let (then, now) = (Clock::of(before), Clock::of(after));
    let mut changed = false;
    for (&id, recovery) in after.btech.recoveries() {
        let old = before.btech.recoveries().get(&id);
        // An identical record keeps its deadline unless a countdown sat still while the
        // clock moved.
        if old == Some(recovery) && (then == now || recovery.remaining == 0) {
            continue;
        }
        let desired = encode(recovery, now);
        if before
            .btech
            .recoveries()
            .get(&id)
            .map(|old| encode(old, then))
            == Some(desired.clone())
        {
            continue;
        }
        changed |= sync_rows(
            c,
            "btech_character_recovery",
            &[("player_dbref", id.0)],
            &[],
            COLUMNS,
            &BTreeMap::from([(Vec::new(), desired)]),
        )
        .await?;
    }
    Ok(changed)
}

/// Explicit cleanup before object tombstones replace player identities.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    purge_rows(c, "btech_character_recovery", "player_dbref", ids).await
}
