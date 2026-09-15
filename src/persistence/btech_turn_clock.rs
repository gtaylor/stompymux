//! Persist the global BattleTech phase in the same transaction as rolls and their consequences.
use crate::{World, btech::turn_clock::TurnClock};
use anyhow::Result;
use sqlx::SqliteConnection;

/// Read-only loading never advances the simulation for time spent offline.
pub(super) async fn load(c: &mut SqliteConnection) -> Result<TurnClock> {
    let installed: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_turn_clock'",
    )
    .fetch_one(&mut *c)
    .await?;
    if installed == 0 {
        return Ok(TurnClock::default());
    }
    let phase: u8 = sqlx::query_scalar("SELECT phase FROM btech_turn_clock WHERE id=1")
        .fetch_one(c)
        .await?;
    phase.try_into()
}

/// Only the enclosing committed heartbeat can consume a turn phase.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    if before.btech.turn_clock == after.btech.turn_clock {
        return Ok(false);
    }
    sqlx::query("CREATE TABLE IF NOT EXISTS btech_turn_clock (id INTEGER PRIMARY KEY CHECK(id=1), phase INTEGER NOT NULL CHECK(phase BETWEEN 0 AND 29))").execute(&mut *c).await?;
    sqlx::query("INSERT INTO btech_turn_clock VALUES(1,?) ON CONFLICT(id) DO UPDATE SET phase=excluded.phase").bind(u8::from(after.btech.turn_clock)).execute(c).await?;
    Ok(true)
}
