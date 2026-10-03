//! Detached, read-only BattleTech inspection records shared by host adapters.

use super::{
    BattleLoadout, BattlePart, BattlePartKind, BattleSection, BattleTemplate, BattleUnit,
    CriticalLocation, SectionDefinition,
};
use anyhow::{Context, Result};
use std::collections::BTreeMap;

/// Effective C-facing cargo maximum using both live host speed policies.
pub fn inspection_effective_maximum_speed(
    world: &crate::World,
    id: crate::ObjectId,
    config: &crate::Config,
) -> Result<f64> {
    super::effective_speed::configured(world, id, super::SpeedPolicy::configured(config))
}

/// Battle value using the same live host load policy as C-facing speed.
pub fn inspection_battle_value(
    world: &crate::World,
    id: crate::ObjectId,
    config: &crate::Config,
) -> Result<super::BattleValue> {
    super::battle_value::configured(world, id, super::SpeedPolicy::configured(config))
}

/// Pristine BattleMech value after normalizing C weapon names.
pub fn inspection_template_battle_value(template: &BattleTemplate) -> Result<super::BattleValue> {
    let compatible = inspection_compatible_template(template);
    let loadout = BattleLoadout::resolve(&compatible)?;
    let flag = |name: &str| {
        ["specials", "specials2"]
            .iter()
            .filter_map(|key| template.attributes.get(*key))
            .any(|value| {
                value
                    .split_ascii_whitespace()
                    .any(|item| item.eq_ignore_ascii_case(name))
            })
    };
    let clan = flag("Clan");
    let engine_factor = if flag("LightEngine_Tech") || (clan && flag("XLEngine_Tech")) {
        0.75_f32
    } else if flag("XLEngine_Tech") || flag("XXL_Tech") {
        0.5_f32
    } else {
        1.0_f32
    };
    let armor: u32 = template
        .sections
        .values()
        .map(|section| u32::from(section.armor) + u32::from(section.rear))
        .sum();
    let structure: u32 = BattleSection::ALL
        .into_iter()
        .map(|section| {
            canonical_mech_internal(template, section).unwrap_or_else(|| {
                template
                    .sections
                    .get(&section)
                    .map_or(0, |definition| definition.internal)
            })
        })
        .map(u32::from)
        .sum();
    let tons = super::administrative_template_tonnage(&template.attributes, template.tons);
    let mut defense = armor as f32 * 2.5 + structure as f32 * 1.5 * engine_factor;
    defense += tons as f32
        * if flag("HDGyro_Tech") || flag("HDGYRO") {
            1.0
        } else {
            0.5
        };
    let settings = super::BattleWeaponSettings::default();
    let has_case_ii = |section| {
        loadout.systems.iter().any(|part| {
            part.system == super::BattleSystem::CaseIi && part.location.section == section
        })
    };
    let ecm = loadout
        .systems
        .iter()
        .filter(|part| part.system == super::BattleSystem::Ecm)
        .count()
        >= if clan { 1 } else { 2 };
    let probe = loadout
        .systems
        .iter()
        .filter(|part| part.system == super::BattleSystem::BeagleProbe)
        .count()
        >= if clan { 1 } else { 2 };
    defense += if ecm { 61.0 } else { 0.0 } + if probe { 10.0 } else { 0.0 };
    for mount in &loadout.weapons {
        if mount.weapon.is_ams()
            || matches!(
                mount.weapon,
                super::BattleWeapon::APod | super::BattleWeapon::ClanAPod
            )
        {
            defense += settings.battle_value(mount.weapon) as f32;
        }
    }
    for bin in &loadout.ammunition {
        defense += match bin.weapon {
            super::BattleWeapon::AntiMissileSystem => 11.0,
            super::BattleWeapon::ClanAntiMissileSystem => 21.0,
            _ => 0.0,
        };
        let vulnerable = matches!(
            bin.location.section,
            BattleSection::CenterTorso
                | BattleSection::Head
                | BattleSection::LeftLeg
                | BattleSection::RightLeg
        );
        let has_case = loadout.systems.iter().any(|part| {
            part.system == super::BattleSystem::Case
                && part.location.section == bin.location.section
        });
        if !has_case_ii(bin.location.section)
            && (vulnerable || flag("XLEngine_Tech") || flag("XXL_Tech") || !has_case)
        {
            defense -= 15.0;
        }
    }
    let vulnerable_core = |section| {
        matches!(
            section,
            BattleSection::CenterTorso
                | BattleSection::Head
                | BattleSection::LeftLeg
                | BattleSection::RightLeg
        )
    };
    let xl = flag("XLEngine_Tech") || flag("XXL_Tech");
    let has_case = |section| {
        template.sections.get(&section).is_some_and(|definition| {
            definition
                .configuration
                .as_deref()
                .is_some_and(|value| value.eq_ignore_ascii_case("Case"))
        })
    };
    for (&section, definition) in &template.sections {
        let exposed = if has_case_ii(section) {
            false
        } else if (clan && vulnerable_core(section)) || xl {
            true
        } else if vulnerable_core(section) {
            !has_case(section)
        } else {
            match section {
                BattleSection::LeftArm => !has_case(BattleSection::LeftTorso),
                BattleSection::RightArm => !has_case(BattleSection::RightTorso),
                _ => false,
            }
        };
        if exposed {
            defense -= definition
                .criticals
                .values()
                .filter(|critical| {
                    super::BattleWeapon::parse(&critical.equipment)
                        .is_ok_and(|weapon| weapon.weapon_explosion_damage() > 0)
                })
                .count() as f32;
        }
    }
    let jump = (template.jump_speed as f32 / 10.75) as i32;
    let running = (template.max_speed as f32 / 10.75) as i32;
    let movement = match running {
        ..=2 => 0,
        3..=4 => 1,
        5..=6 => 2,
        7..=9 => 3,
        10..=17 => 4,
        18..=24 => 5,
        _ => 6,
    } + i32::from(jump > running);
    defense += defense * movement as f32 * 0.1;
    defense = ((defense * 100.0).round() / 100.0).max(0.0);
    let heat_efficiency = 6 + i32::from(template.heat_sinks) - jump.max(2);
    let mut weapons: Vec<_> = loadout.weapons.iter().map(|mount| mount.weapon).collect();
    weapons.sort_by_key(|weapon| settings.battle_value(*weapon));
    let mut heat = 0_i32;
    let mut offense = tons as f32;
    for weapon in weapons.into_iter().rev().filter(|weapon| !weapon.is_ams()) {
        heat += i32::from(weapon.profile().heat);
        let value = settings.battle_value(weapon);
        offense += (if heat > heat_efficiency {
            value / 2
        } else {
            value
        }) as f32;
    }
    Ok(super::BattleValue {
        offensive: f64::from(offense),
        defensive: f64::from(defense),
        total: f64::from(offense) + f64::from(defense),
    })
}

/// Pristine vehicle value after normalizing C weapon names.
pub fn inspection_vehicle_template_battle_value(
    template: &super::BattleVehicleTemplate,
) -> Result<super::BattleValue> {
    super::BattleVehicle::new(inspection_compatible_vehicle_template(template))?.battle_value()
}

/// Stable C-facing section identity for a supported BattleMech.
pub fn inspection_section_code(template: &BattleTemplate, section: BattleSection) -> Result<i32> {
    use BattleSection::*;
    let quad = template.chassis()? == super::BattleMechChassis::Quad;
    Ok(match (quad, section) {
        (true, LeftArm) => 0,
        (true, RightArm) => 1,
        (_, LeftTorso) => 2,
        (_, RightTorso) => 3,
        (_, CenterTorso) => 4,
        (true, LeftLeg) => 5,
        (true, RightLeg) => 6,
        (_, Head) => 7,
        (false, LeftArm) => 8,
        (false, RightArm) => 9,
        (false, LeftLeg) => 10,
        (false, RightLeg) => 11,
    })
}

/// Decode a stable C-facing section identity for a supported BattleMech.
pub fn inspection_section(template: &BattleTemplate, code: i32) -> Result<BattleSection> {
    BattleSection::ALL
        .into_iter()
        .find(|section| inspection_section_code(template, *section).ok() == Some(code))
        .context("section is not valid for this unit")
}

/// Current and original protection totals for one section or an entire unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InspectionArmor {
    pub section: Option<i32>,
    pub armor: (u32, u32),
    pub internal: (u32, u32),
    pub rear_armor: (u32, u32),
}

/// Inspect current live protection, aggregating when `section` is absent.
pub fn inspect_unit_armor(
    unit: &BattleUnit,
    section: Option<BattleSection>,
) -> Result<InspectionArmor> {
    let mut row = InspectionArmor {
        section: section
            .map(|value| inspection_section_code(unit.definition(), value))
            .transpose()?,
        armor: (0, 0),
        internal: (0, 0),
        rear_armor: (0, 0),
    };
    for current_section in BattleSection::ALL {
        if section.is_some_and(|selected| selected != current_section) {
            continue;
        }
        let current = unit
            .sections()
            .get(&current_section)
            .context("unit section is missing")?;
        let original = unit
            .definition()
            .sections
            .get(&current_section)
            .context("template section is missing")?;
        row.armor.0 += u32::from(current.armor);
        row.armor.1 += u32::from(original.armor);
        row.internal.0 += u32::from(current.internal);
        row.internal.1 += u32::from(original.internal);
        row.rear_armor.0 += u32::from(current.rear);
        row.rear_armor.1 += u32::from(original.rear);
    }
    Ok(row)
}

/// Inspect pristine template protection.
pub fn inspect_template_armor(
    template: &BattleTemplate,
    section: Option<BattleSection>,
) -> Result<InspectionArmor> {
    let mut row = InspectionArmor {
        section: section
            .map(|value| inspection_section_code(template, value))
            .transpose()?,
        armor: (0, 0),
        internal: (0, 0),
        rear_armor: (0, 0),
    };
    for current_section in BattleSection::ALL {
        if section.is_some_and(|selected| selected != current_section) {
            continue;
        }
        let original = template
            .sections
            .get(&current_section)
            .context("template section is missing")?;
        row.armor.0 += u32::from(original.armor);
        row.armor.1 += u32::from(original.armor);
        let internal =
            canonical_mech_internal(template, current_section).unwrap_or(original.internal);
        row.internal.0 += u32::from(internal);
        row.internal.1 += u32::from(internal);
        row.rear_armor.0 += u32::from(original.rear);
        row.rear_armor.1 += u32::from(original.rear);
    }
    Ok(row)
}

