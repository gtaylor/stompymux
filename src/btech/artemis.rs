//! Derived Artemis links share matching and availability policy across unit anatomies.
use super::{
    BattleSection, BattleSystem, BattleUnit, BattleVehicle, BattleVehicleSection, CriticalLocation,
    SystemCritical, VehicleCriticalLocation, WeaponMount,
};
use anyhow::{Result, ensure};
use serde::Serialize;

/// Installed controller metadata, without a second copy of link or equipment damage state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleArtemisController<L = CriticalLocation> {
    pub location: L,
    /// One-based template link, or zero for an unassigned controller.
    pub link: u8,
    /// Stable indices of missile mounts whose primary critical matches the link.
    pub weapon_indices: Vec<usize>,
    pub operational: bool,
}

/// Match authored links once; callers supply only section relationships and equipment availability.
fn controllers<L: Copy>(
    weapons: &[WeaponMount<L>],
    systems: &[SystemCritical<L>],
    describe: impl Fn(L) -> Result<(u8, bool)>,
    slot: impl Fn(L) -> u8,
    sections_match: impl Fn(L, L) -> bool,
) -> Result<Vec<BattleArtemisController<L>>> {
    systems
        .iter()
        .filter(|part| part.system == BattleSystem::ArtemisIv)
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
            Ok(BattleArtemisController {
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
fn operational<L>(controllers: &[BattleArtemisController<L>], index: usize) -> bool {
    controllers
        .iter()
        .any(|controller| controller.operational && controller.weapon_indices.contains(&index))
}

impl BattleUnit {
    /// Mechs permit same-section links and a head controller for a center-torso launcher.
    pub fn artemis_controllers(&self) -> Result<Vec<BattleArtemisController>> {
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
                    || (controller.section == BattleSection::Head
                        && primary.section == BattleSection::CenterTorso)
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

impl BattleVehicle {
    /// Ground turrets can use rear controllers; VTOLs require a same-section controller.
    pub fn artemis_controllers(
        &self,
    ) -> Result<Vec<BattleArtemisController<VehicleCriticalLocation>>> {
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
                        && controller.section == BattleVehicleSection::Rear
                        && primary.section == BattleVehicleSection::Turret)
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
