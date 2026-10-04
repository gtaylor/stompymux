//! Player controls for Ultra autocannon double-shot firing.
use super::{BattleFireMode, FireModeFeedback};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// Toggle an intact, recycled Ultra autocannon between one- and two-round firing.
pub fn toggle_ultra(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleFireMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(ready.weapon.is_ultra(), "That weapon cannot be set ULTRA!");
    Ok(super::weapon_controls::toggle_fire_mode(
        world,
        id,
        index,
        BattleFireMode::Ultra,
    ))
}

/// Native Ultra controls share bounded weapon selections and transaction handling.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_ultra(world, id, pilot, index).map(|mode| mode.ultra_message(index))
    })
}
