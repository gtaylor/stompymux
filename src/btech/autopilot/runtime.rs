//! Runtime execution for attached ground autopilots.
//!
//! The controller in [`super`] stores only durable intent.  This module keeps
//! path searches and steering cursors in a runtime-only map on `BtechState` and
//! turns those decisions into the ordinary movement and targeting adapters.
//! Every operation is performed inside the heartbeat's candidate world, so a
//! failed commit restores both the unit and the transient plan.

pub use super::congestion::CongestionMetrics;
use super::congestion::{Congestion, Retry};
use super::{
    AutopilotConfig, AutopilotOrder, AutopilotOrderRecord, AutopilotReason, AutopilotState,
};
use crate::btech::autopilot::combat_policy::choose_target;
use crate::btech::autopilot::navigation::{AStarSearch, Goal, GridHex, SearchStatus};
use crate::btech::autopilot::observations::{self, AutopilotObservation};
use crate::btech::autopilot::traversal;
use crate::btech::{HexCoordinate, Notice, Position, Power};
use crate::{Config, ObjectId, World};
use anyhow::Result;
use std::sync::Arc;
use std::time::Instant;

pub(crate) const EXPANSIONS_PER_CONTROLLER: usize = 256;
pub(crate) const GLOBAL_EXPANSION_BUDGET: usize = 25_600;
const MAX_SEARCH_RECORDS: usize = 1_000_000;
const SIGHTING_MEMORY_SECONDS: i64 = 30;
const STAGNANT_TICKS_BEFORE_REPLAN: u16 = 10;
const MAX_RECOVERY_ATTEMPTS: u8 = 3;

/// Optional counters collected by the benchmark harness. The normal heartbeat
/// uses [`advance`] and keeps this instrumentation entirely disabled.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AutopilotRuntimeMetrics {
    /// Enable detailed synchronous CPU attribution (benchmark only).
    pub diagnostics_enabled: bool,
    /// Isolated harness comparison input; never set by the server.
    pub(crate) direct_pursuit: bool,
    pub(crate) pursuit_policy: super::interception::PursuitPolicy,
    pub diagnostics: super::diagnostics::AutopilotDiagnostics,
    /// Capture autonomous notices for deterministic comparisons, outside timing samples.
    pub capture_outcomes: bool,
    pub notice_trace: Vec<(ObjectId, String)>,
    pub controller_ticks: u64,
    pub service_time_ns: u128,
    pub movement_service_time_ns: u128,
    pub combat_service_time_ns: u128,
    pub navigation_service_time_ns: u128,
    pub observation_service_time_ns: u128,
    pub expansions: u64,
    pub max_controller_expansions: u64,
    pub peak_search_records: usize,
    pub replans: u64,
    pub completed_orders: u64,
    pub blocked_orders: u64,
    pub autonomous_shots: u64,
    /// Optional committed-shot attribution, collected only when outcomes are requested.
    pub shots_by_unit: std::collections::BTreeMap<ObjectId, u64>,
    /// Per-unit search restarts, including attempts that find no route.
    pub replans_by_unit: std::collections::BTreeMap<ObjectId, u64>,
    pub geometry_checks: u64,
    /// Committed ticks selecting a predicted navigation aim (opt-in outcomes only).
    pub prediction_ticks: u64,
    /// Predicted regions exhausted before retrying the observed target.
    pub prediction_fallbacks: u64,
    pub congestion_by_unit: std::collections::BTreeMap<ObjectId, CongestionMetrics>,
}

impl AutopilotRuntimeMetrics {
    /// Record an admitted shot without allocating per-unit data during ordinary timing runs.
    fn record_shot(&mut self, shooter: ObjectId) {
        self.autonomous_shots = self.autonomous_shots.saturating_add(1);
        if self.capture_outcomes {
            *self.shots_by_unit.entry(shooter).or_default() += 1;
        }
    }

    pub fn merge(&mut self, other: &Self) {
        self.notice_trace.extend(other.notice_trace.iter().cloned());
        for (&id, &count) in &other.replans_by_unit {
            *self.replans_by_unit.entry(id).or_default() += count;
        }
        for (&id, &shots) in &other.shots_by_unit {
            *self.shots_by_unit.entry(id).or_default() += shots;
        }
        for (&id, value) in &other.congestion_by_unit {
            self.congestion_by_unit.entry(id).or_default().merge(value);
        }
        self.diagnostics.merge(&other.diagnostics);
        self.controller_ticks = self.controller_ticks.saturating_add(other.controller_ticks);
        self.service_time_ns = self.service_time_ns.saturating_add(other.service_time_ns);
        self.movement_service_time_ns = self
            .movement_service_time_ns
            .saturating_add(other.movement_service_time_ns);
        self.combat_service_time_ns = self
            .combat_service_time_ns
            .saturating_add(other.combat_service_time_ns);
        self.navigation_service_time_ns = self
            .navigation_service_time_ns
            .saturating_add(other.navigation_service_time_ns);
        self.observation_service_time_ns = self
            .observation_service_time_ns
            .saturating_add(other.observation_service_time_ns);
        self.expansions = self.expansions.saturating_add(other.expansions);
        self.max_controller_expansions = self
            .max_controller_expansions
            .max(other.max_controller_expansions);
        self.peak_search_records = self.peak_search_records.max(other.peak_search_records);
        self.replans = self.replans.saturating_add(other.replans);
        self.completed_orders = self.completed_orders.saturating_add(other.completed_orders);
        self.blocked_orders = self.blocked_orders.saturating_add(other.blocked_orders);
        self.autonomous_shots = self.autonomous_shots.saturating_add(other.autonomous_shots);
        self.geometry_checks += other.geometry_checks;
        self.prediction_ticks += other.prediction_ticks;
        self.prediction_fallbacks += other.prediction_fallbacks;
    }

    /// Wall-clock duration spent in movement and combat decision phases.
    pub fn service_duration(&self) -> std::time::Duration {
        std::time::Duration::from_nanos(self.service_time_ns.min(u128::from(u64::MAX)) as u64)
    }
}

/// Runtime-only execution state.  It is intentionally excluded from the
/// serialized controller and rebuilt after a restart from the active order.
#[derive(Debug, Clone, Default)]
pub(crate) struct AutopilotPlan {
    pub(crate) engagement: Option<super::engagement::Engagement>,
    pub(crate) pursuit: super::interception::Pursuit,
    pub(crate) fallback: bool,
    pub(crate) congestion: Congestion,
    pub(crate) steering: super::steering::SteeringState,
    pub(crate) order_id: u64,
    pub(crate) goal: Option<(Position, u16)>,
    pub(crate) search: Option<AStarSearch>,
    pub(crate) replacement_pending: bool,
    pub(crate) route: Vec<GridHex>,
    pub(crate) route_index: usize,
    pub(crate) last_hex: Option<GridHex>,
    pub(crate) best_waypoint_distance: Option<f64>,
    pub(crate) stagnant_ticks: u16,
    pub(crate) recovery_attempts: u8,
    pub(crate) steering_grace: u16,
    /// One bounded courtesy wait per occupied waypoint, retained across replans.
    pub(crate) yielding: Option<(GridHex, i64)>,
    pub(crate) terrain_revision: usize,
    pub(crate) mobility_revision: u64,
}

// AStarSearch contains a binary heap and is deliberately not comparable.  The
// transient plan participates in BtechState's checkpoint equality only through
// its durable cursor; equality of the search frontier has no game meaning.
impl PartialEq for AutopilotPlan {
    fn eq(&self, other: &Self) -> bool {
        self.pursuit == other.pursuit
            && self.replacement_pending == other.replacement_pending
            && self.congestion == other.congestion
            && self.engagement == other.engagement
            && self.fallback == other.fallback
            && self.steering == other.steering
            && self.order_id == other.order_id
            && self.goal == other.goal
            && self.route == other.route
            && self.route_index == other.route_index
            && self.last_hex == other.last_hex
            && self.best_waypoint_distance == other.best_waypoint_distance
            && self.stagnant_ticks == other.stagnant_ticks
            && self.recovery_attempts == other.recovery_attempts
            && self.steering_grace == other.steering_grace
            && self.yielding == other.yielding
            && self.terrain_revision == other.terrain_revision
            && self.mobility_revision == other.mobility_revision
    }
}

/// Whether at least one controller can make progress on the next heartbeat.
pub(crate) fn pending(world: &World) -> bool {
    world.btech.controllers().values().any(|controller| {
        controller.state() == AutopilotState::Executing
            || (controller.state() == AutopilotState::Idle
                && controller.config().fire_mode != super::AutopilotFireMode::Hold)
    })
}

/// Advance all enabled controllers in stable ID order.  Search work is fairly
/// bounded by the global expansion budget; a pending search resumes next tick.
pub(crate) fn advance(
    world: &mut World,
    config: &Config,
    simulation_time: i64,
) -> Result<Vec<Notice>> {
    advance_inner(world, config, simulation_time, None)
}

/// Advance movement while collecting optional benchmark counters.
pub(crate) fn advance_with_metrics(
    world: &mut World,
    config: &Config,
    simulation_time: i64,
    metrics: &mut AutopilotRuntimeMetrics,
) -> Result<Vec<Notice>> {
    advance_inner(world, config, simulation_time, Some(metrics))
}

fn advance_inner(
    world: &mut World,
    config: &Config,
    simulation_time: i64,
    metrics: Option<&mut AutopilotRuntimeMetrics>,
) -> Result<Vec<Notice>> {
    advance_budgeted(
        world,
        config,
        simulation_time,
        metrics,
        GLOBAL_EXPANSION_BUDGET,
        MAX_SEARCH_RECORDS,
    )
}

