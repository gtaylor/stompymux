//! One battlefield hex as independent layers: ground height and cover, woods, water, structure,
//! and a fire or smoke overlay.
//!
//! Each layer answers one question, so a hex can hold woods on a hill, ice over deep water or a
//! bridge deck above a river without one value standing in for another. Fire and smoke never
//! replace what they burn or cover: they are an overlay the map applies from its decorations.
//! Rules ask the layers. [`BattleHex::terrain`] names the one feature a map shows for a hex,
//! and [`BattleHex::new`] reads the compact symbol-and-digit notation used by `ADDHEX` and
//! [`BattleMapAsset::from_cells`](super::BattleMapAsset::from_cells).
use super::{BattleDecorationKind, Terrain};
use serde::{Deserialize, Serialize};

/// What the ground itself is made of, beneath any woods, water or structure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Ground {
    Clear,
    Road,
    Rough,
    Mountains,
    Snow,
    Sand,
}

/// Forest density covering the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Woods {
    Light,
    Heavy,
}

/// Standing water whose surface is at the hex's ground level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Water {
    /// Levels from the surface down to the bottom.
    pub depth: u8,
    /// Whether the surface is frozen over.
    pub frozen: bool,
}

/// A built feature standing on the ground or spanning it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Structure {
    /// A building `height` levels tall.
    Building { height: u8 },
    /// An impassable wall `height` levels tall.
    Wall { height: u8 },
    /// A bridge deck `deck` levels above the ground level.
    Bridge { deck: u8 },
}

/// The largest ground level, structure height or bridge deck a hex may have.
pub const MAX_HEIGHT: u8 = 35;

/// The deepest water a hex may hold.
pub const MAX_DEPTH: u8 = 9;

/// One-character height for maps and map files: `0`-`9`, then `a`-`z` for 10 through 35.
/// Anything higher shows as `?`.
pub fn height_glyph(height: u8) -> char {
    char::from_digit(u32::from(height), 36).unwrap_or('?')
}

/// A battlefield hex. Build one with [`BattleHex::new`] from its single-terrain description.
/// Saved and scripted as its layers; absent woods, water, structure and overlay are omitted.
/// A map's terrain grid never holds an overlay: [`StoredBattleMap::hex`](super::StoredBattleMap::hex)
/// adds it from the map's fire and smoke decorations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleHex {
    level: u8,
    ground: Ground,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    woods: Option<Woods>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    water: Option<Water>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    structure: Option<Structure>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    overlay: Option<BattleDecorationKind>,
}

impl BattleHex {
    /// Build the hex the compact notation describes: a terrain symbol and one digit. The digit
    /// is the ground height, except for water and ice (depth), bridges (deck height) and
    /// buildings and walls (their height). Bridges span water one level deep; fire and smoke
    /// overlay clear ground. Rules build and change hexes through their layers instead.
    pub const fn new(terrain: Terrain, elevation: u8) -> Self {
        let mut hex = Self {
            level: 0,
            ground: Ground::Clear,
            woods: None,
            water: None,
            structure: None,
            overlay: None,
        };
        match terrain {
            Terrain::Grassland => hex.level = elevation,
            Terrain::Road => (hex.level, hex.ground) = (elevation, Ground::Road),
            Terrain::Rough => (hex.level, hex.ground) = (elevation, Ground::Rough),
            Terrain::Mountains => (hex.level, hex.ground) = (elevation, Ground::Mountains),
            Terrain::Snow => (hex.level, hex.ground) = (elevation, Ground::Snow),
            Terrain::Sand => (hex.level, hex.ground) = (elevation, Ground::Sand),
            Terrain::Fire => {
                (hex.level, hex.overlay) = (elevation, Some(BattleDecorationKind::Fire))
            }
            Terrain::Smoke => {
                (hex.level, hex.overlay) = (elevation, Some(BattleDecorationKind::Smoke))
            }
            Terrain::LightForest => (hex.level, hex.woods) = (elevation, Some(Woods::Light)),
            Terrain::HeavyForest => (hex.level, hex.woods) = (elevation, Some(Woods::Heavy)),
            Terrain::Water | Terrain::Ice => {
                hex.water = Some(Water {
                    depth: elevation,
                    frozen: matches!(terrain, Terrain::Ice),
                });
            }
            Terrain::Bridge => {
                hex.water = Some(Water {
                    depth: 1,
                    frozen: false,
                });
                hex.structure = Some(Structure::Bridge { deck: elevation });
            }
            Terrain::Building => hex.structure = Some(Structure::Building { height: elevation }),
            Terrain::Wall => hex.structure = Some(Structure::Wall { height: elevation }),
        }
        hex
    }

