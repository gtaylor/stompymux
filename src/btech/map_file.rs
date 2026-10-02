//! The TOML map file format: environment settings, one-character-per-hex layer grids and an
//! explicit list of bridges.
//!
//! ```toml
//! gravity = 100              # optional, default 100
//! temperature = 20           # optional, default 20 (Celsius)
//! flags = ["dark"]           # optional; when absent a reload keeps the map's current flags
//!
//! terrain = '''
//! ..""~~#.
//! .^^"~~#.
//! '''
//! level = '''
//! 00110000
//! 02210000
//! '''
//! # Water depth under every ~ and - hex; . elsewhere.
//! depth = '''
//! ....23..
//! ....34..
//! '''
//!
//! [[bridges]]
//! deck = 2
//! hexes = [[4, 0], [5, 0]]
//! ```
//!
//! Grids are TOML literal strings (`'''`), since `"` is the heavy-woods symbol.
//! `structure_height` gives the height of every `@` (building) and `=` (wall) hex the same way
//! `depth` does for water. Heights use `0`-`9` then `a`-`z`. Width and height come from the
//! grids, whose rows must all be the same length.
use super::hex::MAX_HEIGHT;
use super::{
    BattleDecorationKind, BattleHex, BattleMapAsset, BattleMapFlag, Ground, Structure, Water, Woods,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fmt::Write, sync::Arc};

/// The largest map either dimension may have.
const MAX_DIMENSION: usize = 1000;

/// The deserialized shape of a map file.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct MapFile {
    #[serde(default = "default_gravity")]
    gravity: u8,
    #[serde(default = "default_temperature")]
    temperature: i8,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    flags: Option<Vec<BattleMapFlag>>,
    terrain: String,
    level: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    depth: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    structure_height: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    bridges: Vec<Bridge>,
}

/// A bridge deck spanning water hexes.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Bridge {
    /// Deck height above the water surface.
    deck: u8,
    /// `[x, y]` hexes the deck covers.
    hexes: Vec<[u16; 2]>,
}

fn default_gravity() -> u8 {
    100
}

fn default_temperature() -> i8 {
    20
}

/// What a terrain-grid character puts in a hex, before heights are applied.
#[derive(Clone, Copy)]
enum Cell {
    Ground(Ground),
    /// Permanent fire or smoke over clear ground.
    Overlay(BattleDecorationKind),
    Woods(Woods),
    Water {
        frozen: bool,
    },
    Building,
    Wall,
}

/// Decode one terrain-grid character.
fn cell(symbol: char) -> Option<Cell> {
    Some(match symbol {
        '.' => Cell::Ground(Ground::Clear),
        '#' => Cell::Ground(Ground::Road),
        '%' => Cell::Ground(Ground::Rough),
        '^' => Cell::Ground(Ground::Mountains),
        '+' => Cell::Ground(Ground::Snow),
        '}' => Cell::Ground(Ground::Sand),
        '&' => Cell::Overlay(BattleDecorationKind::Fire),
        ':' => Cell::Overlay(BattleDecorationKind::Smoke),
        '`' => Cell::Woods(Woods::Light),
        '"' => Cell::Woods(Woods::Heavy),
        '~' => Cell::Water { frozen: false },
        '-' => Cell::Water { frozen: true },
        '@' => Cell::Building,
        '=' => Cell::Wall,
        _ => return None,
    })
}

/// The terrain-grid character for a hex; bridges show the water beneath them.
/// Fire and smoke show only over clear ground; the file has no way to place them elsewhere.
fn symbol(hex: BattleHex) -> char {
    match hex.overlay() {
        Some(BattleDecorationKind::Fire) if overlay_fits(hex) => return '&',
        Some(BattleDecorationKind::Smoke) if overlay_fits(hex) => return ':',
        _ => {}
    }
    match (hex.structure(), hex.water(), hex.woods()) {
        (Some(Structure::Building { .. }), _, _) => '@',
        (Some(Structure::Wall { .. }), _, _) => '=',
        (_, Some(Water { frozen: true, .. }), _) => '-',
        (_, Some(Water { frozen: false, .. }), _) => '~',
        (_, None, Some(Woods::Light)) => '`',
        (_, None, Some(Woods::Heavy)) => '"',
        (_, None, None) => match hex.ground() {
            Ground::Clear => '.',
            Ground::Road => '#',
            Ground::Rough => '%',
            Ground::Mountains => '^',
            Ground::Snow => '+',
            Ground::Sand => '}',
        },
    }
}

