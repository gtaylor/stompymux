//! Shared authorization, mechanical guards and state updates for cockpit weapon-mode controls.
use super::{FireMode, Power, WeaponReadiness};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};

/// Validate cockpit authority, physical loss, recycle and manual feed jams in reference order.
/// Vehicle critical failures remain separate from manually cleared ammunition-feed jams.
pub(super) fn ready_weapon(
    world: &World,
    id: ObjectId,
    pilot: ObjectId,
    index: usize,
) -> Result<WeaponReadiness> {
    let (ready, feed_jammed) = crate::btech::with_unit!(world.btech.unit(id).unwrap(), |unit| {
        super::power::controlled(world, id, pilot)?;
        ensure!(unit.power() == Power::Running, "Start the unit first");
        (
            unit.weapon_readiness(index)?,
            unit.jammed_weapons.contains(&index),
        )
    });
    ensure!(ready.intact, "That weapon has been destroyed");
    ensure!(
        ready.recycle_remaining == 0,
        "That weapon is still recycling"
    );
    ensure!(
        !feed_jammed,
        "The ammo feed mechanism for that weapon is jammed! Unable to change modes!"
    );
    let one_shot = crate::btech::with_unit!(world.btech.unit(id).unwrap(), |unit| {
        unit.loadout()?.weapons[index].one_shot
    });
    ensure!(!one_shot, "One-shot weapons' mode cannot be altered!");
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        ensure!(
            !unit.weapon_damage_effects(index)?.feed_locked,
            "That weapon's ammo feed mechanism is damaged!"
        );
    }
    Ok(ready)
}

/// Set a previously authorized and equipment-validated mode, storing normal implicitly.
/// Reports whether the setting changed, including repeated rotary burst selection.
pub(super) fn set_fire_mode(world: &mut World, id: ObjectId, index: usize, mode: FireMode) -> bool {
    let modes = fire_modes_mut(world, id);
    let previous = modes.get(&index).copied().unwrap_or_default();
    if mode == FireMode::Normal {
        modes.remove(&index);
    } else {
        modes.insert(index, mode);
    }
    previous != mode
}

/// Toggle a validated non-normal mode, sharing implicit-normal storage with explicit selection.
pub(super) fn toggle_fire_mode(
    world: &mut World,
    id: ObjectId,
    index: usize,
    selected: FireMode,
) -> FireMode {
    let current = fire_modes_mut(world, id)
        .get(&index)
        .copied()
        .unwrap_or_default();
    let mode = if current == selected {
        FireMode::Normal
    } else {
        selected
    };
    set_fire_mode(world, id, index, mode);
    mode
}

/// Toggle an equipment-validated ammunition selection without copying storage rules between controls.
pub(super) fn toggle_ammunition_mode(
    world: &mut World,
    id: ObjectId,
    index: usize,
    selected: super::AmmunitionMode,
) -> super::AmmunitionMode {
    let current = ammunition_modes_mut(world, id)
        .get(&index)
        .copied()
        .unwrap_or_default();
    let mode = if !weapon(world, id, index).is_some_and(super::Weapon::is_mml) {
        if current == selected {
            super::AmmunitionMode::Normal
        } else {
            selected
        }
    } else if selected == super::AmmunitionMode::MmlLrm {
        // The family control keeps a compatible special round and otherwise falls back to normal.
        let long_range = !current.is_mml_lrm();
        current
            .with_mml_family(long_range)
            .or_else(|| super::AmmunitionMode::Normal.with_mml_family(long_range))
            .unwrap_or_default()
    } else {
        let munition = if current.munition() == selected.munition() {
            super::AmmunitionMode::Normal
        } else {
            selected.munition()
        };
        current.with_munition(munition)
    };
    set_ammunition_mode(world, id, index, mode);
    mode
}

/// Whether a cockpit control may select `munition` for this weapon. MML launchers also require
/// the round to exist in the currently selected short- or long-range family.
pub(super) fn selectable_munition(
    world: &World,
    id: ObjectId,
    index: usize,
    munition: super::AmmunitionMode,
) -> bool {
    let Some(weapon) = weapon(world, id, index) else {
        return false;
    };
    if !weapon.is_mml() {
        return munition.supports(weapon);
    }
    let current = crate::btech::with_unit!(world.btech.unit(id).unwrap(), |unit| {
        unit.ammunition_mode(index)
    });
    current.is_ok_and(|current| munition.with_mml_family(current.is_mml_lrm()).is_some())
}

/// The mounted weapon at `index` for either unit class.
fn weapon(world: &World, id: ObjectId, index: usize) -> Option<super::Weapon> {
    super::with_unit!(world.btech.unit(id)?, |unit| {
        unit.loadout()
            .ok()?
            .weapons
            .get(index)
            .map(|mount| mount.weapon)
    })
}

/// Select the owning class's firing-mode storage after authorization.
fn fire_modes_mut(
    world: &mut World,
    id: ObjectId,
) -> &mut std::collections::BTreeMap<usize, FireMode> {
    if world.btech.vehicles().contains_key(&id) {
        return &mut world
            .btech
            .vehicles
            .get_mut(&id)
            .expect("authorized vehicle")
            .fire_modes;
    }
    &mut world
        .btech
        .constructed
        .get_mut(&id)
        .expect("authorized unit")
        .fire_modes
}

/// Set an authorized ammunition selection, retaining implicit normal state for both classes.
pub(super) fn set_ammunition_mode(
    world: &mut World,
    id: ObjectId,
    index: usize,
    mode: super::AmmunitionMode,
) {
    let modes = ammunition_modes_mut(world, id);
    if mode == super::AmmunitionMode::Normal {
        modes.remove(&index);
    } else {
        modes.insert(index, mode);
    }
}

/// Borrow the owning unit's ammunition-mode storage after authorization.
fn ammunition_modes_mut(
    world: &mut World,
    id: ObjectId,
) -> &mut std::collections::BTreeMap<usize, super::AmmunitionMode> {
    if world.btech.vehicles().contains_key(&id) {
        &mut world
            .btech
            .vehicles
            .get_mut(&id)
            .expect("authorized vehicle")
            .ammunition_modes
    } else {
        &mut world
            .btech
            .constructed
            .get_mut(&id)
            .expect("authorized unit")
            .ammunition_modes
    }
}