/// Shared scheduler with explicit hard limits, also used by deterministic pressure tests.
fn advance_budgeted(
    world: &mut World,
    config: &Config,
    simulation_time: i64,
    metrics: Option<&mut AutopilotRuntimeMetrics>,
    expansion_budget: usize,
    record_limit: usize,
) -> Result<Vec<Notice>> {
    let diagnostic_scope =
        super::diagnostics::Scope::begin(metrics.as_ref().is_some_and(|m| m.diagnostics_enabled));
    let started = metrics.as_ref().map(|_| Instant::now());
    let mut ids: Vec<_> = world.btech.controllers().keys().copied().collect();
    if !ids.is_empty() {
        let offset = simulation_time.unsigned_abs() as usize % ids.len();
        ids.rotate_left(offset);
    }
    let mut notices = Vec::new();
    let mut budget = expansion_budget;
    let mut geometry_budget = 1600usize;
    let mut metrics = metrics;
    for id in ids {
        if budget == 0 {
            break;
        }
        let before_feedback = metrics.as_ref().and_then(|_| {
            world
                .btech
                .controllers()
                .get(&id)
                .and_then(|controller| controller.feedback_records().last().map(|f| f.sequence))
        });
        let geometry_before = geometry_budget;
        let expansions = advance_controller(
            world,
            config,
            id,
            simulation_time,
            budget,
            record_limit,
            &mut geometry_budget,
            &mut notices,
            &mut metrics,
        )?;
        assert!(
            geometry_before - geometry_budget <= 16,
            "autopilot geometry budget exceeded"
        );
        assert!(
            expansions <= EXPANSIONS_PER_CONTROLLER,
            "autopilot controller expansion budget exceeded"
        );
        assert!(
            expansions <= budget,
            "autopilot global expansion budget exceeded"
        );
        budget -= expansions;
        if let Some(metrics) = metrics.as_deref_mut() {
            metrics.controller_ticks = metrics.controller_ticks.saturating_add(1);
            metrics.expansions = metrics
                .expansions
                .saturating_add(u64::try_from(expansions).unwrap_or(u64::MAX));
            metrics.max_controller_expansions = metrics
                .max_controller_expansions
                .max(u64::try_from(expansions).unwrap_or(u64::MAX));
            metrics.peak_search_records =
                metrics.peak_search_records.max(total_search_records(world));
            assert!(
                total_search_records(world) <= record_limit,
                "autopilot global search-record cap exceeded"
            );
            if let Some(controller) = world.btech.controllers().get(&id) {
                for feedback in controller.feedback_since(before_feedback).records {
                    match feedback.event {
                        super::AutopilotFeedbackEvent::OrderSucceeded => {
                            metrics.completed_orders = metrics.completed_orders.saturating_add(1)
                        }
                        super::AutopilotFeedbackEvent::Blocked => {
                            metrics.blocked_orders = metrics.blocked_orders.saturating_add(1)
                        }
                        _ => {}
                    }
                }
            }
        }
    }
    if let (Some(metrics), Some(started)) = (metrics.as_deref_mut(), started) {
        let elapsed = started.elapsed().as_nanos();
        metrics.service_time_ns = metrics.service_time_ns.saturating_add(elapsed);
        metrics.movement_service_time_ns = metrics.movement_service_time_ns.saturating_add(elapsed);
    }
    if let Some(metrics) = metrics {
        diagnostic_scope.finish(&mut metrics.diagnostics);
        if metrics.capture_outcomes {
            metrics
                .notice_trace
                .extend(notices.iter().map(|n| (n.unit, n.text.clone())));
        }
    }
    Ok(notices)
}

/// Resolve autonomous direct fire after movement, sensor refresh, recycle and
/// target-lock advancement.  Keeping this phase separate means path steering
/// never consumes weapon dice before the ordinary visibility update.
pub(crate) fn advance_combat(
    world: &mut World,
    config: &Config,
    simulation_time: i64,
) -> Result<Vec<Notice>> {
    advance_combat_inner(world, config, simulation_time, None)
}

/// Advance autonomous direct fire while collecting optional benchmark counters.
pub(crate) fn advance_combat_with_metrics(
    world: &mut World,
    config: &Config,
    simulation_time: i64,
    metrics: &mut AutopilotRuntimeMetrics,
) -> Result<Vec<Notice>> {
    advance_combat_inner(world, config, simulation_time, Some(metrics))
}

fn advance_combat_inner(
    world: &mut World,
    config: &Config,
    simulation_time: i64,
    mut metrics: Option<&mut AutopilotRuntimeMetrics>,
) -> Result<Vec<Notice>> {
    let diagnostic_scope =
        super::diagnostics::Scope::begin(metrics.as_ref().is_some_and(|m| m.diagnostics_enabled));
    let started = metrics.as_ref().map(|_| Instant::now());
    let equipment_scope = crate::btech::equipment_context::Scope::begin(&world.btech);
    let validation_scope = crate::btech::validation_context::Scope::begin(&world.btech);
    let ids: Vec<_> = world.btech.controllers().keys().copied().collect();
    let mut notices = Vec::new();
    for id in ids {
        let Some(controller) = world.btech.controllers().get(&id) else {
            continue;
        };
        let state = controller.state();
        let config_for_unit = controller.config().clone();
        let sightings = controller.sightings().clone();
        let active_order = controller.active_order().map(|record| record.order.clone());
        if state != AutopilotState::Executing
            && !(state == AutopilotState::Idle
                && config_for_unit.fire_mode != super::AutopilotFireMode::Hold)
        {
            continue;
        }
        let observation_started = metrics.as_ref().map(|_| Instant::now());
        let Some(observation) =
            observations::observe_with_memory(world, id, simulation_time, &sightings).ok()
        else {
            if let (Some(metrics), Some(started)) = (metrics.as_deref_mut(), observation_started) {
                metrics.observation_service_time_ns = metrics
                    .observation_service_time_ns
                    .saturating_add(started.elapsed().as_nanos());
            }
            continue;
        };
        if let (Some(metrics), Some(started)) = (metrics.as_deref_mut(), observation_started) {
            metrics.observation_service_time_ns = metrics
                .observation_service_time_ns
                .saturating_add(started.elapsed().as_nanos());
        }
        update_memory(world, id, simulation_time, &observation);
        let target = match active_order.as_ref() {
            Some(AutopilotOrder::Attack { target, .. }) => {
                choose_target(&observation, Some(*target), None)
            }
            Some(_) | None => {
                let selected = crate::btech::scanner::scanner_unit(world, id)
                    .and_then(|scanner| scanner.selected);
                combat_target(&config_for_unit, &observation, selected)
            }
        };
        if let Some(target) = target {
            let contact = observation.contacts.iter().find(|c| c.unit == target);
            // A network-relayed target is faced in advance but cannot be locked or fired on.
            let relayed = contact.is_some_and(|contact| contact.relayed);
            if let Some(contact) = contact {
                let stationary = super::steering::motion(world, id)
                    .is_some_and(|m| m.speed.abs() < 0.1 && m.desired_speed.abs() < 0.1);
                let navigating = world
                    .btech
                    .autopilot_plans
                    .get(&id)
                    .is_some_and(|p| p.search.is_some() || p.route_index < p.route.len());
                let allow_hull = stationary
                    && !navigating
                    && matches!(
                        active_order,
                        Some(
                            AutopilotOrder::Attack { .. }
                                | AutopilotOrder::AttackMove { .. }
                                | AutopilotOrder::Hold
                        ) | None
                    );
                super::alignment::align(
                    world,
                    id,
                    contact.position,
                    &observation,
                    allow_hull,
                    &mut notices,
                );
            }
            // Weapons hold forbids expenditure, not aiming an explicitly assigned target.
            if config_for_unit.fire_mode != super::AutopilotFireMode::Hold && !relayed {
                fire_target_if_ready(
                    world,
                    config,
                    id,
                    target,
                    &config_for_unit,
                    &mut notices,
                    &mut metrics,
                );
            }
        }
    }
    drop(validation_scope);
    drop(equipment_scope);
    if let (Some(metrics), Some(started)) = (metrics.as_deref_mut(), started) {
        let elapsed = started.elapsed().as_nanos();
        metrics.service_time_ns = metrics.service_time_ns.saturating_add(elapsed);
        metrics.combat_service_time_ns = metrics.combat_service_time_ns.saturating_add(elapsed);
    }
    if let Some(metrics) = metrics {
        diagnostic_scope.finish(&mut metrics.diagnostics);
        if metrics.capture_outcomes {
            metrics
                .notice_trace
                .extend(notices.iter().map(|n| (n.unit, n.text.clone())));
        }
    }
    Ok(notices)
}

