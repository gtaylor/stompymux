//! Map assets: a battlefield's dimensions, environment, hexes and scripted points of
//! interest, as read from and written to map files.
use crate::{BattleHex, Terrain};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Parsed map terrain and settings; fire and smoke drawn in the file are hex overlays.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleMapAsset {
    pub width: u16,
    pub height: u16,
    pub flags: i32,
    pub gravity: u8,
    pub temperature: i8,
    /// Row-major immutable tiles shared by transaction checkpoints.
    pub hexes: Arc<Vec<BattleHex>>,
    /// Scripted points of interest in file order.
    pub points_of_interest: Vec<MapPointOfInterest>,
}

/// A scripted point of interest on a map. Points of interest are map metadata for scripts:
/// they are never shown to units and do not change terrain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapPointOfInterest {
    /// Case-sensitive category chosen by the map author, such as `objective`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Display name chosen by the map author.
    pub name: String,
    /// Zero-based column.
    pub x: u16,
    /// Zero-based row.
    pub y: u16,
    /// Height in levels relative to the hex's ground level; absent when the point has no
    /// particular height.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub elevation: Option<i8>,
}

impl MapPointOfInterest {
    /// Require a non-empty type and name without NUL characters, and a hex inside a
    /// `width` by `height` map.
    pub fn validate(&self, width: i64, height: i64) -> Result<()> {
        for (field, value) in [("type", &self.kind), ("name", &self.name)] {
            ensure!(
                !value.is_empty() && !value.contains('\0'),
                "point of interest {field} must be non-empty text without NUL characters"
            );
        }
        ensure!(
            i64::from(self.x) < width && i64::from(self.y) < height,
            "point of interest {:?} at {},{} is off the map",
            self.name,
            self.x,
            self.y
        );
        Ok(())
    }
}

impl BattleMapAsset {
    /// Build a map from the compact cell notation used to set up maps in code and tests: a
    /// `width height` line, then one line per row of terrain-symbol and elevation-digit pairs
    /// (`.` is grassland), then optionally `flags: gravity temperature`.
    pub fn from_cells(source: &str) -> Result<Self> {
        let mut lines = source.lines();
        let header = lines.next().context("missing map dimensions")?;
        let (width, height) = header
            .split_once(' ')
            .and_then(|(width, height)| Some((width.parse().ok()?, height.parse().ok()?)))
            .context("expected map width and height")?;
        ensure!(
            (1..=1000).contains(&width) && (1..=1000).contains(&height),
            "map dimensions must be between 1 and 1000"
        );
        let mut hexes = Vec::with_capacity(usize::from(width) * usize::from(height));
        for y in 0..height {
            let row = lines
                .next()
                .with_context(|| format!("missing map row {y}"))?
                .as_bytes();
            ensure!(
                row.len() == usize::from(width) * 2,
                "map row {y} must contain exactly {width} terrain/elevation pairs"
            );
            for (x, pair) in row.as_chunks::<2>().0.iter().enumerate() {
                let terrain = match pair[0] {
                    b'.' => Terrain::Grassland,
                    symbol => Terrain::from_symbol(char::from(symbol))
                        .with_context(|| format!("at {x},{y}"))?,
                };
                ensure!(pair[1].is_ascii_digit(), "invalid elevation at {x},{y}");
                hexes.push(BattleHex::new(terrain, pair[1] - b'0'));
            }
        }
        let (mut flags, mut gravity, mut temperature) = (0, 100, 20);
        if let Some(settings) = lines.next() {
            let (bits, conditions) = settings
                .split_once(':')
                .context("expected `flags: gravity temperature`")?;
            let conditions: Vec<_> = conditions.split_whitespace().collect();
            ensure!(
                conditions.len() == 2,
                "expected `flags: gravity temperature`"
            );
            flags = bits.trim().parse().context("invalid map flags")?;
            gravity = conditions[0].parse().context("invalid gravity")?;
            temperature = conditions[1].parse().context("invalid temperature")?;
        }
        ensure!(lines.next().is_none(), "unexpected text after the map");
        Ok(Self {
            width,
            height,
            flags,
            gravity,
            temperature,
            hexes: Arc::new(hexes),
            points_of_interest: Vec::new(),
        })
    }

    /// Resolve a tile without wrapping negative or out-of-range coordinates.
    pub fn hex(&self, x: i32, y: i32) -> Option<BattleHex> {
        if x < 0 || y < 0 || x >= i32::from(self.width) || y >= i32::from(self.height) {
            return None;
        }
        self.hexes
            .get(y as usize * usize::from(self.width) + x as usize)
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bridge_jump_collision_distinguishes_entry_and_vertical_integration() {
        for deck in 0..=9 {
            let bridge = BattleHex::new(Terrain::Bridge, deck);
            for altitude in -3..=12 {
                assert_eq!(
                    bridge.blocks_jump_entry(altitude),
                    altitude < 0 || altitude == i32::from(deck) - 1
                );
                assert_eq!(
                    bridge.strikes_bridge_during_jump(altitude),
                    altitude > 0 && altitude == i32::from(deck) - 1
                );
            }
        }
        let high_span = BattleHex::new(Terrain::Bridge, 9);
        assert!(!high_span.blocks_jump_entry(4));
        assert!(high_span.blocks_jump_entry(8));
        assert!(!high_span.blocks_jump_entry(9));
    }

    #[test]
    fn jump_entry_uses_ground_height_and_preserves_water_entry() {
        for altitude in -4..=4 {
            let ground = BattleHex::new(Terrain::Grassland, 3);
            let ice = BattleHex::new(Terrain::Ice, 3);
            let water = BattleHex::new(Terrain::Water, 3);
            assert_eq!(ground.blocks_jump_entry(altitude), altitude < 3);
            assert_eq!(ice.blocks_jump_entry(altitude), altitude < -3);
            assert!(!water.blocks_jump_entry(altitude));
            assert!(!ground.strikes_bridge_during_jump(altitude));
            assert!(!ice.strikes_bridge_during_jump(altitude));
            assert!(!water.strikes_bridge_during_jump(altitude));
        }
    }

    #[test]
    fn rectangular_map_preserves_axes_depth_and_environment() {
        let map = BattleMapAsset::from_cells("3 2\n.0~2`1\n#0-3^9\n32: 75 -12\n").unwrap();
        assert_eq!((map.width, map.height), (3, 2));
        assert_eq!(map.hex(1, 0).unwrap().surface_height(), -2);
        assert_eq!(map.hex(1, 1).unwrap().surface_height(), -3);
        assert_eq!(map.hex(2, 0).unwrap().terrain(), Terrain::LightForest);
        assert_eq!((map.flags, map.gravity, map.temperature), (32, 75, -12));
        assert!(map.hex(-1, 0).is_none());
        assert!(map.hex(3, 0).is_none());
        assert!(map.hex(0, 2).is_none());
        assert!(Arc::ptr_eq(&map.hexes, &map.clone().hexes));
    }

    #[test]
    fn invalid_input_is_an_error_without_partial_maps() {
        for source in ["", "0 1", "1 1001", "2 1\n.0", "1 1\n.:"] {
            assert!(BattleMapAsset::from_cells(source).is_err(), "{source:?}");
        }
    }
}
