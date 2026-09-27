//! Persist the bounded initial reactor window alongside unit-owned damage windows.
use super::{btech_deadlines::Clock, write::Cell};
use crate::{World, btech::reactor_instability::BattleReactorState};
use anyhow::Result;
use sqlx::SqliteConnection;

/// Detect this native extension without modifying a database during load.
async fn installed(c: &mut SqliteConnection) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name='btech_reactor_clock'",
    )
    .fetch_one(c)
    .await?
        == 1)
}

/// Longest startup window, in seconds.
const MAX_REMAINING: i64 = 31;

/// Table definition, installed by the first write. `closes_at` is the simulation second
/// the startup window closes, or NULL once it has closed.
const SCHEMA: &str = "CREATE TABLE IF NOT EXISTS btech_reactor_clock (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    closes_at INTEGER CHECK (closes_at > 0)
) STRICT";

/// A fresh world starts at event tick zero; an existing clock never reopens on reload.
pub(super) async fn load(c: &mut SqliteConnection, clock: Clock) -> Result<BattleReactorState> {
    let mut state = BattleReactorState::default();
    if installed(c).await? {
        let closes_at: Option<i64> =
            sqlx::query_scalar("SELECT closes_at FROM btech_reactor_clock WHERE id=1")
                .fetch_one(c)
                .await?;
        state.startup_remaining = clock.optional_remaining(closes_at, MAX_REMAINING)?;
    }
    state.validate()?;
    Ok(state)
}

/// Save the window's closing second; a window counting down with the clock is not rewritten.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let remaining = after.btech.reactor.startup_remaining;
    let closes_at = Clock::of(after).optional_deadline(remaining);
    let previous = Clock::of(before).optional_deadline(before.btech.reactor.startup_remaining);
    // Without a table a load starts a fresh window, so only a window that has begun
    // counting down needs a row.
    let unchanged = if installed(c).await? {
        previous == closes_at
    } else {
        remaining == BattleReactorState::default().startup_remaining
    };
    if unchanged {
        return Ok(false);
    }
    sqlx::query(SCHEMA).execute(&mut *c).await?;
    let closes_at = match closes_at {
        Cell::Integer(second) => Some(second),
        _ => None,
    };
    sqlx::query("INSERT INTO btech_reactor_clock(id,closes_at) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET closes_at=excluded.closes_at")
        .bind(closes_at)
        .execute(&mut *c)
        .await?;
    Ok(true)
}
