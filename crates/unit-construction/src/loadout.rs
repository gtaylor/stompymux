//! Resolve template critical slots into distinct weapons, ammunition bins and systems.
use super::{MechSection, MechTemplate, System, Weapon};
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A zero-based slot in a named section: a BattleMech section unless another
/// section type is given, as [`super::VehicleCriticalLocation`] does for vehicles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct CriticalLocation<S = MechSection> {
    pub section: S,
    pub slot: u8,
}

/// One installed weapon, even when it occupies several critical slots.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WeaponMount<L = CriticalLocation> {
    pub weapon: Weapon,
    /// Primary-section slots come first, followed by any linked extension slots.
    pub criticals: Vec<L>,
    pub rear_mount: bool,
    /// Explicit authored computer link, independent of automatic equipment eligibility.
    pub on_targeting_computer: bool,
    /// The mount carries one self-contained salvo instead of drawing from bins.
    pub one_shot: bool,
    /// Templates may explicitly represent an already expended launcher.
    pub initially_spent: bool,
    pub initial_fire_mode: super::FireMode,
    pub initial_ammunition_mode: super::AmmunitionMode,
}

/// One independent ammunition bin; rounds count complete weapon salvos.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AmmunitionBin<L = CriticalLocation> {
    pub location: L,
    pub weapon: Weapon,
    pub rounds: u16,
    /// Maximum salvos in this installed bin, independent of its initial contents.
    pub capacity: u16,
    pub half_ton: bool,
    /// Retained bin fire flag; launcher hotloading is selected on the weapon, not its supply.
    pub hotload: bool,
    pub mode: super::AmmunitionMode,
}

impl AmmunitionBin {
    /// Validate supported bin flags and derive capacity without inspecting live contents.
    pub fn configuration(
        weapon: Weapon,
        flags: &[String],
    ) -> Result<(u16, bool, super::AmmunitionMode)> {
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
        let mode = super::AmmunitionMode::from_flags(weapon, &ammunition_flags)?;
        let full_capacity = weapon.profile_for_ammunition(mode).ammunition_per_ton;
        let capacity = u16::from(
            if half_ton
                || matches!(
                    mode,
                    super::AmmunitionMode::Precision
                        | super::AmmunitionMode::ArmorPiercing
                        | super::AmmunitionMode::ThunderAugmented
                        | super::AmmunitionMode::ThunderActive
                )
            {
                full_capacity / 2
            } else if mode == super::AmmunitionMode::Caseless {
                full_capacity * 2
            } else {
                full_capacity
            },
        );
        Ok((capacity, half_ton, mode))
    }

    pub fn configuration_contract(
        weapon: Weapon,
        flags: &[String],
    ) -> Result<(u16, bool, super::AmmunitionMode)> {
        let full_capacity = weapon.profile().ammunition_per_ton;
        ensure!(full_capacity > 0, "Weapon does not use ammunition");
        ensure!(
            flags.iter().collect::<BTreeSet<_>>().len() == flags.len(),
            "Duplicate ammunition flags"
        );
        let half_ton = flags.iter().any(|mode| mode == "Halfton");
        let ammunition_flags: Vec<_> = flags
            .iter()
            .filter(|mode| !is_contract_fire_mode(mode))
            .cloned()
            .collect();
        ensure!(
            ammunition_flags
                .iter()
                .all(|mode| is_contract_ammunition_mode(mode)),
            "Unsupported ammunition mode {:?} for {}",
            ammunition_flags,
            weapon.name()
        );
        // The native administrator stores an unrestricted ammunition bit mask. The combat
        // projection uses its normal precedence while the critical retains every bit.
        let mode = super::AmmunitionMode::initial_selection(weapon, &ammunition_flags);
        let full_capacity = weapon.profile_for_ammunition(mode).ammunition_per_ton;
        let capacity = u16::from(
            if half_ton
                || matches!(
                    mode,
                    super::AmmunitionMode::Precision
                        | super::AmmunitionMode::ArmorPiercing
                        | super::AmmunitionMode::ThunderAugmented
                        | super::AmmunitionMode::ThunderActive
                )
            {
                full_capacity / 2
            } else if mode == super::AmmunitionMode::Caseless {
                full_capacity * 2
            } else {
                full_capacity
            },
        );
        Ok((capacity, half_ton, mode))
    }
}

/// One system critical at its installed location.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SystemCritical<L = CriticalLocation> {
    pub location: L,
    pub system: System,
}