/// Whether a map file can hold this hex's fire or smoke: only over bare clear ground.
pub(super) fn overlay_fits(hex: BattleHex) -> bool {
    hex.with_overlay(None) == BattleHex::from_layers(hex.level(), Ground::Clear, None, None, None)
}

/// Decode a height character: `0`-`9`, then `a`-`z` for 10 through 35.
fn height(symbol: char) -> Option<u8> {
    symbol.to_digit(36).map(|value| value as u8)
}

/// Encode a height as its grid character.
fn height_symbol(value: u8) -> char {
    char::from_digit(u32::from(value), 36).expect("height within grid range")
}

/// Split a grid into rows of characters, requiring a rectangle no larger than the map limit.
fn grid(name: &str, text: &str) -> Result<Vec<Vec<char>>> {
    let rows: Vec<Vec<char>> = text.lines().map(|row| row.chars().collect()).collect();
    ensure!(!rows.is_empty(), "{name} grid is empty");
    let width = rows[0].len();
    ensure!(width > 0, "{name} grid has an empty first row");
    for (y, row) in rows.iter().enumerate() {
        ensure!(
            row.len() == width,
            "{name} grid row {y} has {} hexes; the first row has {width}",
            row.len()
        );
    }
    ensure!(
        width <= MAX_DIMENSION && rows.len() <= MAX_DIMENSION,
        "{name} grid is larger than {MAX_DIMENSION}x{MAX_DIMENSION}"
    );
    Ok(rows)
}

/// Require an optional grid to have the terrain grid's shape.
fn matching_grid(
    name: &str,
    text: Option<&str>,
    width: usize,
    height: usize,
) -> Result<Option<Vec<Vec<char>>>> {
    let Some(text) = text else {
        return Ok(None);
    };
    let rows = grid(name, text)?;
    ensure!(
        rows.len() == height && rows[0].len() == width,
        "{name} grid is {}x{}; the terrain grid is {width}x{height}",
        rows[0].len(),
        rows.len()
    );
    Ok(Some(rows))
}

impl BattleMapAsset {
    /// Decode a map file. `inherited_flags` are kept when the file has no `flags` key, so a
    /// reload does not clear flags an operator set on the live map.
    pub fn parse(source: &str) -> Result<Self> {
        Self::parse_with_flags(source, 0)
    }

    /// Decode a map file, keeping `inherited_flags` when the file has no `flags` key.
    pub fn parse_with_flags(source: &str, inherited_flags: i64) -> Result<Self> {
        let file: MapFile = toml::from_str(source).context("invalid map file")?;
        let terrain = grid("terrain", &file.terrain)?;
        let (width, rows) = (terrain[0].len(), terrain.len());
        let level = matching_grid("level", Some(&file.level), width, rows)?.unwrap();
        let depth = matching_grid("depth", file.depth.as_deref(), width, rows)?;
        let stature = matching_grid(
            "structure_height",
            file.structure_height.as_deref(),
            width,
            rows,
        )?;
        let mut hexes = Vec::with_capacity(width * rows);
        for y in 0..rows {
            for x in 0..width {
                let at = || format!("at {x},{y}");
                let symbol = terrain[y][x];
                let cell = cell(symbol)
                    .with_context(|| format!("unknown terrain symbol {symbol:?} {}", at()))?;
                let ground = height(level[y][x])
                    .with_context(|| format!("invalid level {:?} {}", level[y][x], at()))?;
                let water = |frozen| -> Result<Water> {
                    let depth = depth
                        .as_ref()
                        .map(|rows| rows[y][x])
                        .with_context(|| format!("missing depth for water {}", at()))?;
                    let depth = depth
                        .to_digit(10)
                        .with_context(|| format!("invalid depth {depth:?} {}", at()))?;
                    Ok(Water {
                        depth: depth as u8,
                        frozen,
                    })
                };
                let structure_height = || -> Result<u8> {
                    let value = stature
                        .as_ref()
                        .map(|rows| rows[y][x])
                        .with_context(|| format!("missing structure height {}", at()))?;
                    height(value)
                        .with_context(|| format!("invalid structure height {value:?} {}", at()))
                };
                let hex = match cell {
                    Cell::Ground(kind) => BattleHex::from_layers(ground, kind, None, None, None),
                    Cell::Overlay(kind) => {
                        BattleHex::from_layers(ground, Ground::Clear, None, None, None)
                            .with_overlay(Some(kind))
                    }
                    Cell::Woods(woods) => {
                        BattleHex::from_layers(ground, Ground::Clear, Some(woods), None, None)
                    }
                    Cell::Water { frozen } => BattleHex::from_layers(
                        ground,
                        Ground::Clear,
                        None,
                        Some(water(frozen)?),
                        None,
                    ),
                    Cell::Building => BattleHex::from_layers(
                        ground,
                        Ground::Clear,
                        None,
                        None,
                        Some(Structure::Building {
                            height: structure_height()?,
                        }),
                    ),
                    Cell::Wall => BattleHex::from_layers(
                        ground,
                        Ground::Clear,
                        None,
                        None,
                        Some(Structure::Wall {
                            height: structure_height()?,
                        }),
                    ),
                };
                let water_cell = matches!(cell, Cell::Water { .. });
                if let Some(depth) = &depth
                    && !water_cell
                {
                    ensure!(
                        depth[y][x] == '.',
                        "depth given for a hex without water {}",
                        at()
                    );
                }
                if let Some(stature) = &stature
                    && !matches!(cell, Cell::Building | Cell::Wall)
                {
                    ensure!(
                        stature[y][x] == '.',
                        "structure height given for a hex without a structure {}",
                        at()
                    );
                }
                hexes.push(hex);
            }
        }
        for bridge in &file.bridges {
            ensure!(
                bridge.deck <= MAX_HEIGHT,
                "bridge deck {} is too high",
                bridge.deck
            );
            ensure!(
                !bridge.hexes.is_empty(),
                "a bridge must cover at least one hex"
            );
            for &[x, y] in &bridge.hexes {
                let (x, y) = (usize::from(x), usize::from(y));
                ensure!(x < width && y < rows, "bridge hex {x},{y} is off the map");
                let hex = &mut hexes[y * width + x];
                ensure!(
                    hex.water().is_some() && hex.structure().is_none(),
                    "bridge hex {x},{y} must be water or ice without another structure"
                );
                *hex = hex.with_structure(Some(Structure::Bridge { deck: bridge.deck }));
            }
        }
        let flags = match file.flags {
            Some(flags) => flags
                .into_iter()
                .fold(0, |bits, flag| flag.apply(bits, true)),
            None => inherited_flags,
        };
        Ok(Self {
            width: width as u16,
            height: rows as u16,
            flags: i32::try_from(flags).context("invalid map flags")?,
            gravity: file.gravity,
            temperature: file.temperature,
            hexes: Arc::new(hexes),
        })
    }

