//! Underwater weapon catalogue and rounded range brackets, independent of unit anatomy.
use crate::Weapon;
use crate::{RangeBracket, WeaponRange};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Water-specific bands. Some small lasers have no long band or extreme extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct WaterRanges {
    pub minimum_range: u8,
    pub short_range: u8,
    pub medium_range: u8,
    pub long_range: Option<u8>,
}

impl WaterRanges {
    /// Missing long range falls back to medium, without enabling extreme range.
    pub fn effective_range(self, extended: bool) -> u16 {
        match self.long_range {
            Some(long) if extended => u16::from(long).max(u16::from(self.medium_range) * 2),
            Some(long) => u16::from(long),
            None => u16::from(self.medium_range),
        }
    }
}

impl Weapon {
    /// Explicit catalogue eligibility; energy classification alone does not permit water fire.
    pub fn water_ranges(self) -> Option<WaterRanges> {
        if self.is_torpedo() {
            // Torpedoes are built for water and keep their ordinary reach.
            let profile = self.profile();
            return Some(WaterRanges {
                minimum_range: profile.minimum_range,
                short_range: profile.short_range,
                medium_range: profile.medium_range,
                long_range: Some(profile.long_range),
            });
        }
        let (minimum_range, short_range, medium_range, long_range) = match self {
            Self::ClanErLargeLaser => (0, 5, 10, Some(15)),
            Self::ClanErMediumLaser => (0, 3, 7, Some(10)),
            Self::ClanErSmallLaser | Self::ClanSmallPulseLaser => (0, 1, 2, Some(4)),
            Self::ClanErMicroLaser | Self::ClanMicroPulseLaser => (0, 1, 2, Some(2)),
            Self::ClanErPpc | Self::ErPpc | Self::ClanPlasmaRifle => (0, 4, 10, Some(16)),
            Self::ClanHeavyLargeLaser | Self::LargeLaser | Self::XLargePulseLaser => {
                (0, 3, 6, Some(9))
            }
            Self::ClanHeavyMediumLaser | Self::MediumLaser | Self::XMediumPulseLaser => {
                (0, 2, 4, Some(6))
            }
            Self::ClanHeavySmallLaser | Self::SmallLaser | Self::SmallPulseLaser => (0, 1, 2, None),
            Self::ClanLargePulseLaser => (0, 4, 10, Some(14)),
            Self::ClanMediumPulseLaser => (0, 3, 5, Some(8)),
            Self::ClanErLargePulseLaser => (0, 4, 10, Some(16)),
            Self::ClanErMediumPulseLaser => (0, 3, 6, Some(8)),
            Self::ClanErSmallPulseLaser => (0, 2, 3, Some(4)),
            Self::Ppc => (3, 4, 7, Some(10)),
            Self::ErLargeLaser => (0, 3, 5, Some(12)),
            Self::ErMediumLaser => (0, 3, 5, Some(8)),
            Self::ErSmallLaser => (0, 1, 2, Some(3)),
            Self::LargePulseLaser => (0, 2, 5, Some(7)),
            Self::MediumPulseLaser => (0, 2, 3, Some(4)),
            Self::SnubNosedPpc => (0, 6, 8, Some(9)),
            Self::LightPpc | Self::HeavyPpc => (3, 4, 8, Some(10)),
            _ => return None,
        };
        Some(WaterRanges {
            minimum_range,
            short_range,
            medium_range,
            long_range,
        })
    }

