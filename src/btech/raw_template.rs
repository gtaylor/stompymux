//! Class-neutral BattleTech template state used by administrative and inspection contracts.
use super::{
    BattleSection, BattleTemplate, BattleVehicleMovement, BattleVehicleSection,
    BattleVehicleTemplate, CriticalDefinition, SectionDefinition,
};
use super::template_document::ParsedTemplate;
use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Unit classes in the numeric order exposed by the native Lua contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum RawUnitClass {
    Mech = 0,
    Vehicle = 1,
    Vtol = 2,
    Naval = 3,
    SpheroidDropship = 4,
    AeroFighter = 5,
    MechWarrior = 6,
    AerodyneDropship = 7,
    BattleSuit = 8,
}

impl RawUnitClass {
    /// Decode the case-insensitive spelling accepted by the native loader.
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "mech" => Ok(Self::Mech),
            "vehicle" => Ok(Self::Vehicle),
            "vtol" => Ok(Self::Vtol),
            "naval" => Ok(Self::Naval),
            "spheroid_dropship" => Ok(Self::SpheroidDropship),
            "aerofighter" => Ok(Self::AeroFighter),
            "mechwarrior" => Ok(Self::MechWarrior),
            "aerodyne_dropship" => Ok(Self::AerodyneDropship),
            "battlesuit" => Ok(Self::BattleSuit),
            _ => bail!("unsupported unit template type {value}"),
        }
    }

    /// Canonical template spelling.
    pub fn name(self) -> &'static str {
        match self {
            Self::Mech => "Mech",
            Self::Vehicle => "Vehicle",
            Self::Vtol => "VTOL",
            Self::Naval => "Naval",
            Self::SpheroidDropship => "Spheroid_DropShip",
            Self::AeroFighter => "AeroFighter",
            Self::MechWarrior => "Mechwarrior",
            Self::AerodyneDropship => "Aerodyne_DropShip",
            Self::BattleSuit => "Battlesuit",
        }
    }
}

/// Movement types in the numeric order exposed by the native Lua contract.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[repr(i32)]
pub enum RawMovement {
    Biped = 0,
    Tracked = 1,
    Wheeled = 2,
    Hover = 3,
    Vtol = 4,
    Hull = 5,
    Foil = 6,
    Fly = 7,
    Quad = 8,
    Submarine = 9,
    Stationary = 10,
}

impl RawMovement {
    /// Decode the case-insensitive spelling accepted by the native loader.
    pub fn parse(value: &str) -> Result<Self> {
        match value.to_ascii_lowercase().as_str() {
            "biped" => Ok(Self::Biped),
            "track" => Ok(Self::Tracked),
            "wheel" => Ok(Self::Wheeled),
            "hover" => Ok(Self::Hover),
            "vtol" => Ok(Self::Vtol),
            "hull" => Ok(Self::Hull),
            "foil" => Ok(Self::Foil),
            "fly" => Ok(Self::Fly),
            "quad" => Ok(Self::Quad),
            "sub" => Ok(Self::Submarine),
            "none" => Ok(Self::Stationary),
            _ => bail!("unsupported movement type {value}"),
        }
    }

    /// Canonical template spelling.
    pub fn name(self) -> &'static str {
        match self {
            Self::Biped => "Biped",
            Self::Tracked => "Track",
            Self::Wheeled => "Wheel",
            Self::Hover => "Hover",
            Self::Vtol => "VTOL",
            Self::Hull => "Hull",
            Self::Foil => "Foil",
            Self::Fly => "Fly",
            Self::Quad => "Quad",
            Self::Submarine => "Sub",
            Self::Stationary => "None",
        }
    }
}

