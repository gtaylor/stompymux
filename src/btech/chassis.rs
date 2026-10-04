//! Anatomy of a constructed Mech, read from its validated template.
use super::{Mech, MechChassis};

impl Mech {
    /// Anatomy of a validated constructed definition.
    pub fn chassis(&self) -> MechChassis {
        self.definition().chassis().expect("validated chassis")
    }
}
