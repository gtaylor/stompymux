//! Shared ground-vehicle and VTOL asset anatomy, independent of live simulation admission.
use super::{SectionDefinition, document::ParsedTemplate};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Vehicle locomotion retained from assets; fixed installations and rotorcraft remain distinct.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleVehicleMovement {
    Tracked,
    Wheeled,
    Hover,
    Stationary,
    Vtol,
}

impl BattleVehicleMovement {
    /// Configured control skill; fixed installations have no extended locomotion skill.
    pub fn piloting_skill(self, extended: bool) -> Option<&'static str> {
        if !extended {
            return Some(if self == Self::Vtol {
                "Piloting-Aerospace"
            } else {
                "Drive"
            });
        }
        match self {
            Self::Tracked => Some("Piloting-Tracked"),
            Self::Wheeled => Some("Piloting-Wheeled"),
            Self::Hover => Some("Piloting-Hover"),
            Self::Stationary => None,
            Self::Vtol => Some("Piloting-Aerospace"),
        }
    }

    /// Decode locomotion without conflating hovercraft and rotorcraft.
    pub fn parse(name: &str) -> Result<Self> {
        match name.to_ascii_lowercase().as_str() {
            "track" => Ok(Self::Tracked),
            "wheel" => Ok(Self::Wheeled),
            "hover" => Ok(Self::Hover),
            "none" => Ok(Self::Stationary),
            "vtol" => Ok(Self::Vtol),
            _ => anyhow::bail!("Unsupported vehicle movement {name}"),
        }
    }
}

/// Vehicle armor faces have their own identities; they are not BattleMech limbs or torso sections.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleVehicleSection {
    Left,
    Right,
    Front,
    Rear,
    Turret,
    Rotor,
}

impl BattleVehicleSection {
    /// Stable asset headings for hull faces, turrets and rotors.
    pub fn name(self) -> &'static str {
        match self {
            Self::Left => "Left_Side",
            Self::Right => "Right_Side",
            Self::Front => "Front_Side",
            Self::Rear => "Aft_Side",
            Self::Turret => "Turret",
            Self::Rotor => "Rotor",
        }
    }

    /// Decode cockpit face names, compact labels and canonical template headings.
    pub fn parse_location(value: &str) -> Result<Self> {
        let normalized = value.to_ascii_lowercase().replace(['_', ' '], "");
        match normalized.as_str() {
            "l" | "ls" | "left" | "leftside" => Ok(Self::Left),
            "r" | "rs" | "right" | "rightside" => Ok(Self::Right),
            "f" | "fs" | "front" | "frontside" => Ok(Self::Front),
            "a" | "as" | "rear" | "aft" | "aftside" => Ok(Self::Rear),
            "t" | "tu" | "turret" => Ok(Self::Turret),
            "ro" | "rotor" => Ok(Self::Rotor),
            _ => anyhow::bail!("Invalid vehicle section {value}"),
        }
    }

    /// Parse shared vehicle anatomy without accepting BattleMech or aerospace headings.
    pub fn parse(name: &str) -> Result<Self> {
        [
            Self::Left,
            Self::Right,
            Self::Front,
            Self::Rear,
            Self::Turret,
            Self::Rotor,
        ]
        .into_iter()
        .find(|section| section.name().eq_ignore_ascii_case(name))
        .with_context(|| format!("Unsupported vehicle section {name}"))
    }
}

/// A parsed vehicle asset. Equipment validation, construction and vehicle simulation are separate work.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BattleVehicleTemplate {
    pub name: String,
    pub reference: String,
    pub movement: BattleVehicleMovement,
    pub tons: u16,
    pub max_speed: f64,
    /// Absence is preserved; the eventual engine rules determine the installed cooling.
    pub heat_sinks: Option<u16>,
    pub sections: BTreeMap<BattleVehicleSection, SectionDefinition>,
    /// Preserve unit-level fields, including cargo, specials and equipment awaiting implementation.
    pub attributes: BTreeMap<String, String>,
}

impl BattleVehicleTemplate {
    /// Installed cooling capacity; fusion vehicles retain ten sinks when omitted or zero.
    pub fn heat_sink_capacity(&self) -> u16 {
        match self.heat_sinks.unwrap_or(0) {
            0 if !self.has_special("ICEEngine_Tech") => 10,
            count => count,
        }
    }

    /// Class remains distinct from movement for stationary observation rotorcraft.
    pub fn is_vtol(&self) -> bool {
        self.attributes
            .get("type")
            .is_some_and(|kind| kind.eq_ignore_ascii_case("VTOL"))
    }

