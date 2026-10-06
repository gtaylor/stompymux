//! Battlefield-wide conditions a map can set: light and wind. Gravity, temperature and the
//! rule flags live on [`MapAsset`](crate::MapAsset) directly.
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

/// Battlefield light, after the light conditions of Tactical Operations (p. 56). Darkness
/// makes weapon attacks harder, and at night physical attacks too; searchlights and hot
/// targets offset it. Glare plays as a full moon night and a solar flare as a moonless one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Light {
    Day,
    Dawn,
    Dusk,
    FullMoonNight,
    MoonlessNight,
    PitchBlack,
}

impl Light {
    /// Every light level, brightest first.
    pub const ALL: [Self; 6] = [
        Self::Day,
        Self::Dawn,
        Self::Dusk,
        Self::FullMoonNight,
        Self::MoonlessNight,
        Self::PitchBlack,
    ];

    /// Decode the persisted map column: zero is daylight and five is pitch black, in the order
    /// of [`Light::ALL`].
    pub fn from_stored(value: i64) -> Result<Self> {
        usize::try_from(value)
            .ok()
            .and_then(|index| Self::ALL.get(index).copied())
            .map_or_else(|| bail!("Map light must be 0 to 5"), Ok)
    }

    /// Encode this level for the persisted map column.
    pub fn stored(self) -> i64 {
        Self::ALL
            .iter()
            .position(|light| *light == self)
            .unwrap_or_default() as i64
    }

    /// Lowercase name used by map files, operator commands and Lua.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Dawn => "dawn",
            Self::Dusk => "dusk",
            Self::FullMoonNight => "full_moon_night",
            Self::MoonlessNight => "moonless_night",
            Self::PitchBlack => "pitch_black",
        }
    }

    /// Display name for players and editors.
    pub const fn label(self) -> &'static str {
        match self {
            Self::Day => "Day",
            Self::Dawn => "Dawn",
            Self::Dusk => "Dusk",
            Self::FullMoonNight => "Full moon night",
            Self::MoonlessNight => "Moonless night",
            Self::PitchBlack => "Pitch black",
        }
    }

    /// Whether this is one of the night levels, where sight shrinks to what is lit and
    /// searchlights offset the darkness.
    pub const fn is_night(self) -> bool {
        matches!(
            self,
            Self::FullMoonNight | Self::MoonlessNight | Self::PitchBlack
        )
    }

    /// To-hit modifier for a weapon attack against a unit, or for a physical attack with
    /// `physical`. `lit` says the target is illuminated or has its own searchlight on, and
    /// `target_heat` is the heat of a target that tracks heat; every full step of heat for
    /// this level takes one off a weapon attack's modifier.
    pub fn aim_modifier(self, physical: bool, lit: bool, target_heat: Option<u16>) -> i16 {
        let (weapon, unarmed, lit_weapon, heat_step) = match self {
            Self::Day => return 0,
            Self::Dawn | Self::Dusk => (1, 0, 1, 25),
            Self::FullMoonNight => (2, 0, 0, 20),
            Self::MoonlessNight => (3, 1, 0, 15),
            Self::PitchBlack => (4, 2, 1, 10),
        };
        let searchlight = lit && self.is_night();
        if physical {
            return if searchlight { 0 } else { unarmed };
        }
        let darkness = if searchlight { lit_weapon } else { weapon };
        darkness - target_heat.map_or(0, |heat| (heat / heat_step) as i16)
    }
}

impl std::str::FromStr for Light {
    type Err = anyhow::Error;

    /// Parse named light levels used by operator commands and Lua; spaces and hyphens may
    /// stand in for underscores.
    fn from_str(value: &str) -> Result<Self> {
        let name = value.trim().to_ascii_lowercase().replace([' ', '-'], "_");
        Self::ALL
            .into_iter()
            .find(|light| light.name() == name)
            .map_or_else(
                || {
                    bail!(
                        "Expected day, dawn, dusk, full_moon_night, moonless_night or pitch_black"
                    )
                },
                Ok,
            )
    }
}

