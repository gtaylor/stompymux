//! Deterministic full-heartbeat load benchmark for ground autopilots.
//!
//! The benchmark uses [`crate::HeartbeatHarness`], the same server heartbeat
//! and transactional commit path used in production.  It creates a fresh game
//! root and database for every repetition, so running it cannot touch a live
//! game database.

use super::{
    AutopilotConfig, AutopilotController, AutopilotFeedbackEvent, AutopilotFireMode,
    AutopilotOrder, AutopilotState, AutopilotSubmissionMode,
};
use crate::{
    BattleMapAsset, BattlePosition, BattleUnitTemplate, Config, HeartbeatHarness, Kind, World,
    persistence,
};
use anyhow::{Context, Result, bail, ensure};
use std::path::Path;
use std::time::Duration;

const MAP_WIDTH: u16 = 100;
const MAP_HEIGHT: u16 = 100;
const DEFAULT_CONTROLLERS: usize = 100;
const CHASSIS_PER_KIND: usize = 25;

/// Battlefield geometry used by the acceptance workload.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BenchmarkScenario {
    Open,
    Obstacles,
    MovingCongestion,
    MovingPursuit,
}

impl BenchmarkScenario {
    pub const ALL: [Self; 3] = [Self::Open, Self::Obstacles, Self::MovingCongestion];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Open => "open",
            Self::Obstacles => "obstacles",
            Self::MovingCongestion => "moving_congestion",
            Self::MovingPursuit => "moving_pursuit",
        }
    }
}

/// Benchmark controls.  Defaults are 30 warmup and 600 measured ticks over
/// five repetitions.  The CLI exposes smaller values for smoke runs.
#[derive(Debug, Clone)]
pub struct BenchmarkOptions {
    pub warmup_ticks: usize,
    pub measured_ticks: usize,
    pub repetitions: usize,
    pub seed: u64,
    pub controllers: usize,
    /// Inclusive CPU attribution, excluded from acceptance timings.
    pub detailed: bool,
    /// Isolated same-executable control; never changes production configuration.
    pub direct_pursuit: bool,
    /// Explicit experimental policy for isolated comparisons.
    pub pursuit_policy: super::interception::PursuitPolicy,
    /// Optional JSONL gameplay checksums; serialization occurs outside measured phases.
    pub trace: Option<std::path::PathBuf>,
    /// None selects all battlefield geometries.
    pub scenario: Option<BenchmarkScenario>,
    /// None selects both weapons-hold and opportunistic fire.
    pub fire: Option<bool>,
}

impl Default for BenchmarkOptions {
    fn default() -> Self {
        Self {
            warmup_ticks: 30,
            measured_ticks: 600,
            repetitions: 5,
            seed: 0x5eed_1000_1000_0001,
            controllers: DEFAULT_CONTROLLERS,
            detailed: false,
            direct_pursuit: false,
            pursuit_policy: Default::default(),
            trace: None,
            scenario: None,
            fire: None,
        }
    }
}

/// One scenario/fire-mode result.  Durations are p95 values across measured
/// heartbeat ticks; counters are accumulated over all repetitions.
#[derive(Debug, Clone)]
pub struct BenchmarkResult {
    pub scenario: &'static str,
    pub opportunistic_fire: bool,
    pub repetitions: usize,
    pub warmup_ticks: usize,
    pub measured_ticks: usize,
    pub p50_heartbeat: Duration,
    pub p95_heartbeat: Duration,
    pub max_heartbeat: Duration,
    pub p50_autopilot: Duration,
    pub p95_autopilot: Duration,
    pub max_autopilot: Duration,
    pub p95_navigation: Duration,
    pub p95_observation: Duration,
    pub p95_movement: Duration,
    pub p95_combat: Duration,
    pub diagnostics: super::diagnostics::AutopilotDiagnostics,
    pub peak_trace_entries: usize,
    pub peak_trace_cells: usize,
    pub p50_persistence: Duration,
    pub p95_persistence: Duration,
    pub max_persistence: Duration,
    pub expansions: u64,
    pub max_controller_expansions: u64,
    pub peak_search_records: usize,
    pub replans: u64,
    pub completed_orders: u64,
    pub blocked_orders: u64,
    pub autonomous_shots: u64,
    pub completion_latency_ticks: Option<u64>,
    /// Minimum enabled controllers before a measured heartbeat.
    pub minimum_enabled_controllers: usize,
    /// Largest service gap, in ticks, for the bounded 100-controller workload.
    pub maximum_service_delay_ticks: usize,
    /// Fresh unit restorations after destruction or terminal blocking, outside measured phases.
    pub workload_renewals: u64,
}