fn advance_controller(
    world: &mut World,
    config: &Config,
    id: ObjectId,
    simulation_time: i64,
    budget: usize,
    record_limit: usize,
    geometry_budget: &mut usize,
    notices: &mut Vec<Notice>,
    metrics: &mut Option<&mut AutopilotRuntimeMetrics>,
) -> Result<usize> {
    let Some(controller) = world.btech.controllers().get(&id) else {
        return Ok(0);
    };
    let idle_fire = controller.state() == AutopilotState::Idle
        && controller.config().fire_mode != super::AutopilotFireMode::Hold;
    if controller.state() != AutopilotState::Executing && !idle_fire {
        return Ok(0);
    }

    // Queue transitions happen at the beginning of a simulation tick.  The
    // controller API deliberately leaves this internal transition private.
    if controller.state() == AutopilotState::Executing && controller.active_order().is_none() {
        world
            .btech
            .controllers
            .get_mut(&id)
            .and_then(|controller| controller.start_next(simulation_time));
    }
    let Some(controller) = world.btech.controllers().get(&id) else {
        return Ok(0);
    };
    let controller_config = controller.config().clone();

    let Some(active) = controller.active_order().cloned() else {
        return Ok(0);
    };

    if !supported_ground_unit(world, id) {
        fail(world, id, simulation_time, AutopilotReason::Unsupported);
        return Ok(0);
    }

    // Startup is the only automatic lifecycle recovery.  Motion and weapon
    // adapters continue to enforce all ordinary power and damage checks.
    if unit_power(world, id) == Some(Power::Off) {
        match crate::btech::power::start_unit_autopilot(world, id, false) {
            Ok(notice) => notices.push(notice),
            Err(_) => fail(world, id, simulation_time, AutopilotReason::UnitUnavailable),
        }
        return Ok(0);
    }
    if unit_power(world, id) != Some(Power::Running) {
        return Ok(0);
    }

    // A successful stand changes posture immediately; motion remains inadmissible
    // until its rising timer ends. Waiting must not fail an otherwise valid order.
    if world
        .btech
        .constructed_units()
        .get(&id)
        .is_some_and(|unit| unit.stand_timer().is_some())
    {
        return Ok(0);
    }

    // A prone Mech must use the ordinary stand transaction before movement.
    // The stand timer throttles retries; the durable recovery cursor bounds
    // repeated failed checks instead of retrying forever each heartbeat.
    if let Some(unit) = world.btech.constructed_units().get(&id)
        && unit.posture() == crate::btech::Posture::Prone
    {
        if unit.stand_timer().is_none()
            && active.progress.recovery_attempts >= MAX_RECOVERY_ATTEMPTS
        {
            fail(world, id, simulation_time, AutopilotReason::Stuck);
            return Ok(0);
        }
        let attempt = match crate::btech::stand::begin_stand_autopilot(
            world,
            id,
            crate::btech::StandMode::Normal,
            false,
            crate::btech::FallRules::configured(config),
        ) {
            Ok(attempt) => attempt,
            Err(_) => {
                fail(world, id, simulation_time, AutopilotReason::UnitUnavailable);
                return Ok(0);
            }
        };
        notices.extend(attempt.notices);
        let recovery_attempts = if attempt.check.success {
            0
        } else {
            active.progress.recovery_attempts.saturating_add(1)
        };
        if let Some(controller) = world.btech.controllers.get_mut(&id)
            && let Some(order) = controller.active_order_mut()
        {
            order.progress.recovery_attempts = recovery_attempts;
        }
        return Ok(0);
    }

    // Only pursuing orders need contacts before movement. Every enabled controller
    // receives a fresh observation after movement/scanning in the combat phase,
    // including weapons-hold controllers, so memory remains current.
    let observation = if matches!(
        active.order,
        AutopilotOrder::Attack { .. } | AutopilotOrder::AttackMove { .. }
    ) {
        let started = metrics.as_ref().map(|_| Instant::now());
        let observation = match observations::observe_with_memory(
            world,
            id,
            simulation_time,
            controller.sightings(),
        ) {
            Ok(observation) => observation,
            Err(_) => {
                fail(world, id, simulation_time, AutopilotReason::UnitUnavailable);
                return Ok(0);
            }
        };
        if let (Some(metrics), Some(started)) = (metrics.as_deref_mut(), started) {
            metrics.observation_service_time_ns = metrics
                .observation_service_time_ns
                .saturating_add(started.elapsed().as_nanos());
        }
        update_memory(world, id, simulation_time, &observation);
        Some(observation)
    } else {
        None
    };

    // Replacement can become stationary before reaching navigation invalidation.
    if world
        .btech
        .autopilot_plans
        .get(&id)
        .is_some_and(|p| p.order_id != active.id)
    {
        remove_plan(world, id);
    }

    if let AutopilotOrder::Attack { target, .. } = &active.order
        && observation.as_ref().is_some_and(|observation| {
            observation.contacts.iter().any(|contact| {
                contact.unit == *target && contact.identified && contact.known_destroyed
            })
        })
    {
        if issue_stop(world, id, simulation_time, notices) {
            complete(world, id, simulation_time);
            remove_plan(world, id);
        }
        return Ok(0);
    }

    // Reuse the durable diversion origin when attack-move is already pursuing
    // a contact. This keeps pursuit bounded across replans and restarts.
    let plan_clone = super::diagnostics::pursuit("plan_clone");
    let mut existing_plan = world
        .btech
        .autopilot_plans
        .get(&id)
        .cloned()
        .unwrap_or_default();
    drop(plan_clone);
    let mut engagement = observation
        .as_ref()
        .and_then(|o| super::engagement::resolve(world, id, &active, &controller_config, o));
    // Prediction consumes only filtered sightings and own capability. It survives route
    // changes but never a lifecycle, terrain, mobility, order, or target invalidation.
    let direct = metrics.as_ref().is_some_and(|m| m.direct_pursuit);
    let mut checked_initial = None;
    if let (Some(e), Some(o)) = (engagement.as_mut(), observation.as_ref()) {
        let _estimation = super::diagnostics::pursuit("estimation");
        let map = &world.btech.maps()[&e.target.map];
        if existing_plan.terrain_revision != map_revision(map)
            || existing_plan.mobility_revision != unit_mobility_revision(world, id)
        {
            existing_plan.pursuit = Default::default();
        }
        if !direct {
            // Geometry and all candidate forecasts read the same immutable own
            // equipment. The scope ends before any admitted controls mutate the world.
            let _loadouts = (metrics
                .as_ref()
                .map_or(Default::default(), |m| m.pursuit_policy)
                == super::interception::PursuitPolicy::Adaptive)
                .then(|| {
                    crate::btech::loadout_context::LoadoutScope::participants(
                        &world.btech,
                        [id, id],
                    )
                });
            existing_plan.pursuit.sample_policy(
                active.id,
                e.target_id,
                e.target,
                simulation_time,
                metrics
                    .as_ref()
                    .map_or(Default::default(), |m| m.pursuit_policy),
            );
            if let Some(own) = o.position {
                let rate = if map.movement_modifier > 0 {
                    map.movement_modifier as f64 / 100.0
                } else {
                    1.0
                };
                let maximum = crate::btech::motion_controls::throttle_maximum(
                    world,
                    id,
                    config.battletech.tsm_tow_bonus != 0,
                )
                .unwrap_or(0.0);
                let speed =
                    maximum * f64::from(controller_config.speed_percent) / 100.0 / 645.0 * rate;
                let in_firing_range = o
                    .contacts
                    .iter()
                    .find(|c| c.unit == e.target_id)
                    .is_some_and(|c| c.range <= f64::from(e.maximum));
                let adaptive = metrics
                    .as_ref()
                    .map_or(Default::default(), |m| m.pursuit_policy)
                    == super::interception::PursuitPolicy::Adaptive;
                if adaptive && *geometry_budget > 0 {
                    *geometry_budget -= 1;
                    let positional = e.usable(world, o, GridHex::new(own.x, own.y));
                    let preferred = e.observed_goal(false).contains(GridHex::new(own.x, own.y));
                    // Cache positional eligibility independently of the selected
                    // band: navigation may already have admitted a fallback band.
                    checked_initial = Some(positional);
                    existing_plan.pursuit.geometry(
                        simulation_time,
                        own,
                        e.band,
                        positional,
                        positional && preferred && e.actual_arc(world, id, o),
                    );
                    if let Some(m) = metrics.as_deref_mut() {
                        m.geometry_checks += 1;
                    }
                }
                if in_firing_range && !adaptive {
                    // Once engagement is possible, use the observed target for
                    // closing to the preferred band instead of chasing a future point.
                    existing_plan.pursuit.settle();
                } else if let Some(aim) = existing_plan.pursuit.choose(
                    simulation_time,
                    own,
                    speed,
                    e.band.maximum,
                    map.width,
                    map.height,
                    e.leash,
                    |aim, velocity, uncertainty| {
                        super::steering::pursuit_score(
                            world,
                            config,
                            id,
                            aim,
                            e.target,
                            velocity,
                            uncertainty,
                            maximum * f64::from(controller_config.speed_percent) / 100.0,
                            speed,
                            e.band.maximum,
                            Some((o, &existing_plan.steering, simulation_time)),
                        )
                    },
                ) {
                    e.aim = aim;
                }
            }
        } else {
            existing_plan.pursuit = Default::default();
        }
    } else {
        existing_plan.pursuit = Default::default();
    }
    if engagement.is_some() && *geometry_budget == 0 {
        persist_plan(world, id, existing_plan);
        issue_stop(world, id, simulation_time, notices);
        return Ok(0);
    }
    let same_navigation = |a: Option<super::engagement::Engagement>,
                           b: Option<super::engagement::Engagement>| {
        match (a, b) {
            (Some(a), Some(b)) => a.same_navigation(b),
            (None, None) => true,
            _ => false,
        }
    };
    let fallback = same_navigation(existing_plan.engagement, engagement) && existing_plan.fallback;
    // The observed-position check spends from the same per-controller allowance
    // as prospective goal filtering; it is not a supplemental search budget.
    let mut geometry_left = (*geometry_budget).min(16 - usize::from(checked_initial.is_some()));
    let mut geometry_used = 0u64;
    let mut usable = |hex: GridHex| -> Option<bool> {
        let Some(engagement) = engagement else {
            return Some(true);
        };
        if geometry_left == 0 {
            return None;
        }
        geometry_left -= 1;
        *geometry_budget -= 1;
        geometry_used += 1;
        Some(engagement.usable(world, observation.as_ref().unwrap(), hex))
    };
    // Goal filtering below owns its own bounded closure after mutation begins.
    let initial_usable = engagement
        .and_then(|e| {
            current_position(world, id)
                .filter(|p| e.observed_goal(fallback).contains(GridHex::new(p.x, p.y)))
        })
        .is_some_and(|p| {
            checked_initial.unwrap_or_else(|| usable(GridHex::new(p.x, p.y)).unwrap_or(false))
        });
    if let Some(metrics) = metrics.as_deref_mut() {
        metrics.geometry_checks += geometry_used;
    }
    if initial_usable {
        if let Some(e) = engagement.as_mut() {
            e.aim = e.target;
        }
        if metrics
            .as_ref()
            .map_or(Default::default(), |m| m.pursuit_policy)
            != super::interception::PursuitPolicy::Adaptive
        {
            existing_plan.pursuit.settle();
        }
    }
    if engagement.is_some_and(|e| e.aim != e.target)
        && let Some(m) = metrics.as_deref_mut().filter(|m| m.capture_outcomes)
    {
        m.prediction_ticks += 1;
    }
    let directive = if let Some(e) = engagement {
        Directive {
            goal: Some(e.aim),
            arrival_radius: e.band.maximum,
            complete_on_arrival: false,
            next_waypoint: None,
            attack_move_diversion: e.leash.is_some(),
            attack_move_suppressed: false,
        }
    } else {
        match directive_for_order(
            world,
            id,
            &active,
            &controller_config,
            observation.as_ref(),
            active.progress.attack_move_origin,
        ) {
            Ok(directive) => directive,
            Err(reason) => {
                fail(world, id, simulation_time, reason);
                return Ok(0);
            }
        }
    };

    let Some(goal) = directive.goal else {
        // Follow, attack and hold can deliberately remain stationary.
        if existing_plan.congestion != Congestion::default()
            || world
                .btech
                .autopilot_plans
                .get(&id)
                .is_some_and(|p| p.pursuit != existing_plan.pursuit)
        {
            let mut stationary = existing_plan;
            stationary.congestion = Default::default();
            stationary.search = None;
            persist_plan(world, id, stationary);
        }
        if !issue_stop(world, id, simulation_time, notices) {
            return Ok(0);
        }
        return Ok(0);
    };

    let Some(position) = current_position(world, id) else {
        fail(world, id, simulation_time, AutopilotReason::UnitUnavailable);
        return Ok(0);
    };
    if position.map != goal.map {
        fail(world, id, simulation_time, AutopilotReason::MapChanged);
        return Ok(0);
    }
    let current_hex = GridHex::new(position.x, position.y);
    let goal_hex = GridHex::new(goal.x, goal.y);
    let radius = u32::from(directive.arrival_radius);
    let at_goal = engagement.map_or(current_hex.distance(goal_hex) <= radius, |e| {
        e.observed_goal(fallback).contains(current_hex) && initial_usable
    });
    if at_goal {
        if engagement.is_some() {
            let mut settled = existing_plan;
            settled.congestion = Default::default();
            settled.search = None;
            settled.replacement_pending = false;
            settled.route.clear();
            settled.route_index = 0;
            settled.stagnant_ticks = 0;
            settled.recovery_attempts = 0;
            settled.steering_grace = 0;
            settled.best_waypoint_distance = None;
            settled.order_id = active.id;
            settled.goal = Some((goal, directive.arrival_radius));
            settled.engagement = engagement;
            settled.terrain_revision = map_revision(&world.btech.maps()[&goal.map]);
            settled.mobility_revision = unit_mobility_revision(world, id);
            persist_plan(world, id, settled);
        }
        let stopped = crate::btech::scanner::scanner_unit(world, id)
            .is_some_and(|scanner| scanner.speed.abs() <= 0.1);
        if !stopped {
            if !issue_stop(world, id, simulation_time, notices) {
                return Ok(0);
            }
            return Ok(0);
        }
        if directive.complete_on_arrival {
            if !issue_stop(world, id, simulation_time, notices) {
                return Ok(0);
            }
            complete(world, id, simulation_time);
            remove_plan(world, id);
        } else if let Some(next_waypoint) = directive.next_waypoint {
            update_waypoint(world, id, next_waypoint);
            remove_plan(world, id);
            if !issue_stop(world, id, simulation_time, notices) {
                return Ok(0);
            }
        } else {
            if !issue_stop(world, id, simulation_time, notices) {
                return Ok(0);
            }
        }
        return Ok(0);
    }

    let selected_before_movement =
        crate::btech::scanner::scanner_unit(world, id).and_then(|unit| unit.selected);
    let navigation_changed = existing_plan.order_id != active.id
        || existing_plan.goal != Some((goal, directive.arrival_radius))
        || !same_navigation(existing_plan.engagement, engagement);
    let (mut plan, expansions) = {
        let mut plan = existing_plan;
        let mut expansions = 0;
        if navigation_changed {
            let retain = metrics
                .as_ref()
                .map_or(Default::default(), |m| m.pursuit_policy)
                == super::interception::PursuitPolicy::Adaptive
                && plan.order_id == active.id
                && plan.terrain_revision == map_revision(&world.btech.maps()[&goal.map])
                && plan.mobility_revision == unit_mobility_revision(world, id)
                && plan.engagement.zip(engagement).is_some_and(|(a, b)| {
                    a.target_id == b.target_id
                        && a.target.map == b.target.map
                        && a.leash == b.leash
                        && (a.aim != a.target || b.aim != b.target)
                });
            plan = AutopilotPlan {
                route: if retain {
                    std::mem::take(&mut plan.route)
                } else {
                    Vec::new()
                },
                route_index: if retain { plan.route_index } else { 0 },
                replacement_pending: retain,
                terrain_revision: if retain { plan.terrain_revision } else { 0 },
                mobility_revision: if retain { plan.mobility_revision } else { 0 },
                steering: plan.steering.clone(),
                pursuit: plan.pursuit.clone(),
                engagement,
                order_id: active.id,
                goal: Some((goal, directive.arrival_radius)),
                recovery_attempts: plan.recovery_attempts,
                steering_grace: plan.steering_grace,
                yielding: plan.yielding,
                stagnant_ticks: plan.stagnant_ticks,
                last_hex: plan.last_hex,
                ..AutopilotPlan::default()
            };
        }
        if matches!(&active.order, AutopilotOrder::AttackMove { .. }) {
            if directive.attack_move_diversion && active.progress.attack_move_origin.is_none() {
                if let Some(controller) = world.btech.controllers.get_mut(&id)
                    && let Some(order) = controller.active_order_mut()
                {
                    order.progress.attack_move_origin = Some(position);
                }
            } else if let Some(controller) = world.btech.controllers.get_mut(&id)
                && let Some(order) = controller.active_order_mut()
            {
                if directive.attack_move_suppressed {
                    order.progress.attack_move_suppressed_target =
                        observation.as_ref().and_then(|observation| {
                            opportunistic_target(
                                &controller_config,
                                observation,
                                selected_before_movement,
                            )
                        });
                }
                if !directive.attack_move_diversion {
                    order.progress.attack_move_origin = None;
                }
            }
        }
        plan.engagement = engagement;
        let Some(map) = world.btech.maps().get(&goal.map).cloned() else {
            fail(world, id, simulation_time, AutopilotReason::MapChanged);
            return Ok(0);
        };
        let Ok(width) = u16::try_from(map.width) else {
            fail(world, id, simulation_time, AutopilotReason::Unsupported);
            return Ok(0);
        };
        let Ok(height) = u16::try_from(map.height) else {
            fail(world, id, simulation_time, AutopilotReason::Unsupported);
            return Ok(0);
        };
        let terrain_revision = map_revision(&map);
        let mobility_key = unit_mobility_revision(world, id);
        if plan.terrain_revision != terrain_revision || plan.mobility_revision != mobility_key {
            plan.route.clear();
            plan.replacement_pending = false;
            plan.fallback = false;
            plan.congestion = Default::default();
            if let Some(search) = plan.search.as_mut() {
                search.invalidate();
            }
            plan.search = None;
            plan.route_index = 0;
            plan.stagnant_ticks = 0;
            plan.best_waypoint_distance = None;
        }
        plan.terrain_revision = terrain_revision;
        plan.mobility_revision = mobility_key;
        if plan.congestion.should_poll(simulation_time) {
            let cleared = traversal::watched_clearance(world, id, goal.map, &plan.congestion.cells);
            plan.congestion.polled(simulation_time, cleared);
            congestion_metric(metrics, id, |m| {
                m.clearance_checks += 1;
                m.clearances += u64::from(cleared);
            });
        }
        let retry = plan.congestion.eligible(simulation_time);
        if plan.congestion.waiting && retry.is_none() {
            persist_plan(world, id, plan);
            issue_stop(world, id, simulation_time, notices);
            return Ok(0);
        }
        if plan.route.is_empty() || plan.replacement_pending {
            if plan.search.is_none() {
                // Reserve enough records for every map cell before admitting a job.
                // Existing jobs can finish without eviction or quota shrinkage, while
                // the rotating scheduler fairly admits waiting jobs as reservations free.
                let quota = (usize::from(width) * usize::from(height)).min(record_limit);
                let reserved: usize = world
                    .btech
                    .autopilot_plans
                    .iter()
                    .filter(|(other, _)| **other != id)
                    .filter_map(|(_, plan)| plan.search.as_ref())
                    .map(AStarSearch::record_limit)
                    .sum();
                if quota == 0 || quota > record_limit.saturating_sub(reserved) {
                    if retry.is_some() {
                        congestion_metric(metrics, id, |m| m.resource_deferrals += 1);
                    }
                    drive_pending(
                        world,
                        config,
                        id,
                        &mut plan,
                        engagement,
                        observation.as_ref(),
                        controller_config.speed_percent,
                        simulation_time,
                        notices,
                    );
                    persist_plan(world, id, plan);
                    return Ok(0);
                }
                record_replan(metrics, id);
                super::diagnostics::pursuit_count(if retry.is_some() {
                    "search_congestion"
                } else if plan.replacement_pending {
                    "search_replacement"
                } else {
                    "search_route"
                });
                let Ok(search) = AStarSearch::with_record_limit(
                    width,
                    height,
                    current_hex,
                    engagement.map_or(
                        Goal::new(goal_hex, u32::from(directive.arrival_radius)),
                        |e| e.goal(plan.fallback),
                    ),
                    quota,
                ) else {
                    fail(world, id, simulation_time, AutopilotReason::Invalidated);
                    return Ok(0);
                };
                plan.congestion.started(retry);
                if let Some(retry) = retry {
                    congestion_metric(metrics, id, |m| match retry {
                        Retry::Early => m.early_starts += 1,
                        Retry::Timed => m.timed_starts += 1,
                    });
                }
                plan.search = Some(search);
            }
            let navigation_started = metrics.as_ref().map(|_| Instant::now());
            let traversal = traversal::GroundTraversal::new(world, id, goal.map);
            let allowance = budget.min(EXPANSIONS_PER_CONTROLLER);
            let search = plan.search.as_mut().expect("search initialized");
            let expanded_before = search.total_expanded();
            let traversal_with_leash = |from: GridHex, to: GridHex| {
                if engagement.is_some_and(|e| !e.permits(to)) {
                    None
                } else {
                    super::navigation::Traversal::traversal_cost(&traversal, from, to)
                }
            };
            let mut accept = |hex: GridHex| {
                let Some(e) = engagement else {
                    return Some(true);
                };
                if geometry_left == 0 {
                    return None;
                }
                geometry_left -= 1;
                *geometry_budget -= 1;
                if let Some(m) = metrics.as_deref_mut() {
                    m.geometry_checks += 1;
                }
                Some(e.navigation_usable(world, observation.as_ref().unwrap(), hex))
            };
            let status = search.step_filtered(allowance, &traversal_with_leash, &mut accept);
            for cell in traversal.congested_cells() {
                super::congestion::remember(&mut plan.congestion.cells, cell);
            }
            if let (Some(metrics), Some(started)) = (metrics.as_deref_mut(), navigation_started) {
                metrics.navigation_service_time_ns = metrics
                    .navigation_service_time_ns
                    .saturating_add(started.elapsed().as_nanos());
            }
            let spent = search
                .total_expanded()
                .saturating_sub(expanded_before)
                .try_into()
                .unwrap_or(usize::MAX);
            expansions = match status {
                SearchStatus::Pending { .. } => {
                    drive_pending(
                        world,
                        config,
                        id,
                        &mut plan,
                        engagement,
                        observation.as_ref(),
                        controller_config.speed_percent,
                        simulation_time,
                        notices,
                    );
                    persist_plan(world, id, plan);
                    // Keep the frontier intact for the next heartbeat.  A
                    // pending search must never be mistaken for an empty
                    // route and restarted from scratch.
                    return Ok(spent);
                }
                SearchStatus::Found { path } => {
                    if plan.replacement_pending && !path.cells.contains(&current_hex) {
                        plan.search = None;
                        drive_pending(
                            world,
                            config,
                            id,
                            &mut plan,
                            engagement,
                            observation.as_ref(),
                            controller_config.speed_percent,
                            simulation_time,
                            notices,
                        );
                        persist_plan(world, id, plan);
                        return Ok(spent);
                    }
                    plan.replacement_pending = false;
                    plan.congestion = Default::default();
                    plan.route = path.cells;
                    plan.route_index = 0;
                    // A completed route no longer needs its search frontier.
                    // Release those records for other controllers immediately.
                    plan.search = None;
                    spent
                }
                SearchStatus::Unreachable => {
                    // A crowd is transient. Three bounded retries allow an admitted blocker
                    // to clear without changing static-unreachable or search budget semantics.
                    if plan.congestion.failed(simulation_time) {
                        plan.search = None;
                        record_replan(metrics, id);
                        persist_plan(world, id, plan);
                        issue_stop(world, id, simulation_time, notices);
                        return Ok(spent);
                    }
                    if let Some(e) = engagement.filter(|e| e.aim != e.target) {
                        if let Some(m) = metrics.as_deref_mut().filter(|m| m.capture_outcomes) {
                            m.prediction_fallbacks += 1;
                        }
                        plan.pursuit.reject(e.target);
                        plan.search = None;
                        persist_plan(world, id, plan);
                        issue_stop(world, id, simulation_time, notices);
                        return Ok(spent);
                    }
                    if engagement.is_some() && !plan.fallback {
                        plan.fallback = true;
                        plan.search = None;
                        persist_plan(world, id, plan);
                        issue_stop(world, id, simulation_time, notices);
                        return Ok(spent);
                    }
                    fail(world, id, simulation_time, AutopilotReason::Unreachable);
                    remove_plan(world, id);
                    return Ok(spent);
                }
                SearchStatus::Invalidated => {
                    persist_plan(world, id, plan);
                    return Ok(spent);
                }
                SearchStatus::ResourceLimit { .. } => {
                    fail(world, id, simulation_time, AutopilotReason::ResourceLimit);
                    remove_plan(world, id);
                    return Ok(spent);
                }
            };
        }
        (plan, expansions)
    };

    if let Some(route_position) = plan.route.iter().position(|cell| *cell == current_hex) {
        let next_index = route_position.saturating_add(1);
        if plan.route_index != next_index {
            plan.best_waypoint_distance = None;
        }
        plan.route_index = next_index;
    } else {
        record_replan(metrics, id);
        plan.route.clear();
        plan.search = None;
        plan.route_index = 0;
        let exhausted = navigation_stalled(&mut plan);
        sync_stagnation(world, id, plan.stagnant_ticks);
        if exhausted {
            fail(world, id, simulation_time, AutopilotReason::Stuck);
            remove_plan(world, id);
            return Ok(expansions);
        }
        persist_plan(world, id, plan);
        return Ok(expansions);
    }
    if plan.last_hex != Some(current_hex) {
        if plan.last_hex.is_some() {
            navigation_progress(&mut plan);
        }
        plan.last_hex = Some(current_hex);
        plan.stagnant_ticks = 0;
        plan.best_waypoint_distance = None;
    } else if let Some(next) = plan.route.get(plan.route_index)
        && let Some(point) =
            crate::btech::scanner::scanner_unit(world, id).and_then(|unit| unit.point)
    {
        let center = HexCoordinate {
            x: i32::from(next.x),
            y: i32::from(next.y),
        }
        .center();
        let distance = point.range(center)?;
        if plan
            .best_waypoint_distance
            .is_none_or(|best| distance + 0.02 < best)
        {
            if plan.best_waypoint_distance.is_some() {
                navigation_progress(&mut plan);
            } else if navigation_changed {
                // A new goal establishes a distance baseline; it is not travel.
                plan.stagnant_ticks = plan.stagnant_ticks.saturating_add(1);
            }
            plan.best_waypoint_distance = Some(distance);
        } else {
            plan.stagnant_ticks = plan.stagnant_ticks.saturating_add(1);
        }
    } else {
        plan.stagnant_ticks = plan.stagnant_ticks.saturating_add(1);
    }
    if plan.stagnant_ticks >= STAGNANT_TICKS_BEFORE_REPLAN {
        record_replan(metrics, id);
        plan.route.clear();
        plan.search = None;
        plan.route_index = 0;
        plan.stagnant_ticks = 0;
        plan.best_waypoint_distance = None;
        plan.recovery_attempts = plan.recovery_attempts.saturating_add(1);
        sync_stagnation(world, id, 0);
        if plan.recovery_attempts > MAX_RECOVERY_ATTEMPTS {
            fail(world, id, simulation_time, AutopilotReason::Stuck);
            remove_plan(world, id);
            return Ok(expansions);
        }
        persist_plan(world, id, plan);
        return Ok(expansions);
    }

    sync_stagnation(world, id, plan.stagnant_ticks);

    let Some(next) = plan.route.get(plan.route_index).copied() else {
        plan.route.clear();
        plan.search = None;
        issue_stop(world, id, simulation_time, notices);
        persist_plan(world, id, plan);
        return Ok(expansions);
    };
    let Some(_map) = world.btech.maps().get(&goal.map) else {
        fail(world, id, simulation_time, AutopilotReason::MapChanged);
        remove_plan(world, id);
        return Ok(expansions);
    };
    let assessment = traversal::assess(
        world,
        id,
        Position {
            map: goal.map,
            x: current_hex.x,
            y: current_hex.y,
        },
        Position {
            map: goal.map,
            x: next.x,
            y: next.y,
        },
    );
    if !assessment.eligible {
        record_replan(metrics, id);
        // The terrain or mobility rules changed since the route was found.
        // Rebuild the frontier from the live position on the next tick.
        plan.route.clear();
        plan.search = None;
        plan.route_index = 0;
        persist_plan(world, id, plan);
        return Ok(expansions);
    }
    if matches!(
        assessment.reason,
        traversal::TraversalReason::Occupied | traversal::TraversalReason::Congested
    ) && should_yield(world, id, goal.map, next)
        && courtesy_wait(&mut plan, next, simulation_time)
    {
        if !issue_stop(world, id, simulation_time, notices) {
            return Ok(expansions);
        }
        persist_plan(world, id, plan);
        return Ok(expansions);
    }
    let cap = maximum_speed(world, id) * f64::from(controller_config.speed_percent) / 100.0;
    let combat = engagement.and_then(|e| observation.as_ref().map(|o| (o, e.target)));
    match super::steering::drive(
        world,
        config,
        id,
        &plan.route,
        plan.route_index,
        cap,
        combat,
        engagement.is_some_and(|e| e.aim != e.target),
        plan.pursuit.velocity().map(|velocity| {
            (
                velocity,
                cap / 645.0
                    * world.btech.maps().get(&position.map).map_or(1.0, |m| {
                        if m.movement_modifier > 0 {
                            m.movement_modifier as f64 / 100.0
                        } else {
                            1.0
                        }
                    }),
            )
        }),
        &mut plan.steering,
        simulation_time,
        notices,
    ) {
        Ok(true) => {
            navigation_control_progress(&mut plan);
            sync_stagnation(world, id, plan.stagnant_ticks);
        }
        Ok(false) => {}
        Err(_) => {
            fail(world, id, simulation_time, AutopilotReason::UnitUnavailable);
            return Ok(expansions);
        }
    }
    persist_plan(world, id, plan);
    Ok(expansions)
}

