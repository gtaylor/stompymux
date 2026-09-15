//! Resolve template critical slots into distinct weapons, ammunition bins and systems.
use super::{BattleSection, BattleSystem, BattleTemplate, BattleWeapon};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A zero-based slot in a named BattleMech section.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CriticalLocation {
    pub section: BattleSection,
    pub slot: u8,
}

/// One installed weapon, even when it occupies several critical slots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WeaponMount<L = CriticalLocation> {
    pub weapon: BattleWeapon,
    /// Primary-section slots come first, followed by any linked extension slots.
    pub criticals: Vec<L>,
    pub rear_mount: bool,
    /// Explicit authored computer link, independent of automatic equipment eligibility.
    pub on_targeting_computer: bool,
    /// The mount carries one self-contained salvo instead of drawing from bins.
    pub one_shot: bool,
    /// Templates may explicitly represent an already expended launcher.
    pub initially_spent: bool,
    pub initial_fire_mode: super::BattleFireMode,
    pub initial_ammunition_mode: super::BattleAmmunitionMode,
    pub brand: Option<u8>,
}

/// One independent ammunition bin; rounds count complete weapon salvos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AmmunitionBin<L = CriticalLocation> {
    pub location: L,
    pub weapon: BattleWeapon,
    pub rounds: u16,
    /// Maximum salvos in this installed bin, independent of its initial contents.
    pub capacity: u16,
    pub half_ton: bool,
    /// Retained bin fire flag; launcher hotloading is selected on the weapon, not its supply.
    pub hotload: bool,
    pub mode: super::BattleAmmunitionMode,
    pub brand: Option<u8>,
}

impl AmmunitionBin {
    /// Validate supported bin flags and derive capacity without inspecting live contents.
    pub(super) fn configuration(
        weapon: BattleWeapon,
        flags: &[String],
    ) -> Result<(u16, bool, super::BattleAmmunitionMode)> {
        let full_capacity = weapon.profile().ammunition_per_ton;
        ensure!(full_capacity > 0, "Weapon does not use ammunition");
        ensure!(
            flags.iter().collect::<BTreeSet<_>>().len() == flags.len(),
            "Duplicate ammunition flags"
        );
        let half_ton = flags.iter().any(|mode| mode == "Halfton");
        let ammunition_flags: Vec<_> = flags
            .iter()
            .filter(|mode| !matches!(mode.as_str(), "Halfton" | "Hotload" | "OnTC"))
            .cloned()
            .collect();
        let mode = super::BattleAmmunitionMode::from_flags(weapon, &ammunition_flags)?;
        let full_capacity = weapon.profile_for_ammunition(mode).ammunition_per_ton;
        let capacity = u16::from(
            if half_ton
                || matches!(
                    mode,
                    super::BattleAmmunitionMode::Precision
                        | super::BattleAmmunitionMode::ArmorPiercing
                )
            {
                full_capacity / 2
            } else if mode == super::BattleAmmunitionMode::Caseless {
                full_capacity * 2
            } else {
                full_capacity
            },
        );
        Ok((capacity, half_ton, mode))
    }
}

/// One system critical, retaining optional manufacturer information.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SystemCritical<L = CriticalLocation> {
    pub location: L,
    pub system: BattleSystem,
    pub brand: Option<u8>,
}

impl<L> WeaponMount<L> {
    /// Explicit links survive without computer slots; automatic links require installed hardware.
    /// Any unavailable computer slot disables assistance, and cluster ammunition never benefits.
    pub(super) fn computer_assists(
        &self,
        ammunition: super::BattleAmmunitionMode,
        available_slots: impl Iterator<Item = bool>,
    ) -> bool {
        if ammunition == super::BattleAmmunitionMode::Cluster {
            return false;
        }
        let mut installed = false;
        for available in available_slots {
            installed = true;
            if !available {
                return false;
            }
        }
        self.on_targeting_computer || (installed && self.weapon.supports_targeting_computer())
    }