/// Inspect pristine protection for any native unit class without constructing a combat runtime.
pub fn inspect_raw_template_armor(
    template: &super::RawTemplate,
    section: Option<super::RawSectionCode>,
) -> Result<InspectionArmor> {
    let mut row = InspectionArmor {
        section: section.map(|value| value as i32),
        armor: (0, 0),
        internal: (0, 0),
        rear_armor: (0, 0),
    };
    for current in super::RawSectionCode::for_unit(template.class, template.movement) {
        if section.is_some_and(|selected| selected != *current) {
            continue;
        }
        let definition = template
            .sections
            .get(current)
            .context("raw template section is missing")?;
        row.armor.0 += u32::from(definition.armor);
        row.armor.1 += u32::from(definition.armor);
        let internal =
            raw_canonical_mech_internal(template, *current).unwrap_or(definition.internal);
        row.internal.0 += u32::from(internal);
        row.internal.1 += u32::from(internal);
        row.rear_armor.0 += u32::from(definition.rear);
        row.rear_armor.1 += u32::from(definition.rear);
    }
    Ok(row)
}

fn raw_canonical_mech_internal(
    template: &super::RawTemplate,
    section: super::RawSectionCode,
) -> Option<u16> {
    if matches!(
        template.class,
        super::RawUnitClass::Vehicle | super::RawUnitClass::Vtol | super::RawUnitClass::Naval
    ) {
        let authored = template.sections.get(&section)?.internal;
        if authored == 0 {
            return Some(0);
        }
        let rounded = template.tons.saturating_add(5);
        return Some((rounded.max(10) / 10).clamp(0, 255) as u16);
    }
    if template.class != super::RawUnitClass::Mech || template.tons == 0 {
        return None;
    }
    if section == super::RawSectionCode::Head {
        return Some(3);
    }
    const STRUCTURE: [[u16; 4]; 19] = [
        [4, 3, 1, 2],
        [5, 4, 2, 3],
        [6, 5, 3, 4],
        [8, 6, 4, 6],
        [10, 7, 5, 7],
        [11, 8, 6, 8],
        [12, 10, 6, 10],
        [14, 11, 7, 11],
        [16, 12, 8, 12],
        [18, 13, 9, 13],
        [20, 14, 10, 14],
        [21, 15, 10, 15],
        [22, 15, 11, 15],
        [23, 16, 12, 16],
        [25, 17, 13, 17],
        [27, 18, 14, 18],
        [29, 19, 15, 19],
        [30, 20, 16, 20],
        [31, 21, 17, 21],
    ];
    let tons = usize::try_from(template.tons).ok()?;
    if !(10..=100).contains(&tons) || tons % 5 != 0 {
        return None;
    }
    let values = STRUCTURE[(tons - 10) / 5];
    Some(match section {
        super::RawSectionCode::CenterTorso => values[0],
        super::RawSectionCode::LeftTorso | super::RawSectionCode::RightTorso => values[1],
        super::RawSectionCode::LeftArm | super::RawSectionCode::RightArm => values[2],
        super::RawSectionCode::LeftLeg | super::RawSectionCode::RightLeg => values[3],
        super::RawSectionCode::FrontLeftLeg
        | super::RawSectionCode::FrontRightLeg
        | super::RawSectionCode::RearLeftLeg
        | super::RawSectionCode::RearRightLeg => values[3],
        _ => return None,
    })
}

/// Return the post-load internal structure enforced by the native template checker.
fn canonical_mech_internal(template: &BattleTemplate, section: BattleSection) -> Option<u16> {
    // A freshly registered native MECH has cleared zero-ton construction state.
    // Structure normalization begins only after a nonzero template is loaded.
    if template.tons == 0 {
        return None;
    }
    const STRUCTURE: [[u16; 4]; 19] = [
        [4, 3, 1, 2],
        [5, 4, 2, 3],
        [6, 5, 3, 4],
        [8, 6, 4, 6],
        [10, 7, 5, 7],
        [11, 8, 6, 8],
        [12, 10, 6, 10],
        [14, 11, 7, 11],
        [16, 12, 8, 12],
        [18, 13, 9, 13],
        [20, 14, 10, 14],
        [21, 15, 10, 15],
        [22, 15, 11, 15],
        [23, 16, 12, 16],
        [25, 17, 13, 17],
        [27, 18, 14, 18],
        [29, 19, 15, 19],
        [30, 20, 16, 20],
        [31, 21, 17, 21],
    ];
    if section == BattleSection::Head {
        return Some(3);
    }
    let tons = usize::from(template.tons);
    if !(10..=100).contains(&tons) || tons % 5 != 0 {
        return None;
    }
    let values = STRUCTURE[(tons - 10) / 5];
    let quad = template.chassis().ok()?.is_leg(section);
    Some(match section {
        BattleSection::CenterTorso => values[0],
        BattleSection::LeftTorso | BattleSection::RightTorso => values[1],
        BattleSection::LeftArm | BattleSection::RightArm if quad => values[3],
        BattleSection::LeftArm | BattleSection::RightArm => values[2],
        BattleSection::LeftLeg | BattleSection::RightLeg => values[3],
        BattleSection::Head => unreachable!(),
    })
}

/// Detached identity of an installed part.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct InspectionPart {
    pub id: i32,
}

/// Resolve the equipment spelling used by C templates to its stable catalogue identity.
///
/// Template internals occupy the component range before similarly named cargo.  In
/// particular `CASE-II` means component 427 in a critical slot, while loose cargo
/// 562 deliberately has the same display name.
pub(crate) fn inspection_template_part(name: &str) -> Option<(InspectionPart, bool, bool)> {
    if let Ok(weapon) = super::BattleWeapon::parse(name) {
        return Some((
            InspectionPart {
                id: weapon.part_id(),
            },
            true,
            true,
        ));
    }
    if let Some(name) = super::equipment::strip_name_prefix(name, "Ammo_") {
        let weapon = super::BattleWeapon::parse(name).ok()?;
        return Some((
            InspectionPart {
                id: weapon.ammunition_part_id(),
            },
            false,
            true,
        ));
    }
    if let Some(form) = super::part_catalogue().iter().find(|form| {
        form.very_long_name.eq_ignore_ascii_case(name)
            || form.long_name.eq_ignore_ascii_case(name)
            || form.short_name.eq_ignore_ascii_case(name)
    }) {
        let part = BattlePart::from_id(form.part_id)?;
        let is_weapon = part.kind == BattlePartKind::Weapon;
        return Some((
            InspectionPart { id: form.part_id },
            is_weapon,
            is_weapon || part.kind == BattlePartKind::Ammunition,
        ));
    }
    let internal = (394..=445)
        .filter_map(BattlePart::from_id)
        .find(|part| part.name.eq_ignore_ascii_case(name));
    let part =
        internal.or_else(|| BattlePart::all().find(|part| part.name.eq_ignore_ascii_case(name)))?;
    let weapon = part.kind == BattlePartKind::Weapon;
    Some((InspectionPart { id: part.part_id }, weapon, weapon))
}

/// Normalize native weapon names and mech internals for the combat catalogue.
pub(crate) fn inspection_compatible_template(template: &BattleTemplate) -> BattleTemplate {
    let mut compatible = template.clone();
    for section in BattleSection::ALL {
        if let Some(internal) = canonical_mech_internal(template, section)
            && let Some(definition) = compatible.sections.get_mut(&section)
        {
            definition.internal = internal;
        }
    }
    compatible.jump_speed = inspection_normalized_jump_speed(template);
    for definition in compatible.sections.values_mut() {
        for critical in definition.criticals.values_mut() {
            if let Ok(weapon) = super::BattleWeapon::parse(&critical.equipment) {
                critical.equipment = weapon.name().to_owned();
            }
        }
    }
    compatible
}

/// Jump speed after C's `do_sub_magic` reconciles authored speed with installed jets.
pub(crate) fn inspection_normalized_jump_speed(template: &BattleTemplate) -> f64 {
    let improved = configured_technology(template, "ImprovedJJ_Tech", "secondary");
    let mut criticals = template
        .sections
        .values()
        .flat_map(|section| section.criticals.values())
        .filter(|critical| critical.equipment.eq_ignore_ascii_case("JumpJet"))
        .count() as u32;
    if improved {
        criticals /= 2;
        criticals = criticals.min((template.max_speed as f32 * 0.093_023_3_f32).trunc() as u32);
    } else {
        // Preserve the source's integer `(2 / 3)` maximum-jump multiplier.
        criticals = 0;
    }
    f64::from(criticals) * 10.75
}

/// Normalize native vehicle weapon names for the combat catalogue.
pub(crate) fn inspection_compatible_vehicle_template(
    template: &super::BattleVehicleTemplate,
) -> super::BattleVehicleTemplate {
    let mut compatible = template.clone();
    for definition in compatible.sections.values_mut() {
        for critical in definition.criticals.values_mut() {
            if let Ok(weapon) = super::BattleWeapon::parse(&critical.equipment) {
                critical.equipment = weapon.name().to_owned();
            }
        }
    }
    compatible
}

/// One C-compatible mounted-weapon row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectionWeapon {
    pub number: usize,
    pub section: i32,
    pub first_slot: u8,
    pub part: InspectionPart,
    pub slot_count: u8,
    pub recycle: u16,
    pub recycle_time: u8,
    pub operational: bool,
}

/// One physical critical slot, including empty locations.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectionCritical {
    pub section: i32,
    pub slot: u8,
    pub kind: &'static str,
    pub part: Option<InspectionPart>,
    pub operational: bool,
    pub temporary_failure: bool,
    pub auxiliary_data: i32,
    pub ammunition: Option<(u16, u16)>,
    pub fire_modes: Vec<i32>,
    pub ammunition_modes: Vec<i32>,
}