#[derive(Debug, Clone, Copy)]
struct Directive {
    goal: Option<Position>,
    arrival_radius: u16,
    complete_on_arrival: bool,
    next_waypoint: Option<u16>,
    attack_move_diversion: bool,
    attack_move_suppressed: bool,
}

fn directive_for_order(
    world: &World,
    id: ObjectId,
    record: &AutopilotOrderRecord,
    config: &AutopilotConfig,
    observation: Option<&AutopilotObservation>,
    attack_move_origin: Option<Position>,
) -> std::result::Result<Directive, AutopilotReason> {
    let current = current_position(world, id).ok_or(AutopilotReason::UnitUnavailable)?;
    match &record.order {
        AutopilotOrder::Move {
            destination,
            arrival_radius,
        } => Ok(Directive {
            goal: Some(*destination),
            arrival_radius: *arrival_radius,
            complete_on_arrival: true,
            next_waypoint: None,
            attack_move_diversion: false,
            attack_move_suppressed: false,
        }),
        AutopilotOrder::AttackMove {
            destination,
            arrival_radius,
        } => {
            let observation = observation.ok_or(AutopilotReason::Invalidated)?;
            let selected =
                crate::btech::scanner::scanner_unit(world, id).and_then(|unit| unit.selected);
            let target = opportunistic_target(config, observation, selected)
                .filter(|target| Some(*target) != record.progress.attack_move_suppressed_target);
            let mut suppressed = false;
            if let Some(target) = target
                && let Some(contact) = observation
                    .contacts
                    .iter()
                    .find(|contact| contact.unit == target && contact.identified)
            {
                let distance = current_position(world, id)
                    .map(|position| {
                        GridHex::new(position.x, position.y)
                            .distance(GridHex::new(contact.position.x, contact.position.y))
                    })
                    .unwrap_or(u32::MAX);
                let origin = attack_move_origin.unwrap_or(current);
                let origin_distance = GridHex::new(origin.x, origin.y)
                    .distance(GridHex::new(contact.position.x, contact.position.y));
                if origin.map == current.map && origin_distance <= 6 && distance > 1 {
                    return Ok(Directive {
                        goal: Some(contact.position),
                        arrival_radius: 1,
                        complete_on_arrival: false,
                        next_waypoint: None,
                        attack_move_diversion: true,
                        attack_move_suppressed: false,
                    });
                }
                suppressed = true;
            }
            Ok(Directive {
                goal: Some(*destination),
                arrival_radius: *arrival_radius,
                complete_on_arrival: true,
                next_waypoint: None,
                attack_move_diversion: false,
                attack_move_suppressed: suppressed,
            })
        }
        AutopilotOrder::Follow { target, separation } => {
            if !friendly_target(world, id, *target) {
                return Err(AutopilotReason::InvalidTarget);
            }
            let target_position =
                current_position(world, *target).ok_or(AutopilotReason::InvalidTarget)?;
            if target_position.map != current.map {
                return Err(AutopilotReason::MapChanged);
            }
            if GridHex::new(current.x, current.y)
                .distance(GridHex::new(target_position.x, target_position.y))
                <= u32::from(*separation)
            {
                return Ok(Directive {
                    goal: None,
                    arrival_radius: *separation,
                    complete_on_arrival: false,
                    next_waypoint: None,
                    attack_move_diversion: false,
                    attack_move_suppressed: false,
                });
            }
            Ok(Directive {
                goal: Some(target_position),
                arrival_radius: *separation,
                complete_on_arrival: false,
                next_waypoint: None,
                attack_move_diversion: false,
                attack_move_suppressed: false,
            })
        }
        AutopilotOrder::Patrol { waypoints } => {
            let index = usize::from(record.progress.waypoint_index) % waypoints.len();
            let waypoint = waypoints[index];
            Ok(Directive {
                goal: Some(waypoint),
                arrival_radius: 0,
                complete_on_arrival: false,
                next_waypoint: Some(((index + 1) % waypoints.len()) as u16),
                attack_move_diversion: false,
                attack_move_suppressed: false,
            })
        }
        AutopilotOrder::Attack { target, range } => {
            let observation = observation.ok_or(AutopilotReason::Invalidated)?;
            if observation
                .contacts
                .iter()
                .any(|contact| contact.unit == *target && contact.identified && contact.friendly)
            {
                return Err(AutopilotReason::InvalidTarget);
            }
            let selected = choose_target(observation, Some(*target), None);
            let target_position = selected
                .and_then(|target| {
                    observation
                        .contacts
                        .iter()
                        .find(|contact| contact.unit == target)
                        .map(|c| c.position)
                })
                .or_else(|| {
                    observation
                        .remembered
                        .iter()
                        .find(|sighting| sighting.unit == *target)
                        .map(|sighting| sighting.position)
                })
                .ok_or(AutopilotReason::ContactLost)?;
            if target_position.map != current.map {
                return Err(AutopilotReason::MapChanged);
            }
            let range = if selected.is_some() {
                range.or(config.preferred_range)
            } else {
                None
            };
            let distance = GridHex::new(current.x, current.y)
                .distance(GridHex::new(target_position.x, target_position.y));
            if range.is_some_and(|band| distance < u32::from(band.minimum)) {
                let retreat = retreat_position(world, id, current, target_position)
                    .ok_or(AutopilotReason::Unreachable)?;
                return Ok(Directive {
                    goal: Some(retreat),
                    arrival_radius: 0,
                    complete_on_arrival: false,
                    next_waypoint: None,
                    attack_move_diversion: false,
                    attack_move_suppressed: false,
                });
            }
            if range.is_some_and(|band| {
                distance >= u32::from(band.minimum) && distance <= u32::from(band.maximum)
            }) {
                return Ok(Directive {
                    goal: None,
                    arrival_radius: 0,
                    complete_on_arrival: false,
                    next_waypoint: None,
                    attack_move_diversion: false,
                    attack_move_suppressed: false,
                });
            }
            Ok(Directive {
                goal: Some(target_position),
                arrival_radius: range.map_or(0, |band| band.maximum),
                complete_on_arrival: false,
                next_waypoint: None,
                attack_move_diversion: false,
                attack_move_suppressed: false,
            })
        }
        AutopilotOrder::Hold => Ok(Directive {
            goal: None,
            arrival_radius: 0,
            complete_on_arrival: false,
            next_waypoint: None,
            attack_move_diversion: false,
            attack_move_suppressed: false,
        }),
    }
}