    /// Build a hex directly from its layers, without an overlay.
    pub const fn from_layers(
        level: u8,
        ground: Ground,
        woods: Option<Woods>,
        water: Option<Water>,
        structure: Option<Structure>,
    ) -> Self {
        Self {
            level,
            ground,
            woods,
            water,
            structure,
            overlay: None,
        }
    }

    /// This hex with fire or smoke laid over it, or with its overlay removed.
    pub const fn with_overlay(self, overlay: Option<BattleDecorationKind>) -> Self {
        Self { overlay, ..self }
    }

    /// Clear ground at `level`, with nothing on it.
    pub const fn at_level(level: u8) -> Self {
        Self::from_layers(level, Ground::Clear, None, None, None)
    }

    /// This hex with its ground height changed.
    pub const fn with_level(self, level: u8) -> Self {
        Self { level, ..self }
    }

    /// This hex with its ground made of `ground`.
    pub const fn with_ground(self, ground: Ground) -> Self {
        Self { ground, ..self }
    }

    /// This hex with its woods replaced or cleared.
    pub const fn with_woods(self, woods: Option<Woods>) -> Self {
        Self { woods, ..self }
    }

    /// This hex with its water replaced or drained.
    pub const fn with_water(self, water: Option<Water>) -> Self {
        Self { water, ..self }
    }

    /// This hex with its structure replaced or removed.
    pub const fn with_structure(self, structure: Option<Structure>) -> Self {
        Self { structure, ..self }
    }

    /// Check every height and depth is within the battlefield's limits.
    pub fn validate(self) -> anyhow::Result<()> {
        let structure = match self.structure {
            Some(Structure::Building { height } | Structure::Wall { height }) => height,
            Some(Structure::Bridge { deck }) => deck,
            None => 0,
        };
        anyhow::ensure!(
            self.level <= MAX_HEIGHT && structure <= MAX_HEIGHT && self.water_depth() <= MAX_DEPTH,
            "Hex heights exceed map limits"
        );
        Ok(())
    }

    /// Ground height in levels; water surfaces sit at this height.
    pub const fn level(self) -> u8 {
        self.level
    }

    /// What the ground is made of.
    pub const fn ground(self) -> Ground {
        self.ground
    }

    /// Forest covering the ground, if any.
    pub const fn woods(self) -> Option<Woods> {
        self.woods
    }

    /// Standing water, if any.
    pub const fn water(self) -> Option<Water> {
        self.water
    }

    /// The built feature in this hex, if any.
    pub const fn structure(self) -> Option<Structure> {
        self.structure
    }

    /// Fire or smoke over this hex, if any.
    pub const fn overlay(self) -> Option<BattleDecorationKind> {
        self.overlay
    }

