//! The installed gyro family of a live Mech and its effective damage from critical losses.
use super::{Gyro, Mech};

impl Mech {
    /// Installed family, retained after critical and section losses.
    pub fn gyro(&self) -> Gyro {
        Gyro::from_definition(self.definition())
    }

    /// Effective gyro damage; the first hardened critical does not impair stability.
    pub fn gyro_damage(&self) -> u8 {
        self.gyro_condition().0
    }
}
