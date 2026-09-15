//! Explicit asset-class dispatch for administrative unit construction.
use super::{BattleTemplate, BattleVehicleTemplate, template::ParsedTemplate};
use anyhow::{Result, bail};

/// Supported construction asset classes retain their own anatomy and validation.
#[derive(Debug, Clone, PartialEq)]
pub enum BattleUnitTemplate {
    Mech(BattleTemplate),
    Vehicle(BattleVehicleTemplate),
}

impl BattleUnitTemplate {
    /// Select by the declared class; a malformed Mech never falls back to vehicle parsing.
    pub fn parse(source: &str) -> Result<Self> {
        let kind = ParsedTemplate::parse(source)?.required("type")?;
        match kind.to_ascii_lowercase().as_str() {
            "mech" => Ok(Self::Mech(BattleTemplate::parse(source)?)),
            "vehicle" | "vtol" => Ok(Self::Vehicle(BattleVehicleTemplate::parse(source)?)),
            _ => bail!("Unsupported unit template type {kind}"),
        }
    }

    /// Construct through the selected class's checked world operation.
    pub fn create(self, world: &mut crate::World, id: crate::ObjectId) -> Result<()> {
        match self {
            Self::Mech(definition) => super::create_unit(world, id, definition),
            Self::Vehicle(definition) => super::create_vehicle(world, id, definition),
        }
    }
}
