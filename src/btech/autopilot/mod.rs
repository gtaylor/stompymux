//! Ground-unit autopilot intent and queue state.
//!
//! This module owns deterministic, serializable control state.  Navigation,
//! observations, and combat execution consume this state from later heartbeat
//! work; no path or world mutation is stored here.

mod adaptive_pursuit;
pub(crate) mod alignment;
pub mod benchmark;
pub mod combat_policy;
mod congestion;
pub mod diagnostics;
pub mod encounters;
pub(crate) mod interception;
pub mod navigation;
pub mod observations;
pub mod orders;
pub mod pursuit_encounters;
pub mod runtime;
pub(crate) mod traversal;

pub use orders::{
    AutopilotOrder, AutopilotOrderProgress, AutopilotOrderRecord, AutopilotOrderState,
    AutopilotRangeBand, MAX_PATROL_WAYPOINTS, MAX_QUEUED_ORDERS,
};
pub use runtime::AutopilotRuntimeMetrics;

use crate::{ObjectId, World, btech::BattlePosition};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_FEEDBACK: usize = 128;
const DEFAULT_SPEED_PERCENT: u8 = 100;
const DEFAULT_HEAT_CEILING: u16 = 8;
const DEFAULT_NEXT_ID: u64 = 1;

/// How an attached controller chooses weapons during an order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutopilotFireMode {
    Hold,
    AssignedTarget,
    Opportunistic,
}

impl Default for AutopilotFireMode {
    fn default() -> Self {
        Self::Hold
    }
}

impl AutopilotFireMode {
    pub const fn code(self) -> u8 {
        match self {
            Self::Hold => 0,
            Self::AssignedTarget => 1,
            Self::Opportunistic => 2,
        }
    }
}

/// Controller settings that survive a restart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutopilotConfig {
    #[serde(default = "default_speed_percent")]
    pub speed_percent: u8,
    #[serde(default)]
    pub fire_mode: AutopilotFireMode,
    #[serde(default = "default_heat_ceiling")]
    pub heat_ceiling: u16,
    #[serde(default)]
    pub preferred_range: Option<AutopilotRangeBand>,
}

const fn default_speed_percent() -> u8 {
    DEFAULT_SPEED_PERCENT
}

const fn default_heat_ceiling() -> u16 {
    DEFAULT_HEAT_CEILING
}

impl Default for AutopilotConfig {
    fn default() -> Self {
        Self {
            speed_percent: DEFAULT_SPEED_PERCENT,
            fire_mode: AutopilotFireMode::default(),
            heat_ceiling: DEFAULT_HEAT_CEILING,
            preferred_range: None,
        }
    }
}

impl AutopilotConfig {
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.speed_percent <= 100,
            "Autopilot speed percentage must be between 0 and 100"
        );
        ensure!(
            self.heat_ceiling <= 1000,
            "Autopilot heat ceiling is out of range"
        );
        if let Some(range) = self.preferred_range {
            range.validate()?;
        }
        Ok(())
    }
}

/// Partial settings update used by Lua and future tactical directors.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutopilotConfigPatch {
    #[serde(default)]
    pub speed_percent: Option<u8>,
    #[serde(default)]
    pub fire_mode: Option<AutopilotFireMode>,
    #[serde(default)]
    pub heat_ceiling: Option<u16>,
    /// `None` leaves the range unchanged; `Some(None)` clears it.
    #[serde(default)]
    pub preferred_range: Option<Option<AutopilotRangeBand>>,
}

impl AutopilotConfigPatch {
    fn apply_to(&self, config: &mut AutopilotConfig) {
        if let Some(value) = self.speed_percent {
            config.speed_percent = value;
        }
        if let Some(value) = self.fire_mode {
            config.fire_mode = value;
        }
        if let Some(value) = self.heat_ceiling {
            config.heat_ceiling = value;
        }
        if let Some(value) = self.preferred_range {
            config.preferred_range = value;
        }
    }
}

/// Controller lifecycle.  Paused and blocked controllers retain their intent.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutopilotState {
    Paused,
    Idle,
    Executing,
    Blocked,
}

impl Default for AutopilotState {
    fn default() -> Self {
        Self::Paused
    }
}