/// Class-neutral section identities in native Lua catalog order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[repr(i32)]
pub enum RawSectionCode {
    FrontLeftLeg = 0,
    FrontRightLeg = 1,
    LeftTorso = 2,
    RightTorso = 3,
    CenterTorso = 4,
    RearLeftLeg = 5,
    RearRightLeg = 6,
    Head = 7,
    LeftArm = 8,
    RightArm = 9,
    LeftLeg = 10,
    RightLeg = 11,
    Suit1 = 12,
    Suit2 = 13,
    Suit3 = 14,
    Suit4 = 15,
    Suit5 = 16,
    Suit6 = 17,
    Suit7 = 18,
    Suit8 = 19,
    LeftSide = 20,
    RightSide = 21,
    FrontSide = 22,
    AftSide = 23,
    Turret = 24,
    Rotor = 25,
    Nose = 26,
    LeftWing = 27,
    RightWing = 28,
    LeftRearWing = 29,
    RightRearWing = 30,
    Aft = 31,
    FrontRightSide = 32,
    FrontLeftSide = 33,
    RearLeftSide = 34,
    RearRightSide = 35,
}

const QUAD: [RawSectionCode; 8] = [
    RawSectionCode::FrontLeftLeg,
    RawSectionCode::FrontRightLeg,
    RawSectionCode::LeftTorso,
    RawSectionCode::RightTorso,
    RawSectionCode::CenterTorso,
    RawSectionCode::RearLeftLeg,
    RawSectionCode::RearRightLeg,
    RawSectionCode::Head,
];
const MECH: [RawSectionCode; 8] = [
    RawSectionCode::LeftArm,
    RawSectionCode::RightArm,
    RawSectionCode::LeftTorso,
    RawSectionCode::RightTorso,
    RawSectionCode::CenterTorso,
    RawSectionCode::LeftLeg,
    RawSectionCode::RightLeg,
    RawSectionCode::Head,
];
const SUITS: [RawSectionCode; 8] = [
    RawSectionCode::Suit1,
    RawSectionCode::Suit2,
    RawSectionCode::Suit3,
    RawSectionCode::Suit4,
    RawSectionCode::Suit5,
    RawSectionCode::Suit6,
    RawSectionCode::Suit7,
    RawSectionCode::Suit8,
];
const GROUND: [RawSectionCode; 5] = [
    RawSectionCode::LeftSide,
    RawSectionCode::RightSide,
    RawSectionCode::FrontSide,
    RawSectionCode::AftSide,
    RawSectionCode::Turret,
];
const VTOL: [RawSectionCode; 6] = [
    RawSectionCode::LeftSide,
    RawSectionCode::RightSide,
    RawSectionCode::FrontSide,
    RawSectionCode::AftSide,
    RawSectionCode::Turret,
    RawSectionCode::Rotor,
];
const AERO: [RawSectionCode; 4] = [
    RawSectionCode::Nose,
    RawSectionCode::LeftWing,
    RawSectionCode::RightWing,
    RawSectionCode::AftSide,
];
const AERODYNE: [RawSectionCode; 6] = [
    RawSectionCode::RightWing,
    RawSectionCode::LeftWing,
    RawSectionCode::LeftRearWing,
    RawSectionCode::RightRearWing,
    RawSectionCode::Aft,
    RawSectionCode::Nose,
];
const SPHEROID: [RawSectionCode; 6] = [
    RawSectionCode::FrontRightSide,
    RawSectionCode::FrontLeftSide,
    RawSectionCode::RearLeftSide,
    RawSectionCode::RearRightSide,
    RawSectionCode::Aft,
    RawSectionCode::Nose,
];

