//! Terrain illumination from active battlefield fires, inferno burns and forward searchlight beams.
use super::{DecorationKind, HexCoordinate};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Inspect whether a checked battlefield hex is lit, without identifying any emitting unit.
/// Fires and infernos light their own hex and six neighbors. Searchlights project
/// within their forward arc to terrain less than sixty spatial hexes away.
pub fn hex_illuminated(world: &World, map: ObjectId, target: HexCoordinate) -> Result<bool> {
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|o| !o.flags.contains(Flag::Going)),
        "Map is unavailable"
    );
    let record = world.btech.maps().get(&map).context("Map not found")?;
    record.base_hex(i64::from(target.x), i64::from(target.y))?;
    for coordinate in std::iter::once(target).chain(record.neighbors(target)?.into_iter().flatten())
    {
        if record
            .decoration(coordinate)?
            .is_some_and(|effect| effect.kind == DecorationKind::Fire)
        {
            return Ok(true);
        }
    }
    let jelly = world
        .btech
        .constructed_units()
        .iter()
        .map(|(&id, unit)| (id, unit.position(), unit.inferno_remaining()))
        .chain(
            world
                .btech
                .vehicles()
                .iter()
                .map(|(&id, unit)| (id, unit.position(), unit.inferno_remaining())),
        );
    for (id, position, remaining) in jelly {
        if remaining == 0
            || world
                .objects
                .get(&id)
                .is_none_or(|object| object.flags.contains(Flag::Going))
        {
            continue;
        }
        if let Some(position) = position.filter(|position| position.map == map) {
            let source = HexCoordinate {
                x: i32::from(position.x),
                y: i32::from(position.y),
            };
            if source.distance(target) <= 1 {
                return Ok(true);
            }
        }
    }
    for id in super::searchlight::emitter_ids(world) {
        let Some((position, point, heading)) = super::searchlight::beam(world, id) else {
            continue;
        };
        if position.map != map {
            continue;
        }
        let bearing = point.bearing(target.center())?.unwrap_or(heading);
        let angle = (bearing - heading).rem_euclid(360.0);
        if angle > 60.0 && angle < 300.0 {
            continue;
        }
        let (los, distance) = super::los::unit_hex_los(world, id, target)?;
        if distance < 60.0 && !los.blocked && los.woods <= 2 && los.water == 0 && !los.smoke {
            return Ok(true);
        }
    }
    Ok(false)
}