impl<L> WeaponMount<L> {
    /// Explicit links survive without computer slots; automatic links require installed hardware.
    /// Any unavailable computer slot disables assistance, and cluster ammunition never benefits.
    pub fn computer_assists(
        &self,
        ammunition: super::AmmunitionMode,
        available_slots: impl Iterator<Item = bool>,
    ) -> bool {
        if ammunition == super::AmmunitionMode::Cluster {
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
    pub fn from_critical(
        weapon: Weapon,
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
                    || super::AmmunitionMode::from_flag(weapon, mode)
                        .is_some_and(|mode| mode.supports(weapon))
                    || (mode == "Gattling" && weapon.supports_gatling())
                    || (matches!(
                        mode.as_str(),
                        "Rotary_TwoShot"
                            | "Rotary_ThreeShot"
                            | "Rotary_FourShot"
                            | "Rotary_FiveShot"
                            | "Rotary_SixShot"
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
        Ok(Self::project(weapon, critical, criticals))
    }

    pub fn from_critical_contract(
        weapon: Weapon,
        critical: &super::CriticalDefinition,
        criticals: Vec<L>,
    ) -> Result<Self> {
        ensure!(
            critical.modes.iter().collect::<BTreeSet<_>>().len() == critical.modes.len()
                && critical.modes.iter().all(|mode| is_contract_mode(mode)),
            "Unsupported weapon modes {:?}",
            critical.modes
        );
        Ok(Self::project(weapon, critical, criticals))
    }

    fn project(weapon: Weapon, critical: &super::CriticalDefinition, criticals: Vec<L>) -> Self {
        let one_shot = critical.modes.iter().any(|mode| mode == "OneShot");
        let initially_spent = critical.modes.iter().any(|mode| mode == "OneShot_Used");
        Self {
            weapon,
            criticals,
            one_shot,
            initially_spent,
            rear_mount: critical.modes.iter().any(|mode| mode == "RearMount"),
            on_targeting_computer: critical.modes.iter().any(|mode| mode == "OnTC"),
            initial_fire_mode: if critical.modes.iter().any(|mode| mode == "Heat") {
                super::FireMode::Heat
            } else if critical.modes.iter().any(|mode| mode == "Hotload") {
                super::FireMode::Hotload
            } else if critical.modes.iter().any(|mode| mode == "UltraMode") {
                super::FireMode::Ultra
            } else if critical.modes.iter().any(|mode| mode == "RapidFire") {
                super::FireMode::Rapid
            } else if critical.modes.iter().any(|mode| mode == "Gattling") {
                super::FireMode::Gatling
            } else if critical.modes.iter().any(|mode| mode == "Rotary_TwoShot") {
                super::FireMode::Rotary2
            } else if critical.modes.iter().any(|mode| mode == "Rotary_ThreeShot") {
                super::FireMode::Rotary3
            } else if critical.modes.iter().any(|mode| mode == "Rotary_FourShot") {
                super::FireMode::Rotary4
            } else if critical.modes.iter().any(|mode| mode == "Rotary_FiveShot") {
                super::FireMode::Rotary5
            } else if critical.modes.iter().any(|mode| mode == "Rotary_SixShot") {
                super::FireMode::Rotary6
            } else {
                super::FireMode::Normal
            },
            initial_ammunition_mode: super::AmmunitionMode::initial_selection(
                weapon,
                &critical.modes,
            ),
        }
    }
}

fn is_contract_ammunition_mode(mode: &str) -> bool {
    matches!(
        mode,
        "LBX/Cluster"
            | "Artemis/Mine"
            | "Narc/Smoke"
            | "Cluster"
            | "Mine"
            | "Smoke"
            | "Inferno"
            | "Swarm"
            | "Swarm1"
            | "iNarc_Explosive"
            | "iNarc_Haywire"
            | "iNarc_ECM"
            | "iNarc_Nemesis"
            | "AP"
            | "Flechette"
            | "Incendiary"
            | "Precision"
            | "Stinger"
            | "Caseless"
            | "Sguided"
            | "ExtendedRange"
            | "HighExplosive"
            | "MML_LRM"
            | "ThunderAug"
            | "ThunderVibra"
            | "ThunderActive"
    )
}

fn is_contract_mode(mode: &str) -> bool {
    is_contract_fire_mode(mode) || is_contract_ammunition_mode(mode)
}

fn is_contract_fire_mode(mode: &str) -> bool {
    matches!(
        mode,
        "Destroyed"
            | "Disabled"
            | "Broken"
            | "Damaged"
            | "OnTC"
            | "RearMount"
            | "Hotload"
            | "Halfton"
            | "OneShot"
            | "OneShot_Used"
            | "UltraMode"
            | "RapidFire"
            | "Gattling"
            | "Rotary_TwoShot"
            | "Rotary_ThreeShot"
            | "Rotary_FourShot"
            | "Rotary_FiveShot"
            | "Rotary_SixShot"
            | "Heat"
            | "BackPack"
            | "Jettisoned"
            | "OmniBase"
            | "RocketFired"
    )
}

impl<L> AmmunitionBin<L> {
    /// Resolve a bin using common catalogue capacity and ammunition flag rules.
    pub fn from_critical(
        name: &str,
        critical: &super::CriticalDefinition,
        location: L,
    ) -> Result<Self> {
        let weapon = Weapon::parse(name)?;
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
        })
    }

    pub fn from_critical_contract(
        name: &str,
        critical: &super::CriticalDefinition,
        location: L,
    ) -> Result<Self> {
        let weapon = Weapon::parse(name)?;
        let (capacity, half_ton, mode) =
            AmmunitionBin::configuration_contract(weapon, &critical.modes)?;
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
        })
    }
}

