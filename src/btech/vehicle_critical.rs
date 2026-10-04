//! Vehicle equipment losses preserve construction while disabling individual installed slots.
use super::{BattleVehicle, VehicleCriticalLocation};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::collections::BTreeSet;

impl BattleVehicle {
    /// Explicit equipment losses; section destruction also makes all its slots unavailable.
    pub fn lost_criticals(&self) -> &BTreeSet<VehicleCriticalLocation> {
        &self.lost_criticals
    }

    /// Availability of an installed slot, including implicit loss of its containing section.
    pub fn critical_destroyed(&self, location: VehicleCriticalLocation) -> bool {
        self.lost_criticals.contains(&location)
            || self
                .sections()
                .get(&location.section)
                .is_none_or(|section| section.internal == 0)
    }

    /// The bin a self-destruct detonates: the most destructive loaded and available one,
    /// first on a tie.
    pub fn largest_ammunition_hazard_bin(&self) -> Result<Option<usize>> {
        let loadout = self.loadout()?;
        let mut largest = None;
        let mut maximum = 0;
        for (index, bin) in loadout.ammunition.iter().enumerate() {
            let damage = bin
                .weapon
                .ammunition_explosion_damage_for_mode(self.ammunition()[index], bin.mode);
            if damage > maximum && !self.critical_unavailable(bin.location) {
                largest = Some(index);
                maximum = damage;
            }
        }
        Ok(largest)
    }

    /// A weapon requires every mounting slot to remain operational; ammo and recycle are separate.
    pub fn weapon_intact(&self, index: usize) -> Result<bool> {
        let loadout = self.loadout()?;
        let weapon = loadout
            .weapons
            .get(index)
            .context("Weapon index out of bounds")?;
        Ok(weapon
            .criticals
            .iter()
            .all(|location| !self.critical_unavailable(*location)))
    }

    /// Vacuum disables equipment without destroying the slot or expending its ammunition.
    pub fn critical_unavailable(&self, location: VehicleCriticalLocation) -> bool {
        self.critical_destroyed(location) || self.breached_sections.contains(&location.section)
    }

    /// Persisted vacuum breaches, independent of later map conditions.
    pub fn breached_sections(&self) -> &BTreeSet<super::BattleVehicleSection> {
        &self.breached_sections
    }

    /// Mark a slot lost, cancelling its weapon timer or emptying its ammunition bin.
    /// Returns false for already unavailable equipment. The enclosing attack owns
    /// explosions, system-specific consequences, crew effects and notices.
    pub fn destroy_critical(&mut self, location: VehicleCriticalLocation) -> Result<bool> {
        ensure!(
            self.definition()
                .sections
                .get(&location.section)
                .is_some_and(|section| section.criticals.contains_key(&location.slot)),
            "Vehicle equipment slot is not installed"
        );
        if self.critical_destroyed(location) {
            return Ok(false);
        }
        let loadout = self.loadout()?;
        if let Some(index) = loadout
            .ammunition
            .iter()
            .position(|bin| bin.location == location)
        {
            let rounds = self.ammunition()[index];
            // A destroyed hull already makes all equipment unavailable for gameplay.
            if rounds > 0 {
                self.expend_ammunition(index, rounds)?;
            }
        }
        for (index, mount) in loadout.weapons.iter().enumerate() {
            if mount.criticals.contains(&location) {
                self.jammed_weapons.remove(&index);
                self.weapon_recycle.remove(&index);
                self.weapon_failures.remove(&index);
            }
        }
        if loadout
            .systems
            .iter()
            .any(|part| part.location == location && part.system == super::BattleSystem::LightProbe)
        {
            self.critical_conditions.lose_light_probe();
        }
        self.lost_criticals.insert(location);
        self.reconcile_electronics();
        Ok(true)
    }
}

/// Apply an explicit equipment loss within a caller-owned world/damage transaction.
pub fn destroy_vehicle_critical(
    world: &mut World,
    id: ObjectId,
    location: VehicleCriticalLocation,
) -> Result<bool> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Vehicle is unavailable"
    );
    let vehicle = world
        .btech
        .vehicles
        .get_mut(&id)
        .context("Vehicle is unavailable")?;
    vehicle.destroy_critical(location)
}

impl BattleVehicle {
    /// Surviving weapons in slot order for a section-local weapon critical.
    /// Empty ammunition, expenditure and recycling do not protect an intact weapon.
    pub fn weapon_critical_candidates(
        &self,
        section: super::BattleVehicleSection,
    ) -> Result<Vec<usize>> {
        if self.is_destroyed() {
            return Ok(Vec::new());
        }
        Ok(self
            .loadout()?
            .weapons
            .iter()
            .enumerate()
            .filter(|(_, mount)| {
                mount.criticals[0].section == section
                    && !self.critical_unavailable(mount.criticals[0])
            })
            .map(|(index, _)| index)
            .collect())
    }
}

/// Select a surviving section-local weapon with this vehicle's committed random stream.
/// Empty candidate sets consume no dice. The caller must apply the selected weapon's
/// critical consequences, including explosive equipment effects, in the same transaction.
#[must_use = "Apply the selected weapon critical before committing the enclosing damage transaction"]
pub fn select_vehicle_weapon_critical(
    world: &mut World,
    id: ObjectId,
    section: super::BattleVehicleSection,
) -> Result<Option<usize>> {
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
    let candidates = vehicle.weapon_critical_candidates(section)?;
    if candidates.is_empty() {
        return Ok(None);
    }
    let count = u16::try_from(candidates.len()).context("Too many vehicle weapons")?;
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    let selected = usize::from(vehicle.dice.die(count)? - 1);
    Ok(Some(candidates[selected]))
}
