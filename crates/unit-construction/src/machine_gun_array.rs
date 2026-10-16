//! Machine gun arrays: a one-slot module that links two to four machine guns of one size
//! class in its location so they fire as one attack (TechManual p. 228; Total Warfare p. 137).
use super::{CriticalLocation, ResolvedLoadout, System, Weapon};
use serde::Serialize;
use std::collections::BTreeMap;

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
    /// Indices of the linked machine guns in the loadout's weapons, nearest first.
    pub weapon_indices: Vec<usize>,
}

impl<S: Copy + Ord> ResolvedLoadout<CriticalLocation<S>> {
    /// Each array with the guns it links: the run of up to four machine guns of one size
    /// class in the slots directly above it in its location, or, when none sit above it, in
    /// the slots directly below. A vehicle location's item list is read the same way, so its
    /// array sits beside the guns it links. Construction checks that each links two to four
    /// guns.
    pub fn machine_gun_arrays(&self) -> Vec<MachineGunArray<CriticalLocation<S>>> {
        let guns: BTreeMap<CriticalLocation<S>, (usize, super::MachineGunClass)> = self
            .weapons
            .iter()
            .enumerate()
            .filter_map(|(index, mount)| {
                let class = mount.weapon.machine_gun_class()?;
                Some((mount.criticals[0], (index, class)))
            })
            .collect();
        let run = |location: CriticalLocation<S>, step: i16| {
            let mut indices = Vec::new();
            let mut class = None;
            let mut slot = i16::from(location.slot) + step;
            while (0..12).contains(&slot) && indices.len() < *ARRAY_GUNS.end() {
                let at = CriticalLocation {
                    section: location.section,
                    slot: slot as u8,
                };
                let Some(&(index, gun)) = guns.get(&at) else {
                    break;
                };
                if class.is_some_and(|class| class != gun) {
                    break;
                }
                class = Some(gun);
                indices.push(index);
                slot += step;
            }
            indices
        };
        self.systems
            .iter()
            .filter(|part| part.system == System::MachineGunArray)
            .map(|part| {
                let above = run(part.location, -1);
                MachineGunArray {
                    location: part.location,
                    weapon_indices: if above.is_empty() {
                        run(part.location, 1)
                    } else {
                        above
                    },
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::{MechLoadout, MechTemplate};

    /// A Jenner whose right torso holds `slots`.
    fn loadout(slots: &str) -> MechLoadout {
        let jenner = include_str!("../tests/fixtures/JR7-D.toml").replace(
            "slots = [\n    { at = 1, item = \"Ammo_IS.SRM-4\", rounds = 25 },\n    { at = \"2-3\", item = \"JumpJet\" },\n]",
            &format!("slots = [\n{slots}]"),
        );
        MechLoadout::resolve(&MechTemplate::parse("JR7-D", &jenner).unwrap()).unwrap()
    }

    #[test]
    fn arrays_link_the_adjacent_guns_of_one_class() {
        let above = loadout(
            "    { at = \"1-3\", item = \"IS.MachineGun\" },\n    { at = 4, item = \"IS.MGA\" },\n",
        );
        let arrays = above.machine_gun_arrays();
        assert_eq!(arrays.len(), 1);
        assert_eq!(arrays[0].weapon_indices.len(), 3);
        let below = loadout(
            "    { at = 1, item = \"CL.MGA\" },\n    { at = \"2-3\", item = \"IS.LightMachineGun\" },\n    { at = 4, item = \"IS.MachineGun\" },\n",
        );
        let arrays = below.machine_gun_arrays();
        // The standard gun after the light ones is another class.
        assert_eq!(arrays[0].weapon_indices.len(), 2);
        let five = loadout(
            "    { at = \"1-5\", item = \"IS.HeavyMachineGun\" },\n    { at = 6, item = \"IS.MGA\" },\n",
        );
        assert_eq!(five.machine_gun_arrays()[0].weapon_indices.len(), 4);
    }
}
