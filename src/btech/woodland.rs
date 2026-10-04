//! Woodland attack outcomes, independent of map storage and command publication.
use super::{BattleAmmunitionMode, BattleDice, BattleWeapon, Ground, Hex, Woods};
use serde::{Deserialize, Serialize};

/// Purpose of a terrain effect; incidental effects use the lower accidental ignition chance.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleWoodlandIntent {
    Ignite,
    Clear,
    Incidental,
}

/// Detached effect to apply inside the enclosing attack transaction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "effect", rename_all = "snake_case")]
pub enum BattleWoodlandEffect {
    None,
    /// Temporary fire must retain the underlying woodland tile.
    Ignite {
        seconds: u16,
    },
    /// Woods cut back, keeping the hex's height.
    Clear {
        clearing: BattleWoodlandClearing,
    },
}

/// How clearing changes a wooded hex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleWoodlandClearing {
    /// Heavy woods thin to light woods.
    ThinToLight,
    /// Light woods are cut down to clear ground.
    CutToClear,
    /// Light woods are cut down, leaving rough ground.
    CutToRough,
}

impl BattleWoodlandClearing {
    /// The hex after clearing, or `None` when its woods cannot be cleared this way.
    pub fn apply(self, hex: Hex) -> Option<Hex> {
        Some(match (self, hex.woods()?) {
            (Self::ThinToLight, Woods::Heavy) => hex.with_woods(Some(Woods::Light)),
            (Self::CutToClear, Woods::Light) => hex.with_woods(None),
            (Self::CutToRough, Woods::Light) => hex.with_woods(None).with_ground(Ground::Rough),
            _ => return None,
        })
    }
}

impl BattleWeapon {
    /// Whether ordinary ammunition can start a woodland fire.
    pub fn can_ignite_terrain(self) -> bool {
        !matches!(
            self,
            Self::INarcBeacon
                | Self::NarcBeacon
                | Self::ClanNarcBeacon
                | Self::ClanSrm2
                | Self::ClanStreakSrm2
                | Self::ClanGaussRifle
                | Self::ClanMachineGun
                | Self::ClanLightMachineGun
                | Self::ClanHeavyMachineGun
                | Self::ClanErSmallLaser
                | Self::ClanHeavySmallLaser
                | Self::ClanSmallPulseLaser
                | Self::ClanErSmallPulseLaser
                | Self::MachineGun
                | Self::HeavyMachineGun
                | Self::SmallLaser
                | Self::ErSmallLaser
                | Self::SmallPulseLaser
                | Self::XSmallPulseLaser
                | Self::Srm2
                | Self::StreakSrm2
                | Self::HeavyGaussRifle
                | Self::GaussRifle
                | Self::LightGaussRifle
                | Self::MagshotGaussRifle
        )
    }

    /// Whether a sufficiently damaging shot can reduce woodland density.
    pub fn can_clear_terrain(self) -> bool {
        !matches!(
            self,
            Self::ClanLbx2
                | Self::ClanLbx5
                | Self::ClanUltraAc2
                | Self::ClanUltraAc5
                | Self::ClanSrm2
                | Self::ClanStreakSrm2
                | Self::ClanMachineGun
                | Self::ClanLightMachineGun
                | Self::ClanHeavyMachineGun
                | Self::ClanErSmallLaser
                | Self::ClanHeavySmallLaser
                | Self::ClanSmallPulseLaser
                | Self::ClanErSmallPulseLaser
                | Self::HyperAc2
                | Self::HyperAc5
                | Self::MachineGun
                | Self::HeavyMachineGun
                | Self::LightAc2
                | Self::LightAc5
                | Self::SmallLaser
                | Self::ErSmallLaser
                | Self::SmallPulseLaser
                | Self::XSmallPulseLaser
                | Self::Srm2
                | Self::StreakSrm2
                | Self::Lbx2
                | Self::Lbx5
                | Self::Ac2
                | Self::Ac5
                | Self::UltraAc2
                | Self::UltraAc5
                | Self::RotaryAc2
                | Self::RotaryAc5
                | Self::ClanRotaryAc2
                | Self::ClanRotaryAc5
        )
    }

