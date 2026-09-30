//! Bounded map-file decoding with explicit terrain, elevation and environmental metadata.
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
    HighWater,
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
            '?' => Self::HighWater,
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
            Self::HighWater => '?',
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

/// A terrain tile; depth is stored as a magnitude for water and ice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct BattleHex {
    pub terrain: Terrain,
    pub elevation: u8,
}

impl BattleHex {
    /// Supported standing surface; intact ice is at water level while its depth stays in the asset.
    pub fn standing_height(self) -> i16 {
        if self.terrain == Terrain::Ice {
            return 0;
        }
        self.surface_height()
    }

    /// Whether entering this hex at a rounded jump altitude hits an obstacle.
    /// Water entry uses immersion; bridge spans permit passage below their underside.
    /// This predicate does not authorize a route or move the unit.
    pub fn blocks_jump_entry(self, altitude: i32) -> bool {
        match self.terrain {
            Terrain::Water => false,
            Terrain::Bridge => altitude < 0 || altitude == i32::from(self.elevation) - 1,
            _ => altitude < i32::from(self.surface_height()),
        }
    }

    /// Bridge contact checked during vertical integration, before the hex transition.
    /// Unlike entry checks, this stage only collides at positive altitude.
    pub fn strikes_bridge_during_jump(self, altitude: i32) -> bool {
        self.terrain == Terrain::Bridge && altitude > 0 && self.blocks_jump_entry(altitude)
    }

    /// Terrain-relative height used by ground movement.
    pub fn surface_height(self) -> i16 {
        let elevation = i16::from(self.elevation);
        if matches!(self.terrain, Terrain::Water | Terrain::Ice) {
            return -elevation;
        }
        elevation
    }
}

/// Parsed source terrain, before simulation-specific overlays or bridge generation.
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

/// A terrain substitution recorded in source row order for the host's diagnostic channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct MapTerrainWarning {
    pub x: usize,
    pub y: u16,
    pub symbol: char,
}

/// Structural file failures used by map-loading presentation without parsing error strings.
#[derive(Debug, Clone, Copy)]
pub(super) enum MapFileFailure {
    Unavailable,
    Dimensions,
    Rows,
}

impl std::fmt::Display for MapFileFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unavailable => "#-1 Map not found.",
            Self::Dimensions => "#-1 Map invalid - Bad Height/Width.",
            Self::Rows => "#-1 Map invalid - Height not loaded properly",
        })
    }
}

impl std::error::Error for MapFileFailure {}

/// Consume one bounded file record, including its newline, as the map format reader does.
/// A NUL terminates the visible record but its remaining buffered bytes are consumed.
fn take_record<'a>(remaining: &mut &'a [u8], limit: usize) -> Option<&'a [u8]> {
    if remaining.is_empty() {
        return None;
    }
    let length = remaining
        .iter()
        .take(limit)
        .position(|&byte| byte == b'\n')
        .map_or(remaining.len().min(limit), |index| index + 1);
    let (record, rest) = remaining.split_at(length);
    *remaining = rest;
    Some(
        &record[..record
            .iter()
            .position(|&byte| byte == 0)
            .unwrap_or(record.len())],
    )
}

/// File fields split on the format's four delimiters, not arbitrary Unicode whitespace.
fn map_fields(record: &str) -> impl Iterator<Item = &str> {
    record
        .split([' ', '\t', '\r', '\n'])
        .filter(|part| !part.is_empty())
}

/// Checked signed decimal fields accept the six surrounding ASCII C whitespace bytes.
fn map_integer(value: &str) -> Option<i32> {
    value
        .trim_matches(|c: char| matches!(c, ' ' | '\t'..='\r'))
        .parse()
        .ok()
}

/// Optional conditions are applied only when the first post-terrain record is fully valid.
/// Invalid metadata leaves inferred fire flags and standard gravity/temperature intact.
fn parse_metadata(record: &[u8]) -> Option<(i32, i32, i32)> {
    let line = std::str::from_utf8(record).ok()?;
    let (flags, conditions) = line.split_once(':')?;
    let mut fields = map_fields(conditions);
    let gravity = map_integer(fields.next()?)?;
    let temperature = map_integer(fields.next()?)?;
    if fields.next().is_some() {
        return None;
    }
    let flags = map_integer(flags)?;
    Some((flags, gravity, temperature))
}

