//! Shared ownership checks for orbital-drop cursors in persisted Mechs and ground vehicles.
use super::{DropProtection, Mech, OrbitalDrop, Vehicle};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// Target-owned protection, independent of the attacker's chassis and selection owner.
pub(super) fn current(world: &World, id: ObjectId) -> Option<OrbitalDrop> {
    world
        .btech
        .constructed_units()
        .get(&id)
        .and_then(Mech::orbital_drop)
        .or_else(|| {
            world
                .btech
                .vehicles()
                .get(&id)
                .and_then(Vehicle::orbital_drop)
        })
}

/// Every active drop has a physical pose and exclusively owns vertical movement.
fn validate(
    drop: Option<OrbitalDrop>,
    placed: bool,
    has_motion: bool,
    competing_altitude: bool,
) -> Result<()> {
    let Some(drop) = drop else {
        return Ok(());
    };
    ensure!(
        placed && has_motion && !competing_altitude,
        "Orbital drop requires an exclusive placed altitude owner"
    );
    ensure!(
        drop.protection() != DropProtection::Breached,
        "A breached orbital drop must hand control to landing or free fall"
    );
    Ok(())
}

impl Mech {
    /// Current cocoon or jump-jet descent, independent of ordinary jump flight.
    pub fn orbital_drop(&self) -> Option<OrbitalDrop> {
        self.orbital_drop
    }

    /// Detached units retain their physical pose until the deferred map cleanup.
    pub(super) fn validate_orbital_drop(&self) -> Result<()> {
        validate(
            self.orbital_drop,
            self.position.is_some(),
            self.motion.is_some(),
            self.flight.is_some() || self.free_fall.is_some() || self.ground_elevation.is_some(),
        )
    }
}

impl Vehicle {
    /// Ground chassis use the same protection and descent cursor as Mechs.
    pub fn orbital_drop(&self) -> Option<OrbitalDrop> {
        self.orbital_drop
    }

    /// Aircraft insertion uses powered flight and cannot own a ground-unit cocoon.
    pub(super) fn validate_orbital_drop(&self) -> Result<()> {
        validate(
            self.orbital_drop,
            self.position().is_some() || self.detached,
            self.motion.is_some(),
            self.definition().is_vtol()
                || self.vtol_flight.is_some()
                || self.free_fall.is_some()
                || self.ground_elevation.is_some(),
        )
    }
}
