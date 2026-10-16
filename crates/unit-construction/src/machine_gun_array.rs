//! Machine gun arrays: a one-slot module that links two to four machine guns of one size
//! class in its location so they fire as one attack (TechManual p. 228; Total Warfare p. 137).
//! A template names the guns an array links with `links`, the one-based first slots of the
//! guns in the array's location, and construction checks them.
use super::{CriticalLocation, ResolvedLoadout, System, Weapon};
use anyhow::{Result, anyhow, bail, ensure};
use serde::Serialize;
use std::collections::BTreeSet;

/// Machine gun size classes; an array links guns of one class only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MachineGunClass {
    Light,
    Standard,
    Heavy,
}

impl Weapon {
    /// The machine gun size class of this weapon, or `None` for other weapons.
    pub fn machine_gun_class(self) -> Option<MachineGunClass> {
        Some(match self {
            Self::LightMachineGun | Self::ClanLightMachineGun => MachineGunClass::Light,
            Self::MachineGun | Self::ClanMachineGun => MachineGunClass::Standard,
            Self::HeavyMachineGun | Self::ClanHeavyMachineGun => MachineGunClass::Heavy,
            _ => return None,
        })
    }
}

/// Fewest and most machine guns an array links.
pub const ARRAY_GUNS: std::ops::RangeInclusive<usize> = 2..=4;

/// One installed machine gun array and the machine guns it links, at a BattleMech location
/// unless another location type is given, as vehicles do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MachineGunArray<L = CriticalLocation> {
    pub location: L,
    /// Indices of the linked machine guns in the loadout's weapons, in the order the template
    /// lists their slots.
    pub weapon_indices: Vec<usize>,
}

/// The one-based first slots of the machine guns a machine gun array critical's stored data
/// links, in the array's own location: a comma-separated list, as the template's `links`
/// array stores it. A critical with no stored data links nothing, which construction refuses.
pub fn machine_gun_array_links(data: &str) -> Result<Vec<u8>> {
    if data == "-" {
        return Ok(Vec::new());
    }
    data.split(',')
        .map(|slot| {
            let slot: u8 = slot
                .trim()
                .parse()
                .map_err(|_| anyhow!("has an invalid machine gun link {data:?}"))?;
            ensure!(slot > 0, "has an invalid machine gun link {data:?}");
            Ok(slot)
        })
        .collect()
}

/// Store machine gun array links in a critical's data, the form
/// [`machine_gun_array_links`] reads.
pub fn store_machine_gun_array_links(links: &[u8]) -> String {
    if links.is_empty() {
        return "-".into();
    }
    links
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(",")
}

impl<S: Copy + Ord> ResolvedLoadout<CriticalLocation<S>> {
    /// Each array with the guns its template `links` name: the machine guns whose first slot
    /// is a linked slot in the array's location. Construction refuses an array whose links do
    /// not name two to four machine guns of one size class there, so every constructed unit's
    /// arrays link exactly the guns their templates list.
    pub fn machine_gun_arrays(&self) -> Vec<MachineGunArray<CriticalLocation<S>>> {
        self.systems
            .iter()
            .filter(|part| part.system == System::MachineGunArray)
            .map(|part| MachineGunArray {
                location: part.location,
                weapon_indices: part
                    .links
                    .iter()
                    .filter_map(|&link| self.linked_gun(part.location, link))
                    .collect(),
            })
            .collect()
    }

    /// The machine gun whose first critical is one-based slot `link` in `array`'s location.
    fn linked_gun(&self, array: CriticalLocation<S>, link: u8) -> Option<usize> {
        let target = CriticalLocation {
            section: array.section,
            slot: link.checked_sub(1)?,
        };
        self.weapons.iter().position(|mount| {
            mount.criticals[0] == target && mount.weapon.machine_gun_class().is_some()
        })
    }
}