    /// Validate shared weapon flags independently of chassis-specific slot allocation.
    pub(super) fn from_critical(
        weapon: BattleWeapon,
        critical: &super::CriticalDefinition,
        criticals: Vec<L>,
    ) -> Result<Self> {
        ensure!(
            !weapon.is_artillery()
                || critical
                    .modes
                    .iter()
                    .filter(|flag| matches!(flag.as_str(), "Cluster" | "Smoke" | "Mine"))
                    .count()
                    <= 1,
            "Conflicting artillery ammunition flags"
        );
        ensure!(
            critical.modes.iter().collect::<BTreeSet<_>>().len() == critical.modes.len()
                && critical.modes.iter().all(|mode| mode == "RearMount"
                    || (mode == "Heat" && weapon.supports_heat_mode())
                    || (mode == "Hotload" && weapon.supports_hotload())
                    || (mode == "UltraMode" && weapon.is_ultra())
                    || (mode == "RapidFire" && weapon.supports_rapid_fire())
                    || super::BattleAmmunitionMode::from_flag(weapon, mode)
                        .is_some_and(|mode| mode.supports(weapon))
                    || (mode == "Gattling" && weapon.supports_gatling())
                    || (matches!(
                        mode.as_str(),
                        "Rotary_TwoShot" | "Rotary_FourShot" | "Rotary_SixShot"
                    ) && weapon.is_rotary())
                    || mode == "OnTC"
                    || (matches!(mode.as_str(), "OneShot" | "OneShot_Used")
                        && weapon.profile().missiles > 0)),
            "Unsupported weapon modes {:?}",
            critical.modes
        );
        let one_shot = critical.modes.iter().any(|mode| mode == "OneShot");
        let initially_spent = critical.modes.iter().any(|mode| mode == "OneShot_Used");
        ensure!(
            !initially_spent || one_shot,
            "Used launcher requires OneShot"
        );
        ensure!(
            !weapon.is_rocket() || one_shot,
            "Rocket launcher requires OneShot"
        );
        Ok(Self {
            weapon,
            criticals,
            one_shot,
            initially_spent,
            rear_mount: critical.modes.iter().any(|mode| mode == "RearMount"),
            on_targeting_computer: critical.modes.iter().any(|mode| mode == "OnTC"),
            initial_fire_mode: if critical.modes.iter().any(|mode| mode == "Heat") {
                super::BattleFireMode::Heat
            } else if critical.modes.iter().any(|mode| mode == "Hotload") {
                super::BattleFireMode::Hotload
            } else if critical.modes.iter().any(|mode| mode == "UltraMode") {
                super::BattleFireMode::Ultra
            } else if critical.modes.iter().any(|mode| mode == "RapidFire") {
                super::BattleFireMode::Rapid
            } else if critical.modes.iter().any(|mode| mode == "Gattling") {
                super::BattleFireMode::Gatling
            } else if critical.modes.iter().any(|mode| mode == "Rotary_TwoShot") {
                super::BattleFireMode::Rotary2
            } else if critical.modes.iter().any(|mode| mode == "Rotary_FourShot") {
                super::BattleFireMode::Rotary4
            } else if critical.modes.iter().any(|mode| mode == "Rotary_SixShot") {
                super::BattleFireMode::Rotary6
            } else {
                super::BattleFireMode::Normal
            },
            initial_ammunition_mode: super::BattleAmmunitionMode::initial_selection(
                weapon,
                &critical.modes,
            ),
            brand: critical.brand,
        })
    }
}

impl<L> AmmunitionBin<L> {
    /// Resolve a bin using common catalogue capacity and ammunition flag rules.
    pub(super) fn from_critical(
        name: &str,
        critical: &super::CriticalDefinition,
        location: L,
    ) -> Result<Self> {
        let weapon = BattleWeapon::parse(name)?;
        let (capacity, half_ton, mode) = AmmunitionBin::configuration(weapon, &critical.modes)?;
        let rounds: u16 = critical.data.parse().context("Invalid ammunition count")?;
        ensure!(rounds <= capacity, "Ammunition exceeds bin capacity");
        Ok(Self {
            location,
            weapon,
            rounds,
            capacity,
            half_ton,
            hotload: critical.modes.iter().any(|mode| mode == "Hotload"),
            mode,
            brand: critical.brand,
        })
    }
}

/// Catalog-resolved loadout; this describes equipment, not a running combat unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleLoadout {
    pub weapons: Vec<WeaponMount>,
    pub ammunition: Vec<AmmunitionBin>,
    pub systems: Vec<SystemCritical>,
}

impl BattleLoadout {
    /// Group complete contiguous sink installations using the chassis slot count.
    pub(super) fn heat_sink_groups(&self, slots: usize) -> Result<Vec<Vec<CriticalLocation>>> {
        self.system_groups(BattleSystem::HeatSink, slots, "heat sink")
    }

    /// Improved jump jets occupy two contiguous, matching slots per unit of thrust.
    pub(super) fn jump_jet_groups(&self, improved: bool) -> Result<Vec<Vec<CriticalLocation>>> {
        self.system_groups(
            BattleSystem::JumpJet,
            if improved { 2 } else { 1 },
            "jump jet",
        )
    }