impl RawSectionCode {
    /// Ordered sections exposed for one class and movement combination.
    pub fn for_unit(class: RawUnitClass, movement: RawMovement) -> &'static [Self] {
        match class {
            RawUnitClass::BattleSuit => &SUITS,
            RawUnitClass::Mech | RawUnitClass::MechWarrior if movement == RawMovement::Quad => {
                &QUAD
            }
            RawUnitClass::Mech | RawUnitClass::MechWarrior => &MECH,
            RawUnitClass::Vehicle | RawUnitClass::Naval => &GROUND,
            RawUnitClass::Vtol => &VTOL,
            RawUnitClass::AeroFighter => &AERO,
            RawUnitClass::AerodyneDropship => &AERODYNE,
            RawUnitClass::SpheroidDropship => &SPHEROID,
        }
    }

    /// Resolve one native physical section ordinal for a class.
    pub fn from_ordinal(
        class: RawUnitClass,
        movement: RawMovement,
        ordinal: usize,
    ) -> Option<Self> {
        Self::for_unit(class, movement).get(ordinal).copied()
    }

    /// Resolve this identity back to its physical section ordinal for a class.
    pub fn ordinal(self, class: RawUnitClass, movement: RawMovement) -> Option<usize> {
        Self::for_unit(class, movement)
            .iter()
            .position(|section| *section == self)
    }

    /// Native display and template heading with spaces.
    pub fn name(self) -> &'static str {
        match self {
            Self::FrontLeftLeg => "Front Left Leg",
            Self::FrontRightLeg => "Front Right Leg",
            Self::LeftTorso => "Left Torso",
            Self::RightTorso => "Right Torso",
            Self::CenterTorso => "Center Torso",
            Self::RearLeftLeg => "Rear Left Leg",
            Self::RearRightLeg => "Rear Right Leg",
            Self::Head => "Head",
            Self::LeftArm => "Left Arm",
            Self::RightArm => "Right Arm",
            Self::LeftLeg => "Left Leg",
            Self::RightLeg => "Right Leg",
            Self::Suit1 => "Suit 1",
            Self::Suit2 => "Suit 2",
            Self::Suit3 => "Suit 3",
            Self::Suit4 => "Suit 4",
            Self::Suit5 => "Suit 5",
            Self::Suit6 => "Suit 6",
            Self::Suit7 => "Suit 7",
            Self::Suit8 => "Suit 8",
            Self::LeftSide => "Left Side",
            Self::RightSide => "Right Side",
            Self::FrontSide => "Front Side",
            Self::AftSide => "Aft Side",
            Self::Turret => "Turret",
            Self::Rotor => "Rotor",
            Self::Nose => "Nose",
            Self::LeftWing => "Left Wing",
            Self::RightWing => "Right Wing",
            Self::LeftRearWing => "Left Rear Wing",
            Self::RightRearWing => "Right Rear Wing",
            Self::Aft => "Aft",
            Self::FrontRightSide => "Front Right Side",
            Self::FrontLeftSide => "Front Left Side",
            Self::RearLeftSide => "Rear Left Side",
            Self::RearRightSide => "Rear Right Side",
        }
    }

    /// Decode an exact class-local template heading after underscore normalization.
    pub fn parse_template_heading(
        class: RawUnitClass,
        movement: RawMovement,
        heading: &str,
    ) -> Result<Self> {
        let heading = heading.replace('_', " ");
        Self::for_unit(class, movement)
            .iter()
            .copied()
            .find(|section| section.name().eq_ignore_ascii_case(&heading))
            .with_context(|| format!("unsupported section {heading}"))
    }
}

/// A normalized, class-neutral template without a combat runtime prerequisite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawTemplate {
    pub name: String,
    pub reference: String,
    pub class: RawUnitClass,
    pub movement: RawMovement,
    pub tons: i32,
    pub max_speed: f64,
    pub jump_speed: f64,
    pub heat_sinks: i32,
    pub sections: BTreeMap<RawSectionCode, SectionDefinition>,
    pub attributes: BTreeMap<String, String>,
}

impl RawTemplate {
    /// Construct the native zero state with every section valid for the class.
    pub fn empty(class: RawUnitClass, movement: RawMovement) -> Self {
        Self {
            name: String::new(),
            reference: String::new(),
            class,
            movement,
            tons: 0,
            max_speed: 0.0,
            jump_speed: 0.0,
            heat_sinks: 0,
            sections: RawSectionCode::for_unit(class, movement)
                .iter()
                .copied()
                .map(|section| (section, SectionDefinition::default()))
                .collect(),
            attributes: BTreeMap::from([
                ("type".into(), class.name().into()),
                ("move_type".into(), movement.name().into()),
            ]),
        }
    }

    /// Decode a TOML template document whose file stem is `reference` into class-neutral native state.
    pub fn parse(reference: &str, source: &str) -> Result<Self> {
        Self::from_parsed(ParsedTemplate::parse(reference, source)?)
    }