    /// Enforce chassis identity for parsed assets and directly edited construction definitions.
    pub fn validate_anatomy(&self) -> Result<()> {
        ensure!(self.tons > 0, "Vehicle tonnage must be positive");
        super::validate_unit_metadata(&self.attributes)?;
        super::read_engine_sink_override(&self.attributes)?;
        super::read_template_speed(&self.attributes, self.max_speed)?;
        let kind = self
            .attributes
            .get("type")
            .context("Vehicle type is missing")?;
        ensure!(
            kind.eq_ignore_ascii_case("Vehicle") || self.is_vtol(),
            "Unsupported vehicle class"
        );
        ensure!(
            if self.is_vtol() {
                matches!(
                    self.movement,
                    BattleVehicleMovement::Vtol | BattleVehicleMovement::Stationary
                )
            } else {
                self.movement != BattleVehicleMovement::Vtol
            },
            "Vehicle type and locomotion disagree"
        );
        let rotor = self.sections.get(&BattleVehicleSection::Rotor);
        ensure!(
            if self.is_vtol() {
                rotor.is_some_and(|section| section.internal > 0)
            } else {
                rotor.is_none()
            },
            "Rotor anatomy must match the vehicle class"
        );
        // Hardened armor is too heavy for craft that ride on air: hovercraft and VTOLs.
        ensure!(
            !self.has_technology(super::BattleTechnology::HardenedArmor)
                || !matches!(
                    self.movement,
                    BattleVehicleMovement::Hover | BattleVehicleMovement::Vtol
                ),
            "Hovercraft and VTOLs cannot mount hardened armor"
        );
        Ok(())
    }

    /// Decode a TOML vehicle document whose file stem is `reference`, without admitting unsupported equipment to live play.
    pub fn parse(reference: &str, source: &str) -> Result<Self> {
        Self::from_parsed(ParsedTemplate::parse(reference, source)?)
    }

    /// Validate decoded fields and sections as a ground vehicle or VTOL.
    pub fn from_parsed(parsed: ParsedTemplate) -> Result<Self> {
        let kind = parsed.required("type")?;
        ensure!(
            kind.eq_ignore_ascii_case("Vehicle") || kind.eq_ignore_ascii_case("VTOL"),
            "Expected a vehicle or VTOL template"
        );
        let movement = BattleVehicleMovement::parse(&parsed.required("move_type")?)?;
        let name = parsed.required("name")?;
        let reference = parsed.required("reference")?;
        let tons = parsed
            .required("tons")?
            .parse::<u16>()
            .context("invalid vehicle tonnage")?;
        ensure!(tons > 0, "vehicle tonnage must be positive");
        let max_speed = parsed
            .fields
            .get("max_speed")
            .map(String::as_str)
            .unwrap_or("0")
            .parse::<f64>()
            .context("invalid vehicle speed")?;
        ensure!(
            max_speed.is_finite() && max_speed >= 0.0,
            "invalid vehicle speed"
        );
        let heat_sinks = parsed
            .fields
            .get("heat_sinks")
            .map(|value| value.parse::<u16>().context("invalid vehicle heat sinks"))
            .transpose()?;
        let mut sections: BTreeMap<_, _> = parsed
            .sections
            .into_iter()
            .map(|(name, section)| Ok((BattleVehicleSection::parse(&name)?, section)))
            .collect::<Result<_>>()?;
        for section in [
            BattleVehicleSection::Left,
            BattleVehicleSection::Right,
            BattleVehicleSection::Front,
            BattleVehicleSection::Rear,
        ] {
            ensure!(
                sections
                    .get(&section)
                    .is_some_and(|layout| layout.internal > 0),
                "Vehicle hull face {} requires positive internals",
                section.name()
            );
        }
        // The reference load forces vehicle internal structure to
        // (tons + 5, at least 10) / 10 for every section that has any
        // (vehicle_int_check), overriding authored Internals lines. Keeping the
        // rewrite here, where C's parse and finalize are one operation, leaves
        // saved definitions and later construction edits restored verbatim.
        let expected_internal =
            u16::try_from((i32::from(tons) + 5).max(10) / 10).expect("vehicle tonnage is bounded");
        for layout in sections.values_mut() {
            if layout.internal != 0 {
                layout.internal = expected_internal;
            }
        }
        let template = Self {
            name,
            reference,
            movement,
            tons,
            max_speed,
            heat_sinks,
            sections,
            attributes: parsed.fields,
        };
        template.validate_anatomy()?;
        Ok(template)
    }
}
