//! Resolve ordered damage descriptions against owned construction before any live-state mutation.
use super::{
    AmmunitionBin, BattleDamageRecord, BattleSectionState, SectionDefinition, WeaponMount,
};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::collections::{BTreeMap, BTreeSet};

/// A section/slot identity in the compact damage format, independent of unit anatomy enums.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BattleDamageSlot {
    pub section: u8,
    pub slot: u8,
}

/// Desired material and failure assignments; applying runtime consequences is a separate operation.
/// Omitted records restore protection, ammunition and non-placeholder criticals to construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BattleDamageReplacement {
    pub sections: BTreeMap<u8, BattleSectionState>,
    pub destroyed_criticals: BTreeSet<BattleDamageSlot>,
    pub ammunition: BTreeMap<BattleDamageSlot, u16>,
    /// Nonzero failure codes by system slot or weapon primary slot.
    pub failures: BTreeMap<BattleDamageSlot, u8>,
}

/// Construction facts adapted once so replacement rules do not branch on chassis type.
struct Material<'a> {
    sections: BTreeMap<u8, &'a SectionDefinition>,
    ammunition: BTreeMap<BattleDamageSlot, u16>,
    weapon_heads: BTreeMap<BattleDamageSlot, BattleDamageSlot>,
    groups: Vec<Vec<BattleDamageSlot>>,
}

impl<'a> Material<'a> {
    /// Convert anatomical locations into stable format identities without copying equipment.
    fn new<S: Copy + Ord, L: Copy>(
        sections: &'a BTreeMap<S, SectionDefinition>,
        bins: &[AmmunitionBin<L>],
        weapons: &[WeaponMount<L>],
        number: impl Fn(S) -> u8,
        location: impl Fn(L) -> BattleDamageSlot,
        groups: Vec<Vec<BattleDamageSlot>>,
    ) -> Self {
        Self {
            sections: sections
                .iter()
                .map(|(&section, definition)| (number(section), definition))
                .collect(),
            ammunition: bins
                .iter()
                .map(|bin| (location(bin.location), bin.capacity))
                .collect(),
            weapon_heads: weapons
                .iter()
                .flat_map(|weapon| {
                    let head = weapon.criticals.first().copied().map(&location);
                    let location = &location;
                    weapon
                        .criticals
                        .iter()
                        .filter_map(move |&slot| head.map(|head| (location(slot), head)))
                })
                .collect(),
            groups,
        }
    }

    /// Reject section identities absent from this chassis, including empty placeholder faces.
    fn section(&self, section: u8) -> Result<&SectionDefinition> {
        let definition = self
            .sections
            .get(&section)
            .context("Damage section is not installed")?;
        ensure!(definition.internal > 0, "Damage section has no structure");
        Ok(definition)
    }

    /// Resolve real installed equipment rather than accepting vacant or structural filler slots.
    fn equipment(&self, location: BattleDamageSlot) -> Result<()> {
        let part = self
            .section(location.section)?
            .criticals
            .get(&location.slot)
            .context("Damage critical slot is not installed")?;
        ensure!(
            !super::damage_field::placeholder(&part.equipment),
            "Damage critical is structural filler"
        );
        Ok(())
    }

    /// Last assignments win, then all final material values are checked together.
    fn replace(&self, records: &[BattleDamageRecord]) -> Result<BattleDamageReplacement> {
        let mut armor = BTreeMap::new();
        let mut internal = BTreeMap::new();
        let mut spent = BTreeMap::new();
        let mut failures = BTreeMap::new();
        let mut destroyed = BTreeSet::new();
        for &record in records {
            match record {
                BattleDamageRecord::Armor {
                    section,
                    rear,
                    loss,
                } => {
                    self.section(section)?;
                    armor.insert((section, rear), loss);
                }
                BattleDamageRecord::Internal { section, loss } => {
                    self.section(section)?;
                    internal.insert(section, loss);
                }
                BattleDamageRecord::Critical { section, slot } => {
                    let location = BattleDamageSlot { section, slot };
                    self.equipment(location)?;
                    destroyed.insert(location);
                }
                BattleDamageRecord::Ammunition {
                    section,
                    slot,
                    spent: value,
                } => {
                    let location = BattleDamageSlot { section, slot };
                    self.equipment(location)?;
                    ensure!(
                        self.ammunition.contains_key(&location),
                        "Reload record does not identify an ammunition bin"
                    );
                    spent.insert(location, value);
                }
                BattleDamageRecord::Failure {
                    section,
                    slot,
                    failure,
                } => {
                    let location = BattleDamageSlot { section, slot };
                    self.equipment(location)?;
                    ensure!(
                        !self.ammunition.contains_key(&location),
                        "Failure record identifies ammunition"
                    );
                    let head = self
                        .weapon_heads
                        .get(&location)
                        .copied()
                        .unwrap_or(location);
                    failures.insert(head, failure);
                }
            }
        }
        // These installations already share atomic loss semantics in the combat model.
        for group in &self.groups {
            if group.iter().any(|slot| destroyed.contains(slot)) {
                destroyed.extend(group);
            }
        }
        let sections: BTreeMap<_, _> = self
            .sections
            .iter()
            .map(|(&section, original)| {
                let state = BattleSectionState {
                    armor: remaining(
                        original.armor,
                        armor.get(&(section, false)).copied().unwrap_or(0),
                    )?,
                    rear: remaining(
                        original.rear,
                        armor.get(&(section, true)).copied().unwrap_or(0),
                    )?,
                    internal: remaining(
                        original.internal,
                        internal.get(&section).copied().unwrap_or(0),
                    )?,
                };
                ensure!(
                    state.internal > 0 || (state.armor == 0 && state.rear == 0),
                    "Destroyed section retains armor"
                );
                Ok((section, state))
            })
            .collect::<Result<_>>()?;
        let unavailable = |slot: &BattleDamageSlot| {
            destroyed.contains(slot) || sections[&slot.section].internal == 0
        };
        let ammunition = self
            .ammunition
            .iter()
            .map(|(&slot, &capacity)| {
                let rounds = remaining(capacity, spent.get(&slot).copied().unwrap_or(0))?;
                Ok((slot, if unavailable(&slot) { 0 } else { rounds }))
            })
            .collect::<Result<_>>()?;
        let failures = failures
            .into_iter()
            .map(|(slot, value)| {
                ensure!((0..=7).contains(&value), "Unknown temporary weapon failure");
                Ok((slot, value as u8))
            })
            .collect::<Result<BTreeMap<_, _>>>()?
            .into_iter()
            .filter(|(slot, value)| *value != 0 && !unavailable(slot))
            .collect();
        Ok(BattleDamageReplacement {
            sections,
            destroyed_criticals: destroyed,
            ammunition,
            failures,
        })
    }
}