#[derive(Debug, Clone)]
pub struct BenchmarkReport {
    pub results: Vec<BenchmarkResult>,
}

/// Execute all geometry × fire-mode cases using the production heartbeat.
pub async fn run(options: &BenchmarkOptions) -> Result<BenchmarkReport> {
    if options.repetitions == 0 || options.controllers == 0 || options.measured_ticks == 0 {
        bail!("benchmark repetitions, measured ticks, and controller count must be positive");
    }
    if options.controllers > DEFAULT_CONTROLLERS {
        bail!("benchmark controller count cannot exceed {DEFAULT_CONTROLLERS}");
    }
    let mut trace = options
        .trace
        .as_ref()
        .map(std::fs::File::create)
        .transpose()?
        .map(std::io::BufWriter::new);
    let mut results = Vec::new();
    for scenario in BenchmarkScenario::ALL.into_iter().chain(
        options
            .scenario
            .filter(|s| *s == BenchmarkScenario::MovingPursuit),
    ) {
        if options
            .scenario
            .is_some_and(|selected| selected != scenario)
        {
            continue;
        }
        for opportunistic_fire in [false, true] {
            if options
                .fire
                .is_some_and(|selected| selected != opportunistic_fire)
            {
                continue;
            }
            results.push(run_case(options, scenario, opportunistic_fire, &mut trace).await?);
        }
    }
    if let Some(trace) = trace.as_mut() {
        std::io::Write::flush(trace)?;
    }
    Ok(BenchmarkReport { results })
}

