//! Compact damage inspection projects authoritative material and equipment state without mutating it.
use super::{
    AmmunitionBin, CriticalLocation, DamageRecord, MechSection, SectionDefinition, SectionState,
    System, VehicleCriticalLocation, VehicleSection, WeaponMount,
};
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use std::collections::BTreeMap;

/// Shared borrowed construction and current ammunition for either anatomical location type.
struct Inventory<'a, S, L> {
    definitions: &'a BTreeMap<S, SectionDefinition>,
    sections: &'a BTreeMap<S, SectionState>,
    weapons: &'a [WeaponMount<L>],
    bins: &'a [AmmunitionBin<L>],
    remaining: &'a [u16],
    components: &'a [super::ComponentFailure<L>],
}

/// Structural filler and split links do not represent damage-list equipment records.
pub(super) fn placeholder(name: &str) -> bool {
    name.eq_ignore_ascii_case("SplitCrit_Left")
        || name.eq_ignore_ascii_case("SplitCrit_Right")
        || matches!(
            System::named(name),
            Some(
                System::EndoSteel
                    | System::FerroFibrous
                    | System::HeavyFerroFibrous
                    | System::LightFerroFibrous
                    | System::TripleStrengthMyomer
                    | System::StealthArmor
                    | System::LaserReflective
            )
        )
}

/// Retained numeric identities belong to this text format, independently of Rust enum layout.
pub(super) fn mech_section(section: MechSection) -> u8 {
    match section {
        MechSection::LeftArm => 0,
        MechSection::RightArm => 1,
        MechSection::LeftTorso => 2,
        MechSection::RightTorso => 3,
        MechSection::CenterTorso => 4,
        MechSection::LeftLeg => 5,
        MechSection::RightLeg => 6,
        MechSection::Head => 7,
    }
}

/// Vehicle faces use their own physical identities in the same numeric text format.
pub(super) fn vehicle_section(section: VehicleSection) -> u8 {
    match section {
        VehicleSection::Left => 0,
        VehicleSection::Right => 1,
        VehicleSection::Front => 2,
        VehicleSection::Rear => 3,
        VehicleSection::Turret => 4,
        VehicleSection::Rotor => 5,
    }
}

/// A powered-down weapon or critical failure takes precedence over a manual feed jam.
fn failure(powered_down: bool, critical: u8, feed_jammed: bool) -> u8 {
    if powered_down {
        5
    } else if critical != 0 {
        critical
    } else if feed_jammed {
        6
    } else {
        0
    }
}

/// Format armor, internal structure, then per-slot losses in deterministic anatomical order.
fn render<S: Copy + Ord, L: Copy + PartialEq>(
    inventory: Inventory<'_, S, L>,
    number: impl Fn(S) -> u8,
    location: impl Fn(S, u8) -> L,
    destroyed: impl Fn(L) -> bool,
    failure: impl Fn(usize) -> u8,
) -> String {
    let mut result = Vec::new();
    for (&section, original) in inventory.definitions {
        if original.internal == 0 {
            continue;
        }
        let current = &inventory.sections[&section];
        for (rear, before, after) in [
            (false, original.armor, current.armor),
            (true, original.rear, current.rear),
        ] {
            if before != after {
                result.push(
                    DamageRecord::Armor {
                        section: number(section),
                        rear,
                        loss: i32::from(before) - i32::from(after),
                    }
                    .to_string(),
                );
            }
        }
    }
    for (&section, original) in inventory.definitions {
        let current = &inventory.sections[&section];
        if original.internal != 0 && original.internal != current.internal {
            result.push(
                DamageRecord::Internal {
                    section: number(section),
                    loss: i32::from(original.internal) - i32::from(current.internal),
                }
                .to_string(),
            );
        }
    }
    for (&section, original) in inventory.definitions {
        for (&slot, part) in &original.criticals {
            if placeholder(&part.equipment) {
                continue;
            }
            let location = location(section, slot);
            let section_number = number(section);
            if destroyed(location) {
                result.push(
                    DamageRecord::Critical {
                        section: section_number,
                        slot,
                    }
                    .to_string(),
                );
                continue;
            }
            if let Some((index, bin)) = inventory
                .bins
                .iter()
                .enumerate()
                .find(|(_, bin)| bin.location == location)
            {
                let spent = i32::from(bin.capacity) - i32::from(inventory.remaining[index]);
                if spent != 0 {
                    result.push(
                        DamageRecord::Ammunition {
                            section: section_number,
                            slot,
                            spent,
                        }
                        .to_string(),
                    );
                }
            } else if let Some((index, _)) = inventory
                .weapons
                .iter()
                .enumerate()
                .find(|(_, mount)| mount.criticals.first() == Some(&location))
            {
                let state = failure(index);
                if state != 0 {
                    result.push(
                        DamageRecord::Failure {
                            section: section_number,
                            slot,
                            failure: i32::from(state),
                        }
                        .to_string(),
                    );
                }
            } else if let Some(failure) =
                super::component_failure::at(inventory.components, location)
            {
                result.push(
                    DamageRecord::Failure {
                        section: section_number,
                        slot,
                        failure: i32::from(failure.code()),
                    }
                    .to_string(),
                );
            }
        }
    }
    result.join(",")
}

