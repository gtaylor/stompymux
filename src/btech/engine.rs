//! Installed engine family of a constructed Mech.
use super::{BattleEngine, BattleUnit};
use anyhow::Result;

impl BattleUnit {
    /// Installed fusion-engine type, retained through critical or section losses.
    pub fn engine(&self) -> Result<BattleEngine> {
        BattleEngine::resolve(&self.loadout()?, self.definition().clan_engine())
    }
}
