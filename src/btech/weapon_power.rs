//! Persistent Gauss power-down state with shared cockpit admission and selection behavior.
use super::{BattleNotice, WeaponMount};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};
use std::{collections::BTreeSet, sync::Arc};

/// Saved power-down identities remain valid after physical destruction but must name Gauss mounts.
pub(super) fn validate<L>(
    powered_down: &BTreeSet<usize>,
    weapons: &[WeaponMount<L>],
) -> Result<()> {
    ensure!(
        powered_down.iter().all(|index| weapons
            .get(*index)
            .is_some_and(|mount| mount.weapon.weapon_explosion_damage() > 0)),
        "Invalid powered-down Gauss weapon"
    );
    Ok(())
}

/// Power down a charged Gauss mount without destroying equipment, spending ammunition or rolling dice.
/// No cockpit power-up action exists; the state survives ordinary shutdown and restart.
pub fn disable_gauss_weapon(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleNotice> {
    super::power::controlled_running_unit(world, id, pilot)?;
    let (ready, intact) = if let Some(unit) = world.btech.vehicles().get(&id) {
        (
            unit.weapon_readiness(index)?,
            !unit.critical_unavailable(
                unit.loadout()?
                    .weapons
                    .get(index)
                    .ok_or_else(|| anyhow::anyhow!("Weapon index out of bounds"))?
                    .criticals[0],
            ),
        )
    } else {
        let unit = &world.btech.constructed_units()[&id];
        (unit.weapon_readiness(index)?, unit.weapon_intact(index)?)
    };
    ensure!(intact, "That weapon has been destroyed!");
    ensure!(
        ready.weapon.weapon_explosion_damage() > 0,
        "You can only disable Gauss weapons."
    );
    ensure!(
        ready.recycle_remaining == 0,
        "That weapon is still recharging!"
    );
    if world.btech.vehicles().contains_key(&id) {
        Arc::make_mut(&mut world.btech.vehicles)
            .get_mut(&id)
            .unwrap()
            .powered_down_weapons
            .insert(index);
    } else {
        Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&id)
            .unwrap()
            .powered_down_weapons
            .insert(index);
    }
    Ok(BattleNotice {
        unit: id,
        text: format!("You power down weapon {index}."),
    })
}

/// Native ranges, comma clauses, partial mechanical rejections and publication rollback are shared.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        disable_gauss_weapon(world, id, pilot, index).map(|notice| notice.text)
    })
}
