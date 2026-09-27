//! Countdowns stored as the simulation second at which they end.
//!
//! Game state counts timers down one second per heartbeat. Storing `remaining` would
//! rewrite every running timer every second, so tables store the deadline instead:
//! the saved simulation clock plus the remaining seconds. While a timer counts down in
//! step with the clock its deadline is constant and its row is left alone; loading
//! subtracts the saved clock to recover the countdown. A timer that pauses while the
//! clock advances gets a later deadline, which is simply saved as a change.
//!
//! The simulation clock stops while the server is down, so saved timers resume where
//! they left off rather than expiring during downtime.
use super::write::Cell;
use crate::World;
use anyhow::{Context, Result, ensure};

/// The simulation second at which a world was saved or loaded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Clock(i64);

impl Clock {
    /// The clock of a world about to be saved.
    pub(super) fn of(world: &World) -> Self {
        Self(world.btech.simulation_seconds)
    }

    /// The clock read back from the database during a load.
    pub(super) fn at(seconds: i64) -> Self {
        Self(seconds)
    }

    /// The simulation second itself.
    pub(super) fn seconds(self) -> i64 {
        self.0
    }

    /// Deadline of a running countdown with at least one second left.
    pub(super) fn deadline(self, remaining: impl Into<i64>) -> Cell {
        Cell::Integer(self.0.saturating_add(remaining.into()))
    }

    /// Deadline of a countdown where zero means none is running, stored as NULL.
    pub(super) fn optional_deadline(self, remaining: impl Into<i64>) -> Cell {
        match remaining.into() {
            0 => Cell::Null,
            remaining => self.deadline(remaining),
        }
    }

    /// Seconds left before `deadline`, which must lie between one and `maximum` seconds
    /// ahead of this clock.
    pub(super) fn remaining<T: TryFrom<i64>>(self, deadline: i64, maximum: i64) -> Result<T> {
        let remaining = deadline.saturating_sub(self.0);
        ensure!(
            (1..=maximum).contains(&remaining),
            "Saved deadline {deadline} is not within {maximum} seconds after simulation second {}",
            self.0
        );
        T::try_from(remaining)
            .ok()
            .context("Saved countdown does not fit its type")
    }

    /// Seconds left before an optional deadline; no deadline means zero.
    pub(super) fn optional_remaining<T: TryFrom<i64>>(
        self,
        deadline: Option<i64>,
        maximum: i64,
    ) -> Result<T> {
        match deadline {
            Some(deadline) => self.remaining(deadline, maximum),
            None => T::try_from(0)
                .ok()
                .context("Saved countdown does not fit its type"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deadlines_stay_fixed_while_counting_down() {
        let start = Clock::at(100);
        let later = Clock::at(107);
        assert_eq!(start.deadline(30), later.deadline(23));
        assert_eq!(later.remaining::<u8>(130, 30).unwrap(), 23);
    }

    #[test]
    fn optional_deadlines_encode_zero_as_absent() {
        let clock = Clock::at(5);
        assert_eq!(clock.optional_deadline(0), Cell::Null);
        assert_eq!(clock.optional_deadline(3), Cell::Integer(8));
        assert_eq!(clock.optional_remaining::<u8>(None, 30).unwrap(), 0);
    }

    #[test]
    fn deadlines_outside_their_window_are_rejected() {
        let clock = Clock::at(50);
        assert!(clock.remaining::<u8>(50, 30).is_err());
        assert!(clock.remaining::<u8>(81, 30).is_err());
        assert!(clock.remaining::<u8>(400, 1000).is_err());
    }
}
