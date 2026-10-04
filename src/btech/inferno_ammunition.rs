//! Inferno ammunition controls share ordinary readiness and exclusive ammunition selection.
use super::{AmmunitionFeedback, AmmunitionMode};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// Toggle a recycled missile launcher; disposable launchers cannot change ammunition.
pub fn toggle_inferno(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<AmmunitionMode> {
    super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        super::weapon_controls::selectable_munition(world, id, index, AmmunitionMode::Inferno),
        "That weapon cannot be set to fire Inferno missiles!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        AmmunitionMode::Inferno,
    ))
}

/// Native controls preserve shared selection ordering and partial-error feedback.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_inferno(world, id, pilot, index).map(|mode| mode.inferno_message(index))
    })
}
