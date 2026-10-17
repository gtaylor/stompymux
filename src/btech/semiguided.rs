//! Semi-guided missile ammunition controls and friendly TAG target-movement assistance.
use super::{AmmunitionFeedback, AmmunitionMode, TargetMovement};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// Semi-guided ammunition's use of friendly TAG designation (Total Warfare p. 142).
pub(crate) trait SemiGuidedAim {
    /// A friendly TAG on the target, the shooter's own included, removes the target's
    /// movement modifier; the immobile and prone adjustments remain.
    fn tag_movement_modifier(self, movement: TargetMovement, tag_guided: bool) -> TargetMovement;
}

impl SemiGuidedAim for AmmunitionMode {
    fn tag_movement_modifier(self, movement: TargetMovement, tag_guided: bool) -> TargetMovement {
        if self.munition() != Self::SemiGuided || !tag_guided {
            return movement;
        }
        TargetMovement {
            motion: 0,
            ..movement
        }
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

    /// TAG assistance is specific to this ammunition and never removes an immobile, prone or
    /// skidding adjustment.
    #[test]
    fn semiguided_tag_movement_preserves_immobile_and_prone_terms() {
        for motion in [0, 1, 3, 7] {
            for (immobile, prone) in [(0, 0), (-4, 0), (0, 1), (-4, -2)] {
                let movement = TargetMovement {
                    motion,
                    immobile,
                    prone,
                    skidding: 2,
                };
                assert_eq!(
                    AmmunitionMode::SemiGuided.tag_movement_modifier(movement, true),
                    TargetMovement {
                        motion: 0,
                        immobile,
                        prone,
                        skidding: 2,
                    }
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
}
