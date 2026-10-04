//! Live vehicle mass: current structure, protection and loaded rounds over the template's
//! construction arithmetic.
use super::{Vehicle, VehicleMass, VehicleMaterial, VehicleSection};
use anyhow::Result;

impl VehicleMaterial for Vehicle {
    fn internal(&self, section: VehicleSection) -> u16 {
        self.sections()[&section].internal
    }

    fn protection(&self, section: VehicleSection) -> u32 {
        let state = &self.sections()[&section];
        u32::from(state.armor) + u32::from(state.rear)
    }

    fn rounds(&self, bin: usize, _listed: u16) -> u16 {
        self.ammunition()[bin]
    }
}

impl Vehicle {
    /// Derive physical mass from current protection, surviving sections and loaded rounds.
    /// Broken equipment retains mass until its section is lost; crew loss does not remove material.
    pub fn mass(&self) -> Result<VehicleMass> {
        self.definition().mass_of(self)
    }
}
