//! Detached tactical observations and atomic intentions over the ground autopilot.

use super::autopilot::observations::{AutopilotObservation, observe_with_memory};
use super::autopilot::{
    AutopilotController, AutopilotFeedbackPage, AutopilotOrder, AutopilotSubmissionMode,
};
use super::{BattlePosition, BattleVehicleMovement};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

/// Maximum explicitly assigned units in a tactical request.
pub const MAX_TACTICAL_UNITS: usize = 100;

/// One controller's detached observation, intent and outcomes.
#[derive(Debug, Clone, Serialize)]
pub struct TacticalUnitSnapshot {
    /// Unit identity in the trusted in-game object namespace.
    pub unit: ObjectId,
    /// Current controller management revision.
    pub revision: u64,
    /// Detached intent and lifecycle state, with raw sighting caches removed.
    pub status: AutopilotController,
    /// Only the unit’s own readiness and permitted sensor view.
    pub observation: AutopilotObservation,
    /// Bounded outcomes following the requested cursor.
    pub feedback: AutopilotFeedbackPage,
}

/// An individual source of current or remembered intelligence.
#[derive(Debug, Clone, Serialize)]
pub struct TacticalSighting {
    /// Assigned friendly unit that acquired this intelligence.
    pub observer: ObjectId,
    /// Position at the time of observation.
    pub position: BattlePosition,
    /// Committed simulation second of acquisition.
    pub seen_at: i64,
    /// Whether this source currently acquires the contact.
    pub current: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Current observed allegiance; absent for remembered locations.
    pub friendly: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Current identification state; absent for remembered locations.
    pub identified: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// Destruction reported by current sensors, never inferred from lost contact.
    pub known_destroyed: Option<bool>,
}

/// Intelligence about one contact, retaining every observer's provenance.
#[derive(Debug, Clone, Serialize)]
pub struct TacticalContact {
    /// Unit identity in the trusted in-game object namespace.
    pub unit: ObjectId,
    /// Individual sources retained in ascending observer order.
    pub observations: Vec<TacticalSighting>,
}

/// Versioned tactical input. It contains no world handles or hidden enemy fields.
#[derive(Debug, Clone, Serialize)]
pub struct TacticalSnapshot {
    /// Contract version, currently one.
    pub version: u16,
    /// Shared committed simulation second.
    pub time: i64,
    /// Assigned controllers in ascending unit order.
    pub units: Vec<TacticalUnitSnapshot>,
    /// Aggregated contacts in ascending identity order.
    pub contacts: Vec<TacticalContact>,
}

/// A revision-guarded use of the ordinary controller order engine.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TacticalIntention {
    /// Unit identity in the trusted in-game object namespace.
    pub unit: ObjectId,
    /// Required revision guard checked before any batch mutation.
    pub expected_revision: u64,
    /// Ordinary append or replace semantics.
    pub mode: AutopilotSubmissionMode,
    /// At most 64 ordinary unit-level orders.
    pub orders: Vec<AutopilotOrder>,
}

/// The IDs allocated to a successfully admitted intention.
#[derive(Debug, Clone, Serialize)]
pub struct TacticalSubmitResult {
    /// Unit identity in the trusted in-game object namespace.
    pub unit: ObjectId,
    /// Newly allocated monotonically increasing order IDs.
    pub ids: Vec<u64>,
    /// Current controller management revision.
    pub revision: u64,
}

/// Require distinct supported controllers belonging to one friendly force and map.
fn validate_members(world: &World, units: &[ObjectId]) -> Result<Vec<ObjectId>> {
    ensure!(
        (1..=MAX_TACTICAL_UNITS).contains(&units.len()),
        "Tactical requests require 1 to 100 units"
    );
    let mut seen = BTreeSet::new();
    let mut group = None;
    for &id in units {
        ensure!(seen.insert(id), "Duplicate tactical unit");
        ensure!(
            world.btech.controllers().contains_key(&id),
            "No autopilot is attached to this unit"
        );
        ensure!(
            world.btech.constructed_units().contains_key(&id)
                || world.btech.vehicles().get(&id).is_some_and(|v| matches!(
                    v.definition().movement,
                    BattleVehicleMovement::Tracked
                        | BattleVehicleMovement::Wheeled
                        | BattleVehicleMovement::Hover
                )),
            "Tactical control requires a ground unit"
        );
        let unit = super::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
        let position = unit
            .position
            .context("Tactical unit is not on a battlefield")?;
        let membership = (position.map, unit.signature.team);
        ensure!(
            group.is_none_or(|group| group == membership),
            "Tactical units must be friendly and on one map"
        );
        group = Some(membership);
    }
    Ok(seen.into_iter().collect())
}

