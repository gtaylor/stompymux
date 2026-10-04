//! Counters the world owner publishes as its periodic work completes, so embedders can
//! wait on a finished heartbeat or maintenance tick instead of sleeping.

/// Running totals of periodic server work, published through a `tokio::sync::watch`
/// channel that [`crate::Scripts::progress`] subscribes to.
///
/// The world owner updates the counters after each step has finished, including the
/// database commit and the flush of its output to session queues. A receiver that sees
/// a counter rise therefore observes everything that step persisted.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RuntimeProgress {
    /// BattleTech heartbeats run, whether or not their world commit succeeded.
    pub heartbeats_attempted: u64,
    /// BattleTech heartbeats whose world commit succeeded.
    pub heartbeats_committed: u64,
    /// Maintenance ticks completed: schedule observation, automatic cleaning, idle
    /// rechecks and login timeouts.
    pub maintenance_ticks: u64,
}

impl RuntimeProgress {
    /// Count one finished heartbeat, and whether it committed.
    pub(crate) fn record_heartbeat(&mut self, committed: bool) {
        self.heartbeats_attempted += 1;
        if committed {
            self.heartbeats_committed += 1;
        }
    }

    /// Count one finished maintenance tick.
    pub(crate) fn record_maintenance(&mut self) {
        self.maintenance_ticks += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimeProgress;

    /// Every heartbeat counts as attempted; only a successful commit counts as committed.
    #[test]
    fn heartbeats_count_attempts_and_commits_separately() {
        let mut progress = RuntimeProgress::default();
        progress.record_heartbeat(true);
        progress.record_heartbeat(false);
        progress.record_maintenance();
        assert_eq!(
            progress,
            RuntimeProgress {
                heartbeats_attempted: 2,
                heartbeats_committed: 1,
                maintenance_ticks: 1,
            }
        );
    }
}
