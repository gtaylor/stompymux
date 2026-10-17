//! Range brackets: how far a weapon reaches and the to-hit modifier each band applies, with
//! hotloading and ammunition adjustments.
use crate::{AmmunitionMode, FireMode, Weapon};
use anyhow::{Result, ensure};
use serde::Serialize;

/// Conventional range bracket after the game's fractional-distance rounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RangeBracket {
    Minimum,
    Short,
    Medium,
    Long,
}

/// In-range weapon contribution; absence means beyond the effective physical range limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WeaponRange {
    pub bracket: RangeBracket,
    pub modifier: u8,
}

impl Weapon {
    /// Preserve raw minimum/maximum checks and the rounded brackets used between them.
    pub fn range_modifier(self, distance: f64) -> Result<Option<WeaponRange>> {
        self.range_modifier_for_mode(distance, FireMode::Normal)
    }

    /// Hotloading removes minimum-range penalties without changing the ordinary range brackets.
    pub fn range_modifier_for_mode(
        self,
        distance: f64,
        mode: FireMode,
    ) -> Result<Option<WeaponRange>> {
        self.range_modifier_for_ammunition(distance, mode, AmmunitionMode::Normal)
    }

    /// Ammunition-specific reach retains the ordinary minimum and intermediate brackets.
    pub fn range_modifier_for_ammunition(
        self,
        distance: f64,
        mode: FireMode,
        ammunition: AmmunitionMode,
    ) -> Result<Option<WeaponRange>> {
        ensure!(
            ammunition.supports(self),
            "Unsupported ammunition for weapon"
        );
        ensure!(
            distance.is_finite() && distance >= 0.0,
            "Invalid weapon range"
        );
        // Artillery cannons aim as ballistic weapons with hex ranges (Tactical Operations:
        // Advanced Units & Equipment p. 97); indirect artillery has its own aim rules.
        ensure!(
            !self.is_indirect_artillery(),
            "Artillery requires artillery aim rules"
        );
        let mut profile = self.profile_for_ammunition(ammunition);
        if mode == FireMode::Hotload {
            ensure!(self.supports_hotload(), "Weapon cannot be hotloaded");
            profile.minimum_range = 0;
        }
        let maximum = self.effective_range_for_ammunition(ammunition);
        if distance > f64::from(maximum) {
            return Ok(None);
        }
        if profile.minimum_range > 0 && distance <= f64::from(profile.minimum_range) {
            return Ok(Some(WeaponRange {
                bracket: RangeBracket::Minimum,
                modifier: (f64::from(profile.minimum_range) - distance + 1.0).floor() as u8,
            }));
        }
        let rounded = (distance + 0.95).floor() as u8;
        // Total Warfare p. 107: beyond long range a weapon cannot fire at all.
        let (bracket, modifier) = if rounded > profile.long_range {
            return Ok(None);
        } else if rounded > profile.medium_range {
            (RangeBracket::Long, 4)
        } else if rounded > profile.short_range {
            (RangeBracket::Medium, 2)
        } else if profile.minimum_range > 0 && rounded <= profile.minimum_range {
            (RangeBracket::Minimum, profile.minimum_range - rounded + 1)
        } else {
            (RangeBracket::Short, 0)
        };
        Ok(Some(WeaponRange { bracket, modifier }))
    }
}
