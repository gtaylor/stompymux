//! Bounded map-file decoding with explicit terrain, elevation and environmental metadata.
use super::BattleHex;
use anyhow::{Context, Result, bail, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Terrain identity; its spelling belongs to the map-file codec.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Terrain {
    Grassland,
    Road,
    LightForest,
    HeavyForest,
    Water,
    Ice,
    Bridge,
    Rough,
    Mountains,
    Fire,
    Smoke,
    Snow,
    Building,
    Wall,
    Sand,
}

impl Terrain {
    /// Every terrain, in symbol-table order.
    pub const ALL: [Self; 15] = [
        Self::Grassland,
        Self::Road,
        Self::LightForest,
        Self::HeavyForest,
        Self::Water,
        Self::Ice,
        Self::Bridge,
        Self::Rough,
        Self::Mountains,
        Self::Fire,
        Self::Smoke,
        Self::Snow,
        Self::Building,
        Self::Wall,
        Self::Sand,
    ];

    /// Snake_case name shared by serialization, Lua reports and Lua arguments.
    pub const fn name(self) -> &'static str {
        match self {
            Self::Grassland => "grassland",
            Self::Road => "road",
            Self::LightForest => "light_forest",
            Self::HeavyForest => "heavy_forest",
            Self::Water => "water",
            Self::Ice => "ice",
            Self::Bridge => "bridge",
            Self::Rough => "rough",
            Self::Mountains => "mountains",
            Self::Fire => "fire",
            Self::Smoke => "smoke",
            Self::Snow => "snow",
            Self::Building => "building",
            Self::Wall => "wall",
            Self::Sand => "sand",
        }
    }

    /// Decode a snake_case terrain name.
    pub fn from_name(name: &str) -> Result<Self> {
        Self::ALL
            .into_iter()
            .find(|terrain| terrain.name() == name)
            .with_context(|| format!("unknown terrain {name:?}"))
    }

    /// Decode a canonical terrain symbol without applying asset-file normalization.
    pub fn from_symbol(symbol: char) -> Result<Self> {
        Ok(match symbol {
            ' ' => Self::Grassland,
            '#' => Self::Road,
            '`' => Self::LightForest,
            '"' => Self::HeavyForest,
            '~' => Self::Water,
            '-' => Self::Ice,
            '/' => Self::Bridge,
            '%' => Self::Rough,
            '^' => Self::Mountains,
            '&' => Self::Fire,
            ':' => Self::Smoke,
            '+' => Self::Snow,
            '@' => Self::Building,
            '=' => Self::Wall,
            '}' => Self::Sand,
            _ => bail!("unknown terrain symbol {symbol:?}"),
        })
    }

    /// Encode the canonical symbol used by map assets.
    pub fn symbol(self) -> char {
        match self {
            Self::Grassland => ' ',
            Self::Road => '#',
            Self::LightForest => '`',
            Self::HeavyForest => '"',
            Self::Water => '~',
            Self::Ice => '-',
            Self::Bridge => '/',
            Self::Rough => '%',
            Self::Mountains => '^',
            Self::Fire => '&',
            Self::Smoke => ':',
            Self::Snow => '+',
            Self::Building => '@',
            Self::Wall => '=',
            Self::Sand => '}',
        }
    }
}

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
}

/// Why a named map file could not be loaded, without parsing error strings.
#[derive(Debug, Clone, Copy)]
pub(super) enum MapFileFailure {
    /// No map file has that name.
    Unavailable,
    /// The file exists but is not a valid map; the error chain carries the reason.
    Invalid,
}

impl std::fmt::Display for MapFileFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "#-1 Map not found.",
            Self::Invalid => "#-1 Map invalid.",
        })
    }
}

impl std::error::Error for MapFileFailure {}

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

    /// Names and symbols are distinct for every terrain and agree with serialization.
    #[test]
    fn terrain_names_and_symbols_round_trip() {
        let mut symbols = std::collections::BTreeSet::new();
        for terrain in Terrain::ALL {
            assert!(symbols.insert(terrain.symbol()), "{terrain:?}");
            assert_eq!(Terrain::from_symbol(terrain.symbol()).unwrap(), terrain);
            assert_eq!(Terrain::from_name(terrain.name()).unwrap(), terrain);
            assert_eq!(
                serde_json::to_value(terrain).unwrap(),
                serde_json::Value::from(terrain.name())
            );
        }
        assert!(Terrain::from_name("Heavy_Forest").is_err());
        assert!(Terrain::from_name("\"").is_err());
    }

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
