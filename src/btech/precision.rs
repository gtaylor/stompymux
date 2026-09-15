//! Precision autocannon ammunition controls and target-movement adjustment.
use super::BattleAmmunitionMode;
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

impl BattleAmmunitionMode {
    /// Precision reduces the complete target-movement modifier by two, with a zero floor.
    pub(super) fn target_movement_modifier(self, movement: i8) -> i8 {
        if self == Self::Precision {
            movement.saturating_sub(2).max(0)
        } else {
            movement
        }
    }

    /// Shared cockpit feedback for normal and Precision rounds.
    pub(crate) fn precision_message(self, index: usize) -> String {
        format!(
            "Weapon {index} has been set to fire {} rounds",
            if self == Self::Precision {
                "Precision"
            } else {
                "normal"
            }
        )
    }
}

/// Select Precision or normal rounds on an authorized, intact and recycled conventional autocannon.
pub fn toggle_precision(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    let ready = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        BattleAmmunitionMode::Precision.supports(ready.weapon),
        "That weapon cannot fire Precision rounds!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        BattleAmmunitionMode::Precision,
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
    #[test]
    fn precision_movement_reduction_has_a_zero_floor() {
        for (movement, expected) in [(-4, 0), (-2, 0), (0, 0), (1, 0), (2, 0), (3, 1), (7, 5)] {
            assert_eq!(
                BattleAmmunitionMode::Precision.target_movement_modifier(movement),
                expected
            );
            assert_eq!(
                BattleAmmunitionMode::Normal.target_movement_modifier(movement),
                movement
            );
        }
    }
}