/// Reassess on staggered three-tick boundaries, or immediately after contact loss.
pub(super) fn opportunistic_target(
    config: &AutopilotConfig,
    observation: &AutopilotObservation,
    selected: Option<ObjectId>,
) -> Option<ObjectId> {
    if config.fire_mode != super::AutopilotFireMode::Opportunistic {
        return None;
    }
    combat_target(config, observation, selected)
}

fn combat_target(
    config: &AutopilotConfig,
    observation: &AutopilotObservation,
    selected: Option<ObjectId>,
) -> Option<ObjectId> {
    let selected = selected.filter(|target| {
        observation.contacts.iter().any(|contact| {
            contact.unit == *target
                && contact.identified
                && !contact.friendly
                && !contact.known_destroyed
        })
    });
    match config.fire_mode {
        super::AutopilotFireMode::Hold => None,
        super::AutopilotFireMode::Opportunistic => {
            let reassess = observation.time.rem_euclid(3) == observation.unit.0.rem_euclid(3);
            if selected.is_some() && !reassess {
                selected
            } else {
                choose_target(observation, None, selected)
            }
        }
        super::AutopilotFireMode::AssignedTarget => selected,
    }
}

fn supported_ground_unit(world: &World, id: ObjectId) -> bool {
    if world
        .btech
        .vehicles()
        .get(&id)
        .is_some_and(|vehicle| vehicle.definition().is_vtol())
    {
        return false;
    }
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        return !unit.is_destroyed()
            && unit.flight().is_none()
            && !unit.airborne()
            && unit.position().is_some();
    }
    world
        .btech
        .vehicles()
        .get(&id)
        .is_some_and(|vehicle| !vehicle.is_destroyed() && vehicle.position().is_some())
}