    /// Place decoded sections by class anatomy, narrowing values to native storage.
    pub(super) fn from_parsed(parsed: ParsedTemplate) -> Result<Self> {
        let ParsedTemplate { fields, sections } = parsed;
        let class = fields
            .get("type")
            .map(|value| RawUnitClass::parse(value))
            .transpose()?
            .unwrap_or(RawUnitClass::Mech);
        let movement = fields
            .get("move_type")
            .map(|value| RawMovement::parse(value))
            .transpose()?
            .unwrap_or(RawMovement::Biped);
        let mut layouts: BTreeMap<_, _> = RawSectionCode::for_unit(class, movement)
            .iter()
            .map(|section| (*section, SectionDefinition::default()))
            .collect();
        for (heading, mut layout) in sections {
            let section = RawSectionCode::parse_template_heading(class, movement, &heading)?;
            layout.armor = layout.armor.min(255);
            layout.internal = layout.internal.min(255);
            layout.rear = layout.rear.min(255);
            layouts.insert(section, layout);
        }
        let heat_sinks = loaded_heat_sinks(&fields)?;
        Ok(Self {
            name: fields.get("name").cloned().unwrap_or_default(),
            reference: fields.get("reference").cloned().unwrap_or_default(),
            class,
            movement,
            tons: parse_i32_field(&fields, "tons")?,
            max_speed: parse_f32_field(&fields, "max_speed")?,
            jump_speed: parse_f32_field(&fields, "jump_speed")?,
            heat_sinks,
            sections: layouts,
            attributes: fields,
        })
    }

    /// Decode the brace-delimited syntax used before TOML documents.
    pub(super) fn parse_legacy(source: &str) -> Result<Self> {
        ensure_source_bound(source)?;
        let mut class = RawUnitClass::Mech;
        let mut movement = RawMovement::Biped;
        let mut layouts = vec![SectionDefinition::default(); 8];
        let mut active = None;
        let mut attributes = BTreeMap::new();
        let mut pending = String::new();
        let mut pending_line = 0;
        for (index, line) in source.lines().enumerate() {
            let line = line.trim();
            if pending.is_empty() && (line.is_empty() || line.starts_with('#')) {
                continue;
            }
            if pending.is_empty() {
                pending_line = index + 1;
            }
            if !pending.is_empty() {
                pending.push(' ');
            }
            pending.push_str(line);
            if pending.contains('{') && !pending.contains('}') {
                continue;
            }
            apply_record(
                &pending,
                &mut class,
                &mut movement,
                &mut active,
                &mut layouts,
                &mut attributes,
            )
            .with_context(|| format!("template line {pending_line}"))?;
            pending.clear();
        }
        if !pending.is_empty() {
            bail!("unclosed template field on line {pending_line}");
        }
        let sections = RawSectionCode::for_unit(class, movement)
            .iter()
            .copied()
            .enumerate()
            .map(|(index, section)| (section, layouts[index].clone()))
            .collect();
        let heat_sinks = loaded_heat_sinks(&attributes)?;
        Ok(Self {
            name: attributes.get("name").cloned().unwrap_or_default(),
            reference: attributes.get("reference").cloned().unwrap_or_default(),
            class,
            movement,
            tons: parse_i32_field(&attributes, "tons")?,
            max_speed: parse_f32_field(&attributes, "max_speed")?,
            jump_speed: parse_f32_field(&attributes, "jump_speed")?,
            heat_sinks,
            sections,
            attributes,
        })
    }
}

/// Authored sinks narrowed to native storage; non-ICE units without any receive the loader default.
fn loaded_heat_sinks(fields: &BTreeMap<String, String>) -> Result<i32> {
    let specials = fields.get("specials").map(String::as_str).unwrap_or("");
    let authored = parse_i32_field(fields, "heat_sinks")?.clamp(-128, 127);
    let ice = specials
        .split_ascii_whitespace()
        .any(|flag| flag.eq_ignore_ascii_case("ICEEngine_Tech"));
    Ok(if authored == 0 && !ice { 10 } else { authored })
}