/// Check every machine gun array's links: each lists the first slots of two to four machine
/// guns of one size class in the array's own location, and a gun belongs to one array at most
/// (TechManual p. 228).
pub(crate) fn check_machine_gun_arrays<S: Copy + Ord>(
    loadout: &ResolvedLoadout<CriticalLocation<S>>,
    section_name: impl Fn(S) -> &'static str,
) -> Result<()> {
    let mut linked = BTreeSet::new();
    for part in loadout
        .systems
        .iter()
        .filter(|part| part.system == System::MachineGunArray)
    {
        let name = section_name(part.location.section);
        let here = format!(
            "The machine gun array in {name} slot {}",
            part.location.slot + 1
        );
        ensure!(
            ARRAY_GUNS.contains(&part.links.len()),
            "{here} links {} slots; its `links` must list the first slots of two to four machine guns of one size class in {name} (TechManual p. 228)",
            part.links.len()
        );
        let mut class = None;
        let mut slots = BTreeSet::new();
        for &link in &part.links {
            ensure!(slots.insert(link), "{here} links slot {link} twice");
            let Some(index) = loadout.linked_gun(part.location, link) else {
                bail!(
                    "{here} links slot {link}, which is not the first slot of a machine gun in {name} (TechManual p. 228)"
                );
            };
            let gun = loadout.weapons[index].weapon.machine_gun_class();
            ensure!(
                class.is_none_or(|class| Some(class) == gun),
                "{here} links machine guns of more than one size class (TechManual p. 228)"
            );
            class = gun;
            ensure!(
                linked.insert(index),
                "The machine gun in {name} slot {link} is linked to two machine gun arrays (TechManual p. 228)"
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MechLoadout, MechSection, MechTemplate};

    /// A Jenner whose right torso holds `slots`.
    fn loadout(slots: &str) -> MechLoadout {
        let jenner = include_str!("../tests/fixtures/JR7-D.toml").replace(
            "slots = [\n    { at = 1, item = \"Ammo_IS.SRM-4\", rounds = 25 },\n    { at = \"2-3\", item = \"JumpJet\" },\n]",
            &format!("slots = [\n{slots}]"),
        );
        MechLoadout::resolve(&MechTemplate::parse("JR7-D", &jenner).unwrap()).unwrap()
    }

    /// Stored links parse to positive slots; anything else is refused.
    #[test]
    fn links_round_trip_through_critical_data() {
        assert_eq!(machine_gun_array_links("-").unwrap(), Vec::<u8>::new());
        assert_eq!(machine_gun_array_links("1,3,4").unwrap(), [1, 3, 4]);
        assert_eq!(store_machine_gun_array_links(&[1, 3, 4]), "1,3,4");
        assert_eq!(store_machine_gun_array_links(&[]), "-");
        for data in ["0", "1,,2", "x", "1,256"] {
            assert!(machine_gun_array_links(data).is_err(), "{data}");
        }
    }

    /// An array links exactly the guns its links name, wherever they sit in the location.
    #[test]
    fn arrays_link_the_listed_guns() {
        let spread = loadout(
            "    { at = 1, item = \"IS.MachineGun\" },\n    { at = 4, item = \"IS.MGA\", links = [1, 9] },\n    { at = 9, item = \"IS.MachineGun\" },\n    { at = 10, item = \"IS.MachineGun\" },\n",
        );
        let arrays = spread.machine_gun_arrays();
        assert_eq!(arrays.len(), 1);
        let slots: Vec<_> = arrays[0]
            .weapon_indices
            .iter()
            .map(|&index| spread.weapons[index].criticals[0])
            .collect();
        assert_eq!(
            slots,
            [0, 8].map(|slot| CriticalLocation {
                section: MechSection::RightTorso,
                slot
            })
        );
        assert!(check_machine_gun_arrays(&spread, |_| "Right Torso").is_ok());
    }

    /// Construction refuses an array whose links are missing, too few or too many, name
    /// anything but a machine gun's first slot, mix size classes or share a gun.
    #[test]
    fn construction_refuses_bad_links() {
        let refused = |slots: &str, expected: &str| {
            let error = check_machine_gun_arrays(&loadout(slots), |_| "Right Torso")
                .unwrap_err()
                .to_string();
            assert!(error.contains(expected), "{error}");
        };
        let guns = "    { at = \"1-3\", item = \"IS.MachineGun\" },\n    { at = 5, item = \"IS.HeavyMachineGun\" },\n";
        refused(
            &format!("{guns}    {{ at = 4, item = \"IS.MGA\" }},\n"),
            "links 0 slots",
        );
        refused(
            &format!("{guns}    {{ at = 4, item = \"IS.MGA\", links = [1] }},\n"),
            "links 1 slots",
        );
        refused(
            &format!("{guns}    {{ at = 4, item = \"IS.MGA\", links = [1, 2, 3, 5, 1] }},\n"),
            "links 5 slots",
        );
        refused(
            &format!("{guns}    {{ at = 4, item = \"IS.MGA\", links = [1, 1] }},\n"),
            "links slot 1 twice",
        );
        refused(
            &format!("{guns}    {{ at = 4, item = \"IS.MGA\", links = [1, 6] }},\n"),
            "not the first slot of a machine gun",
        );
        refused(
            &format!("{guns}    {{ at = 4, item = \"IS.MGA\", links = [1, 5] }},\n"),
            "more than one size class",
        );
        refused(
            &format!(
                "{guns}    {{ at = 4, item = \"IS.MGA\", links = [1, 2] }},\n    {{ at = 6, item = \"IS.MGA\", links = [2, 3] }},\n"
            ),
            "linked to two machine gun arrays",
        );
    }
}
