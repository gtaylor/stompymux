//! Temporary administrative mass corrections shared by every combat chassis.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Fixed-point mass correction, discarded when protection or ammunition changes.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct LiveMass(Option<u32>);

impl LiveMass {
    /// Saved corrections obey the same signed integer bound as field edits.
    pub(super) fn validate(self) -> Result<()> {
        ensure!(
            self.0.is_none_or(|mass| mass <= i32::MAX as u32),
            "Invalid live mass"
        );
        Ok(())
    }

    /// Resume calculation from surviving construction after a material change.
    pub(super) fn invalidate(&mut self) {
        self.0 = None;
    }
}

impl super::BattleUnit {
    /// Current gameplay mass in 1/1024 tons, including an administrative correction.
    pub fn effective_mass(&self) -> Result<u32> {
        self.live_mass
            .0
            .map_or_else(|| self.mass().map(|mass| mass.total), Ok)
    }
}

impl super::BattleVehicle {
    /// Current gameplay mass in 1/1024 tons, including an administrative correction.
    pub fn effective_mass(&self) -> Result<u32> {
        self.live_mass
            .0
            .map_or_else(|| self.mass().map(|mass| mass.total), Ok)
    }
}

/// Set the live mass and reconcile both this unit and any towing partner atomically.
/// The field transaction owns validation, rollback and publication.
pub(super) fn set(world: &mut World, id: ObjectId, value: &str, tsm_bonus: bool) -> Result<()> {
    let mass = value.trim().parse::<i32>().context("Invalid live mass")?;
    ensure!(mass >= 0, "Live mass cannot be negative");
    let correction = LiveMass(Some(mass as u32));
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        unit.live_mass = correction;
    } else {
        world
            .btech
            .vehicles
            .get_mut(&id)
            .context("Unit is unavailable")?
            .live_mass = correction;
    }
    super::load::reconcile(world, id, tsm_bonus)?;
    if let Some(carrier) = world.btech.towed_by(id) {
        super::load::reconcile(world, carrier, tsm_bonus)?;
    }
    Ok(())
}
