//! Named battlefield rule switches stored in a map's `flags` bitmask.
//!
//! Callers test and change map rules through [`BattleMapFlag`] instead of bare bit literals.
//! Bit positions are the persisted encoding shared by map files and saved state.
use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

/// One battlefield rule switch an operator can enable on a map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleMapFlag {
    /// Environmental rules (gravity, temperature, vacuum) apply to units on the map.
    SpecialRules,
    /// The map has no atmosphere.
    Vacuum,
    /// The map has a ceiling: no jumping, flight or indirect fire without an observer.
    Underground,
    /// Units only see terrain they have line of sight to.
    Dark,
    /// Weapon fire cannot break bridges.
    IndestructibleBridges,
    /// Teammates cannot damage each other with non-coolant weapons.
    NoFriendlyFire,
    /// Physical attacks are not allowed.
    NoPhysicalAttacks,
}

impl BattleMapFlag {
    /// Every flag, in bit order.
    pub const ALL: [Self; 7] = [
        Self::SpecialRules,
        Self::Vacuum,
        Self::Underground,
        Self::Dark,
        Self::IndestructibleBridges,
        Self::NoFriendlyFire,
        Self::NoPhysicalAttacks,
    ];

    /// Persisted bit for this flag.
    pub const fn bit(self) -> i64 {
        match self {
            Self::SpecialRules => 2,
            Self::Vacuum => 4,
            Self::Underground => 16,
            Self::Dark => 32,
            Self::IndestructibleBridges => 64,
            Self::NoFriendlyFire => 256,
            Self::NoPhysicalAttacks => 512,
        }
    }

    /// Operator-facing spelling used by `@SETMAP flags` and `@VIEWMAP`.
    pub const fn name(self) -> &'static str {
        match self {
            Self::SpecialRules => "special_rules",
            Self::Vacuum => "vacuum",
            Self::Underground => "underground",
            Self::Dark => "dark",
            Self::IndestructibleBridges => "indestructible_bridges",
            Self::NoFriendlyFire => "no_friendly_fire",
            Self::NoPhysicalAttacks => "no_physical_attacks",
        }
    }

    /// Decode an operator-facing flag name, ignoring ASCII case.
    pub fn parse(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|flag| flag.name().eq_ignore_ascii_case(name))
            .map_or_else(|| bail!("Unknown map flag {name:?}"), Ok)
    }

    /// Whether `flags` has this flag set.
    pub const fn is_set(self, flags: i64) -> bool {
        flags & self.bit() != 0
    }

    /// `flags` with this flag switched on or off.
    pub const fn apply(self, flags: i64, enabled: bool) -> i64 {
        if enabled {
            flags | self.bit()
        } else {
            flags & !self.bit()
        }
    }

    /// Every bit any named flag uses.
    pub fn mask() -> i64 {
        Self::ALL
            .into_iter()
            .fold(0, |mask, flag| mask | flag.bit())
    }
}

impl super::StoredBattleMap {
    /// Whether this map has `flag` switched on.
    pub fn has_flag(&self, flag: BattleMapFlag) -> bool {
        flag.is_set(self.flags)
    }

    /// Switch `flag` on or off without touching other flags.
    pub fn set_flag(&mut self, flag: BattleMapFlag, enabled: bool) {
        self.flags = flag.apply(self.flags, enabled);
    }
}

impl super::BattleMapAsset {
    /// Whether this map asset has `flag` switched on.
    pub fn has_flag(&self, flag: BattleMapFlag) -> bool {
        flag.is_set(i64::from(self.flags))
    }
}

/// Parse a whitespace- or comma-separated list of flag names into a bitmask; `-` means none.
pub fn parse_map_flags(value: &str) -> Result<i64> {
    let value = value.trim();
    if value == "-" {
        return Ok(0);
    }
    value
        .split([' ', '\t', ','])
        .filter(|name| !name.is_empty())
        .try_fold(0, |flags, name| {
            Ok(flags | BattleMapFlag::parse(name)?.bit())
        })
}

/// Display the named flags in `flags` in bit order; `-` when none are set.
pub fn format_map_flags(flags: i64) -> String {
    let names: Vec<_> = BattleMapFlag::ALL
        .into_iter()
        .filter(|flag| flag.is_set(flags))
        .map(BattleMapFlag::name)
        .collect();
    if names.is_empty() {
        return "-".into();
    }
    names.join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bits_are_distinct_and_names_round_trip() {
        let mut seen = 0;
        for flag in BattleMapFlag::ALL {
            assert_eq!(seen & flag.bit(), 0, "{flag:?}");
            seen |= flag.bit();
            assert_eq!(BattleMapFlag::parse(flag.name()).unwrap(), flag);
            assert_eq!(
                BattleMapFlag::parse(&flag.name().to_ascii_uppercase()).unwrap(),
                flag
            );
            assert!(flag.is_set(flag.apply(0, true)));
            assert!(!flag.is_set(flag.apply(-1, false)));
        }
        assert_eq!(seen, BattleMapFlag::mask());
        assert_eq!(seen & 1, 0);
    }

    #[test]
    fn flag_lists_parse_and_display() {
        assert_eq!(parse_map_flags("-").unwrap(), 0);
        assert_eq!(parse_map_flags("").unwrap(), 0);
        let flags = parse_map_flags("dark, special_rules  NO_FRIENDLY_FIRE").unwrap();
        assert_eq!(flags, 2 | 32 | 256);
        assert_eq!(
            format_map_flags(flags),
            "special_rules dark no_friendly_fire"
        );
        assert_eq!(parse_map_flags(&format_map_flags(flags)).unwrap(), flags);
        assert_eq!(format_map_flags(0), "-");
        assert_eq!(format_map_flags(1), "-");
        assert!(parse_map_flags("dark bogus").is_err());
        assert!(parse_map_flags("ab").is_err());
    }
}