    /// Resolve complete same-section installations without joining separate pieces of equipment.
    fn system_groups(
        &self,
        system: BattleSystem,
        size: usize,
        label: &str,
    ) -> Result<Vec<Vec<CriticalLocation>>> {
        let slots: Vec<_> = self
            .systems
            .iter()
            .filter(|part| part.system == system)
            .collect();
        ensure!(
            slots.len().is_multiple_of(size),
            "Incomplete {label} installation"
        );
        slots
            .chunks(size)
            .map(|group| {
                let first = group[0];
                ensure!(
                    group
                        .iter()
                        .enumerate()
                        .all(
                            |(offset, part)| part.location.section == first.location.section
                                && usize::from(part.location.slot)
                                    == usize::from(first.location.slot) + offset
                                && part.brand == first.brand
                        ),
                    "Incomplete or inconsistent {label} criticals"
                );
                Ok(group.iter().map(|part| part.location).collect())
            })
            .collect()
    }

    /// Resolve every occupied slot, failing on unknown equipment or unsupported modes.
    /// Slots form complete contiguous runs, with explicit links for supported split mounts.
    pub fn resolve(template: &BattleTemplate) -> Result<Self> {
        let chassis = template.chassis()?;
        let mut loadout = Self {
            weapons: Vec::new(),
            ammunition: Vec::new(),
            systems: Vec::new(),
        };
        let mut split = split_criticals(template)?;
        for (&section, definition) in &template.sections {
            let mut consumed = BTreeSet::new();
            for (&slot, critical) in &definition.criticals {
                if consumed.contains(&slot)
                    || critical.equipment.eq_ignore_ascii_case("SplitCrit_Left")
                    || critical.equipment.eq_ignore_ascii_case("SplitCrit_Right")
                {
                    continue;
                }
                let location = CriticalLocation { section, slot };
                (|| -> Result<()> {
                    if let Some(name) =
                        super::equipment::strip_name_prefix(&critical.equipment, "Ammo_")
                    {
                        loadout
                            .ammunition
                            .push(AmmunitionBin::from_critical(name, critical, location)?);
                        return Ok(());
                    }
                    ensure!(
                        critical.data == "-"
                            || (critical.equipment.eq_ignore_ascii_case("ArtemisIV")
                                && critical.data.parse::<u8>().is_ok()),
                        "Unsupported critical data {}",
                        critical.data
                    );
                    if super::equipment::strip_name_prefix(&critical.equipment, "IS.").is_some()
                        || super::equipment::strip_name_prefix(&critical.equipment, "CL.").is_some()
                    {
                        let weapon = BattleWeapon::parse(&critical.equipment)?;
                        let mut mount = WeaponMount::from_critical(weapon, critical, Vec::new())?;
                        let count = weapon.profile().critical_slots;
                        let extension = split.remove(&location).unwrap_or_default();
                        ensure!(
                            extension.is_empty() || weapon.supports_split_mount(),
                            "Weapon does not support split criticals"
                        );
                        ensure!(
                            extension.len() < usize::from(count),
                            "Incorrect split weapon slot count"
                        );
                        let local_count = count - extension.len() as u8;
                        let local_end = slot
                            .checked_add(local_count)
                            .context("Critical range overflow")?;
                        ensure!(local_end <= 12, "Split weapon is missing its extension");
                        let mut criticals = Vec::new();
                        for index in slot..local_end {
                            ensure!(
                                !consumed.contains(&index)
                                    && definition.criticals.get(&index).is_some_and(|part| {
                                        part.equipment.eq_ignore_ascii_case(&critical.equipment)
                                            && part.data == critical.data
                                            && part.brand == critical.brand
                                            && (part.modes.is_empty()
                                                || part.modes == critical.modes)
                                    }),
                                "Incomplete or inconsistent {} criticals",
                                weapon.name()
                            );
                            criticals.push(CriticalLocation {
                                section,
                                slot: index,
                            });
                        }
                        criticals.extend(extension);
                        consumed.extend(slot..local_end);
                        mount.criticals = criticals;
                        loadout.weapons.push(mount);
                        return Ok(());
                    }
                    ensure!(critical.modes.is_empty(), "Unsupported system mode");
                    let system = BattleSystem::parse(&critical.equipment)?;
                    loadout.systems.push(SystemCritical {
                        location,
                        system,
                        brand: critical.brand,
                    });
                    Ok(())
                })()
                .with_context(|| {
                    format!(
                        "{} critical {} ({})",
                        chassis.section_name(section),
                        slot + 1,
                        critical.equipment
                    )
                })?;
            }
        }
        ensure!(
            split.is_empty(),
            "Split critical does not refer to the start of a weapon"
        );
        let myomer_slots = loadout
            .systems
            .iter()
            .filter(|part| part.system == BattleSystem::TripleStrengthMyomer)
            .count();
        ensure!(
            myomer_slots == 0 || myomer_slots >= 6,
            "Triple Strength Myomer requires six installed critical slots"
        );
        ensure!(
            !template.has_special("TripleMyomerTech") || myomer_slots >= 6,
            "TripleMyomerTech requires six installed critical slots"
        );
        loadout.heat_sink_groups(template.heat_sink_slots())?;
        loadout.jump_jet_groups(template.has_special("ImprovedJJ_Tech"))?;
        Ok(loadout)
    }
}

