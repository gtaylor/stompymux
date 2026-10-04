//! Shared building destinations independent of chassis, entry timers and host movement publication.
use super::{BattlePosition, HexCoordinate, StoredBattleMap};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

/// An interior arrival coordinate and its saved single-byte direction selector.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleBuildingEntryPoint {
    pub coordinate: HexCoordinate,
    pub direction: u8,
    /// Authored object reference; route selection uses the direction and coordinate.
    pub object: ObjectId,
    /// Authored signed-short payload retained for map-object inspection.
    pub data_short: i16,
    /// Authored scalar payload retained across edits and copies.
    pub data_int: i64,
}

/// A return-map record; its coordinate is selection metadata, not the exterior arrival point.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleBuildingExit {
    pub coordinate: HexCoordinate,
    pub destination: ObjectId,
    /// Authored byte payload retained for map-object inspection.
    pub data_char: u8,
    /// Authored signed-short payload retained across destination changes.
    pub data_short: i16,
    /// Authored scalar payload retained across destination changes.
    pub data_int: i64,
}

impl StoredBattleMap {
    /// Inspect authored arrival points in stable selection order.
    pub fn building_entry_points(&self) -> &BTreeMap<u32, BattleBuildingEntryPoint> {
        &self.building_entry_points
    }

    /// Inspect ordered return-map links without evaluating exit policy.
    pub fn building_exits(&self) -> &BTreeMap<u32, BattleBuildingExit> {
        &self.building_exits
    }
}

/// Validate map identity once for route administration and traversal lookup.
fn map(world: &World, id: ObjectId) -> Result<&StoredBattleMap> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Building route map is unavailable"
    );
    world
        .btech
        .maps()
        .get(&id)
        .context("Building route map is unavailable")
}

/// Set one interior arrival slot; duplicate directions retain first-slot selection.
pub fn set_building_entry_point(
    world: &mut World,
    id: ObjectId,
    ordinal: u32,
    point: Option<BattleBuildingEntryPoint>,
) -> Result<()> {
    let record = map(world, id)?;
    if let Some(point) = point {
        record.base_hex(i64::from(point.coordinate.x), i64::from(point.coordinate.y))?;
        ensure!(
            record.building_entry_points.contains_key(&ordinal)
                || record.building_entry_points.len() < 1_000_000,
            "Too many building entry points"
        );
    }
    let record = world.btech.maps.get_mut(&id).unwrap();
    let points = Arc::make_mut(&mut record.building_entry_points);
    if let Some(point) = point {
        points.insert(ordinal, point);
    } else {
        points.remove(&ordinal);
    }
    Ok(())
}

/// Set one return-map link without requiring a reciprocal entrance to exist yet.
pub fn set_building_exit(
    world: &mut World,
    id: ObjectId,
    ordinal: u32,
    destination: Option<ObjectId>,
) -> Result<()> {
    let previous = map(world, id)?.building_exits.get(&ordinal).copied();
    set_building_return_link(
        world,
        id,
        ordinal,
        destination.map(|destination| BattleBuildingExit {
            coordinate: previous.map_or(HexCoordinate { x: 0, y: 0 }, |exit| exit.coordinate),
            destination,
            data_char: previous.map_or(0, |exit| exit.data_char),
            data_short: previous.map_or(0, |exit| exit.data_short),
            data_int: previous.map_or(0, |exit| exit.data_int),
        }),
    )
}

/// Configure a complete return record. Metadata coordinates may lie outside the interior map.
pub fn set_building_return_link(
    world: &mut World,
    id: ObjectId,
    ordinal: u32,
    exit: Option<BattleBuildingExit>,
) -> Result<()> {
    let record = map(world, id)?;
    if let Some(exit) = exit {
        map(world, exit.destination)?;
        ensure!(
            exit.destination != id,
            "Building exit cannot return to itself"
        );
        ensure!(
            record.building_exits.contains_key(&ordinal) || record.building_exits.len() < 1_000_000,
            "Too many building exits"
        );
    }
    let exits = Arc::make_mut(&mut world.btech.maps.get_mut(&id).unwrap().building_exits);
    if let Some(exit) = exit {
        exits.insert(ordinal, exit);
    } else {
        exits.remove(&ordinal);
    }
    Ok(())
}

/// Resolve the first exterior entrance and matching interior point. Locks and motion admission belong to the caller.
/// Omitted or zero direction selects the first point; requested ASCII directions are case-insensitive.
pub fn building_entry_destination(
    world: &World,
    exterior: ObjectId,
    coordinate: HexCoordinate,
    direction: Option<u8>,
) -> Result<BattlePosition> {
    let entrance = map(world, exterior)?
        .building_at(coordinate)?
        .context("You see nothing to enter here!")?;
    let interior = map(world, entrance.interior)?;
    let direction = direction
        .filter(|direction| *direction != 0)
        .map(|direction| direction.to_ascii_lowercase());
    let point = interior
        .building_entry_points
        .values()
        .find(|point| direction.is_none_or(|direction| point.direction == direction))
        .context("Building has no matching entry point")?;
    interior.base_hex(i64::from(point.coordinate.x), i64::from(point.coordinate.y))?;
    Ok(BattlePosition {
        map: entrance.interior,
        x: u16::try_from(point.coordinate.x)?,
        y: u16::try_from(point.coordinate.y)?,
    })
}

/// Resolve the first return link and the first reciprocal exterior entrance without moving a unit.
pub fn building_exit_destination(world: &World, interior: ObjectId) -> Result<BattlePosition> {
    let destination = map(world, interior)?
        .building_exits
        .values()
        .next()
        .context("Building has no return map")?
        .destination;
    ensure!(
        destination != interior,
        "Building exit cannot return to itself"
    );
    let exterior = map(world, destination)?;
    let entrance = exterior
        .building_entrances
        .values()
        .find(|entrance| entrance.interior == interior)
        .context("Return map has no reciprocal building entrance")?;
    exterior.base_hex(
        i64::from(entrance.coordinate.x),
        i64::from(entrance.coordinate.y),
    )?;
    Ok(BattlePosition {
        map: destination,
        x: u16::try_from(entrance.coordinate.x)?,
        y: u16::try_from(entrance.coordinate.y)?,
    })
}
