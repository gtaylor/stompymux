//! Range brackets: how far a weapon reaches and the to-hit modifier each band applies, with
//! hotloading and ammunition adjustments.
use crate::{BattleAmmunitionMode, BattleFireMode, BattleWeapon};
use anyhow::{Result, ensure};
use serde::Serialize;

/// Conventional range bracket after the game's fractional-distance rounding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleRangeBracket {
    Minimum,
    Short,
    Medium,
    Long,
    Extreme,
}

/// In-range weapon contribution; absence means beyond the effective physical range limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleWeaponRange {
    pub bracket: BattleRangeBracket,
    pub modifier: u8,
}

impl BattleWeapon {
    /// Preserve raw minimum/maximum checks and the rounded brackets used between them.
    pub fn range_modifier(
        self,
        distance: f64,
        extended: bool,
    ) -> Result<Option<BattleWeaponRange>> {
        self.range_modifier_for_mode(distance, extended, BattleFireMode::Normal, false)
    }

    /// Hotloading removes minimum-range penalties without changing the ordinary range brackets.
    pub fn range_modifier_for_mode(
        self,
        distance: f64,
        extended: bool,
        mode: BattleFireMode,
        half_minimum: bool,
    ) -> Result<Option<BattleWeaponRange>> {
        self.range_modifier_for_ammunition(
            distance,
            extended,
            mode,
            half_minimum,
            BattleAmmunitionMode::Normal,
        )
    }

    /// Ammunition-specific reach retains the ordinary minimum and intermediate brackets.
    pub fn range_modifier_for_ammunition(
        self,
        distance: f64,
        extended: bool,
        mode: BattleFireMode,
        half_minimum: bool,
        ammunition: BattleAmmunitionMode,
    ) -> Result<Option<BattleWeaponRange>> {
        ensure!(
            ammunition.supports(self),
            "Unsupported ammunition for weapon"
        );
        ensure!(
            distance.is_finite() && distance >= 0.0,
            "Invalid weapon range"
        );
        ensure!(
            !self.is_artillery(),
            "Artillery requires artillery aim rules"
        );
        let mut profile = self.profile_for_ammunition(ammunition);
        let minimum = profile.minimum_range;
        if mode == BattleFireMode::Hotload {
            ensure!(self.supports_hotload(), "Weapon cannot be hotloaded");
            profile.minimum_range = 0;
        }
        let maximum = self.effective_range_for_ammunition(extended, ammunition)
            + if ammunition.munition() == BattleAmmunitionMode::Stinger {
                7
            } else {
                0
            };
        if distance > f64::from(maximum) {
            return Ok(None);
        }
        if mode == BattleFireMode::Hotload
            && half_minimum
            && minimum > 0
            && distance <= f64::from(minimum)
        {
            return Ok(Some(BattleWeaponRange {
                bracket: BattleRangeBracket::Short,
                modifier: ((f64::from(minimum) - distance + 2.0) / 2.0).floor() as u8,
            }));
        }
        if profile.minimum_range > 0 && distance <= f64::from(profile.minimum_range) {
            return Ok(Some(BattleWeaponRange {
                bracket: BattleRangeBracket::Minimum,
                modifier: (f64::from(profile.minimum_range) - distance + 1.0).floor() as u8,
            }));
        }
        let rounded = (distance + 0.95).floor() as u8;
        let (bracket, modifier) = if rounded > profile.long_range {
            (BattleRangeBracket::Extreme, 8)
        } else if rounded > profile.medium_range {
            (BattleRangeBracket::Long, 4)
        } else if rounded > profile.short_range {
            (BattleRangeBracket::Medium, 2)
        } else if profile.minimum_range > 0 && rounded <= profile.minimum_range {
            (
                BattleRangeBracket::Minimum,
                profile.minimum_range - rounded + 1,
            )
        } else {
            (BattleRangeBracket::Short, 0)
        };
        Ok(Some(BattleWeaponRange { bracket, modifier }))
    }
}