/// Catalog-resolved equipment at locations of type `L`; this describes equipment, not
/// a running combat unit. [`MechLoadout`] and [`super::VehicleLoadout`] name it
/// for each chassis, and code written against this type serves both.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ResolvedLoadout<L> {
    pub weapons: Vec<WeaponMount<L>>,
    pub ammunition: Vec<AmmunitionBin<L>>,
    pub systems: Vec<SystemCritical<L>>,
}

/// A BattleMech's resolved equipment.
pub type MechLoadout = ResolvedLoadout<CriticalLocation>;

impl MechLoadout {
    /// Group complete contiguous sink installations using the chassis slot count.
    pub fn heat_sink_groups(&self, slots: usize) -> Result<Vec<Vec<CriticalLocation>>> {
        self.system_groups(System::HeatSink, slots, "heat sink")
    }

    /// Improved jump jets occupy two contiguous, matching slots per unit of thrust.
    pub fn jump_jet_groups(&self, improved: bool) -> Result<Vec<Vec<CriticalLocation>>> {
        self.system_groups(System::JumpJet, if improved { 2 } else { 1 }, "jump jet")
    }

    /// Resolve complete same-section installations without joining separate pieces of equipment.
    fn system_groups(
        &self,
        system: System,
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
                        ),
                    "Incomplete or inconsistent {label} criticals"
                );
                Ok(group.iter().map(|part| part.location).collect())
            })
            .collect()
    }

    /// Resolve every occupied slot, failing on unknown equipment or unsupported modes.
    /// Slots form complete contiguous runs, with explicit links for supported split mounts.
    pub fn resolve(template: &MechTemplate) -> Result<Self> {
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
                    let equipment = critical.equipment.clone();
                    if super::equipment::strip_name_prefix(&equipment, "IS.").is_some()
                        || super::equipment::strip_name_prefix(&equipment, "CL.").is_some()
                    {
                        let weapon = Weapon::parse(&equipment)?;
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
                    let system = System::parse(&critical.equipment)?;
                    loadout.systems.push(SystemCritical { location, system });
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
            .filter(|part| part.system == System::TripleStrengthMyomer)
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

    pub fn resolve_contract(template: &MechTemplate) -> Result<Self> {
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
                        match AmmunitionBin::from_critical_contract(name, critical, location) {
                            Ok(bin) => loadout.ammunition.push(bin),
                            Err(_)
                                if super::Part::parse(&critical.equipment)
                                    .is_ok_and(|part| part.kind == super::PartKind::Ammunition) => {
                            }
                            Err(error) => return Err(error),
                        }
                        return Ok(());
                    }
                    // C administration stores signed auxiliary metadata on any special slot.
                    // Artemis consumes only a one-byte link later; retaining the integer here
                    // lets construction and inspection round-trip other registered specials.
                    ensure!(
                        critical.data == "-" || critical.data.parse::<i32>().is_ok(),
                        "Unsupported critical data {}",
                        critical.data
                    );
                    let equipment = critical.equipment.clone();
                    if super::equipment::strip_name_prefix(&equipment, "IS.").is_some()
                        || super::equipment::strip_name_prefix(&equipment, "CL.").is_some()
                    {
                        let weapon = match Weapon::parse(&equipment) {
                            Ok(weapon) => weapon,
                            Err(_)
                                if contract_raw_weapon(&critical.equipment)
                                    || super::Part::parse(&critical.equipment)
                                        .is_ok_and(|part| part.kind == super::PartKind::Weapon) =>
                            {
                                return Ok(());
                            }
                            Err(error) => return Err(error),
                        };
                        let mut mount =
                            WeaponMount::from_critical_contract(weapon, critical, Vec::new())?;
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
                        let local_count = usize::from(count) - extension.len();
                        // Native administration accepts the requested critical indices in any
                        // order and, for weapons requiring at least nine slots, accepts a partial
                        // installation. Group matching slots in authored order so that exact C
                        // layouts remain usable by the runtime. Per-slot auxiliary data is not
                        // part of C weapon identity (find_weapons_advanced groups contiguous
                        // same-type runs, mech_weapons.c:349-399), so repaired weapons whose
                        // mech_repair_part zeroed one slot's data still resolve.
                        let local: Vec<_> = definition
                            .criticals
                            .range(slot..)
                            .filter(|(index, part)| {
                                !consumed.contains(*index)
                                    && part.equipment.eq_ignore_ascii_case(&critical.equipment)
                                    && (part.modes.is_empty() || part.modes == critical.modes)
                            })
                            .map(|(&index, _)| index)
                            .take(local_count)
                            .collect();
                        ensure!(
                            local.len() + extension.len() == usize::from(count)
                                || (extension.is_empty() && count >= 9 && !local.is_empty()),
                            "Incomplete or inconsistent {} criticals",
                            weapon.name()
                        );
                        let mut criticals: Vec<_> = local
                            .iter()
                            .map(|&index| CriticalLocation {
                                section,
                                slot: index,
                            })
                            .collect();
                        criticals.extend(extension);
                        consumed.extend(local);
                        mount.criticals = criticals;
                        loadout.weapons.push(mount);
                        return Ok(());
                    }
                    let system = match System::named(&critical.equipment) {
                        Some(system) => system,
                        None if super::Part::parse(&critical.equipment).is_ok_and(|part| {
                            matches!(
                                part.kind,
                                super::PartKind::Component | super::PartKind::Bomb
                            )
                        }) =>
                        {
                            return Ok(());
                        }
                        None => bail!("Unsupported equipment {}", critical.equipment),
                    };
                    ensure!(critical.modes.is_empty(), "Unsupported system mode");
                    loadout.systems.push(SystemCritical { location, system });
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
        Ok(loadout)
    }
}

