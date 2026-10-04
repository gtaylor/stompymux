//! Target-side missile admission is distinct from the launcher's near-miss feedback threshold.
use super::{AmmunitionMode, BeaconLaunch, GlancingMode, Weapon};

/// Whether a launched roll reaches defenses/material and how it modifies those damage packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TargetHit {
    pub hit: bool,
    pub glancing: bool,
}

impl TargetHit {
    /// Missiles require the base target number even when launch feedback admits a near miss.
    /// Their boundary cluster modifier also uses that base; other weapons keep launch classification.
    pub(super) fn for_weapon(
        self,
        weapon: Weapon,
        ammunition: AmmunitionMode,
        roll: u8,
        base: Option<i32>,
        mode: GlancingMode,
        streak_confused: bool,
    ) -> Self {
        if weapon.gunnery_skill(true) != "Gunnery-Missile" {
            return self;
        }
        let hit = self.hit && base.is_some_and(|base| i32::from(roll) >= base);
        Self {
            hit,
            glancing: hit
                && weapon.beacon_kind(ammunition).is_none()
                && mode != GlancingMode::Disabled
                && base == Some(i32::from(roll))
                && (!weapon.is_streak() || streak_confused),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Missile effects use the base boundary in every glancing mode, independently of launch feedback.
    #[test]
    fn missile_boundary_does_not_follow_near_miss_classification() {
        for mode in [
            GlancingMode::Disabled,
            GlancingMode::AtTarget,
            GlancingMode::BelowTarget,
        ] {
            for roll in [5, 6, 7] {
                let threshold = mode.threshold(6);
                let launch = TargetHit {
                    hit: i32::from(roll) >= threshold,
                    glancing: mode != GlancingMode::Disabled && i32::from(roll) == threshold,
                };
                for weapon in [Weapon::Lrm20, Weapon::ClanLrm20, Weapon::Srm6] {
                    assert_eq!(
                        launch.for_weapon(
                            weapon,
                            AmmunitionMode::Normal,
                            roll,
                            Some(6),
                            mode,
                            false
                        ),
                        TargetHit {
                            hit: roll >= 6,
                            glancing: roll == 6 && mode != GlancingMode::Disabled,
                        }
                    );
                    assert_eq!(
                        launch.for_weapon(weapon, AmmunitionMode::Normal, roll, None, mode, false),
                        TargetHit {
                            hit: false,
                            glancing: false
                        }
                    );
                }
                assert_eq!(
                    launch.for_weapon(
                        Weapon::SmallLaser,
                        AmmunitionMode::Normal,
                        roll,
                        Some(6),
                        mode,
                        false
                    ),
                    launch
                );
            }
        }
    }

    /// Beacon pods do not glance; intact Streak guidance retains its full cluster at the boundary.
    #[test]
    fn pod_and_streak_boundaries_preserve_special_effects() {
        let launch = TargetHit {
            hit: true,
            glancing: false,
        };
        for weapon in [Weapon::NarcBeacon, Weapon::StreakSrm6] {
            assert_eq!(
                launch.for_weapon(
                    weapon,
                    AmmunitionMode::Normal,
                    6,
                    Some(6),
                    GlancingMode::BelowTarget,
                    false
                ),
                TargetHit {
                    hit: true,
                    glancing: false
                }
            );
        }
        assert_eq!(
            launch.for_weapon(
                Weapon::StreakSrm6,
                AmmunitionMode::Normal,
                6,
                Some(6),
                GlancingMode::BelowTarget,
                true
            ),
            TargetHit {
                hit: true,
                glancing: true
            }
        );
    }
}
