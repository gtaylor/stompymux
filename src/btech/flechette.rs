//! Flechette autocannon ammunition controls and damage against armored units.
use super::BattleAmmunitionMode;
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

impl BattleAmmunitionMode {
    /// Current constructed targets are armored bipeds; Flechette halves shell damage rounded down.
    pub(super) fn armored_damage(self, damage: u16) -> u16 {
        if self == Self::Flechette {
            damage / 2
        } else {
            damage
        }
    }

    /// Shared cockpit feedback for normal and Flechette rounds.
    pub(crate) fn flechette_message(self, index: usize) -> String {
        format!(
            "Weapon {index} has been set to fire {} rounds",
            if self == Self::Flechette {
                "Flechette"
            } else {
                "normal"
            }
        )
    }
}

/// Select Flechette or normal rounds on an authorized, intact and recycled conventional autocannon.
pub fn toggle_flechette(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        BattleAmmunitionMode::Flechette.supports(ready.weapon),
        "That weapon cannot fire Flechette rounds!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        BattleAmmunitionMode::Flechette,
    ))
}

/// Flechette controls share bounded cockpit selections and transaction handling.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_flechette(world, id, pilot, index).map(|mode| mode.flechette_message(index))
    })
}
