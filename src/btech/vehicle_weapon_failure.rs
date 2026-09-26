//! Vehicle weapon failures share powered recycle state and the saved critical-selection dice stream.
use super::{
    BattleEquipmentFailure, BattleNotice, BattleVehicle, BattleVehicleSection, BattleWeapon,
};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::BTreeMap;

/// Applied critical failure and its initial recovery duration for caller-owned notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish the critical notice in the enclosing damage transaction"]
pub struct BattleVehicleWeaponJam {
    pub index: usize,
    pub weapon: BattleWeapon,
    pub failure: BattleEquipmentFailure,
    pub seconds: u16,
}

impl BattleVehicleWeaponJam {
    /// Feedback for the occupants of the affected vehicle.
    pub fn notice(&self, unit: ObjectId) -> BattleNotice {
        let name = self.weapon.name().split_once('.').unwrap().1;
        let text = match self.failure {
            BattleEquipmentFailure::Jammed => {
                format!("[fg=red bold]The shot temporarily jams your {name}![reset]")
            }
            BattleEquipmentFailure::Disabled => {
                format!("[fg=red bold]Your {name} is jammed![reset]")
            }
            BattleEquipmentFailure::Shorted => {
                format!("[fg=red bold]The shot causes your {name} to temporarily short out![reset]")
            }
            BattleEquipmentFailure::Dud
            | BattleEquipmentFailure::Empty
            | BattleEquipmentFailure::AmmunitionJam
            | BattleEquipmentFailure::CriticalAmmunitionJam => {
                format!("[fg=red bold]Your {name} cannot fire![reset]")
            }
        };
        BattleNotice { unit, text }
    }
}

impl BattleVehicle {
    /// Critical failures keyed by weapon index; any active duration is in `weapon_recycle`.
    pub fn weapon_failures(&self) -> &BTreeMap<usize, BattleEquipmentFailure> {
        &self.weapon_failures
    }
}

/// Select and disable an intact, unaffected weapon in the hit section for 60–120 powered seconds.
/// Recycling or empty weapons remain eligible. An empty candidate set consumes no dice.
pub fn jam_vehicle_weapon(
    world: &mut World,
    id: ObjectId,
    section: BattleVehicleSection,
) -> Result<Option<BattleVehicleWeaponJam>> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Vehicle is unavailable"
    );
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    let candidates: Vec<_> = vehicle
        .weapon_critical_candidates(section)?
        .into_iter()
        .filter(|index| !vehicle.weapon_failures.contains_key(index))
        .collect();
    if candidates.is_empty() {
        return Ok(None);
    }
    let loadout = vehicle.loadout()?;
    let count = u16::try_from(candidates.len()).context("Too many vehicle weapons")?;
    let mut dice = vehicle.dice.clone();
    let index = candidates[usize::from(dice.die(count)? - 1)];
    let seconds = dice.die(61)? + 59;
    let weapon = loadout.weapons[index].weapon;
    let failure = BattleEquipmentFailure::for_weapon(weapon);
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    vehicle.weapon_failures.insert(index, failure);
    vehicle.weapon_recycle.insert(index, seconds);
    vehicle.dice = dice;
    Ok(Some(BattleVehicleWeaponJam {
        index,
        weapon,
        failure,
        seconds,
    }))
}

/// Main-weapon critical selection; the disabled mount retains any existing recycle timer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish the main-weapon jam notice in the enclosing damage transaction"]
pub struct BattleVehicleMainWeaponJam {
    pub index: usize,
    pub weapon: BattleWeapon,
}

impl BattleVehicleMainWeaponJam {
    /// Feedback for occupants without implying that the mount was physically destroyed.
    pub fn notice(&self, unit: ObjectId) -> BattleNotice {
        BattleNotice {
            unit,
            text: format!(
                "[fg=red bold]Your {} is jammed![reset]",
                self.weapon.name().split_once('.').unwrap().1
            ),
        }
    }
}

/// Rank every intact weapon in section/slot order; the highest positive draw wins.
/// Existing failures and empty ammunition do not exclude mounts. No recovery timer is added.
pub fn jam_vehicle_main_weapon(
    world: &mut World,
    id: ObjectId,
) -> Result<Option<BattleVehicleMainWeaponJam>> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Vehicle is unavailable"
    );
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    if vehicle.is_destroyed() {
        return Ok(None);
    }
    let mut dice = vehicle.dice.clone();
    let selected = vehicle
        .rank_main_weapon(&mut dice)?
        .map(|(index, weapon)| BattleVehicleMainWeaponJam { index, weapon });
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    vehicle.dice = dice;
    if let Some(jam) = &selected {
        vehicle
            .weapon_failures
            .insert(jam.index, BattleEquipmentFailure::Disabled);
    }
    Ok(selected)
}

impl super::BattleVehicle {
    /// Rank intact mounts once for either main-weapon jamming or destruction.
    /// Empty ammunition and existing failures retain eligibility; ties keep the first mount.
    pub(super) fn rank_main_weapon(
        &self,
        dice: &mut super::BattleDice,
    ) -> Result<Option<(usize, super::BattleWeapon)>> {
        if self.is_destroyed() {
            return Ok(None);
        }
        let loadout = self.loadout()?;
        let mut highest = 0;
        let mut selected = None;
        for (index, mount) in loadout.weapons.iter().enumerate() {
            if self.critical_unavailable(mount.criticals[0]) {
                continue;
            }
            let rank = dice.rank_i31();
            if rank > highest {
                highest = rank;
                selected = Some((index, mount.weapon));
            }
        }
        Ok(selected)
    }
}
