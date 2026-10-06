//! The generated map: one layered [`Hex`] per cell in a column-staggered hex grid, and its
//! conversion to the [`MapAsset`] that `stompymux-map` writes as a map file.
//!
//! Generation works directly on `stompymux-map`'s hexes, so every layer the game knows
//! (ground, water, foliage, route, structure and weather condition) can be generated.
//!
//! Columns are staggered like the game's: even columns sit half a hex south of odd ones.
//! [`HexMap::neighbors`] and [`HexMap::distance`] follow that layout, so roads and rivers built
//! from them connect in game.
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use stompymux_map::{Hex, HexCoordinate, MapAsset, MapFlag};

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
            hexes: vec![Hex::at_level(0); usize::from(width) * usize::from(height)],
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
        HexCoordinate { x, y }
            .neighbors_within(self.width, self.height)
            .map(|neighbor| neighbor.map(|hex| (hex.x, hex.y)))
    }

    /// The up to six on-map neighbors of `(x, y)`, clockwise from north.
    pub fn neighbors(&self, x: i32, y: i32) -> impl Iterator<Item = (i32, i32)> + use<> {
        self.adjacent(x, y).into_iter().flatten()
    }

    /// Number of hex steps between two hexes, saturating at `i32::MAX`.
    pub fn distance((ax, ay): (i32, i32), (bx, by): (i32, i32)) -> i32 {
        let steps = HexCoordinate { x: ax, y: ay }.distance(HexCoordinate { x: bx, y: by });
        i32::try_from(steps).unwrap_or(i32::MAX)
    }

    /// Check every hex is one a map file can store: valid layers within the battlefield's
    /// limits, water at least one level deep and buildings and walls at least one level tall.
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
            hex.validate().with_context(|| format!("at {x},{y}"))?;
            if let Some(water) = hex.water() {
                ensure!(water.depth >= 1, "dry water bed at {x},{y}");
            }
            if let Some(structure) = hex.structure()
                && structure.is_standing()
            {
                ensure!(structure.height >= 1, "flat structure at {x},{y}");
            }
        }
        Ok(())
    }

    /// The map as a [`MapAsset`], after checking every hex holds values a map file can
    /// store.
    pub fn to_asset(&self) -> Result<MapAsset> {
        self.validate()?;
        let flags = self
            .flags
            .iter()
            .fold(0, |bits, flag| flag.apply(bits, true));
        Ok(MapAsset {
            width: self.width,
            height: self.height,
            flags: i32::try_from(flags)?,
            gravity: self.gravity,
            temperature: self.temperature,
            light: None,
            visibility: None,
            wind: None,
            hexes: Arc::new(self.hexes.clone()),
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
    use stompymux_map::{Condition, DecorationKind, Foliage, Ground, Route, Structure, Water};

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
    fn writes_every_layer_and_structure() {
        let mut map = HexMap::new(3, 2);
        map.flags = vec![MapFlag::Dark];
        map.hexes[0] = Hex::at_level(0)
            .with_water(Some(Water::still(2)))
            .with_structure(Some(Structure::bridge(1)))
            .with_condition(Some(Condition::Ice));
        map.hexes[1] = Hex::at_level(0)
            .with_ground(Ground::Pavement)
            .with_structure(Some(Structure::building(12)));
        map.hexes[2] = map.hexes[2].with_overlay(Some(DecorationKind::Smoke));
        map.hexes[3] = Hex::at_level(1)
            .with_foliage(Some(Foliage::HeavyJungle))
            .with_route(Some(Route::DirtRoad));
        map.hexes[4] = Hex::at_level(3)
            .with_ground(Ground::UltraRough)
            .with_condition(Some(Condition::DeepSnow));
        map.hexes[5] = map.hexes[5].with_level(11);
        let text = map.to_toml().unwrap();
        assert!(text.contains("flags = [\"dark\"]"), "{text}");
        assert!(text.contains("terrain = '''\n~_.\n.^.\n'''"), "{text}");
        assert!(text.contains("level = '''\n000\n13b\n'''"), "{text}");
        assert!(text.contains("depth = '''\n2..\n...\n'''"), "{text}");
        assert!(text.contains("foliage = '''\n...\nJ..\n'''"), "{text}");
        assert!(text.contains("route = '''\n...\nd..\n'''"), "{text}");
        assert!(text.contains("condition = '''\n-..\n.+.\n'''"), "{text}");
        assert!(text.contains("overlay = '''\n..:\n"), "{text}");
        assert!(
            text.contains("kind = \"bridge\"\nclass = \"medium\"\nheight = 1\nhexes = [[0, 0]]"),
            "{text}"
        );
        assert!(text.contains("kind = \"building\""), "{text}");
        assert_eq!(MapAsset::parse(&text).unwrap(), map.to_asset().unwrap());
    }

    #[test]
    fn rejects_values_a_map_file_cannot_hold() {
        let mut map = HexMap::new(2, 2);
        map.hexes[0] = map.hexes[0].with_structure(Some(Structure::bridge(1)));
        assert!(map.to_toml().is_err(), "a bridge needs water");
        let mut map = HexMap::new(2, 2);
        map.hexes[0] = map.hexes[0].with_water(Some(Water::still(12)));
        assert!(map.to_toml().is_err(), "too deep");
        let mut map = HexMap::new(2, 2);
        map.hexes[0] = map.hexes[0].with_water(Some(Water::still(0)));
        assert!(map.validate().is_err(), "dry water bed");
    }
}