fn ensure_source_bound(source: &str) -> Result<()> {
    if source.len() > 1_048_576 {
        bail!("template exceeds size limit");
    }
    Ok(())
}

fn parse_i32_field(fields: &BTreeMap<String, String>, name: &str) -> Result<i32> {
    fields
        .get(name)
        .map(|value| value.parse().with_context(|| format!("invalid {name}")))
        .transpose()
        .map(Option::unwrap_or_default)
}

fn parse_f32_field(fields: &BTreeMap<String, String>, name: &str) -> Result<f64> {
    let value: f32 = fields
        .get(name)
        .map(|value| value.parse().with_context(|| format!("invalid {name}")))
        .transpose()?
        .unwrap_or_default();
    if !value.is_finite() {
        bail!("invalid {name}");
    }
    Ok(f64::from(value))
}

fn apply_record(
    record: &str,
    class: &mut RawUnitClass,
    movement: &mut RawMovement,
    active: &mut Option<usize>,
    layouts: &mut [SectionDefinition],
    attributes: &mut BTreeMap<String, String>,
) -> Result<()> {
    let Some((name, rest)) = record.split_once('{') else {
        let section = RawSectionCode::parse_template_heading(*class, *movement, record)?;
        *active = RawSectionCode::for_unit(*class, *movement)
            .iter()
            .position(|candidate| *candidate == section);
        return Ok(());
    };
    let (value, tail) = rest.split_once('}').context("unclosed template field")?;
    if value.contains('{') || !tail.trim().is_empty() {
        bail!("invalid template field delimiters");
    }
    let name = name.trim().to_ascii_lowercase();
    let value = value.trim();
    if name.starts_with("crit_") {
        let section = active.context("critical outside a section")?;
        let (first, last) = raw_critical_range(&name)?;
        let definition = parse_raw_equipment(value)?;
        for slot in first..=last {
            layouts[section].criticals.insert(slot, definition.clone());
        }
        return Ok(());
    }
    if matches!(name.as_str(), "armor" | "internals" | "rear" | "config") {
        let section = active.context("section field outside a section")?;
        match name.as_str() {
            "armor" => layouts[section].armor = u16::from(parse_clamped_u8(value, "armor")?),
            "internals" => {
                layouts[section].internal = u16::from(parse_clamped_u8(value, "internals")?)
            }
            "rear" => layouts[section].rear = u16::from(parse_clamped_u8(value, "rear")?),
            "config" => layouts[section].configuration = Some(value.to_owned()),
            _ => unreachable!(),
        }
        return Ok(());
    }
    const COMMANDS: &[&str] = &[
        "reference",
        "type",
        "move_type",
        "tons",
        "tac_range",
        "lrs_range",
        "radio_range",
        "scan_range",
        "heat_sinks",
        "max_speed",
        "specials",
        "computer",
        "name",
        "jump_speed",
        "radio",
        "si",
        "fuel",
        "comment",
        "radiotype",
        "cargo_space",
        "max_suits",
        "infantryspecials",
        "max_ton",
        "hsengoverride",
        "unit_era",
        "unit_tro",
    ];
    if !COMMANDS.contains(&name.as_str()) {
        bail!("unsupported template field {name}");
    }
    match name.as_str() {
        "type" => *class = RawUnitClass::parse(value)?,
        "move_type" => *movement = RawMovement::parse(value)?,
        "tons" | "tac_range" | "lrs_range" | "radio_range" | "scan_range" | "heat_sinks"
        | "computer" | "radio" | "si" | "fuel" | "radiotype" | "cargo_space" | "max_suits"
        | "max_ton" | "hsengoverride" => {
            value
                .parse::<i32>()
                .with_context(|| format!("invalid {name}"))?;
        }
        "max_speed" | "jump_speed" => {
            let parsed: f32 = value.parse().with_context(|| format!("invalid {name}"))?;
            if !parsed.is_finite() {
                bail!("invalid {name}");
            }
        }
        _ => {}
    }
    if matches!(name.as_str(), "specials" | "infantryspecials") {
        let combined = attributes.entry(name).or_default();
        append_unique_flags(combined, value);
    } else if name != "comment" {
        attributes.insert(name, value.to_owned());
    }
    Ok(())
}

