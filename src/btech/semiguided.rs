//! Semi-guided missile ammunition controls and friendly TAG target-movement assistance.
use super::{AmmunitionFeedback, AmmunitionMode};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// Semi-guided ammunition's use of friendly TAG designation.
pub(crate) trait SemiGuidedAim {
    /// Friendly TAG from another unit removes positive movement penalties but preserves negative modifiers.
    fn tag_movement_modifier(self, movement: i8, friendly_other_tag: bool) -> i8;
}

impl SemiGuidedAim for AmmunitionMode {
    fn tag_movement_modifier(self, movement: i8, friendly_other_tag: bool) -> i8 {
        if self.munition() == Self::SemiGuided && friendly_other_tag {
            return movement.min(0);
        }
        movement
    }
}

/// Toggle a controlled, intact and recycled launcher between normal and semi-guided rounds.
pub fn toggle_semiguided(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<AmmunitionMode> {
    super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        super::weapon_controls::selectable_munition(world, id, index, AmmunitionMode::SemiGuided),
        "That weapon cannot fire Sguided missiles!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        AmmunitionMode::SemiGuided,
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
                AmmunitionMode::SemiGuided.tag_movement_modifier(movement, true),
                movement.min(0)
            );
            assert_eq!(
                AmmunitionMode::SemiGuided.tag_movement_modifier(movement, false),
                movement
            );
            assert_eq!(
                AmmunitionMode::Normal.tag_movement_modifier(movement, true),
                movement
            );
        }
    }
}
