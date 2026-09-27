//! Persist the bounded initial reactor window alongside unit-owned damage windows.
//!
//! The window's closing second is stored in `btech_reactor_clock`; a window counting down
//! with the clock is not rewritten. A database without the row holds a fresh world, whose
//! window has not started closing.
use super::{btech_deadlines::Clock, write::Cell};
use crate::{World, btech::reactor_instability::BattleReactorState};
use anyhow::Result;
use sqlx::SqliteConnection;

/// Longest startup window, in seconds.
const MAX_REMAINING: i64 = 31;

/// The stored closing second, if a row exists.
async fn stored(c: &mut SqliteConnection) -> Result<Option<Option<i64>>> {
    Ok(
        sqlx::query_scalar("SELECT closes_at FROM btech_reactor_clock WHERE id=1")
            .fetch_optional(c)
            .await?,
    )
}

/// A fresh world starts at event tick zero; an existing clock never reopens on reload.
pub(super) async fn load(c: &mut SqliteConnection, clock: Clock) -> Result<BattleReactorState> {
    let mut state = BattleReactorState::default();
    if let Some(closes_at) = stored(c).await? {
        state.startup_remaining = clock.optional_remaining(closes_at, MAX_REMAINING)?;
    }
    state.validate()?;
    Ok(state)
}

/// Save the window's closing second when it changed, or when a window that has started
/// counting down has no row yet.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let remaining = after.btech.reactor.startup_remaining;
    let closes_at = Clock::of(after).optional_deadline(remaining);
    let previous = Clock::of(before).optional_deadline(before.btech.reactor.startup_remaining);
    if previous == closes_at {
        // A fresh world's untouched window needs no row, and a row that exists already
        // holds this deadline.
        if remaining == BattleReactorState::default().startup_remaining
            || stored(c).await?.is_some()
        {
            return Ok(false);
        }
    }
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
