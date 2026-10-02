//! Class-neutral BattleTech template state used by administrative and inspection contracts.
use super::template_document::ParsedTemplate;
use super::{
    BattleSection, BattleTemplate, BattleVehicleMovement, BattleVehicleSection,
    BattleVehicleTemplate, CriticalDefinition, SectionDefinition,
};
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
            "class = \"aerofighter\"\nmovement = \"fly\"\nspecials = [\"ECM\", \"ecm\"]\n[construction]\ntech_base = \"clan\"\n[sections.left_wing]\narmor = 2\n",
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
            RawTemplate::parse("X", "[construction]\nengine = \"ice\"")
                .unwrap()
                .heat_sinks,
            0
        );
    }
}