    /// Encode this map in the map file format. Absent optional grids are left out.
    pub fn to_file(&self) -> Result<String> {
        let width = usize::from(self.width);
        let rows = |encode: &dyn Fn(BattleHex) -> char| -> String {
            let mut text = String::with_capacity((width + 1) * usize::from(self.height));
            for row in self.hexes.chunks(width) {
                text.extend(row.iter().map(|&hex| encode(hex)));
                text.push('\n');
            }
            text
        };
        for hex in self.hexes.iter() {
            ensure!(
                hex.level() <= MAX_HEIGHT,
                "level {} is too high",
                hex.level()
            );
            if let Some(Structure::Building { height } | Structure::Wall { height }) =
                hex.structure()
            {
                ensure!(
                    height <= MAX_HEIGHT,
                    "structure height {height} is too high"
                );
            }
        }
        let has_water = self.hexes.iter().any(|hex| hex.water().is_some());
        let has_structures = self.hexes.iter().any(|hex| {
            matches!(
                hex.structure(),
                Some(Structure::Building { .. } | Structure::Wall { .. })
            )
        });
        let mut bridges: BTreeMap<u8, Vec<[u16; 2]>> = BTreeMap::new();
        for (index, hex) in self.hexes.iter().enumerate() {
            if let Some(deck) = hex.deck_clearance() {
                bridges
                    .entry(deck)
                    .or_default()
                    .push([(index % width) as u16, (index / width) as u16]);
            }
        }
        let flags = BattleMapFlag::ALL
            .into_iter()
            .filter(|flag| flag.is_set(i64::from(self.flags)))
            .collect::<Vec<_>>();
        let mut text = String::new();
        writeln!(text, "gravity = {}", self.gravity)?;
        writeln!(text, "temperature = {}", self.temperature)?;
        writeln!(
            text,
            "flags = {}",
            toml::to_string(&Flags { flags })?
                .trim_start_matches("flags = ")
                .trim_end()
        )?;
        writeln!(text)?;
        writeln!(text, "terrain = '''\n{}'''", rows(&symbol))?;
        writeln!(
            text,
            "level = '''\n{}'''",
            rows(&|hex| height_symbol(hex.level()))
        )?;
        if has_water {
            writeln!(
                text,
                "depth = '''\n{}'''",
                rows(&|hex| match hex.water() {
                    Some(water) => height_symbol(water.depth),
                    None => '.',
                })
            )?;
        }
        if has_structures {
            writeln!(
                text,
                "structure_height = '''\n{}'''",
                rows(&|hex| match hex.structure() {
                    Some(Structure::Building { height } | Structure::Wall { height }) => {
                        height_symbol(height)
                    }
                    _ => '.',
                })
            )?;
        }
        for (deck, hexes) in bridges {
            writeln!(text, "\n[[bridges]]\ndeck = {deck}")?;
            let hexes = hexes
                .iter()
                .map(|[x, y]| format!("[{x}, {y}]"))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(text, "hexes = [{hexes}]")?;
        }
        Ok(text)
    }
}

