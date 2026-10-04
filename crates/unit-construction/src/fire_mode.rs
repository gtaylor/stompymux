//! Firing modes a weapon can be set to, with their burst sizes, jam thresholds and launch heat.
use super::Weapon;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Supported live weapon behavior; normal is implicit when no override is stored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FireMode {
    #[default]
    Normal,
    Heat,
    Hotload,
    Ultra,
    Rapid,
    Rotary2,
    Rotary3,
    Rotary4,
    Rotary5,
    Rotary6,
    Gatling,
}

impl FireMode {
    /// Heat for one completed launch, shared by all carriers and their supply fallback modes.
    pub fn launch_heat(self, weapon: Weapon, gatling_damage: Option<u8>, launched: bool) -> u8 {
        if !launched {
            return 0;
        }
        gatling_damage.unwrap_or(weapon.profile().heat * self.rounds_per_cycle() as u8)
    }

    /// Coolant heat mode redirects the shot to its carrier before target selection.
    pub fn self_cooling(self, weapon: Weapon) -> bool {
        self == Self::Heat && weapon == Weapon::CoolantGun
    }

    /// Whether this firing behavior is valid for the installed weapon family.
    pub fn supports(self, weapon: Weapon) -> bool {
        match self {
            Self::Normal => true,
            Self::Heat => weapon.supports_heat_mode(),
            Self::Hotload => weapon.supports_hotload(),
            Self::Ultra => weapon.is_ultra(),
            Self::Rapid => weapon.supports_rapid_fire(),
            Self::Gatling => weapon.supports_gatling(),
            Self::Rotary2 | Self::Rotary3 | Self::Rotary4 | Self::Rotary5 | Self::Rotary6 => {
                weapon.is_rotary()
            }
        }
    }
}

impl Weapon {
    /// Two-shell hit table; glancing shifts the cluster roll instead of halving shell damage.
    pub fn double_shot_damage_groups(self, roll: u8, glancing: bool) -> Result<Vec<u16>> {
        ensure!(
            self.is_ultra() || self.supports_rapid_fire(),
            "Weapon does not support two-round firing"
        );
        ensure!((2..=12).contains(&roll), "Invalid two-shell cluster roll");
        let hits = if roll.saturating_sub(if glancing { 4 } else { 0 }) >= 8 {
            2
        } else {
            1
        };
        Ok(vec![u16::from(self.profile().damage); hits])
    }
}

impl FireMode {
    /// Requested rounds before ammunition shortage can reset the firing mode.
    pub fn rounds_per_cycle(self) -> u16 {
        match self {
            Self::Ultra | Self::Rapid | Self::Rotary2 => 2,
            Self::Rotary3 => 3,
            Self::Rotary4 => 4,
            Self::Rotary5 => 5,
            Self::Rotary6 => 6,
            _ => 1,
        }
    }

    /// Rotary bursts jam at increasing thresholds and never destroy the loader directly: a
    /// to-hit roll at or below 2 jams two- and three-round bursts, 3 jams four and five, and 4
    /// jams six, as in MegaMek.
    pub fn rotary_jam_threshold(self) -> u8 {
        match self {
            Self::Rotary2 | Self::Rotary3 => 2,
            Self::Rotary4 | Self::Rotary5 => 3,
            Self::Rotary6 => 4,
            _ => 0,
        }
    }

    /// Feed failures retain the mount and ammunition, requiring an explicit clearing attempt.
    pub fn jams_on(self, roll: u8) -> bool {
        (self == Self::Hotload && roll <= 3)
            || (self == Self::Rapid && (3..=4).contains(&roll))
            || roll <= self.rotary_jam_threshold()
    }

    /// Ultra and conventional rapid fire have catastrophic loader failure on a two.
    pub fn is_double_shot(self) -> bool {
        matches!(self, Self::Ultra | Self::Rapid)
    }
}

#[cfg(test)]
mod tests {
    use crate::Weapon;

    #[test]
    fn two_shell_tables_and_glancing_preserve_individual_damage() {
        for weapon in [
            Weapon::UltraAc2,
            Weapon::UltraAc5,
            Weapon::UltraAc10,
            Weapon::UltraAc20,
            Weapon::Ac2,
            Weapon::Ac5,
            Weapon::Ac10,
            Weapon::Ac20,
            Weapon::LightAc2,
            Weapon::LightAc5,
        ] {
            for roll in 2..=12 {
                for glancing in [false, true] {
                    let groups = weapon.double_shot_damage_groups(roll, glancing).unwrap();
                    assert_eq!(
                        groups.len(),
                        if roll >= if glancing { 12 } else { 8 } {
                            2
                        } else {
                            1
                        }
                    );
                    assert!(
                        groups
                            .iter()
                            .all(|&damage| damage == u16::from(weapon.profile().damage))
                    );
                }
            }
            for roll in [0, 1, 13, 255] {
                assert!(weapon.double_shot_damage_groups(roll, false).is_err());
            }
        }
        assert!(
            Weapon::MediumLaser
                .double_shot_damage_groups(8, false)
                .is_err()
        );
    }
}