fn critical_rows(
    template: &BattleTemplate,
    live: Option<&BattleUnit>,
    section: BattleSection,
) -> Result<Vec<InspectionCritical>> {
    let compatible = inspection_compatible_template(template);
    let loadout = BattleLoadout::resolve(&compatible).ok();
    let definition = &template.sections[&section];
    let section_code = inspection_section_code(template, section)?;
    let mut rows = Vec::with_capacity(12);
    for slot in 0..12_u8 {
        let location = CriticalLocation { section, slot };
        let critical = definition.criticals.get(&slot);
        let weapon = loadout.as_ref().and_then(|loadout| {
            loadout
                .weapons
                .iter()
                .find(|mount| mount.criticals.contains(&location))
        });
        let ammunition = loadout.as_ref().and_then(|loadout| {
            loadout
                .ammunition
                .iter()
                .enumerate()
                .find(|(_, bin)| bin.location == location)
        });
        let system = loadout.as_ref().and_then(|loadout| {
            loadout
                .systems
                .iter()
                .find(|part| part.location == location)
        });
        let raw_identity = critical.and_then(raw_part);
        let part = if let Some(mount) = weapon {
            Some(InspectionPart {
                id: mount.weapon.part_id(),
            })
        } else if let Some((_, bin)) = ammunition {
            Some(InspectionPart {
                id: bin.weapon.ammunition_part_id(),
            })
        } else if system.is_some() {
            critical
                .and_then(|raw| BattlePart::parse(&raw.equipment).ok())
                .map(|part| InspectionPart { id: part.part_id })
        } else {
            raw_identity.map(|value| value.0)
        };
        let rounds = ammunition
            .map(|(index, bin)| {
                (
                    live.map_or(bin.rounds, |unit| unit.ammunition()[index]),
                    bin.capacity,
                )
            })
            .or_else(|| {
                raw_identity
                    .filter(|(_, _, payload)| {
                        *payload
                            && part.is_some_and(|part| {
                                BattlePart::ammunition_weapon_id(part.id).is_some()
                            })
                    })
                    .map(|_| {
                        let capacity = critical
                            .and_then(|raw| raw.data.parse::<u16>().ok())
                            .unwrap_or(0);
                        (capacity, capacity)
                    })
            });
        let weapon_index = weapon.and_then(|mount| {
            loadout
                .as_ref()?
                .weapons
                .iter()
                .position(|item| std::ptr::eq(item, mount))
        });
        let mut fire_modes =
            critical.map_or_else(Vec::new, |part| template_fire_modes(&part.modes));
        let mut ammunition_modes =
            critical.map_or_else(Vec::new, |part| template_ammunition_modes(&part.modes));
        if let (Some(unit), Some(index)) = (live, weapon_index) {
            fire_modes.extend(live_fire_mode(unit.fire_mode(index).unwrap_or_default()));
            ammunition_modes.extend(live_ammunition_mode(
                unit.ammunition_mode(index).unwrap_or_default(),
                loadout.as_ref().unwrap().weapons[index].weapon,
            ));
        }
        if live.is_some_and(|unit| unit.lost_criticals().contains(&location)) {
            fire_modes.push(1);
        }
        fire_modes.sort_unstable();
        fire_modes.dedup();
        ammunition_modes.sort_unstable();
        ammunition_modes.dedup();
        rows.push(InspectionCritical {
            section: section_code,
            slot: slot + 1,
            kind: if weapon.is_some() || raw_identity.is_some_and(|(_, weapon, _)| weapon) {
                "weapon"
            } else if ammunition.is_some() || rounds.is_some() {
                "ammunition"
            } else if let Some(part) = part.and_then(|p| BattlePart::from_id(p.id)) {
                match part.kind {
                    BattlePartKind::Bomb => "bomb",
                    BattlePartKind::Commodity => "cargo",
                    _ => "special",
                }
            } else {
                "empty"
            },
            part,
            operational: !fire_modes.iter().any(|mode| matches!(mode, 1 | 2 | 4)),
            temporary_failure: live.is_some_and(|unit| {
                weapon_index.is_some_and(|index| unit.weapon_failures().contains_key(&index))
            }),
            auxiliary_data: rounds.map_or_else(
                || {
                    critical
                        .and_then(|part| part.data.parse().ok())
                        .unwrap_or(0)
                },
                |value| i32::from(value.0),
            ),
            ammunition: rounds,
            fire_modes,
            ammunition_modes,
        });
    }
    Ok(rows)
}

fn template_fire_modes(flags: &[String]) -> Vec<i32> {
    const MODES: &[(&str, i32)] = &[
        ("Destroyed", 1),
        ("Disabled", 2),
        ("Broken", 4),
        ("Damaged", 8),
        ("RearMount", 32),
        ("Hotload", 64),
        ("Halfton", 128),
        ("OneShot", 256),
        ("OneShot_Used", 512),
        ("UltraMode", 1024),
        ("RapidFire", 2048),
        ("Gattling", 4096),
        ("Rotary_TwoShot", 8192),
        ("Rotary_FourShot", 16384),
        ("Rotary_SixShot", 32768),
        ("Heat", 65536),
        ("BackPack", 131072),
        ("Jettisoned", 262144),
        ("OmniBase", 524288),
        ("RocketFired", 1048576),
        ("Rotary_ThreeShot", 2097152),
        ("Rotary_FiveShot", 4194304),
        ("OnTC", 16),
    ];
    MODES
        .iter()
        .filter(|(name, _)| flags.iter().any(|flag| flag == name))
        .map(|(_, value)| *value)
        .collect()
}

fn template_ammunition_modes(flags: &[String]) -> Vec<i32> {
    const MODES: &[(&str, i32)] = &[
        ("LBX/Cluster", 1),
        ("Artemis/Mine", 2),
        ("Narc/Smoke", 4),
        ("Cluster", 8),
        ("Mine", 16),
        ("Smoke", 32),
        ("Inferno", 64),
        ("Swarm", 128),
        ("Swarm1", 256),
        ("iNarc_Explosive", 512),
        ("iNarc_Haywire", 1024),
        ("iNarc_ECM", 2048),
        ("iNarc_Nemesis", 4096),
        ("AP", 8192),
        ("Flechette", 16384),
        ("Incendiary", 32768),
        ("Precision", 65536),
        ("Stinger", 131072),
        ("Caseless", 262144),
        ("Sguided", 524288),
        ("ExtendedRange", 1048576),
        ("HighExplosive", 2097152),
        ("MML_LRM", 4194304),
        ("ThunderAug", 8388608),
        ("ThunderVibra", 16777216),
        ("ThunderActive", 33554432),
    ];
    MODES
        .iter()
        .filter(|(name, _)| flags.iter().any(|flag| flag == name))
        .map(|(_, value)| *value)
        .collect()
}

fn live_fire_mode(mode: super::BattleFireMode) -> Option<i32> {
    Some(match mode {
        super::BattleFireMode::Normal => return None,
        super::BattleFireMode::Heat => 65536,
        super::BattleFireMode::Hotload => 64,
        super::BattleFireMode::Ultra => 1024,
        super::BattleFireMode::Rapid => 2048,
        super::BattleFireMode::Rotary2 => 8192,
        super::BattleFireMode::Rotary3 => 2097152,
        super::BattleFireMode::Rotary4 => 16384,
        super::BattleFireMode::Rotary5 => 4194304,
        super::BattleFireMode::Rotary6 => 32768,
        super::BattleFireMode::Gatling => 4096,
    })
}
/// Live selection bits; MML long-range special rounds report both the round and family bits.
fn live_ammunition_mode(
    mode: super::BattleAmmunitionMode,
    weapon: super::BattleWeapon,
) -> Vec<i32> {
    let mut bits: Vec<i32> = live_munition_bit(mode.munition(), weapon)
        .into_iter()
        .collect();
    if mode.is_mml_lrm() {
        bits.push(4194304);
    }
    bits
}

/// The single reference bit for a round, excluding the MML family bit.
fn live_munition_bit(
    mode: super::BattleAmmunitionMode,
    weapon: super::BattleWeapon,
) -> Option<i32> {
    Some(match mode {
        super::BattleAmmunitionMode::Normal
        | super::BattleAmmunitionMode::MmlLrm
        | super::BattleAmmunitionMode::MmlLrmArtemis
        | super::BattleAmmunitionMode::MmlLrmNarc
        | super::BattleAmmunitionMode::MmlLrmSwarm
        | super::BattleAmmunitionMode::MmlLrmSwarm1
        | super::BattleAmmunitionMode::MmlLrmSemiGuided
        | super::BattleAmmunitionMode::MmlLrmStinger => return None,
        super::BattleAmmunitionMode::Cluster => {
            if weapon.is_lbx() {
                1
            } else {
                8
            }
        }
        super::BattleAmmunitionMode::Smoke => 32,
        super::BattleAmmunitionMode::Mine => 16,
        super::BattleAmmunitionMode::Artemis => 2,
        super::BattleAmmunitionMode::Narc => 4,
        super::BattleAmmunitionMode::SemiGuided => 524288,
        super::BattleAmmunitionMode::Swarm => 128,
        super::BattleAmmunitionMode::Swarm1 => 256,
        super::BattleAmmunitionMode::Stinger => 131072,
        super::BattleAmmunitionMode::ExtendedRange => 1048576,
        super::BattleAmmunitionMode::HighExplosive => 2097152,
        super::BattleAmmunitionMode::INarcExplosive => 512,
        super::BattleAmmunitionMode::INarcHaywire => 1024,
        super::BattleAmmunitionMode::INarcEcm => 2048,
        super::BattleAmmunitionMode::INarcNemesis => 4096,
        super::BattleAmmunitionMode::Precision => 65536,
        super::BattleAmmunitionMode::Flechette => 16384,
        super::BattleAmmunitionMode::ArmorPiercing => 8192,
        super::BattleAmmunitionMode::Caseless => 262144,
        super::BattleAmmunitionMode::Incendiary => 32768,
        super::BattleAmmunitionMode::Inferno => 64,
        super::BattleAmmunitionMode::ThunderAugmented => 8388608,
        super::BattleAmmunitionMode::ThunderVibrabomb => 16777216,
        super::BattleAmmunitionMode::ThunderActive => 33554432,
    })
}

/// Inspect all twelve slots of a pristine template section.
pub fn inspect_template_criticals(
    template: &BattleTemplate,
    section: BattleSection,
) -> Result<Vec<InspectionCritical>> {
    critical_rows(template, None, section)
}

/// Inspect all twelve raw slots without requiring a class-specific runtime.
pub fn inspect_raw_template_criticals(
    template: &super::RawTemplate,
    section: super::RawSectionCode,
) -> Result<Vec<InspectionCritical>> {
    let definition = template
        .sections
        .get(&section)
        .context("raw template section is missing")?;
    let mut rows = Vec::with_capacity(12);
    for slot in 0..12_u8 {
        let critical = definition.criticals.get(&slot);
        let identity = critical.and_then(raw_part);
        let part = identity.map(|value| value.0);
        let mut fire_modes = critical.map_or_else(Vec::new, |raw| template_fire_modes(&raw.modes));
        let mut ammunition_modes =
            critical.map_or_else(Vec::new, |raw| template_ammunition_modes(&raw.modes));
        fire_modes.sort_unstable();
        fire_modes.dedup();
        ammunition_modes.sort_unstable();
        ammunition_modes.dedup();
        let ammunition = identity
            .filter(|(part, _, _)| BattlePart::ammunition_weapon_id(part.id).is_some())
            .map(|_| {
                let rounds = critical
                    .and_then(|raw| raw.data.parse::<i32>().ok())
                    .unwrap_or_default()
                    .clamp(0, 255) as u16;
                (rounds, rounds)
            });
        let auxiliary_data = ammunition.map_or_else(
            || {
                critical
                    .and_then(|raw| raw.data.parse::<i32>().ok())
                    .unwrap_or_default()
                    .clamp(0, 255)
            },
            |(rounds, _)| i32::from(rounds),
        );
        rows.push(InspectionCritical {
            section: section as i32,
            slot: slot + 1,
            kind: match identity {
                Some((_, true, _)) => "weapon",
                Some((part, _, _)) if BattlePart::ammunition_weapon_id(part.id).is_some() => {
                    "ammunition"
                }
                Some((part, _, _)) => match BattlePart::from_id(part.id).map(|part| part.kind) {
                    Some(BattlePartKind::Bomb) => "bomb",
                    Some(BattlePartKind::Commodity) => "cargo",
                    Some(BattlePartKind::Weapon) => "weapon",
                    _ => "special",
                },
                None => "empty",
            },
            part,
            operational: !fire_modes.iter().any(|mode| matches!(mode, 1 | 2 | 4)),
            temporary_failure: false,
            auxiliary_data,
            ammunition,
            fire_modes,
            ammunition_modes,
        });
    }
    Ok(rows)
}

