//! Explicit cockpit destinations compose with registered stations through one nonrecursive audience.
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeSet, sync::Arc};

/// Three named destinations retain deferred references independently of station ownership.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct CockpitLinks(pub [ObjectId; 3]);

impl Default for CockpitLinks {
    fn default() -> Self {
        Self([ObjectId(-1); 3])
    }
}

/// Read a unit's saved destinations without resolving or repairing deferred references.
pub(super) fn links(world: &World, id: ObjectId) -> CockpitLinks {
    world.btech.constructed_units().get(&id).map_or_else(
        || {
            world
                .btech
                .vehicles()
                .get(&id)
                .map_or_else(CockpitLinks::default, |unit| unit.cockpit_links)
        },
        |unit| unit.cockpit_links,
    )
}

/// Assign an explicit slot; the caller owns wizard admission and the transaction boundary.
pub(super) fn set(
    world: &mut World,
    id: ObjectId,
    slot: usize,
    destination: ObjectId,
) -> Result<()> {
    let links = if let Some(unit) = Arc::make_mut(&mut world.btech.constructed).get_mut(&id) {
        &mut unit.cockpit_links
    } else {
        &mut Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .context("Unit is unavailable")?
            .cockpit_links
    };
    *links
        .0
        .get_mut(slot)
        .context("Invalid cockpit destination slot")? = destination;
    Ok(())
}

/// Resolve live room audiences once, excluding the source and duplicate explicit or registered links.
pub(super) fn audiences(world: &World, parent: ObjectId) -> BTreeSet<ObjectId> {
    links(world, parent)
        .0
        .into_iter()
        .chain(
            world
                .btech
                .gunner_stations()
                .iter()
                .filter_map(|(id, station)| (station.parent == parent).then_some(*id)),
        )
        .filter(|id| {
            id.0 > 0
                && *id != parent
                && world.objects.get(id).is_some_and(|object| {
                    matches!(object.kind, Kind::Thing | Kind::Room)
                        && !object.flags.contains(Flag::Going)
                })
        })
        .collect()
}
