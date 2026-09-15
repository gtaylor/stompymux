//! Shared surviving jump thrust for administrative inspection and orbital compensation.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Raw surviving jump thrust controls cocoon compensation even while the engine is off.
pub(super) fn speed(world: &World, id: ObjectId) -> Result<f64> {
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        return Ok(unit.jump_capacity(100)?.speed);
    }
    let unit = world
        .btech
        .vehicles()
        .get(&id)
        .context("Unit is unavailable")?;
    let speed = unit
        .definition()
        .attributes
        .get("jump_speed")
        .map(|value| value.parse::<f64>())
        .transpose()?
        .unwrap_or(0.0);
    ensure!(
        speed.is_finite() && speed >= 0.0,
        "Invalid vehicle jump thrust"
    );
    let lost = vehicle_losses(unit)?;
    Ok(if unit.is_destroyed() {
        0.0
    } else {
        unit.propulsion.jump(speed, lost)
    })
}

/// Vehicle jet availability comes from the same loadout used by orbital compensation.
fn vehicle_losses(unit: &BattleVehicle) -> Result<usize> {
    Ok(unit
        .loadout()?
        .systems
        .iter()
        .filter(|part| {
            part.system == BattleSystem::JumpJet && unit.critical_unavailable(part.location)
        })
        .count())
}

/// Correct current jump thrust without changing equipment or mass; the caller owns rollback.
pub(super) fn set(world: &mut World, id: ObjectId, value: &str) -> Result<()> {
    let speed = super::propulsion::parse(value)?;
    BattleJumpCapacity::from_speed(speed * 2.0)
        .context("Jump speed exceeds supported flight capacity at low gravity")?;
    if let Some(unit) = std::sync::Arc::make_mut(&mut world.btech.constructed).get_mut(&id) {
        ensure!(
            !unit.is_destroyed() || speed == 0.0,
            "Destroyed units cannot provide jump thrust"
        );
        let lost = usize::from(unit.system_hits(BattleSystem::JumpJet));
        unit.propulsion.set_jump(speed, lost);
    } else {
        let unit = std::sync::Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .context("Unit is unavailable")?;
        ensure!(
            !unit.is_destroyed() || speed == 0.0,
            "Destroyed units cannot provide jump thrust"
        );
        let lost = vehicle_losses(unit)?;
        unit.propulsion.set_jump(speed, lost);
    }
    Ok(())
}