/// Inspect all twelve slots of a live unit section.
pub fn inspect_unit_criticals(
    unit: &BattleUnit,
    section: BattleSection,
) -> Result<Vec<InspectionCritical>> {
    critical_rows(unit.definition(), Some(unit), section)
}

fn weapon_rows(
    template: &BattleTemplate,
    loadout: &BattleLoadout,
) -> Result<Vec<InspectionWeapon>> {
    loadout
        .weapons
        .iter()
        .enumerate()
        .map(|(number, mount)| {
            let first = *mount
                .criticals
                .first()
                .context("weapon has no critical slot")?;
            Ok(InspectionWeapon {
                number,
                section: inspection_section_code(template, first.section)?,
                first_slot: first.slot + 1,
                part: InspectionPart {
                    id: mount.weapon.part_id(),
                },
                slot_count: mount.weapon.profile().critical_slots,
                recycle: 0,
                recycle_time: mount.weapon.profile().recycle_seconds,
                operational: true,
            })
        })
        .collect()
}

/// Inspect pristine template weapons in game-number order.
pub fn inspect_template_weapons(template: &BattleTemplate) -> Result<Vec<InspectionWeapon>> {
    let compatible = inspection_compatible_template(template);
    if let Ok(loadout) = BattleLoadout::resolve(&compatible) {
        return weapon_rows(template, &loadout);
    }
    let mut sections: Vec<_> = template.sections.keys().copied().collect();
    sections.sort_by_key(|section| inspection_section_code(template, *section).unwrap_or(i32::MAX));
    let mut rows = Vec::new();
    for section in sections {
        let definition = &template.sections[&section];
        let mut consumed = std::collections::BTreeSet::new();
        for (&slot, raw) in &definition.criticals {
            if consumed.contains(&slot) {
                continue;
            }
            let Ok(weapon) = super::BattleWeapon::parse(&raw.equipment) else {
                continue;
            };
            let slots = weapon.profile().critical_slots;
            for (&candidate, other) in definition.criticals.range(slot..) {
                if consumed.len() >= usize::from(slots) {
                    break;
                }
                if other.equipment.eq_ignore_ascii_case(&raw.equipment) && other.data == raw.data {
                    consumed.insert(candidate);
                }
            }
            rows.push(InspectionWeapon {
                number: rows.len(),
                section: inspection_section_code(template, section)?,
                first_slot: slot + 1,
                part: InspectionPart {
                    id: weapon.part_id(),
                },
                slot_count: slots,
                recycle: 0,
                recycle_time: weapon.profile().recycle_seconds,
                operational: true,
            });
        }
    }
    Ok(rows)
}

/// Inspect raw mounted weapons in native section and slot order.
pub fn inspect_raw_template_weapons(
    template: &super::RawTemplate,
) -> Result<Vec<InspectionWeapon>> {
    let mut rows = Vec::new();
    for section in super::RawSectionCode::for_unit(template.class, template.movement) {
        let definition = template
            .sections
            .get(section)
            .context("raw template section is missing")?;
        let mut slot = 0_u8;
        while slot < 12 {
            let Some(raw) = definition.criticals.get(&slot) else {
                slot += 1;
                continue;
            };
            let Some((part, true, _)) = raw_part(raw) else {
                slot += 1;
                continue;
            };
            let profile_slots = super::BattleWeapon::from_part_id(part.id)
                .map_or(1, |weapon| weapon.profile().critical_slots);
            let slot_count = if template.class == super::RawUnitClass::Mech {
                profile_slots
            } else {
                1
            };
            rows.push(InspectionWeapon {
                number: rows.len(),
                section: *section as i32,
                first_slot: slot + 1,
                part,
                slot_count,
                recycle: 0,
                recycle_time: raw_weapon_recycle_time(part.id),
                operational: !template_fire_modes(&raw.modes)
                    .iter()
                    .any(|mode| matches!(mode, 1 | 2 | 4)),
            });
            slot = slot.saturating_add(slot_count.max(1));
        }
    }
    Ok(rows)
}

fn raw_weapon_recycle_time(part_id: i32) -> u8 {
    super::BattleWeapon::from_part_id(part_id).map_or_else(
        || match part_id {
            171 => 7,
            172 => 10,
            173 => 15,
            174 => 5,
            175 | 176 => 10,
            177 | 178 => 20,
            _ => 0,
        },
        |weapon| weapon.profile().recycle_seconds,
    )
}

/// Inspect live mounted weapons in game-number order.
pub fn inspect_unit_weapons(unit: &BattleUnit) -> Result<Vec<InspectionWeapon>> {
    let mut rows = weapon_rows(unit.definition(), &unit.loadout()?)?;
    for row in &mut rows {
        row.recycle = unit.weapon_recycle().get(&row.number).copied().unwrap_or(0);
        row.operational = unit.weapon_intact(row.number)?;
    }
    Ok(rows)
}

/// Count installed parts in catalogue identity order.
pub fn inspect_template_inventory(
    template: &BattleTemplate,
    payload_only: bool,
) -> Result<Vec<(InspectionPart, u32)>> {
    inventory_rows(template, None, payload_only)
}

/// Count raw installed parts in native catalogue identity order.
pub fn inspect_raw_template_inventory(
    template: &super::RawTemplate,
    payload_only: bool,
) -> Result<Vec<(InspectionPart, u32)>> {
    let registered: std::collections::BTreeSet<_> = super::part_catalogue()
        .iter()
        .map(|form| form.part_id)
        .collect();
    let mut quantities = BTreeMap::<InspectionPart, u32>::new();
    for section in super::RawSectionCode::for_unit(template.class, template.movement) {
        let definition = template
            .sections
            .get(section)
            .context("raw template section is missing")?;
        let mut previous = None;
        for slot in 0..12_u8 {
            let Some(raw) = definition.criticals.get(&slot) else {
                previous = None;
                continue;
            };
            if template_fire_modes(&raw.modes).contains(&1) {
                previous = None;
                continue;
            }
            let Some((part, is_weapon, is_payload)) = raw_part(raw) else {
                previous = None;
                continue;
            };
            if !registered.contains(&part.id) || (payload_only && !is_payload) {
                previous = None;
                continue;
            }
            if is_weapon && previous == Some(part) {
                continue;
            }
            *quantities.entry(part).or_default() += 1;
            previous = Some(part);
        }
    }
    Ok(quantities.into_iter().collect())
}

/// Count surviving live parts in catalogue identity order.
pub fn inspect_unit_inventory(
    unit: &BattleUnit,
    payload_only: bool,
) -> Result<Vec<(InspectionPart, u32)>> {
    inventory_rows(unit.definition(), Some(unit), payload_only)
}

fn inventory_rows(
    template: &BattleTemplate,
    live: Option<&BattleUnit>,
    payload_only: bool,
) -> Result<Vec<(InspectionPart, u32)>> {
    let mut quantities = BTreeMap::<InspectionPart, u32>::new();
    for section in BattleSection::ALL {
        let mut previous = None;
        for slot in 0..12_u8 {
            let location = CriticalLocation { section, slot };
            let Some(raw) = template.sections[&section].criticals.get(&slot) else {
                previous = None;
                continue;
            };
            if template_fire_modes(&raw.modes).contains(&1)
                || live.is_some_and(|unit| unit.lost_criticals().contains(&location))
            {
                previous = None;
                continue;
            }
            let Some((part, is_weapon, is_payload)) = raw_part(raw) else {
                previous = None;
                continue;
            };
            if payload_only && !is_payload {
                previous = None;
                continue;
            }
            if is_weapon && previous == Some(part) {
                continue;
            }
            *quantities.entry(part).or_default() += 1;
            previous = Some(part);
        }
    }
    Ok(quantities.into_iter().collect())
}

fn raw_part(raw: &super::CriticalDefinition) -> Option<(InspectionPart, bool, bool)> {
    inspection_template_part(&raw.equipment)
}

/// Ordered TIC membership without requiring cockpit authority.
pub fn inspect_unit_tic(unit: &BattleUnit, group: usize) -> Result<Vec<usize>> {
    unit.tics
        .0
        .get(group)
        .map(|set| set.iter().copied().collect())
        .context("TIC number is out of bounds")
}

/// Current section material condition.
pub fn inspect_section_condition(unit: &BattleUnit, section: BattleSection) -> &'static str {
    if unit
        .sections()
        .get(&section)
        .is_none_or(|state| state.internal == 0)
    {
        "destroyed"
    } else if unit.flooded_sections.contains(&section) {
        "flooded"
    } else {
        "operational"
    }
}

/// C-compatible nominal engine output for a template.
pub fn inspection_engine_rating(template: &BattleTemplate) -> Result<u32> {
    let tons = super::administrative_template_tonnage(&template.attributes, template.tons);
    anyhow::ensure!(
        template.max_speed.is_finite() && template.max_speed >= 0.0,
        "Invalid engine inputs"
    );
    if tons == 0 {
        return Ok(0);
    }
    let walking = ((2.0 * template.max_speed as f32 / 10.75) / 3.0).round();
    let rating = f64::from(walking) * f64::from(tons);
    anyhow::ensure!(
        rating.is_finite() && rating <= f64::from(i32::MAX),
        "Engine rating is out of bounds"
    );
    Ok(rating as u32)
}

/// C-facing vehicle rating and suspension using the effective wide tonnage.
pub fn inspection_vehicle_engine_rating(
    template: &super::BattleVehicleTemplate,
) -> Result<(u32, u16)> {
    let tons = super::administrative_template_tonnage(&template.attributes, template.tons);
    let movement = super::administrative_template_movement(&template.attributes, template.movement);
    inspection_vehicle_engine_values(tons, movement, template.max_speed)
}

