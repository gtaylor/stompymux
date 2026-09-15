//! Caseless autocannon ammunition controls.
use super::BattleAmmunitionMode;
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

impl BattleAmmunitionMode {
    /// Shared cockpit feedback for normal and CASELESS rounds.
    pub(crate) fn caseless_message(self, index: usize) -> String {
        format!(
            "Weapon {index} has been set to fire {} rounds",
            if self == Self::Caseless {
                "CASELESS"
            } else {
                "normal"
            }
        )
    }
}

/// Select CASELESS or normal rounds on an authorized, intact and recycled conventional autocannon.
pub fn toggle_caseless(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        BattleAmmunitionMode::Caseless.supports(ready.weapon),
        "That weapon cannot fire CASELESS rounds!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        BattleAmmunitionMode::Caseless,
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
