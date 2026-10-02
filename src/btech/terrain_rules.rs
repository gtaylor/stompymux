//! Shared terrain rule lookups: terrain groupings, woods density and ground speed divisors.
//!
//! Combat, movement and sensor code ask these questions of a [`Terrain`] instead of
//! matching on variants locally, so a rule changes in exactly one place.
use super::{BattleHex, Ground, Terrain, Woods};

impl Terrain {
    /// Light or heavy forest.
    pub fn is_woods(self) -> bool {
        matches!(self, Self::LightForest | Self::HeavyForest)
    }

    /// Woods density used by to-hit modifiers and line of sight: light 1, heavy 2, otherwise 0.
    pub fn woods_density(self) -> u8 {
        match self {
            Self::LightForest => 1,
            Self::HeavyForest => 2,
            _ => 0,
        }
    }

    /// Terrain whose column holds water a unit below the surface datum is immersed in.
    /// Bridges span water, so a unit beneath a deck is in the water under it.
    pub fn holds_water(self) -> bool {
        matches!(self, Self::Water | Self::Ice | Self::Bridge)
    }

    /// Divisor terrain applies to a ground unit's desired speed.
    /// Loose sand bogs down wheels; legs, tracks and hover skirts cross it like clear ground.
    pub fn ground_speed_divisor(self, wheeled: bool) -> f64 {
        match self {
            Self::Rough | Self::Snow | Self::LightForest => 2.0,
            Self::Mountains | Self::HeavyForest => 3.0,
            Self::Sand if wheeled => 2.0,
            _ => 1.0,
        }
    }
}

impl BattleHex {
    /// Whether woods cover this hex.
    pub fn is_woods(self) -> bool {
        self.woods().is_some()
    }

    /// Woods density used by to-hit modifiers and line of sight: light 1, heavy 2, otherwise 0.
    pub fn woods_density(self) -> u8 {
        match self.woods() {
            Some(Woods::Light) => 1,
            Some(Woods::Heavy) => 2,
            None => 0,
        }
    }

    /// Whether this hex holds water, including under ice or a bridge.
    pub fn holds_water(self) -> bool {
        self.water().is_some()
    }

    /// Divisor this hex applies to a ground unit's desired speed: the slower of its woods and
    /// its ground. Water and structures are governed by their own movement rules.
    pub fn ground_speed_divisor(self, wheeled: bool) -> f64 {
        if self.water().is_some() || self.structure().is_some() {
            return 1.0;
        }
        let woods = match self.woods() {
            Some(Woods::Light) => 2.0,
            Some(Woods::Heavy) => 3.0,
            None => 1.0,
        };
        let ground = match self.ground() {
            Ground::Rough | Ground::Snow => 2.0,
            Ground::Mountains => 3.0,
            Ground::Sand if wheeled => 2.0,
            _ => 1.0,
        };
        f64::max(woods, ground)
    }

    /// Whether a unit standing at `level` relative to this hex is in water: below the
    /// surface datum of a water column.
    pub fn immerses(self, level: i32) -> bool {
        self.holds_water() && level < i32::from(self.level())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn woods_density_matches_woods_grouping() {
        for terrain in [
            Terrain::Grassland,
            Terrain::LightForest,
            Terrain::HeavyForest,
            Terrain::Smoke,
            Terrain::Rough,
        ] {
            assert_eq!(terrain.is_woods(), terrain.woods_density() > 0);
        }
        assert_eq!(Terrain::LightForest.woods_density(), 1);
        assert_eq!(Terrain::HeavyForest.woods_density(), 2);
    }

    #[test]
    fn sand_slows_only_wheeled_units() {
        assert_eq!(Terrain::Sand.ground_speed_divisor(true), 2.0);
        assert_eq!(Terrain::Sand.ground_speed_divisor(false), 1.0);
        for wheeled in [false, true] {
            assert_eq!(Terrain::Rough.ground_speed_divisor(wheeled), 2.0);
            assert_eq!(Terrain::HeavyForest.ground_speed_divisor(wheeled), 3.0);
            assert_eq!(Terrain::Road.ground_speed_divisor(wheeled), 1.0);
        }
    }

    /// Hex answers match the terrain answers for every hex the compact notation describes.
    #[test]
    fn hex_rules_match_terrain_rules() {
        for terrain in Terrain::ALL {
            let hex = BattleHex::new(terrain, 1);
            assert_eq!(hex.is_woods(), terrain.is_woods(), "{terrain:?}");
            assert_eq!(hex.woods_density(), terrain.woods_density());
            assert_eq!(hex.holds_water(), terrain.holds_water());
            for wheeled in [false, true] {
                assert_eq!(
                    hex.ground_speed_divisor(wheeled),
                    terrain.ground_speed_divisor(wheeled)
                );
            }
        }
    }

    #[test]
    fn water_columns_include_ice_and_bridges() {
        assert!(Terrain::Water.holds_water());
        assert!(Terrain::Ice.holds_water());
        assert!(Terrain::Bridge.holds_water());
        assert!(!Terrain::Grassland.holds_water());
    }

    #[test]
    fn immersion_depends_on_level() {
        let hex = |terrain| BattleHex::new(terrain, 2);
        assert!(!hex(Terrain::Water).immerses(3));
        assert!(hex(Terrain::Water).immerses(-1));
        assert!(!hex(Terrain::Water).immerses(0));
        assert!(hex(Terrain::Bridge).immerses(-2));
        assert!(!hex(Terrain::Grassland).immerses(-1));
    }
}
