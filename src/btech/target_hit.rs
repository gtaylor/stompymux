//! Target-side missile admission is distinct from the launcher's near-miss feedback threshold.
use super::{AmmunitionMode, BeaconLaunch, Weapon};

/// Whether a launched roll reaches defenses/material and how it modifies those damage packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct TargetHit {
    pub hit: bool,
    pub glancing: bool,
}

impl TargetHit {
    /// Missiles require the base target number; a roll exactly on it is a glancing blow
    /// (Tactical Operations) unless the weapon is a beacon pod or a locked Streak launcher.
    pub(super) fn for_weapon(
        self,
        weapon: Weapon,
        ammunition: AmmunitionMode,
        roll: u8,
        base: Option<i32>,
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
                && base == Some(i32::from(roll))
                && (!weapon.is_streak() || streak_confused),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Missile effects use the base boundary independently of launch feedback.
    #[test]
    fn missile_boundary_follows_the_base_target_number() {
        for roll in [5, 6, 7] {
            let launch = TargetHit {
                hit: roll >= 6,
                glancing: roll == 6,
            };
            for weapon in [Weapon::Lrm20, Weapon::ClanLrm20, Weapon::Srm6] {
                assert_eq!(
                    launch.for_weapon(weapon, AmmunitionMode::Normal, roll, Some(6), false),
                    TargetHit {
                        hit: roll >= 6,
                        glancing: roll == 6,
                    }
                );
                assert_eq!(
                    launch.for_weapon(weapon, AmmunitionMode::Normal, roll, None, false),
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
                    false
                ),
                launch
            );
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
                launch.for_weapon(weapon, AmmunitionMode::Normal, 6, Some(6), false),
                TargetHit {
                    hit: true,
                    glancing: false
                }
            );
        }
        assert_eq!(
            launch.for_weapon(Weapon::StreakSrm6, AmmunitionMode::Normal, 6, Some(6), true),
            TargetHit {
                hit: true,
                glancing: true
            }
        );
    }
}