/// Resolve explicit zero-based parent links without turning extension markers into equipment.
fn split_criticals(
    template: &BattleTemplate,
) -> Result<BTreeMap<CriticalLocation, Vec<CriticalLocation>>> {
    use BattleSection::*;
    let mut links: BTreeMap<CriticalLocation, Vec<CriticalLocation>> = BTreeMap::new();
    for (&section, layout) in &template.sections {
        for (&slot, part) in &layout.criticals {
            let parent_section = match (part.equipment.as_str(), section) {
                (name, LeftArm | LeftLeg | CenterTorso)
                    if name.eq_ignore_ascii_case("SplitCrit_Left") =>
                {
                    LeftTorso
                }
                (name, RightArm | RightLeg | CenterTorso)
                    if name.eq_ignore_ascii_case("SplitCrit_Right") =>
                {
                    RightTorso
                }
                (name, LeftTorso) if name.eq_ignore_ascii_case("SplitCrit_Left") => LeftArm,
                (name, RightTorso) if name.eq_ignore_ascii_case("SplitCrit_Right") => RightArm,
                (name, _)
                    if name.eq_ignore_ascii_case("SplitCrit_Left")
                        || name.eq_ignore_ascii_case("SplitCrit_Right") =>
                {
                    anyhow::bail!("Invalid split critical section")
                }
                _ => continue,
            };
            let parent_slot: u8 = part
                .data
                .parse()
                .context("Invalid split critical parent slot")?;
            let parent = CriticalLocation {
                section: parent_section,
                slot: parent_slot,
            };
            let primary = template
                .sections
                .get(&parent.section)
                .and_then(|layout| layout.criticals.get(&parent.slot))
                .context("Split critical parent is not installed")?;
            let weapon = BattleWeapon::parse(&primary.equipment)?;
            ensure!(
                weapon.supports_split_mount(),
                "Weapon does not support split criticals"
            );
            ensure!(
                part.modes.is_empty() && part.brand == primary.brand,
                "Inconsistent split critical metadata"
            );
            links
                .entry(parent)
                .or_default()
                .push(CriticalLocation { section, slot });
        }
    }
    for extension in links.values() {
        let first = extension[0];
        ensure!(
            extension
                .iter()
                .enumerate()
                .all(|(offset, location)| location.section == first.section
                    && usize::from(location.slot) == usize::from(first.slot) + offset),
            "Split criticals must be contiguous in one extension section"
        );
    }
    Ok(links)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Automatic eligibility, explicit links, hardware loss and cluster exclusion compose independently.
    #[test]
    fn computer_links_share_hardware_and_ammunition_rules() {
        for weapon in [
            BattleWeapon::MediumLaser,
            BattleWeapon::MachineGun,
            BattleWeapon::Lbx10,
        ] {
            for linked in [false, true] {
                let critical = super::super::CriticalDefinition {
                    equipment: weapon.name().into(),
                    data: "-".into(),
                    brand: None,
                    modes: if linked {
                        vec!["OnTC".into()]
                    } else {
                        Vec::new()
                    },
                };
                let mount =
                    WeaponMount::<CriticalLocation>::from_critical(weapon, &critical, Vec::new())
                        .unwrap();
                for slots in [
                    vec![],
                    vec![true],
                    vec![true, true],
                    vec![true, false],
                    vec![false, true],
                ] {
                    let expected = slots.iter().all(|&available| available)
                        && (linked || (!slots.is_empty() && weapon.supports_targeting_computer()));
                    assert_eq!(
                        mount.computer_assists(
                            super::super::BattleAmmunitionMode::Normal,
                            slots.iter().copied()
                        ),
                        expected
                    );
                    assert!(!mount.computer_assists(
                        super::super::BattleAmmunitionMode::Cluster,
                        slots.iter().copied()
                    ));
                }
            }
        }
    }
}