/// Read the compact damage field; empty text means no represented losses or failures.
/// This is a damage report, not a full unit-state snapshot or a repair operation.
pub fn unit_damage_field(world: &World, id: ObjectId) -> Result<String> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        let loadout = unit.loadout()?;
        return Ok(render(
            Inventory {
                definitions: &unit.definition().sections,
                sections: unit.sections(),
                weapons: &loadout.weapons,
                bins: &loadout.ammunition,
                remaining: unit.ammunition(),
                components: &unit.component_failures,
            },
            vehicle_section,
            |section, slot| VehicleCriticalLocation { section, slot },
            |location| unit.critical_destroyed(location),
            |index| {
                let critical = unit
                    .weapon_failures()
                    .get(&index)
                    .map_or(0, |state| state.code());
                failure(
                    unit.powered_down_weapons.contains(&index),
                    critical,
                    unit.jammed_weapons.contains(&index),
                )
            },
        ));
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction is unavailable")?;
    let loadout = unit.loadout()?;
    Ok(render(
        Inventory {
            definitions: &unit.definition().sections,
            sections: unit.sections(),
            weapons: &loadout.weapons,
            bins: &loadout.ammunition,
            remaining: unit.ammunition(),
            components: &unit.component_failures,
        },
        mech_section,
        |section, slot| CriticalLocation { section, slot },
        |location| unit.critical_destroyed(location),
        |index| {
            failure(
                unit.powered_down_weapons.contains(&index),
                if let Some(failure) = unit.weapon_failures.get(&index) {
                    failure.code()
                } else if unit.weapon_damage_jams.contains(&index) {
                    7
                } else {
                    0
                },
                unit.jammed_weapons.contains(&index),
            )
        },
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::CriticalDefinition;

    /// Section destruction reports actual equipment, including CASE, but omits structural filler.
    #[test]
    fn destroyed_section_omits_fillers_and_retains_case() {
        let definitions = BTreeMap::from([(
            MechSection::LeftTorso,
            SectionDefinition {
                armor: 8,
                rear: 4,
                internal: 8,
                criticals: [
                    "CASE",
                    "EndoSteel",
                    "FerroFibrous",
                    "HvyFerroFibrous",
                    "LtFerroFibrous",
                    "TripleStrengthMyomer",
                    "StealthArmor",
                    "SplitCrit_Left",
                    "SplitCrit_Right",
                    "HeatSink",
                ]
                .into_iter()
                .enumerate()
                .map(|(slot, equipment)| {
                    (
                        slot as u8,
                        CriticalDefinition {
                            equipment: equipment.into(),
                            data: "-".into(),
                            modes: vec![],
                        },
                    )
                })
                .collect(),
                ..Default::default()
            },
        )]);
        let sections = BTreeMap::from([(
            MechSection::LeftTorso,
            SectionState {
                armor: 0,
                rear: 0,
                internal: 0,
            },
        )]);
        assert_eq!(
            render(
                Inventory {
                    definitions: &definitions,
                    sections: &sections,
                    weapons: &[],
                    bins: &[],
                    remaining: &[],
                    components: &[],
                },
                mech_section,
                |section, slot| CriticalLocation { section, slot },
                |_| true,
                |_| 0,
            ),
            "A:2/8,A(R):2/4,I:2/8,C:2/0,C:2/9"
        );
    }
}
