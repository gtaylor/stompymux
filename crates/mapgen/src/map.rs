//! The generated map: one [`Hex`] per cell in a column-staggered hex grid, and its conversion
//! to the [`BattleMapAsset`] that `stompymux-map` writes as a map file.
//!
//! Columns are staggered like the game's: even columns sit half a hex south of odd ones.
//! [`HexMap::neighbors`] and [`HexMap::distance`] follow that layout, so roads and rivers built
//! from them connect in game.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use stompymux_map::{
    BattleDecorationKind, BattleHex, BattleHexCoordinate, BattleMapAsset, BattleMapFlag, Ground,
    MAX_DEPTH, MAX_HEIGHT, Structure, Water, Woods,
};

/// What fills a hex. Each variant is one map-file terrain symbol.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum Terrain {
    /// Open ground (`.`).
    Clear,
    /// Paved road (`#`).
    Road,
    /// Rough ground (`%`).
    Rough,
    /// Mountains (`^`).
    Mountains,
    /// Snow (`+`).
    Snow,
    /// Sand (`}`).
    Sand,
    /// Light woods (`` ` ``).
    LightWoods,
    /// Heavy woods (`"`).
    HeavyWoods,
    /// Water `depth` levels deep (`~`).
    Water { depth: u8 },
    /// Ice over water `depth` levels deep (`-`).
    Ice { depth: u8 },
    /// A building `height` levels tall (`@`).
    Building { height: u8 },
    /// An impassable wall `height` levels tall (`=`).
    Wall { height: u8 },
}

impl Terrain {
    /// The map-file terrain symbol.
    pub const fn symbol(self) -> char {
        match self {
            Self::Clear => '.',
            Self::Road => '#',
            Self::Rough => '%',
            Self::Mountains => '^',
            Self::Snow => '+',
            Self::Sand => '}',
            Self::LightWoods => '`',
            Self::HeavyWoods => '"',
            Self::Water { .. } => '~',
            Self::Ice { .. } => '-',
            Self::Building { .. } => '@',
            Self::Wall { .. } => '=',
        }
    }

    /// Water or ice depth, if this is either.
    pub const fn depth(self) -> Option<u8> {
        match self {
            Self::Water { depth } | Self::Ice { depth } => Some(depth),
            _ => None,
        }
    }

    /// Building or wall height, if this is either.
    pub const fn structure_height(self) -> Option<u8> {
        match self {
            Self::Building { height } | Self::Wall { height } => Some(height),
            _ => None,
        }
    }

    /// Whether this is water or ice.
    pub const fn is_water(self) -> bool {
        self.depth().is_some()
    }

    /// Whether this is a building or wall.
    pub const fn is_structure(self) -> bool {
        self.structure_height().is_some()
    }

    /// Whether this is light or heavy woods.
    pub const fn is_woods(self) -> bool {
        matches!(self, Self::LightWoods | Self::HeavyWoods)
    }
}

/// One hex of a generated map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hex {
    /// Ground level; water and ice surfaces sit at this level.
    pub level: u8,
    /// What fills the hex.
    pub terrain: Terrain,
    /// Bridge deck height above the water, for a bridge over water or ice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bridge: Option<u8>,
    /// Permanent fire or smoke.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overlay: Option<BattleDecorationKind>,
}

impl Hex {
    /// Clear ground at `level`.
    pub const fn clear(level: u8) -> Self {
        Self {
            level,
            terrain: Terrain::Clear,
            bridge: None,
            overlay: None,
        }
    }