/// Calculate the native engine row from class-neutral template scalars.
///
/// The native loader caches this rating after parsing, so every unit class uses
/// the same single-precision rounding before applying its movement suspension.
pub fn inspect_raw_template_engine(template: &super::RawTemplate) -> (i32, i32) {
    let speed = template.max_speed as f32;
    let movement_points = ((2.0_f32 * speed / 10.75_f32) / 3.0_f32).round() as i32;
    let rating = movement_points.saturating_mul(template.tons);
    let tons = template.tons;
    let suspension_factor = match template.movement {
        super::RawMovement::Tracked => 0,
        super::RawMovement::Wheeled => 20,
        super::RawMovement::Foil => match tons {
            ..=10 => 60,
            11..=20 => 105,
            21..=30 => 150,
            31..=40 => 195,
            41..=50 => 255,
            51..=60 => 300,
            61..=70 => 345,
            71..=80 => 390,
            81..=90 => 435,
            _ => 480,
        },
        super::RawMovement::Hover => match tons {
            ..=10 => 40,
            11..=20 => 85,
            21..=30 => 130,
            31..=40 => 175,
            _ => 235,
        },
        super::RawMovement::Hull | super::RawMovement::Submarine => 30,
        super::RawMovement::Vtol => match tons {
            ..=10 => 50,
            11..=20 => 95,
            _ => 140,
        },
        _ => 0,
    };
    (rating, suspension_factor)
}

fn raw_technology_codes(template: &super::RawTemplate) -> Result<std::collections::BTreeSet<i32>> {
    Ok(inspect_raw_template_technologies(template)?
        .into_iter()
        .map(|technology| technology.code)
        .collect())
}

fn raw_normalized_jump_speed(
    template: &super::RawTemplate,
    technology: &std::collections::BTreeSet<i32>,
) -> f32 {
    if template.class != super::RawUnitClass::Mech {
        return template.jump_speed as f32;
    }
    let mut jets = template
        .sections
        .values()
        .flat_map(|section| section.criticals.values())
        .filter(|critical| critical.equipment.eq_ignore_ascii_case("JumpJet"))
        .count() as i32;
    if technology.contains(&38) {
        jets /= 2;
        let maximum = (template.max_speed as f32 * 0.093_023_3_f32) as i32;
        jets = jets.min(maximum);
    } else {
        // Native integer `(2 / 3)` makes the admitted standard-jet maximum zero.
        jets = 0;
    }
    10.75_f32 * jets.max(0) as f32
}

fn raw_weapon_heat_and_value(part_id: i32) -> Option<(i32, i32)> {
    if let Some(weapon) = super::BattleWeapon::from_part_id(part_id) {
        return Some((
            i32::from(weapon.profile().heat),
            i32::from(weapon.battle_value()),
        ));
    }
    match part_id {
        171 => Some((0, 5)),
        172 => Some((0, 7)),
        173 => Some((0, 9)),
        174 => Some((0, 8)),
        175 => Some((1, 9)),
        176 => Some((1, 5)),
        177 => Some((1, 12)),
        178 => Some((1, 22)),
        _ => None,
    }
}

/// Calculate pristine native Battle Value from class-neutral template material.
pub fn inspect_raw_template_battle_value(
    template: &super::RawTemplate,
) -> Result<super::BattleValue> {
    let technology = raw_technology_codes(template)?;
    let armor = inspect_raw_template_armor(template, None)?;
    let mut defense = (armor.armor.1 + armor.rear_armor.1) as f32 * 2.5_f32;
    let engine_factor = if template.class != super::RawUnitClass::Mech {
        1.0_f32
    } else if technology.contains(&24) || (technology.contains(&5) && technology.contains(&21)) {
        0.75_f32
    } else if technology.contains(&21) || technology.contains(&25) {
        0.5_f32
    } else {
        1.0_f32
    };
    defense += armor.internal.1 as f32 * 1.5_f32 * engine_factor;
    if template.class == super::RawUnitClass::Mech {
        defense += template.tons as f32 * if technology.contains(&53) { 1.0 } else { 0.5 };
    }

    let discount = match template.movement {
        super::RawMovement::Tracked => 0.1_f32,
        super::RawMovement::Wheeled => 0.2_f32,
        super::RawMovement::Hover | super::RawMovement::Vtol => 0.3_f32,
        super::RawMovement::Hull | super::RawMovement::Foil | super::RawMovement::Submarine => {
            0.4_f32
        }
        _ => 0.0_f32,
    };
    defense -= defense * discount;

    let jump = (raw_normalized_jump_speed(template, &technology) / 10.75_f32) as i32;
    let mut running = (template.max_speed as f32 / 10.75_f32) as i32;
    if technology.contains(&0) {
        running = (((template.max_speed as f32 / 1.5_f32 / 10.75_f32).round_ties_even() + 1.0_f32)
            * 1.5_f32) as i32;
    }
    if technology.contains(&4) && technology.contains(&37) {
        running = ((running * 2 / 3) as f32 * 2.5_f32) as i32;
    }
    if technology.contains(&4) || technology.contains(&37) {
        running = (running * 2 / 3) * 2;
    }
    let mut movement = match running {
        ..=2 => 0,
        3..=4 => 1,
        5..=6 => 2,
        7..=9 => 3,
        10..=17 => 4,
        18..=24 => 5,
        _ => 6,
    };
    if matches!(
        template.class,
        super::RawUnitClass::BattleSuit
            | super::RawUnitClass::Vtol
            | super::RawUnitClass::AeroFighter
    ) {
        movement += 1;
    }
    movement += i32::from(jump > running);
    if technology.contains(&31) {
        movement += 2;
    }
    defense += defense * movement as f32 * 0.1_f32;
    defense = ((defense * 100.0_f32).round() / 100.0_f32).max(0.0);

    let heat_efficiency = 6 + template.heat_sinks - jump.max(2);
    let mut weapons: Vec<_> = inspect_raw_template_weapons(template)?
        .into_iter()
        .filter_map(|weapon| raw_weapon_heat_and_value(weapon.part.id))
        .collect();
    weapons.sort_by_key(|(_, battle_value)| *battle_value);
    let mut heat = 0;
    let mut offense = template.tons as f32;
    for (weapon_heat, battle_value) in weapons.into_iter().rev() {
        heat += weapon_heat;
        offense += if heat > heat_efficiency {
            (battle_value / 2) as f32
        } else {
            battle_value as f32
        };
    }
    Ok(super::BattleValue {
        offensive: f64::from(offense),
        defensive: f64::from(defense),
        total: f64::from(offense) + f64::from(defense),
    })
}

fn compose_raw_template(
    mut raw: super::RawTemplate,
    class: super::RawUnitClass,
    movement: super::RawMovement,
    physical: impl IntoIterator<Item = (usize, SectionDefinition)>,
    extra: Option<&std::collections::BTreeMap<usize, super::AdministrativeRawSection>>,
) -> super::RawTemplate {
    raw.class = class;
    raw.movement = movement;
    raw.attributes.insert("type".into(), class.name().into());
    raw.attributes
        .insert("move_type".into(), movement.name().into());
    raw.sections = super::RawSectionCode::for_unit(class, movement)
        .iter()
        .copied()
        .map(|section| (section, SectionDefinition::default()))
        .collect();
    for (ordinal, definition) in physical {
        let Some(section) = super::RawSectionCode::from_ordinal(class, movement, ordinal) else {
            break;
        };
        raw.sections.insert(section, definition);
    }
    if let Some(extra) = extra {
        for (&ordinal, state) in extra {
            if let Some(section) = super::RawSectionCode::from_ordinal(class, movement, ordinal) {
                raw.sections.insert(section, state.definition.clone());
            }
        }
    }
    raw
}

/// Compose the current physical Mech definition with its class-neutral administrative overlay.
pub fn compose_unit_raw_inspection(unit: &BattleUnit) -> super::RawTemplate {
    let base = super::RawTemplate::from(unit.definition());
    let overlay = unit.administrative_raw();
    let class = overlay.map_or(base.class, |raw| raw.class);
    let movement = overlay.map_or(base.movement, |raw| raw.movement);
    compose_raw_template(
        base,
        class,
        movement,
        BattleSection::ALL
            .into_iter()
            .enumerate()
            .map(|(ordinal, section)| (ordinal, unit.definition().sections[&section].clone())),
        overlay.map(|raw| &raw.extra_sections),
    )
}

/// Compose the current physical vehicle definition with its class-neutral administrative overlay.
pub fn compose_vehicle_raw_inspection(unit: &super::BattleVehicle) -> super::RawTemplate {
    let base = super::RawTemplate::from(unit.definition());
    let overlay = unit.administrative_raw();
    let class = overlay.map_or(base.class, |raw| raw.class);
    let movement = overlay.map_or(base.movement, |raw| raw.movement);
    const PHYSICAL: [super::BattleVehicleSection; 6] = [
        super::BattleVehicleSection::Left,
        super::BattleVehicleSection::Right,
        super::BattleVehicleSection::Front,
        super::BattleVehicleSection::Rear,
        super::BattleVehicleSection::Turret,
        super::BattleVehicleSection::Rotor,
    ];
    compose_raw_template(
        base,
        class,
        movement,
        PHYSICAL
            .into_iter()
            .enumerate()
            .filter_map(|(ordinal, section)| {
                unit.definition()
                    .sections
                    .get(&section)
                    .cloned()
                    .map(|definition| (ordinal, definition))
            }),
        overlay.map(|raw| &raw.extra_sections),
    )
}

fn accumulate_armor(target: &mut InspectionArmor, source: InspectionArmor) {
    target.armor.0 += source.armor.0;
    target.armor.1 += source.armor.1;
    target.internal.0 += source.internal.0;
    target.internal.1 += source.internal.1;
    target.rear_armor.0 += source.rear_armor.0;
    target.rear_armor.1 += source.rear_armor.1;
}

fn extra_armor(
    code: super::RawSectionCode,
    section: &super::AdministrativeRawSection,
) -> InspectionArmor {
    InspectionArmor {
        section: Some(code as i32),
        armor: (
            u32::from(section.current.armor),
            u32::from(section.definition.armor),
        ),
        internal: (
            u32::from(section.current.internal),
            u32::from(section.definition.internal),
        ),
        rear_armor: (
            u32::from(section.current.rear),
            u32::from(section.definition.rear),
        ),
    }
}

