//! Administrative construction edits preserve live equipment and use shared load rules.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Change nominal tonnage and reconcile affected movement limits; the caller owns rollback.
pub(super) fn set_tonnage(
    world: &mut World,
    id: ObjectId,
    value: &str,
    tsm_bonus: bool,
) -> Result<()> {
    let tons = value
        .trim()
        .parse::<u16>()
        .context("Invalid unit tonnage")?;
    ensure!(tons > 0, "Unit tonnage must be positive");
    super::with_unit_mut!(
        world.btech.unit_mut(id).context("Unit is unavailable")?,
        |unit| {
            unit.set_nominal_tonnage(tons);
            unit.validate()?;
            unit.mass()?;
        }
    );
    reconcile(world, id, tsm_bonus)
}

/// Change locomotion without inventing hardware or rebuilding the live unit.
pub(super) fn set_movement(
    world: &mut World,
    id: ObjectId,
    value: &str,
    tsm_bonus: bool,
) -> Result<()> {
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        let chassis = super::MechChassis::parse(value.trim())?;
        unit.set_chassis(chassis);
        unit.validate()?;
        unit.mass()?;
    } else {
        let movement = super::VehicleMovement::parse(value.trim())?;
        let unit = world
            .btech
            .vehicles
            .get_mut(&id)
            .context("Unit is unavailable")?;
        unit.set_movement(movement);
        unit.validate()?;
        unit.mass()?;
    }
    reconcile(world, id, tsm_bonus)
}

/// Refresh derived identity and load limits after an owned construction edit.
fn reconcile(world: &mut World, id: ObjectId, tsm_bonus: bool) -> Result<()> {
    super::unit_identity::refresh(world, id)?;
    super::load::reconcile(world, id, tsm_bonus)?;
    if let Some(carrier) = world.btech.towed_by(id) {
        super::load::reconcile(world, carrier, tsm_bonus)?;
    }
    Ok(())
}
