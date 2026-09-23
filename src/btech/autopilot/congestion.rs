//! Bounded, transient clearance watches and retry accounting for exhausted searches.
use std::collections::BTreeSet;

pub(crate) const WATCH_LIMIT: usize = 64;
const POLL_SECONDS: i64 = 5;
const RETRY_SECONDS: i64 = 30;
const RETRIES: u8 = 3;
pub(crate) type Cells = BTreeSet<(u16, u16)>;

/// Retain a deterministic bounded subset regardless of edge discovery order.
pub(crate) fn remember(cells: &mut Cells, cell: (u16, u16)) {
    cells.insert(cell);
    if cells.len() > WATCH_LIMIT {
        cells.pop_last();
    }
}

/// A search starts only after quota admission; eligibility alone spends no retry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Retry {
    Early,
    Timed,
}

/// Runtime checkpoint state; deliberately not serializable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Congestion {
    pub cells: Cells,
    pub waiting: bool,
    pub timed: u8,
    pub early: u8,
    pub deadline: i64,
    pub poll_at: i64,
    pub cleared: bool,
    last_start: Option<Retry>,
}

impl Congestion {
    /// Start waiting after failure; early failures retain the existing deadline.
    pub fn failed(&mut self, now: i64) -> bool {
        if self.cells.is_empty() || self.timed >= RETRIES {
            return false;
        }
        if self.deadline == 0 || self.last_start == Some(Retry::Timed) {
            self.deadline = now.saturating_add(RETRY_SECONDS);
        }
        self.waiting = true;
        self.cleared = false;
        self.poll_at = now.saturating_add(POLL_SECONDS);
        true
    }

    pub fn should_poll(&self, now: i64) -> bool {
        self.waiting
            && now < self.deadline
            && self.early < RETRIES
            && !self.cleared
            && now >= self.poll_at
    }

    pub fn polled(&mut self, now: i64, cleared: bool) {
        self.poll_at = now.saturating_add(POLL_SECONDS);
        self.cleared |= cleared;
    }

    pub fn eligible(&self, now: i64) -> Option<Retry> {
        if !self.waiting {
            return None;
        }
        if now >= self.deadline {
            return Some(Retry::Timed);
        }
        (self.cleared && self.early < RETRIES).then_some(Retry::Early)
    }

    pub fn started(&mut self, retry: Option<Retry>) {
        match retry {
            Some(Retry::Early) => self.early += 1,
            Some(Retry::Timed) => self.timed += 1,
            None => {}
        }
        self.last_start = retry;
        self.waiting = false;
        self.cleared = false;
        self.cells.clear();
    }
}

/// Opt-in committed diagnostics, with per-unit attribution supplied by the runtime.
#[derive(Debug, Clone, Default, PartialEq, Eq, serde::Serialize)]
pub struct CongestionMetrics {
    pub clearance_checks: u64,
    pub clearances: u64,
    pub early_starts: u64,
    pub timed_starts: u64,
    pub resource_deferrals: u64,
}
impl CongestionMetrics {
    pub(crate) fn merge(&mut self, other: &Self) {
        self.clearance_checks += other.clearance_checks;
        self.clearances += other.clearances;
        self.early_starts += other.early_starts;
        self.timed_starts += other.timed_starts;
        self.resource_deferrals += other.resource_deferrals;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn watches_are_bounded_and_order_independent() {
        let mut a = Cells::new();
        let mut b = Cells::new();
        for x in 0..100 {
            remember(&mut a, (x, 0));
        }
        for x in (0..100).rev() {
            remember(&mut b, (x, 0));
        }
        assert_eq!(a, b);
        assert_eq!(a.len(), WATCH_LIMIT);
        assert_eq!(a.last(), Some(&(63, 0)));
    }
    #[test]
    fn early_retries_preserve_timed_patience_and_have_a_separate_bound() {
        let mut c = Congestion::default();
        c.cells.insert((1, 1));
        assert!(c.failed(1));
        assert_eq!(c.deadline, 31);
        for now in [6, 11, 16] {
            assert!(!c.should_poll(now - 1));
            assert!(c.should_poll(now));
            c.polled(now, true);
            assert_eq!(c.eligible(now), Some(Retry::Early));
            // Repeated eligibility/resource deferral does not spend an attempt.
            assert_eq!(c.eligible(now), Some(Retry::Early));
            c.started(c.eligible(now));
            c.cells.insert((1, 1));
            assert!(c.failed(now));
            assert_eq!(c.deadline, 31);
        }
        assert_eq!(c.early, 3);
        assert!(!c.should_poll(21));
        for now in [31, 61, 91] {
            assert_eq!(c.eligible(now), Some(Retry::Timed));
            c.started(c.eligible(now));
            c.cells.insert((1, 1));
            assert_eq!(c.failed(now), now != 91);
        }
        assert_eq!(c.timed, 3);
    }
    #[test]
    fn timed_trigger_wins_and_unchanged_poll_does_not_wake() {
        let mut c = Congestion::default();
        c.cells.insert((1, 1));
        c.failed(1);
        c.polled(6, false);
        assert_eq!(c.eligible(6), None);
        c.polled(11, true);
        assert_eq!(c.eligible(31), Some(Retry::Timed));
        c.started(c.eligible(31));
        assert_eq!(c.early, 0);
        assert_eq!(c.timed, 1);
        assert!(c.cells.is_empty());
    }
}
