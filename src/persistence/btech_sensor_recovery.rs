//! Ordered computer recovery events participate in the enclosing database transaction.
//!
//! Each event is one typed row keyed by its queue position and holding the simulation
//! second it recovers, so running events cause no writes until one completes.
use super::btech_deadlines::Clock;
use super::write::{Cell, Fields, Rows, fields, sync_rows};
use crate::{
    ObjectId, World,
    btech::computer_runtime::{Display, SensorRecovery},
};
use anyhow::{Result, bail};
use sqlx::{Row, SqliteConnection};
use std::collections::BTreeSet;

/// Stored columns besides the queue position.
const COLUMNS: &[&str] = &["unit_dbref", "display", "value", "recovers_at"];

/// Table definition, installed by the first write. `display` is 0 (tactical),
/// 1 (long range) or 2 (scanner).
const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS btech_sensor_recovery (
    position INTEGER PRIMARY KEY CHECK (position >= 0),
    unit_dbref INTEGER NOT NULL REFERENCES objects(dbref) ON DELETE CASCADE,
    display INTEGER NOT NULL CHECK (display BETWEEN 0 AND 2),
    value INTEGER NOT NULL CHECK (value BETWEEN 0 AND 127),
    recovers_at INTEGER NOT NULL CHECK (recovers_at > 0)
) STRICT";

/// Whether the queue table exists; reads never install it.
async fn installed(c: &mut SqliteConnection) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_sensor_recovery'",
    )
    .fetch_one(c)
    .await?
        == 1)
}

/// Stored code for a display.
fn display_code(display: Display) -> i64 {
    match display {
        Display::Tactical => 0,
        Display::LongRange => 1,
        Display::Scanner => 2,
    }
}

/// Display for a stored code.
fn display_from_code(code: i64) -> Result<Display> {
    Ok(match code {
        0 => Display::Tactical,
        1 => Display::LongRange,
        2 => Display::Scanner,
        other => bail!("Unknown recovering display {other}"),
    })
}

/// Owned column values for one event, relative to the saved clock.
fn encode(event: &SensorRecovery, clock: Clock) -> Fields {
    fields([
        ("unit_dbref", Cell::Integer(event.unit().0)),
        ("display", Cell::Integer(display_code(event.display()))),
        ("value", Cell::Integer(i64::from(event.value()))),
        ("recovers_at", clock.deadline(event.remaining())),
    ])
}

/// Missing extension tables represent an empty event queue and are not created during reads.
pub(super) async fn load(c: &mut SqliteConnection, clock: Clock) -> Result<Vec<SensorRecovery>> {
    if !installed(c).await? {
        return Ok(Vec::new());
    }
    let mut events = Vec::new();
    for entry in sqlx::query(
        "SELECT unit_dbref,display,value,recovers_at FROM btech_sensor_recovery ORDER BY position",
    )
    .fetch_all(c)
    .await?
    {
        events.push(SensorRecovery::from_saved(
            ObjectId(entry.try_get("unit_dbref")?),
            display_from_code(entry.try_get("display")?)?,
            u8::try_from(entry.try_get::<i64, _>("value")?)?,
            clock.remaining(entry.try_get("recovers_at")?, 200)?,
        ));
    }
    Ok(events)
}

/// Rows for a queue, relative to `clock`.
fn rows(events: &[SensorRecovery], clock: Clock) -> Rows {
    events
        .iter()
        .enumerate()
        .map(|(position, event)| (vec![position as i64], encode(event, clock)))
        .collect()
}

/// Preserve queue order, simultaneous deadlines and an explicitly emptied queue on restart.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let previous = rows(&before.btech.sensor_recoveries, Clock::of(before));
    let desired = rows(&after.btech.sensor_recoveries, Clock::of(after));
    if previous == desired {
        return Ok(false);
    }
    sqlx::query(SCHEMA).execute(&mut *c).await?;
    sync_rows(
        c,
        "btech_sensor_recovery",
        &[],
        &["position"],
        COLUMNS,
        &desired,
    )
    .await?;
    Ok(true)
}

/// Remove a purged unit's events before its object identity is tombstoned.
pub(super) async fn purge(c: &mut SqliteConnection, ids: &BTreeSet<ObjectId>) -> Result<()> {
    if !installed(c).await? {
        return Ok(());
    }
    for id in ids {
        sqlx::query("DELETE FROM btech_sensor_recovery WHERE unit_dbref=?")
            .bind(id.0)
            .execute(&mut *c)
            .await?;
    }
    Ok(())
}
