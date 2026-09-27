//! Ordered computer recovery events participate in the enclosing database transaction.
//!
//! Each event is one typed row keyed by its queue position and holding the simulation
//! second it recovers, so running events cause no writes until one completes.
use super::btech_deadlines::Clock;
use super::write::{Cell, Fields, Rows, purge_rows, sync_rows};
use crate::{
    ObjectId, World,
    btech::computer_runtime::{Display, SensorRecovery},
};
use anyhow::{Result, bail};
use sqlx::{Row, SqliteConnection};
use std::collections::BTreeSet;

/// Stored columns besides the queue position.
const COLUMNS: &[&str] = &["unit_dbref", "display", "value", "recovers_at"];

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
    Fields::from([
        ("unit_dbref", Cell::Integer(event.unit().0)),
        ("display", Cell::Integer(display_code(event.display()))),
        ("value", Cell::Integer(i64::from(event.value()))),
        ("recovers_at", clock.deadline(event.remaining())),
    ])
}

/// Read the queue in order.
pub(super) async fn load(c: &mut SqliteConnection, clock: Clock) -> Result<Vec<SensorRecovery>> {
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
    purge_rows(c, "btech_sensor_recovery", "unit_dbref", ids).await
}
