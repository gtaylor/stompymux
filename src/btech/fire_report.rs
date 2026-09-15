//! Distinct unit and terrain firing results sharing the configured command transaction.
use super::{BattleHexShotReport, BattleShotReport};
use serde::Serialize;

/// The target kind determines the report shape; unit reports retain their existing fields.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum BattleFireReport {
    Unit(Box<BattleShotReport>),
    Vehicle(Box<super::BattleVehicleShotReport>),
    Hex(Box<BattleHexShotReport>),
    Artillery(Box<super::BattleArtilleryLaunchReport>),
}

impl From<BattleShotReport> for BattleFireReport {
    /// Keep large reports off the enum stack.
    fn from(report: BattleShotReport) -> Self {
        Self::Unit(Box::new(report))
    }
}

impl From<BattleHexShotReport> for BattleFireReport {
    /// Preserve coordinate-only identity in terrain reports.
    fn from(report: BattleHexShotReport) -> Self {
        Self::Hex(Box::new(report))
    }
}

impl From<super::BattleArtilleryLaunchReport> for BattleFireReport {
    /// Preserve queued launch identity separately from immediate target damage.
    fn from(report: super::BattleArtilleryLaunchReport) -> Self {
        Self::Artillery(Box::new(report))
    }
}

impl From<super::BattleVehicleShotReport> for BattleFireReport {
    /// Retain vehicle launch and target anatomy in the shared firing transaction.
    fn from(report: super::BattleVehicleShotReport) -> Self {
        Self::Vehicle(Box::new(report))
    }
}
