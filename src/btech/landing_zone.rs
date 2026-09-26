//! Terrain suitability and saved circular landing exclusions, independent of aircraft movement.
use super::{BattleHexCoordinate, StoredBattleMap, Terrain};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, sync::Arc};

/// A circular exclusion; a nonzero exempt team may land within its radius.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleLandingExclusion {
    pub coordinate: BattleHexCoordinate,
    /// Signed radius; negative values retain an inactive restriction.
    pub radius: i64,
    pub exempt_team: i32,
    pub owner: ObjectId,
    /// Authored signed-short payload retained for operator inspection.
    pub data_short: i16,
}

/// First failed strict landing requirement, or a usable landing location.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleLandingSuitability {
    Ready,
    ImproperTerrain,
    UnevenGround,
    Blocked,
}

impl StoredBattleMap {
    /// Stable restriction slots, including overlapping circles.
    pub fn landing_exclusions(&self) -> &BTreeMap<u32, BattleLandingExclusion> {
        &self.landing_exclusions
    }

    /// Traverse restrictions in their saved list order without renumbering identities.
    pub fn ordered_landing_exclusions(
        &self,
    ) -> impl Iterator<Item = (&u32, &BattleLandingExclusion)> {
        self.landing_exclusion_order
            .iter()
            .map(|slot| (slot, &self.landing_exclusions[slot]))
    }

    /// Check strict terrain rules and team exclusions without acquiring contacts or consuming dice.
    /// Grass/road require six on-map neighbors of equal elevation; neighbor terrain is unrestricted.
    pub fn landing_suitability(
        &self,
        coordinate: BattleHexCoordinate,
        team: i32,
    ) -> Result<BattleLandingSuitability> {
        let tile = self.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
        if !matches!(tile.terrain, Terrain::Grassland | Terrain::Road) {
            return Ok(BattleLandingSuitability::ImproperTerrain);
        }
        for neighbor in coordinate.neighbors()? {
            if neighbor.x < 0
                || neighbor.y < 0
                || i64::from(neighbor.x) >= self.width
                || i64::from(neighbor.y) >= self.height
            {
                return Ok(BattleLandingSuitability::UnevenGround);
            }
            if self
                .base_hex(i64::from(neighbor.x), i64::from(neighbor.y))?
                .elevation
                != tile.elevation
            {
                return Ok(BattleLandingSuitability::UnevenGround);
            }
        }
        for zone in self.landing_exclusions.values() {
            if zone.exempt_team != 0 && zone.exempt_team == team {
                continue;
            }
            if coordinate.center().range(zone.coordinate.center())? <= zone.radius as f64 {
                return Ok(BattleLandingSuitability::Blocked);
            }
        }
        Ok(BattleLandingSuitability::Ready)
    }
}

/// Configure a restriction without changing terrain or movement; host owns administrative authorization.
pub fn set_landing_exclusion(
    world: &mut World,
    map: ObjectId,
    ordinal: u32,
    zone: Option<BattleLandingExclusion>,
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
    if let Some(zone) = zone {
        record.base_hex(i64::from(zone.coordinate.x), i64::from(zone.coordinate.y))?;
        ensure!(
            zone.owner == ObjectId(-1)
                || world
                    .objects
                    .get(&zone.owner)
                    .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
            "Landing exclusion owner is unavailable"
        );
        ensure!(
            record.landing_exclusions.contains_key(&ordinal)
                || record.landing_exclusions.len() < 1_000_000,
            "Too many landing exclusions"
        );
    }
    let record = world.btech.maps.get_mut(&map).unwrap();
    let zones = Arc::make_mut(&mut record.landing_exclusions);
    if let Some(zone) = zone {
        if zones.insert(ordinal, zone).is_none() {
            let order = Arc::make_mut(&mut record.landing_exclusion_order);
            let position = order
                .iter()
                .position(|slot| *slot > ordinal)
                .unwrap_or(order.len());
            order.insert(position, ordinal);
        }
    } else {
        zones.remove(&ordinal);
        Arc::make_mut(&mut record.landing_exclusion_order).retain(|slot| *slot != ordinal);
    }
    Ok(())
}