fn current_position(world: &World, id: ObjectId) -> Option<Position> {
    crate::btech::scanner::scanner_unit(world, id).and_then(|unit| unit.position)
}

fn friendly_target(world: &World, source: ObjectId, target: ObjectId) -> bool {
    let Some(source) = crate::btech::scanner::scanner_unit(world, source) else {
        return false;
    };
    let Some(target) = crate::btech::scanner::scanner_unit(world, target) else {
        return false;
    };
    source.signature.team == target.signature.team
}

fn retreat_position(
    world: &World,
    id: ObjectId,
    current: Position,
    target: Position,
) -> Option<Position> {
    let map = world.btech.maps().get(&current.map)?;
    let width = u16::try_from(map.width).ok()?;
    let height = u16::try_from(map.height).ok()?;
    let current_hex = GridHex::new(current.x, current.y);
    let target_hex = GridHex::new(target.x, target.y);
    current_hex
        .neighbors_within(width, height)
        .filter(|hex| {
            traversal::assess(
                world,
                id,
                current,
                Position {
                    map: current.map,
                    x: hex.x,
                    y: hex.y,
                },
            )
            .eligible
        })
        .max_by_key(|hex| (hex.distance(target_hex), std::cmp::Reverse((hex.x, hex.y))))
        .map(|hex| Position {
            map: current.map,
            x: hex.x,
            y: hex.y,
        })
}

/// Lower object IDs win a known occupied hex.  A unit only yields to an
/// occupant it can know about through team identity or an acquired contact;
/// hidden enemy placement must not leak into path decisions.
fn should_yield(world: &World, moving: ObjectId, map: ObjectId, next: GridHex) -> bool {
    let Ok(occupants) = super::super::map_slots::hex_occupants(
        world,
        map,
        HexCoordinate {
            x: i32::from(next.x),
            y: i32::from(next.y),
        },
    ) else {
        return false;
    };
    occupants.into_iter().any(|occupant| {
        occupant < moving && super::traversal::known_occupant(world, moving, occupant)
    })
}

/// Yield briefly for known traffic, then let authoritative entry rules decide.
/// A stationary occupant must not permanently reserve a legally traversable hex.
fn courtesy_wait(plan: &mut AutopilotPlan, next: GridHex, now: i64) -> bool {
    if plan.yielding.is_none_or(|(hex, _)| hex != next) {
        plan.yielding = Some((next, now + 5));
    }
    plan.yielding.is_some_and(|(_, until)| now < until)
}

fn unit_power(world: &World, id: ObjectId) -> Option<Power> {
    world
        .btech
        .constructed_units()
        .get(&id)
        .map(|unit| unit.power())
        .or_else(|| {
            world
                .btech
                .vehicles()
                .get(&id)
                .map(|vehicle| vehicle.power())
        })
}

fn maximum_speed(world: &World, id: ObjectId) -> f64 {
    world
        .btech
        .constructed_units()
        .get(&id)
        .map(|unit| unit.mobility().maximum_speed)
        .or_else(|| {
            world
                .btech
                .vehicles()
                .get(&id)
                .map(|vehicle| vehicle.maximum_speed())
        })
        .unwrap_or(0.0)
}

fn unit_mobility_revision(world: &World, id: ObjectId) -> u64 {
    maximum_speed(world, id).to_bits()
        ^ crate::btech::scanner::scanner_unit(world, id)
            .map_or(0, |scanner| u64::from(scanner.vehicle))
}

fn map_revision(map: &crate::btech::StoredMap) -> usize {
    let terrain = map
        .terrain
        .as_ref()
        .map(|value| Arc::as_ptr(value) as usize)
        .unwrap_or(0);
    let decorations = Arc::as_ptr(&map.decorations) as usize;
    terrain ^ decorations.rotate_left(17)
}

fn stop_unit(world: &mut World, id: ObjectId) -> Result<Notice> {
    crate::btech::motion::set_speed_autopilot(world, id, 0.0)
}

fn issue_stop(
    world: &mut World,
    id: ObjectId,
    simulation_time: i64,
    notices: &mut Vec<Notice>,
) -> bool {
    match stop_unit(world, id) {
        Ok(notice) => {
            notices.push(notice);
            true
        }
        Err(_) => {
            fail(world, id, simulation_time, AutopilotReason::UnitUnavailable);
            false
        }
    }
}

fn update_memory(world: &mut World, id: ObjectId, now: i64, observation: &AutopilotObservation) {
    let hostile = observation
        .contacts
        .iter()
        .filter(|contact| contact.identified && !contact.friendly && !contact.known_destroyed)
        .map(|contact| (contact.unit, contact.position));
    if let Some(controller) = world.btech.controllers.get_mut(&id) {
        for contact in observation
            .contacts
            .iter()
            .filter(|contact| contact.known_destroyed)
        {
            controller.sightings.remove(&contact.unit);
        }
        controller.update_sightings(hostile, now);
        controller.expire_sightings(now.saturating_sub(SIGHTING_MEMORY_SECONDS));
    }
}

fn update_waypoint(world: &mut World, id: ObjectId, next: u16) {
    if let Some(controller) = world.btech.controllers.get_mut(&id)
        && let Some(active) = controller.active_order_mut()
    {
        active.progress.waypoint_index = next;
    }
}

