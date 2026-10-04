//! Stinger ammunition selection uses the ordinary transactional weapon controls.
use super::{AmmunitionFeedback, AmmunitionMode};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// Toggle a controlled, intact and recycled launcher between normal and Stinger rounds.
pub fn toggle_stinger(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<AmmunitionMode> {
    super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        super::weapon_controls::selectable_munition(world, id, index, AmmunitionMode::Stinger),
        "That weapon cannot be set STINGER!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        AmmunitionMode::Stinger,
    ))
}

/// Use the ordinary bounded multi-weapon selection and world/effect checkpoint.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_stinger(world, id, pilot, index).map(|mode| mode.stinger_message(index))
    })
}

/// Stinger seekers accept airborne units, including orbital drops; ground launch preparation is insufficient.
pub(super) fn target_airborne(world: &World, target: ObjectId) -> bool {
    world
        .btech
        .constructed_units()
        .get(&target)
        .is_some_and(|unit| unit.airborne())
        || world.btech.vehicles().get(&target).is_some_and(|unit| {
            unit.orbital_drop().is_some()
                || unit.vtol_flight().is_some_and(|flight| {
                    matches!(
                        flight.phase,
                        super::VtolFlightPhase::Airborne | super::VtolFlightPhase::Falling
                    )
                })
        })
}
