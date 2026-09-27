//! Persist elapsed simulation time and the turn phase that advances with it.
//!
//! Both are stored in one row: the simulation second and the phase's fixed offset from
//! it. The phase advances in lockstep with the clock, so the offset only changes when a
//! phase is set directly. Every other stored countdown is a deadline on this clock, so
//! the row is written with any other change but an idle tick that changes nothing else
//! needs no write, until `database.clock_save_interval` seconds have passed. A crash
//! restores the world exactly as of the last stored second, losing at most that much
//! idle time; explicit saves and shutdown always store the current clock.
use crate::{World, btech::turn_clock::TurnClock};
use anyhow::{Result, ensure};
use sqlx::SqliteConnection;

/// Length of one turn, in seconds.
const TURN: i64 = 30;

/// The phase's offset from the simulation clock.
fn phase_offset(world: &World) -> i64 {
    (i64::from(u8::from(world.btech.turn_clock)) - world.btech.simulation_seconds).rem_euclid(TURN)
}

/// The stored second and phase offset; a missing row stands for second and phase zero.
async fn stored(c: &mut SqliteConnection) -> Result<(i64, i64)> {
    Ok(
        sqlx::query_as("SELECT seconds,phase_offset FROM btech_simulation_clock WHERE id=1")
            .fetch_optional(&mut *c)
            .await?
            .unwrap_or((0, 0)),
    )
}

/// Read the clock without advancing offline simulation.
pub(super) async fn load(c: &mut SqliteConnection) -> Result<(TurnClock, i64)> {
    let (seconds, offset) = stored(c).await?;
    ensure!(seconds >= 0, "Invalid simulation time");
    let phase = u8::try_from((seconds + offset).rem_euclid(TURN))?;
    Ok((phase.try_into()?, seconds))
}

/// Store the clock when it differs from the stored one and either `changed` says the
/// transaction wrote anything else, the phase offset changed, or the clock has run
/// `interval` seconds past the stored one (never when zero). Rows written alongside
/// other changes are relative to this clock, so it must match them. Returns whether
/// the row was written.
pub(super) async fn save(
    c: &mut SqliteConnection,
    after: &World,
    changed: bool,
    interval: u64,
) -> Result<bool> {
    let current = (after.btech.simulation_seconds, phase_offset(after));
    let (seconds, offset) = stored(c).await?;
    if (seconds, offset) == current {
        return Ok(false);
    }
    let due = interval > 0 && current.0 - seconds >= i64::try_from(interval).unwrap_or(i64::MAX);
    if !changed && offset == current.1 && !due {
        return Ok(false);
    }
    sqlx::query(
        "INSERT INTO btech_simulation_clock(id,seconds,phase_offset) VALUES(1,?,?) \
         ON CONFLICT(id) DO UPDATE SET seconds=excluded.seconds,phase_offset=excluded.phase_offset",
    )
    .bind(after.btech.simulation_seconds)
    .bind(phase_offset(after))
    .execute(c)
    .await?;
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sqlx::Connection;

    /// Whether the clock row exists.
    async fn has_row(c: &mut SqliteConnection) -> bool {
        sqlx::query_scalar::<_, i64>("SELECT count(*) FROM btech_simulation_clock")
            .fetch_one(c)
            .await
            .unwrap()
            == 1
    }

    /// The phase is rebuilt from the clock, and clock-only changes are not written alone.
    #[tokio::test]
    async fn clock_rows_follow_other_writes_and_keep_the_phase() {
        let mut db = SqliteConnection::connect("sqlite::memory:").await.unwrap();
        sqlx::raw_sql(include_str!("btech_schema.sql"))
            .execute(&mut db)
            .await
            .unwrap();
        let (phase, seconds) = load(&mut db).await.unwrap();
        assert_eq!((u8::from(phase), seconds), (0, 0));
        assert!(!has_row(&mut db).await);
        let at = |seconds: i64, phase: u8| {
            let mut world = World::default();
            world.btech.simulation_seconds = seconds;
            world.btech.turn_clock = phase.try_into().unwrap();
            world
        };
        let read = async |db: &mut SqliteConnection| {
            let (phase, seconds) = load(db).await.unwrap();
            (seconds, u8::from(phase))
        };

        // The clock at its start needs no row; a phase out of step with it is stored.
        assert!(!save(&mut db, &at(0, 0), true, 0).await.unwrap());
        assert!(!has_row(&mut db).await);
        assert!(save(&mut db, &at(100, 14), false, 0).await.unwrap());
        assert_eq!(read(&mut db).await, (100, 14));

        // A clock that is the only change waits for other writes.
        assert!(!save(&mut db, &at(101, 15), false, 0).await.unwrap());
        assert_eq!(read(&mut db).await, (100, 14));
        assert!(save(&mut db, &at(101, 15), true, 0).await.unwrap());
        assert_eq!(read(&mut db).await, (101, 15));
        assert!(!save(&mut db, &at(101, 15), true, 0).await.unwrap());

        // A clock that is the only change is stored once it runs a full interval ahead.
        assert!(!save(&mut db, &at(102, 16), false, 2).await.unwrap());
        assert!(save(&mut db, &at(103, 17), false, 2).await.unwrap());
        assert_eq!(read(&mut db).await, (103, 17));
    }
}
