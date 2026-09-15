//! Persist the bounded initial reactor window alongside unit-owned damage windows.
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

/// A fresh world starts at event tick zero; an existing clock never reopens on reload.
pub(super) async fn load(c: &mut SqliteConnection) -> Result<BattleReactorState> {
    let mut state = BattleReactorState::default();
    if installed(c).await? {
        state.startup_remaining =
            sqlx::query_scalar("SELECT startup_remaining FROM btech_reactor_clock WHERE id=1")
                .fetch_one(c)
                .await?;
    }
    state.validate()?;
    Ok(state)
}

/// Save only elapsed timing, using the enclosing world transaction for failure recovery.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let remaining = after.btech.reactor.startup_remaining;
    if before.btech.reactor.startup_remaining == remaining {
        return Ok(false);
    }
    sqlx::query("CREATE TABLE IF NOT EXISTS btech_reactor_clock (id INTEGER PRIMARY KEY CHECK (id=1), startup_remaining INTEGER NOT NULL CHECK (startup_remaining BETWEEN 0 AND 31))").execute(&mut *c).await?;
    sqlx::query("INSERT INTO btech_reactor_clock VALUES(1,?) ON CONFLICT(id) DO UPDATE SET startup_remaining=excluded.startup_remaining").bind(remaining).execute(&mut *c).await?;
    Ok(true)
}
