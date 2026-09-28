//! Stinger ammunition selection uses the ordinary transactional weapon controls.
use super::BattleAmmunitionMode;
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

impl BattleAmmunitionMode {
    /// Cockpit feedback shared by native commands and Lua.
    pub(crate) fn stinger_message(self, index: usize) -> String {
        if self.munition() == Self::Stinger {
            return format!("Weapon {index} has been set to fire stinger missiles.");
        }
        format!("Weapon {index} has been set to fire normal missiles")
    }
}

/// Toggle a controlled, intact and recycled launcher between normal and Stinger rounds.
pub fn toggle_stinger(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(
        super::weapon_controls::selectable_munition(
            world,
            id,
            index,
            BattleAmmunitionMode::Stinger
        ),
        "That weapon cannot be set STINGER!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        BattleAmmunitionMode::Stinger,
    ))
}

/// Use the ordinary bounded multi-weapon selection and world/effect checkpoint.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_stinger(world, id, pilot, index).map(|mode| mode.stinger_message(index))
    })
}

/// Stinger seekers accept airborne units, including orbital drops; ground launch preparation is insufficient.
pub(super) fn target_airborne(world: &World, target: ObjectId) -> bool {
    world
        .btech
        .constructed_units()
        .get(&target)
        .is_some_and(|unit| unit.airborne())
        || world.btech.vehicles().get(&target).is_some_and(|unit| {
            unit.orbital_drop().is_some()
                || unit.vtol_flight().is_some_and(|flight| {
                    matches!(
                        flight.phase,
                        super::BattleVtolFlightPhase::Airborne
                            | super::BattleVtolFlightPhase::Falling
                    )
                })
        })
}
