//! Wind settings shared by fire propagation and map persistence.
use super::StoredBattleMap;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

impl StoredBattleMap {
    /// Delay in committed seconds between fire spreading events.
    pub fn fire_spread_interval(&self) -> u16 {
        (60 - self.wind_speed).max(20) as u16
    }
}

/// Update wind conditions without reloading terrain or disturbing map occupants.
pub fn set_map_wind(world: &mut World, map: ObjectId, direction: i64, speed: i64) -> Result<()> {
    ensure!(
        (0..360).contains(&direction) && (0..=i64::from(i16::MAX)).contains(&speed),
        "Invalid map wind"
    );
    ensure!(
        world
            .objects
            .get(&map)
            .is_some_and(|object| !object.flags.contains(crate::Flag::Going)),
        "Map is unavailable"
    );
    let record = world.btech.maps().get(&map).context("Map not found")?;
    ensure!(record.terrain_ready(), "Map terrain is unavailable");
    let record = world.btech.maps.get_mut(&map).unwrap();
    record.wind_direction = direction;
    record.wind_speed = speed;
    Ok(())
}