async fn run_case(
    options: &BenchmarkOptions,
    scenario: BenchmarkScenario,
    opportunistic_fire: bool,
    trace: &mut Option<std::io::BufWriter<std::fs::File>>,
) -> Result<BenchmarkResult> {
    let mut heartbeat_samples = Vec::new();
    let mut autopilot_samples = Vec::new();
    let mut persistence_samples = Vec::new();
    let mut navigation_samples = Vec::new();
    let mut observation_samples = Vec::new();
    let mut movement_samples = Vec::new();
    let mut combat_samples = Vec::new();
    let mut diagnostics = super::diagnostics::AutopilotDiagnostics::default();
    let mut peak_trace_entries = 0;
    let mut peak_trace_cells = 0;
    let mut expansions = 0_u64;
    let mut max_controller_expansions = 0_u64;
    let mut peak_search_records = 0_usize;
    let mut replans = 0_u64;
    let mut completed_orders = 0_u64;
    let mut blocked_orders = 0_u64;
    let mut autonomous_shots = 0_u64;
    let mut completion_latencies = Vec::new();
    let mut minimum_enabled_controllers = options.controllers;
    let mut workload_renewals = 0_u64;

    for repetition in 0..options.repetitions {
        crate::btech::los_trace::clear();
        eprintln!(
            "benchmark: {} fire={} repetition {}/{}",
            scenario.name(),
            opportunistic_fire,
            repetition + 1,
            options.repetitions
        );
        let root = copy_game_root()?;
        let config = Config::load(&root)?;
        let seed = options
            .seed
            .wrapping_add((repetition as u64).wrapping_mul(0x9e37_79b9));
        let initial_world = persistence::load(&config.database()).await?;
        let (world, map_id) = fixture_world(
            &config,
            initial_world,
            scenario,
            opportunistic_fire,
            options.controllers,
            seed,
        )?;
        let base_terrain = BattleMapAsset::parse(&map_source(scenario, seed))?.hexes;
        // The harness runs the normal persistence commit.  Initializing its
        // isolated database makes rollback and save validation identical to a
        // server tick while keeping the benchmark self-contained.
        persistence::save(&config.database(), &world).await?;
        let pristine = world.clone();
        let mut harness = HeartbeatHarness::new(config, world)?;
        let total_ticks = options.warmup_ticks.saturating_add(options.measured_ticks);
        for tick in 0..total_ticks {
            let renewed = renew_workload(&mut harness, &pristine)?;
            if tick >= options.warmup_ticks {
                workload_renewals += renewed;
            }
            let enabled = harness
                .scripts()
                .world
                .borrow()
                .btech
                .controllers()
                .values()
                .filter(|controller| controller.state() == AutopilotState::Executing)
                .count();
            ensure!(
                enabled == options.controllers,
                "benchmark must retain every enabled controller"
            );
            // Exercise transient terrain invalidation in the measured run.
            // The edit is applied between committed heartbeats and restored
            // after thirty seconds, as a scenario event would be.
            if tick == options.warmup_ticks.saturating_add(300) {
                alter_benchmark_terrain(&mut harness, map_id, true);
            } else if tick == options.warmup_ticks.saturating_add(330) {
                alter_benchmark_terrain_with(&mut harness, map_id, base_terrain.clone());
            }
            let heartbeat = harness
                .step_pursuit_policy(
                    1_000_000_i64.saturating_add(tick as i64),
                    options.detailed,
                    trace.is_some(),
                    options.direct_pursuit,
                    options.pursuit_policy,
                )
                .await;
            ensure!(
                heartbeat.autopilot.pursuit_policy == options.pursuit_policy
                    && heartbeat.autopilot.direct_pursuit == options.direct_pursuit,
                "Requested and resolved pursuit policies differ"
            );
            ensure!(
                heartbeat.committed,
                "production benchmark heartbeat failed to commit at repetition {repetition}, tick {tick}"
            );
            let (entries, cells) = crate::btech::los_trace::retained();
            ensure!(
                entries <= crate::btech::los_trace::MAX_ENTRIES
                    && cells <= crate::btech::los_trace::MAX_CELLS,
                "topology cache bounds exceeded"
            );
            peak_trace_entries = peak_trace_entries.max(entries);
            peak_trace_cells = peak_trace_cells.max(cells);
            if let Some(trace) = trace.as_mut() {
                use std::io::Write;
                let mut digest = GameplayDigest::default();
                serde_json::to_writer(&mut digest, &harness.world().btech)?;
                serde_json::to_writer(&mut digest, &heartbeat.autopilot.notice_trace)?;
                writeln!(
                    trace,
                    "{{\"scenario\":\"{}\",\"fire\":{},\"repetition\":{},\"tick\":{},\"digest\":\"{:032x}\"}}",
                    scenario.name(),
                    opportunistic_fire,
                    repetition,
                    tick,
                    digest.0
                )?;
            }
            if tick < options.warmup_ticks {
                continue;
            }
            // With at most 100 controllers and 256 expansions each, every controller
            // must be serviced every tick under the 25,600 global budget.
            ensure!(
                heartbeat.autopilot.controller_ticks == options.controllers as u64,
                "benchmark controller service starvation"
            );
            ensure!(
                heartbeat.autopilot.expansions <= 25_600,
                "benchmark global expansion limit exceeded"
            );
            minimum_enabled_controllers = minimum_enabled_controllers.min(enabled);
            if tick == options.warmup_ticks || (tick + 1) % 100 == 0 {
                eprintln!(
                    "benchmark: {} fire={} repetition {} tick {}/{}; autopilot {:.1} ms, enabled {}, renewed {}",
                    scenario.name(),
                    opportunistic_fire,
                    repetition + 1,
                    tick + 1,
                    total_ticks,
                    heartbeat.autopilot.service_duration().as_secs_f64() * 1000.0,
                    enabled,
                    workload_renewals
                );
            }
            heartbeat_samples.push(heartbeat.heartbeat);
            autopilot_samples.push(heartbeat.autopilot.service_duration());
            persistence_samples.push(heartbeat.persistence);
            navigation_samples.push(duration_from_nanos(
                heartbeat.autopilot.navigation_service_time_ns,
            ));
            observation_samples.push(duration_from_nanos(
                heartbeat.autopilot.observation_service_time_ns,
            ));
            movement_samples.push(duration_from_nanos(
                heartbeat.autopilot.movement_service_time_ns,
            ));
            combat_samples.push(duration_from_nanos(
                heartbeat.autopilot.combat_service_time_ns,
            ));
            diagnostics.merge(&heartbeat.autopilot.diagnostics);
            expansions = expansions.saturating_add(heartbeat.autopilot.expansions);
            max_controller_expansions =
                max_controller_expansions.max(heartbeat.autopilot.max_controller_expansions);
            peak_search_records = peak_search_records.max(heartbeat.autopilot.peak_search_records);
            replans = replans.saturating_add(heartbeat.autopilot.replans);
            completed_orders =
                completed_orders.saturating_add(heartbeat.autopilot.completed_orders);
            blocked_orders = blocked_orders.saturating_add(heartbeat.autopilot.blocked_orders);
            autonomous_shots =
                autonomous_shots.saturating_add(heartbeat.autopilot.autonomous_shots);
            collect_completion_latencies(&harness.world(), &mut completion_latencies);
        }
    }

    Ok(BenchmarkResult {
        scenario: scenario.name(),
        opportunistic_fire,
        repetitions: options.repetitions,
        warmup_ticks: options.warmup_ticks,
        measured_ticks: options.measured_ticks,
        p50_heartbeat: percentile_at(&mut heartbeat_samples, 50),
        p95_heartbeat: percentile(&mut heartbeat_samples),
        max_heartbeat: maximum(&mut heartbeat_samples),
        p50_autopilot: percentile_at(&mut autopilot_samples, 50),
        p95_autopilot: percentile(&mut autopilot_samples),
        max_autopilot: maximum(&mut autopilot_samples),
        p95_navigation: percentile(&mut navigation_samples),
        p95_observation: percentile(&mut observation_samples),
        p95_movement: percentile(&mut movement_samples),
        p95_combat: percentile(&mut combat_samples),
        diagnostics,
        peak_trace_entries,
        peak_trace_cells,
        p50_persistence: percentile_at(&mut persistence_samples, 50),
        p95_persistence: percentile(&mut persistence_samples),
        max_persistence: maximum(&mut persistence_samples),
        expansions,
        max_controller_expansions,
        peak_search_records,
        replans,
        completed_orders,
        blocked_orders,
        autonomous_shots,
        completion_latency_ticks: percentile_u64(&mut completion_latencies),
        minimum_enabled_controllers,
        maximum_service_delay_ticks: 1,
        workload_renewals,
    })
}