/// Inspect live protection through the administrative class's physical ordinals.
pub fn inspect_composed_unit_armor(
    unit: &BattleUnit,
    selected: Option<super::RawSectionCode>,
) -> Result<InspectionArmor> {
    let raw = compose_unit_raw_inspection(unit);
    let mut result = InspectionArmor {
        section: selected.map(|section| section as i32),
        armor: (0, 0),
        internal: (0, 0),
        rear_armor: (0, 0),
    };
    for (ordinal, code) in super::RawSectionCode::for_unit(raw.class, raw.movement)
        .iter()
        .copied()
        .enumerate()
    {
        if selected.is_some_and(|selected| selected != code) {
            continue;
        }
        if let Some(section) = BattleSection::ALL.get(ordinal).copied() {
            let mut row = inspect_unit_armor(unit, Some(section))?;
            row.section = Some(code as i32);
            accumulate_armor(&mut result, row);
        } else if let Some(section) = unit
            .administrative_raw()
            .and_then(|raw| raw.extra_sections.get(&ordinal))
        {
            accumulate_armor(&mut result, extra_armor(code, section));
        }
    }
    Ok(result)
}

const VEHICLE_PHYSICAL: [super::BattleVehicleSection; 6] = [
    super::BattleVehicleSection::Left,
    super::BattleVehicleSection::Right,
    super::BattleVehicleSection::Front,
    super::BattleVehicleSection::Rear,
    super::BattleVehicleSection::Turret,
    super::BattleVehicleSection::Rotor,
];

/// Inspect live vehicle protection through the administrative class's physical ordinals.
pub fn inspect_composed_vehicle_armor(
    unit: &super::BattleVehicle,
    selected: Option<super::RawSectionCode>,
) -> Result<InspectionArmor> {
    let raw = compose_vehicle_raw_inspection(unit);
    let mut result = InspectionArmor {
        section: selected.map(|section| section as i32),
        armor: (0, 0),
        internal: (0, 0),
        rear_armor: (0, 0),
    };
    for (ordinal, code) in super::RawSectionCode::for_unit(raw.class, raw.movement)
        .iter()
        .copied()
        .enumerate()
    {
        if selected.is_some_and(|selected| selected != code) {
            continue;
        }
        if let Some(section) = VEHICLE_PHYSICAL
            .get(ordinal)
            .filter(|section| unit.definition().sections.contains_key(section))
            .copied()
        {
            let mut row = inspect_vehicle_armor(unit, Some(section))?;
            row.section = Some(code as i32);
            accumulate_armor(&mut result, row);
        } else if let Some(section) = unit
            .administrative_raw()
            .and_then(|raw| raw.extra_sections.get(&ordinal))
        {
            accumulate_armor(&mut result, extra_armor(code, section));
        }
    }
    Ok(result)
}

/// Calculate the vehicle engine projection from already-resolved administrative fields.
pub fn inspection_vehicle_engine_values(
    tons: u32,
    movement: super::BattleVehicleMovement,
    maximum_speed: f64,
) -> Result<(u32, u16)> {
    anyhow::ensure!(
        tons > 0 && maximum_speed.is_finite() && maximum_speed >= 0.0,
        "Invalid engine inputs"
    );
    let walking = ((2.0 * maximum_speed as f32 / 10.75) / 3.0).round();
    let rating = f64::from(walking) * f64::from(tons);
    anyhow::ensure!(
        rating.is_finite() && rating <= f64::from(i32::MAX),
        "Engine rating is out of bounds"
    );
    let suspension = match movement {
        super::BattleVehicleMovement::Tracked | super::BattleVehicleMovement::Stationary => 0,
        super::BattleVehicleMovement::Wheeled => 20,
        super::BattleVehicleMovement::Vtol => match tons {
            0..=10 => 50,
            11..=20 => 95,
            _ => 140,
        },
        super::BattleVehicleMovement::Hover => match tons {
            0..=10 => 40,
            11..=20 => 85,
            21..=30 => 130,
            31..=40 => 175,
            _ => 235,
        },
    };
    Ok((rating as u32, suspension))
}

/// Stable C-facing section identity for a ground vehicle or VTOL.
pub fn inspection_vehicle_section_code(section: super::BattleVehicleSection) -> i32 {
    match section {
        super::BattleVehicleSection::Left => 20,
        super::BattleVehicleSection::Right => 21,
        super::BattleVehicleSection::Front => 22,
        super::BattleVehicleSection::Rear => 23,
        super::BattleVehicleSection::Turret => 24,
        super::BattleVehicleSection::Rotor => 25,
    }
}

/// Decode a C-facing vehicle section identity.
pub fn inspection_vehicle_section(code: i32) -> Result<super::BattleVehicleSection> {
    [
        super::BattleVehicleSection::Left,
        super::BattleVehicleSection::Right,
        super::BattleVehicleSection::Front,
        super::BattleVehicleSection::Rear,
        super::BattleVehicleSection::Turret,
        super::BattleVehicleSection::Rotor,
    ]
    .into_iter()
    .find(|section| inspection_vehicle_section_code(*section) == code)
    .context("section is not valid for this unit")
}

/// Decode a C-facing section using the native class section count.
/// Ground vehicles expose five faces and VTOLs expose all six even when an
/// omitted template face contains only the native zero/default state.
pub fn inspection_vehicle_section_for(
    template: &super::BattleVehicleTemplate,
    code: i32,
) -> Result<super::BattleVehicleSection> {
    let section = inspection_vehicle_section(code)?;
    anyhow::ensure!(
        section != super::BattleVehicleSection::Rotor
            || template.movement == super::BattleVehicleMovement::Vtol,
        "section is not valid for this unit"
    );
    Ok(section)
}

/// Inspect current vehicle protection.
pub fn inspect_vehicle_armor(
    unit: &super::BattleVehicle,
    section: Option<super::BattleVehicleSection>,
) -> Result<InspectionArmor> {
    let mut row = InspectionArmor {
        section: section.map(inspection_vehicle_section_code),
        armor: (0, 0),
        internal: (0, 0),
        rear_armor: (0, 0),
    };
    for (&current, state) in unit.sections() {
        if section.is_some_and(|s| s != current) {
            continue;
        }
        let original = &unit.definition().sections[&current];
        row.armor.0 += u32::from(state.armor);
        row.armor.1 += u32::from(original.armor);
        row.internal.0 += u32::from(state.internal);
        row.internal.1 += u32::from(original.internal);
        row.rear_armor.0 += u32::from(state.rear);
        row.rear_armor.1 += u32::from(original.rear);
    }
    Ok(row)
}

/// Inspect pristine vehicle or VTOL protection.
pub fn inspect_vehicle_template_armor(
    template: &super::BattleVehicleTemplate,
    section: Option<super::BattleVehicleSection>,
) -> Result<InspectionArmor> {
    let mut row = InspectionArmor {
        section: section.map(inspection_vehicle_section_code),
        armor: (0, 0),
        internal: (0, 0),
        rear_armor: (0, 0),
    };
    for (&current, state) in &template.sections {
        if section.is_some_and(|s| s != current) {
            continue;
        }
        row.armor.0 += u32::from(state.armor);
        row.armor.1 += u32::from(state.armor);
        row.internal.0 += u32::from(state.internal);
        row.internal.1 += u32::from(state.internal);
        row.rear_armor.0 += u32::from(state.rear);
        row.rear_armor.1 += u32::from(state.rear);
    }
    Ok(row)
}

/// Inspect pristine vehicle weapons in game-number order.
pub fn inspect_vehicle_template_weapons(
    template: &super::BattleVehicleTemplate,
) -> Result<Vec<InspectionWeapon>> {
    let compatible = inspection_compatible_vehicle_template(template);
    let loadout = super::BattleVehicleLoadout::resolve(&compatible)?;
    loadout
        .weapons
        .iter()
        .enumerate()
        .map(|(number, mount)| {
            let first = mount.criticals[0];
            Ok(InspectionWeapon {
                number,
                section: inspection_vehicle_section_code(first.section),
                first_slot: first.slot + 1,
                part: InspectionPart {
                    id: mount.weapon.part_id(),
                },
                slot_count: mount.criticals.len() as u8,
                recycle: 0,
                recycle_time: mount.weapon.profile().recycle_seconds,
                operational: true,
            })
        })
        .collect()
}

/// Inspect pristine vehicle critical slots through an isolated constructed vehicle.
pub fn inspect_vehicle_template_criticals(
    template: &super::BattleVehicleTemplate,
    section: super::BattleVehicleSection,
) -> Result<Vec<InspectionCritical>> {
    let unit = super::BattleVehicle::new(inspection_compatible_vehicle_template(template))?;
    inspect_vehicle_criticals(&unit, section)
}

/// Count pristine installed vehicle parts.
pub fn inspect_vehicle_template_inventory(
    template: &super::BattleVehicleTemplate,
    payload_only: bool,
) -> Result<Vec<(InspectionPart, u32)>> {
    let unit = super::BattleVehicle::new(inspection_compatible_vehicle_template(template))?;
    inspect_vehicle_inventory(&unit, payload_only)
}

/// Inspect vehicle weapons in game-number order.
pub fn inspect_vehicle_weapons(unit: &super::BattleVehicle) -> Result<Vec<InspectionWeapon>> {
    let loadout = unit.loadout()?;
    loadout
        .weapons
        .iter()
        .enumerate()
        .map(|(number, mount)| {
            let first = mount.criticals[0];
            Ok(InspectionWeapon {
                number,
                section: inspection_vehicle_section_code(first.section),
                first_slot: first.slot + 1,
                part: InspectionPart {
                    id: mount.weapon.part_id(),
                },
                slot_count: mount.criticals.len() as u8,
                recycle: unit.weapon_recycle().get(&number).copied().unwrap_or(0),
                recycle_time: mount.weapon.profile().recycle_seconds,
                operational: !unit.critical_unavailable(first),
            })
        })
        .collect()
}