/// Observe only the explicitly assigned friendly units and the intelligence they have acquired.
pub fn observe_tactical(
    world: &World,
    units: &[ObjectId],
    cursors: &BTreeMap<ObjectId, u64>,
) -> Result<TacticalSnapshot> {
    let units = validate_members(world, units)?;
    ensure!(
        cursors.keys().all(|id| units.binary_search(id).is_ok()),
        "Feedback cursor is outside the assigned units"
    );
    let time = world.btech.simulation_time();
    let mut snapshots = Vec::with_capacity(units.len());
    let mut contacts = BTreeMap::<ObjectId, Vec<TacticalSighting>>::new();
    for id in units {
        let controller = &world.btech.controllers()[&id];
        let observation = observe_with_memory(world, id, time, controller.sightings())?;
        for contact in &observation.contacts {
            contacts
                .entry(contact.unit)
                .or_default()
                .push(TacticalSighting {
                    observer: id,
                    position: contact.position,
                    seen_at: contact.seen_at,
                    current: true,
                    friendly: Some(contact.friendly),
                    identified: Some(contact.identified),
                    known_destroyed: Some(contact.known_destroyed),
                });
        }
        for memory in &observation.remembered {
            contacts
                .entry(memory.unit)
                .or_default()
                .push(TacticalSighting {
                    observer: id,
                    position: memory.position,
                    seen_at: memory.seen_at,
                    current: false,
                    friendly: None,
                    identified: None,
                    known_destroyed: None,
                });
        }
        let mut status = controller.clone();
        // Raw controller memory can contain expired fields before its next active tick.
        // The public tactical snapshot exposes only the filtered observation memory.
        status.sightings.clear();
        snapshots.push(TacticalUnitSnapshot {
            unit: id,
            revision: controller.revision(),
            status,
            feedback: controller.feedback_since(cursors.get(&id).copied()),
            observation,
        });
    }
    Ok(TacticalSnapshot {
        version: 1,
        time,
        units: snapshots,
        contacts: contacts
            .into_iter()
            .map(|(unit, observations)| TacticalContact { unit, observations })
            .collect(),
    })
}

/// Validate all intentions before publishing any state, even when Lua catches an error.
/// Does not configure or resume a controller, or grant shared sensor acquisition.
pub fn submit_tactical(
    world: &mut World,
    intentions: &[TacticalIntention],
) -> Result<Vec<TacticalSubmitResult>> {
    validate_members(
        world,
        &intentions
            .iter()
            .map(|intent| intent.unit)
            .collect::<Vec<_>>(),
    )?;
    let mut candidates = Vec::with_capacity(intentions.len());
    let mut results = Vec::with_capacity(intentions.len());
    for intent in intentions {
        ensure!(intent.orders.len() <= 64, "Too many autopilot orders");
        for order in &intent.orders {
            super::autopilot::orders::validate_for_unit(world, intent.unit, order)?;
        }
        let mut controller = world.btech.controllers()[&intent.unit].clone();
        let sequence = controller.next_feedback_sequence;
        let ids = controller.submit(
            intent.orders.clone(),
            intent.mode,
            Some(intent.expected_revision),
        )?;
        controller.stamp_feedback(sequence, world.btech.simulation_time());
        results.push(TacticalSubmitResult {
            unit: intent.unit,
            ids,
            revision: controller.revision(),
        });
        candidates.push((intent.unit, controller));
    }
    // All fallible validation is above this boundary.
    for (id, controller) in candidates {
        let empty = controller.order_count() == 0;
        Arc::make_mut(&mut world.btech.controllers).insert(id, controller);
        Arc::make_mut(&mut world.btech.autopilot_plans).remove(&id);
        if empty {
            let _ = super::motion::set_speed_autopilot(world, id, 0.0);
        }
    }
    Ok(results)
}
