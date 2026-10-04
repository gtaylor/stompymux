//! Persisted battlefield membership order with first-vacant-slot reuse.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeSet;

impl BattleUnit {
    /// The unit's position in its current battlefield membership list.
    pub fn map_slot(&self) -> Option<u32> {
        self.map_slot
    }
}

/// Mech membership in common slot order, including destroyed and pending-removal units.
pub fn map_unit_order(world: &World, map: ObjectId) -> Result<Vec<ObjectId>> {
    Ok(all_unit_order(world, map)?
        .into_iter()
        .filter(|id| world.btech.constructed_units().contains_key(id))
        .collect())
}

/// Same-map placement retains membership; entry takes the lowest unoccupied slot.
pub(super) fn placement_slot(world: &World, id: ObjectId, map: ObjectId) -> Result<u32> {
    let memberships = world
        .btech
        .constructed_units()
        .iter()
        .map(|(&id, unit)| (id, unit.position(), unit.map_slot()))
        .chain(
            world
                .btech
                .vehicles()
                .iter()
                .map(|(&id, unit)| (id, unit.position(), unit.map_slot())),
        );
    let mut occupied = BTreeSet::new();
    let mut existing = None;
    for (member, position, slot) in memberships {
        if position.is_none_or(|position| position.map != map) {
            continue;
        }
        let slot = slot.context("Placed unit lacks a map slot")?;
        ensure!(occupied.insert(slot), "Duplicate battlefield slot");
        if member == id {
            existing = Some(slot);
        }
    }
    if let Some(slot) = existing {
        return Ok(slot);
    }
    (0..=u32::try_from(occupied.len())?)
        .find(|slot| !occupied.contains(slot))
        .context("No battlefield slot available")
}

/// Retain same-map identity and avoid collisions between authored IDs and membership-derived labels.
pub(super) fn placement_label(
    world: &World,
    id: ObjectId,
    map: ObjectId,
    slot: u32,
) -> Result<Option<String>> {
    let current = super::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    let preferred = if current.position.is_some_and(|position| position.map == map) {
        current.label().context("Placed unit lacks an ID")?
    } else {
        super::observer::battlefield_label(slot)
    };
    let used = super::battlefield_identity::occupied_labels(world, map, id);
    let label = if !used.contains(&preferred) {
        preferred
    } else {
        (0..=u32::try_from(used.len())?)
            .map(super::observer::battlefield_label)
            .find(|label| !used.contains(label))
            .context("No battlefield ID available")?
    };
    Ok((label != super::observer::battlefield_label(slot)).then_some(label))
}

/// Mixed construction membership ordered by the one persisted battlefield slot space.
pub(super) fn all_unit_order(world: &World, map: ObjectId) -> Result<Vec<ObjectId>> {
    ensure!(world.btech.maps().contains_key(&map), "Map not found");
    let mut slots = Vec::new();
    for id in super::scanner::scanner_ids(world) {
        let unit = super::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
        if unit.position.is_some_and(|position| position.map == map) {
            slots.push((unit.slot.context("Placed unit lacks a map slot")?, id));
        }
    }
    slots.sort_unstable();
    ensure!(
        slots.windows(2).all(|pair| pair[0].0 != pair[1].0),
        "Duplicate battlefield slot"
    );
    Ok(slots.into_iter().map(|(_, id)| id).collect())
}

/// Occupants retain slot order; callers choose their own visibility and admission policy.
pub(super) fn hex_occupants(
    world: &World,
    map: ObjectId,
    hex: HexCoordinate,
) -> Result<Vec<ObjectId>> {
    world
        .btech
        .maps()
        .get(&map)
        .context("Map not found")?
        .hex(i64::from(hex.x), i64::from(hex.y))?;
    Ok(all_unit_order(world, map)?
        .into_iter()
        .filter(|id| {
            super::scanner::scanner_unit(world, *id)
                .and_then(|unit| unit.position)
                .is_some_and(|position| {
                    i32::from(position.x) == hex.x && i32::from(position.y) == hex.y
                })
        })
        .collect())
}

/// Saved allocated span may include vacant tail slots; live members establish its minimum extent.
pub(super) fn extent(state: &BtechState, map: ObjectId) -> u32 {
    let occupied = state
        .constructed_units()
        .values()
        .filter_map(|unit| {
            unit.position()
                .filter(|p| p.map == map)
                .and(unit.map_slot())
        })
        .chain(state.vehicles().values().filter_map(|unit| {
            unit.position()
                .filter(|p| p.map == map)
                .and(unit.map_slot())
        }))
        .max()
        .map_or(0, |slot| slot.saturating_add(1));
    state
        .maps()
        .get(&map)
        .map_or(occupied, |record| record.membership_extent.max(occupied))
}

/// Removing the last allocated slot shortens the span once, leaving earlier holes allocated.
pub(super) fn depart(state: &mut BtechState, id: ObjectId) {
    // Searches and clearance watches belong to the battlefield being left.
    state.autopilot_plans.remove(&id);
    let membership = state
        .constructed_units()
        .get(&id)
        .and_then(|unit| unit.position().zip(unit.map_slot()))
        .or_else(|| {
            state
                .vehicles()
                .get(&id)
                .and_then(|unit| unit.position().zip(unit.map_slot()))
        });
    let Some((position, slot)) = membership else {
        return;
    };
    let span = extent(state, position.map);
    if let Some(map) = state.maps.get_mut(&position.map) {
        map.membership_extent = if slot.checked_add(1) == Some(span) {
            span - 1
        } else {
            span
        };
    }
}

/// Admission and same-map moves share one span update without retaining a second membership list.
pub(super) fn arrive(state: &mut BtechState, id: ObjectId, map: ObjectId, slot: u32) {
    let old_map = state
        .constructed_units()
        .get(&id)
        .and_then(|unit| unit.position())
        .or_else(|| state.vehicles().get(&id).and_then(|unit| unit.position()))
        .map(|p| p.map);
    if old_map != Some(map) {
        depart(state, id);
    }
    let span = extent(state, map).max(slot.saturating_add(1));
    state
        .maps
        .get_mut(&map)
        .expect("admitted map")
        .membership_extent = span;
}