/// Helper so the flag list is written with TOML's own string quoting.
#[derive(Serialize)]
struct Flags {
    flags: Vec<BattleMapFlag>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::Terrain;

    const SAMPLE: &str = r#"
gravity = 80
temperature = -10
flags = ["dark", "special_rules"]

terrain = '''
.`"~-
#%^@=
'''
level = '''
01200
1a000
'''
depth = '''
...23
.....
'''
structure_height = '''
.....
...45
'''

[[bridges]]
deck = 2
hexes = [[3, 0]]
"#;

    #[test]
    fn parses_every_layer() {
        let map = BattleMapAsset::parse(SAMPLE).unwrap();
        assert_eq!((map.width, map.height), (5, 2));
        assert_eq!((map.gravity, map.temperature), (80, -10));
        assert_eq!(i64::from(map.flags), 2 | 32);
        let hex = |x, y| map.hex(x, y).unwrap();
        assert_eq!(hex(0, 0), BattleHex::new(Terrain::Grassland, 0));
        assert_eq!(hex(1, 0), BattleHex::new(Terrain::LightForest, 1));
        assert_eq!(hex(2, 0), BattleHex::new(Terrain::HeavyForest, 2));
        assert_eq!(hex(3, 0).deck_clearance(), Some(2));
        assert_eq!(hex(3, 0).water_depth(), 2);
        assert_eq!(hex(4, 0), BattleHex::new(Terrain::Ice, 3));
        assert_eq!(hex(0, 1), BattleHex::new(Terrain::Road, 1));
        assert_eq!(hex(1, 1).level(), 10);
        assert_eq!(hex(2, 1), BattleHex::new(Terrain::Mountains, 0));
        assert_eq!(hex(3, 1), BattleHex::new(Terrain::Building, 4));
        assert_eq!(hex(4, 1), BattleHex::new(Terrain::Wall, 5));
    }

    #[test]
    fn writes_what_it_reads() {
        let map = BattleMapAsset::parse(SAMPLE).unwrap();
        let text = map.to_file().unwrap();
        assert_eq!(BattleMapAsset::parse(&text).unwrap(), map);
        assert_eq!(map.to_file().unwrap(), text);
        // Three heavy-woods hexes in a row would end a basic multi-line string.
        let woods = BattleMapAsset::from_cells("3 1\n\"0\"0\"0\n").unwrap();
        assert_eq!(
            BattleMapAsset::parse(&woods.to_file().unwrap()).unwrap(),
            woods
        );
        let plain = BattleMapAsset::parse("terrain = \"..\\n\"\nlevel = \"01\\n\"").unwrap();
        let text = plain.to_file().unwrap();
        assert!(
            !text.contains("depth") && !text.contains("bridges"),
            "{text}"
        );
        assert_eq!(BattleMapAsset::parse(&text).unwrap(), plain);
    }

    /// A building on high ground keeps both heights; its top is their sum.
    #[test]
    fn structures_on_raised_ground_load_and_validate() {
        let source = "terrain = '@'\nlevel = 'a'\nstructure_height = 'b'\n";
        let map = BattleMapAsset::parse(source).unwrap();
        let tower = map.hex(0, 0).unwrap();
        assert_eq!((tower.level(), tower.surface_height()), (10, 21));
        tower.validate().unwrap();
        assert_eq!(BattleMapAsset::parse(&map.to_file().unwrap()).unwrap(), map);
        let stored = crate::btech::state::map_from_asset("tower", map).unwrap();
        stored.validate().unwrap();
    }

