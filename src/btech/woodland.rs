//! Woodland attack outcomes, independent of map storage and command publication.
//!
//! Woods, jungle and crops wear down under fire by Total Warfare's Terrain Factor (p. 112): a
//! hex takes damage until its factor is spent, then heavy growth thins a density, light growth
//! falls to rough ground and crops are flattened. Fires start by Tactical Operations' starting
//! fires table (p. 41), with a target number by weapon class and a modifier for the ground.
use super::{AmmunitionMode, Condition, Dice, Foliage, Ground, Hex, Weapon};
use serde::{Deserialize, Serialize};

/// Purpose of a terrain effect. Only a shot aimed at the hex clears its growth; a stray shot
/// can still start a fire; a shot the trees absorb on its way to a unit wears them down
/// without lighting them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WoodlandIntent {
    Ignite,
    Clear,
    Incidental,
    Absorb,
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
    /// The hex's growth took damage but still stands; `total` is what it has taken so far.
    Damage {
        total: u16,
    },
}

/// How clearing changes a wooded hex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WoodlandClearing {
    /// Heavy or ultra-heavy woods or jungle thin by one density.
    Thin,
    /// Planted fields are flattened to clear ground.
    CutToClear,
    /// Light woods or jungle are felled, leaving rough ground.
    CutToRough,
}

impl WoodlandClearing {
    /// How spent terrain factor reduces `foliage` (Tactical Operations' terrain factor and
    /// conversion table).
    pub fn of(foliage: Foliage) -> Self {
        if foliage.thinned().is_some() {
            return Self::Thin;
        }
        if foliage.density().is_some() {
            return Self::CutToRough;
        }
        Self::CutToClear
    }

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
    /// Required ignition roll on 2d6 before terrain modifiers, or `None` for a weapon that
    /// cannot start a fire. Zero means the munition sets the hex alight without a roll.
    fn terrain_ignition_target(self, ammunition: AmmunitionMode) -> Option<u8>;
}

impl TerrainIgnition for Weapon {
    fn terrain_ignition_target(self, ammunition: AmmunitionMode) -> Option<u8> {
        if ammunition == AmmunitionMode::Inferno || self.is_plasma() {
            return Some(0);
        }
        if matches!(self, Self::Flamer | Self::ClanFlamer | Self::HeavyFlamer) {
            return Some(4);
        }
        if !self.can_ignite_terrain() || self.is_artillery() {
            return None;
        }
        Some(if self.is_energy() { 7 } else { 9 })
    }
}

/// What the ground adds to the roll to start a fire in `hex`, or `None` where nothing burns.
pub fn ignition_terrain_modifier(hex: Hex) -> Option<i16> {
    let foliage = hex.foliage()?;
    let mut modifier = match foliage {
        Foliage::PlantedFields => -1,
        _ if foliage.is_jungle() => 1,
        _ => 0,
    };
    if hex.condition() == Some(Condition::DeepSnow) {
        modifier += 2;
    }
    if hex.ground() == Ground::HeavyIndustrial {
        modifier += 2;
    }
    Some(modifier)
}

