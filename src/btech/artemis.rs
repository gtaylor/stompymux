//! Derived Artemis and MRM "Apollo" fire-control links share matching and availability policy
//! across unit anatomies.
use super::{
    CriticalLocation, FireControl, Mech, System, SystemCritical, Vehicle, VehicleCriticalLocation,
    WeaponMount,
};
use anyhow::{Context, Result, anyhow, ensure};
use serde::Serialize;

/// Installed controller metadata, without a second copy of link or equipment damage state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ArtemisController<L = CriticalLocation> {
    pub location: L,
    /// One-based template link: the slot of the guided launcher's first critical.
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

/// Whether the launcher at `index` is guided by its own working Artemis V system: Artemis V
/// takes one off the to-hit number of the launcher it is linked to (Tactical Operations:
/// Advanced Units & Equipment p. 95), so a destroyed system, or one serving another launcher,
/// does nothing for it.
pub(super) fn artemis_v_guides(
    world: &crate::World,
    id: crate::ObjectId,
    index: usize,
) -> Result<bool> {
    fn guides<L: PartialEq + Copy>(
        controllers: &[ArtemisController<L>],
        systems: &[SystemCritical<L>],
        index: usize,
    ) -> bool {
        controllers.iter().any(|controller| {
            controller.operational
                && controller.weapon_indices.contains(&index)
                && systems.iter().any(|part| {
                    part.location == controller.location && part.system == System::ArtemisV
                })
        })
    }
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        return Ok(guides(
            &unit.artemis_controllers()?,
            &unit.loadout()?.systems,
            index,
        ));
    }
    if let Some(vehicle) = world.btech.vehicles().get(&id) {
        return Ok(guides(
            &vehicle.artemis_controllers()?,
            &vehicle.loadout()?.systems,
            index,
        ));
    }
    Ok(false)
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

/// The authored launcher link of the controller critical at a location. Construction refuses
/// a controller without one, so a missing or zero link is an error here too, never a
/// controller that silently guides nothing.
fn link(part: &super::CriticalDefinition) -> Result<u8> {
    super::fire_control_link(&part.data).map_err(|error| anyhow!("{} {error}", part.equipment))
}

/// Refuse to construct a unit whose fire-control critical lacks a launcher link. Installations
/// that skip the construction rules still need explicit links, so no controller silently
/// guides nothing.
pub(super) fn require_links<S: Copy + Ord>(
    loadout: &super::ResolvedLoadout<CriticalLocation<S>>,
    sections: &std::collections::BTreeMap<S, super::SectionDefinition>,
) -> Result<()> {
    for part in &loadout.systems {
        if !super::is_fire_control(part.system) {
            continue;
        }
        let critical = sections
            .get(&part.location.section)
            .and_then(|section| section.criticals.get(&part.location.slot))
            .context("fire-control critical outside its template section")?;
        super::fire_control_link(&critical.data).map_err(|error| {
            anyhow!(
                "{} in slot {} {error}",
                critical.equipment,
                part.location.slot + 1
            )
        })?;
    }
    Ok(())
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
                Ok((link(part)?, !self.critical_unavailable(location)))
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
                Ok((link(part)?, !self.critical_unavailable(location)))
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
    /// A vehicle's controller serves launchers in its own location (TechManual p. 206).
    pub fn artemis_controllers(&self) -> Result<Vec<ArtemisController<VehicleCriticalLocation>>> {
        let loadout = self.loadout()?;
        controllers(
            FireControl::Artemis,
            &loadout.weapons,
            &loadout.systems,
            |location| {
                let part = &self.definition().sections[&location.section].criticals[&location.slot];
                Ok((link(part)?, !self.critical_unavailable(location)))
            },
            |location| location.slot,
            |controller, primary| controller.section == primary.section,
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
                Ok((link(part)?, !self.critical_unavailable(location)))
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