fn append_unique_flags(combined: &mut String, value: &str) {
    for flag in value.split_ascii_whitespace().filter(|flag| *flag != "-") {
        if combined
            .split_ascii_whitespace()
            .any(|existing| existing.eq_ignore_ascii_case(flag))
        {
            continue;
        }
        if !combined.is_empty() {
            combined.push(' ');
        }
        combined.push_str(flag);
    }
}

fn parse_clamped_u8(value: &str, label: &str) -> Result<u8> {
    let value: i32 = value.parse().with_context(|| format!("invalid {label}"))?;
    Ok(value.clamp(0, 255) as u8)
}

fn raw_critical_range(name: &str) -> Result<(u8, u8)> {
    let range = name
        .strip_prefix("crit_")
        .context("invalid critical heading")?;
    let (first, last) = range.split_once('-').unwrap_or((range, range));
    let first: i32 = first.parse().context("invalid first critical position")?;
    let last: i32 = last.parse().context("invalid last critical position")?;
    if first <= 0 || first > last || last > 12 {
        bail!("critical positions must be between 1 and 12 in ascending order");
    }
    Ok(((first - 1) as u8, (last - 1) as u8))
}

fn parse_raw_equipment(value: &str) -> Result<CriticalDefinition> {
    let words: Vec<_> = value.split_whitespace().collect();
    if words.len() < 3
        && words
            .first()
            .is_some_and(|name| super::BattleSystem::parse(name).is_ok())
    {
        return Ok(CriticalDefinition {
            equipment: words[0].to_owned(),
            data: words.get(1).unwrap_or(&"-").to_string(),
            modes: Vec::new(),
            brand: None,
        });
    }
    if words.len() < 3 {
        bail!("expected equipment, data, modes, and optional brand");
    }
    let ammunition = super::equipment::strip_name_prefix(words[0], "Ammo_").is_some();
    if !ammunition && words.len() > 4 {
        bail!("expected equipment, data, modes, and optional brand");
    }
    let mut end = words.len();
    let mut brand = None;
    if words.len() > 3 {
        let last = words[end - 1];
        let numeric = last.strip_prefix(['+', '-']).unwrap_or(last);
        if ammunition && last == "-" {
            end -= 1;
        } else if !ammunition
            || (!numeric.is_empty() && numeric.bytes().all(|c| c.is_ascii_digit()))
        {
            let parsed: i32 = last.parse().context("invalid equipment brand")?;
            brand = Some(parsed.clamp(0, 255) as u8);
            end -= 1;
        }
    }
    Ok(CriticalDefinition {
        equipment: words[0].to_owned(),
        data: words[1].to_owned(),
        modes: words[2..end]
            .iter()
            .filter(|word| **word != "-")
            .flat_map(|word| word.split('|'))
            .map(str::to_owned)
            .collect(),
        brand,
    })
}

impl From<&BattleTemplate> for RawTemplate {
    fn from(template: &BattleTemplate) -> Self {
        let movement = template
            .attributes
            .get("move_type")
            .and_then(|value| RawMovement::parse(value).ok())
            .unwrap_or(RawMovement::Biped);
        let mut raw = Self::empty(RawUnitClass::Mech, movement);
        raw.name = template.name.clone();
        raw.reference = template.reference.clone();
        raw.tons = i32::from(template.tons);
        raw.max_speed = template.max_speed;
        raw.jump_speed = template.jump_speed;
        raw.heat_sinks = i32::from(template.heat_sinks);
        raw.attributes = template.attributes.clone();
        for (section, definition) in &template.sections {
            let code = match section {
                BattleSection::LeftArm if movement == RawMovement::Quad => {
                    RawSectionCode::FrontLeftLeg
                }
                BattleSection::RightArm if movement == RawMovement::Quad => {
                    RawSectionCode::FrontRightLeg
                }
                BattleSection::LeftLeg if movement == RawMovement::Quad => {
                    RawSectionCode::RearLeftLeg
                }
                BattleSection::RightLeg if movement == RawMovement::Quad => {
                    RawSectionCode::RearRightLeg
                }
                BattleSection::LeftArm => RawSectionCode::LeftArm,
                BattleSection::RightArm => RawSectionCode::RightArm,
                BattleSection::LeftTorso => RawSectionCode::LeftTorso,
                BattleSection::RightTorso => RawSectionCode::RightTorso,
                BattleSection::CenterTorso => RawSectionCode::CenterTorso,
                BattleSection::LeftLeg => RawSectionCode::LeftLeg,
                BattleSection::RightLeg => RawSectionCode::RightLeg,
                BattleSection::Head => RawSectionCode::Head,
            };
            raw.sections.insert(code, definition.clone());
        }
        raw
    }
}

