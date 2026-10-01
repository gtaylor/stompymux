//! Typed BattleMech templates decoded from TOML documents; equipment awaits catalogue validation.
use super::template_document::ParsedTemplate;
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Stable BattleMech attachment identity in persisted order.
/// Limb identifiers use biped names; use the chassis for quad names and leg roles.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum BattleSection {
    LeftArm,
    RightArm,
    LeftTorso,
    RightTorso,
    CenterTorso,
    LeftLeg,
    RightLeg,
    Head,
}

impl BattleSection {
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
    pub brand: Option<u8>,
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
pub struct BattleTemplate {
    pub name: String,
    pub reference: String,
    pub tons: u16,
    pub max_speed: f64,
    pub jump_speed: f64,
    pub heat_sinks: u16,
    pub sections: BTreeMap<BattleSection, SectionDefinition>,
    /// All unit-level fields, including features awaiting domain implementation.
    pub attributes: BTreeMap<String, String>,
}

impl BattleTemplate {
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

    /// Test a chassis technology by its full name or the reference's abbreviation, which
    /// templates use interchangeably.
    pub(crate) fn has_technology(&self, technology: super::BattleTechnology) -> bool {
        let (name, abbreviation) = technology.names();
        self.has_special(name) || self.has_special(abbreviation)
    }

    /// Test a whitespace-separated chassis feature using the asset's case-insensitive spelling.
    pub(crate) fn has_special(&self, name: &str) -> bool {
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
    pub(super) fn from_parsed(parsed: ParsedTemplate) -> Result<Self> {
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
        let chassis = super::BattleMechChassis::parse(&required("move_type")?)?;
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
        let quad = chassis == super::BattleMechChassis::Quad;
        for section in BattleSection::ALL {
            if let Some(expected) = Self::chart_internal(template.tons, section, quad)
                && let Some(layout) = template.sections.get_mut(&section)
            {
                layout.internal = expected;
            }
        }
        Ok(template)
    }

    /// Expected mech internal structure from the reference tonnage chart.
    ///
    /// Rows are `[tons, center torso, side torsos, arms, legs]`; quad chassis
    /// use the leg column for arms. Head structure is always three and unknown
    /// tonnage leaves authored internals untouched, matching mech_int_check.
    fn chart_internal(tons: u16, section: BattleSection, quad: bool) -> Option<u16> {
        const STRUCTURE: [[i16; 5]; 20] = [
            [10, 4, 3, 1, 2],
            [15, 5, 4, 2, 3],
            [20, 6, 5, 3, 4],
            [25, 8, 6, 4, 6],
            [30, 10, 7, 5, 7],
            [35, 11, 8, 6, 8],
            [40, 12, 10, 6, 10],
            [45, 14, 11, 7, 11],
            [50, 16, 12, 8, 12],
            [55, 18, 13, 9, 13],
            [60, 20, 14, 10, 14],
            [65, 21, 15, 10, 15],
            [70, 22, 15, 11, 15],
            [75, 23, 16, 12, 16],
            [80, 25, 17, 13, 17],
            [85, 27, 18, 14, 18],
            [90, 29, 19, 15, 19],
            [95, 30, 20, 16, 20],
            [100, 31, 21, 17, 21],
            [-1, 0, 0, 0, 0],
        ];
        let row = STRUCTURE.iter().find(|row| row[0] == tons as i16)?;
        let column = match section {
            BattleSection::Head => return Some(3),
            BattleSection::CenterTorso => 1,
            BattleSection::LeftTorso | BattleSection::RightTorso => 2,
            BattleSection::LeftArm | BattleSection::RightArm if quad => 4,
            BattleSection::LeftArm | BattleSection::RightArm => 3,
            BattleSection::LeftLeg | BattleSection::RightLeg => 4,
        };
        u16::try_from(row[column]).ok()
    }
}
