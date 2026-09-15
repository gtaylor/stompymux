//! Armor-piercing autocannon controls and critical-roll adjustment.
use super::BattleAmmunitionMode;
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

impl BattleAmmunitionMode {
    /// Shared cockpit feedback for normal and AP rounds.
    pub(crate) fn armor_piercing_message(self, index: usize) -> String {
        format!(
            "Weapon {index} has been set to fire {} rounds",
            if self == Self::ArmorPiercing {
                "AP"
            } else {
                "normal"
            }
        )
    }
}

/// Select AP or normal rounds on an authorized, intact and recycled conventional autocannon.
pub fn toggle_armor_piercing(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        BattleAmmunitionMode::ArmorPiercing.supports(ready.weapon),
        "That weapon cannot fire AP rounds!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        BattleAmmunitionMode::ArmorPiercing,
    ))
}

/// AP controls share bounded cockpit selections and transaction handling.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_armor_piercing(world, id, pilot, index)
            .map(|mode| mode.armor_piercing_message(index))
    })
}

impl super::BattleWeapon {
    /// AP critical-roll reduction for conventional and light autocannons.
    pub(super) fn armor_piercing_penalty(self) -> u8 {
        match self {
            Self::Ac2 | Self::LightAc2 => 4,
            Self::Ac5 | Self::LightAc5 => 3,
            Self::Ac10 => 2,
            Self::Ac20 => 1,
            _ => 10,
        }
    }
}