/// Inspect all twelve slots of a live vehicle section.
pub fn inspect_vehicle_criticals(
    unit: &super::BattleVehicle,
    section: super::BattleVehicleSection,
) -> Result<Vec<InspectionCritical>> {
    let loadout = unit.loadout()?;
    let code = inspection_vehicle_section_code(section);
    let Some(definition) = unit.definition().sections.get(&section) else {
        return Ok((0..12)
            .map(|slot| InspectionCritical {
                section: code,
                slot: slot + 1,
                kind: "empty",
                part: None,
                operational: true,
                temporary_failure: false,
                auxiliary_data: 0,
                ammunition: None,
                fire_modes: Vec::new(),
                ammunition_modes: Vec::new(),
            })
            .collect());
    };
    let mut rows = Vec::with_capacity(12);
    for slot in 0..12_u8 {
        let location = super::VehicleCriticalLocation { section, slot };
        let raw = definition.criticals.get(&slot);
        let weapon = loadout
            .weapons
            .iter()
            .enumerate()
            .find(|(_, mount)| mount.criticals.contains(&location));
        let ammo = loadout
            .ammunition
            .iter()
            .enumerate()
            .find(|(_, bin)| bin.location == location);
        let system = loadout
            .systems
            .iter()
            .find(|part| part.location == location);
        let part = if let Some((_, mount)) = weapon {
            Some(InspectionPart {
                id: mount.weapon.part_id(),
            })
        } else if let Some((_, bin)) = ammo {
            Some(InspectionPart {
                id: bin.weapon.ammunition_part_id(),
            })
        } else if system.is_some() {
            raw.and_then(|raw| BattlePart::parse(&raw.equipment).ok())
                .map(|part| InspectionPart { id: part.part_id })
        } else {
            None
        };
        let ammunition = ammo.map(|(index, bin)| (unit.ammunition()[index], bin.capacity));
        let mut fire_modes = raw.map_or_else(Vec::new, |raw| template_fire_modes(&raw.modes));
        let mut ammunition_modes =
            raw.map_or_else(Vec::new, |raw| template_ammunition_modes(&raw.modes));
        if let Some((index, mount)) = weapon {
            fire_modes.extend(live_fire_mode(unit.fire_mode(index).unwrap_or_default()));
            ammunition_modes.extend(live_ammunition_mode(
                unit.ammunition_mode(index).unwrap_or_default(),
                mount.weapon,
            ));
        }
        if unit.lost_criticals().contains(&location) {
            fire_modes.push(1)
        }
        fire_modes.sort_unstable();
        fire_modes.dedup();
        ammunition_modes.sort_unstable();
        ammunition_modes.dedup();
        rows.push(InspectionCritical {
            section: code,
            slot: slot + 1,
            kind: if weapon.is_some() {
                "weapon"
            } else if ammo.is_some() {
                "ammunition"
            } else if part.is_some() {
                "special"
            } else {
                "empty"
            },
            part,
            operational: !fire_modes.iter().any(|mode| matches!(mode, 1 | 2 | 4)),
            temporary_failure: weapon
                .is_some_and(|(index, _)| unit.weapon_failures().contains_key(&index)),
            auxiliary_data: ammunition.map_or_else(
                || raw.and_then(|raw| raw.data.parse().ok()).unwrap_or(0),
                |value| i32::from(value.0),
            ),
            ammunition,
            fire_modes,
            ammunition_modes,
        });
    }
    Ok(rows)
}

/// Count surviving installed vehicle parts in catalogue identity order.
pub fn inspect_vehicle_inventory(
    unit: &super::BattleVehicle,
    payload_only: bool,
) -> Result<Vec<(InspectionPart, u32)>> {
    let mut quantities = BTreeMap::new();
    for section in [
        super::BattleVehicleSection::Left,
        super::BattleVehicleSection::Right,
        super::BattleVehicleSection::Front,
        super::BattleVehicleSection::Rear,
        super::BattleVehicleSection::Turret,
        super::BattleVehicleSection::Rotor,
    ] {
        let Some(definition) = unit.definition().sections.get(&section) else {
            continue;
        };
        let mut previous = None;
        for slot in 0..12_u8 {
            let location = super::VehicleCriticalLocation { section, slot };
            let Some(raw) = definition.criticals.get(&slot) else {
                previous = None;
                continue;
            };
            if template_fire_modes(&raw.modes).contains(&1)
                || unit.lost_criticals().contains(&location)
            {
                previous = None;
                continue;
            }
            let Some((part, is_weapon, is_payload)) = raw_part(raw) else {
                previous = None;
                continue;
            };
            if payload_only && !is_payload {
                previous = None;
                continue;
            }
            if is_weapon && previous == Some(part) {
                continue;
            }
            *quantities.entry(part).or_default() += 1;
            previous = Some(part);
        }
    }
    Ok(quantities.into_iter().collect())
}

/// Ordered vehicle TIC membership without cockpit authority.
pub fn inspect_vehicle_tic(unit: &super::BattleVehicle, group: usize) -> Result<Vec<usize>> {
    unit.tics
        .0
        .get(group)
        .map(|set| set.iter().copied().collect())
        .context("TIC number is out of bounds")
}

/// Current vehicle section material condition.
pub fn inspect_vehicle_section_condition(
    unit: &super::BattleVehicle,
    section: super::BattleVehicleSection,
) -> &'static str {
    if unit
        .sections()
        .get(&section)
        .is_none_or(|state| state.internal == 0)
    {
        "destroyed"
    } else if unit.flooded() {
        "flooded"
    } else {
        "operational"
    }
}

/// One configured or equipment-inferred technology flag.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InspectionTechnology {
    pub code: i32,
    pub name: &'static str,
    pub group: &'static str,
    pub source: &'static str,
}

fn configured_technology(template: &BattleTemplate, name: &str, group: &str) -> bool {
    configured_technology_attributes(&template.attributes, name, group)
}

fn configured_technology_attributes(
    attributes: &BTreeMap<String, String>,
    name: &str,
    group: &str,
) -> bool {
    let keys: &[&str] = match group {
        "primary" | "secondary" => &["specials", "specials2"],
        "infantry" => &["infantry_specials"],
        _ => return false,
    };
    keys.iter().any(|attribute| {
        attributes.get(*attribute).is_some_and(|value| {
            value
                .split_ascii_whitespace()
                .any(|item| item.eq_ignore_ascii_case(name))
        })
    })
}

fn template_system_at(
    template: &BattleTemplate,
    section: BattleSection,
    slot: u8,
) -> Option<super::BattleSystem> {
    let critical = template.sections.get(&section)?.criticals.get(&slot)?;
    super::BattleSystem::named(&critical.equipment)
}

/// Equipment-derived native technology flags produced during template loading.
pub(crate) fn inspection_template_inferred_technology(
    template: &BattleTemplate,
    code: i32,
) -> bool {
    match code {
        // The loader permits arm flipping only when both arms omit their lower
        // and hand actuators from the conventional third and fourth slots.
        6 => {
            !matches!(
                template_system_at(template, BattleSection::LeftArm, 2),
                Some(super::BattleSystem::LowerActuator)
            ) && !matches!(
                template_system_at(template, BattleSection::RightArm, 2),
                Some(super::BattleSystem::LowerActuator)
            ) && !matches!(
                template_system_at(template, BattleSection::LeftArm, 3),
                Some(super::BattleSystem::HandOrFootActuator)
            ) && !matches!(
                template_system_at(template, BattleSection::RightArm, 3),
                Some(super::BattleSystem::HandOrFootActuator)
            )
        }
        // Fewer than four center-torso engine criticals identify a compact
        // engine, including sparse but valid inspection templates.
        26 => {
            template
                .sections
                .get(&BattleSection::CenterTorso)
                .map_or(0, |section| {
                    section
                        .criticals
                        .values()
                        .filter(|critical| {
                            super::BattleSystem::named(&critical.equipment)
                                .is_some_and(|system| system == super::BattleSystem::Engine)
                        })
                        .count()
                })
                < 4
        }
        _ => false,
    }
}

/// Resolve the supported primary technology flags from authored features and installed systems.
pub fn inspect_technologies(template: &BattleTemplate) -> Result<Vec<InspectionTechnology>> {
    let mut rows = Vec::new();
    for code in 0..=66 {
        let (flag, group) = super::admin_contract::administrative_technology(code)
            .expect("complete unit technology catalogue");
        let configured = configured_technology(template, flag, group);
        let loaded = inspection_template_inferred_technology(template, code);
        if configured || loaded {
            rows.push(InspectionTechnology {
                code,
                name: flag,
                group,
                // Native template loading reconciles structural flags before
                // the Lua query snapshots the configured bitsets.
                source: "configured",
            });
        }
    }
    let loadout = BattleLoadout::resolve(&inspection_compatible_template(template))?;
    for (system, code, name) in [
        (super::BattleSystem::Masc, 4, "Masc"),
        (super::BattleSystem::C3Master, 7, "C3MasterTech"),
        (super::BattleSystem::C3Slave, 8, "C3SlaveTech"),
        (super::BattleSystem::ArtemisIv, 9, "ArtemisIV"),
        (super::BattleSystem::Ecm, 10, "ECM"),
        (super::BattleSystem::BeagleProbe, 11, "BeagleProbe"),
        (super::BattleSystem::LightProbe, 15, "LightBAP"),
        (super::BattleSystem::Supercharger, 37, "SuperCharger_Tech"),
        (
            super::BattleSystem::BloodhoundProbe,
            42,
            "BloodhoundProbe_Tech",
        ),
        (super::BattleSystem::AngelEcm, 43, "AngelECM_Tech"),
        (super::BattleSystem::Tag, 46, "TAG_Tech"),
        (super::BattleSystem::TargetingComputer, 55, "TargComp_Tech"),
    ] {
        if loadout.systems.iter().any(|part| part.system == system)
            && !rows.iter().any(|row| row.code == code)
        {
            rows.push(InspectionTechnology {
                code,
                name,
                group: "primary",
                source: "inferred",
            });
        }
    }
    rows.sort_by_key(|row| row.code);
    Ok(rows)
}

/// Resolve configured and equipment-inferred flags from class-neutral template state.
pub fn inspect_raw_template_technologies(
    template: &super::RawTemplate,
) -> Result<Vec<InspectionTechnology>> {
    let mut rows = Vec::new();
    for code in 0..=66 {
        let (flag, group) = super::admin_contract::administrative_technology(code)
            .expect("complete unit technology catalogue");
        if configured_technology_attributes(&template.attributes, flag, group) {
            rows.push(InspectionTechnology {
                code,
                name: flag,
                group,
                source: "configured",
            });
        }
    }
    if template.class == super::RawUnitClass::Mech {
        let section = |code| template.sections.get(&code);
        // The native update_specials re-derives a compact engine from the
        // center-torso installation, so cleared compact-engine flags resurface
        // as inferred; the loader-stored flippable-arms bit is not re-derived.
        let compact = section(super::RawSectionCode::CenterTorso).map_or(0, |section| {
            section
                .criticals
                .values()
                .filter(|critical| critical.equipment.eq_ignore_ascii_case("Engine"))
                .count()
        }) < 4;
        if compact && !rows.iter().any(|row| row.code == 26) {
            rows.push(InspectionTechnology {
                code: 26,
                name: "CompactEngine_Tech",
                group: "primary",
                source: "inferred",
            });
        }
    }
    let systems: Vec<_> = template
        .sections
        .values()
        .flat_map(|section| section.criticals.values())
        .filter_map(|critical| super::BattleSystem::named(&critical.equipment))
        .collect();
    for (system, code, name) in [
        (super::BattleSystem::Masc, 4, "Masc"),
        (super::BattleSystem::C3Master, 7, "C3MasterTech"),
        (super::BattleSystem::C3Slave, 8, "C3SlaveTech"),
        (super::BattleSystem::ArtemisIv, 9, "ArtemisIV"),
        (super::BattleSystem::Ecm, 10, "ECM"),
        (super::BattleSystem::BeagleProbe, 11, "BeagleProbe"),
        (super::BattleSystem::LightProbe, 15, "LightBAP"),
        (super::BattleSystem::Supercharger, 37, "SuperCharger_Tech"),
        (
            super::BattleSystem::BloodhoundProbe,
            42,
            "BloodhoundProbe_Tech",
        ),
        (super::BattleSystem::AngelEcm, 43, "AngelECM_Tech"),
        (super::BattleSystem::Tag, 46, "TAG_Tech"),
        (super::BattleSystem::TargetingComputer, 55, "TargComp_Tech"),
    ] {
        if systems.contains(&system) && !rows.iter().any(|row| row.code == code) {
            rows.push(InspectionTechnology {
                code,
                name,
                group: "primary",
                source: "inferred",
            });
        }
    }
    rows.sort_by_key(|row| row.code);
    Ok(rows)
}

