//! Derived Artemis and MRM "Apollo" fire-control links share matching and availability policy
//! across unit anatomies.
use super::{
    CriticalLocation, Mech, System, SystemCritical, Vehicle, VehicleCriticalLocation,
    VehicleSection, WeaponMount,
};
use anyhow::{Result, ensure};
use serde::Serialize;

/// Installed controller metadata, without a second copy of link or equipment damage state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtemisController<L = CriticalLocation> {
    pub location: L,
    /// One-based template link, or zero for an unassigned controller.
    pub link: u8,
    /// Stable indices of missile mounts whose primary critical matches the link.
    pub weapon_indices: Vec<usize>,
    pub operational: bool,
}

/// Whether a unit's Artemis controllers are Artemis V, which adds one more to the Artemis
/// cluster bonus and one to hit. Artemis V is its own two-slot system (Tactical Operations:
/// Advanced Units & Equipment p. 217); either unit class checks its installed systems.
pub(super) fn artemis_v(world: &crate::World, id: crate::ObjectId) -> bool {
    let installed = |systems: Vec<System>| systems.contains(&System::ArtemisV);
    world
        .btech
        .constructed_units()
        .get(&id)
        .and_then(|unit| unit.loadout().ok())
        .map(|loadout| installed(loadout.systems.iter().map(|part| part.system).collect()))
        .or_else(|| {
            world
                .btech
                .vehicles()
                .get(&id)
                .and_then(|vehicle| vehicle.loadout().ok())
                .map(|loadout| installed(loadout.systems.iter().map(|part| part.system).collect()))
        })
        .unwrap_or(false)
}

/// Which fire-control systems a set of controllers covers and the launchers they serve.
#[derive(Clone, Copy, PartialEq, Eq)]
enum FireControl {
    /// Artemis IV and V serve missile launchers.
    Artemis,
    /// The MRM "Apollo" system serves MRM launchers.
    Apollo,
}

impl FireControl {
    /// Whether `system` is one of these controllers.
    fn controls(self, system: System) -> bool {
        match self {
            Self::Artemis => matches!(system, System::ArtemisIv | System::ArtemisV),
            Self::Apollo => system == System::ApolloFcs,
        }
    }

    /// Whether `weapon` is a launcher these controllers serve.
    fn serves(self, weapon: super::Weapon) -> bool {
        match self {
            Self::Artemis => weapon.profile().missiles > 0,
            Self::Apollo => weapon.is_mrm(),
        }
    }
}

/// Match authored links once; callers supply only section relationships and equipment availability.
fn controllers<L: Copy>(
    kind: FireControl,
    weapons: &[WeaponMount<L>],
    systems: &[SystemCritical<L>],
    describe: impl Fn(L) -> Result<(u8, bool)>,
    slot: impl Fn(L) -> u8,
    sections_match: impl Fn(L, L) -> bool,
) -> Result<Vec<ArtemisController<L>>> {
    systems
        .iter()
        .filter(|part| kind.controls(part.system))
        .map(|part| {
            let (link, operational) = describe(part.location)?;
            let weapon_indices = weapons
                .iter()
                .enumerate()
                .filter(|(_, mount)| {
                    let primary = mount.criticals[0];
                    kind.serves(mount.weapon)
                        && slot(primary) + 1 == link
                        && sections_match(part.location, primary)
                })
                .map(|(index, _)| index)
                .collect();
            Ok(ArtemisController {
                location: part.location,
                link,
                weapon_indices,
                operational,
            })
        })
        .collect()
}

/// Unassigned equipment retains zero; invalid authored numbers remain construction errors.
fn link(data: &str) -> Result<u8> {
    if data == "-" {
        return Ok(0);
    }
    Ok(data.parse()?)
}

/// A controller must both survive and refer to the requested mount.
fn operational<L>(controllers: &[ArtemisController<L>], index: usize) -> bool {
    controllers
        .iter()
        .any(|controller| controller.operational && controller.weapon_indices.contains(&index))
}

