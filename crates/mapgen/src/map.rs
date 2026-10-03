//! The generated map: one [`Hex`] per cell in a column-staggered hex grid, and its encoding in
//! the stompymux TOML map file format.
//!
//! Columns are staggered like the game's: even columns sit half a hex south of odd ones.
//! [`HexMap::neighbors`] and [`HexMap::distance`] follow that layout, so roads and rivers built
//! from them connect in game.
use crate::spec::MapFlag;
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Write};

/// The tallest ground level, structure or bridge deck a map file allows.
pub const MAX_HEIGHT: u8 = 35;

/// The deepest water a map file allows.
pub const MAX_DEPTH: u8 = 9;

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

/// Permanent fire or smoke laid over a hex.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Overlay {
    /// Permanent fire (`&`).
    Fire,
    /// Permanent smoke (`:`).
    Smoke,
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
    pub overlay: Option<Overlay>,
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
    pub flags: Vec<MapFlag>,
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

    /// The up to six on-map neighbors of `(x, y)`, clockwise from north.
    pub fn neighbors(&self, x: i32, y: i32) -> impl Iterator<Item = (i32, i32)> + '_ {
        neighbors(x, y)
            .into_iter()
            .filter(|&(x, y)| self.contains(x, y))
    }

    /// Number of hex steps between two hexes.
    pub fn distance(a: (i32, i32), b: (i32, i32)) -> i32 {
        let [aq, ar, as_] = cube(a);
        let [bq, br, bs] = cube(b);
        (aq - bq).abs().max((ar - br).abs()).max((as_ - bs).abs())
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

    /// Encode the map in the stompymux TOML map file format.
    pub fn to_toml(&self) -> Result<String> {
        self.validate()?;
        let grid = |encode: &dyn Fn(&Hex) -> char| -> String {
            let mut text = String::new();
            for row in self.hexes.chunks(usize::from(self.width)) {
                text.extend(row.iter().map(encode));
                text.push('\n');
            }
            text
        };
        let flags = self
            .flags
            .iter()
            .map(|flag| format!("\"{}\"", flag.name()))
            .collect::<Vec<_>>()
            .join(", ");
        let mut text = String::new();
        writeln!(text, "gravity = {}", self.gravity)?;
        writeln!(text, "temperature = {}", self.temperature)?;
        writeln!(text, "flags = [{flags}]")?;
        writeln!(text)?;
        writeln!(
            text,
            "terrain = '''\n{}'''",
            grid(&|hex| hex.terrain.symbol())
        )?;
        writeln!(text, "level = '''\n{}'''", grid(&|hex| glyph(hex.level)))?;
        if self.hexes.iter().any(|hex| hex.terrain.is_water()) {
            let depth = grid(&|hex| hex.terrain.depth().map_or('.', glyph));
            writeln!(text, "depth = '''\n{depth}'''")?;
        }
        if self.hexes.iter().any(|hex| hex.terrain.is_structure()) {
            let heights = grid(&|hex| hex.terrain.structure_height().map_or('.', glyph));
            writeln!(text, "structure_height = '''\n{heights}'''")?;
        }
        if self.hexes.iter().any(|hex| hex.overlay.is_some()) {
            let overlay = grid(&|hex| match hex.overlay {
                Some(Overlay::Fire) => '&',
                Some(Overlay::Smoke) => ':',
                None => '.',
            });
            writeln!(text, "overlay = '''\n{overlay}'''")?;
        }
        let mut bridges: BTreeMap<u8, Vec<String>> = BTreeMap::new();
        for (index, hex) in self.hexes.iter().enumerate() {
            if let Some(deck) = hex.bridge {
                let (x, y) = self.coordinate(index);
                bridges.entry(deck).or_default().push(format!("[{x}, {y}]"));
            }
        }
        for (deck, hexes) in bridges {
            writeln!(text, "\n[[bridges]]\ndeck = {deck}")?;
            writeln!(text, "hexes = [{}]", hexes.join(", "))?;
        }
        Ok(text)
    }
}

/// One-character height: `0`-`9`, then `a`-`z` for 10 through 35.
fn glyph(value: u8) -> char {
    char::from_digit(u32::from(value), 36).expect("validated height")
}

/// Cube coordinates for adjacency and distance; even columns sit half a hex south.
fn cube((x, y): (i32, i32)) -> [i32; 3] {
    let r = y - (x + x.rem_euclid(2)) / 2;
    [x, r, -x - r]
}

/// The six neighbors of `(x, y)` clockwise from north, on or off the map.
pub(crate) fn neighbors(x: i32, y: i32) -> [(i32, i32); 6] {
    let [q, r, _] = cube((x, y));
    [(0, -1), (1, -1), (1, 0), (0, 1), (-1, 1), (-1, 0)].map(|(dq, dr)| {
        let q = q + dq;
        (q, r + dr + (q + q.rem_euclid(2)) / 2)
    })
}

/// Center of hex `(x, y)` in hex heights, for sampling smooth fields and measuring
/// straight-line distance.
pub(crate) fn center(x: i32, y: i32) -> (f64, f64) {
    let half = if x.rem_euclid(2) == 0 { 0.5 } else { 0.0 };
    (f64::from(x) * 3.0_f64.sqrt() / 2.0, f64::from(y) + half)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn neighbors_are_one_step_away_and_symmetric() {
        for (x, y) in [(4, 4), (5, 4), (0, 0), (7, 2)] {
            for (nx, ny) in neighbors(x, y) {
                assert_eq!(HexMap::distance((x, y), (nx, ny)), 1);
                assert!(neighbors(nx, ny).contains(&(x, y)));
            }
        }
        // Even columns sit south, so (2, 3)'s north-east neighbor is (3, 3).
        assert_eq!(neighbors(2, 3)[1], (3, 3));
        assert_eq!(neighbors(3, 3)[1], (4, 2));
    }

    #[test]
    fn writes_every_grid_and_bridge() {
        let mut map = HexMap::new(3, 2);
        map.flags = vec![MapFlag::Dark];
        map.hexes[0].terrain = Terrain::Water { depth: 2 };
        map.hexes[0].bridge = Some(1);
        map.hexes[1].terrain = Terrain::Building { height: 12 };
        map.hexes[2].overlay = Some(Overlay::Smoke);
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