fn sync_stagnation(world: &mut World, id: ObjectId, stagnant_ticks: u16) {
    if let Some(controller) = world.btech.controllers.get_mut(&id)
        && let Some(order) = controller.active_order_mut()
    {
        order.progress.stagnant_ticks = stagnant_ticks;
    }
}

/// Only actual forward travel or actual-target settling replenishes navigation retries.
fn navigation_progress(plan: &mut AutopilotPlan) {
    plan.recovery_attempts = 0;
    plan.stagnant_ticks = 0;
    plan.steering_grace = 0;
}

/// Turning and braking get finite patience, but cannot replenish failed-route retries.
fn navigation_control_progress(plan: &mut AutopilotPlan) {
    if plan.steering_grace < STAGNANT_TICKS_BEFORE_REPLAN {
        plan.steering_grace += 1;
        plan.stagnant_ticks = 0;
    }
}

/// Stale route origins cannot consume a retry every tick or defer failure indefinitely.
fn navigation_stalled(plan: &mut AutopilotPlan) -> bool {
    plan.stagnant_ticks = plan.stagnant_ticks.saturating_add(1);
    if plan.stagnant_ticks >= STAGNANT_TICKS_BEFORE_REPLAN {
        plan.stagnant_ticks = 0;
        plan.recovery_attempts = plan.recovery_attempts.saturating_add(1);
    }
    plan.recovery_attempts > MAX_RECOVERY_ATTEMPTS
}

fn complete(world: &mut World, id: ObjectId, now: i64) {
    if let Some(controller) = world.btech.controllers.get_mut(&id) {
        controller.complete_active(now);
    }
}

fn fail(world: &mut World, id: ObjectId, now: i64, reason: AutopilotReason) {
    // A blocked controller must not retain a previous route's desired speed.
    // This is an internal stop, so it cannot trigger manual takeover.
    let _ = stop_unit(world, id);
    if let Some(controller) = world.btech.controllers.get_mut(&id) {
        controller.fail_active(now, reason);
    }
    world.btech.autopilot_plans.remove(&id);
}

fn persist_plan(world: &mut World, id: ObjectId, plan: AutopilotPlan) {
    let _publication = super::diagnostics::pursuit("plan_publication");
    world.btech.autopilot_plans.insert(id, plan);
}

fn remove_plan(world: &mut World, id: ObjectId) {
    world.btech.autopilot_plans.remove(&id);
}

fn total_search_records(world: &World) -> usize {
    world
        .btech
        .autopilot_plans
        .values()
        .filter_map(|plan| plan.search.as_ref())
        .map(AStarSearch::record_count)
        .sum()
}

/// Count a replan only when optional diagnostics are enabled.
fn record_replan(metrics: &mut Option<&mut AutopilotRuntimeMetrics>, id: ObjectId) {
    if let Some(metrics) = metrics.as_deref_mut() {
        metrics.replans = metrics.replans.saturating_add(1);
        if metrics.capture_outcomes {
            *metrics.replans_by_unit.entry(id).or_default() += 1;
        }
    }
}

/// Bound a launch without rolling combat dice, using the same mode heat rule as
/// expenditure. Gatling fire uses its maximum six-point roll for admission.
fn projected_launch_heat(weapon: crate::Weapon, mode: crate::FireMode, damage_heat: u8) -> u16 {
    u16::from(mode.launch_heat(
        weapon,
        (mode == crate::FireMode::Gatling).then_some(6),
        true,
    )) + u16::from(damage_heat)
}

fn fire_target_if_ready(
    world: &mut World,
    config: &Config,
    shooter: ObjectId,
    target: ObjectId,
    controller_config: &AutopilotConfig,
    notices: &mut Vec<Notice>,
    metrics: &mut Option<&mut AutopilotRuntimeMetrics>,
) {
    let _measurement = crate::btech::autopilot::diagnostics::measure(
        crate::btech::autopilot::diagnostics::Category::Shots,
    );
    let selected =
        crate::btech::scanner::scanner_unit(world, shooter).and_then(|scanner| scanner.selected);
    if selected != Some(target)
        && crate::btech::targeting::select_target_autopilot(world, shooter, Some(target)).is_err()
    {
        return;
    }
    if world.btech.constructed_units().contains_key(&shooter) {
        let indices: Vec<_> = world.btech.constructed_units()[&shooter]
            .loadout()
            .ok()
            .map(|loadout| {
                loadout
                    .weapons
                    .iter()
                    .enumerate()
                    .filter(|(_, mount)| !mount.weapon.is_ams() && !mount.weapon.is_artillery())
                    .map(|(index, _)| index)
                    .collect()
            })
            .unwrap_or_default();
        for index in indices {
            let preparation = super::diagnostics::combat("mount_preparation");
            let unit = &world.btech.constructed_units()[&shooter];
            let Ok(loadout) = crate::btech::validation_context::loadout(shooter, unit) else {
                continue;
            };
            let Some(readiness) = unit
                .weapon_readiness_with_loadout(&loadout, index)
                .ok()
                .filter(|readiness| readiness.ready)
            else {
                continue;
            };
            let unit = &world.btech.constructed_units()[&shooter];
            let (Ok(mode), Ok(damage)) = (
                unit.effective_fire_mode_with_loadout(&loadout, index),
                unit.weapon_damage_with_loadout(&loadout, index),
            ) else {
                continue;
            };
            let projected = projected_launch_heat(readiness.weapon, mode, damage.heat);
            if unit.heat().stored + f64::from(projected) > f64::from(controller_config.heat_ceiling)
            {
                continue;
            }
            // Admission is checked per mount. A ready weapon outside its arc
            // or range must not hide a later ordinary direct-fire mount.
            drop(preparation);
            if let Ok(report) = crate::btech::shot::resolve_shot_autopilot(
                world,
                shooter,
                target,
                index,
                crate::btech::ShotRules::configured(&config.battletech, false),
            ) {
                notices.extend(report.notices());
                if let Some(metrics) = metrics.as_deref_mut() {
                    metrics.record_shot(shooter);
                }
                // Continue through the ordinary mounts.  Readiness and heat
                // are read again at the top of the loop, so one shot can
                // make the next mount ineligible without leaking past the
                // configured ceiling.
            }
        }
    } else if world.btech.vehicles().contains_key(&shooter) {
        let indices: Vec<_> = world.btech.vehicles()[&shooter]
            .loadout()
            .ok()
            .map(|loadout| {
                loadout
                    .weapons
                    .iter()
                    .enumerate()
                    .filter(|(_, mount)| !mount.weapon.is_ams() && !mount.weapon.is_artillery())
                    .map(|(index, _)| index)
                    .collect()
            })
            .unwrap_or_default();
        let rules = crate::btech::VehicleShotRules {
            shot: crate::btech::ShotRules::configured(&config.battletech, false),
            shooter_criticals: crate::btech::VehicleImpactRules::configured(
                &config.battletech,
                false,
            )
            .criticals,
        };
        for index in indices {
            let preparation = super::diagnostics::combat("mount_preparation");
            let unit = &world.btech.vehicles()[&shooter];
            let Ok(loadout) = unit.loadout() else {
                continue;
            };
            let Some(readiness) = unit
                .weapon_readiness_with_loadout(&loadout, index)
                .ok()
                .filter(|readiness| readiness.ready)
            else {
                continue;
            };
            let unit = &world.btech.vehicles()[&shooter];
            let Ok(mode) = unit.effective_fire_mode_with_loadout(&loadout, index) else {
                continue;
            };
            let projected = projected_launch_heat(readiness.weapon, mode, 0);
            if unit.weapon_heat() + f64::from(projected) > f64::from(controller_config.heat_ceiling)
            {
                continue;
            }
            drop(preparation);
            if let Ok(report) = crate::btech::vehicle_fire::fire_vehicle_shot_autopilot(
                world, shooter, target, index, rules,
            ) {
                notices.extend(report.notices());
                if let Some(metrics) = metrics.as_deref_mut() {
                    metrics.record_shot(shooter);
                }
                // Re-evaluate live readiness, ammunition and heat before a
                // subsequent mount rather than treating the first admitted
                // shot as the whole firing decision.
            }
        }
    }
}

#[cfg(test)]
#[path = "scheduler_tests.rs"]
mod scheduler_tests;

/// Diagnostics describe committed attempts only and are disabled in ordinary execution.
fn congestion_metric(
    metrics: &mut Option<&mut AutopilotRuntimeMetrics>,
    id: ObjectId,
    update: impl FnOnce(&mut CongestionMetrics),
) {
    if let Some(metrics) = metrics.as_deref_mut().filter(|m| m.capture_outcomes) {
        update(metrics.congestion_by_unit.entry(id).or_default());
    }
}

#[cfg(test)]
mod navigation_recovery_tests {
    use super::*;

    #[test]
    fn courtesy_wait_is_bounded_and_checkpointed() {
        let mut plan = AutopilotPlan::default();
        let cell = GridHex::new(4, 5);
        assert!(courtesy_wait(&mut plan, cell, 100));
        let checkpoint = plan.clone();
        for time in 101..105 {
            assert!(courtesy_wait(&mut plan, cell, time));
        }
        for time in 105..200 {
            assert!(!courtesy_wait(&mut plan, cell, time));
        }
        assert_eq!(plan, checkpoint);
        assert!(courtesy_wait(&mut plan, GridHex::new(5, 5), 200));
        assert_ne!(plan, checkpoint);
        plan = checkpoint;
        assert!(!courtesy_wait(&mut plan, cell, 200));
    }

    #[test]
    fn changes_of_goal_do_not_replenish_failed_navigation() {
        let mut plan = AutopilotPlan::default();
        for tick in 1..=40 {
            plan.goal = Some((
                Position {
                    map: ObjectId(1),
                    x: tick % 2,
                    y: 1,
                },
                0,
            ));
            assert_eq!(navigation_stalled(&mut plan), tick == 40);
        }
        assert_eq!(plan.recovery_attempts, 4);
    }

    #[test]
    fn actual_progress_replenishes_only_the_transient_budget() {
        let mut plan = AutopilotPlan::default();
        for _ in 0..30 {
            assert!(!navigation_stalled(&mut plan));
        }
        let checkpoint = plan.clone();
        assert_eq!(plan.recovery_attempts, 3);
        navigation_progress(&mut plan);
        assert_eq!(plan.recovery_attempts, 0);
        assert_ne!(plan, checkpoint);
        for _ in 0..30 {
            assert!(!navigation_stalled(&mut plan));
        }
        assert_eq!(plan, checkpoint);
    }

