//! Woodland attack outcomes, independent of map storage and command publication.
use super::{AmmunitionMode, Dice, Foliage, Ground, Hex, Weapon};
use serde::{Deserialize, Serialize};

/// Purpose of a terrain effect; incidental effects use the lower accidental ignition chance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WoodlandIntent {
    Ignite,
    Clear,
    Incidental,
}

/// Detached effect to apply inside the enclosing attack transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum WoodlandEffect {
    None,
    /// Temporary fire must retain the underlying woodland tile.
    Ignite {
        seconds: u16,
    },
    /// Woods cut back, keeping the hex's height.
    Clear {
        clearing: WoodlandClearing,
    },
}

/// How clearing changes a wooded hex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WoodlandClearing {
    /// Heavy or ultra-heavy woods or jungle thin by one density.
    Thin,
    /// Light woods or jungle, or planted fields, are cut down to clear ground.
    CutToClear,
    /// Light woods or jungle, or planted fields, are cut down, leaving rough ground.
    CutToRough,
}

impl WoodlandClearing {
    /// The hex after clearing, or `None` when its foliage cannot be cleared this way.
    pub fn apply(self, hex: Hex) -> Option<Hex> {
        let foliage = hex.foliage()?;
        let thinned = foliage.thinned();
        Some(match (self, thinned) {
            (Self::Thin, Some(thinner)) => hex.with_foliage(Some(thinner)),
            (Self::CutToClear, None) => hex.with_foliage(None),
            (Self::CutToRough, None) => hex.with_foliage(None).with_ground(Ground::Rough),
            _ => return None,
        })
    }
}

/// The roll a weapon needs to ignite woodland.
pub trait TerrainIgnition {
    /// Required ignition roll. Ammunition overrides precede ordinary weapon exclusions.
    fn terrain_ignition_target(self, ammunition: AmmunitionMode) -> Option<u8>;
}

impl TerrainIgnition for Weapon {
    fn terrain_ignition_target(self, ammunition: AmmunitionMode) -> Option<u8> {
        if matches!(self, Self::Flamer | Self::ClanFlamer | Self::HeavyFlamer) {
            return Some(4);
        }
        if ammunition == AmmunitionMode::Inferno && self.profile().missiles > 0 {
            return Some(5);
        }
        if ammunition == AmmunitionMode::Flechette
            && self.gunnery_skill(true) == "Gunnery-Ballistic"
        {
            return Some(5);
        }
        if !self.can_ignite_terrain() {
            return None;
        }
        Some(if self.gunnery_skill(true) == "Gunnery-Laser" {
            5
        } else {
            9
        })
    }
}

