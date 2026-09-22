//! Deterministic production-heartbeat traces for the autopilot integration suite.

use stompymux_rs::{Config, HeartbeatHarness, World, persistence};

/// Advance a fixed observation window through real commits, without TCP or reload polling.
pub async fn heartbeat_snapshots(config: &Config, world: &World, ticks: usize) -> Vec<World> {
    heartbeat_snapshots_until(config, world, ticks, |_| false).await
}

/// Stop at the required behavior, retaining a bounded trace for ordering assertions.
/// The bound is a simulation deadline, not a wall-clock performance assertion.
pub async fn heartbeat_snapshots_until(
    config: &Config,
    world: &World,
    max_ticks: usize,
    mut finished: impl FnMut(&World) -> bool,
) -> Vec<World> {
    assert!(max_ticks > 0);
    persistence::save(&config.database(), world).await.unwrap();
    let mut harness = HeartbeatHarness::new(config.clone(), world.clone()).unwrap();
    let mut snapshots = Vec::new();
    for _ in 0..max_ticks {
        let now = harness.world().btech.simulation_time() + 1;
        let metrics = harness.step(now).await;
        assert!(metrics.committed, "autopilot heartbeat must commit");
        let snapshot = harness.world();
        let done = finished(&snapshot);
        snapshots.push(snapshot);
        if done {
            break;
        }
    }
    // The live committed state supplies the trace. Independently verify durable
    // controller intent once at the boundary rather than reloading every tick.
    let restored = persistence::load(&config.database()).await.unwrap();
    let mut saved = serde_json::to_value(restored.btech.controllers()).unwrap();
    let mut live = serde_json::to_value(snapshots.last().unwrap().btech.controllers()).unwrap();
    // Sensor memory deliberately does not survive restart.
    for controllers in [&mut saved, &mut live] {
        for controller in controllers.as_object_mut().unwrap().values_mut() {
            controller.as_object_mut().unwrap().remove("sightings");
        }
    }
    assert_eq!(saved, live);
    snapshots
}
