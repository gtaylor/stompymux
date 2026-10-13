//! Vehicle weapon failures share powered recycle state and the saved critical-selection dice stream.
use super::{EquipmentFailure, Notice, Vehicle, VehicleSection, Weapon};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::collections::BTreeMap;

/// Applied critical failure and its initial recovery duration for caller-owned notification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[must_use = "Publish the critical notice in the enclosing damage transaction"]
pub struct VehicleWeaponJam {
    pub index: usize,
    pub weapon: Weapon,
    pub failure: EquipmentFailure,
    pub seconds: u16,
}

impl VehicleWeaponJam {
    /// Feedback for the occupants of the affected vehicle.
    pub fn notice(&self, unit: ObjectId) -> Notice {
        let name = self.weapon.name().split_once('.').unwrap().1;
        let text = match self.failure {
            EquipmentFailure::Jammed => {
                format!("[fg=red bold]The shot temporarily jams your {name}![reset]")
            }
            EquipmentFailure::Disabled => {
                format!("[fg=red bold]Your {name} is jammed![reset]")
            }
            EquipmentFailure::Shorted => {
                format!("[fg=red bold]The shot causes your {name} to temporarily short out![reset]")
            }
            EquipmentFailure::Dud
            | EquipmentFailure::Empty
            | EquipmentFailure::AmmunitionJam
            | EquipmentFailure::CriticalAmmunitionJam => {
                format!("[fg=red bold]Your {name} cannot fire![reset]")
            }
        };
        Notice { unit, text }
    }
}

impl Vehicle {
    /// Critical failures keyed by weapon index; any active duration is in `weapon_recycle`.
    pub fn weapon_failures(&self) -> &BTreeMap<usize, EquipmentFailure> {
        &self.weapon_failures
    }
}

/// Select and disable an intact, unaffected weapon in the hit section for 60–120 powered seconds.
/// Recycling or empty weapons remain eligible. An empty candidate set consumes no dice.
pub fn jam_vehicle_weapon(
    world: &mut World,
    id: ObjectId,
    section: VehicleSection,
) -> Result<Option<VehicleWeaponJam>> {
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
    let failure = EquipmentFailure::for_weapon(weapon);
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    vehicle.weapon_failures.insert(index, failure);
    vehicle.weapon_recycle.insert(index, seconds);
    vehicle.dice = dice;
    Ok(Some(VehicleWeaponJam {
        index,
        weapon,
        failure,
        seconds,
    }))
}
