//! Caseless autocannon ammunition controls.
use super::{AmmunitionFeedback, AmmunitionMode};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// Select CASELESS or normal rounds on an authorized, intact and recycled conventional autocannon.
pub fn toggle_caseless(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<AmmunitionMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        AmmunitionMode::Caseless.supports(ready.weapon),
        "That weapon cannot fire CASELESS rounds!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        AmmunitionMode::Caseless,
    ))
}

/// CASELESS controls share bounded cockpit selections and transaction handling.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_caseless(world, id, pilot, index).map(|mode| mode.caseless_message(index))
    })
}