pub(super) fn copy_game_root() -> Result<tempfile::TempDir> {
    let directory = tempfile::tempdir()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    copy_tree(&repository.join("tests/fixtures/game"), directory.path())?;
    // The test fixture carries the complete Lua surface but intentionally
    // omits the large asset catalog.  Copy only the read-only asset trees
    // required by the benchmark; never copy the repository's database.
    for asset_dir in ["lua", "mechs", "maps"] {
        copy_tree(
            &repository.join("game").join(asset_dir),
            &directory.path().join(asset_dir),
        )?;
    }
    Ok(directory)
}

fn copy_tree(source: &Path, target: &Path) -> Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let destination = target.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_tree(&entry.path(), &destination)?;
        } else {
            std::fs::copy(entry.path(), destination)?;
        }
    }
    Ok(())
}

fn fixture_world(
    config: &Config,
    mut world: World,
    scenario: BenchmarkScenario,
    opportunistic_fire: bool,
    controller_count: usize,
    seed: u64,
) -> Result<(World, crate::ObjectId)> {
    let map_id = world.create(config, "autopilot-benchmark-map".into(), Kind::Room);
    crate::btech::create_map(
        &mut world,
        map_id,
        "autopilot-benchmark",
        BattleMapAsset::parse(&map_source(scenario, seed))?,
    )?;
    if let Some(map) = world.btech.maps.get_mut(&map_id) {
        map.fire_dice = Some(crate::BattleDice::seeded(seed_bytes(seed, 0)));
    }

    let count = controller_count.min(DEFAULT_CONTROLLERS);
    let templates = [
        include_str!("../../../game/mechs/JR7-D"),
        include_str!("../../../game/mechs/Demolisher"),
        include_str!("../../../game/mechs/Flatbed_Truck"),
        include_str!("../../../game/mechs/Fulcrum"),
    ];
    for index in 0..count {
        let unit_id = world.create(config, format!("autopilot-benchmark-{index}"), Kind::Thing);
        BattleUnitTemplate::parse(templates[index / CHASSIS_PER_KIND])?
            .create(&mut world, unit_id)?;
        let stream_seed = seed_bytes(seed, index as u64 + 1);
        if let Some(unit) = world.btech.constructed.get_mut(&unit_id) {
            unit.dice = crate::BattleDice::seeded(stream_seed);
            let mut recovery = serde_json::to_value(&unit.crew_recovery)?;
            recovery["dice"] = serde_json::to_value(crate::BattleDice::seeded(seed_bytes(
                seed,
                index as u64 + 2_000,
            )))?;
            unit.crew_recovery = serde_json::from_value(recovery)?;
            unit.signature.team = if index % 2 == 0 { 1 } else { 2 };
        }
        if let Some(vehicle) = world.btech.vehicles.get_mut(&unit_id) {
            vehicle.dice = crate::BattleDice::seeded(stream_seed);
            let mut recovery = serde_json::to_value(&vehicle.crew_recovery)?;
            recovery["dice"] = serde_json::to_value(crate::BattleDice::seeded(seed_bytes(
                seed,
                index as u64 + 2_000,
            )))?;
            vehicle.crew_recovery = serde_json::from_value(recovery)?;
            vehicle.signature.team = if index % 2 == 0 { 1 } else { 2 };
        }
        let row = (index / 10) as i64;
        let col = (index % 10) as i64;
        let x = if scenario == BenchmarkScenario::MovingPursuit {
            2 + (col / 2) * 18 + (col % 2) * 4
        } else {
            2 + col * 2
        };
        let y = if scenario == BenchmarkScenario::MovingPursuit {
            5 + row * 7
        } else {
            2 + row * 3
        };
        crate::btech::place_unit(&mut world, unit_id, map_id, x, y)?;
        let fire_mode = if opportunistic_fire {
            AutopilotFireMode::Opportunistic
        } else {
            AutopilotFireMode::Hold
        };
        let mut controller = AutopilotController::with_config(AutopilotConfig {
            fire_mode,
            ..AutopilotConfig::default()
        })?;
        let mut orders = Vec::with_capacity(13);
        for leg in 0..12 {
            orders.push(AutopilotOrder::Move {
                destination: BattlePosition {
                    map: map_id,
                    x: if leg % 2 == 0 {
                        (x + 25).min(i64::from(MAP_WIDTH - 1)) as u16
                    } else {
                        x as u16
                    },
                    y: y as u16,
                },
                arrival_radius: 0,
            });
        }
        orders.push(AutopilotOrder::Patrol {
            waypoints: vec![
                BattlePosition {
                    map: map_id,
                    x: x as u16,
                    y: y as u16,
                },
                BattlePosition {
                    map: map_id,
                    x: (x + 25).min(i64::from(MAP_WIDTH - 1)) as u16,
                    y: y as u16,
                },
            ],
        });
        controller.submit(orders, AutopilotSubmissionMode::Replace, None)?;
        controller.resume(None)?;
        world.btech.controllers.insert(unit_id, controller);
    }
    if scenario == BenchmarkScenario::MovingPursuit {
        let ids = world
            .btech
            .controllers()
            .keys()
            .copied()
            .collect::<Vec<_>>();
        for &id in &ids {
            if let Some(u) = world.btech.constructed.get_mut(&id) {
                u.power = crate::BattlePower::Running;
                if let Some(m) = u.motion.as_mut() {
                    m.heading = 90.0;
                    m.desired_heading = 90.0;
                }
            }
            if let Some(u) = world.btech.vehicles.get_mut(&id) {
                u.power = crate::BattlePower::Running;
                if let Some(m) = u.motion.as_mut() {
                    m.heading = 90.0;
                    m.desired_heading = 90.0;
                }
            }
        }
        // Setup-only, bounded ordinary acquisition; never inject a contact to admit an attack.
        for _ in 0..16 {
            crate::btech::refresh_contacts(&mut world, &ids)?;
            if ids.chunks_exact(2).all(|pair| {
                super::orders::validate_for_unit(
                    &world,
                    pair[0],
                    &AutopilotOrder::Attack {
                        target: pair[1],
                        range: None,
                    },
                )
                .is_ok()
            }) {
                break;
            }
        }
        for pair in ids.chunks_exact(2) {
            let order = AutopilotOrder::Attack {
                target: pair[1],
                range: Some(super::AutopilotRangeBand {
                    minimum: 2,
                    maximum: 3,
                }),
            };
            super::orders::validate_for_unit(&world, pair[0], &order).with_context(|| {
                format!("moving pursuit acquisition {:?} -> {:?}", pair[0], pair[1])
            })?;
            world.btech.controllers.get_mut(&pair[0]).unwrap().submit(
                vec![order],
                AutopilotSubmissionMode::Replace,
                None,
            )?;
        }
    }
    Ok((world, map_id))
}