    /// Required ignition roll. Ammunition overrides precede ordinary weapon exclusions.
    pub fn terrain_ignition_target(self, ammunition: BattleAmmunitionMode) -> Option<u8> {
        if matches!(self, Self::Flamer | Self::ClanFlamer | Self::HeavyFlamer) {
            return Some(4);
        }
        if ammunition == BattleAmmunitionMode::Inferno && self.profile().missiles > 0 {
            return Some(5);
        }
        if ammunition == BattleAmmunitionMode::Flechette
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
    weapon: BattleWeapon,
    ammunition: BattleAmmunitionMode,
    damage: u16,
    intent: BattleWoodlandIntent,
    dice: &mut BattleDice,
) -> BattleWoodlandEffect {
    let woods = hex.is_woods() && !hex.is_burning();
    if intent != BattleWoodlandIntent::Ignite {
        let ignition_roll = dice.generic_roll();
        let clearing_roll = dice.generic_roll();
        let threshold = if intent == BattleWoodlandIntent::Clear {
            5
        } else {
            3
        };
        if ignition_roll > threshold {
            if !woods || !weapon.can_clear_terrain() || u16::from(clearing_roll) > damage {
                return BattleWoodlandEffect::None;
            }
            let clearing = if hex.woods() == Some(Woods::Heavy) {
                BattleWoodlandClearing::ThinToLight
            } else if dice.die(2).expect("nonzero die") == 1 {
                BattleWoodlandClearing::CutToRough
            } else {
                BattleWoodlandClearing::CutToClear
            };
            return BattleWoodlandEffect::Clear { clearing };
        }
    }
    let roll = dice.generic_roll();
    if !woods
        || weapon
            .terrain_ignition_target(ammunition)
            .is_none_or(|target| roll < target)
    {
        return BattleWoodlandEffect::None;
    }
    BattleWoodlandEffect::Ignite {
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
            for terrain in [
                Terrain::Grassland,
                Terrain::LightForest,
                Terrain::HeavyForest,
            ] {
                for intent in [
                    BattleWoodlandIntent::Ignite,
                    BattleWoodlandIntent::Clear,
                    BattleWoodlandIntent::Incidental,
                ] {
                    let mut actual = BattleDice::seeded([seed; 32]);
                    let mut expected = actual.clone();
                    let mut histogram = crate::BattleRollStatistics::default();
                    let ignition = if intent == BattleWoodlandIntent::Ignite {
                        true
                    } else {
                        let first = expected.two_d6();
                        histogram.record(first).unwrap();
                        histogram.record(expected.two_d6()).unwrap();
                        first
                            <= if intent == BattleWoodlandIntent::Clear {
                                5
                            } else {
                                3
                            }
                    };
                    if ignition {
                        let roll = expected.two_d6();
                        histogram.record(roll).unwrap();
                        if terrain != Terrain::Grassland && roll >= 5 {
                            expected.die(121).unwrap();
                        }
                    } else if terrain == Terrain::LightForest {
                        expected.die(2).unwrap();
                    }
                    resolve_woodland_effect(
                        Hex::new(terrain, 0),
                        BattleWeapon::MediumLaser,
                        BattleAmmunitionMode::Normal,
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
        for weapon in [BattleWeapon::GaussRifle, BattleWeapon::ClanGaussRifle] {
            assert!(!weapon.can_ignite_terrain());
            assert!(weapon.can_clear_terrain());
        }
        for weapon in [BattleWeapon::Ac2, BattleWeapon::Ac5, BattleWeapon::ClanLbx5] {
            assert!(weapon.can_ignite_terrain());
            assert!(!weapon.can_clear_terrain());
        }
        for weapon in [
            BattleWeapon::SmallLaser,
            BattleWeapon::ClanErSmallLaser,
            BattleWeapon::Srm2,
        ] {
            assert!(!weapon.can_ignite_terrain());
            assert!(!weapon.can_clear_terrain());
        }
        for (weapon, target) in [
            (BattleWeapon::Flamer, 4),
            (BattleWeapon::HeavyFlamer, 4),
            (BattleWeapon::MediumLaser, 5),
            (BattleWeapon::Ac10, 9),
            (BattleWeapon::VehicleFlamer, 9),
        ] {
            assert_eq!(
                weapon.terrain_ignition_target(BattleAmmunitionMode::Normal),
                Some(target)
            );
        }
        assert_eq!(
            BattleWeapon::Ac10.terrain_ignition_target(BattleAmmunitionMode::Flechette),
            Some(5)
        );
        assert_eq!(
            BattleWeapon::Ac10.terrain_ignition_target(BattleAmmunitionMode::Incendiary),
            Some(9)
        );
    }

    #[test]
    fn ignition_preserves_replay_and_only_affects_woods() {
        let mut fires = 0;
        for seed in 0..=255 {
            let original = BattleDice::seeded([seed; 32]);
            let mut dice = original.clone();
            let mut expected = original.clone();
            let roll = expected.two_d6();
            let result = resolve_woodland_effect(
                Hex::new(Terrain::HeavyForest, 0),
                BattleWeapon::MediumLaser,
                BattleAmmunitionMode::Normal,
                5,
                BattleWoodlandIntent::Ignite,
                &mut dice,
            );
            if roll >= 5 {
                let seconds = 59 + expected.die(121).unwrap();
                assert_eq!(result, BattleWoodlandEffect::Ignite { seconds });
                assert!((60..=180).contains(&seconds));
                fires += 1;
            } else {
                assert_eq!(result, BattleWoodlandEffect::None);
            }
            assert_eq!(dice, expected);
            for terrain in [
                Terrain::Building,
                Terrain::Water,
                Terrain::Grassland,
                Terrain::Fire,
            ] {
                let mut dice = original.clone();
                let mut expected = original.clone();
                expected.two_d6();
                assert_eq!(
                    resolve_woodland_effect(
                        Hex::new(terrain, 0),
                        BattleWeapon::Flamer,
                        BattleAmmunitionMode::Normal,
                        100,
                        BattleWoodlandIntent::Ignite,
                        &mut dice
                    ),
                    BattleWoodlandEffect::None
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
            let original = BattleDice::seeded([seed; 32]);
            let mut preview = original.clone();
            let ignition_roll = preview.two_d6();
            for intent in [
                BattleWoodlandIntent::Clear,
                BattleWoodlandIntent::Incidental,
            ] {
                let mut dice = original.clone();
                // Gauss can clear woods but cannot ignite them, even on the ignition branch.
                let effect = resolve_woodland_effect(
                    Hex::new(Terrain::HeavyForest, 0),
                    BattleWeapon::GaussRifle,
                    BattleAmmunitionMode::Normal,
                    15,
                    intent,
                    &mut dice,
                );
                let threshold = if intent == BattleWoodlandIntent::Clear {
                    5
                } else {
                    3
                };
                if ignition_roll <= threshold {
                    assert_eq!(effect, BattleWoodlandEffect::None);
                    preempted = true;
                } else {
                    assert_eq!(
                        effect,
                        BattleWoodlandEffect::Clear {
                            clearing: BattleWoodlandClearing::ThinToLight
                        }
                    );
                    cleared = true;
                }
                let mut replay = original.clone();
                assert_eq!(
                    effect,
                    resolve_woodland_effect(
                        Hex::new(Terrain::HeavyForest, 0),
                        BattleWeapon::GaussRifle,
                        BattleAmmunitionMode::Normal,
                        15,
                        intent,
                        &mut replay
                    )
                );
                assert_eq!(dice, replay);
                let light = resolve_woodland_effect(
                    Hex::new(Terrain::LightForest, 0),
                    BattleWeapon::GaussRifle,
                    BattleAmmunitionMode::Normal,
                    15,
                    intent,
                    &mut original.clone(),
                );
                if let BattleWoodlandEffect::Clear { clearing } = light {
                    surfaces.insert(clearing);
                }
            }
        }
        assert!(preempted && cleared);
        assert_eq!(
            surfaces,
            [
                BattleWoodlandClearing::CutToClear,
                BattleWoodlandClearing::CutToRough
            ]
            .into_iter()
            .collect()
        );
    }
}