/// Resolve woodland checks against `hex` using the caller's candidate dice stream.
/// `terrain_damage` is what the hex's growth has already taken. This does not authorize fire,
/// mutate a map, clear mines, or publish notifications.
///
/// A shot aimed at the hex, or absorbed by its trees, adds its damage to the terrain factor
/// and clears the growth once the factor is spent; an aimed shot that leaves the growth
/// standing, and a stray shot, lets a weapon that can start fires roll 2d6 against its
/// starting-fire number plus the ground's modifier. Burning growth neither clears nor ignites
/// again. Fire duration is inclusive 60–180 seconds.
pub fn resolve_woodland_effect(
    hex: Hex,
    weapon: Weapon,
    ammunition: AmmunitionMode,
    damage: u16,
    intent: WoodlandIntent,
    terrain_damage: u16,
    dice: &mut Dice,
) -> WoodlandEffect {
    let Some(foliage) = hex.foliage() else {
        return WoodlandEffect::None;
    };
    if hex.is_burning() {
        return WoodlandEffect::None;
    }
    if matches!(intent, WoodlandIntent::Clear | WoodlandIntent::Absorb)
        && weapon.can_clear_terrain()
        && damage > 0
    {
        let total = terrain_damage.saturating_add(damage);
        if total >= foliage.terrain_factor() {
            return WoodlandEffect::Clear {
                clearing: WoodlandClearing::of(foliage),
            };
        }
        if intent == WoodlandIntent::Absorb {
            return WoodlandEffect::Damage { total };
        }
        let Some(target) = weapon.terrain_ignition_target(ammunition) else {
            return WoodlandEffect::Damage { total };
        };
        if ignites(hex, target, dice) {
            return WoodlandEffect::Ignite {
                seconds: 59 + dice.die(121).expect("nonzero die"),
            };
        }
        return WoodlandEffect::Damage { total };
    }
    let Some(target) = weapon.terrain_ignition_target(ammunition) else {
        return WoodlandEffect::None;
    };
    if !ignites(hex, target, dice) {
        return WoodlandEffect::None;
    }
    WoodlandEffect::Ignite {
        seconds: 59 + dice.die(121).expect("nonzero die"),
    }
}

