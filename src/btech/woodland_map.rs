//! Atomic woodland reductions on occupied maps, with terrain-only result data.
use super::{BattleHex, BattleHexCoordinate, Terrain};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// A committed woodland reduction; the enclosing attack owns notifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish terrain and enclosing attack consequences together"]
pub struct BattleWoodlandChange {
    pub map: ObjectId,
    pub coordinate: BattleHexCoordinate,
    pub before: BattleHex,
    pub after: BattleHex,
}

/// Apply one successful clearing result without consuming another random draw.
/// The expected tile prevents a detached result from being applied to changed terrain.
/// Authority, attack rolls and notification routing belong to the caller. Minefields stay intact.
pub fn apply_woodland_clearing(
    world: &mut World,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    expected: BattleHex,
    replacement: Terrain,
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
    ensure!(
        matches!(
            (before.terrain(), replacement),
            (Terrain::HeavyForest, Terrain::LightForest)
                | (Terrain::LightForest, Terrain::Rough | Terrain::Grassland)
        ),
        "Invalid woodland reduction"
    );
    let after = before.with_terrain(replacement);
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
