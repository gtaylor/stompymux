//! Read-only slot inspection uses shared equipment rendering with chassis-specific state adapters.
use super::*;
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};

/// One physical equipment slot. Indices are zero-based; cockpit labels add one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleCriticalInspection {
    pub slot: u8,
    pub equipment: String,
    pub condition: BattleEquipmentCondition,
    pub weapon_index: Option<usize>,
    pub ammunition_index: Option<usize>,
    pub ammunition_remaining: Option<u16>,
    pub ammunition_capacity: Option<u16>,
    pub rear_mount: bool,
    pub one_shot: bool,
    pub spent: bool,
    /// Authored Artemis control-slot label, already one-based for this display.
    pub controls_slot: Option<u8>,
}

/// Complete physical slot layout, including empty slots and destroyed sections.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleCriticalReport {
    pub section: BattleUnitSection,
    pub name: String,
    pub slots: Vec<BattleCriticalInspection>,
}

/// Material condition supplied by each chassis without repeating equipment interpretation.
#[derive(Default)]
struct SlotDamage {
    destroyed: bool,
    disabled: bool,
    damaged: bool,
    failure: Option<BattleEquipmentFailure>,
}

/// Borrowed equipment shared by Mech and vehicle slot adapters.
struct Inventory<'a, L> {
    definitions: &'a BTreeMap<u8, CriticalDefinition>,
    weapons: &'a [WeaponMount<L>],
    ammunition: &'a [AmmunitionBin<L>],
    remaining: &'a [u16],
    spent: &'a BTreeSet<usize>,
    powered_down: &'a BTreeSet<usize>,
    section_destroyed: bool,
    failures: &'a BTreeMap<usize, BattleEquipmentFailure>,
}

/// Names whose spelling depends on construction rather than critical-slot data.
struct ConstructionNames {
    leg: bool,
    double_sinks: bool,
    engine: BattleEngine,
    improved_jets: bool,
    small_cockpit: bool,
}

/// Inspect a section by its cockpit alias, without changing authority, dice or saved equipment.
pub fn critical_report(world: &World, id: ObjectId, section: &str) -> Result<BattleCriticalReport> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        let section = BattleVehicleSection::parse_location(section)
            .map_err(|_| anyhow::anyhow!("Invalid section!"))?;
        let layout = unit
            .definition()
            .sections
            .get(&section)
            .filter(|layout| layout.internal > 0)
            .context("Invalid section!")?;
        let loadout = unit.loadout()?;
        let engine = BattleEngine::display_from_flags(
            unit.definition().has_special("LightEngine_Tech"),
            unit.definition().has_special("CompactEngine_Tech"),
            unit.definition().has_special("XXL_Tech"),
            unit.definition().has_special("XLEngine_Tech"),
        );
        let names = ConstructionNames {
            leg: false,
            double_sinks: unit.definition().has_special("Clan")
                || unit.definition().has_special("DoubleHS"),
            engine,
            improved_jets: false,
            small_cockpit: false,
        };
        let inventory = Inventory {
            definitions: &layout.criticals,
            weapons: &loadout.weapons,
            ammunition: &loadout.ammunition,
            remaining: unit.ammunition(),
            spent: &unit.spent_launchers,
            powered_down: &unit.powered_down_weapons,
            section_destroyed: unit.sections()[&section].internal == 0,
            failures: &unit.weapon_failures,
        };
        return Ok(BattleCriticalReport {
            section: BattleUnitSection::Vehicle(section),
            name: section.name().replace('_', " "),
            slots: inspect(
                &inventory,
                &names,
                12,
                |slot| VehicleCriticalLocation { section, slot },
                |location| SlotDamage {
                    destroyed: unit.critical_destroyed(location),
                    disabled: unit.critical_unavailable(location),
                    failure: super::component_failure::at(&unit.component_failures, location),
                    ..Default::default()
                },
            ),
        });
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?;
    let section = unit
        .chassis()
        .parse_location(section)
        .map_err(|_| anyhow::anyhow!("Invalid section!"))?;
    let layout = unit
        .definition()
        .sections
        .get(&section)
        .filter(|layout| layout.internal > 0)
        .context("Invalid section!")?;
    let loadout = unit.loadout()?;
    let names = ConstructionNames {
        leg: unit.chassis().is_leg(section),
        double_sinks: unit.definition().has_double_heat_sinks(),
        engine: BattleEngine::display_family(&loadout, unit.definition().clan_engine())?,
        improved_jets: unit.definition().has_special("ImprovedJJ_Tech"),
        small_cockpit: unit
            .definition()
            .has_technology(super::BattleTechnology::SmallCockpit),
    };
    let inventory = Inventory {
        definitions: &layout.criticals,
        weapons: &loadout.weapons,
        ammunition: &loadout.ammunition,
        remaining: unit.ammunition(),
        spent: &unit.spent_launchers,
        powered_down: &unit.powered_down_weapons,
        section_destroyed: unit.sections()[&section].internal == 0,
        failures: &unit.weapon_failures,
    };
    Ok(BattleCriticalReport {
        section: BattleUnitSection::Mech(section),
        name: unit.chassis().section_name(section).replace('_', " "),
        slots: inspect(
            &inventory,
            &names,
            unit.chassis().critical_slots(section),
            |slot| CriticalLocation { section, slot },
            |location| SlotDamage {
                destroyed: unit.critical_destroyed(location),
                disabled: unit.critical_unavailable(location),
                failure: super::component_failure::at(&unit.component_failures, location),
                damaged: unit
                    .weapon_damage()
                    .iter()
                    .any(|damage| damage.location == location),
            },
        ),
    })
}

