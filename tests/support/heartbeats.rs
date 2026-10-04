//! Drive the embedded server's heartbeat and maintenance tick on the paused tokio clock,
//! waiting on the progress the server publishes instead of on real time.

use std::time::Duration;
use stompymux_rs::{Config, RuntimeProgress, World, persistence};
use tokio::sync::watch;

/// Real time any single wait may take before the test fails instead of hanging.
const STEP_LIMIT: Duration = Duration::from_secs(30);

/// Heartbeats in a row that may be refused before [`Heartbeats::commit`] gives up. A
/// refusal can only come from a commit that started before the test lifted its fault.
const COMMIT_ATTEMPTS: usize = 3;

/// Handle on one embedded server's published progress, returned by [`crate::start`].
pub struct Heartbeats {
    progress: watch::Receiver<RuntimeProgress>,
    maintenance_interval: Duration,
}

impl Heartbeats {
    /// Watch `progress` from a server running with `config`.
    pub fn new(progress: watch::Receiver<RuntimeProgress>, config: &Config) -> Self {
        Self {
            progress,
            maintenance_interval: Duration::from_millis(config.runtime.maintenance_interval_ms),
        }
    }

    /// Wait until the server loop has started, so its heartbeat interval is armed and a
    /// clock advance fires it. A server that fails to start closes the channel instead.
    pub async fn ready(&mut self) {
        let started = self
            .progress
            .wait_for(|progress| progress.maintenance_ticks > 0);
        let _ = tokio::time::timeout(STEP_LIMIT, started)
            .await
            .expect("server loop did not start");
    }

    /// Fire one heartbeat by advancing the clock a second, then wait until the server has
    /// finished that heartbeat, whether or not its commit succeeded. A heartbeat already
    /// running when the clock moved does not count. Tests that refuse commits use this.
    pub async fn attempt(&mut self) -> RuntimeProgress {
        let fired = advance(Duration::from_secs(1)).await;
        self.wait("heartbeat", |progress| {
            progress
                .heartbeat_started
                .is_some_and(|started| started >= fired)
        })
        .await
    }

    /// Fire heartbeats until one commits, so the database holds that heartbeat's state.
    pub async fn commit(&mut self) {
        let before = self.progress.borrow().heartbeats_committed;
        for _ in 0..COMMIT_ATTEMPTS {
            if self.attempt().await.heartbeats_committed > before {
                return;
            }
        }
        panic!("no heartbeat committed in {COMMIT_ATTEMPTS} attempts");
    }

    /// Run `count` committed heartbeats.
    pub async fn commit_n(&mut self, count: usize) {
        for _ in 0..count {
            self.commit().await;
        }
    }

    /// Load the saved world until `done` accepts it, committing one heartbeat between
    /// loads, and return the accepted world. Fails after `heartbeats` heartbeats.
    pub async fn until_saved(
        &mut self,
        config: &Config,
        heartbeats: usize,
        mut done: impl FnMut(&World) -> bool,
    ) -> World {
        for _ in 0..heartbeats {
            let saved = persistence::load(&config.database()).await.unwrap();
            if done(&saved) {
                return saved;
            }
            self.commit().await;
        }
        let saved = persistence::load(&config.database()).await.unwrap();
        assert!(
            done(&saved),
            "saved world not reached after {heartbeats} heartbeats"
        );
        saved
    }

    /// Run two whole maintenance ticks, so schedule observation, automatic cleaning and
    /// login timeouts have all seen the current schedule and cleaning clocks.
    pub async fn maintenance(&mut self) {
        for _ in 0..2 {
            let before = self.progress.borrow().maintenance_ticks;
            advance(self.maintenance_interval).await;
            self.wait("maintenance tick", |progress| {
                progress.maintenance_ticks > before
            })
            .await;
        }
    }

    /// Wait in real time, bounded by [`STEP_LIMIT`], until `reached` holds.
    async fn wait(
        &mut self,
        what: &str,
        reached: impl FnMut(&RuntimeProgress) -> bool,
    ) -> RuntimeProgress {
        match tokio::time::timeout(STEP_LIMIT, self.progress.wait_for(reached)).await {
            Ok(Ok(progress)) => *progress,
            Ok(Err(_)) => panic!("server stopped before its next {what}"),
            Err(_) => panic!("no {what} within {STEP_LIMIT:?}"),
        }
    }
}

/// Move the runtime clock forward without waiting out real time, returning the advanced
/// instant. The clock resumes afterwards so SQLite work on other threads never lets
/// paused timers run ahead.
async fn advance(by: Duration) -> tokio::time::Instant {
    tokio::time::pause();
    tokio::time::advance(by).await;
    let advanced = tokio::time::Instant::now();
    tokio::time::resume();
    advanced
}
