//! The live map's light level, read from its persisted column. The [`Light`] levels
//! themselves belong to the map crate.
use super::Light;
use anyhow::Result;

impl super::StoredMap {
    /// Current battlefield light level, rejecting corrupt persisted values.
    pub fn light_level(&self) -> Result<Light> {
        Light::from_stored(self.light)
    }
}
