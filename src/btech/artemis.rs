//! Derived Artemis links share matching and availability policy across unit anatomies.
use super::{
    CriticalLocation, Mech, MechSection, System, SystemCritical, Vehicle, VehicleCriticalLocation,
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
/// cluster bonus and one to hit. Either unit class reads its chassis technology flag.
pub(super) fn artemis_v(world: &crate::World, id: crate::ObjectId) -> bool {
    let technology = super::Technology::ArtemisV;
    world
        .btech
        .constructed_units()
        .get(&id)
        .map(|unit| unit.definition().has_technology(technology))
        .or_else(|| {
            world
                .btech
                .vehicles()
                .get(&id)
                .map(|vehicle| vehicle.definition().has_technology(technology))
        })
        .unwrap_or(false)
}

/// Match authored links once; callers supply only section relationships and equipment availability.
fn controllers<L: Copy>(
    weapons: &[WeaponMount<L>],
    systems: &[SystemCritical<L>],
    describe: impl Fn(L) -> Result<(u8, bool)>,
    slot: impl Fn(L) -> u8,
    sections_match: impl Fn(L, L) -> bool,
) -> Result<Vec<ArtemisController<L>>> {
    systems
        .iter()
        .filter(|part| part.system == System::ArtemisIv)
        .map(|part| {
            let (link, operational) = describe(part.location)?;
            let weapon_indices = weapons
                .iter()
                .enumerate()
                .filter(|(_, mount)| {
                    let primary = mount.criticals[0];
                    mount.weapon.profile().missiles > 0
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
    /// Mechs permit same-section links and a head controller for a center-torso launcher.
    pub fn artemis_controllers(&self) -> Result<Vec<ArtemisController>> {
        let loadout = self.loadout()?;
        controllers(
            &loadout.weapons,
            &loadout.systems,
            |location| {
                let part = &self.definition().sections[&location.section].criticals[&location.slot];
                Ok((link(&part.data)?, !self.critical_unavailable(location)))
            },
            |location| location.slot,
            |controller, primary| {
                controller.section == primary.section
                    || (controller.section == MechSection::Head
                        && primary.section == MechSection::CenterTorso)
            },
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
}

impl Vehicle {
    /// Ground turrets can use rear controllers; VTOLs require a same-section controller.
    pub fn artemis_controllers(&self) -> Result<Vec<ArtemisController<VehicleCriticalLocation>>> {
        let loadout = self.loadout()?;
        controllers(
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
}
