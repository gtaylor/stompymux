//! Shared terrain rule lookups on a hex's layers: open ground, woods density, water and
//! ground speed divisors.
//!
//! Combat, movement and sensor code ask these questions of a [`BattleHex`] instead of
//! matching on layers locally, so a rule changes in exactly one place.
use super::{BattleHex, Ground, Woods};

impl BattleHex {
    /// Whether nothing stands on or covers the ground: no woods, water or structure.
    pub fn is_bare(self) -> bool {
        self.woods().is_none() && self.water().is_none() && self.structure().is_none()
    }

    /// Bare clear ground, road or sand with no fire or smoke over it: firm, open ground a
    /// unit can set down on.
    pub fn is_open_ground(self) -> bool {
        self.is_bare()
            && self.overlay().is_none()
            && matches!(self.ground(), Ground::Clear | Ground::Road | Ground::Sand)
    }

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
    use crate::btech::Terrain;

    #[test]
    fn woods_density_matches_woods_layer() {
        let density = |terrain| BattleHex::new(terrain, 0).woods_density();
        for terrain in [Terrain::Grassland, Terrain::Smoke, Terrain::Rough] {
            assert_eq!(density(terrain), 0);
            assert!(!BattleHex::new(terrain, 0).is_woods());
        }
        assert_eq!(density(Terrain::LightForest), 1);
        assert_eq!(density(Terrain::HeavyForest), 2);
        let burning = BattleHex::new(Terrain::HeavyForest, 0)
            .with_overlay(Some(crate::btech::BattleDecorationKind::Fire));
        assert_eq!(burning.woods_density(), 2);
    }

    #[test]
    fn sand_slows_only_wheeled_units() {
        let divisor = |terrain, wheeled| BattleHex::new(terrain, 0).ground_speed_divisor(wheeled);
        assert_eq!(divisor(Terrain::Sand, true), 2.0);
        assert_eq!(divisor(Terrain::Sand, false), 1.0);
        for wheeled in [false, true] {
            assert_eq!(divisor(Terrain::Rough, wheeled), 2.0);
            assert_eq!(divisor(Terrain::HeavyForest, wheeled), 3.0);
            assert_eq!(divisor(Terrain::Mountains, wheeled), 3.0);
            assert_eq!(divisor(Terrain::Road, wheeled), 1.0);
            assert_eq!(divisor(Terrain::Bridge, wheeled), 1.0);
        }
    }

    #[test]
    fn open_ground_is_bare_clear_road_or_sand_without_fire_or_smoke() {
        for terrain in Terrain::ALL {
            assert_eq!(
                BattleHex::new(terrain, 0).is_open_ground(),
                matches!(terrain, Terrain::Grassland | Terrain::Road | Terrain::Sand),
                "{terrain:?}"
            );
        }
    }

    #[test]
    fn water_columns_include_ice_and_bridges() {
        let holds = |terrain| BattleHex::new(terrain, 1).holds_water();
        assert!(holds(Terrain::Water));
        assert!(holds(Terrain::Ice));
        assert!(holds(Terrain::Bridge));
        assert!(!holds(Terrain::Grassland));
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
