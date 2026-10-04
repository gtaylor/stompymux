//! Installed engine family of a constructed Mech.
use super::{Engine, Mech};
use anyhow::Result;

impl Mech {
    /// Installed fusion-engine type, retained through critical or section losses.
    pub fn engine(&self) -> Result<Engine> {
        Engine::resolve(&self.loadout()?, self.definition().clan_engine())
    }
}
