//! Ordered battlefield entrances referencing shared interior-map construction state.
use super::{BattleHexCoordinate, StoredBattleMap};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

/// A battlefield coordinate leading to the map that owns the building's integrity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleBuildingEntrance {
    pub coordinate: BattleHexCoordinate,
    pub interior: ObjectId,
    /// Authored byte payload retained for map-object inspection.
    pub data_char: u8,
    /// Authored signed-short payload, independent of interior integrity.
    pub data_short: i16,
    /// Authored scalar payload retained across route edits and copies.
    pub data_int: i64,
}

impl StoredBattleMap {
    /// Stable entrance order; duplicate coordinates retain first-entry selection semantics.
    pub fn building_entrances(&self) -> &BTreeMap<u32, BattleBuildingEntrance> {
        &self.building_entrances
    }

    /// Inspect the first entrance at a checked coordinate without selecting an occupant.
    pub fn building_at(
        &self,
        coordinate: BattleHexCoordinate,
    ) -> Result<Option<BattleBuildingEntrance>> {
        self.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
        Ok(self
            .building_entrances
            .values()
            .find(|entrance| entrance.coordinate == coordinate)
            .copied())
    }
}

/// Set or remove one stable entrance slot without rewriting terrain or building integrity.
/// Removing an entrance also removes all return links from its interior map.
/// Administrative authorization and movement through the entrance belong to the host caller.
pub fn set_building_entrance(
    world: &mut World,
    map: ObjectId,
    ordinal: u32,
    entrance: Option<BattleBuildingEntrance>,
) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Map is unavailable"
    );
    let record = world.btech.maps().get(&map).context("Map not found")?;
    ensure!(record.terrain_ready(), "Map terrain is unavailable");
    if let Some(entrance) = entrance {
        record.base_hex(
            i64::from(entrance.coordinate.x),
            i64::from(entrance.coordinate.y),
        )?;
        ensure!(
            world.btech.maps().contains_key(&entrance.interior)
                && world
                    .objects
                    .get(&entrance.interior)
                    .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
            "Building interior is unavailable"
        );
        ensure!(
            record.building_entrances.contains_key(&ordinal)
                || record.building_entrances.len() < 1_000_000,
            "Too many building entrances"
        );
    }
    let record = Arc::make_mut(&mut world.btech.maps).get_mut(&map).unwrap();
    let entrances = Arc::make_mut(&mut record.building_entrances);
    if let Some(entrance) = entrance {
        entrances.insert(ordinal, entrance);
        record.set_lookup_bit(
            entrance.coordinate,
            super::map_bits::LookupKind::Hangar,
            true,
        )?;
    } else if let Some(removed) = entrances.remove(&ordinal) {
        // Return routes belong to the removed exterior entrance, even when another
        // entrance points at the same interior. Interior arrival points stay intact.
        if let Some(interior) = Arc::make_mut(&mut world.btech.maps).get_mut(&removed.interior) {
            interior.building_exits = Default::default();
            interior.building_parent = 0;
        }
    }
    Ok(())
}

/// Structure actions use the authored object name independently of contact-report identification.
pub(super) fn structure_name(world: &World, interior: ObjectId) -> Result<String> {
    let object = world
        .objects
        .get(&interior)
        .context("Building interior is unavailable")?;
    ensure!(
        !object.flags.contains(crate::Flag::Going),
        "Building interior is unavailable"
    );
    Ok(format!("the {}", object.name))
}
