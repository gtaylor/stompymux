//! Live vehicle mass: current structure, protection and loaded rounds over the template's
//! construction arithmetic.
use super::{BattleVehicle, BattleVehicleMass, BattleVehicleMaterial, BattleVehicleSection};
use anyhow::Result;

impl BattleVehicleMaterial for BattleVehicle {
    fn internal(&self, section: BattleVehicleSection) -> u16 {
        self.sections()[&section].internal
    }

    fn protection(&self, section: BattleVehicleSection) -> u32 {
        let state = &self.sections()[&section];
        u32::from(state.armor) + u32::from(state.rear)
    }

    fn rounds(&self, bin: usize, _listed: u16) -> u16 {
        self.ammunition()[bin]
    }
}

impl BattleVehicle {
    /// Derive physical mass from current protection, surviving sections and loaded rounds.
    /// Broken equipment retains mass until its section is lost; crew loss does not remove material.
    pub fn mass(&self) -> Result<BattleVehicleMass> {
        self.definition().mass_of(self)
    }
}
