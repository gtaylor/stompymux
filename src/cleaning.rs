//! Runtime-only database cleaning controls and monotonic maintenance deadlines.
use std::time::Duration;
use tokio::time::Instant;

/// Automatic maintenance is enabled on every server startup.
pub struct Cleaning {
    pub enabled: bool,
    deadline: Instant,
    interval: Duration,
}
impl Cleaning {
    /// Construct after serve-readiness validation has checked the configured seconds.
    pub fn new(now: Instant, interval: u64, offset: u64) -> Self {
        Self {
            enabled: true,
            deadline: now + Duration::from_secs(if offset == 0 { interval } else { offset }),
            interval: Duration::from_secs(interval),
        }
    }
    /// Change the next interval without resetting the pending deadline.
    pub fn set_interval(&mut self, seconds: u64) {
        self.interval = Duration::from_secs(seconds);
    }
    /// Consume one due attempt, regardless of its eventual success. Never catch up.
    pub fn take_due(&mut self, now: Instant) -> bool {
        if !self.enabled || now < self.deadline {
            return false;
        }
        self.deadline = now + self.interval;
        true
    }
    /// Status text exposes only implemented global controls.
    pub fn status(&self) -> String {
        format!(
            "Global parameters: cleaning...{}",
            if self.enabled { "enabled" } else { "disabled" }
        )
    }
}
impl Default for Cleaning {
    fn default() -> Self {
        Self::new(
            Instant::now(),
            crate::config::MuxConfig::default().check_interval as u64,
            crate::config::MuxConfig::default().check_offset as u64,
        )
    }
}

/// Distinguish maintenance diagnostics from replies to manual commands.
#[derive(Clone, Copy)]
pub enum CheckOrigin {
    Interactive {
        session: crate::sessions::SessionId,
        actor: crate::world::ObjectId,
        cause: crate::world::ObjectId,
    },
    Queued {
        actor: crate::world::ObjectId,
        cause: crate::world::ObjectId,
    },
    Automatic,
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn offsets_disabled_periods_and_failed_attempts_do_not_catch_up() {
        let now = Instant::now();
        let mut c = Cleaning::new(now, 600, 300);
        assert!(!c.take_due(now + Duration::from_secs(299)));
        assert!(c.take_due(now + Duration::from_secs(300)));
        assert!(!c.take_due(now + Duration::from_secs(300)));
        c.enabled = false;
        assert!(!c.take_due(now + Duration::from_secs(2000)));
        c.enabled = true;
        assert!(c.take_due(now + Duration::from_secs(2000)));
        assert!(!c.take_due(now + Duration::from_secs(2599)));
        assert!(c.take_due(now + Duration::from_secs(2600)));
        let mut changed = Cleaning::new(now, 10, 5);
        changed.set_interval(20);
        assert!(!changed.take_due(now + Duration::from_secs(4)));
        assert!(changed.take_due(now + Duration::from_secs(5)));
        assert!(!changed.take_due(now + Duration::from_secs(24)));
        assert!(changed.take_due(now + Duration::from_secs(25)));
        let mut zero = Cleaning::new(now, 10, 0);
        assert!(!zero.take_due(now + Duration::from_secs(9)));
        assert!(zero.take_due(now + Duration::from_secs(10)));
    }
}