/// Roll to start a fire in `hex` against `target`; zero needs no roll.
fn ignites(hex: Hex, target: u8, dice: &mut Dice) -> bool {
    if target == 0 {
        return true;
    }
    let modifier = ignition_terrain_modifier(hex).unwrap_or(0);
    i16::from(dice.generic_roll()) >= i16::from(target) + modifier
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::Terrain;

    /// Starting-fire numbers follow the Tactical Operations table by weapon class.
    #[test]
    fn ignition_targets_follow_the_starting_fires_table() {
        for (weapon, target) in [
            (Weapon::Flamer, Some(4)),
            (Weapon::HeavyFlamer, Some(4)),
            (Weapon::MediumLaser, Some(7)),
            (Weapon::Ppc, Some(7)),
            (Weapon::Ac10, Some(9)),
            (Weapon::Lrm20, Some(9)),
            (Weapon::VehicleFlamer, Some(7)),
            (Weapon::PlasmaRifle, Some(0)),
            (Weapon::ClanPlasmaCannon, Some(0)),
            (Weapon::GaussRifle, None),
            (Weapon::SmallLaser, None),
            (Weapon::Srm2, None),
            (Weapon::LongTom, None),
        ] {
            assert_eq!(
                weapon.terrain_ignition_target(AmmunitionMode::Normal),
                target,
                "{weapon:?}"
            );
        }
        assert_eq!(
            Weapon::Srm6.terrain_ignition_target(AmmunitionMode::Inferno),
            Some(0)
        );
        assert_eq!(
            Weapon::Ac10.terrain_ignition_target(AmmunitionMode::Precision),
            Some(9)
        );
    }

    /// Crops catch more easily, jungle, deep snow and factories less, and bare ground never.
    #[test]
    fn ground_modifies_the_ignition_roll() {
        assert_eq!(
            ignition_terrain_modifier(Hex::new(Terrain::PlantedFields, 0)),
            Some(-1)
        );
        assert_eq!(
            ignition_terrain_modifier(Hex::new(Terrain::HeavyWoods, 0)),
            Some(0)
        );
        assert_eq!(
            ignition_terrain_modifier(Hex::new(Terrain::LightJungle, 0)),
            Some(1)
        );
        let snowy = Hex::new(Terrain::LightWoods, 0).with_condition(Some(Condition::DeepSnow));
        assert_eq!(ignition_terrain_modifier(snowy), Some(2));
        assert_eq!(ignition_terrain_modifier(Hex::new(Terrain::Clear, 0)), None);
        assert_eq!(ignition_terrain_modifier(Hex::new(Terrain::Water, 2)), None);
    }

    /// Terrain factor accumulates across shots and clears the growth when spent.
    #[test]
    fn terrain_factor_accumulates_until_the_growth_falls() {
        let mut dice = Dice::seeded([7; 32]);
        let heavy = Hex::new(Terrain::HeavyWoods, 0);
        let effect = resolve_woodland_effect(
            heavy,
            Weapon::GaussRifle,
            AmmunitionMode::Normal,
            15,
            WoodlandIntent::Clear,
            0,
            &mut dice,
        );
        assert_eq!(effect, WoodlandEffect::Damage { total: 15 });
        let effect = resolve_woodland_effect(
            heavy,
            Weapon::GaussRifle,
            AmmunitionMode::Normal,
            15,
            WoodlandIntent::Clear,
            75,
            &mut dice,
        );
        assert_eq!(
            effect,
            WoodlandEffect::Clear {
                clearing: WoodlandClearing::Thin
            }
        );
        assert_eq!(
            WoodlandClearing::Thin.apply(heavy).unwrap().terrain(),
            Terrain::LightWoods
        );
        let light = Hex::new(Terrain::LightWoods, 0);
        assert_eq!(
            resolve_woodland_effect(
                light,
                Weapon::GaussRifle,
                AmmunitionMode::Normal,
                50,
                WoodlandIntent::Clear,
                0,
                &mut dice,
            ),
            WoodlandEffect::Clear {
                clearing: WoodlandClearing::CutToRough
            }
        );
        assert_eq!(
            WoodlandClearing::CutToRough.apply(light).unwrap().terrain(),
            Terrain::Rough
        );
        let fields = Hex::new(Terrain::PlantedFields, 0);
        assert_eq!(
            resolve_woodland_effect(
                fields,
                Weapon::Ac20,
                AmmunitionMode::Normal,
                20,
                WoodlandIntent::Clear,
                10,
                &mut dice,
            ),
            WoodlandEffect::Clear {
                clearing: WoodlandClearing::CutToClear
            }
        );
        assert_eq!(
            WoodlandClearing::CutToClear
                .apply(fields)
                .unwrap()
                .terrain(),
            Terrain::Clear
        );
        // Small weapons cannot fell trees, and stray shots never count against the factor.
        for (weapon, intent) in [
            (Weapon::MachineGun, WoodlandIntent::Clear),
            (Weapon::GaussRifle, WoodlandIntent::Incidental),
        ] {
            assert_eq!(
                resolve_woodland_effect(
                    heavy,
                    weapon,
                    AmmunitionMode::Normal,
                    100,
                    intent,
                    0,
                    &mut Dice::seeded([1; 32]),
                ),
                WoodlandEffect::None
            );
        }
    }

    /// Ignition rolls follow the weapon's number plus the ground's modifier, and only woods
    /// that are not already burning catch.
    #[test]
    fn ignition_preserves_replay_and_only_affects_woods() {
        let mut fires = 0;
        for seed in 0..=255 {
            let original = Dice::seeded([seed; 32]);
            let mut dice = original.clone();
            let mut expected = original.clone();
            let roll = expected.two_d6();
            let result = resolve_woodland_effect(
                Hex::new(Terrain::HeavyJungle, 0),
                Weapon::MediumLaser,
                AmmunitionMode::Normal,
                5,
                WoodlandIntent::Ignite,
                0,
                &mut dice,
            );
            if roll >= 8 {
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
                assert_eq!(
                    resolve_woodland_effect(
                        Hex::new(terrain, 0),
                        Weapon::Flamer,
                        AmmunitionMode::Normal,
                        100,
                        WoodlandIntent::Ignite,
                        0,
                        &mut dice
                    ),
                    WoodlandEffect::None
                );
                assert_eq!(dice, original);
            }
        }
        assert!(fires > 0 && fires < 256);
        // Infernos need no roll.
        let mut dice = Dice::seeded([3; 32]);
        assert!(matches!(
            resolve_woodland_effect(
                Hex::new(Terrain::LightWoods, 0),
                Weapon::Srm6,
                AmmunitionMode::Inferno,
                0,
                WoodlandIntent::Incidental,
                0,
                &mut dice,
            ),
            WoodlandEffect::Ignite { .. }
        ));
    }
}
