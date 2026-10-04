//! Armor-piercing autocannon controls and critical-roll adjustment.
use super::{AmmunitionFeedback, AmmunitionMode};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// Select AP or normal rounds on an authorized, intact and recycled conventional autocannon.
pub fn toggle_armor_piercing(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<AmmunitionMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        AmmunitionMode::ArmorPiercing.supports(ready.weapon),
        "That weapon cannot fire AP rounds!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        AmmunitionMode::ArmorPiercing,
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

/// Armor-piercing critical-roll rules for autocannons.
pub(crate) trait ArmorPiercing {
    /// AP critical-roll reduction for conventional and light autocannons.
    fn armor_piercing_penalty(self) -> u8;
}

impl ArmorPiercing for super::Weapon {
    fn armor_piercing_penalty(self) -> u8 {
        match self {
            Self::Ac2 | Self::LightAc2 => 4,
            Self::Ac5 | Self::LightAc5 => 3,
            Self::Ac10 => 2,
            Self::Ac20 => 1,
            _ => 10,
        }
    }
}
