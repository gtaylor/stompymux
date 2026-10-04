//! Typed BattleMech templates decoded from TOML documents; equipment awaits catalogue validation.
use super::document::ParsedTemplate;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable BattleMech attachment identity in persisted order.
/// Limb identifiers use biped names; use the chassis for quad names and leg roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum MechSection {
    LeftArm,
    RightArm,
    LeftTorso,
    RightTorso,
    CenterTorso,
    LeftLeg,
    RightLeg,
    Head,
}

impl MechSection {
    /// All eight sections in storage order.
    pub const ALL: [Self; 8] = [
        Self::LeftArm,
        Self::RightArm,
        Self::LeftTorso,
        Self::RightTorso,
        Self::CenterTorso,
        Self::LeftLeg,
        Self::RightLeg,
        Self::Head,
    ];

    /// Decode a template section heading.
    pub fn parse(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|section| section.name().eq_ignore_ascii_case(name))
            .with_context(|| format!("unsupported section {name}"))
    }

    /// Stable template-file spelling.
    pub fn name(self) -> &'static str {
        match self {
            Self::LeftArm => "Left_Arm",
            Self::RightArm => "Right_Arm",
            Self::LeftTorso => "Left_Torso",
            Self::RightTorso => "Right_Torso",
            Self::CenterTorso => "Center_Torso",
            Self::LeftLeg => "Left_Leg",
            Self::RightLeg => "Right_Leg",
            Self::Head => "Head",
        }
    }
}

/// One occupied template slot, with asset-level data and modes retained verbatim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CriticalDefinition {
    pub equipment: String,
    pub data: String,
    pub modes: Vec<String>,
}

/// Unresolved armor and slot layout shared by unit-specific template decoders.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SectionDefinition {
    pub armor: u16,
    pub internal: u16,
    pub rear: u16,
    /// Zero-based critical positions. Unoccupied slots are absent.
    pub criticals: BTreeMap<u8, CriticalDefinition>,
    pub configuration: Option<String>,
}

/// A decoded BattleMech asset; successful parsing does not imply simulation support.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MechTemplate {
    pub name: String,
    pub reference: String,
    pub tons: u16,
    pub max_speed: f64,
    pub jump_speed: f64,
    pub heat_sinks: u16,
    pub sections: BTreeMap<MechSection, SectionDefinition>,
    /// All unit-level fields, including features awaiting domain implementation.
    pub attributes: BTreeMap<String, String>,
}

impl MechTemplate {
    /// Clan chassis always use double-efficiency heat sinks.
    pub fn has_double_heat_sinks(&self) -> bool {
        self.has_special("Clan") || self.has_special("DoubleHS")
    }

    /// Number of contiguous critical slots occupied by one installed heat sink.
    pub fn heat_sink_slots(&self) -> usize {
        if self.has_special("Clan") {
            return 2;
        }
        if self.has_special("DoubleHS") {
            return 3;
        }
        1
    }

    /// Whether a component is Clan technology: the chassis base unless a mixed-technology flag
    /// names the other base.
    fn clan_component(&self, component: super::construction::Component) -> bool {
        let clan = self.has_special("Clan");
        clan != self.has_special(component.flag(!clan))
    }

    /// Whether the engine is a Clan engine, which governs XL and XXL side slots and mass.
    pub fn clan_engine(&self) -> bool {
        self.clan_component(super::construction::Component::Engine)
    }

    /// Whether the internal structure is Clan technology, which governs endo steel slots.
    pub fn clan_structure(&self) -> bool {
        self.clan_component(super::construction::Component::Structure)
    }

    /// Whether the armor is Clan technology, which governs ferro-fibrous and reflective slots
    /// and ferro-fibrous protection per ton.
    pub fn clan_armor(&self) -> bool {
        self.clan_component(super::construction::Component::Armor)
    }