/// C registers these raw infantry weapon identities even though the simulator has no
/// corresponding combat enum yet.  Contract construction retains their criticals for
/// administration, inspection and persistence while strict construction still rejects them.
pub fn contract_raw_weapon(name: &str) -> bool {
    let name = super::equipment::strip_name_prefix(name, "IS.")
        .or_else(|| super::equipment::strip_name_prefix(name, "CL."));
    name.is_some_and(|name| {
        matches!(
            name.to_ascii_lowercase().as_str(),
            "infantrylaser"
                | "heavyinfantryrifle"
                | "infantrymachinegun"
                | "infantryrifle"
                | "lightinfantryrifle"
                | "infantrylrm"
                | "infantrysrm"
                | "infantryflamer"
        )
    })
}

/// Resolve each extension marker's explicit primary section and zero-based slot
/// without turning the markers into equipment.
fn split_criticals(
    template: &MechTemplate,
) -> Result<BTreeMap<CriticalLocation, Vec<CriticalLocation>>> {
    use super::document::{is_split_proxy, parse_split_link, split_adjacent, split_proxy_name};
    let mut links: BTreeMap<CriticalLocation, Vec<CriticalLocation>> = BTreeMap::new();
    for (&section, layout) in &template.sections {
        for (&slot, part) in &layout.criticals {
            if !is_split_proxy(&part.equipment) {
                continue;
            }
            let (parent_section, parent_slot) = parse_split_link(&part.data)?;
            ensure!(
                split_adjacent(parent_section, section)
                    && part
                        .equipment
                        .eq_ignore_ascii_case(split_proxy_name(parent_section, section)),
                "Invalid split critical section"
            );
            let parent = CriticalLocation {
                section: parent_section,
                slot: parent_slot,
            };
            let primary = template
                .sections
                .get(&parent.section)
                .and_then(|layout| layout.criticals.get(&parent.slot))
                .context("Split critical parent is not installed")?;
            let weapon = Weapon::parse(&primary.equipment)?;
            ensure!(
                weapon.supports_split_mount(),
                "Weapon does not support split criticals"
            );
            ensure!(
                part.modes.is_empty(),
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
        for weapon in [Weapon::MediumLaser, Weapon::MachineGun, Weapon::Lbx10] {
            for linked in [false, true] {
                let critical = super::super::CriticalDefinition {
                    equipment: weapon.name().into(),
                    data: "-".into(),
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
                        mount
                            .computer_assists(crate::AmmunitionMode::Normal, slots.iter().copied()),
                        expected
                    );
                    assert!(
                        !mount.computer_assists(
                            crate::AmmunitionMode::Cluster,
                            slots.iter().copied()
                        )
                    );
                }
            }
        }
    }
}