/// Resolve configured and equipment-inferred technology flags for vehicles and VTOLs.
pub fn inspect_vehicle_technologies(
    template: &super::BattleVehicleTemplate,
) -> Result<Vec<InspectionTechnology>> {
    let specials = template
        .attributes
        .get("specials")
        .map(String::as_str)
        .unwrap_or("");
    let mut rows = Vec::new();
    for code in 0..=66 {
        let (flag, group) = super::admin_contract::administrative_technology(code)
            .expect("complete unit technology catalogue");
        if specials
            .split_ascii_whitespace()
            .any(|item| item.eq_ignore_ascii_case(flag))
        {
            rows.push(InspectionTechnology {
                code,
                name: flag,
                group,
                source: "configured",
            });
        }
    }
    let compatible = inspection_compatible_vehicle_template(template);
    let loadout = super::BattleVehicleLoadout::resolve(&compatible)?;
    for (system, code, name) in [
        (super::BattleSystem::C3i, 36, "C3I_Tech"),
        (super::BattleSystem::Ecm, 10, "ECM"),
        (super::BattleSystem::BeagleProbe, 11, "BeagleProbe"),
        (
            super::BattleSystem::BloodhoundProbe,
            42,
            "BloodhoundProbe_Tech",
        ),
        (super::BattleSystem::AngelEcm, 43, "AngelECM_Tech"),
        (super::BattleSystem::Tag, 46, "TAG_Tech"),
        (super::BattleSystem::TargetingComputer, 55, "TargComp_Tech"),
    ] {
        if loadout.systems.iter().any(|part| part.system == system)
            && !rows.iter().any(|row| row.code == code)
        {
            rows.push(InspectionTechnology {
                code,
                name,
                group: "primary",
                source: "inferred",
            });
        }
    }
    rows.sort_by_key(|row| row.code);
    Ok(rows)
}

fn detached_template_world(
    world: &crate::World,
    template: &BattleTemplate,
) -> Result<(crate::World, crate::ObjectId)> {
    let mut detached = world.clone();
    let id = insert_detached_object(&mut detached, template.name.clone())?;
    let unit = super::BattleUnit::from_contract_template(inspection_compatible_template(template))?;
    detached.btech.units.insert(id, unit.identity());
    detached.btech.constructed.insert(id, unit);
    std::sync::Arc::make_mut(&mut detached.btech.registrations).insert(id, "MECH".into());
    Ok((detached, id))
}

/// Render an immutable template status through isolated contract construction.
pub fn inspect_template_status_text(
    world: &crate::World,
    template: &BattleTemplate,
) -> Result<String> {
    let (detached, id) = detached_template_world(world, template)?;
    Ok(native_status_controls(
        super::unit_status(&detached, id, "R")?.replace("ID:[??]", "ID:[]"),
    ))
}

/// Render immutable weapon specifications through isolated contract construction.
pub fn inspect_template_weapon_text(
    world: &crate::World,
    template: &BattleTemplate,
    extended: bool,
) -> Result<String> {
    let (detached, id) = detached_template_world(world, template)?;
    super::weapon_specification_text(&detached, id, extended).map(native_menu_controls)
}

/// Render immutable critical status through isolated contract construction.
pub fn inspect_template_critical_text(
    world: &crate::World,
    template: &BattleTemplate,
    section: BattleSection,
) -> Result<String> {
    let (detached, id) = detached_template_world(world, template)?;
    // The C binding converts the typed section back through the command's
    // whitespace parser. Its `Left Arm`/`Right Arm` labels consequently select
    // the corresponding leg, an observable compatibility quirk.
    let command_section = match section {
        BattleSection::LeftArm => BattleSection::LeftLeg,
        BattleSection::RightArm => BattleSection::RightLeg,
        section => section,
    };
    decorated_critical_status(&detached, id, command_section.name()).map(native_menu_controls)
}

fn native_status_controls(source: String) -> String {
    let source = source.replace(
        "Temp:[fg=black bold] ",
        "Temp:[fg=black bold] [fg=black bold]",
    );
    let lines: Vec<_> = source.split("\r\n").collect();
    lines
        .iter()
        .enumerate()
        .map(|(index, line)| {
            let reset = lines.get(index + 1).is_some_and(|next| *next == " ")
                || (line.contains('[')
                    && (line.starts_with("Speed:")
                        || line.starts_with("Heading:")
                        || line.starts_with("Temp:")
                        || line.starts_with("LARM:")
                        || line.starts_with("AdvTech:")));
            if reset {
                format!("{line}[reset]")
            } else {
                (*line).to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn native_menu_controls(source: String) -> String {
    let last = source.split("\r\n").count().saturating_sub(1);
    source
        .split("\r\n")
        .enumerate()
        .map(|(index, line)| {
            if index != last && (line.contains("[reset]") || line.contains("Weapon Name")) {
                format!("{line}[reset]")
            } else {
                line.to_owned()
            }
        })
        .collect::<Vec<_>>()
        .join("\r\n")
}

fn decorated_critical_status(
    world: &crate::World,
    id: crate::ObjectId,
    section: &str,
) -> Result<String> {
    let report = super::critical_status(world, id, section)?;
    let mut source = report.split("\r\n");
    let heading = source.next().unwrap_or_default();
    let mut lines = vec![
        super::menu::rule(),
        super::menu::cell(heading, 78, true),
        super::menu::rule(),
    ];
    for row in source {
        let (left, right) = row.split_once(" | ").unwrap_or((row, ""));
        lines.push(format!(
            "{}{}",
            super::menu::cell(left, 39, false),
            super::menu::cell(right, 39, false)
        ));
    }
    lines.push(super::menu::rule());
    Ok(lines.join("\r\n"))
}

fn detached_vehicle_world(
    world: &crate::World,
    template: &super::BattleVehicleTemplate,
) -> Result<(crate::World, crate::ObjectId)> {
    let mut detached = world.clone();
    let id = insert_detached_object(&mut detached, template.name.clone())?;
    super::create_vehicle(
        &mut detached,
        id,
        inspection_compatible_vehicle_template(template),
    )?;
    Ok((detached, id))
}

fn insert_detached_object(world: &mut crate::World, name: String) -> Result<crate::ObjectId> {
    let id = crate::ObjectId(
        world
            .objects
            .keys()
            .next_back()
            .map_or(0, |id| id.0.checked_add(1).unwrap_or(i64::MIN)),
    );
    anyhow::ensure!(
        id.0 != i64::MIN && !world.objects.contains_key(&id),
        "object id space is exhausted"
    );
    world.objects.insert(
        id,
        crate::Object {
            generation: Default::default(),
            pending_destroyer: None,
            id,
            name,
            kind: crate::Kind::Thing,
            location: None,
            zone: None,
            home: None,
            affiliation: None,
            destination: None,
            dropto: None,
            description: None,
            internal_description: None,
            lua_parent: "thing".to_owned(),
            flags: Default::default(),
            powers: Default::default(),
            state: Default::default(),
        },
    );
    Ok(id)
}
/// Render a vehicle template status through isolated temporary state.
pub fn inspect_vehicle_template_status_text(
    world: &crate::World,
    template: &super::BattleVehicleTemplate,
) -> Result<String> {
    let (detached, id) = detached_vehicle_world(world, template)?;
    Ok(native_status_controls(
        super::unit_status(&detached, id, "R")?
            .replace("ID:[??]", "ID:[]")
            .replace("\r\nLANDED\r\n", "\r\n"),
    ))
}
/// Render vehicle weapon specifications through isolated temporary state.
pub fn inspect_vehicle_template_weapon_text(
    world: &crate::World,
    template: &super::BattleVehicleTemplate,
    extended: bool,
) -> Result<String> {
    let (detached, id) = detached_vehicle_world(world, template)?;
    super::weapon_specification_text(&detached, id, extended).map(native_menu_controls)
}
/// Render vehicle critical status through isolated temporary state.
pub fn inspect_vehicle_template_critical_text(
    world: &crate::World,
    template: &super::BattleVehicleTemplate,
    section: super::BattleVehicleSection,
) -> Result<String> {
    let (detached, id) = detached_vehicle_world(world, template)?;
    decorated_critical_status(&detached, id, section.name()).map(native_menu_controls)
}

#[cfg(test)]
mod raw_engine_tests {
    use super::*;

    fn engine(tons: i32, movement: crate::btech::RawMovement) -> (i32, i32) {
        let mut template =
            crate::btech::RawTemplate::empty(crate::btech::RawUnitClass::Vehicle, movement);
        template.tons = tons;
        template.max_speed = 32.25;
        inspect_raw_template_engine(&template)
    }

    #[test]
    fn native_suspension_matrix_and_rating_rounding() {
        assert_eq!(engine(60, crate::btech::RawMovement::Tracked), (120, 0));
        assert_eq!(engine(60, crate::btech::RawMovement::Wheeled), (120, 20));
        assert_eq!(engine(60, crate::btech::RawMovement::Hull), (120, 30));
        assert_eq!(engine(60, crate::btech::RawMovement::Submarine), (120, 30));
        assert_eq!(engine(60, crate::btech::RawMovement::Stationary), (120, 0));
        for (tons, factor) in [
            (10, 60),
            (20, 105),
            (30, 150),
            (40, 195),
            (50, 255),
            (60, 300),
            (70, 345),
            (80, 390),
            (90, 435),
            (91, 480),
        ] {
            assert_eq!(
                engine(tons, crate::btech::RawMovement::Foil),
                (tons * 2, factor)
            );
        }
        for (tons, factor) in [(10, 40), (20, 85), (30, 130), (40, 175), (41, 235)] {
            assert_eq!(
                engine(tons, crate::btech::RawMovement::Hover),
                (tons * 2, factor)
            );
        }
        for (tons, factor) in [(10, 50), (20, 95), (21, 140)] {
            assert_eq!(
                engine(tons, crate::btech::RawMovement::Vtol),
                (tons * 2, factor)
            );
        }
    }
}