/// Compute a bounded material remainder without overflowing on a signed loss.
fn remaining(original: u16, loss: i32) -> Result<u16> {
    ensure!(
        (0..=i32::from(original)).contains(&loss),
        "Damage loss exceeds installed material"
    );
    Ok(original - loss as u16)
}

/// Prepare replacement material without changing the world, dice, crew or pending actions.
/// This validates anatomy and material only; runtime failure effects, lifecycle reconciliation
/// and conditional Mech system recalculation still belong to the eventual field transaction.
pub fn prepare_damage_field(
    world: &World,
    id: ObjectId,
    value: &str,
) -> Result<BattleDamageReplacement> {
    let records = super::parse_damage_field(value)?;
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        let loadout = unit.loadout()?;
        let location = |location: super::CriticalLocation| BattleDamageSlot {
            section: super::damage_field::mech_section(location.section),
            slot: location.slot,
        };
        let mut groups = Vec::new();
        if unit.definition().has_double_heat_sinks() {
            groups.extend(
                loadout
                    .heat_sink_groups(unit.definition().heat_sink_slots())?
                    .into_iter()
                    .map(|group| group.into_iter().map(location).collect()),
            );
        }
        if unit.definition().has_special("ImprovedJJ_Tech") {
            groups.extend(
                loadout
                    .jump_jet_groups(true)?
                    .into_iter()
                    .map(|group| group.into_iter().map(location).collect()),
            );
        }
        return Material::new(
            &unit.definition().sections,
            &loadout.ammunition,
            &loadout.weapons,
            super::damage_field::mech_section,
            location,
            groups,
        )
        .replace(&records);
    }
    let unit = world
        .btech
        .vehicles()
        .get(&id)
        .context("Unit construction is unavailable")?;
    let loadout = unit.loadout()?;
    Material::new(
        &unit.definition().sections,
        &loadout.ammunition,
        &loadout.weapons,
        super::damage_field::vehicle_section,
        |location| BattleDamageSlot {
            section: super::damage_field::vehicle_section(location.section),
            slot: location.slot,
        },
        Vec::new(),
    )
    .replace(&records)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::CriticalDefinition;

    /// Format locations still require real equipment; CASE and structural filler are distinct.
    #[test]
    fn replacement_rejects_structural_filler_and_accepts_case() {
        let section = SectionDefinition {
            armor: 5,
            internal: 5,
            criticals: ["EndoSteel", "CASE"]
                .into_iter()
                .enumerate()
                .map(|(slot, equipment)| {
                    (
                        slot as u8,
                        CriticalDefinition {
                            equipment: equipment.into(),
                            data: "-".into(),
                            modes: Vec::new(),
                            brand: None,
                        },
                    )
                })
                .collect(),
            ..Default::default()
        };
        let material = Material {
            sections: [(2, &section)].into_iter().collect(),
            ammunition: BTreeMap::new(),
            weapon_heads: BTreeMap::new(),
            groups: Vec::new(),
        };
        assert!(
            material
                .replace(&[BattleDamageRecord::Critical {
                    section: 2,
                    slot: 0
                }])
                .is_err()
        );
        assert!(
            material
                .replace(&[BattleDamageRecord::Critical {
                    section: 2,
                    slot: 2
                }])
                .is_err()
        );
        let replacement = material
            .replace(&[BattleDamageRecord::Critical {
                section: 2,
                slot: 1,
            }])
            .unwrap();
        assert_eq!(
            replacement.destroyed_criticals,
            [BattleDamageSlot {
                section: 2,
                slot: 1
            }]
            .into_iter()
            .collect()
        );
    }
}