impl BattleMapAsset {
    /// Decode width/height, two-byte tiles, and optional `flags: gravity temperature`.
    /// Unknown terrain becomes grassland; malformed dimensions and elevations remain errors.
    pub fn parse(source: &str) -> Result<Self> {
        Self::parse_diagnostics(source.as_bytes(), 0).map(|(map, _)| map)
    }

    /// Decode with existing flags for reload; fresh inspection supplies zero.
    /// Return terrain substitutions without coupling decoding to channels.
    pub(super) fn parse_diagnostics(
        source: &[u8],
        initial_flags: i32,
    ) -> Result<(Self, Vec<MapTerrainWarning>)> {
        ensure!(source.len() <= 2_100_000, "map asset exceeds size limit");
        let mut remaining = source;
        let (width, height) = (|| -> Result<(u16, u16)> {
            let dimensions = take_record(&mut remaining, 63).context("missing map dimensions")?;
            let dimensions = std::str::from_utf8(dimensions).context("invalid map dimensions")?;
            let dimensions: Vec<_> = map_fields(dimensions).collect();
            ensure!(dimensions.len() == 2, "expected map width and height");
            let width = map_integer(dimensions[0]).context("invalid map width")?;
            let height = map_integer(dimensions[1]).context("invalid map height")?;
            ensure!(
                (1..=1000).contains(&width) && (1..=1000).contains(&height),
                "map dimensions must be between 1 and 1000"
            );
            Ok((width as u16, height as u16))
        })()
        .context(MapFileFailure::Dimensions)?;
        let mut hexes = Vec::with_capacity(usize::from(width) * usize::from(height));
        let mut warnings = Vec::new();
        for y in 0..height {
            let row = take_record(&mut remaining, 2001)
                .with_context(|| format!("missing map row {y}"))
                .context(MapFileFailure::Rows)?;
            let required = usize::from(width) * 2;
            if row.len() < required {
                return Err(anyhow::anyhow!(
                    "map row {y} must contain at least {width} terrain/elevation pairs"
                )
                .context(MapFileFailure::Rows));
            }
            let tiles = &row[..required];
            for (x, pair) in tiles.as_chunks::<2>().0.iter().enumerate() {
                let symbol = match pair[0] {
                    b'.' | b'>' | b':' => ' ',
                    b'\'' => '`',
                    value => char::from(value),
                };
                let terrain = Terrain::from_symbol(symbol).unwrap_or_else(|_| {
                    warnings.push(MapTerrainWarning { x, y, symbol });
                    Terrain::Grassland
                });
                ensure!(pair[1].is_ascii_digit(), "invalid elevation at {x},{y}");
                hexes.push(BattleHex {
                    terrain,
                    elevation: pair[1] - b'0',
                });
            }
        }
        let mut map = Self {
            width,
            height,
            // Authored fire is permanent unless explicit metadata overrides this default.
            flags: initial_flags
                | if hexes.iter().any(|hex| hex.terrain == Terrain::Fire) {
                    8
                } else {
                    0
                },
            gravity: 100,
            temperature: 20,
            hexes: Arc::new(hexes),
        };
        if let Some((flags, gravity, temperature)) =
            take_record(&mut remaining, 2002).and_then(parse_metadata)
        {
            map.flags = flags;
            map.gravity = gravity.clamp(0, 255) as u8;
            map.temperature = temperature.clamp(-128, 127) as i8;
        }
        Ok((map, warnings))
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

    /// Only authored fire infers the eternal-fire bit; explicit flags remain authoritative.
    #[test]
    fn authored_fire_flags_follow_metadata_precedence() {
        for (source, flags) in [
            ("1 1\n&2\n", 8),
            ("1 1\n>2\n", 0),
            ("1 1\n:2\n", 0),
            ("1 1\n.2\n", 0),
            ("2 1\n.0&2\n", 8),
            ("1 1\n&2\n0: 100 20\n", 0),
            ("1 1\n&2\n2: 75 -30\n", 2),
            ("1 1\n&2\n10: 75 -30\n", 10),
        ] {
            assert_eq!(
                BattleMapAsset::parse(source).unwrap().flags,
                flags,
                "{source}"
            );
        }
    }

    /// Only one optional record is read; invalid values cannot partially override conditions.
    #[test]
    fn optional_metadata_uses_reference_fallback_and_clamping() {
        for (metadata, expected) in [
            ("", (8, 100, 20)),
            ("\n2: 50 -40\n", (8, 100, 20)),
            ("comment\n2: 50 -40\n", (8, 100, 20)),
            ("2: 50\n", (8, 100, 20)),
            ("2: 50 -40 extra\n", (8, 100, 20)),
            ("2: 50 nope\n", (8, 100, 20)),
            ("2147483648: 50 -40\n", (8, 100, 20)),
            ("2: 2147483648 -40\n", (8, 100, 20)),
            ("2: 50 -2147483649\n", (8, 100, 20)),
            ("2: 50 -40\nignored\n", (2, 50, -40)),
            ("0: 300 -300\n", (0, 255, -128)),
            ("-1: -20 300\n", (-1, 0, 127)),
            ("+0: -0 +0\r\n", (0, 0, 0)),
            ("2: 50 -40\0 extra\n", (2, 50, -40)),
        ] {
            let map = BattleMapAsset::parse(&format!("1 1\n&2\n{metadata}")).unwrap();
            assert_eq!(
                (map.flags, map.gravity, map.temperature),
                expected,
                "{metadata:?}"
            );
        }
        let metadata = format!("0: 50 20{}extra\n", " ".repeat(1994));
        let map = BattleMapAsset::parse(&format!("1 1\n&2\n{metadata}")).unwrap();
        assert_eq!((map.flags, map.gravity, map.temperature), (0, 50, 20));
    }

    /// Reload inherits flags only in the absence of a valid explicit replacement.
    #[test]
    fn inherited_flags_and_authored_fire_obey_metadata_precedence() {
        for initial in [0, 49, 8, -1] {
            for (terrain, fire) in [(".0", 0), ("&0", 8)] {
                for (metadata, replacement) in [
                    ("", None),
                    ("invalid\n", None),
                    ("0: 100 20\n", Some(0)),
                    ("2: 50 -40\n", Some(2)),
                ] {
                    let source = format!("1 1\n{terrain}\n{metadata}");
                    let (map, _) =
                        BattleMapAsset::parse_diagnostics(source.as_bytes(), initial).unwrap();
                    assert_eq!(map.flags, replacement.unwrap_or(initial | fire));
                    assert_eq!(
                        BattleMapAsset::parse(&source).unwrap().flags,
                        replacement.unwrap_or(fire)
                    );
                }
            }
        }
    }

    /// Byte maps ignore uninterpreted suffixes and substitute unknown terrain before lookup.
    #[test]
    fn byte_map_records_decode_without_global_utf8_validation() {
        let (map, warnings) =
            BattleMapAsset::parse_diagnostics(b"2 1\n\xff3&2\xfe\n\xff: 50 20\n", 32).unwrap();
        assert_eq!((map.flags, map.gravity, map.temperature), (40, 100, 20));
        assert_eq!(
            map.hex(0, 0).unwrap(),
            BattleHex {
                terrain: Terrain::Grassland,
                elevation: 3
            }
        );
        assert_eq!(warnings.len(), 1);
        assert_eq!(
            (warnings[0].x, warnings[0].y, warnings[0].symbol),
            (0, 0, '\u{ff}')
        );
        assert!(BattleMapAsset::parse_diagnostics(b"1 \xff\n.0\n", 0).is_err());
        assert!(BattleMapAsset::parse_diagnostics(b"1 1\n.\xff\n", 0).is_err());
        let (map, warnings) =
            BattleMapAsset::parse_diagnostics(b"1 1\n.3\xff\n0: 50 20\n\xff", 0).unwrap();
        assert_eq!((map.flags, map.gravity, map.temperature), (0, 50, 20));
        assert!(warnings.is_empty());
    }

    /// Sand's `}` symbol decodes without a substitution warning and round-trips.
    #[test]
    fn sand_symbol_decodes_and_round_trips() {
        let (map, warnings) = BattleMapAsset::parse_diagnostics(b"2 1\n}1.0\n", 0).unwrap();
        assert!(warnings.is_empty());
        assert_eq!(
            map.hex(0, 0).unwrap(),
            BattleHex {
                terrain: Terrain::Sand,
                elevation: 1
            }
        );
        assert_eq!(Terrain::Sand.symbol(), '}');
        assert_eq!(Terrain::from_symbol('}').unwrap(), Terrain::Sand);
    }

    /// Delimiters and integer whitespace are distinct in the file format.
    #[test]
    fn numeric_file_fields_match_ascii_token_boundaries() {
        for header in ["+1 +1", "\u{b}1 1\u{c}", "1\u{c} \u{b}1"] {
            let map = BattleMapAsset::parse(&format!(
                "{header}\n.0\n\u{b}2\u{c}: \u{b}50\u{c} \u{c}-40\u{b}\n"
            ))
            .unwrap();
            assert_eq!((map.flags, map.gravity, map.temperature), (2, 50, -40));
        }
        for header in ["1\u{b}1", "1\u{a0}1", "\u{a0}1 1", "2147483648 1", "1 -0"] {
            assert!(
                BattleMapAsset::parse(&format!("{header}\n.0\n")).is_err(),
                "{header:?}"
            );
        }
        for metadata in ["2: 50\u{b}-40", "2: 50\u{a0}-40", "2: \u{a0}50 -40"] {
            let map = BattleMapAsset::parse(&format!("1 1\n&0\n{metadata}\n")).unwrap();
            assert_eq!((map.flags, map.gravity, map.temperature), (8, 100, 20));
        }
    }

    #[test]
    fn bridge_jump_collision_distinguishes_entry_and_vertical_integration() {
        for deck in 0..=9 {
            let bridge = BattleHex {
                terrain: Terrain::Bridge,
                elevation: deck,
            };
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
        let high_span = BattleHex {
            terrain: Terrain::Bridge,
            elevation: 9,
        };
        assert!(!high_span.blocks_jump_entry(4));
        assert!(high_span.blocks_jump_entry(8));
        assert!(!high_span.blocks_jump_entry(9));
    }

    #[test]
    fn jump_entry_uses_ground_height_and_preserves_water_entry() {
        for altitude in -4..=4 {
            let ground = BattleHex {
                terrain: Terrain::Grassland,
                elevation: 3,
            };
            let ice = BattleHex {
                terrain: Terrain::Ice,
                elevation: 3,
            };
            let water = BattleHex {
                terrain: Terrain::Water,
                elevation: 3,
            };
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
        let map = BattleMapAsset::parse("3 2\r\n.0~2'1\r\n#0-3^9\r\n32: 75 -12\r\n").unwrap();
        assert_eq!((map.width, map.height), (3, 2));
        assert_eq!(map.hex(1, 0).unwrap().surface_height(), -2);
        assert_eq!(map.hex(1, 1).unwrap().surface_height(), -3);
        assert_eq!(map.hex(2, 0).unwrap().terrain, Terrain::LightForest);
        assert_eq!((map.flags, map.gravity, map.temperature), (32, 75, -12));
        assert!(map.hex(-1, 0).is_none());
        assert!(map.hex(3, 0).is_none());
        assert!(map.hex(0, 2).is_none());
        assert!(Arc::ptr_eq(&map.hexes, &map.clone().hexes));
    }

    #[test]
    fn invalid_input_is_an_error_without_partial_maps() {
        for source in ["", "0 1", "1 1001", "2 1\n.0", "1 1\n.:"] {
            assert!(BattleMapAsset::parse(source).is_err(), "{source:?}");
        }
    }

    /// Row suffixes are discarded; a full input buffer continues at the next row.
    #[test]
    fn bounded_map_records_preserve_reference_row_consumption() {
        for source in [
            "1 1\n.3suffix\n",
            "1 1\n.3.8",
            "1 1\r\n.3suffix\r\n",
            "1 1\n.3\0ignored\n",
        ] {
            let map = BattleMapAsset::parse(source).unwrap();
            assert_eq!(map.hexes.len(), 1);
            assert_eq!(map.hex(0, 0).unwrap().elevation, 3);
        }
        let source = format!("1 2\n.3{}.7\n", "x".repeat(1999));
        let map = BattleMapAsset::parse(&source).unwrap();
        assert_eq!(map.hex(0, 0).unwrap().elevation, 3);
        assert_eq!(map.hex(0, 1).unwrap().elevation, 7);
        for ending in ["\n", "\r\n"] {
            let source = format!("1000 1\n{}{ending}", ".0".repeat(1000));
            assert_eq!(BattleMapAsset::parse(&source).unwrap().hexes.len(), 1000);
        }
        // The extra LF after a maximum-width CRLF row is the next bounded record.
        let source = format!("1000 2\n{}\r\n{}\r\n", ".0".repeat(1000), ".0".repeat(1000));
        assert!(BattleMapAsset::parse(&source).is_err());
        for source in ["1 1\n.\n", "1 1\n.\r\n", "1 1\n.\0ignored\n", "1 2\n.0\n"] {
            assert!(BattleMapAsset::parse(source).is_err(), "{source:?}");
        }
    }
}
