//! Distinct unit and terrain firing results sharing the configured command transaction.
use super::{HexShotReport, MechShotReport};
use serde::Serialize;

/// The target kind determines the report shape; unit reports retain their existing fields.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(untagged)]
pub enum FireReport {
    Unit(Box<MechShotReport>),
    Vehicle(Box<super::VehicleShotReport>),
    Hex(Box<HexShotReport>),
    Artillery(Box<super::ArtilleryLaunchReport>),
}

impl From<MechShotReport> for FireReport {
    /// Keep large reports off the enum stack.
    fn from(report: MechShotReport) -> Self {
        Self::Unit(Box::new(report))
    }
}

impl From<HexShotReport> for FireReport {
    /// Preserve coordinate-only identity in terrain reports.
    fn from(report: HexShotReport) -> Self {
        Self::Hex(Box::new(report))
    }
}

impl From<super::ArtilleryLaunchReport> for FireReport {
    /// Preserve queued launch identity separately from immediate target damage.
    fn from(report: super::ArtilleryLaunchReport) -> Self {
        Self::Artillery(Box::new(report))
    }
}

impl From<super::VehicleShotReport> for FireReport {
    /// Retain vehicle launch and target anatomy in the shared firing transaction.
    fn from(report: super::VehicleShotReport) -> Self {
        Self::Vehicle(Box::new(report))
    }
}
