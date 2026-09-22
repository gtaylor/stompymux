//! Persist turn phase and elapsed simulation time in the same transaction as their consequences.
use crate::{World, btech::turn_clock::TurnClock};
use anyhow::{Result, ensure};
use sqlx::SqliteConnection;

async fn installed(c: &mut SqliteConnection, table: &str) -> Result<bool> {
    Ok(sqlx::query_scalar::<_, i64>(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?",
    )
    .bind(table)
    .fetch_one(c)
    .await?
        == 1)
}

/// Optional extension reads never create schema or advance offline simulation.
pub(super) async fn load(c: &mut SqliteConnection) -> Result<(TurnClock, i64)> {
    let phase: u8 = if installed(c, "btech_turn_clock").await? {
        sqlx::query_scalar("SELECT phase FROM btech_turn_clock WHERE id=1")
            .fetch_one(&mut *c)
            .await?
    } else {
        0
    };
    let seconds: i64 = if installed(c, "btech_simulation_clock").await? {
        sqlx::query_scalar("SELECT seconds FROM btech_simulation_clock WHERE id=1")
            .fetch_one(&mut *c)
            .await?
    } else {
        0
    };
    ensure!(seconds >= 0, "Invalid simulation time");
    Ok((phase.try_into()?, seconds))
}

/// Write both clocks through the enclosing heartbeat transaction.
pub(super) async fn save(c: &mut SqliteConnection, before: &World, after: &World) -> Result<bool> {
    let mut changed = false;
    if before.btech.turn_clock != after.btech.turn_clock {
        sqlx::query("CREATE TABLE IF NOT EXISTS btech_turn_clock (id INTEGER PRIMARY KEY CHECK(id=1), phase INTEGER NOT NULL CHECK(phase BETWEEN 0 AND 29))").execute(&mut *c).await?;
        sqlx::query("INSERT INTO btech_turn_clock(id,phase) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET phase=excluded.phase").bind(u8::from(after.btech.turn_clock)).execute(&mut *c).await?;
        changed = true;
    }
    if before.btech.simulation_seconds != after.btech.simulation_seconds {
        sqlx::query("CREATE TABLE IF NOT EXISTS btech_simulation_clock (id INTEGER PRIMARY KEY CHECK(id=1), seconds INTEGER NOT NULL CHECK(seconds>=0))").execute(&mut *c).await?;
        sqlx::query("INSERT INTO btech_simulation_clock(id,seconds) VALUES(1,?) ON CONFLICT(id) DO UPDATE SET seconds=excluded.seconds").bind(after.btech.simulation_seconds).execute(&mut *c).await?;
        changed = true;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Connection;

    #[tokio::test]
    async fn missing_elapsed_clock_loads_without_schema_writes_and_persists_independently() {
        let mut db = SqliteConnection::connect("sqlite::memory:").await.unwrap();
        sqlx::query("CREATE TABLE btech_turn_clock(id INTEGER PRIMARY KEY,phase INTEGER NOT NULL)")
            .execute(&mut db)
            .await
            .unwrap();
        sqlx::query("INSERT INTO btech_turn_clock VALUES(1,14)")
            .execute(&mut db)
            .await
            .unwrap();
        let (phase, seconds) = load(&mut db).await.unwrap();
        assert_eq!(u8::from(phase), 14);
        assert_eq!(seconds, 0);
        assert!(!installed(&mut db, "btech_simulation_clock").await.unwrap());
        let mut before = World::default();
        before.btech.turn_clock = phase;
        let mut after = before.clone();
        after.btech.simulation_seconds = 42;
        assert!(save(&mut db, &before, &after).await.unwrap());
        let (phase, seconds) = load(&mut db).await.unwrap();
        assert_eq!(u8::from(phase), 14);
        assert_eq!(seconds, 42);
    }
}