/// Keep the benchmark saturated rather than silently timing dead or blocked controllers.
/// Restorations are scenario events between ticks, excluded from all reported timings.
/// Orders retain their monotonic IDs and feedback history through ordinary replacement.
fn renew_workload(harness: &mut HeartbeatHarness, pristine: &World) -> Result<u64> {
    let mut world = harness.scripts().world.borrow_mut();
    let ids: Vec<_> = world
        .btech
        .controllers()
        .iter()
        .filter_map(|(&id, controller)| {
            let unavailable = world
                .btech
                .constructed_units()
                .get(&id)
                .is_some_and(|unit| unit.is_destroyed())
                || world
                    .btech
                    .vehicles()
                    .get(&id)
                    .is_some_and(|unit| unit.is_destroyed());
            (unavailable || controller.state() != AutopilotState::Executing).then_some(id)
        })
        .collect();
    for &id in &ids {
        if let Some(unit) = pristine.btech.constructed_units().get(&id) {
            let mut unit = unit.clone();
            unit.power = crate::BattlePower::Running;
            world.btech.constructed.insert(id, unit);
        } else if let Some(unit) = pristine.btech.vehicles().get(&id) {
            let mut unit = unit.clone();
            unit.power = crate::BattlePower::Running;
            world.btech.vehicles.insert(id, unit);
        }
        let orders = pristine.btech.controllers()[&id]
            .queued_orders()
            .iter()
            .map(|record| record.order.clone())
            .collect();
        let controller = world
            .btech
            .controllers
            .get_mut(&id)
            .expect("benchmark roster persists");
        controller.submit(orders, AutopilotSubmissionMode::Replace, None)?;
        controller.resume(None)?;
        world.btech.autopilot_plans.remove(&id);
    }
    Ok(ids.len() as u64)
}

