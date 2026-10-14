//! Unit storage adapters feed one ordered AMS selection and expenditure policy.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// One selected mount and its first usable normal-ammunition bin; laser AMS draws no ammunition.
pub(super) struct Defense {
    pub index: usize,
    pub weapon: Weapon,
    pub bin: Option<usize>,
}

/// Inspect the saved whole-unit switch without requiring a specific construction class.
pub(super) fn enabled(world: &World, id: ObjectId) -> Result<bool> {
    let unit = world.btech.unit(id).context("Unit is unavailable")?;
    Ok(unit.ams_enabled())
}

/// Store a switch after the caller has checked cockpit authority and installation.
pub(super) fn set_enabled(world: &mut World, id: ObjectId, enabled: bool) -> Result<()> {
    super::with_unit_mut!(
        world.btech.unit_mut(id).context("Unit is unavailable")?,
        |unit| {
            unit.ams_enabled = enabled;
            Ok(())
        }
    )
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

/// Project temporary failure, bin state and arc coverage into the common mount-order selection.
/// Only a mount whose firing arc covers the attacker may engage (Total Warfare p. 129).
pub(super) fn select(world: &World, id: ObjectId, attacker: ObjectId) -> Result<Option<Defense>> {
    let covering = covering_mounts(world, id, attacker)?;
    super::with_unit!(
        world.btech.unit(id).context("Unit is unavailable")?,
        |unit| {
            let loadout = unit.loadout()?;
            Ok(first_defense(
                &loadout.weapons,
                &loadout.ammunition,
                |index, mount| {
                    covering.contains(&index)
                        && mount
                            .criticals
                            .iter()
                            .all(|location| !unit.critical_unavailable(*location))
                        && !unit.weapon_failures.contains_key(&index)
                        && !unit.jammed_weapons.contains(&index)
                        && !unit.weapon_recycle.contains_key(&index)
                },
                |index, bin| {
                    unit.ammunition()[index] > 0 && !unit.critical_unavailable(bin.location)
                },
                |left, right| left.section == right.section,
            ))
        }
    )
}

/// The defender's AMS mounts whose firing arcs cover the attacker's bearing.
fn covering_mounts(
    world: &World,
    id: ObjectId,
    attacker: ObjectId,
) -> Result<std::collections::BTreeSet<usize>> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        let bearing = super::unit_range(world, id, attacker)?
            .bearing
            .unwrap_or(180.0);
        return unit
            .loadout()?
            .weapons
            .iter()
            .enumerate()
            .filter(|(_, mount)| mount.weapon.is_ams())
            .filter_map(|(index, _)| match unit.weapon_bears_on(index, bearing) {
                Ok(true) => Some(Ok(index)),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            })
            .collect();
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is unavailable")?;
    unit.loadout()?
        .weapons
        .iter()
        .enumerate()
        .filter(|(_, mount)| mount.weapon.is_ams())
        .filter_map(
            |(index, _)| match super::weapon_bears_on(world, id, attacker, index) {
                Ok(true) => Some(Ok(index)),
                Ok(false) => None,
                Err(error) => Some(Err(error)),
            },
        )
        .collect()
}

/// Select the first ready mount, then a matching bin with local-section preference. Laser AMS
/// needs no bin. A ready but unloaded mount does not cause another mount to activate in the same attack.
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
    if mount.weapon.profile().ammunition_per_ton == 0 {
        return Some(Defense {
            index,
            weapon: mount.weapon,
            bin: None,
        });
    }
    let (bin, _) = bins
        .iter()
        .enumerate()
        .filter(|(index, bin)| {
            bin.weapon == mount.weapon
                && bin.mode == AmmunitionMode::Normal
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
        bin: Some(bin),
    })
}

/// Commit one engagement to its owner: one round from the selected bin (laser AMS none), the
/// system's heat and the mount's recycle (Total Warfare p. 130). Returns the rounds spent.
pub(super) fn expend(
    world: &mut World,
    id: ObjectId,
    index: usize,
    weapon: Weapon,
    bin: Option<usize>,
) -> Result<u16> {
    let recycle = world.btech.weapon_settings.recycle_seconds(weapon);
    if let Some(unit) = world.btech.vehicles.get_mut(&id) {
        let spent = bin.map_or(0, |bin| unit.ammunition()[bin].min(1));
        if let Some(bin) = bin {
            unit.expend_reserved_ammunition(bin, spent)?;
        }
        unit.weapon_recycle.insert(index, u16::from(recycle));
        return Ok(spent);
    }
    let unit = world
        .btech
        .constructed
        .get_mut(&id)
        .context("Unit is unavailable")?;
    let spent = bin.map_or(0, |bin| unit.ammunition[bin].min(1));
    if let Some(bin) = bin {
        unit.ammunition[bin] -= spent;
    }
    unit.live_mass.invalidate();
    unit.heat.stored += f64::from(weapon.profile().heat);
    unit.weapon_recycle.insert(index, u16::from(recycle));
    Ok(spent)
}
