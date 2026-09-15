//! Semi-guided missile ammunition controls and friendly TAG target-movement assistance.
use super::{BattleAmmunitionMode, BattleWeapon};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

impl BattleWeapon {
    /// Supported indirect-fire missile profiles accept semi-guided ammunition; rockets and artillery do not.
    pub fn supports_semiguided(self) -> bool {
        self.supports_indirect_fire() && !self.is_rocket() && !self.is_mml()
    }
}

impl BattleAmmunitionMode {
    /// Friendly TAG from another unit removes positive movement penalties but preserves negative modifiers.
    pub(super) fn tag_movement_modifier(self, movement: i8, friendly_other_tag: bool) -> i8 {
        if self == Self::SemiGuided && friendly_other_tag {
            return movement.min(0);
        }
        movement
    }

    /// Shared native/Lua cockpit feedback.
    pub(crate) fn semiguided_message(self, index: usize) -> String {
        if self == Self::SemiGuided {
            return format!("Weapon {index} has been set to fire Sguided missiles.");
        }
        format!("Weapon {index} has been set to fire normal missiles")
    }
}

/// Toggle a controlled, intact and recycled launcher between normal and semi-guided rounds.
pub fn toggle_semiguided(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        ready.weapon.supports_semiguided(),
        "That weapon cannot fire Sguided missiles!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        BattleAmmunitionMode::SemiGuided,
    ))
}

/// Use the ordinary bounded multi-weapon selection and world/effect checkpoint.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_semiguided(world, id, pilot, index).map(|mode| mode.semiguided_message(index))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// TAG assistance is specific to this ammunition and never removes a negative target modifier.
    #[test]
    fn semiguided_tag_movement_preserves_negative_modifiers() {
        for movement in [-4, -2, 0, 1, 3, 7] {
            assert_eq!(
                BattleAmmunitionMode::SemiGuided.tag_movement_modifier(movement, true),
                movement.min(0)
            );
            assert_eq!(
                BattleAmmunitionMode::SemiGuided.tag_movement_modifier(movement, false),
                movement
            );
            assert_eq!(
                BattleAmmunitionMode::Normal.tag_movement_modifier(movement, true),
                movement
            );
        }
    }
}