fn map_source(scenario: BenchmarkScenario, seed: u64) -> String {
    let mut rng = seed;
    let mut source = format!("{MAP_WIDTH} {MAP_HEIGHT}\n");
    for y in 0..MAP_HEIGHT {
        for x in 0..MAP_WIDTH {
            let obstacle = match scenario {
                BenchmarkScenario::Open | BenchmarkScenario::MovingPursuit => false,
                BenchmarkScenario::Obstacles => {
                    x % 11 == 0 && (y + (next_random(&mut rng) % 5) as u16) % 17 != 0
                }
                BenchmarkScenario::MovingCongestion => {
                    // Crossing lanes force units with the same destination row
                    // to yield around deterministic chokepoints while leaving
                    // a staggered opening for every route.
                    x % 17 == 0 && y % 9 != 4
                }
            };
            source.push(if obstacle { '=' } else { '.' });
            source.push('0');
        }
        source.push('\n');
    }
    source
}

fn next_random(state: &mut u64) -> u64 {
    *state ^= *state << 7;
    *state ^= *state >> 9;
    *state
}

fn seed_bytes(seed: u64, stream: u64) -> [u8; 32] {
    let mut bytes = [0_u8; 32];
    let value = seed.wrapping_add(stream.wrapping_mul(0x9e37_79b9_7f4a_7c15));
    for (index, chunk) in bytes.chunks_exact_mut(8).enumerate() {
        chunk.copy_from_slice(&value.rotate_left((index * 13) as u32).to_le_bytes());
    }
    bytes
}

