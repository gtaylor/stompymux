//! Template inspection records: armor, criticals, weapons, engine ratings and configured
//! technologies read from a template alone, in the shapes the native inspection commands report.
use super::{
    CriticalDefinition, MechSection, MechTemplate, Part, PartKind, RawMovement, RawSectionCode,
    RawTemplate, RawUnitClass, VehicleMovement, VehicleTemplate, Weapon,
    administrative_template_movement, administrative_template_tonnage, part_catalogue,
    strip_name_prefix,
};
use anyhow::{Context, Result};
use std::collections::BTreeMap;

/// Current and original protection totals for one section or an entire unit.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InspectionArmor {
    pub section: Option<i32>,
    pub armor: (u32, u32),
    pub internal: (u32, u32),
    pub rear_armor: (u32, u32),
}

/// Inspect pristine protection for any native unit class without constructing a combat runtime.
pub fn inspect_raw_template_armor(
    template: &RawTemplate,
    section: Option<RawSectionCode>,
) -> Result<InspectionArmor> {
    let mut row = InspectionArmor {
        section: section.map(|value| value as i32),
        armor: (0, 0),
        internal: (0, 0),
        rear_armor: (0, 0),
    };
    for current in RawSectionCode::for_unit(template.class, template.movement) {
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

fn raw_canonical_mech_internal(template: &RawTemplate, section: RawSectionCode) -> Option<u16> {
    if matches!(
        template.class,
        RawUnitClass::Vehicle | RawUnitClass::Vtol | RawUnitClass::Naval
    ) {
        let authored = template.sections.get(&section)?.internal;
        if authored == 0 {
            return Some(0);
        }
        let rounded = template.tons.saturating_add(5);
        return Some((rounded.max(10) / 10).clamp(0, 255) as u16);
    }
    if template.class != RawUnitClass::Mech || template.tons == 0 {
        return None;
    }
    if section == RawSectionCode::Head {
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
        RawSectionCode::CenterTorso => values[0],
        RawSectionCode::LeftTorso | RawSectionCode::RightTorso => values[1],
        RawSectionCode::LeftArm | RawSectionCode::RightArm => values[2],
        RawSectionCode::LeftLeg | RawSectionCode::RightLeg => values[3],
        RawSectionCode::FrontLeftLeg
        | RawSectionCode::FrontRightLeg
        | RawSectionCode::RearLeftLeg
        | RawSectionCode::RearRightLeg => values[3],
        _ => return None,
    })
}

/// Return the post-load internal structure enforced by the native template checker.
pub fn inspection_canonical_mech_internal(
    template: &MechTemplate,
    section: MechSection,
) -> Option<u16> {
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
    if section == MechSection::Head {
        return Some(3);
    }
    let tons = usize::from(template.tons);
    if !(10..=100).contains(&tons) || tons % 5 != 0 {
        return None;
    }
    let values = STRUCTURE[(tons - 10) / 5];
    let quad = template.chassis().ok()?.is_leg(section);
    Some(match section {
        MechSection::CenterTorso => values[0],
        MechSection::LeftTorso | MechSection::RightTorso => values[1],
        MechSection::LeftArm | MechSection::RightArm if quad => values[3],
        MechSection::LeftArm | MechSection::RightArm => values[2],
        MechSection::LeftLeg | MechSection::RightLeg => values[3],
        MechSection::Head => unreachable!(),
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
pub fn inspection_template_part(name: &str) -> Option<(InspectionPart, bool, bool)> {
    if let Ok(weapon) = Weapon::parse(name) {
        return Some((
            InspectionPart {
                id: weapon.part_id(),
            },
            true,
            true,
        ));
    }
    if let Some(name) = strip_name_prefix(name, "Ammo_") {
        let weapon = Weapon::parse(name).ok()?;
        return Some((
            InspectionPart {
                id: weapon.ammunition_part_id(),
            },
            false,
            true,
        ));
    }
    if let Some(form) = part_catalogue().iter().find(|form| {
        form.very_long_name.eq_ignore_ascii_case(name)
            || form.long_name.eq_ignore_ascii_case(name)
            || form.short_name.eq_ignore_ascii_case(name)
    }) {
        let part = Part::from_id(form.part_id)?;
        let is_weapon = part.kind == PartKind::Weapon;
        return Some((
            InspectionPart { id: form.part_id },
            is_weapon,
            is_weapon || part.kind == PartKind::Ammunition,
        ));
    }
    let internal = (394..=445)
        .filter_map(Part::from_id)
        .find(|part| part.name.eq_ignore_ascii_case(name));
    let part =
        internal.or_else(|| Part::all().find(|part| part.name.eq_ignore_ascii_case(name)))?;
    let weapon = part.kind == PartKind::Weapon;
    Some((InspectionPart { id: part.part_id }, weapon, weapon))
}

/// Normalize native weapon names and mech internals for the combat catalogue.
pub fn inspection_compatible_template(template: &MechTemplate) -> MechTemplate {
    let mut compatible = template.clone();
    for section in MechSection::ALL {
        if let Some(internal) = inspection_canonical_mech_internal(template, section)
            && let Some(definition) = compatible.sections.get_mut(&section)
        {
            definition.internal = internal;
        }
    }
    compatible.jump_speed = inspection_normalized_jump_speed(template);
    for definition in compatible.sections.values_mut() {
        for critical in definition.criticals.values_mut() {
            if let Ok(weapon) = Weapon::parse(&critical.equipment) {
                critical.equipment = weapon.name().to_owned();
            }
        }
    }
    compatible
}

/// Jump speed after C's `do_sub_magic` reconciles authored speed with installed jets.
pub fn inspection_normalized_jump_speed(template: &MechTemplate) -> f64 {
    let improved = inspection_configured_technology(template, "ImprovedJJ_Tech", "secondary");
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
pub fn inspection_compatible_vehicle_template(template: &VehicleTemplate) -> VehicleTemplate {
    let mut compatible = template.clone();
    for definition in compatible.sections.values_mut() {
        for critical in definition.criticals.values_mut() {
            if let Ok(weapon) = Weapon::parse(&critical.equipment) {
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

pub fn inspection_fire_modes(flags: &[String]) -> Vec<i32> {
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

pub fn inspection_ammunition_modes(flags: &[String]) -> Vec<i32> {
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

/// Inspect all twelve raw slots without requiring a class-specific runtime.
pub fn inspect_raw_template_criticals(
    template: &RawTemplate,
    section: RawSectionCode,
) -> Result<Vec<InspectionCritical>> {
    let definition = template
        .sections
        .get(&section)
        .context("raw template section is missing")?;
    let mut rows = Vec::with_capacity(12);
    for slot in 0..12_u8 {
        let critical = definition.criticals.get(&slot);
        let identity = critical.and_then(inspection_raw_part);
        let part = identity.map(|value| value.0);
        let mut fire_modes =
            critical.map_or_else(Vec::new, |raw| inspection_fire_modes(&raw.modes));
        let mut ammunition_modes =
            critical.map_or_else(Vec::new, |raw| inspection_ammunition_modes(&raw.modes));
        fire_modes.sort_unstable();
        fire_modes.dedup();
        ammunition_modes.sort_unstable();
        ammunition_modes.dedup();
        let ammunition = identity
            .filter(|(part, _, _)| Part::ammunition_weapon_id(part.id).is_some())
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
                Some((part, _, _)) if Part::ammunition_weapon_id(part.id).is_some() => "ammunition",
                Some((part, _, _)) => match Part::from_id(part.id).map(|part| part.kind) {
                    Some(PartKind::Bomb) => "bomb",
                    Some(PartKind::Commodity) => "cargo",
                    Some(PartKind::Weapon) => "weapon",
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

/// Inspect raw mounted weapons in native section and slot order.
pub fn inspect_raw_template_weapons(template: &RawTemplate) -> Result<Vec<InspectionWeapon>> {
    let mut rows = Vec::new();
    for section in RawSectionCode::for_unit(template.class, template.movement) {
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
            let Some((part, true, _)) = inspection_raw_part(raw) else {
                slot += 1;
                continue;
            };
            let profile_slots =
                Weapon::from_part_id(part.id).map_or(1, |weapon| weapon.profile().critical_slots);
            let slot_count = if template.class == RawUnitClass::Mech {
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
                operational: !inspection_fire_modes(&raw.modes)
                    .iter()
                    .any(|mode| matches!(mode, 1 | 2 | 4)),
            });
            slot = slot.saturating_add(slot_count.max(1));
        }
    }
    Ok(rows)
}

fn raw_weapon_recycle_time(part_id: i32) -> u8 {
    Weapon::from_part_id(part_id).map_or_else(
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

pub fn inspection_raw_part(raw: &CriticalDefinition) -> Option<(InspectionPart, bool, bool)> {
    inspection_template_part(&raw.equipment)
}

/// C-compatible nominal engine output for a template.
pub fn inspection_engine_rating(template: &MechTemplate) -> Result<u32> {
    let tons = administrative_template_tonnage(&template.attributes, template.tons);
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
pub fn inspection_vehicle_engine_rating(template: &VehicleTemplate) -> Result<(u32, u16)> {
    let tons = administrative_template_tonnage(&template.attributes, template.tons);
    let movement = administrative_template_movement(&template.attributes, template.movement);
    inspection_vehicle_engine_values(tons, movement, template.max_speed)
}

/// Calculate the native engine row from class-neutral template scalars.
///
/// The native loader caches this rating after parsing, so every unit class uses
/// the same single-precision rounding before applying its movement suspension.
pub fn inspect_raw_template_engine(template: &RawTemplate) -> (i32, i32) {
    let speed = template.max_speed as f32;
    let movement_points = ((2.0_f32 * speed / 10.75_f32) / 3.0_f32).round() as i32;
    let rating = movement_points.saturating_mul(template.tons);
    let tons = template.tons;
    let suspension_factor = match template.movement {
        RawMovement::Tracked => 0,
        RawMovement::Wheeled => 20,
        RawMovement::Foil => match tons {
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
        RawMovement::Hover => match tons {
            ..=10 => 40,
            11..=20 => 85,
            21..=30 => 130,
            31..=40 => 175,
            _ => 235,
        },
        RawMovement::Hull | RawMovement::Submarine => 30,
        RawMovement::Vtol => match tons {
            ..=10 => 50,
            11..=20 => 95,
            _ => 140,
        },
        _ => 0,
    };
    (rating, suspension_factor)
}

/// Calculate the vehicle engine projection from already-resolved administrative fields.
pub fn inspection_vehicle_engine_values(
    tons: u32,
    movement: VehicleMovement,
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
        VehicleMovement::Tracked | VehicleMovement::Stationary => 0,
        VehicleMovement::Wheeled => 20,
        VehicleMovement::Vtol => match tons {
            0..=10 => 50,
            11..=20 => 95,
            _ => 140,
        },
        VehicleMovement::Hover => match tons {
            0..=10 => 40,
            11..=20 => 85,
            21..=30 => 130,
            31..=40 => 175,
            _ => 235,
        },
    };
    Ok((rating as u32, suspension))
}

pub fn inspection_configured_technology(template: &MechTemplate, name: &str, group: &str) -> bool {
    inspection_configured_technology_attributes(&template.attributes, name, group)
}

pub fn inspection_configured_technology_attributes(
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

#[cfg(test)]
mod tests {
    use super::*;

    fn engine(tons: i32, movement: crate::RawMovement) -> (i32, i32) {
        let mut template = crate::RawTemplate::empty(crate::RawUnitClass::Vehicle, movement);
        template.tons = tons;
        template.max_speed = 32.25;
        inspect_raw_template_engine(&template)
    }

    #[test]
    fn native_suspension_matrix_and_rating_rounding() {
        assert_eq!(engine(60, crate::RawMovement::Tracked), (120, 0));
        assert_eq!(engine(60, crate::RawMovement::Wheeled), (120, 20));
        assert_eq!(engine(60, crate::RawMovement::Hull), (120, 30));
        assert_eq!(engine(60, crate::RawMovement::Submarine), (120, 30));
        assert_eq!(engine(60, crate::RawMovement::Stationary), (120, 0));
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
            assert_eq!(engine(tons, crate::RawMovement::Foil), (tons * 2, factor));
        }
        for (tons, factor) in [(10, 40), (20, 85), (30, 130), (40, 175), (41, 235)] {
            assert_eq!(engine(tons, crate::RawMovement::Hover), (tons * 2, factor));
        }
        for (tons, factor) in [(10, 50), (20, 95), (21, 140)] {
            assert_eq!(engine(tons, crate::RawMovement::Vtol), (tons * 2, factor));
        }
    }
}