impl From<&BattleVehicleTemplate> for RawTemplate {
    fn from(template: &BattleVehicleTemplate) -> Self {
        let class = if template.is_vtol() {
            RawUnitClass::Vtol
        } else {
            RawUnitClass::Vehicle
        };
        let movement = match template.movement {
            BattleVehicleMovement::Tracked => RawMovement::Tracked,
            BattleVehicleMovement::Wheeled => RawMovement::Wheeled,
            BattleVehicleMovement::Hover => RawMovement::Hover,
            BattleVehicleMovement::Stationary => RawMovement::Stationary,
            BattleVehicleMovement::Vtol => RawMovement::Vtol,
        };
        let mut raw = Self::empty(class, movement);
        raw.name = template.name.clone();
        raw.reference = template.reference.clone();
        raw.tons = i32::from(template.tons);
        raw.max_speed = template.max_speed;
        raw.heat_sinks = i32::from(template.heat_sinks.unwrap_or_default());
        raw.attributes = template.attributes.clone();
        for (section, definition) in &template.sections {
            let code = match section {
                BattleVehicleSection::Left => RawSectionCode::LeftSide,
                BattleVehicleSection::Right => RawSectionCode::RightSide,
                BattleVehicleSection::Front => RawSectionCode::FrontSide,
                BattleVehicleSection::Rear => RawSectionCode::AftSide,
                BattleVehicleSection::Turret => RawSectionCode::Turret,
                BattleVehicleSection::Rotor => RawSectionCode::Rotor,
            };
            raw.sections.insert(code, definition.clone());
        }
        raw
    }
}