fn alter_benchmark_terrain(harness: &mut HeartbeatHarness, map_id: crate::ObjectId, blocked: bool) {
    let mut world = harness.scripts().world.borrow_mut();
    if let Some(map) = world.btech.maps.get_mut(&map_id)
        && let Some(terrain) = &map.terrain
    {
        let mut terrain = (**terrain).clone();
        for y in (0..MAP_HEIGHT).step_by(3) {
            let index = usize::from(y) * usize::from(MAP_WIDTH) + 30;
            if let Some(hex) = terrain.get_mut(index) {
                hex.terrain = if blocked {
                    crate::Terrain::Wall
                } else {
                    crate::Terrain::Grassland
                };
            }
        }
        map.terrain = Some(std::sync::Arc::new(terrain));
    }
}

fn alter_benchmark_terrain_with(
    harness: &mut HeartbeatHarness,
    map_id: crate::ObjectId,
    terrain: std::sync::Arc<Vec<crate::BattleHex>>,
) {
    let mut world = harness.scripts().world.borrow_mut();
    if let Some(map) = world.btech.maps.get_mut(&map_id) {
        map.terrain = Some(terrain);
    }
}

fn collect_completion_latencies(world: &World, output: &mut Vec<u64>) {
    let simulation_time = world.btech.simulation_time();
    for controller in world.btech.controllers().values() {
        for feedback in controller.feedback_records() {
            if feedback.event == AutopilotFeedbackEvent::OrderSucceeded
                && feedback.simulation_time == simulation_time
            {
                if let Some(order_id) = feedback.order_id
                    && let Some(started) = controller.feedback_records().iter().find(|candidate| {
                        candidate.order_id == Some(order_id)
                            && candidate.event == AutopilotFeedbackEvent::OrderStarted
                    })
                {
                    output.push(
                        feedback
                            .simulation_time
                            .saturating_sub(started.simulation_time)
                            .max(0) as u64,
                    );
                }
            }
        }
    }
}

fn percentile(samples: &mut [Duration]) -> Duration {
    percentile_at(samples, 95)
}

fn percentile_at(samples: &mut [Duration], percentile: usize) -> Duration {
    if samples.is_empty() {
        return Duration::ZERO;
    }
    samples.sort_unstable();
    samples[(samples.len() * percentile / 100).min(samples.len() - 1)]
}

fn maximum(samples: &mut [Duration]) -> Duration {
    samples.iter().copied().max().unwrap_or(Duration::ZERO)
}

fn percentile_u64(samples: &mut [u64]) -> Option<u64> {
    if samples.is_empty() {
        return None;
    }
    samples.sort_unstable();
    Some(samples[(samples.len() * 95 / 100).min(samples.len() - 1)])
}

fn duration_from_nanos(nanos: u128) -> Duration {
    Duration::from_nanos(nanos.min(u128::from(u64::MAX)) as u64)
}