impl Mech {
    /// A Mech's controller serves launchers in its own location (TechManual p. 206).
    pub fn artemis_controllers(&self) -> Result<Vec<ArtemisController>> {
        let loadout = self.loadout()?;
        controllers(
            FireControl::Artemis,
            &loadout.weapons,
            &loadout.systems,
            |location| {
                let part = &self.definition().sections[&location.section].criticals[&location.slot];
                Ok((link(&part.data)?, !self.critical_unavailable(location)))
            },
            |location| location.slot,
            |controller, primary| controller.section == primary.section,
        )
    }

    /// Inspect a linked, live controller without altering the selected ammunition mode.
    pub fn artemis_operational(&self, index: usize) -> Result<bool> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        Ok(operational(&self.artemis_controllers()?, index))
    }

    /// The MRM "Apollo" fire-control systems and the launchers in their own location they
    /// serve (Tactical Operations: Advanced Units & Equipment p. 142).
    pub fn apollo_controllers(&self) -> Result<Vec<ArtemisController>> {
        let loadout = self.loadout()?;
        controllers(
            FireControl::Apollo,
            &loadout.weapons,
            &loadout.systems,
            |location| {
                let part = &self.definition().sections[&location.section].criticals[&location.slot];
                Ok((link(&part.data)?, !self.critical_unavailable(location)))
            },
            |location| location.slot,
            |controller, primary| controller.section == primary.section,
        )
    }

    /// Whether a surviving Apollo system guides the MRM launcher at `index`. A destroyed one
    /// leaves the launcher firing as a plain MRM (Tactical Operations: Advanced Units &
    /// Equipment p. 143).
    pub fn apollo_operational(&self, index: usize) -> Result<bool> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        Ok(operational(&self.apollo_controllers()?, index))
    }
}

impl Vehicle {
    /// Ground turrets can use rear controllers; VTOLs require a same-section controller.
    pub fn artemis_controllers(&self) -> Result<Vec<ArtemisController<VehicleCriticalLocation>>> {
        let loadout = self.loadout()?;
        controllers(
            FireControl::Artemis,
            &loadout.weapons,
            &loadout.systems,
            |location| {
                let part = &self.definition().sections[&location.section].criticals[&location.slot];
                Ok((link(&part.data)?, !self.critical_unavailable(location)))
            },
            |location| location.slot,
            |controller, primary| {
                controller.section == primary.section
                    || (!self.definition().is_vtol()
                        && controller.section == VehicleSection::Rear
                        && primary.section == VehicleSection::Turret)
            },
        )
    }

    /// Inspect the same derived availability used by the shared cockpit control.
    pub fn artemis_operational(&self, index: usize) -> Result<bool> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        Ok(operational(&self.artemis_controllers()?, index))
    }

    /// The MRM "Apollo" fire-control systems and the launchers in their own location they
    /// serve (Tactical Operations: Advanced Units & Equipment p. 142).
    pub fn apollo_controllers(&self) -> Result<Vec<ArtemisController<VehicleCriticalLocation>>> {
        let loadout = self.loadout()?;
        controllers(
            FireControl::Apollo,
            &loadout.weapons,
            &loadout.systems,
            |location| {
                let part = &self.definition().sections[&location.section].criticals[&location.slot];
                Ok((link(&part.data)?, !self.critical_unavailable(location)))
            },
            |location| location.slot,
            |controller, primary| controller.section == primary.section,
        )
    }

    /// Whether a surviving Apollo system guides the MRM launcher at `index`. A destroyed one
    /// leaves the launcher firing as a plain MRM (Tactical Operations: Advanced Units &
    /// Equipment p. 143).
    pub fn apollo_operational(&self, index: usize) -> Result<bool> {
        ensure!(
            index < self.loadout()?.weapons.len(),
            "Weapon index out of bounds"
        );
        Ok(operational(&self.apollo_controllers()?, index))
    }
}

/// Whether a surviving Apollo system guides the MRM launcher at `index` on either unit class;
/// false for any other weapon or an unknown unit.
pub(super) fn apollo_operational(
    world: &crate::World,
    id: crate::ObjectId,
    index: usize,
) -> Result<bool> {
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        return unit.apollo_operational(index);
    }
    if let Some(vehicle) = world.btech.vehicles().get(&id) {
        return vehicle.apollo_operational(index);
    }
    Ok(false)
}
