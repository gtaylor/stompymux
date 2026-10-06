//! Battlefield-wide conditions a map can set: light and wind. Gravity, temperature and the
//! rule flags live on [`MapAsset`](crate::MapAsset) directly.
use anyhow::{Result, bail, ensure};
use serde::{Deserialize, Serialize};

/// Battlefield illumination levels used by sight range and darkness aim rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Light {
    Night,
    Twilight,
    Day,
}

impl Light {
    /// Every light level, darkest first.
    pub const ALL: [Self; 3] = [Self::Night, Self::Twilight, Self::Day];

    /// Decode the persisted map column, where zero is night and two is full daylight.
    pub fn from_stored(value: i64) -> Result<Self> {
        Ok(match value {
            0 => Self::Night,
            1 => Self::Twilight,
            2 => Self::Day,
            _ => bail!("Map light must be 0, 1 or 2"),
        })
    }

    /// Encode this level for the persisted map column.
    pub fn stored(self) -> i64 {
        match self {
            Self::Night => 0,
            Self::Twilight => 1,
            Self::Day => 2,
        }
    }

    /// Lowercase name used by map files, operator commands and Lua.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Night => "night",
            Self::Twilight => "twilight",
            Self::Day => "day",
        }
    }
}

impl std::str::FromStr for Light {
    type Err = anyhow::Error;

    /// Parse named light levels used by operator commands and Lua.
    fn from_str(value: &str) -> Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "night" => Ok(Self::Night),
            "twilight" => Ok(Self::Twilight),
            "day" => Ok(Self::Day),
            _ => bail!("Expected night, twilight or day"),
        }
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
        for (light, name) in [
            (Light::Night, "night"),
            (Light::Twilight, "Twilight"),
            (Light::Day, " DAY "),
        ] {
            assert_eq!(Light::from_stored(light.stored()).unwrap(), light);
            assert_eq!(name.parse::<Light>().unwrap(), light);
            assert_eq!(light.name().parse::<Light>().unwrap(), light);
        }
        assert!(Light::from_stored(3).is_err());
        assert!("dusk".parse::<Light>().is_err());
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