/// Exact critical layout established for a newly registered Mech.
pub fn raw_default_mech_criticals() -> BTreeMap<RawSectionCode, SectionDefinition> {
    let mut sections = RawTemplate::empty(RawUnitClass::Mech, RawMovement::Biped).sections;
    let mut install = |section, rows: &[(u8, &str)]| {
        let layout = sections.get_mut(&section).expect("Mech section catalog");
        for &(slot, equipment) in rows {
            layout.criticals.insert(
                slot,
                CriticalDefinition {
                    equipment: equipment.into(),
                    data: "-".into(),
                    modes: Vec::new(),
                    brand: None,
                },
            );
        }
    };
    install(
        RawSectionCode::Head,
        &[
            (0, "LifeSupport"),
            (1, "Sensors"),
            (2, "Cockpit"),
            (4, "Sensors"),
            (5, "LifeSupport"),
        ],
    );
    install(
        RawSectionCode::CenterTorso,
        &[
            (0, "Engine"),
            (1, "Engine"),
            (2, "Engine"),
            (3, "Gyro"),
            (4, "Gyro"),
            (5, "Gyro"),
            (6, "Gyro"),
            (7, "Engine"),
            (8, "Engine"),
            (9, "Engine"),
        ],
    );
    for section in [
        RawSectionCode::LeftArm,
        RawSectionCode::RightArm,
        RawSectionCode::LeftLeg,
        RawSectionCode::RightLeg,
    ] {
        install(
            section,
            &[
                (0, "ShoulderOrHip"),
                (1, "UpperActuator"),
                (2, "LowerActuator"),
                (3, "HandOrFootActuator"),
            ],
        );
    }
    sections
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_native_class_has_the_source_ordered_section_catalog() {
        let cases = [
            (RawUnitClass::Mech, RawMovement::Biped, 8),
            (RawUnitClass::Mech, RawMovement::Quad, 8),
            (RawUnitClass::Vehicle, RawMovement::Tracked, 5),
            (RawUnitClass::Vtol, RawMovement::Vtol, 6),
            (RawUnitClass::Naval, RawMovement::Hull, 5),
            (RawUnitClass::SpheroidDropship, RawMovement::Fly, 6),
            (RawUnitClass::AeroFighter, RawMovement::Fly, 4),
            (RawUnitClass::MechWarrior, RawMovement::Biped, 8),
            (RawUnitClass::AerodyneDropship, RawMovement::Fly, 6),
            (RawUnitClass::BattleSuit, RawMovement::Biped, 8),
        ];
        for (class, movement, count) in cases {
            assert_eq!(RawSectionCode::for_unit(class, movement).len(), count);
            assert_eq!(RawTemplate::empty(class, movement).sections.len(), count);
        }
    }

    #[test]
    fn parses_noncombat_classes_without_a_runtime_constructor() {
        let template = RawTemplate::parse(
            "AERO",
            "class = \"aerofighter\"\nmovement = \"fly\"\ntons = 90\n[sections.nose]\narmor = 67\ninternals = 9\n",
        )
        .unwrap();
        assert_eq!(template.class, RawUnitClass::AeroFighter);
        assert_eq!(template.reference, "AERO");
        assert_eq!(template.sections.len(), 4);
        assert_eq!(template.sections[&RawSectionCode::Nose].armor, 67);
        assert_eq!(template.sections[&RawSectionCode::AftSide].armor, 0);
    }

    #[test]
    fn registered_mech_defaults_include_exact_native_system_slots() {
        let sections = raw_default_mech_criticals();
        assert_eq!(sections[&RawSectionCode::Head].criticals.len(), 5);
        assert_eq!(
            sections[&RawSectionCode::CenterTorso].criticals[&3].equipment,
            "Gyro"
        );
        assert_eq!(sections[&RawSectionCode::LeftArm].criticals.len(), 4);
    }

    #[test]
    fn source_integers_follow_native_signed_and_narrow_storage() {
        let template = RawTemplate::parse(
            "X",
            "tons = -2147483648\nheat_sinks = 999\n[sections.head]\narmor = 300\ninternals = 256\nrear = 65535\n",
        )
        .unwrap();
        assert_eq!(template.tons, i32::MIN);
        assert_eq!(template.heat_sinks, 127);
        let head = &template.sections[&RawSectionCode::Head];
        assert_eq!((head.armor, head.internal, head.rear), (255, 255, 255));
        assert!(RawTemplate::parse("X", "tons = 2147483648").is_err());
        assert!(RawTemplate::parse("X", "[sections.head]\narmor = -1").is_err());
    }

    #[test]
    fn sections_follow_class_anatomy_and_flags_merge_case_insensitively() {
        let template = RawTemplate::parse(
            "X",
            "class = \"aerofighter\"\nmovement = \"fly\"\nspecials = [\"Clan\", \"ECM\", \"clan\"]\n[sections.left_wing]\narmor = 2\n",
        )
        .unwrap();
        assert_eq!(template.class, RawUnitClass::AeroFighter);
        assert_eq!(template.sections[&RawSectionCode::LeftWing].armor, 2);
        assert_eq!(template.attributes["specials"], "Clan ECM");
        assert!(RawTemplate::parse("X", "[sections.left_arm]\n[sections.LEFT_ARM]").is_err());
        assert!(RawTemplate::parse("X", "class = \"aerofighter\"\n[sections.left_arm]").is_err());
        assert!(RawTemplate::parse("X", "unknown_field = 1").is_err());
    }

    #[test]
    fn missing_heat_sinks_receive_only_the_non_ice_loader_default() {
        assert_eq!(RawTemplate::parse("X", "").unwrap().heat_sinks, 10);
        assert_eq!(
            RawTemplate::parse("X", "specials = [\"ICEEngine_Tech\"]")
                .unwrap()
                .heat_sinks,
            0
        );
    }
}
