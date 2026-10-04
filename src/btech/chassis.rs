//! Anatomy of a constructed Mech, read from its validated template.
use super::{BattleMechChassis, BattleUnit};

impl BattleUnit {
    /// Anatomy of a validated constructed definition.
    pub fn chassis(&self) -> BattleMechChassis {
        self.definition().chassis().expect("validated chassis")
    }
}
