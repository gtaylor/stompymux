//! Map-owned cargo transfer points shared by every carrying chassis.
use crate::{ObjectId, StoredMap, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Optional loading location; hints are only revealed when explicitly configured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CargoTransferPoint {
    pub x: i32,
    pub y: i32,
    #[serde(default)]
    pub reveal_hint: bool,
}

impl CargoTransferPoint {
    /// A transfer point must name a hex inside its owning map.
    pub(crate) fn validate(self, map: &StoredMap) -> Result<()> {
        ensure!(
            self.x >= 0
                && self.y >= 0
                && i64::from(self.x) < map.width
                && i64::from(self.y) < map.height,
            "Cargo transfer point is outside the map"
        );
        Ok(())
    }
}

impl StoredMap {
    /// Saved cargo location, independent of the map's terrain and unit occupancy.
    pub fn cargo_transfer_point(&self) -> Option<CargoTransferPoint> {
        self.cargo_transfer_point
    }
}

/// Wizard configuration of a loading point; None removes the location restriction.
pub fn set_cargo_transfer_point(
    world: &mut World,
    actor: ObjectId,
    map: ObjectId,
    point: Option<CargoTransferPoint>,
) -> Result<()> {
    ensure!(
        crate::authority::is_wizard(world, actor),
        "Permission denied."
    );
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Map is unavailable"
    );
    let stored = world.btech.maps.get(&map).context("Map not found")?;
    if let Some(point) = point {
        point.validate(stored)?;
    }
    world.btech.maps.get_mut(&map).unwrap().cargo_transfer_point = point;
    Ok(())
}

/// Enforce the configured location without exposing hidden coordinates on failure.
pub fn check_cargo_transfer_point(world: &World, map: ObjectId, x: i32, y: i32) -> Result<()> {
    let map = world.btech.maps.get(&map).context("Map not found")?;
    let Some(point) = map.cargo_transfer_point else {
        return Ok(());
    };
    if point.x == x && point.y == y {
        return Ok(());
    }
    ensure!(
        !point.reveal_hint,
        "You're not where the cargo is! Try looking around {},{} instead.",
        point.x,
        point.y
    );
    anyhow::bail!("You're not where the cargo is!")
}