/// The farthest weather visibility a map can set, in hexes.
pub const MAX_VISIBILITY: u8 = 60;

/// Prevailing wind, which carries smoke and spreads fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Wind {
    /// Bearing the wind blows from, in degrees, 0 to 359.
    pub direction: u16,
    /// Wind strength; zero is calm.
    pub speed: u16,
}

impl Wind {
    /// Require a bearing below 360 and a speed the map record can hold.
    pub fn validate(self) -> Result<()> {
        ensure!(self.direction < 360, "Wind direction must be 0 to 359");
        ensure!(
            i16::try_from(self.speed).is_ok(),
            "Wind speed must be at most {}",
            i16::MAX
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stored and named forms round-trip for every level.
    #[test]
    fn light_levels_round_trip_through_storage_and_names() {
        for light in Light::ALL {
            assert_eq!(Light::from_stored(light.stored()).unwrap(), light);
            assert_eq!(light.name().parse::<Light>().unwrap(), light);
        }
        assert_eq!(Light::Day.stored(), 0);
        assert_eq!(Light::PitchBlack.stored(), 5);
        assert_eq!(
            " Full Moon Night ".parse::<Light>().unwrap(),
            Light::FullMoonNight
        );
        assert_eq!(
            "moonless-night".parse::<Light>().unwrap(),
            Light::MoonlessNight
        );
        assert!(Light::from_stored(6).is_err());
        assert!(Light::from_stored(-1).is_err());
        assert!("twilight".parse::<Light>().is_err());
    }

    /// The Tactical Operations light table: weapon and physical penalties, what searchlights
    /// offset, and the heat steps that make hot targets easier to hit.
    #[test]
    fn light_aim_follows_tactical_operations() {
        use Light::*;
        let rows = [
            (Day, [0, 0, 0, 0]),
            (Dawn, [1, 0, 1, 0]),
            (Dusk, [1, 0, 1, 0]),
            (FullMoonNight, [2, 0, 0, 0]),
            (MoonlessNight, [3, 1, 0, 0]),
            (PitchBlack, [4, 2, 1, 0]),
        ];
        for (light, [weapon, physical, lit_weapon, lit_physical]) in rows {
            assert_eq!(light.aim_modifier(false, false, None), weapon, "{light:?}");
            assert_eq!(light.aim_modifier(true, false, None), physical, "{light:?}");
            assert_eq!(
                light.aim_modifier(false, true, None),
                lit_weapon,
                "{light:?}"
            );
            assert_eq!(
                light.aim_modifier(true, true, None),
                lit_physical,
                "{light:?}"
            );
        }
        assert_eq!(Dusk.aim_modifier(false, false, Some(24)), 1);
        assert_eq!(Dusk.aim_modifier(false, false, Some(25)), 0);
        assert_eq!(FullMoonNight.aim_modifier(false, false, Some(40)), 0);
        assert_eq!(MoonlessNight.aim_modifier(false, false, Some(30)), 1);
        assert_eq!(PitchBlack.aim_modifier(false, false, Some(39)), 1);
        assert_eq!(PitchBlack.aim_modifier(true, false, Some(50)), 2);
        assert_eq!(Day.aim_modifier(false, false, Some(50)), 0);
        assert!(!Dusk.is_night() && FullMoonNight.is_night() && PitchBlack.is_night());
    }

    #[test]
    fn wind_bearings_stay_below_a_full_turn() {
        Wind {
            direction: 359,
            speed: 40,
        }
        .validate()
        .unwrap();
        assert!(
            Wind {
                direction: 360,
                speed: 0
            }
            .validate()
            .is_err()
        );
        assert!(
            Wind {
                direction: 0,
                speed: u16::MAX
            }
            .validate()
            .is_err()
        );
    }
}
