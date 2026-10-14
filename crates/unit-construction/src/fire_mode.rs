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

    /// Rapid-fire jam thresholds (Total Warfare p. 114): a to-hit roll of 2 jams a two- or
    /// three-round burst, 3 or less jams four or five rounds and 4 or less jams six.
    pub fn rotary_jam_threshold(self) -> u8 {
        match self {
            Self::Rotary2 | Self::Rotary3 => 2,
            Self::Rotary4 | Self::Rotary5 => 3,
            Self::Rotary6 => 4,
            _ => 0,
        }
    }

    /// Whether this attack roll jams the feed so that the crew must clear it afterwards. Rotary
    /// bursts jam at their thresholds; TacOps rapid-fire mode jams on a 3 or 4 (p. 98); the
    /// burst that jams is still fired. Hot-loaded launchers never jam (Tactical Operations
    /// p. 101) and an Ultra autocannon's roll of 2 breaks the feed instead.
    pub fn jams_on(self, roll: u8) -> bool {
        (self == Self::Rapid && (3..=4).contains(&roll)) || roll <= self.rotary_jam_threshold()
    }

    /// Whether this attack roll leaves the weapon useless for the rest of the game (Total
    /// Warfare p. 114): an Ultra autocannon firing two shots on a roll of 2. The shots are
    /// still fired; the mount stays intact but its feed cannot be cleared in the field.
    pub fn breaks_feed_on(self, roll: u8) -> bool {
        self == Self::Ultra && roll == 2
    }

    /// Whether this attack roll makes the rounds explode in the barrel (Tactical Operations
    /// p. 98): a standard autocannon in rapid-fire mode on a roll of 2. The burst is still
    /// fired, then the weapon's slots are destroyed and its location takes one shot's damage.
    pub fn explodes_on(self, roll: u8) -> bool {
        self == Self::Rapid && roll == 2
    }

    /// Ultra and conventional rapid fire resolve two shells on the two-shot hit table.
    pub fn is_double_shot(self) -> bool {
        matches!(self, Self::Ultra | Self::Rapid)
    }
}

#[cfg(test)]
mod tests {
    use super::FireMode;
    use crate::Weapon;

    /// Jams follow the rapid-fire thresholds; hot-loading never jams; Ultra and rapid-fire mode
    /// have their own roll-of-2 outcomes.
    #[test]
    fn jam_break_and_explosion_thresholds() {
        for roll in 2..=12 {
            assert!(!FireMode::Hotload.jams_on(roll));
            assert!(!FireMode::Normal.jams_on(roll));
            assert!(!FireMode::Ultra.jams_on(roll));
            assert_eq!(FireMode::Rapid.jams_on(roll), (3..=4).contains(&roll));
            assert_eq!(FireMode::Rotary2.jams_on(roll), roll <= 2);
            assert_eq!(FireMode::Rotary3.jams_on(roll), roll <= 2);
            assert_eq!(FireMode::Rotary4.jams_on(roll), roll <= 3);
            assert_eq!(FireMode::Rotary5.jams_on(roll), roll <= 3);
            assert_eq!(FireMode::Rotary6.jams_on(roll), roll <= 4);
            assert_eq!(FireMode::Ultra.breaks_feed_on(roll), roll == 2);
            assert_eq!(FireMode::Rapid.explodes_on(roll), roll == 2);
            assert!(!FireMode::Rapid.breaks_feed_on(roll));
            assert!(!FireMode::Ultra.explodes_on(roll));
            assert!(!FireMode::Rotary6.breaks_feed_on(roll));
            assert!(!FireMode::Rotary6.explodes_on(roll));
        }
    }

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
