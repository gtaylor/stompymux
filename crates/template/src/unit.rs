//! Explicit asset-class dispatch for administrative unit construction.
use super::{BattleTemplate, BattleVehicleTemplate, document::ParsedTemplate};
use anyhow::{Result, bail};

/// Supported construction asset classes retain their own anatomy and validation.
#[derive(Debug, Clone, PartialEq)]
pub enum BattleUnitTemplate {
    Mech(BattleTemplate),
    Vehicle(BattleVehicleTemplate),
}

impl BattleUnitTemplate {
    /// Decode a TOML document whose file stem is `reference`, selecting by its declared class.
    /// A malformed Mech never falls back to vehicle parsing.
    pub fn parse(reference: &str, source: &str) -> Result<Self> {
        Self::from_parsed(ParsedTemplate::parse(reference, source)?)
    }

    /// Dispatch already decoded fields and sections to their class's validation.
    pub fn from_parsed(parsed: ParsedTemplate) -> Result<Self> {
        let kind = parsed.required("type")?;
        match kind.to_ascii_lowercase().as_str() {
            "mech" => Ok(Self::Mech(BattleTemplate::from_parsed(parsed)?)),
            "vehicle" | "vtol" => Ok(Self::Vehicle(BattleVehicleTemplate::from_parsed(parsed)?)),
            _ => bail!("Unsupported unit template type {kind}"),
        }
    }
}