    /// The one feature a map shows for this hex: overlay, then structure, then water, then
    /// woods, then ground. Rules ask the layers instead.
    pub const fn terrain(self) -> Terrain {
        match self.overlay {
            Some(BattleDecorationKind::Fire) => return Terrain::Fire,
            Some(BattleDecorationKind::Smoke) => return Terrain::Smoke,
            None => {}
        }
        match (self.structure, self.water, self.woods) {
            (Some(Structure::Building { .. }), _, _) => Terrain::Building,
            (Some(Structure::Wall { .. }), _, _) => Terrain::Wall,
            (Some(Structure::Bridge { .. }), _, _) => Terrain::Bridge,
            (None, Some(Water { frozen: true, .. }), _) => Terrain::Ice,
            (None, Some(Water { frozen: false, .. }), _) => Terrain::Water,
            (None, None, Some(Woods::Light)) => Terrain::LightForest,
            (None, None, Some(Woods::Heavy)) => Terrain::HeavyForest,
            (None, None, None) => match self.ground {
                Ground::Clear => Terrain::Grassland,
                Ground::Road => Terrain::Road,
                Ground::Rough => Terrain::Rough,
                Ground::Mountains => Terrain::Mountains,
                Ground::Snow => Terrain::Snow,
                Ground::Sand => Terrain::Sand,
            },
        }
    }

    /// Depth of the standing water in this hex, or zero when there is none.
    pub const fn water_depth(self) -> u8 {
        match self.water {
            Some(Water { depth, .. }) => depth,
            None => 0,
        }
    }

    /// Whether this hex is open water: unfrozen, with no bridge or other structure over it.
    pub const fn is_open_water(self) -> bool {
        matches!(
            (self.water, self.structure),
            (Some(Water { frozen: false, .. }), None)
        )
    }

    /// Whether fire is burning over this hex.
    pub const fn is_burning(self) -> bool {
        matches!(self.overlay, Some(BattleDecorationKind::Fire))
    }

    /// Whether this hex is road with nothing else on it.
    pub const fn is_road(self) -> bool {
        matches!(self.ground, Ground::Road)
            && self.woods.is_none()
            && self.water.is_none()
            && self.structure.is_none()
    }

    /// Whether a bridge spans this hex.
    pub const fn has_bridge(self) -> bool {
        matches!(self.structure, Some(Structure::Bridge { .. }))
    }

    /// Whether this hex's surface is water or ice with nothing built over it.
    pub const fn is_water_surface(self) -> bool {
        self.water.is_some() && self.structure.is_none()
    }

    /// Whether this hex is frozen water with no structure over it.
    pub const fn is_ice(self) -> bool {
        matches!(
            (self.water, self.structure),
            (Some(Water { frozen: true, .. }), None)
        )
    }

    /// Height of the bridge deck in this hex, if it has one.
    pub fn deck_height(self) -> Option<i16> {
        match self.structure {
            Some(Structure::Bridge { deck }) => Some(i16::from(self.level) + i16::from(deck)),
            _ => None,
        }
    }

    /// Height of the bridge deck above the hex's ground level, if it has a bridge.
    pub const fn deck_clearance(self) -> Option<u8> {
        match self.structure {
            Some(Structure::Bridge { deck }) => Some(deck),
            _ => None,
        }
    }

    /// Height of the topmost surface: a structure's top or bridge deck, otherwise the ground
    /// or water surface.
    pub fn top_height(self) -> i16 {
        match self.structure {
            Some(_) => self.surface_height(),
            None => i16::from(self.level),
        }
    }

    /// Height of the bottom of the hex: the ground, or the bed beneath any water.
    pub fn bottom_height(self) -> i16 {
        i16::from(self.level) - i16::from(self.water_depth())
    }

    /// This hex with its water frozen over.
    pub const fn frozen(self) -> Self {
        match self.water {
            Some(water) => self.with_water(Some(Water {
                frozen: true,
                ..water
            })),
            None => self,
        }
    }

    /// This hex after its ice cracks or its bridge collapses: the same water, now open.
    pub const fn with_surface_broken(self) -> Self {
        let mut hex = self;
        if let Some(Structure::Bridge { .. }) = hex.structure {
            hex.structure = None;
        }
        if let Some(water) = hex.water {
            hex.water = Some(Water {
                frozen: false,
                ..water
            });
        }
        hex
    }