impl AutopilotState {
    pub const fn code(self) -> u8 {
        match self {
            Self::Paused => 0,
            Self::Idle => 1,
            Self::Executing => 2,
            Self::Blocked => 3,
        }
    }
}

/// Meaningful lifecycle or execution event returned to a tactical director.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutopilotFeedbackEvent {
    Configured,
    Paused,
    Resumed,
    ManualTakeover,
    OrderQueued,
    OrderStarted,
    OrderSucceeded,
    OrderFailed,
    OrderCanceled,
    Blocked,
}

/// A typed reason accompanies feedback where the event needs explanation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutopilotReason {
    ManualTakeover,
    ContactLost,
    Stuck,
    Unreachable,
    Invalidated,
    ResourceLimit,
    Congested,
    InvalidTarget,
    UnitUnavailable,
    MapChanged,
    Unsupported,
    StaleRevision,
}

/// One bounded, ordered event in a controller's outcome stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutopilotFeedback {
    pub sequence: u64,
    pub simulation_time: i64,
    pub order_id: Option<u64>,
    pub event: AutopilotFeedbackEvent,
    pub reason: Option<AutopilotReason>,
}

/// A page of feedback, including whether records older than the retained window exist.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutopilotFeedbackPage {
    pub records: Vec<AutopilotFeedback>,
    pub history_gap: bool,
}

/// Last sensor-confirmed location retained for a short tactical memory window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LastSighting {
    pub position: BattlePosition,
    pub seen_at: i64,
}

/// Descriptive alias for callers that prefer the subsystem-qualified name.
pub type AutopilotLastSighting = LastSighting;

/// Submission mode for a validated order batch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutopilotSubmissionMode {
    Append,
    Replace,
}

impl AutopilotSubmissionMode {
    pub const fn code(self) -> u8 {
        match self {
            Self::Append => 0,
            Self::Replace => 1,
        }
    }
}

/// A durable controller attached to one BattleTech unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AutopilotController {
    #[serde(default)]
    pub(crate) config: AutopilotConfig,
    #[serde(default)]
    pub(crate) state: AutopilotState,
    /// Last failure that currently blocks execution, if any.
    #[serde(default)]
    pub(crate) blocking_reason: Option<AutopilotReason>,
    #[serde(default)]
    pub(crate) revision: u64,
    #[serde(default = "default_next_id")]
    pub(crate) next_order_id: u64,
    #[serde(default)]
    pub(crate) active: Option<AutopilotOrderRecord>,
    #[serde(default)]
    pub(crate) queue: Vec<AutopilotOrderRecord>,
    #[serde(default)]
    pub(crate) feedback: Vec<AutopilotFeedback>,
    #[serde(default = "default_next_id")]
    pub(crate) next_feedback_sequence: u64,
    /// Sensor memory is durable intent state; paths and live observations are not.
    #[serde(default)]
    pub(crate) sightings: BTreeMap<ObjectId, LastSighting>,
}

const fn default_next_id() -> u64 {
    DEFAULT_NEXT_ID
}

impl Default for AutopilotController {
    fn default() -> Self {
        Self::new()
    }
}

impl AutopilotController {
    /// Create a paused controller with no pending intent.
    pub fn new() -> Self {
        Self {
            config: AutopilotConfig::default(),
            state: AutopilotState::Paused,
            blocking_reason: None,
            revision: 0,
            next_order_id: DEFAULT_NEXT_ID,
            active: None,
            queue: Vec::new(),
            feedback: Vec::new(),
            next_feedback_sequence: DEFAULT_NEXT_ID,
            sightings: BTreeMap::new(),
        }
    }

    pub fn with_config(config: AutopilotConfig) -> Result<Self> {
        config.validate()?;
        Ok(Self {
            config,
            ..Self::new()
        })
    }

    pub fn config(&self) -> &AutopilotConfig {
        &self.config
    }

    pub fn state(&self) -> AutopilotState {
        self.state
    }

