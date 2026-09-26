//! Explicit deterministic access to the production heartbeat for isolated diagnostics.

use super::*;
use crate::btech::autopilot::runtime::AutopilotRuntimeMetrics;

/// Optional timings and counters for a single production heartbeat.
#[derive(Debug, Clone, Default)]
pub struct HeartbeatMetrics {
    /// True only after the ordinary world commit succeeds.
    pub committed: bool,
    /// Whole production step, including commit and effect publication.
    pub heartbeat: Duration,
    /// Validated persistence commit, including world validation and encoding.
    pub persistence: Duration,
    /// Only the movement-decision and combat-decision autopilot phases.
    pub autopilot: AutopilotRuntimeMetrics,
}

/// A server world owner without network listeners or wall-clock scheduling.
///
/// Callers must supply isolated configuration and persist their initial world
/// before stepping. Every step executes the ordinary heartbeat and database
/// transaction. This harness never disables rules, persistence or rollback.
pub struct HeartbeatHarness {
    server: Server,
}

impl HeartbeatHarness {
    /// Construct an unscheduled world owner for a supplied fixture.
    pub fn new(config: Config, world: World) -> Result<Self> {
        world.validate(&config)?;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world)))?;
        let (events, _receiver) = mpsc::channel(config.runtime.event_queue_capacity);
        Ok(Self {
            server: Server {
                config,
                scripts,
                sessions: BTreeMap::new(),
                events,
                authentication: authentication::State::new(),
                started_at: 0,
                listen_port: 0,
                shutdown: None,
                shutdown_failed: false,
                command_queue: Default::default(),
                cleaning: Default::default(),
                controls: Default::default(),
                idle_recheck: false,
                message_cache: Default::default(),
                durable: None,
                database_anchor: None,
            },
        })
    }

    /// Advance exactly one production tick. `now` supplies existing engine deadline rules.
    /// Autopilot time advances only through the committed simulation counter.
    pub async fn step(&mut self, now: i64) -> HeartbeatMetrics {
        self.step_diagnostic(now, false, false).await
    }

    /// Optional profiling and autonomous notice collection; both are off for timing acceptance.
    pub async fn step_diagnostic(
        &mut self,
        now: i64,
        detailed: bool,
        outcomes: bool,
    ) -> HeartbeatMetrics {
        self.step_pursuit(now, detailed, outcomes, false).await
    }

    /// Compare pursuit policies in an isolated harness without a production setting.
    pub async fn step_pursuit(
        &mut self,
        now: i64,
        detailed: bool,
        outcomes: bool,
        direct: bool,
    ) -> HeartbeatMetrics {
        self.step_pursuit_policy(now, detailed, outcomes, direct, Default::default())
            .await
    }

    /// Select one bounded experimental policy in isolated encounters only.
    pub async fn step_pursuit_policy(
        &mut self,
        now: i64,
        detailed: bool,
        outcomes: bool,
        direct: bool,
        policy: crate::AutopilotPursuitPolicy,
    ) -> HeartbeatMetrics {
        let started = Instant::now();
        let mut metrics = HeartbeatMetrics::default();
        metrics.autopilot.pursuit_policy = policy;
        metrics.autopilot.diagnostics_enabled = detailed;
        metrics.autopilot.direct_pursuit = direct;
        metrics.autopilot.capture_outcomes = outcomes;
        self.server
            .btech_tick_measured(now, Some(&mut metrics))
            .await;
        metrics.heartbeat = started.elapsed();
        if !metrics.committed {
            metrics.autopilot.diagnostics.pursuit.clear();
            metrics.autopilot.notice_trace.clear();
            metrics.autopilot.shots_by_unit.clear();
            metrics.autopilot.replans_by_unit.clear();
            metrics.autopilot.congestion_by_unit.clear();
            metrics.autopilot.autonomous_shots = 0;
            metrics.autopilot.prediction_ticks = 0;
            metrics.autopilot.prediction_fallbacks = 0;
            // Decisions from a rejected candidate cannot become the next tick's cached work.
            self.server
                .scripts
                .world
                .borrow_mut()
                .btech
                .autopilot_plans
                .clear();
        }
        metrics
    }

    /// Clone the current committed world for assertions or workload renewal.
    pub fn world(&self) -> World {
        self.server.scripts.world.borrow().clone()
    }

    /// Access trusted callbacks on the same world owner between ticks.
    pub fn scripts(&self) -> &Scripts {
        &self.server.scripts
    }
}