    /// Supported standing surface: ice holds units at the water surface, a bridge or other
    /// structure at its top, and anything else at its bottom.
    pub fn standing_height(self) -> i16 {
        if self.is_ice() {
            return i16::from(self.level);
        }
        self.surface_height()
    }

    /// Whether entering this hex at a rounded jump altitude hits an obstacle.
    /// Water entry uses immersion; bridge spans permit passage below their underside.
    /// This predicate does not authorize a route or move the unit.
    pub fn blocks_jump_entry(self, altitude: i32) -> bool {
        if self.is_open_water() {
            return false;
        }
        if let Some(deck) = self.deck_height() {
            return altitude < i32::from(self.level) || altitude == i32::from(deck) - 1;
        }
        altitude < i32::from(self.surface_height())
    }

    /// Bridge contact checked during vertical integration, before the hex transition.
    /// Unlike entry checks, this stage only collides above the water surface.
    pub fn strikes_bridge_during_jump(self, altitude: i32) -> bool {
        self.deck_height().is_some()
            && altitude > i32::from(self.level)
            && self.blocks_jump_entry(altitude)
    }

    /// Height ground movement treats as this hex's surface: a structure's top, a bridge deck,
    /// the bed under water, or the ground.
    pub fn surface_height(self) -> i16 {
        match self.structure {
            Some(Structure::Building { height } | Structure::Wall { height }) => {
                i16::from(self.level) + i16::from(height)
            }
            Some(Structure::Bridge { .. }) => self.deck_height().unwrap_or_default(),
            None => self.bottom_height(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The layer the notation's digit sets: water depth, structure top or bridge deck, or
    /// ground height.
    fn digit(hex: BattleHex) -> u8 {
        if hex.is_water_surface() {
            return hex.water_depth();
        }
        hex.top_height() as u8
    }

    /// Every terrain and digit of the compact notation survives the layers unchanged.
    #[test]
    fn notation_round_trips_through_layers() {
        for terrain in Terrain::ALL {
            for elevation in 0..=9 {
                let hex = BattleHex::new(terrain, elevation);
                assert_eq!((hex.terrain(), digit(hex)), (terrain, elevation));
                let saved = serde_json::to_value(hex).unwrap();
                assert_eq!(serde_json::from_value::<BattleHex>(saved).unwrap(), hex);
            }
        }
    }

    /// The layer-based height rules give exactly the single-symbol answers they replaced.
    #[test]
    fn heights_match_the_single_symbol_rules() {
        for terrain in Terrain::ALL {
            for elevation in 0..=9 {
                let hex = BattleHex::new(terrain, elevation);
                let digit = i16::from(elevation);
                let surface = if matches!(terrain, Terrain::Water | Terrain::Ice) {
                    -digit
                } else {
                    digit
                };
                assert_eq!(hex.surface_height(), surface, "{terrain:?} {elevation}");
                assert_eq!(
                    hex.standing_height(),
                    if terrain == Terrain::Ice { 0 } else { surface }
                );
                for altitude in -12..=12 {
                    let blocks = match terrain {
                        Terrain::Water => false,
                        Terrain::Bridge => altitude < 0 || altitude == i32::from(elevation) - 1,
                        _ => altitude < i32::from(surface),
                    };
                    assert_eq!(hex.blocks_jump_entry(altitude), blocks);
                    assert_eq!(
                        hex.strikes_bridge_during_jump(altitude),
                        terrain == Terrain::Bridge && altitude > 0 && blocks
                    );
                }
            }
        }
    }

    #[test]
    fn layers_separate_what_the_digit_used_to_mean() {
        let forest = BattleHex::new(Terrain::HeavyForest, 3);
        assert_eq!(
            (forest.level(), forest.woods(), forest.water()),
            (3, Some(Woods::Heavy), None)
        );
        let ice = BattleHex::new(Terrain::Ice, 4);
        assert_eq!(ice.level(), 0);
        assert_eq!(
            ice.water(),
            Some(Water {
                depth: 4,
                frozen: true
            })
        );
        let bridge = BattleHex::new(Terrain::Bridge, 2);
        assert_eq!(bridge.structure(), Some(Structure::Bridge { deck: 2 }));
        assert_eq!(bridge.water().map(|water| water.depth), Some(1));
        assert_eq!(
            BattleHex::new(Terrain::Ice, 4).with_surface_broken(),
            BattleHex::new(Terrain::Water, 4)
        );
        assert_eq!(
            BattleHex::new(Terrain::Bridge, 3).with_surface_broken(),
            BattleHex::new(Terrain::Water, 1)
        );
        let building = BattleHex::new(Terrain::Building, 5);
        assert_eq!(
            building.structure(),
            Some(Structure::Building { height: 5 })
        );
        assert_eq!(building.level(), 0);
    }

    /// A structure stands on its ground, so its top can rise past what one digit could say.
    #[test]
    fn structures_stand_on_raised_ground() {
        let tower = BattleHex::from_layers(
            8,
            Ground::Clear,
            None,
            None,
            Some(Structure::Building { height: 7 }),
        );
        assert_eq!(tower.surface_height(), 15);
        assert_eq!(tower.top_height(), 15);
        assert!(tower.blocks_jump_entry(14));
        assert!(!tower.blocks_jump_entry(15));
        tower.validate().unwrap();
        assert_eq!(height_glyph(15), 'f');
        assert_eq!(height_glyph(36), '?');
        let too_high = BattleHex::from_layers(36, Ground::Clear, None, None, None);
        assert!(too_high.validate().is_err());
        let too_deep = BattleHex::from_layers(
            0,
            Ground::Clear,
            None,
            Some(Water {
                depth: 10,
                frozen: false,
            }),
            None,
        );
        assert!(too_deep.validate().is_err());
    }

    /// Fire and smoke lie over a hex's layers without replacing them.
    #[test]
    fn overlays_keep_the_layers_they_cover() {
        let woods = BattleHex::new(Terrain::HeavyForest, 3);
        let burning = woods.with_overlay(Some(BattleDecorationKind::Fire));
        assert_eq!((burning.terrain(), burning.level()), (Terrain::Fire, 3));
        assert_eq!(burning.woods(), Some(Woods::Heavy));
        assert_eq!(burning.with_overlay(None), woods);
        let smoky =
            BattleHex::new(Terrain::Building, 4).with_overlay(Some(BattleDecorationKind::Smoke));
        assert_eq!(smoky.structure(), Some(Structure::Building { height: 4 }));
        assert_eq!(smoky.surface_height(), 4);
        assert_eq!(
            serde_json::to_value(BattleHex::new(Terrain::Fire, 1)).unwrap(),
            serde_json::json!({"level": 1, "ground": "clear", "overlay": "fire"})
        );
    }

    /// Hexes are saved as their layers, leaving out the ones that are absent.
    #[test]
    fn saved_shape_names_each_layer() {
        assert_eq!(
            serde_json::to_value(BattleHex::new(Terrain::Grassland, 2)).unwrap(),
            serde_json::json!({"level": 2, "ground": "clear"})
        );
        assert_eq!(
            serde_json::to_value(BattleHex::new(Terrain::HeavyForest, 1)).unwrap(),
            serde_json::json!({"level": 1, "ground": "clear", "woods": "heavy"})
        );
        assert_eq!(
            serde_json::to_value(BattleHex::new(Terrain::Bridge, 3)).unwrap(),
            serde_json::json!({
                "level": 0,
                "ground": "clear",
                "water": {"depth": 1, "frozen": false},
                "structure": {"kind": "bridge", "deck": 3}
            })
        );
        assert!(
            serde_json::from_value::<BattleHex>(
                serde_json::json!({"terrain": "road", "elevation": 1})
            )
            .is_err()
        );
    }
}