    /// This hex as the layered [`BattleHex`] map files and the server use. A bridge deck spans
    /// the hex's water or ice.
    pub fn to_battle_hex(self) -> BattleHex {
        let (mut ground, mut woods, mut water, mut structure) = (Ground::Clear, None, None, None);
        match self.terrain {
            Terrain::Clear => {}
            Terrain::Road => ground = Ground::Road,
            Terrain::Rough => ground = Ground::Rough,
            Terrain::Mountains => ground = Ground::Mountains,
            Terrain::Snow => ground = Ground::Snow,
            Terrain::Sand => ground = Ground::Sand,
            Terrain::LightWoods => woods = Some(Woods::Light),
            Terrain::HeavyWoods => woods = Some(Woods::Heavy),
            Terrain::Water { depth } => {
                water = Some(Water {
                    depth,
                    frozen: false,
                })
            }
            Terrain::Ice { depth } => {
                water = Some(Water {
                    depth,
                    frozen: true,
                })
            }
            Terrain::Building { height } => structure = Some(Structure::Building { height }),
            Terrain::Wall { height } => structure = Some(Structure::Wall { height }),
        }
        if let Some(deck) = self.bridge {
            structure = Some(Structure::Bridge { deck });
        }
        BattleHex::from_layers(self.level, ground, woods, water, structure)
            .with_overlay(self.overlay)
    }
}

/// A generated battlefield: its environment and every hex, row by row from the top left.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HexMap {
    /// Width in hexes.
    pub width: u16,
    /// Height in hexes.
    pub height: u16,
    /// Gravity in percent of standard.
    pub gravity: u8,
    /// Temperature in degrees Celsius.
    pub temperature: i8,
    /// Battlefield rule flags.
    pub flags: Vec<BattleMapFlag>,
    /// Hexes in row-major order: index `y * width + x`.
    pub hexes: Vec<Hex>,
}