    pub fn blocking_reason(&self) -> Option<AutopilotReason> {
        self.blocking_reason
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn active_order(&self) -> Option<&AutopilotOrderRecord> {
        self.active.as_ref()
    }

    pub fn queued_orders(&self) -> &[AutopilotOrderRecord] {
        &self.queue
    }

    pub fn feedback_records(&self) -> &[AutopilotFeedback] {
        &self.feedback
    }

    pub fn sightings(&self) -> &BTreeMap<ObjectId, LastSighting> {
        &self.sightings
    }

    pub fn order_count(&self) -> usize {
        self.queue.len() + usize::from(self.active.is_some())
    }

    /// Stamp management outcomes created since a sequence cursor with world simulation time.
    pub(crate) fn stamp_feedback(&mut self, first_sequence: u64, simulation_time: i64) {
        for record in &mut self.feedback {
            if record.sequence >= first_sequence {
                record.simulation_time = simulation_time;
            }
        }
    }

    /// Update a configuration patch, optionally guarding against a stale director.
    pub fn configure(
        &mut self,
        patch: AutopilotConfigPatch,
        expected_revision: Option<u64>,
    ) -> Result<()> {
        self.ensure_revision(expected_revision)?;
        let mut next = self.config.clone();
        patch.apply_to(&mut next);
        next.validate()?;
        if next != self.config {
            self.config = next;
            self.bump_revision()?;
            self.record_feedback(0, None, AutopilotFeedbackEvent::Configured, None);
        }
        Ok(())
    }

    /// Validate and submit a whole batch atomically.  IDs are allocated only after all
    /// orders and capacity have passed validation.
    pub fn submit(
        &mut self,
        orders: Vec<AutopilotOrder>,
        mode: AutopilotSubmissionMode,
        expected_revision: Option<u64>,
    ) -> Result<Vec<u64>> {
        self.ensure_revision(expected_revision)?;
        ensure!(
            !matches!(mode, AutopilotSubmissionMode::Append) || !orders.is_empty(),
            "Cannot append an empty autopilot order batch"
        );
        ensure!(
            orders.len() <= MAX_QUEUED_ORDERS,
            "Too many autopilot orders"
        );
        for order in &orders {
            order.validate()?;
        }
        let existing = match mode {
            AutopilotSubmissionMode::Append => self.order_count(),
            AutopilotSubmissionMode::Replace => 0,
        };
        ensure!(
            existing + orders.len() <= MAX_QUEUED_ORDERS,
            "Autopilot order queue is full"
        );

        let mut next_id = self.next_order_id;
        let mut records = Vec::with_capacity(orders.len());
        for order in orders {
            ensure!(next_id > 0, "Autopilot order ID exhausted");
            records.push(AutopilotOrderRecord::queued(next_id, order));
            next_id = next_id
                .checked_add(1)
                .context("Autopilot order ID exhausted")?;
        }

        if matches!(mode, AutopilotSubmissionMode::Replace) {
            self.blocking_reason = None;
            let canceled = self
                .active
                .take()
                .into_iter()
                .chain(self.queue.drain(..))
                .collect::<Vec<_>>();
            for record in canceled {
                self.record_feedback(
                    0,
                    Some(record.id),
                    AutopilotFeedbackEvent::OrderCanceled,
                    None,
                );
            }
        }
        let ids = records.iter().map(|record| record.id).collect::<Vec<_>>();
        self.queue.extend(records);
        self.next_order_id = next_id;
        if !self.queue.is_empty()
            && self.state != AutopilotState::Paused
            && !(self.state == AutopilotState::Blocked
                && matches!(mode, AutopilotSubmissionMode::Append))
        {
            self.state = AutopilotState::Executing;
        } else if self.queue.is_empty() && self.active.is_none() {
            self.state = if self.state == AutopilotState::Paused {
                AutopilotState::Paused
            } else {
                AutopilotState::Idle
            };
        }
        self.bump_revision()?;
        for id in &ids {
            self.record_feedback(0, Some(*id), AutopilotFeedbackEvent::OrderQueued, None);
        }
        Ok(ids)
    }

    /// Cancel one active or queued order.
    pub fn cancel(&mut self, order_id: u64, expected_revision: Option<u64>) -> Result<bool> {
        self.ensure_revision(expected_revision)?;
        let mut found = false;
        if self
            .active
            .as_ref()
            .is_some_and(|record| record.id == order_id)
        {
            let mut record = self.active.take().expect("active order just checked");
            record.state = AutopilotOrderState::Canceled;
            found = true;
        } else if let Some(index) = self.queue.iter().position(|record| record.id == order_id) {
            let mut record = self.queue.remove(index);
            record.state = AutopilotOrderState::Canceled;
            found = true;
        }
        if found {
            self.record_feedback(
                0,
                Some(order_id),
                AutopilotFeedbackEvent::OrderCanceled,
                None,
            );
            self.bump_revision()?;
            if self.active.is_none() {
                self.blocking_reason = None;
            }
            if self.active.is_none() && self.state != AutopilotState::Paused {
                self.state = if self.queue.is_empty() {
                    AutopilotState::Idle
                } else {
                    AutopilotState::Executing
                };
            }
        }
        Ok(found)
    }

    pub fn pause(&mut self, expected_revision: Option<u64>) -> Result<()> {
        self.ensure_revision(expected_revision)?;
        if self.state != AutopilotState::Paused {
            self.state = AutopilotState::Paused;
            self.bump_revision()?;
            self.record_feedback(0, None, AutopilotFeedbackEvent::Paused, None);
        }
        Ok(())
    }

    pub fn resume(&mut self, expected_revision: Option<u64>) -> Result<()> {
        self.ensure_revision(expected_revision)?;
        if self.state == AutopilotState::Paused || self.state == AutopilotState::Blocked {
            if let Some(active) = self.active.as_mut() {
                active.state = AutopilotOrderState::Running;
            }
            self.state = if self.active.is_some() || !self.queue.is_empty() {
                AutopilotState::Executing
            } else {
                AutopilotState::Idle
            };
            self.blocking_reason = None;
            self.bump_revision()?;
            self.record_feedback(0, None, AutopilotFeedbackEvent::Resumed, None);
        }
        Ok(())
    }

    /// Start the next queued order.  Heartbeat code calls this after checking that
    /// the unit is still placed and mechanically eligible.
    pub(crate) fn start_next(&mut self, simulation_time: i64) -> Option<u64> {
        if self.state != AutopilotState::Executing || self.active.is_some() {
            return None;
        }
        let mut record = self.queue.first().cloned()?;
        self.queue.remove(0);
        record.state = AutopilotOrderState::Running;
        let id = record.id;
        self.active = Some(record);
        self.record_feedback(
            simulation_time,
            Some(id),
            AutopilotFeedbackEvent::OrderStarted,
            None,
        );
        Some(id)
    }

    /// Mark the active order complete and let the next tick start the queue successor.
    pub(crate) fn complete_active(&mut self, simulation_time: i64) -> Option<u64> {
        let mut record = self.active.take()?;
        record.state = AutopilotOrderState::Succeeded;
        let id = record.id;
        self.record_feedback(
            simulation_time,
            Some(id),
            AutopilotFeedbackEvent::OrderSucceeded,
            None,
        );
        if self.queue.is_empty() {
            self.state = AutopilotState::Idle;
        } else {
            self.state = AutopilotState::Executing;
        }
        Some(id)
    }

    /// Fail the active order and retain it for an explicit `resume` retry.
    pub(crate) fn fail_active(
        &mut self,
        simulation_time: i64,
        reason: AutopilotReason,
    ) -> Option<u64> {
        let record = self.active.as_mut()?;
        record.state = AutopilotOrderState::Failed;
        let id = record.id;
        self.state = AutopilotState::Blocked;
        self.blocking_reason = Some(reason);
        self.record_feedback(
            simulation_time,
            Some(id),
            AutopilotFeedbackEvent::OrderFailed,
            Some(reason),
        );
        self.record_feedback(
            simulation_time,
            Some(id),
            AutopilotFeedbackEvent::Blocked,
            Some(reason),
        );
        Some(id)
    }

    pub(crate) fn active_order_mut(&mut self) -> Option<&mut AutopilotOrderRecord> {
        self.active.as_mut()
    }

    /// Merge freshly observed positions into the bounded tactical memory.
    pub(crate) fn update_sightings(
        &mut self,
        observations: impl IntoIterator<Item = (ObjectId, BattlePosition)>,
        seen_at: i64,
    ) {
        self.sightings.extend(
            observations
                .into_iter()
                .map(|(unit, position)| (unit, LastSighting { position, seen_at })),
        );
    }

    /// Remove sightings older than the caller's configured memory horizon.
    pub(crate) fn expire_sightings(&mut self, before: i64) {
        self.sightings
            .retain(|_, sighting| sighting.seen_at >= before);
    }

    pub(crate) fn record_feedback(
        &mut self,
        simulation_time: i64,
        order_id: Option<u64>,
        event: AutopilotFeedbackEvent,
        reason: Option<AutopilotReason>,
    ) -> u64 {
        let sequence = self.next_feedback_sequence;
        self.next_feedback_sequence = self.next_feedback_sequence.saturating_add(1).max(1);
        self.feedback.push(AutopilotFeedback {
            sequence,
            simulation_time,
            order_id,
            event,
            reason,
        });
        if self.feedback.len() > MAX_FEEDBACK {
            self.feedback.remove(0);
        }
        sequence
    }

    pub fn feedback_since(&self, after_sequence: Option<u64>) -> AutopilotFeedbackPage {
        let Some(after) = after_sequence else {
            return AutopilotFeedbackPage {
                records: self.feedback.clone(),
                history_gap: false,
            };
        };
        let history_gap = self
            .feedback
            .first()
            .is_some_and(|first| after.saturating_add(1) < first.sequence);
        AutopilotFeedbackPage {
            records: self
                .feedback
                .iter()
                .copied()
                .filter(|feedback| feedback.sequence > after)
                .collect(),
            history_gap,
        }
    }

    /// Pause after successful player control.  Rejected controls must not call this.
    pub(crate) fn manual_takeover(&mut self, simulation_time: i64) -> bool {
        if !matches!(self.state, AutopilotState::Executing | AutopilotState::Idle) {
            return false;
        }
        self.state = AutopilotState::Paused;
        self.blocking_reason = None;
        self.record_feedback(
            simulation_time,
            self.active.as_ref().map(|record| record.id),
            AutopilotFeedbackEvent::ManualTakeover,
            Some(AutopilotReason::ManualTakeover),
        );
        true
    }

    pub(crate) fn validate(&self) -> Result<()> {
        self.config.validate()?;
        ensure!(self.next_order_id > 0, "Autopilot next order ID is invalid");
        ensure!(
            self.state != AutopilotState::Blocked || self.blocking_reason.is_some(),
            "Blocked autopilot lacks a reason"
        );
        ensure!(
            self.order_count() <= MAX_QUEUED_ORDERS,
            "Autopilot order queue exceeds capacity"
        );
        ensure!(
            self.state != AutopilotState::Idle || self.order_count() == 0,
            "Idle autopilot retains pending orders"
        );
        ensure!(
            self.state != AutopilotState::Executing || self.order_count() > 0,
            "Executing autopilot has no pending order"
        );
        if let Some(active) = &self.active {
            active.validate(true)?;
        }
        let mut ids = BTreeSet::new();
        if let Some(active) = &self.active {
            ensure!(ids.insert(active.id), "Duplicate autopilot order ID");
        }
        for queued in &self.queue {
            queued.validate(false)?;
            ensure!(ids.insert(queued.id), "Duplicate autopilot order ID");
        }
        ensure!(
            ids.iter().all(|id| *id < self.next_order_id),
            "Autopilot next order ID is behind its queue"
        );
        ensure!(
            self.feedback.len() <= MAX_FEEDBACK && self.next_feedback_sequence > 0,
            "Autopilot feedback history is invalid"
        );
        let mut previous = 0;
        for feedback in &self.feedback {
            ensure!(
                feedback.sequence > previous,
                "Autopilot feedback is out of order"
            );
            previous = feedback.sequence;
        }
        ensure!(
            previous < self.next_feedback_sequence,
            "Autopilot feedback sequence is not monotonic"
        );
        ensure!(
            self.sightings
                .values()
                .all(|sighting| sighting.position.map.0 >= 0),
            "Autopilot sighting references an invalid map"
        );
        Ok(())
    }

    fn ensure_revision(&self, expected_revision: Option<u64>) -> Result<()> {
        if let Some(expected) = expected_revision {
            ensure!(
                expected == self.revision,
                "Autopilot revision mismatch: expected {expected}, current {}",
                self.revision
            );
        }
        Ok(())
    }

    fn bump_revision(&mut self) -> Result<()> {
        self.revision = self
            .revision
            .checked_add(1)
            .context("Autopilot revision exhausted")?;
        Ok(())
    }
}

/// Pause a controller after an already-admitted player action.
pub(crate) fn manual_takeover(world: &mut World, id: ObjectId) -> Result<bool> {
    let simulation_time = world.btech.simulation_time();
    let Some(controller) = std::sync::Arc::make_mut(&mut world.btech.controllers).get_mut(&id)
    else {
        return Ok(false);
    };
    let paused = controller.manual_takeover(simulation_time);
    if paused {
        std::sync::Arc::make_mut(&mut world.btech.autopilot_plans).remove(&id);
    }
    Ok(paused)
}

/// Validate controller state independently of world references.
pub(crate) fn validate_controllers(
    controllers: &std::collections::BTreeMap<ObjectId, AutopilotController>,
) -> Result<()> {
    for (unit, controller) in controllers {
        ensure!(
            unit.0 >= 0,
            "Autopilot controller references an invalid unit"
        );
        controller.validate()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn position(map: i64, x: u16, y: u16) -> BattlePosition {
        BattlePosition {
            map: ObjectId(map),
            x,
            y,
        }
    }

    fn move_order(x: u16) -> AutopilotOrder {
        AutopilotOrder::Move {
            destination: position(1, x, 2),
            arrival_radius: 0,
        }
    }

    #[test]
    fn new_controller_uses_safe_defaults_and_is_paused() {
        let controller = AutopilotController::new();
        assert_eq!(controller.state(), AutopilotState::Paused);
        assert_eq!(controller.config(), &AutopilotConfig::default());
        assert_eq!(controller.revision(), 0);
        assert!(controller.validate().is_ok());
    }

    #[test]
    fn replacement_is_atomic_and_allocates_monotonic_ids() {
        let mut controller = AutopilotController::new();
        controller
            .resume(None)
            .expect("resume empty controller succeeds");
        let first = controller
            .submit(
                vec![move_order(1), AutopilotOrder::Hold],
                AutopilotSubmissionMode::Append,
                None,
            )
            .expect("first batch accepted");
        assert_eq!(first, vec![1, 2]);
        assert_eq!(controller.queued_orders().len(), 2);
        let revision = controller.revision();
        let invalid = controller.submit(
            vec![AutopilotOrder::Patrol { waypoints: vec![] }],
            AutopilotSubmissionMode::Replace,
            Some(revision),
        );
        assert!(invalid.is_err());
        assert_eq!(controller.revision(), revision);
        assert_eq!(controller.queued_orders().len(), 2);
        let second = controller
            .submit(
                vec![move_order(3)],
                AutopilotSubmissionMode::Replace,
                Some(revision),
            )
            .expect("replacement accepted");
        assert_eq!(second, vec![3]);
        assert_eq!(controller.queued_orders()[0].id, 3);
        assert!(controller.validate().is_ok());
    }

    #[test]
    fn start_complete_and_cancel_preserve_order_lifecycle() {
        let mut controller = AutopilotController::new();
        controller.resume(None).expect("resume succeeds");
        let ids = controller
            .submit(
                vec![move_order(1), move_order(2)],
                AutopilotSubmissionMode::Append,
                None,
            )
            .expect("submit succeeds");
        assert_eq!(controller.start_next(10), Some(ids[0]));
        assert_eq!(
            controller.active_order().map(|record| record.state),
            Some(AutopilotOrderState::Running)
        );
        assert_eq!(controller.complete_active(11), Some(ids[0]));
        assert_eq!(controller.start_next(12), Some(ids[1]));
        assert!(controller.cancel(ids[1], None).expect("cancel succeeds"));
        assert!(controller.active_order().is_none());
        assert_eq!(controller.state(), AutopilotState::Idle);
        assert!(
            controller
                .feedback_records()
                .iter()
                .any(|feedback| feedback.event == AutopilotFeedbackEvent::OrderSucceeded)
        );
    }

    #[test]
    fn failure_blocks_and_resume_retries_same_order() {
        let mut controller = AutopilotController::new();
        controller.resume(None).expect("resume succeeds");
        controller
            .submit(vec![move_order(1)], AutopilotSubmissionMode::Append, None)
            .expect("submit succeeds");
        controller.start_next(1);
        controller.fail_active(2, AutopilotReason::Stuck);
        assert_eq!(controller.state(), AutopilotState::Blocked);
        assert_eq!(controller.blocking_reason(), Some(AutopilotReason::Stuck));
        let id = controller.active_order().expect("failed order retained").id;
        controller
            .submit(vec![move_order(2)], AutopilotSubmissionMode::Append, None)
            .expect("append behind failed order succeeds");
        assert_eq!(controller.state(), AutopilotState::Blocked);
        controller.resume(None).expect("resume retries");
        assert_eq!(controller.state(), AutopilotState::Executing);
        assert_eq!(controller.blocking_reason(), None);
        assert_eq!(controller.active_order().expect("active order").id, id);
        assert_eq!(
            controller.active_order().expect("active order").state,
            AutopilotOrderState::Running
        );
    }

    #[test]
    fn feedback_page_reports_history_gap_after_bounded_eviction() {
        let mut controller = AutopilotController::new();
        for _ in 0..(MAX_FEEDBACK + 4) {
            controller.record_feedback(0, None, AutopilotFeedbackEvent::Configured, None);
        }
        let page = controller.feedback_since(Some(1));
        assert!(page.history_gap);
        assert_eq!(page.records.len(), MAX_FEEDBACK);
        assert!(controller.validate().is_ok());
    }

    #[test]
    fn canceling_failed_order_unblocks_queued_work() {
        let mut controller = AutopilotController::new();
        controller.resume(None).unwrap();
        let ids = controller
            .submit(
                vec![move_order(1), move_order(2)],
                AutopilotSubmissionMode::Append,
                None,
            )
            .unwrap();
        controller.start_next(1);
        controller.fail_active(2, AutopilotReason::Stuck);
        assert_eq!(controller.state(), AutopilotState::Blocked);
        assert!(controller.cancel(ids[0], None).unwrap());
        assert_eq!(controller.state(), AutopilotState::Executing);
        assert_eq!(controller.blocking_reason(), None);
        assert_eq!(controller.start_next(3), Some(ids[1]));
    }

    #[test]
    fn stale_revision_does_not_mutate_configuration_or_queue() {
        let mut controller = AutopilotController::new();
        controller
            .configure(
                AutopilotConfigPatch {
                    speed_percent: Some(75),
                    ..Default::default()
                },
                None,
            )
            .expect("configuration succeeds");
        let revision = controller.revision();
        assert!(
            controller
                .configure(
                    AutopilotConfigPatch {
                        heat_ceiling: Some(4),
                        ..Default::default()
                    },
                    Some(revision - 1),
                )
                .is_err()
        );
        assert_eq!(controller.config().heat_ceiling, DEFAULT_HEAT_CEILING);
    }

    #[test]
    fn successful_manual_control_pauses_idle_autonomous_fire() {
        let mut controller = AutopilotController::new();
        controller
            .configure(
                AutopilotConfigPatch {
                    fire_mode: Some(AutopilotFireMode::Opportunistic),
                    ..Default::default()
                },
                None,
            )
            .unwrap();
        controller.resume(None).unwrap();
        assert_eq!(controller.state(), AutopilotState::Idle);
        assert!(controller.manual_takeover(42));
        assert_eq!(controller.state(), AutopilotState::Paused);
        assert_eq!(
            controller.feedback_records().last().unwrap().event,
            AutopilotFeedbackEvent::ManualTakeover
        );
    }
}

pub(crate) mod engagement;
pub(crate) mod steering;

/// Multi-participant deterministic adversarial movement diagnostics.
pub mod adversarial;
