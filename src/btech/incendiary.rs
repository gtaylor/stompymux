//! Incendiary autocannon ammunition controls.
use super::{AmmunitionFeedback, AmmunitionMode};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// Select Incendiary or normal rounds on an authorized, intact and recycled conventional autocannon.
pub fn toggle_incendiary(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<AmmunitionMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        AmmunitionMode::Incendiary.supports(ready.weapon),
        "That weapon cannot fire Incendiary rounds!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        AmmunitionMode::Incendiary,
    ))
}

/// Incendiary controls share bounded cockpit selections and transaction handling.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_incendiary(world, id, pilot, index).map(|mode| mode.incendiary_message(index))
    })
}