/// Resolve installed parts once, preserving physical slot identities on split mounts.
fn inspect<L: Copy + PartialEq>(
    inventory: &Inventory<'_, L>,
    names: &ConstructionNames,
    count: u8,
    location: impl Fn(u8) -> L,
    damage: impl Fn(L) -> SlotDamage,
) -> Vec<BattleCriticalInspection> {
    (0..count)
        .map(|slot| {
            let mut row = BattleCriticalInspection {
                slot,
                equipment: "Empty".into(),
                condition: BattleEquipmentCondition::Empty,
                weapon_index: None,
                ammunition_index: None,
                ammunition_remaining: None,
                ammunition_capacity: None,
                rear_mount: false,
                one_shot: false,
                spent: false,
                controls_slot: None,
            };
            let Some(definition) = inventory.definitions.get(&slot) else {
                return row;
            };
            let location = location(slot);
            let facts = damage(location);
            let weapon = inventory
                .weapons
                .iter()
                .enumerate()
                .find(|(_, mount)| mount.criticals.contains(&location));
            let bin = inventory
                .ammunition
                .iter()
                .enumerate()
                .find(|(_, bin)| bin.location == location);
            let system = BattleSystem::named(&definition.equipment);
            let proxy = definition.equipment.eq_ignore_ascii_case("SplitCrit_Left")
                || definition.equipment.eq_ignore_ascii_case("SplitCrit_Right");
            let placeholder = proxy
                || system.is_some_and(|system| {
                    system.is_noncritical()
                        && !matches!(system, BattleSystem::Case | BattleSystem::CaseIi)
                });
            let broken = weapon.is_some_and(|(_, mount)| {
                mount
                    .criticals
                    .iter()
                    .any(|location| damage(*location).destroyed)
            });
            row.condition =
                if (facts.destroyed || broken) && (!placeholder || inventory.section_destroyed) {
                    if facts.destroyed {
                        BattleEquipmentCondition::Destroyed
                    } else {
                        BattleEquipmentCondition::Broken
                    }
                } else if (facts.disabled
                    || weapon.is_some_and(|(index, _)| inventory.powered_down.contains(&index)))
                    && !facts.destroyed
                {
                    BattleEquipmentCondition::Disabled
                } else if facts.damaged {
                    BattleEquipmentCondition::Damaged
                } else if let Some(failure) = facts.failure.or_else(|| {
                    weapon.and_then(|(index, _)| inventory.failures.get(&index).copied())
                }) {
                    failure.condition()
                } else {
                    BattleEquipmentCondition::Operational
                };
            if let Some((index, mount)) = weapon {
                row.weapon_index = Some(index);
                row.equipment = super::equipment_display::weapon_name(mount.weapon);
                // Proxy slots carry their own flags, while their part identity comes from the parent.
                row.one_shot = !proxy && definition.modes.iter().any(|mode| mode == "OneShot");
                row.spent = !proxy
                    && (row.one_shot || mount.weapon.is_rocket())
                    && inventory.spent.contains(&index);
                row.rear_mount = !proxy && definition.modes.iter().any(|mode| mode == "RearMount");
                return row;
            }
            if let Some((index, bin)) = bin {
                row.ammunition_index = Some(index);
                row.equipment = format!(
                    "{}{} Ammo",
                    bin.weapon
                        .name()
                        .split_once('.')
                        .map_or(bin.weapon.name(), |(_, name)| name),
                    super::equipment_display::ammunition_description(bin.weapon, bin.mode)
                );
                row.ammunition_capacity = Some(bin.capacity);
                row.ammunition_remaining = Some(inventory.remaining[index]);
                return row;
            }
            row.equipment = system.map_or_else(
                || definition.equipment.replace(['_', '.'], " "),
                |system| names.system_name(system, &definition.equipment),
            );
            if system == Some(BattleSystem::ArtemisIv)
                && row.condition == BattleEquipmentCondition::Operational
            {
                row.controls_slot = definition.data.parse().ok().filter(|slot| *slot > 0);
            }
            row
        })
        .collect()
}