/// Resolve woodland checks against `hex` using the caller's candidate dice stream.
/// This does not authorize fire, mutate a map, clear mines, or publish notifications.
/// Only woods that are not already burning can be ignited or cleared; other hexes still
/// consume the applicable attack checks. Successful clearing only draws its replacement die
/// for light woods. Fire duration is inclusive 60–180.
pub fn resolve_woodland_effect(
    hex: Hex,
    weapon: Weapon,
    ammunition: AmmunitionMode,
    damage: u16,
    intent: WoodlandIntent,
    dice: &mut Dice,
) -> WoodlandEffect {
    let woods = hex.is_woods() && !hex.is_burning();
    if intent != WoodlandIntent::Ignite {
        let ignition_roll = dice.generic_roll();
        let clearing_roll = dice.generic_roll();
        let threshold = if intent == WoodlandIntent::Clear {
            5
        } else {
            3
        };
        if ignition_roll > threshold {
            if !woods || !weapon.can_clear_terrain() || u16::from(clearing_roll) > damage {
                return WoodlandEffect::None;
            }
            let clearing = if hex.foliage().and_then(Foliage::thinned).is_some() {
                WoodlandClearing::Thin
            } else if dice.die(2).expect("nonzero die") == 1 {
                WoodlandClearing::CutToRough
            } else {
                WoodlandClearing::CutToClear
            };
            return WoodlandEffect::Clear { clearing };
        }
    }
    let roll = dice.generic_roll();
    if !woods
        || weapon
            .terrain_ignition_target(ammunition)
            .is_none_or(|target| roll < target)
    {
        return WoodlandEffect::None;
    }
    WoodlandEffect::Ignite {
        seconds: 59 + dice.die(121).expect("nonzero die"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::Terrain;

    /// The generic histogram excludes duration and replacement dice, including on bare terrain.
    #[test]
    fn woodland_journal_counts_checks_without_counting_effect_dice() {
        let mut observed = std::collections::BTreeSet::new();
        for seed in 0..=255 {
            for terrain in [Terrain::Clear, Terrain::LightWoods, Terrain::HeavyWoods] {
                for intent in [
                    WoodlandIntent::Ignite,
                    WoodlandIntent::Clear,
                    WoodlandIntent::Incidental,
                ] {
                    let mut actual = Dice::seeded([seed; 32]);
                    let mut expected = actual.clone();
                    let mut histogram = crate::RollStatistics::default();
                    let ignition = if intent == WoodlandIntent::Ignite {
                        true
                    } else {
                        let first = expected.two_d6();
                        histogram.record(first).unwrap();
                        histogram.record(expected.two_d6()).unwrap();
                        first
                            <= if intent == WoodlandIntent::Clear {
                                5
                            } else {
                                3
                            }
                    };
                    if ignition {
                        let roll = expected.two_d6();
                        histogram.record(roll).unwrap();
                        if terrain != Terrain::Clear && roll >= 5 {
                            expected.die(121).unwrap();
                        }
                    } else if terrain == Terrain::LightWoods {
                        expected.die(2).unwrap();
                    }
                    resolve_woodland_effect(
                        Hex::new(terrain, 0),
                        Weapon::MediumLaser,
                        AmmunitionMode::Normal,
                        100,
                        intent,
                        &mut actual,
                    );
                    assert_eq!(
                        actual.generic_roll_statistics(),
                        &histogram,
                        "seed={seed}, terrain={terrain:?}, intent={intent:?}"
                    );
                    assert_eq!(actual, expected);
                    observed.insert(histogram.total());
                }
            }
        }
        assert_eq!(observed, [1, 2, 3].into_iter().collect());
    }

    #[test]
    fn weapon_capabilities_distinguish_ignition_from_clearing() {
        for weapon in [Weapon::GaussRifle, Weapon::ClanGaussRifle] {
            assert!(!weapon.can_ignite_terrain());
            assert!(weapon.can_clear_terrain());
        }
        for weapon in [Weapon::Ac2, Weapon::Ac5, Weapon::ClanLbx5] {
            assert!(weapon.can_ignite_terrain());
            assert!(!weapon.can_clear_terrain());
        }
        for weapon in [Weapon::SmallLaser, Weapon::ClanErSmallLaser, Weapon::Srm2] {
            assert!(!weapon.can_ignite_terrain());
            assert!(!weapon.can_clear_terrain());
        }
        for (weapon, target) in [
            (Weapon::Flamer, 4),
            (Weapon::HeavyFlamer, 4),
            (Weapon::MediumLaser, 5),
            (Weapon::Ac10, 9),
            (Weapon::VehicleFlamer, 9),
        ] {
            assert_eq!(
                weapon.terrain_ignition_target(AmmunitionMode::Normal),
                Some(target)
            );
        }
        assert_eq!(
            Weapon::Ac10.terrain_ignition_target(AmmunitionMode::Flechette),
            Some(5)
        );
        assert_eq!(
            Weapon::Ac10.terrain_ignition_target(AmmunitionMode::Incendiary),
            Some(9)
        );
    }

    #[test]
    fn ignition_preserves_replay_and_only_affects_woods() {
        let mut fires = 0;
        for seed in 0..=255 {
            let original = Dice::seeded([seed; 32]);
            let mut dice = original.clone();
            let mut expected = original.clone();
            let roll = expected.two_d6();
            let result = resolve_woodland_effect(
                Hex::new(Terrain::HeavyWoods, 0),
                Weapon::MediumLaser,
                AmmunitionMode::Normal,
                5,
                WoodlandIntent::Ignite,
                &mut dice,
            );
            if roll >= 5 {
                let seconds = 59 + expected.die(121).unwrap();
                assert_eq!(result, WoodlandEffect::Ignite { seconds });
                assert!((60..=180).contains(&seconds));
                fires += 1;
            } else {
                assert_eq!(result, WoodlandEffect::None);
            }
            assert_eq!(dice, expected);
            for terrain in [
                Terrain::Building,
                Terrain::Water,
                Terrain::Clear,
                Terrain::Fire,
            ] {
                let mut dice = original.clone();
                let mut expected = original.clone();
                expected.two_d6();
                assert_eq!(
                    resolve_woodland_effect(
                        Hex::new(terrain, 0),
                        Weapon::Flamer,
                        AmmunitionMode::Normal,
                        100,
                        WoodlandIntent::Ignite,
                        &mut dice
                    ),
                    WoodlandEffect::None
                );
                assert_eq!(dice, expected);
            }
        }
        assert!(fires > 0 && fires < 256);
    }

    #[test]
    fn accidental_ignition_preempts_clearing_and_density_reduction_is_bounded() {
        let mut preempted = false;
        let mut cleared = false;
        let mut surfaces = std::collections::BTreeSet::new();
        for seed in 0..=255 {
            let original = Dice::seeded([seed; 32]);
            let mut preview = original.clone();
            let ignition_roll = preview.two_d6();
            for intent in [WoodlandIntent::Clear, WoodlandIntent::Incidental] {
                let mut dice = original.clone();
                // Gauss can clear woods but cannot ignite them, even on the ignition branch.
                let effect = resolve_woodland_effect(
                    Hex::new(Terrain::HeavyWoods, 0),
                    Weapon::GaussRifle,
                    AmmunitionMode::Normal,
                    15,
                    intent,
                    &mut dice,
                );
                let threshold = if intent == WoodlandIntent::Clear {
                    5
                } else {
                    3
                };
                if ignition_roll <= threshold {
                    assert_eq!(effect, WoodlandEffect::None);
                    preempted = true;
                } else {
                    assert_eq!(
                        effect,
                        WoodlandEffect::Clear {
                            clearing: WoodlandClearing::Thin
                        }
                    );
                    cleared = true;
                }
                let mut replay = original.clone();
                assert_eq!(
                    effect,
                    resolve_woodland_effect(
                        Hex::new(Terrain::HeavyWoods, 0),
                        Weapon::GaussRifle,
                        AmmunitionMode::Normal,
                        15,
                        intent,
                        &mut replay
                    )
                );
                assert_eq!(dice, replay);
                let light = resolve_woodland_effect(
                    Hex::new(Terrain::LightWoods, 0),
                    Weapon::GaussRifle,
                    AmmunitionMode::Normal,
                    15,
                    intent,
                    &mut original.clone(),
                );
                if let WoodlandEffect::Clear { clearing } = light {
                    surfaces.insert(clearing);
                }
            }
        }
        assert!(preempted && cleared);
        assert_eq!(
            surfaces,
            [WoodlandClearing::CutToClear, WoodlandClearing::CutToRough]
                .into_iter()
                .collect()
        );
    }
}