    /// A lake and a bridge on a plateau keep their surfaces at the plateau's level.
    #[test]
    fn water_and_bridges_sit_on_raised_ground() {
        let source = "terrain = '~-~'\nlevel = '432'\ndepth = '231'\n\n[[bridges]]\ndeck = 2\nhexes = [[2, 0]]\n";
        let map = BattleMapAsset::parse(source).unwrap();
        let lake = map.hex(0, 0).unwrap();
        assert_eq!((lake.water_line(), lake.surface_height()), (4, 2));
        let ice = map.hex(1, 0).unwrap();
        assert_eq!((ice.standing_height(), ice.surface_height()), (3, 0));
        let bridge = map.hex(2, 0).unwrap();
        assert_eq!((bridge.water_line(), bridge.deck_height()), (2, Some(4)));
        assert_eq!(BattleMapAsset::parse(&map.to_file().unwrap()).unwrap(), map);
        crate::btech::state::map_from_asset("plateau", map)
            .unwrap()
            .validate()
            .unwrap();
    }

    /// `&` and `:` are permanent fire and smoke over clear ground; the map holds them as
    /// decorations, never in its terrain.
    #[test]
    fn fire_and_smoke_load_as_permanent_overlays() {
        let map = BattleMapAsset::parse("terrain = '&:'\nlevel = '12'\n").unwrap();
        let fire = map.hex(0, 0).unwrap();
        assert_eq!(fire.overlay(), Some(BattleDecorationKind::Fire));
        assert_eq!(
            fire.with_overlay(None),
            BattleHex::new(Terrain::Grassland, 1)
        );
        assert_eq!(map.hex(1, 0).unwrap(), BattleHex::new(Terrain::Smoke, 2));
        assert_eq!(BattleMapAsset::parse(&map.to_file().unwrap()).unwrap(), map);
        let stored = crate::btech::state::map_from_asset("burning", map).unwrap();
        assert_eq!(
            stored.base_hex(0, 0).unwrap(),
            BattleHex::new(Terrain::Grassland, 1)
        );
        assert_eq!(stored.hex(0, 0).unwrap(), fire);
        let effect = stored
            .decoration(super::super::BattleHexCoordinate { x: 1, y: 0 })
            .unwrap()
            .unwrap();
        assert_eq!(
            (effect.kind, effect.remaining),
            (BattleDecorationKind::Smoke, 0)
        );
        // Fire over anything but clear ground has no symbol, so the file keeps the woods.
        let woods = BattleHex::new(Terrain::LightForest, 1);
        let burning = BattleMapAsset {
            hexes: Arc::new(vec![woods.with_overlay(Some(BattleDecorationKind::Fire))]),
            ..BattleMapAsset::parse("terrain = '.'\nlevel = '0'\n").unwrap()
        };
        assert_eq!(
            BattleMapAsset::parse(&burning.to_file().unwrap())
                .unwrap()
                .hex(0, 0),
            Some(woods)
        );
    }

    #[test]
    fn missing_flags_keep_inherited_ones() {
        let source = "terrain = \".\\n\"\nlevel = \"0\\n\"";
        assert_eq!(
            BattleMapAsset::parse_with_flags(source, 32).unwrap().flags,
            32
        );
        let explicit = format!("flags = []\n{source}");
        assert_eq!(
            BattleMapAsset::parse_with_flags(&explicit, 32)
                .unwrap()
                .flags,
            0
        );
    }

    #[test]
    fn rejects_malformed_maps() {
        let base = |terrain: &str, level: &str, extra: &str| {
            format!("terrain = \"\"\"\n{terrain}\"\"\"\nlevel = \"\"\"\n{level}\"\"\"\n{extra}")
        };
        for (source, message) in [
            (base("..\n.\n", "00\n0\n", ""), "row 1 has 1"),
            (base("..\n", "000\n", ""), "level grid is 3x1"),
            (base("X\n", "0\n", ""), "unknown terrain symbol 'X'"),
            (base(".\n", "!\n", ""), "invalid level"),
            (base("~\n", "0\n", ""), "missing depth"),
            (base(".\n", "0\n", "depth = \"2\\n\""), "without water"),
            (base("@\n", "0\n", ""), "missing structure height"),
            (
                base(".\n", "0\n", "[[bridges]]\ndeck = 1\nhexes = [[0, 0]]"),
                "must be water",
            ),
            (
                base(
                    "~\n",
                    "0\n",
                    "depth = \"1\\n\"\n[[bridges]]\ndeck = 1\nhexes = [[1, 0]]",
                ),
                "off the map",
            ),
            (
                base(".\n", "0\n", "flags = [\"bogus\"]"),
                "invalid map file",
            ),
            (base(".\n", "0\n", "colour = 1"), "invalid map file"),
        ] {
            let error = format!("{:#}", BattleMapAsset::parse(&source).unwrap_err());
            assert!(error.contains(message), "{message:?} not in {error:?}");
        }
    }
}