/// Stable FNV-1a/128 checksum of deterministic serialized gameplay state, not a security hash.
struct GameplayDigest(u128);
impl Default for GameplayDigest {
    fn default() -> Self {
        Self(0x6c62272e07bb014262b821756295c58d)
    }
}
impl std::io::Write for GameplayDigest {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        for &byte in bytes {
            self.0 = (self.0 ^ u128::from(byte)).wrapping_mul(0x1000000000000000000013b);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod persistence_tests {
    use super::*;

    /// Heartbeats save against the in-memory baseline instead of rereading the database.
    /// After firing, damage, a terrain edit and object moves and creation between ticks,
    /// a full comparison with the stored world must find nothing left to write, and the
    /// stored containment lists must match the live world.
    #[tokio::test]
    async fn baseline_saves_leave_nothing_for_a_full_comparison() {
        let root = copy_game_root().unwrap();
        let config = Config::load(&root).unwrap();
        let initial = persistence::load(&config.database()).await.unwrap();
        let (mut world, map_id) =
            fixture_world(&config, initial, BenchmarkScenario::Open, true, 30, 7).unwrap();
        let east = world.create(&config, "East".into(), Kind::Room);
        let west = world.create(&config, "West".into(), Kind::Room);
        let cargo: Vec<_> = ["Crate", "Barrel", "Drum"]
            .map(|name| world.create(&config, name.into(), Kind::Thing))
            .into();
        for id in &cargo {
            world.objects.get_mut(id).unwrap().location = Some(east);
        }
        let relocate = |harness: &HeartbeatHarness, id, room| {
            harness
                .scripts()
                .world_mut()
                .objects
                .get_mut(&id)
                .unwrap()
                .location = Some(room);
        };
        persistence::save(&config.database(), &world).await.unwrap();
        let mut harness = HeartbeatHarness::new(config.clone(), world).unwrap();
        let mut shots = 0;
        for tick in 0..60 {
            match tick {
                10 => relocate(&harness, cargo[1], west),
                20 => alter_benchmark_terrain(&mut harness, map_id, true),
                30 => {
                    let late = harness.scripts().world_mut().create(
                        &config,
                        "Late arrival".into(),
                        Kind::Thing,
                    );
                    relocate(&harness, late, east);
                    relocate(&harness, cargo[0], west);
                }
                40 => relocate(&harness, cargo[1], east),
                _ => {}
            }
            let heartbeat = harness.step(1_000_000 + tick).await;
            assert!(heartbeat.committed);
            shots += heartbeat.autopilot.autonomous_shots;
        }
        assert!(shots > 0, "the workload must save combat damage");
        let live = harness.world();
        let written = persistence::save_changes(
            &config.database(),
            None,
            &live,
            config.database.busy_timeout_ms,
            1,
        )
        .await
        .unwrap();
        assert!(!written, "baseline saves left rows unwritten");
        persistence::validate_lists(&config.database(), &live, config.database.busy_timeout_ms)
            .await
            .unwrap();
        // Every unit and vehicle splits into saved parts that merge back unchanged.
        fn round_trip<T>(record: &T) -> T
        where
            T: crate::btech::saved_parts::SavedParts + serde::de::DeserializeOwned,
        {
            let core = record.saved_core_value().unwrap().to_string();
            let live = record.saved_live_value().unwrap().to_string();
            serde_json::from_value(crate::btech::saved_parts::merge(&core, &live).unwrap()).unwrap()
        }
        for unit in live.btech.constructed_units().values() {
            assert_eq!(&round_trip(unit), unit);
        }
        for vehicle in live.btech.vehicles().values() {
            assert_eq!(&round_trip(vehicle), vehicle);
        }
        let (unit, vehicle) = (
            live.btech.constructed_units().values().next().unwrap(),
            live.btech.vehicles().values().next().unwrap(),
        );
        crate::btech::saved_parts::assert_defaulted_fields_load(unit);
        crate::btech::saved_parts::assert_defaulted_fields_load(vehicle);
        let stored = persistence::load(&config.database()).await.unwrap();
        // Autopilot sensor memory deliberately does not survive a restart.
        let [stored, live] = [&stored, &live].map(|world| {
            let mut state = serde_json::to_value(&world.btech).unwrap();
            for controller in state["controllers"].as_object_mut().unwrap().values_mut() {
                controller.as_object_mut().unwrap().remove("sightings");
            }
            state
        });
        assert_eq!(stored, live);
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;

    #[tokio::test]
    async fn observed_and_navigation_geometry_share_controller_budget() {
        // One tick reproduces the crowded firing-region search that previously
        // spent an initial observation check plus sixteen navigation checks.
        let report = run(&BenchmarkOptions {
            warmup_ticks: 0,
            measured_ticks: 1,
            repetitions: 1,
            controllers: 100,
            scenario: Some(BenchmarkScenario::MovingPursuit),
            fire: Some(false),
            pursuit_policy: super::super::interception::PursuitPolicy::Adaptive,
            ..Default::default()
        })
        .await
        .unwrap();
        assert_eq!(report.results.len(), 1);
        assert_eq!(report.results[0].minimum_enabled_controllers, 100);
        assert!(report.results[0].max_controller_expansions <= 256);
    }
}
