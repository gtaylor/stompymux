//! Live ammunition selection on constructed units, and the cockpit controls that change it.
use super::{AmmunitionFeedback, BattleAmmunitionMode, BattleUnit};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

impl BattleUnit {
    /// Current selected ammunition type; independent of bin inventory and recycle readiness.
    pub fn ammunition_mode(&self, index: usize) -> Result<BattleAmmunitionMode> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        Ok(self
            .ammunition_modes
            .get(&index)
            .copied()
            .unwrap_or_default())
    }
}

/// Toggle an intact, recycled LB-X autocannon between slug and cluster ammunition.
pub fn toggle_lbx(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    let readiness = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    ensure!(readiness.weapon.is_lbx(), "That weapon cannot be set LBX!");
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        BattleAmmunitionMode::Cluster,
    ))
}

/// Toggle artillery cluster rounds, preserving an existing smoke or mine selection.
pub fn toggle_cluster(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    let readiness = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    let current = crate::btech::with_unit!(world.btech.unit(id).unwrap(), |unit| {
        unit.ammunition_mode(index)?
    });
    ensure!(readiness.weapon.is_artillery(), "Invalid weapon type!");
    ensure!(
        matches!(
            current,
            BattleAmmunitionMode::Normal | BattleAmmunitionMode::Cluster
        ),
        "That weapon has already been set to fire special rounds!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        BattleAmmunitionMode::Cluster,
    ))
}

/// Apply ordered cockpit selections through the shared weapon-control parser.
pub(crate) fn cluster_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_cluster(world, id, pilot, index).map(|mode| mode.cluster_message(index))
    })
}

/// Use the same bounded, ordered cockpit selection parser as other weapon mode commands.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_lbx(world, id, pilot, index).map(|mode| mode.message(index))
    })
}

/// Toggle compatible missile ammunition; controller lookup is required even when disabling the mode.
pub fn toggle_artemis(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<BattleAmmunitionMode> {
    let readiness = super::weapon_controls::ready_weapon(world, id, pilot, index)?;
    let operational = crate::btech::with_unit!(world.btech.unit(id).unwrap(), |unit| {
        unit.artemis_operational(index)?
    });
    ensure!(
        operational,
        "You do not have an Artemis system for that weapon."
    );
    ensure!(
        super::weapon_controls::selectable_munition(
            world,
            id,
            index,
            BattleAmmunitionMode::Artemis
        ) && !readiness.weapon.is_rocket(),
        "That weapon cannot be set ARTEMIS!"
    );
    Ok(super::weapon_controls::toggle_ammunition_mode(
        world,
        id,
        index,
        BattleAmmunitionMode::Artemis,
    ))
}

/// Native cockpit selection uses the same ordering and partial-error behavior as other modes.
pub(crate) fn artemis_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::fire_mode::selected_command(ctx, input, |world, id, pilot, index| {
        toggle_artemis(world, id, pilot, index).map(|mode| mode.artemis_message(index))
    })
}
