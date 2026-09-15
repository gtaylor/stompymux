//! Shared saved sprint mode for status edits, movement admission and orbital insertion.
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use std::sync::Arc;

/// Read the physical unit's mode without recomputing speed or changing timers.
pub(super) fn enabled(world: &World, id: ObjectId) -> Result<bool> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Ok(unit.sprinting);
    }
    Ok(world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is not constructed")?
        .sprinting)
}

/// Commit mode after the enclosing administrative or movement action has admitted the request.
pub(super) fn set(world: &mut World, id: ObjectId, enabled: bool) -> Result<()> {
    if let Some(unit) = Arc::make_mut(&mut world.btech.vehicles).get_mut(&id) {
        unit.sprinting = enabled;
        return Ok(());
    }
    Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .context("Unit is not constructed")?
        .sprinting = enabled;
    Ok(())
}

/// Apply sprint bonuses to a loaded base using current equipment, pilot and environmental state.
pub(super) fn maximum(world: &World, id: ObjectId, base: f64, tsm_bonus: bool) -> Result<f64> {
    let (pilot, position, bonuses) = if let Some(unit) = world.btech.constructed_units().get(&id) {
        (
            unit.pilot(),
            unit.position(),
            super::speed_bonus::SpeedBonuses {
                masc: unit.masc_active(),
                supercharger: unit.supercharger_active(),
                sprint: unit.sprinting,
                hot_myomer: unit.triple_myomer_active(),
                tsm_sprint_bonus: tsm_bonus,
                ..Default::default()
            },
        )
    } else {
        let unit = world
            .btech
            .vehicles()
            .get(&id)
            .context("Unit is not constructed")?;
        (
            unit.pilot(),
            unit.position(),
            super::speed_bonus::SpeedBonuses {
                sprint: unit.sprinting,
                ..Default::default()
            },
        )
    };
    let bonuses = super::speed_bonus::SpeedBonuses {
        speed_demon: pilot
            .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Speed_Demon")),
        ..bonuses
    };
    super::speed_bonus::on_map(world, position, f64::from(bonuses.apply(base)?))
}

/// Saved controls may retain a prior pilot bonus or gravity ceiling until the next update.
/// This bound includes every allowed policy for the unit's installed equipment.
pub(super) fn saved_limit(base: f64, masc: bool, supercharger: bool, myomer: bool) -> f64 {
    let bonuses = super::speed_bonus::SpeedBonuses {
        masc,
        supercharger,
        sprint: true,
        hot_myomer: myomer,
        tsm_sprint_bonus: true,
        speed_demon: true,
    };
    bonuses
        .apply(base)
        .and_then(|speed| super::speed_bonus::gravity(speed, Some(50)))
        .unwrap_or(f64::NAN)
}
