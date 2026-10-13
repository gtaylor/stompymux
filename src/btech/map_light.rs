//! The live map's light and weather, read from their persisted columns, and how far they let
//! units see. The [`Light`] and [`Weather`] conditions themselves belong to the map crate.
use super::{Light, Weather};
use anyhow::Result;

impl super::StoredMap {
    /// Current battlefield light level, rejecting corrupt persisted values.
    pub fn light_level(&self) -> Result<Light> {
        Light::from_stored(self.light)
    }

    /// Current battlefield weather, rejecting corrupt persisted values.
    pub fn weather_condition(&self) -> Result<Weather> {
        Weather::from_stored(self.weather)
    }

    /// How far a ground unit on this battlefield can see, in hexes: the more restrictive of
    /// its light and weather (Tactical Operations p. 189).
    pub fn visual_range(&self) -> Result<u8> {
        Ok(super::visual_range(
            self.light_level()?,
            self.weather_condition()?,
        ))
    }
}
