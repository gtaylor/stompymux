//! One battlefield hex as independent layers: ground height and cover, woods, water, structure.
//!
//! Each layer answers one question, so a hex can hold woods on a hill, ice over deep water or a
//! bridge deck above a river without one value standing in for another. [`BattleHex::terrain`]
//! and [`BattleHex::elevation`] still give the single-symbol view used by map files, persistence
//! and most rules while those move onto the layers.
use super::Terrain;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

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
    /// Fire painted into the terrain grid rather than held as an overlay.
    Fire,
    /// Smoke painted into the terrain grid rather than held as an overlay.
    Smoke,
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
#[serde(rename_all = "snake_case")]
pub enum Structure {
    /// A building `height` levels tall.
    Building { height: u8 },
    /// An impassable wall `height` levels tall.
    Wall { height: u8 },
    /// A bridge deck `deck` levels above the ground level.
    Bridge { deck: u8 },
}

/// A battlefield hex. Build one with [`BattleHex::new`] from its single-terrain description.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleHex {
    level: u8,
    ground: Ground,
    woods: Option<Woods>,
    water: Option<Water>,
    structure: Option<Structure>,
}

impl BattleHex {
    /// Build the hex a terrain symbol and elevation digit describe. The digit is the ground
    /// height, except for water and ice (depth), bridges (deck height) and buildings and walls
    /// (their height). Bridges span water one level deep.
    pub const fn new(terrain: Terrain, elevation: u8) -> Self {
        let mut hex = Self {
            level: 0,
            ground: Ground::Clear,
            woods: None,
            water: None,
            structure: None,
        };
        match terrain {
            Terrain::Grassland => hex.level = elevation,
            Terrain::Road => (hex.level, hex.ground) = (elevation, Ground::Road),
            Terrain::Rough => (hex.level, hex.ground) = (elevation, Ground::Rough),
            Terrain::Mountains => (hex.level, hex.ground) = (elevation, Ground::Mountains),
            Terrain::Snow => (hex.level, hex.ground) = (elevation, Ground::Snow),
            Terrain::Sand => (hex.level, hex.ground) = (elevation, Ground::Sand),
            Terrain::Fire => (hex.level, hex.ground) = (elevation, Ground::Fire),
            Terrain::Smoke => (hex.level, hex.ground) = (elevation, Ground::Smoke),
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

    /// The single terrain symbol that best describes this hex: structure, then water, then
    /// woods, then ground.
    pub const fn terrain(self) -> Terrain {
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
                Ground::Fire => Terrain::Fire,
                Ground::Smoke => Terrain::Smoke,
            },
        }
    }

    /// The elevation digit paired with [`terrain`](Self::terrain): water depth, bridge deck,
    /// structure top, or ground height.
    pub const fn elevation(self) -> u8 {
        match (self.structure, self.water) {
            (Some(Structure::Building { height } | Structure::Wall { height }), _) => {
                self.level + height
            }
            (Some(Structure::Bridge { deck }), _) => self.level + deck,
            (None, Some(Water { depth, .. })) => depth,
            (None, None) => self.level,
        }
    }

    /// This hex with its elevation digit replaced, keeping its terrain.
    pub const fn with_elevation(self, elevation: u8) -> Self {
        Self::new(self.terrain(), elevation)
    }

    /// This hex with its terrain replaced, keeping its elevation digit.
    pub const fn with_terrain(self, terrain: Terrain) -> Self {
        Self::new(terrain, self.elevation())
    }

    /// Supported standing surface; intact ice is at water level while its depth stays in the asset.
    pub fn standing_height(self) -> i16 {
        if self.terrain() == Terrain::Ice {
            return 0;
        }
        self.surface_height()
    }

    /// Whether entering this hex at a rounded jump altitude hits an obstacle.
    /// Water entry uses immersion; bridge spans permit passage below their underside.
    /// This predicate does not authorize a route or move the unit.
    pub fn blocks_jump_entry(self, altitude: i32) -> bool {
        match self.terrain() {
            Terrain::Water => false,
            Terrain::Bridge => altitude < 0 || altitude == i32::from(self.elevation()) - 1,
            _ => altitude < i32::from(self.surface_height()),
        }
    }

    /// Bridge contact checked during vertical integration, before the hex transition.
    /// Unlike entry checks, this stage only collides at positive altitude.
    pub fn strikes_bridge_during_jump(self, altitude: i32) -> bool {
        self.terrain() == Terrain::Bridge && altitude > 0 && self.blocks_jump_entry(altitude)
    }

    /// Terrain-relative height used by ground movement.
    pub fn surface_height(self) -> i16 {
        let elevation = i16::from(self.elevation());
        if matches!(self.terrain(), Terrain::Water | Terrain::Ice) {
            return -elevation;
        }
        elevation
    }
}

impl PartialOrd for BattleHex {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for BattleHex {
    /// Order by the single-symbol view, which is how saved terrain dictionaries are keyed.
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        (self.terrain(), self.elevation()).cmp(&(other.terrain(), other.elevation()))
    }
}

/// The saved and scripted shape of a hex: its single-symbol view.
#[derive(Serialize, Deserialize)]
struct HexRecord {
    terrain: Terrain,
    elevation: u8,
}

impl Serialize for BattleHex {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        HexRecord {
            terrain: self.terrain(),
            elevation: self.elevation(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for BattleHex {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let record = HexRecord::deserialize(deserializer)?;
        Ok(Self::new(record.terrain, record.elevation))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every terrain and elevation digit survives the layered representation unchanged.
    #[test]
    fn single_symbol_view_round_trips_through_layers() {
        for terrain in Terrain::ALL {
            for elevation in 0..=9 {
                let hex = BattleHex::new(terrain, elevation);
                assert_eq!((hex.terrain(), hex.elevation()), (terrain, elevation));
                let saved = serde_json::to_value(hex).unwrap();
                assert_eq!(
                    saved,
                    serde_json::json!({"terrain": terrain, "elevation": elevation})
                );
                assert_eq!(serde_json::from_value::<BattleHex>(saved).unwrap(), hex);
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
        let building = BattleHex::new(Terrain::Building, 5);
        assert_eq!(
            building.structure(),
            Some(Structure::Building { height: 5 })
        );
        assert_eq!(building.level(), 0);
    }

    #[test]
    fn ordering_follows_the_single_symbol_view() {
        let mut hexes = [
            BattleHex::new(Terrain::Water, 2),
            BattleHex::new(Terrain::Grassland, 9),
            BattleHex::new(Terrain::Road, 0),
            BattleHex::new(Terrain::Grassland, 1),
        ];
        hexes.sort();
        assert_eq!(
            hexes.map(|hex| (hex.terrain(), hex.elevation())),
            [
                (Terrain::Grassland, 1),
                (Terrain::Grassland, 9),
                (Terrain::Road, 0),
                (Terrain::Water, 2),
            ]
        );
    }
}