impl ConstructionNames {
    /// Render construction-dependent names without persisting a second equipment catalogue.
    fn system_name(&self, system: BattleSystem, authored: &str) -> String {
        use BattleSystem::*;
        match system {
            ShoulderOrHip => if self.leg { "Hip" } else { "Shoulder" }.into(),
            HandOrFootActuator => if self.leg {
                "Foot Actuator"
            } else {
                "Hand Actuator"
            }
            .into(),
            UpperActuator => "Upper Actuator".into(),
            LowerActuator => "Lower Actuator".into(),
            HeatSink => if self.double_sinks {
                "Double Heatsink"
            } else {
                "Heatsink"
            }
            .into(),
            Engine => match self.engine {
                BattleEngine::Standard => "Engine",
                BattleEngine::Light => "Engine (Light)",
                BattleEngine::Xl => "Engine (XL)",
                BattleEngine::Xxl => "Engine (XXL)",
                BattleEngine::Compact => "Engine (Compact)",
            }
            .into(),
            JumpJet => if self.improved_jets {
                "JumpJet (Improved)"
            } else {
                "Jumpjet"
            }
            .into(),
            Cockpit => if self.small_cockpit {
                "Small Cockpit"
            } else {
                "Cockpit"
            }
            .into(),
            LifeSupport => "Life Support".into(),
            TripleStrengthMyomer => "Triple Strength Myomer".into(),
            _ => authored.replace(['_', '.'], " "),
        }
    }
}

impl BattleCriticalInspection {
    /// One numbered cockpit entry; unavailable bins suppress inventory quantities.
    fn text(&self) -> String {
        let mut text = format!(
            "{:2} {}{}",
            self.slot + 1,
            if self.one_shot { "OS " } else { "" },
            self.equipment
        );
        if self.spent {
            text.push_str(" (Empty)");
        }
        if self.rear_mount {
            text.push_str(" (R)");
        }
        if matches!(
            self.condition,
            BattleEquipmentCondition::Operational | BattleEquipmentCondition::Damaged
        ) {
            if let (Some(remaining), Some(capacity)) =
                (self.ammunition_remaining, self.ammunition_capacity)
            {
                text.push_str(&format!(" [{remaining:03}/{capacity:03}]"));
            }
            if let Some(slot) = self.controls_slot {
                text.push_str(&format!(" [Controls Slot {slot}]"));
            }
        }
        if !matches!(
            self.condition,
            BattleEquipmentCondition::Empty | BattleEquipmentCondition::Operational
        ) {
            text.push_str(&format!(" ({})", self.condition.label()));
        }
        text
    }
}

/// Render paired columns in physical slot order, without omitting uninstalled positions.
pub fn critical_status(world: &World, id: ObjectId, section: &str) -> Result<String> {
    let report = critical_report(world, id, section)?;
    let rows: Vec<_> = report
        .slots
        .iter()
        .map(BattleCriticalInspection::text)
        .collect();
    let half = rows.len() / 2;
    let width = rows[..half].iter().map(String::len).max().unwrap_or(0);
    let mut lines = vec![format!("{} Criticals", report.name)];
    for slot in 0..half {
        lines.push(format!("{:<width$} | {}", rows[slot], rows[slot + half]));
    }
    Ok(lines.join("\r\n"))
}

/// Native slot inspection checks cockpit authority before parsing the requested section.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    super::weapon_reports::report((|| {
        let id = super::weapon_reports::cockpit(ctx, true, true)?;
        let world = ctx.scripts.world.borrow();
        let section = input
            .args
            .split_whitespace()
            .next()
            .context("You must specify a section to list the criticals for!")?;
        critical_status(&world, id, section)
    })())
}
