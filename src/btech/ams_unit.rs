//! Unit storage adapters feed one ordered AMS selection and expenditure policy.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use std::sync::Arc;

/// One selected mount and its first usable normal-ammunition bin.
pub(super) struct Defense {
    pub index: usize,
    pub weapon: BattleWeapon,
    pub bin: usize,
}

/// Inspect the saved whole-unit switch without requiring a specific construction class.
pub(super) fn enabled(world: &World, id: ObjectId) -> Result<bool> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Ok(unit.ams_enabled());
    }
    Ok(world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is unavailable")?
        .ams_enabled())
}

/// Store a switch after the caller has checked cockpit authority and installation.
pub(super) fn set_enabled(world: &mut World, id: ObjectId, enabled: bool) -> Result<()> {
    if let Some(unit) = Arc::make_mut(&mut world.btech.vehicles).get_mut(&id) {
        unit.ams_enabled = enabled;
        return Ok(());
    }
    Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .context("Unit is unavailable")?
        .ams_enabled = enabled;
    Ok(())
}

/// Critical loss of any installed AMS disables the whole capability for either construction class.
pub(super) fn available(world: &World, id: ObjectId) -> Result<bool> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Ok(installed(&unit.loadout()?.weapons, |location| {
            unit.critical_destroyed(location)
        }));
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is unavailable")?;
    Ok(installed(&unit.loadout()?.weapons, |location| {
        unit.critical_destroyed(location)
            || unit
                .weapon_damage()
                .iter()
                .any(|damage| damage.location == location)
    }))
}

/// Shared installed-capability policy, independent of anatomy and temporary readiness.
fn installed<L: Copy>(weapons: &[WeaponMount<L>], destroyed: impl Fn(L) -> bool) -> bool {
    let mut installed = false;
    for mount in weapons.iter().filter(|mount| mount.weapon.is_ams()) {
        installed = true;
        if mount.criticals.iter().any(|location| destroyed(*location)) {
            return false;
        }
    }
    installed
}

/// Project temporary failure and bin state into the common mount-order selection.
pub(super) fn select(world: &World, id: ObjectId) -> Result<Option<Defense>> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        let loadout = unit.loadout()?;
        return Ok(first_defense(
            &loadout.weapons,
            &loadout.ammunition,
            |index, mount| {
                mount
                    .criticals
                    .iter()
                    .all(|location| !unit.critical_unavailable(*location))
                    && !unit.weapon_failures.contains_key(&index)
                    && !unit.jammed_weapons.contains(&index)
                    && !unit.weapon_recycle.contains_key(&index)
            },
            |index, bin| unit.ammunition()[index] > 0 && !unit.critical_unavailable(bin.location),
            |left, right| left.section == right.section,
        ));
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is unavailable")?;
    let loadout = unit.loadout()?;
    Ok(first_defense(
        &loadout.weapons,
        &loadout.ammunition,
        |index, mount| {
            mount
                .criticals
                .iter()
                .all(|location| !unit.critical_unavailable(*location))
                && !unit.weapon_recycle().contains_key(&index)
        },
        |index, bin| unit.ammunition()[index] > 0 && !unit.critical_unavailable(bin.location),
        |left, right| left.section == right.section,
    ))
}

/// Select the first ready mount, then a matching bin with local-section preference.
/// A ready but unloaded mount does not cause another mount to activate in the same attack.
fn first_defense<L: Copy + Ord>(
    weapons: &[WeaponMount<L>],
    bins: &[AmmunitionBin<L>],
    ready: impl Fn(usize, &WeaponMount<L>) -> bool,
    supplied: impl Fn(usize, &AmmunitionBin<L>) -> bool,
    same_section: impl Fn(L, L) -> bool,
) -> Option<Defense> {
    let (index, mount) = weapons
        .iter()
        .enumerate()
        .find(|(index, mount)| mount.weapon.is_ams() && ready(*index, mount))?;
    let (bin, _) = bins
        .iter()
        .enumerate()
        .filter(|(index, bin)| {
            bin.weapon == mount.weapon
                && bin.mode == BattleAmmunitionMode::Normal
                && supplied(*index, bin)
        })
        .min_by_key(|(_, bin)| {
            (
                !same_section(bin.location, mount.criticals[0]),
                bin.location,
            )
        })?;
    Some(Defense {
        index,
        weapon: mount.weapon,
        bin,
    })
}

/// Commit capped supply and recycle to its owner; both carriers store weapon heat.
pub(super) fn expend(
    world: &mut World,
    id: ObjectId,
    index: usize,
    weapon: BattleWeapon,
    bin: usize,
    rounds: u16,
) -> Result<u16> {
    let recycle = world.btech.weapon_settings.recycle_seconds(weapon);
    if let Some(unit) = Arc::make_mut(&mut world.btech.vehicles).get_mut(&id) {
        let spent = unit.ammunition()[bin].min(rounds);
        unit.expend_reserved_ammunition(bin, spent)?;
        unit.weapon_heat += f64::from(weapon.profile().heat);
        unit.weapon_recycle.insert(index, u16::from(recycle));
        return Ok(spent);
    }
    let unit = Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .context("Unit is unavailable")?;
    let spent = unit.ammunition[bin].min(rounds);
    unit.ammunition[bin] -= spent;
    unit.live_mass.invalidate();
    unit.heat.stored += f64::from(weapon.profile().heat);
    unit.weapon_recycle.insert(index, u16::from(recycle));
    Ok(spent)
}
