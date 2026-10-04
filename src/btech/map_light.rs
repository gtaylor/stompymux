//! Battlefield light levels and their persisted integer encoding.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

/// Battlefield illumination levels used by sight range and darkness aim rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleLight {
    Night,
    Twilight,
    Day,
}

impl BattleLight {
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
}

impl std::str::FromStr for BattleLight {
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

impl super::StoredMap {
    /// Current battlefield light level, rejecting corrupt persisted values.
    pub fn light_level(&self) -> Result<BattleLight> {
        BattleLight::from_stored(self.light)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Stored and named forms round-trip for every level.
    #[test]
    fn light_levels_round_trip_through_storage_and_names() {
        for (light, name) in [
            (BattleLight::Night, "night"),
            (BattleLight::Twilight, "Twilight"),
            (BattleLight::Day, " DAY "),
        ] {
            assert_eq!(BattleLight::from_stored(light.stored()).unwrap(), light);
            assert_eq!(name.parse::<BattleLight>().unwrap(), light);
        }
        assert!(BattleLight::from_stored(3).is_err());
        assert!("dusk".parse::<BattleLight>().is_err());
    }
}