    /// Test a chassis technology by its full name or the reference's abbreviation, which
    /// templates use interchangeably.
    pub fn has_technology(&self, technology: super::Technology) -> bool {
        let (name, abbreviation) = technology.names();
        self.has_special(name) || self.has_special(abbreviation)
    }

    /// Test a whitespace-separated chassis feature using the asset's case-insensitive spelling.
    pub fn has_special(&self, name: &str) -> bool {
        self.attributes.get("specials").is_some_and(|value| {
            value
                .split_ascii_whitespace()
                .any(|flag| flag.eq_ignore_ascii_case(name))
        })
    }

    /// Decode a TOML template document whose file stem is `reference`.
    pub fn parse(reference: &str, source: &str) -> Result<Self> {
        Self::from_parsed(ParsedTemplate::parse(reference, source)?)
    }

    /// Validate decoded fields and sections as a BattleMech.
    pub fn from_parsed(parsed: ParsedTemplate) -> Result<Self> {
        let ParsedTemplate { fields, sections } = parsed;
        let required = |name: &str| -> Result<String> {
            fields
                .get(name)
                .filter(|value| !value.is_empty())
                .cloned()
                .with_context(|| format!("missing template field {name}"))
        };
        ensure!(
            required("type")?.eq_ignore_ascii_case("Mech"),
            "only BattleMech templates are supported"
        );
        let chassis = super::MechChassis::parse(&required("move_type")?)?;
        let sections: BTreeMap<_, _> = sections
            .into_iter()
            .map(|(name, layout)| Ok((chassis.parse_section(&name)?, layout)))
            .collect::<Result<_>>()?;
        let speed = |name: &str| -> Result<f64> {
            let value: f64 = fields
                .get(name)
                .map(String::as_str)
                .unwrap_or("0")
                .parse()
                .with_context(|| format!("invalid {name}"))?;
            ensure!(value.is_finite() && value >= 0.0, "invalid {name}");
            Ok(value)
        };
        ensure!(
            sections.len() == 8 && sections.values().all(|section| section.internal > 0),
            "all eight sections require positive internals"
        );
        let tons = required("tons")?.parse().context("invalid tonnage")?;
        ensure!(tons > 0, "tonnage must be positive");
        let mut template = Self {
            name: required("name")?,
            reference: required("reference")?,
            tons,
            max_speed: speed("max_speed")?,
            jump_speed: speed("jump_speed")?,
            heat_sinks: fields
                .get("heat_sinks")
                .map(String::as_str)
                .unwrap_or("10")
                .parse()
                .context("invalid heat sinks")?,
            sections,
            attributes: fields,
        };
        // The reference load always forces the tonnage-chart internal structure
        // (mech_int_check) while reading the template file, overriding authored
        // Internals lines for both the original and the live value. Keeping the
        // rewrite here, where C's parse and finalize are one operation, leaves
        // saved definitions and later construction edits restored verbatim.
        let quad = chassis == super::MechChassis::Quad;
        for section in MechSection::ALL {
            if let Some(expected) = super::construction::mech_internal(template.tons, section, quad)
                && let Some(layout) = template.sections.get_mut(&section)
            {
                layout.internal = expected;
            }
        }
        Ok(template)
    }
}

impl MechSection {
    /// Destination of excess biped damage; head and center torso have no transfer destination.
    pub fn damage_transfer(self) -> Option<Self> {
        use MechSection::*;
        match self {
            LeftArm | LeftLeg => Some(LeftTorso),
            RightArm | RightLeg => Some(RightTorso),
            LeftTorso | RightTorso => Some(CenterTorso),
            CenterTorso | Head => None,
        }
    }
}

impl MechTemplate {
    /// Installed slots determine TSM technology; loss or flooding does not remove passive myomer.
    pub fn has_triple_myomer(&self) -> bool {
        self.sections
            .values()
            .flat_map(|section| section.criticals.values())
            .filter(|part| part.equipment.eq_ignore_ascii_case("TripleStrengthMyomer"))
            .take(6)
            .count()
            >= 6
    }
}
