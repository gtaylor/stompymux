//! Chooses what starts each BattleTech heartbeat in the server loop: a one-second
//! interval in production, or explicit requests from an embedder that drives time itself.

use anyhow::Result;
use std::time::Duration;
use tokio::sync::mpsc;

/// What starts BattleTech heartbeats in [`crate::run_with_schedule_clock`].
///
/// [`serve`](crate::serve) always uses [`HeartbeatDriver::interval`]; manual mode is only
/// reachable through the embedding API, never through configuration.
#[derive(Debug)]
pub struct HeartbeatDriver {
    /// Requests from a [`HeartbeatTrigger`], or `None` to run on the one-second interval.
    requests: Option<mpsc::Receiver<()>>,
}

impl HeartbeatDriver {
    /// Run one heartbeat every second of the runtime clock.
    pub fn interval() -> Self {
        Self { requests: None }
    }

    /// Run a heartbeat only when the returned trigger asks for one. The loop never starts
    /// a heartbeat on its own, however far the runtime clock moves. Maintenance ticks,
    /// idle checks and command queue deadlines keep following the runtime clock.
    pub fn manual() -> (Self, HeartbeatTrigger) {
        let (sender, requests) = mpsc::channel(1);
        (
            Self {
                requests: Some(requests),
            },
            HeartbeatTrigger { sender },
        )
    }

    /// Arm the driver when the server loop starts; an interval's first beat is one second
    /// later.
    pub(super) fn start(self) -> HeartbeatSource {
        let Some(requests) = self.requests else {
            let mut interval = tokio::time::interval_at(
                tokio::time::Instant::now() + Duration::from_secs(1),
                Duration::from_secs(1),
            );
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            return HeartbeatSource::Interval(interval);
        };
        HeartbeatSource::Manual(requests)
    }
}

/// The armed form of a [`HeartbeatDriver`] that the server loop polls.
pub(super) enum HeartbeatSource {
    /// Today's production cadence.
    Interval(tokio::time::Interval),
    /// One heartbeat per request from a [`HeartbeatTrigger`].
    Manual(mpsc::Receiver<()>),
}

impl HeartbeatSource {
    /// Resolve when the next heartbeat should run. A manual source whose trigger was
    /// dropped never resolves again.
    pub(super) async fn due(&mut self) {
        match self {
            Self::Interval(interval) => {
                interval.tick().await;
            }
            Self::Manual(requests) => {
                if requests.recv().await.is_none() {
                    std::future::pending::<()>().await;
                }
            }
        }
    }
}

/// Requests heartbeats from a server running with [`HeartbeatDriver::manual`].
#[derive(Debug, Clone)]
pub struct HeartbeatTrigger {
    sender: mpsc::Sender<()>,
}

impl HeartbeatTrigger {
    /// Ask the server loop to run exactly one heartbeat. This returns once the request is
    /// queued, not when the heartbeat finishes; watch [`crate::Scripts::progress`] for
    /// that. Fails once the server loop has stopped.
    pub async fn fire(&self) -> Result<()> {
        self.sender
            .send(())
            .await
            .map_err(|_| anyhow::anyhow!("server loop has stopped"))
    }
}

#[cfg(test)]
mod tests {
    use super::{HeartbeatDriver, HeartbeatSource};
    use std::time::Duration;

    /// A manual source resolves once per request and never on elapsed time alone.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn manual_source_runs_only_on_request() {
        let (driver, trigger) = HeartbeatDriver::manual();
        let mut source = driver.start();
        assert!(matches!(source, HeartbeatSource::Manual(_)));
        tokio::time::advance(Duration::from_secs(5)).await;
        let idle = tokio::time::timeout(Duration::from_secs(5), source.due()).await;
        assert!(idle.is_err());
        trigger.fire().await.unwrap();
        tokio::time::timeout(Duration::from_secs(1), source.due())
            .await
            .unwrap();
        drop(trigger);
        let closed = tokio::time::timeout(Duration::from_secs(5), source.due()).await;
        assert!(closed.is_err());
    }

    /// Firing after the loop has dropped its source reports that the server stopped.
    #[tokio::test(flavor = "current_thread")]
    async fn firing_a_stopped_server_fails() {
        let (driver, trigger) = HeartbeatDriver::manual();
        drop(driver);
        assert!(trigger.fire().await.is_err());
    }
}
