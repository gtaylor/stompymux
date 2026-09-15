//! Shared cloud-boundary policy for supported units and persisted operator controls.
use super::BattleSensorMode;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

/// Change a map's cloud boundary; zero disables it and negative levels remain meaningful underground.
pub fn set_map_cloud_base(
    world: &mut World,
    actor: ObjectId,
    map: ObjectId,
    altitude: i16,
) -> Result<()> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| object.kind != crate::Kind::Garbage
                && !object.flags.contains(crate::Flag::Going)),
        "Map is unavailable"
    );
    let map = Arc::make_mut(&mut world.btech.maps)
        .get_mut(&map)
        .context("Map not found")?;
    map.cloud_base = altitude;
    Ok(())
}

/// Optical modes cannot see across the cloud boundary; equality belongs to its upper side.
pub(super) fn blocks_contact(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
    sensor: BattleSensorMode,
) -> Result<bool> {
    if !matches!(
        sensor,
        BattleSensorMode::Visual
            | BattleSensorMode::LightAmplification
            | BattleSensorMode::Infrared
    ) {
        return Ok(false);
    }
    let observer = super::los::unit_sight_point(world, observer)?;
    let target = super::los::unit_sight_point(world, target)?;
    ensure!(
        observer.position.map == target.position.map,
        "Units are on different maps"
    );
    let base = i32::from(
        world
            .btech
            .maps()
            .get(&observer.position.map)
            .context("Map not found")?
            .cloud_base,
    );
    Ok(base != 0 && ((observer.level < base) != (target.level < base)))
}

/// Terrain queries use the reference's distinct mixed-sensor rule, independent of sensor family.
/// Unlike unit contacts, equality is visible and matching primary/secondary modes bypass this gate.
pub(super) fn blocks_terrain(base: i16, elevation: i32, pair: super::BattleSensorPair) -> bool {
    base != 0 && pair.primary != pair.secondary && elevation > i32::from(base)
}
