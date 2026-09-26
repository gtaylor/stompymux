//! Building integrity and policy owned by the interior map, independent of entrance tiles.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Durable construction state of an interior map; zero maximum means no active structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleBuildingState {
    pub integrity: i64,
    pub maximum_integrity: i64,
    pub flags: i64,
    pub regeneration: i64,
}

impl Default for BattleBuildingState {
    /// Ordinary battlefields have no construction integrity and use the standard repair factor.
    fn default() -> Self {
        Self {
            integrity: 0,
            maximum_integrity: 0,
            flags: 0,
            regeneration: 1,
        }
    }
}

impl BattleBuildingState {
    /// A surviving damaged structure starts its standard two-minute repair interval.
    pub(crate) fn repair_delay(self) -> Option<u16> {
        (self.integrity > 0 && self.integrity < self.maximum_integrity).then_some(120)
    }

    /// Validate construction bounds before publishing configuration or loading saved maps.
    pub fn validate(self) -> Result<()> {
        ensure!(
            (0..=i64::from(i16::MAX)).contains(&self.maximum_integrity)
                && (0..=self.maximum_integrity).contains(&self.integrity),
            "Invalid building integrity"
        );
        ensure!((0..=255).contains(&self.flags), "Invalid building flags");
        ensure!(
            i32::try_from(self.regeneration).is_ok(),
            "Invalid building regeneration factor"
        );
        Ok(())
    }

    /// Command-center structures ignore ordinary building weapon damage.
    pub fn is_command_center(self) -> bool {
        self.flags & 1 != 0
    }

    /// Complex interior maps suppress building damage originating inside them.
    pub fn is_complex(self) -> bool {
        self.flags & 2 != 0
    }

    /// Dropship structures and explicitly concealed buildings hide their entrances.
    pub fn is_hidden(self) -> bool {
        self.flags & (4 | 16) != 0
    }

    /// Explicitly invisible buildings also carry the hidden flag.
    pub fn is_invisible(self) -> bool {
        self.flags & 16 != 0
    }

    /// Building safety is a separate entrance policy, not construction invulnerability.
    pub fn is_safe(self) -> bool {
        self.flags & 8 != 0
    }

    /// Dropship interiors retain their distinct structural identity.
    pub fn is_dropship(self) -> bool {
        self.flags & 4 != 0
    }
}

/// Configure an interior map without moving occupants or reloading its terrain.
/// The host caller owns administrative authorization; this does not schedule repair events.
pub fn set_building_state(
    world: &mut World,
    map: ObjectId,
    state: BattleBuildingState,
) -> Result<()> {
    state.validate()?;
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Map is unavailable"
    );
    let record = world.btech.maps().get(&map).context("Map not found")?;
    ensure!(record.terrain_ready(), "Map terrain is unavailable");
    world.btech.maps.get_mut(&map).unwrap().building = state;
    Ok(())
}
