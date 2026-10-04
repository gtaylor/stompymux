//! Sensor-limited, detached observations used by Lua and autonomous decisions.

use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

use super::super::{Heat, Position, Power, WeaponReadiness};
use super::LastSighting;

/// One currently acquired contact. Its fields come only from the existing sensor view.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AutopilotContact {
    /// Unit identity acquired by the observer.
    pub unit: ObjectId,
    /// Position at the observation tick.
    pub position: Position,
    /// Whether the unit is allied with the observer.
    pub friendly: bool,
    /// Whether sensors have identified the contact well enough to determine allegiance.
    pub identified: bool,
    /// Destruction is known only when the existing clear-contact rules reveal it.
    pub known_destroyed: bool,
    /// Observed range in map units.
    pub range: f64,
    /// Shared aiming distance from the unit's active C3/C3i network; absent when unnetworked.
    /// It improves the range band only: physical reach and minimum range still use `range`.
    pub network_range: Option<f64>,
    /// Seen only by network peers. The unit can pursue and face it but cannot lock or fire
    /// on it until its own sensors acquire it.
    pub relayed: bool,
    /// Observation time, in simulation seconds.
    pub seen_at: i64,
}

impl AutopilotContact {
    /// Distance that decides the weapon range band: the network's when available.
    pub fn aiming_range(&self) -> f64 {
        self.network_range.unwrap_or(self.range)
    }
}

/// A detached tactical picture for one controller.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AutopilotObservation {
    /// Observing unit.
    pub unit: ObjectId,
    /// Current simulation time.
    pub time: i64,
    /// Own position, if placed.
    pub position: Option<Position>,
    /// Own current heading, if motion is available.
    pub heading: Option<f64>,
    /// Own current speed.
    pub speed: f64,
    /// Own mechanical and weapon readiness, never inferred from an enemy contact.
    pub own: AutopilotOwnReadiness,
    /// Currently acquired contacts, plus those relayed by active C3/C3i peers; no hidden
    /// battlefield state is included.
    pub contacts: Vec<AutopilotContact>,
    /// Previously acquired enemies whose last sighting has not expired.
    pub remembered: Vec<AutopilotMemory>,
}

/// Capabilities available to a unit's own in-game tactical director.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AutopilotOwnReadiness {
    pub power: Power,
    pub maximum_speed: f64,
    /// BattleMech thermal state; ground vehicles have no conventional overheat meter.
    pub heat: Option<Heat>,
    pub weapons: Vec<WeaponReadiness>,
}

/// A location the unit saw earlier, without current enemy state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct AutopilotMemory {
    pub unit: ObjectId,
    pub position: Position,
    pub seen_at: i64,
}

/// Capture the existing sensor view without scanning, rolling dice, or changing world state.
pub fn observe(world: &World, unit: ObjectId, time: i64) -> Result<AutopilotObservation> {
    observe_with_memory(world, unit, time, &BTreeMap::new())
}

/// Include only fresh retained sightings; current contacts supersede memory.
pub fn observe_with_memory(
    world: &World,
    unit: ObjectId,
    time: i64,
    sightings: &BTreeMap<ObjectId, LastSighting>,
) -> Result<AutopilotObservation> {
    let own = super::super::scanner::scanner_unit(world, unit).context("Unit is unavailable")?;
    // The ordinary contact display requires a running, placed observer because
    // it performs perception geometry.  An attached controller is also observable
    // while it is starting, shut down, or being restored before placement.  In
    // those states expose the own-unit readiness below and no current contacts;
    // never turn a display precondition failure into a controller failure.
    let contacts = if own.power == Power::Running && own.position.is_some() {
        let mut contacts = super::super::network_contacts::networked_contact_facts(world, unit)
            .unwrap_or_default()
            .into_iter()
            .map(|contact| AutopilotContact {
                unit: contact.facts.target,
                position: contact.facts.position,
                friendly: contact.facts.friendly,
                identified: contact.facts.identified,
                known_destroyed: contact.facts.known_destroyed,
                range: contact.facts.range.spatial,
                network_range: contact.network_range,
                relayed: contact.relayed,
                seen_at: time,
            })
            .collect::<Vec<_>>();
        contacts.sort_by(|left, right| {
            left.range
                .total_cmp(&right.range)
                .then(left.unit.cmp(&right.unit))
        });
        contacts
    } else {
        Vec::new()
    };
    let current: BTreeSet<_> = contacts.iter().map(|contact| contact.unit).collect();
    let remembered = sightings
        .iter()
        .filter(|(target, sighting)| {
            sighting.seen_at <= time
                && time.saturating_sub(sighting.seen_at) <= 30
                && own
                    .position
                    .is_some_and(|position| position.map == sighting.position.map)
                && !current.contains(target)
        })
        .map(|(&unit, sighting)| AutopilotMemory {
            unit,
            position: sighting.position,
            seen_at: sighting.seen_at,
        })
        .collect();
    let own_readiness = if let Some(mech) = world.btech.constructed_units().get(&unit) {
        AutopilotOwnReadiness {
            power: mech.power(),
            maximum_speed: mech.mobility().maximum_speed,
            heat: Some(mech.heat()),
            weapons: mech.weapon_readiness_batch().unwrap_or_default(),
        }
    } else {
        let vehicle = world
            .btech
            .vehicles()
            .get(&unit)
            .context("Unit is unavailable")?;
        AutopilotOwnReadiness {
            power: vehicle.power(),
            maximum_speed: vehicle.maximum_speed(),
            heat: None,
            weapons: vehicle.weapon_readiness_batch().unwrap_or_default(),
        }
    };
    Ok(AutopilotObservation {
        unit,
        time,
        position: own.position,
        heading: own.heading,
        speed: own.speed,
        own: own_readiness,
        contacts,
        remembered,
    })
}