    #[test]
    fn turning_without_travel_cannot_renew_navigation_forever() {
        let mut plan = AutopilotPlan::default();
        let mut blocked_at = None;
        for tick in 1..=50 {
            if navigation_stalled(&mut plan) {
                blocked_at = Some(tick);
                break;
            }
            navigation_control_progress(&mut plan);
        }
        assert_eq!(blocked_at, Some(50));
        navigation_progress(&mut plan);
        assert_eq!(plan.steering_grace, 0);
    }

    #[tokio::test]
    async fn explicit_attack_with_weapons_hold_aligns_without_expenditure() {
        let root = super::super::benchmark::copy_game_root().unwrap();
        let config = Config::load(&root).unwrap();
        let base = crate::persistence::load(&config.database()).await.unwrap();
        let (mut world, id, _, _) = super::super::encounters::fixture(
            &config,
            base,
            include_str!("../../../game/units/JR7-D.toml"),
            "behind",
            1,
        )
        .unwrap();
        world
            .btech
            .controllers
            .get_mut(&id)
            .unwrap()
            .configure(
                super::super::AutopilotConfigPatch {
                    fire_mode: Some(super::super::AutopilotFireMode::Hold),
                    ..Default::default()
                },
                None,
            )
            .unwrap();
        world.btech.controllers.get_mut(&id).unwrap().start_next(0);
        let before = world.btech.constructed_units()[&id].clone();
        let mut metrics = AutopilotRuntimeMetrics::default();
        let notices = advance_combat_with_metrics(&mut world, &config, 1, &mut metrics).unwrap();
        let after = &world.btech.constructed_units()[&id];
        assert!(!notices.is_empty());
        assert_eq!(metrics.autonomous_shots, 0);
        assert_eq!(before.ammunition(), after.ammunition());
        assert_eq!(before.heat(), after.heat());
        assert_eq!(before.dice, after.dice);
        // A stopped route may still be turning. Combat can twist/aim its mounts,
        // but must not replace the navigation heading in that phase.
        crate::btech::motion::set_heading_autopilot(&mut world, id, 123.0).unwrap();
        world.btech.autopilot_plans.insert(
            id,
            AutopilotPlan {
                route: vec![GridHex::new(1, 1), GridHex::new(1, 2)],
                route_index: 1,
                ..Default::default()
            },
        );
        advance_combat_with_metrics(&mut world, &config, 2, &mut metrics).unwrap();
        assert_eq!(
            super::super::steering::motion(&world, id)
                .unwrap()
                .desired_heading,
            123.0
        );
    }

    #[tokio::test]
    async fn navigation_publication_does_not_change_standing_attempts() {
        let root = super::super::benchmark::copy_game_root().unwrap();
        let config = Config::load(&root).unwrap();
        let base = crate::persistence::load(&config.database()).await.unwrap();
        let (mut world, id, _, _) = super::super::encounters::fixture(
            &config,
            base,
            include_str!("../../../game/units/JR7-D.toml"),
            "long_approach",
            1,
        )
        .unwrap();
        advance(&mut world, &config, 1).unwrap();
        world
            .btech
            .controllers
            .get_mut(&id)
            .unwrap()
            .active_order_mut()
            .unwrap()
            .progress
            .recovery_attempts = 2;
        sync_stagnation(&mut world, id, 7);
        let progress = &world.btech.controllers()[&id]
            .active_order()
            .unwrap()
            .progress;
        assert_eq!(progress.recovery_attempts, 2);
        assert_eq!(progress.stagnant_ticks, 7);
    }
}

/// Follow only a live-admitted prefix while the single replacement frontier is pending.
fn drive_pending(
    world: &mut World,
    config: &Config,
    id: ObjectId,
    plan: &mut AutopilotPlan,
    engagement: Option<super::engagement::Engagement>,
    observation: Option<&AutopilotObservation>,
    speed_percent: u8,
    now: i64,
    notices: &mut Vec<Notice>,
) {
    let Some(position) = current_position(world, id) else {
        return;
    };
    let hex = GridHex::new(position.x, position.y);
    let Some(index) = plan.route.iter().position(|p| *p == hex) else {
        issue_stop(world, id, now, notices);
        return;
    };
    let traversal = traversal::GroundTraversal::new(world, id, position.map);
    let mut prefix = vec![hex];
    for next in plan.route.iter().skip(index + 1).take(8) {
        if engagement.is_some_and(|e| !e.permits(*next))
            || super::navigation::Traversal::traversal_cost(
                &traversal,
                *prefix.last().unwrap(),
                *next,
            )
            .is_none()
        {
            break;
        }
        prefix.push(*next);
    }
    if prefix.len() < 2 {
        issue_stop(world, id, now, notices);
        return;
    }
    plan.route_index = index + 1;
    let cap = maximum_speed(world, id) * f64::from(speed_percent) / 100.0;
    let combat = engagement.and_then(|e| observation.map(|o| (o, e.target)));
    if super::steering::drive(
        world,
        config,
        id,
        &prefix,
        1,
        cap,
        combat,
        true,
        plan.pursuit.velocity().map(|velocity| {
            (
                velocity,
                cap / 645.0
                    * world.btech.maps().get(&position.map).map_or(1.0, |m| {
                        if m.movement_modifier > 0 {
                            m.movement_modifier as f64 / 100.0
                        } else {
                            1.0
                        }
                    }),
            )
        }),
        &mut plan.steering,
        now,
        notices,
    )
    .is_err()
    {
        issue_stop(world, id, now, notices);
    }
}

#[cfg(test)]
mod replacement_tests {
    use super::*;
    #[tokio::test]
    async fn cached_geometry_honors_an_admitted_fallback_band() {
        let root = super::super::benchmark::copy_game_root().unwrap();
        let config = Config::load(&root).unwrap();
        let base = crate::persistence::load(&config.database()).await.unwrap();
        let (mut world, id, _, _) = super::super::encounters::fixture(
            &config,
            base,
            include_str!("../../../game/units/JR7-D.toml"),
            "fallback",
            1,
        )
        .unwrap();
        for tick in 1..=3 {
            let mut metrics = AutopilotRuntimeMetrics {
                pursuit_policy: super::super::interception::PursuitPolicy::Adaptive,
                ..Default::default()
            };
            advance_with_metrics(&mut world, &config, tick, &mut metrics).unwrap();
        }
        let plan = &world.btech.autopilot_plans[&id];
        assert!(
            plan.fallback,
            "The preferred band is deliberately unreachable"
        );
        assert_eq!(
            plan.stagnant_ticks, 0,
            "A usable fallback is settled, not stuck"
        );
        assert!(plan.route.is_empty() && plan.search.is_none());
        assert_eq!(
            super::super::steering::motion(&world, id)
                .unwrap()
                .desired_speed,
            0.0
        );
    }
    #[test]
    fn pending_route_state_participates_in_checkpoint_equality() {
        let before = AutopilotPlan::default();
        let mut after = before.clone();
        after.replacement_pending = true;
        assert_ne!(before, after);
        after = before.clone();
        assert_eq!(before, after);
    }
    #[tokio::test]
    async fn missing_retained_prefix_requests_normal_stop() {
        let root = super::super::benchmark::copy_game_root().unwrap();
        let config = Config::load(&root).unwrap();
        let base = crate::persistence::load(&config.database()).await.unwrap();
        let (mut world, id, _, _) = super::super::encounters::fixture(
            &config,
            base,
            include_str!("../../../game/units/JR7-D.toml"),
            "approach",
            1,
        )
        .unwrap();
        let mut plan = AutopilotPlan {
            replacement_pending: true,
            ..Default::default()
        };
        let mut notices = Vec::new();
        drive_pending(
            &mut world,
            &config,
            id,
            &mut plan,
            None,
            None,
            100,
            1,
            &mut notices,
        );
        assert_eq!(
            super::super::steering::motion(&world, id)
                .unwrap()
                .desired_speed,
            0.0
        );
        assert!(plan.search.is_none());
        assert!(plan.replacement_pending);
        let position = current_position(&world, id).unwrap();
        let before = world.btech.clone();
        let score = super::super::steering::pursuit_score(
            &world,
            &config,
            id,
            position,
            position,
            (0.0, 0.0),
            0.0,
            100.0,
            0.2,
            3,
            None,
        )
        .unwrap();
        let uncertain = super::super::steering::pursuit_score(
            &world,
            &config,
            id,
            position,
            position,
            (0.0, 0.0),
            1.0,
            100.0,
            0.2,
            3,
            None,
        )
        .unwrap();
        assert!((uncertain.0 - score.0 - 5.0).abs() < 1e-9);
        assert_eq!(uncertain.1, score.1);
        assert_eq!(world.btech, before);

        let mut aim = position;
        aim.x += 1;
        let destination = crate::HexCoordinate {
            x: i32::from(aim.x),
            y: i32::from(aim.y),
        }
        .center();
        let motion = world
            .btech
            .constructed
            .get_mut(&id)
            .unwrap()
            .motion
            .as_mut()
            .unwrap();
        motion.heading = motion.point.bearing(destination).unwrap().unwrap();
        motion.desired_heading = motion.heading;
        motion.speed = 0.0;
        plan.route = vec![
            GridHex::new(position.x, position.y),
            GridHex::new(aim.x, aim.y),
        ];
        drive_pending(
            &mut world,
            &config,
            id,
            &mut plan,
            None,
            None,
            100,
            2,
            &mut notices,
        );
        assert!(
            super::super::steering::motion(&world, id)
                .unwrap()
                .desired_speed
                > 0.0
        );
        assert!(
            plan.search.is_none(),
            "route continuation must not allocate a frontier"
        );
        assert_eq!(plan.route_index, 1);
        world
            .btech
            .constructed
            .get_mut(&id)
            .unwrap()
            .motion
            .as_mut()
            .unwrap()
            .speed = -1.0;
        assert!(
            super::super::steering::pursuit_score(
                &world,
                &config,
                id,
                aim,
                position,
                (0.0, 0.0),
                0.0,
                100.0,
                0.2,
                3,
                None,
            )
            .is_some()
        );
        assert!(
            super::super::steering::pursuit_score(
                &world,
                &config,
                id,
                position,
                position,
                (0.0, 0.0),
                0.0,
                100.0,
                0.2,
                3,
                None,
            )
            .is_some()
        );
    }
}
