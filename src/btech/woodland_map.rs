//! Atomic woodland reductions on occupied maps, with terrain-only result data.
use super::{BattleWoodlandClearing, Hex, HexCoordinate};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// A committed woodland reduction; the enclosing attack owns notifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish terrain and enclosing attack consequences together"]
pub struct BattleWoodlandChange {
    pub map: ObjectId,
    pub coordinate: HexCoordinate,
    pub before: Hex,
    pub after: Hex,
}

/// Apply one successful clearing result without consuming another random draw.
/// The expected tile prevents a detached result from being applied to changed terrain.
/// Authority, attack rolls and notification routing belong to the caller. Minefields stay intact.
pub fn apply_woodland_clearing(
    world: &mut World,
    map: ObjectId,
    coordinate: HexCoordinate,
    expected: Hex,
    clearing: BattleWoodlandClearing,
) -> Result<BattleWoodlandChange> {
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Map is unavailable"
    );
    let record = world.btech.maps().get(&map).context("Map not found")?;
    record.validate()?;
    let before = record.hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    ensure!(
        before == expected,
        "Woodland changed before the clearing result was applied"
    );
    ensure!(!before.is_burning(), "Invalid woodland reduction");
    let after = clearing
        .apply(before.with_overlay(None))
        .context("Invalid woodland reduction")?;
    world.attempt(|world| {
        world.btech.maps.get_mut(&map).unwrap().write_hex(
            i64::from(coordinate.x),
            i64::from(coordinate.y),
            after,
        )?;
        world.btech.validate(world)?;
        Ok(BattleWoodlandChange {
            map,
            coordinate,
            before,
            after,
        })
    })
}
