//! Precision autocannon ammunition controls and target-movement adjustment.
use super::{AmmunitionFeedback, AmmunitionMode, TargetMovement};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// Precision ammunition's reduction of the target-movement modifier (Total Warfare p. 142).
pub(crate) trait PrecisionAim {
    /// Precision reduces the target's movement itself by two, to a minimum of zero; the
    /// immobile -4 and the prone adjustments are untouched.
    fn precision_movement(self, movement: TargetMovement) -> TargetMovement;
}

impl PrecisionAim for AmmunitionMode {
    fn precision_movement(self, movement: TargetMovement) -> TargetMovement {
        if self != Self::Precision {
            return movement;
        }
        TargetMovement {
            motion: movement.motion.saturating_sub(2).max(0),
            ..movement
        }
    }
}

/// Select Precision or normal rounds on an authorized, intact and recycled conventional autocannon.
pub fn toggle_precision(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<AmmunitionMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        AmmunitionMode::Precision.supports(ready.weapon),
        "That weapon cannot fire Precision rounds!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        AmmunitionMode::Precision,
    ))
}

/// Precision controls share bounded cockpit selections and transaction handling.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_precision(world, id, pilot, index).map(|mode| mode.precision_message(index))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Only the movement band shrinks; immobile and prone targets keep their adjustments.
    #[test]
    fn precision_reduces_movement_only_with_a_zero_floor() {
        for (motion, expected) in [(0, 0), (1, 0), (2, 0), (3, 1), (7, 5)] {
            for (immobile, prone) in [(0, 0), (-4, 0), (0, 1), (-4, -2)] {
                let movement = TargetMovement {
                    motion,
                    immobile,
                    prone,
                };
                assert_eq!(
                    AmmunitionMode::Precision.precision_movement(movement),
                    TargetMovement {
                        motion: expected,
                        immobile,
                        prone,
                    }
                );
                assert_eq!(
                    AmmunitionMode::Normal.precision_movement(movement),
                    movement
                );
            }
        }
    }
}