impl HexMap {
    /// A `width` by `height` map of clear level-0 ground.
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            gravity: 100,
            temperature: 20,
            flags: Vec::new(),
            hexes: vec![Hex::clear(0); usize::from(width) * usize::from(height)],
        }
    }

    /// Whether `(x, y)` is on the map.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        x >= 0 && y >= 0 && x < i32::from(self.width) && y < i32::from(self.height)
    }

    /// Row-major index of an on-map `(x, y)`.
    pub fn index(&self, x: i32, y: i32) -> usize {
        y as usize * usize::from(self.width) + x as usize
    }

    /// The `(x, y)` of a row-major index.
    pub fn coordinate(&self, index: usize) -> (i32, i32) {
        let width = usize::from(self.width);
        ((index % width) as i32, (index / width) as i32)
    }

    /// The hex at `(x, y)`, if it is on the map.
    pub fn hex(&self, x: i32, y: i32) -> Option<&Hex> {
        self.contains(x, y).then(|| &self.hexes[self.index(x, y)])
    }

    /// The hex at `(x, y)` for changing, if it is on the map.
    pub fn hex_mut(&mut self, x: i32, y: i32) -> Option<&mut Hex> {
        if !self.contains(x, y) {
            return None;
        }
        let index = self.index(x, y);
        Some(&mut self.hexes[index])
    }

    /// The neighbors of `(x, y)` indexed by direction clockwise from north, with `None` for
    /// each direction that leaves the map.
    pub fn adjacent(&self, x: i32, y: i32) -> [Option<(i32, i32)>; 6] {
        BattleHexCoordinate { x, y }
            .neighbors_within(self.width, self.height)
            .map(|neighbor| neighbor.map(|hex| (hex.x, hex.y)))
    }

    /// The up to six on-map neighbors of `(x, y)`, clockwise from north.
    pub fn neighbors(&self, x: i32, y: i32) -> impl Iterator<Item = (i32, i32)> + use<> {
        self.adjacent(x, y).into_iter().flatten()
    }

    /// Number of hex steps between two hexes, saturating at `i32::MAX`.
    pub fn distance((ax, ay): (i32, i32), (bx, by): (i32, i32)) -> i32 {
        let steps =
            BattleHexCoordinate { x: ax, y: ay }.distance(BattleHexCoordinate { x: bx, y: by });
        i32::try_from(steps).unwrap_or(i32::MAX)
    }

    /// Check every hex holds values a map file can store.
    pub fn validate(&self) -> Result<()> {
        ensure!(
            self.hexes.len() == usize::from(self.width) * usize::from(self.height),
            "map has {} hexes; {}x{} needs {}",
            self.hexes.len(),
            self.width,
            self.height,
            usize::from(self.width) * usize::from(self.height)
        );
        for (index, hex) in self.hexes.iter().enumerate() {
            let (x, y) = self.coordinate(index);
            ensure!(hex.level <= MAX_HEIGHT, "level too high at {x},{y}");
            if let Some(depth) = hex.terrain.depth() {
                ensure!(
                    (1..=MAX_DEPTH).contains(&depth),
                    "bad water depth at {x},{y}"
                );
            }
            if let Some(height) = hex.terrain.structure_height() {
                ensure!(
                    (1..=MAX_HEIGHT).contains(&height),
                    "bad structure height at {x},{y}"
                );
            }
            if let Some(deck) = hex.bridge {
                ensure!(hex.terrain.is_water(), "bridge off water at {x},{y}");
                ensure!(deck <= MAX_HEIGHT, "bridge deck too high at {x},{y}");
            }
        }
        Ok(())
    }

    /// The map as a [`BattleMapAsset`], after checking every hex holds values a map file can
    /// store.
    pub fn to_asset(&self) -> Result<BattleMapAsset> {
        self.validate()?;
        let flags = self
            .flags
            .iter()
            .fold(0, |bits, flag| flag.apply(bits, true));
        Ok(BattleMapAsset {
            width: self.width,
            height: self.height,
            flags: i32::try_from(flags)?,
            gravity: self.gravity,
            temperature: self.temperature,
            hexes: Arc::new(self.hexes.iter().map(|hex| hex.to_battle_hex()).collect()),
            points_of_interest: Vec::new(),
        })
    }

    /// Encode the map in the stompymux TOML map file format.
    pub fn to_toml(&self) -> Result<String> {
        self.to_asset()?.to_file()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbors_are_one_step_away_and_symmetric() {
        let map = HexMap::new(8, 8);
        for (x, y) in [(4, 4), (5, 4), (0, 0), (7, 2)] {
            for (nx, ny) in map.neighbors(x, y) {
                assert_eq!(HexMap::distance((x, y), (nx, ny)), 1);
                assert!(map.neighbors(nx, ny).any(|hex| hex == (x, y)));
            }
        }
        // Even columns sit south, so (2, 3)'s north-east neighbor is (3, 3).
        assert_eq!(map.adjacent(2, 3)[1], Some((3, 3)));
        assert_eq!(map.adjacent(3, 3)[1], Some((4, 2)));
        // A corner keeps the directions that stay on the map.
        assert_eq!(map.neighbors(0, 0).count(), 3);
        assert_eq!(map.adjacent(0, 0)[0], None);
    }

    #[test]
    fn writes_every_grid_and_bridge() {
        let mut map = HexMap::new(3, 2);
        map.flags = vec![BattleMapFlag::Dark];
        map.hexes[0].terrain = Terrain::Water { depth: 2 };
        map.hexes[0].bridge = Some(1);
        map.hexes[1].terrain = Terrain::Building { height: 12 };
        map.hexes[2].overlay = Some(BattleDecorationKind::Smoke);
        map.hexes[5].level = 11;
        let text = map.to_toml().unwrap();
        assert!(text.contains("flags = [\"dark\"]"), "{text}");
        assert!(text.contains("terrain = '''\n~@.\n...\n'''"), "{text}");
        assert!(text.contains("level = '''\n000\n00b\n'''"), "{text}");
        assert!(text.contains("depth = '''\n2..\n...\n'''"), "{text}");
        assert!(text.contains("structure_height = '''\n.c.\n"), "{text}");
        assert!(text.contains("overlay = '''\n..:\n"), "{text}");
        assert!(
            text.contains("[[bridges]]\ndeck = 1\nhexes = [[0, 0]]"),
            "{text}"
        );
    }

    #[test]
    fn rejects_values_a_map_file_cannot_hold() {
        let mut map = HexMap::new(2, 2);
        map.hexes[0].bridge = Some(1);
        assert!(map.to_toml().is_err());
        let mut map = HexMap::new(2, 2);
        map.hexes[0].terrain = Terrain::Water { depth: 12 };
        assert!(map.to_toml().is_err());
    }
}
