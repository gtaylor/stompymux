//! Atomic woodland reductions on occupied maps, with terrain-only result data.
use super::{Hex, HexCoordinate};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// A committed woodland reduction; the enclosing attack owns notifications.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish terrain and enclosing attack consequences together"]
pub struct WoodlandChange {
    pub map: ObjectId,
    pub coordinate: HexCoordinate,
    pub before: Hex,
    pub after: Hex,
}

/// Apply one successful clearing result without consuming another random draw: spend `total`
/// accumulated damage against the growth in the expected tile (see `reduce_woodland`) and keep
/// whatever damage the lighter growth carries against its own terrain factor, so damage is
/// tracked across density changes (Total Warfare p. 112). Damage that leaves the growth
/// unchanged is not a clearing and is refused.
/// The expected tile prevents a detached result from being applied to changed terrain.
/// Authority, attack rolls and notification routing belong to the caller. Minefields stay intact.
pub fn apply_woodland_clearing(
    world: &mut World,
    map: ObjectId,
    coordinate: HexCoordinate,
    expected: Hex,
    total: u16,
) -> Result<WoodlandChange> {
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
    let reduction = super::reduce_woodland(before.with_overlay(None), total)
        .context("Invalid woodland reduction")?;
    let after = reduction.hex;
    world.attempt(|world| {
        let record = world.btech.maps.get_mut(&map).unwrap();
        record.write_hex(i64::from(coordinate.x), i64::from(coordinate.y), after)?;
        let index = record.tile_index(coordinate)?;
        let damage = std::sync::Arc::make_mut(&mut record.terrain_damage);
        if reduction.carried == 0 {
            damage.remove(&index);
        } else {
            damage.insert(index, reduction.carried);
        }
        world.btech.validate(world)?;
        Ok(WoodlandChange {
            map,
            coordinate,
            before,
            after,
        })
    })
}

/// Area-effect damage against the growth in a hex (Total Warfare p. 112): an area-effect weapon
/// always strikes the woods or jungle in a hex it damages, at double its Damage Value against
/// the growth's terrain factor, thinning or felling it as other damage does. A burning hex is
/// left to its fire. Returns the change when the growth changed type.
pub(super) fn area_effect_woodland(
    world: &mut World,
    map: ObjectId,
    coordinate: HexCoordinate,
    damage: u16,
) -> Result<Option<WoodlandChange>> {
    let record = world.btech.maps().get(&map).context("Map not found")?;
    let tile = record.hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    if damage == 0 || !tile.is_woods() || tile.is_burning() {
        return Ok(None);
    }
    let index = record.tile_index(coordinate)?;
    let total = record
        .terrain_damage
        .get(&index)
        .copied()
        .unwrap_or(0)
        .saturating_add(damage.saturating_mul(2));
    if super::reduce_woodland(tile, total).is_none() {
        let record = world.btech.maps.get_mut(&map).unwrap();
        std::sync::Arc::make_mut(&mut record.terrain_damage).insert(index, total);
        return Ok(None);
    }
    apply_woodland_clearing(world, map, coordinate, tile, total).map(Some)
}
