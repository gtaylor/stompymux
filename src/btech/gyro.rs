//! Gyro construction identity and effective damage derived from installed critical losses.
use super::{BattleTemplate, BattleUnit};
use serde::Serialize;

/// Gyro families with implemented construction and damage behavior.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleGyro {
    Standard,
    Hardened,
    Xl,
    Compact,
}

impl BattleGyro {
    /// Resolve the explicit construction family; ambiguous technology is rejected at construction.
    pub(super) fn from_definition(definition: &BattleTemplate) -> Self {
        if definition.has_special("XLGYRO") || definition.has_special("XLGyro_Tech") {
            return Self::Xl;
        }
        if definition.has_special("HDGYRO") || definition.has_special("HDGyro_Tech") {
            return Self::Hardened;
        }
        if definition.has_special("CGYRO") || definition.has_special("CompactGyro_Tech") {
            return Self::Compact;
        }
        Self::Standard
    }

    /// Number of critical slots in a complete installation.
    pub(super) fn critical_slots(self) -> usize {
        match self {
            Self::Standard | Self::Hardened => 4,
            Self::Xl => 6,
            Self::Compact => 2,
        }
    }

    /// Scale the rounded standard mass, including the destroyed-center accounting value.
    pub(super) fn mass(self, standard: i32) -> i32 {
        match self {
            Self::Standard => standard,
            Self::Hardened => standard * 2,
            Self::Xl => standard / 2,
            Self::Compact => standard * 3 / 2,
        }
    }
}

impl BattleUnit {
    /// Installed family, retained after critical and section losses.
    pub fn gyro(&self) -> BattleGyro {
        BattleGyro::from_definition(self.definition())
    }

    /// Effective gyro damage; the first hardened critical does not impair stability.
    pub fn gyro_damage(&self) -> u8 {
        self.gyro_condition().0
    }
}