    /// Underwater accuracy uses rounded distance even at the maximum and minimum boundaries.
    /// Unsupported weapons are errors; a permitted weapon beyond its reach returns `None`.
    pub fn water_range_modifier(
        self,
        distance: f64,
        extended: bool,
    ) -> Result<Option<WeaponRange>> {
        ensure!(
            distance.is_finite() && distance >= 0.0,
            "Invalid weapon range"
        );
        let profile = self
            .water_ranges()
            .context("This weapon may not be fired underwater.")?;
        let range = (distance + 0.95).floor();
        if range > f64::from(profile.effective_range(extended)) {
            return Ok(None);
        }
        let range = range as u8;
        let (bracket, modifier) = if u16::from(range) > profile.effective_range(false) {
            (RangeBracket::Extreme, 8)
        } else if range > profile.medium_range {
            (RangeBracket::Long, 4)
        } else if range > profile.short_range {
            (RangeBracket::Medium, 2)
        } else if range > profile.minimum_range {
            (RangeBracket::Short, 0)
        } else if range == 0 {
            (RangeBracket::Short, profile.minimum_range)
        } else {
            // Positive minimum-range water shots fall through to ordinary
            // minimum arithmetic in the reference; keep that catalogue input.
            (
                RangeBracket::Short,
                self.profile().minimum_range.saturating_sub(range) + 1,
            )
        };
        Ok(Some(WeaponRange { bracket, modifier }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Water eligibility is narrower than energy weapons, and differs between plasma designs.
    /// The fourteen torpedo launchers are the only missiles that fire underwater.
    #[test]
    fn catalogue_admits_only_the_46_water_profiles() {
        assert_eq!(
            Weapon::ALL
                .iter()
                .filter(|w| w.water_ranges().is_some())
                .count(),
            46
        );
        for weapon in [
            Weapon::Flamer,
            Weapon::PlasmaRifle,
            Weapon::XSmallPulseLaser,
            Weapon::Ac20,
            Weapon::Srm6,
        ] {
            assert!(weapon.water_ranges().is_none());
            assert_eq!(
                weapon
                    .water_range_modifier(0.0, true)
                    .unwrap_err()
                    .to_string(),
                "This weapon may not be fired underwater."
            );
        }
        assert_eq!(
            Weapon::ClanPlasmaRifle
                .water_ranges()
                .unwrap()
                .effective_range(true),
            20
        );
    }

    /// An absent long band caps small lasers at medium even with extended ranges enabled.
    #[test]
    fn missing_long_band_does_not_create_extreme_range() {
        for weapon in [
            Weapon::SmallLaser,
            Weapon::SmallPulseLaser,
            Weapon::ClanHeavySmallLaser,
        ] {
            for extended in [false, true] {
                let range = weapon
                    .water_range_modifier(2.04, extended)
                    .unwrap()
                    .unwrap();
                assert_eq!(
                    range,
                    WeaponRange {
                        bracket: RangeBracket::Medium,
                        modifier: 2
                    }
                );
                assert!(
                    weapon
                        .water_range_modifier(2.06, extended)
                        .unwrap()
                        .is_none()
                );
            }
        }
    }

    /// Water range rounds before every band test, unlike ordinary raw-distance limits.
    #[test]
    fn rounded_bands_and_ppc_minimum_match_water_rules() {
        for (distance, bracket, modifier) in [
            (0.0, RangeBracket::Short, 3),
            (0.04, RangeBracket::Short, 3),
            (1.0, RangeBracket::Short, 3),
            (2.0, RangeBracket::Short, 2),
            (3.04, RangeBracket::Short, 1),
            (3.06, RangeBracket::Short, 0),
            (4.06, RangeBracket::Medium, 2),
            (7.06, RangeBracket::Long, 4),
            (10.04, RangeBracket::Long, 4),
            (10.06, RangeBracket::Extreme, 8),
        ] {
            assert_eq!(
                Weapon::Ppc.water_range_modifier(distance, true).unwrap(),
                Some(WeaponRange { bracket, modifier })
            );
        }
        assert!(
            Weapon::Ppc
                .water_range_modifier(10.06, false)
                .unwrap()
                .is_none()
        );
        assert!(
            Weapon::Ppc
                .water_range_modifier(14.06, true)
                .unwrap()
                .is_none()
        );
        assert_eq!(
            Weapon::MediumLaser
                .water_range_modifier(0.0, false)
                .unwrap()
                .unwrap()
                .modifier,
            0
        );
    }

    /// Bad numeric inputs cannot saturate into an apparently valid short-range shot.
    #[test]
    fn invalid_distances_are_rejected_and_large_distances_miss() {
        for distance in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -0.01] {
            assert!(
                Weapon::MediumLaser
                    .water_range_modifier(distance, true)
                    .is_err()
            );
        }
        assert!(
            Weapon::MediumLaser
                .water_range_modifier(f64::MAX, true)
                .unwrap()
                .is_none()
        );
    }
}
