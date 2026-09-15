//! Player controls for rapid firing of conventional and light autocannons.
use super::BattleFireMode;
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

impl BattleFireMode {
    /// Shared cockpit feedback for native and Lua mode controls.
    pub(crate) fn rapid_message(self, index: usize) -> String {
        format!(
            "Weapon {index} has been set to {} mode",
            if self == Self::Rapid {
                "Rapid Fire"
            } else {
                "normal fire"
            }
        )
    }
}

/// Toggle an intact, recycled conventional or light autocannon between one- and two-round firing.
pub fn toggle_rapid(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleFireMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        ready.weapon.supports_rapid_fire(),
        "That weapon cannot be set to do rapid fire!"
    );
    Ok(super::weapon_controls::toggle_fire_mode(
        world,
        id,
        index,
        BattleFireMode::Rapid,
    ))
}

/// Native Rapid controls share bounded weapon selections and transaction handling.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_rapid(world, id, pilot, index).map(|mode| mode.rapid_message(index))
    })
}
