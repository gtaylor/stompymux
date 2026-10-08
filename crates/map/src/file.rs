//! The TOML map file format: battlefield conditions, one grid per hex layer and an explicit list
//! of structures.
//!
//! ```toml
//! gravity = 100              # optional, default 100
//! temperature = 20           # optional, default 20 (Celsius)
//! light = "day"              # optional: day, twilight or night
//! visibility = 30            # optional, weather visibility in hexes, 0 to 60
//! wind = { direction = 90, speed = 10 }  # optional
//! flags = ["dark"]           # optional; when absent a reload keeps the map's current flags
//!
//! # Ground or water: one character per hex.
//! terrain = '''
//! ...%~~..
//! ..^^~~__
//! '''
//! level = '''
//! 00110000
//! 02210000
//! '''
//! # Water depth under every ~ hex; . elsewhere.
//! depth = '''
//! ....23..
//! ....34..
//! '''
//! foliage = '''
//! ."".....
//! ...`....
//! '''
//! route = '''
//! ......##
//! ........
//! '''
//! condition = '''
//! ....--..
//! ........
//! '''
//!
//! [[structures]]
//! kind = "bridge"            # building, wall or bridge
//! class = "heavy"            # optional: light, medium (the default), heavy or hardened
//! height = 2                 # roof, wall top or deck height above the ground
//! hexes = [[4, 0], [5, 0]]
//!
//! [[points_of_interest]]
//! type = "objective"         # case-sensitive; any non-empty text
//! name = "Comms Tower"
//! x = 6
//! y = 1
//! elevation = 3              # optional, levels above (or below) the hex's ground level
//!
//! [[regions]]
//! type = "deployment"        # case-sensitive; any non-empty text
//! name = "North LZ"
//! corners = [[1, 0], [6, 0], [6, 1], [1, 1]]  # outline, in order
//! ```
//!
//! Grids are TOML literal strings (`'''`), since `"` is the heavy-woods symbol. `terrain` and
//! `level` are required; `depth` is required when the map has water. The optional `flow`,
//! `foliage`, `route`, `condition` and `overlay` grids each hold one layer, with `.` where a
//! hex has none. Heights use `0`-`9` then `a`-`z`. Width and height come from the grids, whose
//! rows must all be the same length. Every hex must pass [`Hex::validate`].
//!
//! A structure entry may give `cf` for a damaged structure with less than its class's full
//! construction factor.
//!
//! Points of interest are metadata for scripts, which read them through
//! `btech.map.points_of_interest`. Units never see them and they do not change the terrain.
//!
//! Regions are script metadata too. Each lists its corner hexes in outline order, and its
//! members are the outline plus every hex inside it; see [`MapRegion`].
use crate::{
    Condition, ConstructionClass, DecorationKind, Flow, Foliage, Ground, Hex, Light, MAX_HEIGHT,
    MAX_VISIBILITY, MapAsset, MapFlag, MapPointOfInterest, MapRegion, Route, Structure,
    StructureKind, Water, Wind,
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
    #[serde(default)]
    light: Option<Light>,
    #[serde(default)]
    visibility: Option<u8>,
    #[serde(default)]
    wind: Option<Wind>,
    #[serde(default)]
    flags: Option<Vec<MapFlag>>,
    terrain: String,
    level: String,
    #[serde(default)]
    depth: Option<String>,
    #[serde(default)]
    flow: Option<String>,
    #[serde(default)]
    foliage: Option<String>,
    #[serde(default)]
    route: Option<String>,
    #[serde(default)]
    condition: Option<String>,
    #[serde(default)]
    overlay: Option<String>,
    #[serde(default)]
    structures: Vec<StructureEntry>,
    #[serde(default)]
    points_of_interest: Vec<MapPointOfInterest>,
    #[serde(default)]
    regions: Vec<MapRegion>,
}

/// One building, wall or bridge: hexes sharing a kind, height and construction factor.
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct StructureEntry {
    kind: StructureKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    class: Option<ConstructionClass>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    cf: Option<u16>,
    /// Levels above the ground: roof, wall top or deck.
    height: u8,
    /// `[x, y]` hexes the structure covers.
    hexes: Vec<[u16; 2]>,
}

impl StructureEntry {
    /// The structure each of this entry's hexes holds.
    fn structure(&self) -> Result<Structure> {
        let class = self.class.unwrap_or(ConstructionClass::Medium);
        let full = class.construction_factor();
        let cf = self.cf.unwrap_or(full);
        ensure!(
            (1..=full).contains(&cf),
            "structure cf {cf} must be between 1 and {full} for a {} structure",
            class.label().to_ascii_lowercase()
        );
        ensure!(
            self.height <= MAX_HEIGHT,
            "structure height {} is too high",
            self.height
        );
        ensure!(
            !self.hexes.is_empty(),
            "a structure must cover at least one hex"
        );
        Ok(Structure {
            kind: self.kind,
            class,
            height: self.height,
            cf,
        })
    }
}

fn default_gravity() -> u8 {
    100
}

fn default_temperature() -> i8 {
    20
}

/// A terrain-grid character: the ground, or water.
fn ground(symbol: char) -> Option<Option<Ground>> {
    Some(Some(match symbol {
        '~' => return Some(None),
        '.' => Ground::Clear,
        '_' => Ground::Pavement,
        '%' => Ground::Rough,
        '^' => Ground::UltraRough,
        ';' => Ground::Rubble,
        '!' => Ground::UltraRubble,
        '}' => Ground::Sand,
        '{' => Ground::Tundra,
        'w' => Ground::Swamp,
        'm' => Ground::MagmaCrust,
        'M' => Ground::Magma,
        '$' => Ground::HeavyIndustrial,
        _ => return None,
    }))
}

/// The terrain-grid character for a hex.
fn ground_symbol(hex: Hex) -> char {
    if hex.water().is_some() {
        return '~';
    }
    match hex.ground() {
        Ground::Clear => '.',
        Ground::Pavement => '_',
        Ground::Rough => '%',
        Ground::UltraRough => '^',
        Ground::Rubble => ';',
        Ground::UltraRubble => '!',
        Ground::Sand => '}',
        Ground::Tundra => '{',
        Ground::Swamp => 'w',
        Ground::MagmaCrust => 'm',
        Ground::Magma => 'M',
        Ground::HeavyIndustrial => '$',
    }
}

/// Symbols of one optional layer grid, `.` meaning the layer is absent.
trait LayerSymbol: Sized + Copy + 'static {
    /// The layer's grid name.
    const GRID: &'static str;
    /// Every value with its grid character.
    const SYMBOLS: &'static [(Self, char)];
}

impl LayerSymbol for Flow {
    const GRID: &'static str = "flow";
    const SYMBOLS: &'static [(Self, char)] = &[(Flow::Rapids, 'r'), (Flow::Torrent, 't')];
}

impl LayerSymbol for Foliage {
    const GRID: &'static str = "foliage";
    const SYMBOLS: &'static [(Self, char)] = &[
        (Foliage::LightWoods, '`'),
        (Foliage::HeavyWoods, '"'),
        (Foliage::UltraHeavyWoods, 'W'),
        (Foliage::LightJungle, 'j'),
        (Foliage::HeavyJungle, 'J'),
        (Foliage::UltraHeavyJungle, 'U'),
        (Foliage::PlantedFields, 'f'),
    ];
}

impl LayerSymbol for Route {
    const GRID: &'static str = "route";
    const SYMBOLS: &'static [(Self, char)] = &[
        (Route::PavedRoad, '#'),
        (Route::GravelRoad, 'g'),
        (Route::DirtRoad, 'd'),
        (Route::Rail, '|'),
    ];
}

impl LayerSymbol for Condition {
    const GRID: &'static str = "condition";
    const SYMBOLS: &'static [(Self, char)] = &[
        (Condition::Ice, '-'),
        (Condition::ThinSnow, '*'),
        (Condition::DeepSnow, '+'),
        (Condition::Mud, ','),
    ];
}

impl LayerSymbol for DecorationKind {
    const GRID: &'static str = "overlay";
    const SYMBOLS: &'static [(Self, char)] =
        &[(DecorationKind::Fire, '&'), (DecorationKind::Smoke, ':')];
}

/// Decode a layer-grid character: `.` for none, otherwise one of the layer's symbols.
fn decode<T: LayerSymbol>(symbol: char) -> Option<Option<T>> {
    if symbol == '.' {
        return Some(None);
    }
    T::SYMBOLS
        .iter()
        .find(|(_, candidate)| *candidate == symbol)
        .map(|(value, _)| Some(*value))
}

/// Encode a layer value as its grid character.
fn encode<T: LayerSymbol + PartialEq>(value: Option<T>) -> char {
    let Some(value) = value else {
        return '.';
    };
    T::SYMBOLS
        .iter()
        .find(|(candidate, _)| *candidate == value)
        .map_or('.', |(_, symbol)| *symbol)
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

/// An optional layer grid, decoded hex by hex.
struct LayerGrid<T> {
    rows: Option<Vec<Vec<char>>>,
    layer: std::marker::PhantomData<T>,
}

impl<T: LayerSymbol> LayerGrid<T> {
    fn new(text: Option<&str>, width: usize, height: usize) -> Result<Self> {
        Ok(Self {
            rows: matching_grid(T::GRID, text, width, height)?,
            layer: std::marker::PhantomData,
        })
    }

    /// The layer at `x`, `y`; absent when the grid is.
    fn at(&self, x: usize, y: usize) -> Result<Option<T>> {
        let Some(rows) = &self.rows else {
            return Ok(None);
        };
        let symbol = rows[y][x];
        decode(symbol).with_context(|| format!("unknown {} symbol {symbol:?} at {x},{y}", T::GRID))
    }
}

impl MapAsset {
    /// Decode a map file.
    pub fn parse(source: &str) -> Result<Self> {
        Self::parse_with_flags(source, 0)
    }

    /// Decode a map file, keeping `inherited_flags` when the file has no `flags` key, so a
    /// reload does not clear flags an operator set on the live map.
    pub fn parse_with_flags(source: &str, inherited_flags: i64) -> Result<Self> {
        let file: MapFile = toml::from_str(source).context("invalid map file")?;
        let terrain = grid("terrain", &file.terrain)?;
        let (width, rows) = (terrain[0].len(), terrain.len());
        let level = matching_grid("level", Some(&file.level), width, rows)?.unwrap();
        let depth = matching_grid("depth", file.depth.as_deref(), width, rows)?;
        let flow = LayerGrid::<Flow>::new(file.flow.as_deref(), width, rows)?;
        let foliage = LayerGrid::<Foliage>::new(file.foliage.as_deref(), width, rows)?;
        let route = LayerGrid::<Route>::new(file.route.as_deref(), width, rows)?;
        let condition = LayerGrid::<Condition>::new(file.condition.as_deref(), width, rows)?;
        let overlay = LayerGrid::<DecorationKind>::new(file.overlay.as_deref(), width, rows)?;
        let mut hexes = Vec::with_capacity(width * rows);
        for y in 0..rows {
            for x in 0..width {
                let at = || format!("at {x},{y}");
                let symbol = terrain[y][x];
                let ground = ground(symbol)
                    .with_context(|| format!("unknown terrain symbol {symbol:?} {}", at()))?;
                let level = height(level[y][x])
                    .with_context(|| format!("invalid level {:?} {}", level[y][x], at()))?;
                let depth = depth.as_ref().map(|rows| rows[y][x]);
                let water = match (ground, depth) {
                    (Some(_), None | Some('.')) => None,
                    (Some(_), Some(_)) => {
                        anyhow::bail!("depth given for a hex without water {}", at())
                    }
                    (None, None | Some('.')) => {
                        anyhow::bail!("missing depth for water {}", at())
                    }
                    (None, Some(depth)) => Some(Water {
                        depth: depth
                            .to_digit(10)
                            .with_context(|| format!("invalid depth {depth:?} {}", at()))?
                            as u8,
                        flow: Flow::Still,
                    }),
                };
                let flow = flow.at(x, y)?;
                ensure!(
                    flow.is_none() || water.is_some(),
                    "flow given for a hex without water {}",
                    at()
                );
                let water = water.map(|water| Water {
                    flow: flow.unwrap_or_default(),
                    ..water
                });
                hexes.push(
                    Hex::at_level(level)
                        .with_ground(ground.unwrap_or(Ground::Clear))
                        .with_water(water)
                        .with_foliage(foliage.at(x, y)?)
                        .with_route(route.at(x, y)?)
                        .with_condition(condition.at(x, y)?)
                        .with_overlay(overlay.at(x, y)?),
                );
            }
        }
        for entry in &file.structures {
            let structure = entry.structure()?;
            for &[x, y] in &entry.hexes {
                let (x, y) = (usize::from(x), usize::from(y));
                ensure!(
                    x < width && y < rows,
                    "structure hex {x},{y} is off the map"
                );
                let hex = &mut hexes[y * width + x];
                ensure!(
                    hex.structure().is_none(),
                    "structure hex {x},{y} already has a structure"
                );
                *hex = hex.with_structure(Some(structure));
            }
        }
        for (index, hex) in hexes.iter().enumerate() {
            hex.validate()
                .with_context(|| format!("at {},{}", index % width, index / width))?;
        }
        for point in &file.points_of_interest {
            point.validate(width as i64, rows as i64)?;
        }
        for region in &file.regions {
            region.validate(width as i64, rows as i64)?;
        }
        if let Some(visibility) = file.visibility {
            ensure!(
                visibility <= MAX_VISIBILITY,
                "visibility must be at most {MAX_VISIBILITY}"
            );
        }
        if let Some(wind) = file.wind {
            wind.validate()?;
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
            light: file.light,
            visibility: file.visibility,
            wind: file.wind,
            hexes: Arc::new(hexes),
            points_of_interest: file.points_of_interest,
            regions: file.regions,
        })
    }

    /// Encode this map in the map file format. Layers no hex has are left out.
    pub fn to_file(&self) -> Result<String> {
        let width = usize::from(self.width);
        let rows = |encode: &dyn Fn(Hex) -> char| -> String {
            let mut text = String::with_capacity((width + 1) * usize::from(self.height));
            for row in self.hexes.chunks(width) {
                text.extend(row.iter().map(|&hex| encode(hex)));
                text.push('\n');
            }
            text
        };
        for (index, hex) in self.hexes.iter().enumerate() {
            hex.validate()
                .with_context(|| format!("at {},{}", index % width, index / width))?;
        }
        let any = |has: &dyn Fn(Hex) -> bool| self.hexes.iter().any(|&hex| has(hex));
        let mut structures: BTreeMap<Structure, Vec<[u16; 2]>> = BTreeMap::new();
        for (index, hex) in self.hexes.iter().enumerate() {
            if let Some(structure) = hex.structure() {
                structures
                    .entry(structure)
                    .or_default()
                    .push([(index % width) as u16, (index / width) as u16]);
            }
        }
        let flags = MapFlag::ALL
            .into_iter()
            .filter(|flag| flag.is_set(i64::from(self.flags)))
            .collect::<Vec<_>>();
        let mut text = String::new();
        writeln!(text, "gravity = {}", self.gravity)?;
        writeln!(text, "temperature = {}", self.temperature)?;
        if let Some(light) = self.light {
            writeln!(text, "light = \"{}\"", light.name())?;
        }
        if let Some(visibility) = self.visibility {
            writeln!(text, "visibility = {visibility}")?;
        }
        if let Some(wind) = self.wind {
            writeln!(
                text,
                "wind = {{ direction = {}, speed = {} }}",
                wind.direction, wind.speed
            )?;
        }
        writeln!(
            text,
            "flags = {}",
            toml::to_string(&Flags { flags })?
                .trim_start_matches("flags = ")
                .trim_end()
        )?;
        writeln!(text)?;
        writeln!(text, "terrain = '''\n{}'''", rows(&ground_symbol))?;
        writeln!(
            text,
            "level = '''\n{}'''",
            rows(&|hex| height_symbol(hex.level()))
        )?;
        if any(&|hex| hex.water().is_some()) {
            writeln!(
                text,
                "depth = '''\n{}'''",
                rows(&|hex| hex.water().map_or('.', |water| height_symbol(water.depth)))
            )?;
        }
        if any(&|hex| hex.water().is_some_and(|water| !water.flow.is_still())) {
            writeln!(
                text,
                "flow = '''\n{}'''",
                rows(&|hex| encode(hex.water().map(|water| water.flow)))
            )?;
        }
        if any(&|hex| hex.foliage().is_some()) {
            writeln!(
                text,
                "foliage = '''\n{}'''",
                rows(&|hex| encode(hex.foliage()))
            )?;
        }
        if any(&|hex| hex.route().is_some()) {
            writeln!(text, "route = '''\n{}'''", rows(&|hex| encode(hex.route())))?;
        }
        if any(&|hex| hex.condition().is_some()) {
            writeln!(
                text,
                "condition = '''\n{}'''",
                rows(&|hex| encode(hex.condition()))
            )?;
        }
        if any(&|hex| hex.overlay().is_some()) {
            writeln!(
                text,
                "overlay = '''\n{}'''",
                rows(&|hex| encode(hex.overlay()))
            )?;
        }
        for (structure, hexes) in structures {
            let class = structure.class;
            writeln!(
                text,
                "\n[[structures]]\nkind = \"{}\"\nclass = \"{}\"",
                kind_name(structure.kind),
                class.label().to_ascii_lowercase()
            )?;
            if structure.cf != class.construction_factor() {
                writeln!(text, "cf = {}", structure.cf)?;
            }
            writeln!(text, "height = {}", structure.height)?;
            let hexes = hexes
                .iter()
                .map(|[x, y]| format!("[{x}, {y}]"))
                .collect::<Vec<_>>()
                .join(", ");
            writeln!(text, "hexes = [{hexes}]")?;
        }
        for point in &self.points_of_interest {
            point.validate(i64::from(self.width), i64::from(self.height))?;
        }
        if !self.points_of_interest.is_empty() {
            writeln!(
                text,
                "\n{}",
                toml::to_string(&PointsOfInterest {
                    points_of_interest: &self.points_of_interest,
                })?
                .trim_end()
            )?;
        }
        for region in &self.regions {
            region.validate(i64::from(self.width), i64::from(self.height))?;
        }
        if !self.regions.is_empty() {
            writeln!(
                text,
                "\n{}",
                toml::to_string(&Regions {
                    regions: &self.regions,
                })?
                .trim_end()
            )?;
        }
        Ok(text)
    }
}

/// The file spelling of a structure kind.
fn kind_name(kind: StructureKind) -> &'static str {
    match kind {
        StructureKind::Building => "building",
        StructureKind::Wall => "wall",
        StructureKind::Bridge => "bridge",
    }
}

/// Helper so the flag list is written with TOML's own string quoting.
#[derive(Serialize)]
struct Flags {
    flags: Vec<MapFlag>,
}

/// Helper so points of interest are written as `[[points_of_interest]]` tables with TOML's
/// own string quoting.
#[derive(Serialize)]
struct PointsOfInterest<'a> {
    points_of_interest: &'a [MapPointOfInterest],
}

/// Helper so regions are written as `[[regions]]` tables with TOML's own string quoting.
#[derive(Serialize)]
struct Regions<'a> {
    regions: &'a [MapRegion],
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Terrain;

    const SAMPLE: &str = r#"
gravity = 80
temperature = -10
light = "moonless_night"
visibility = 12
wind = { direction = 270, speed = 15 }
flags = ["dark", "vacuum"]

terrain = '''
....~~
._%^w~
'''
level = '''
012000
1a0000
'''
depth = '''
....23
.....1
'''
flow = '''
.....r
......
'''
foliage = '''
.`"...
...U..
'''
route = '''
......
#.....
'''
condition = '''
...+-.
,.....
'''

[[structures]]
kind = "bridge"
height = 2
hexes = [[4, 0]]

[[structures]]
kind = "building"
class = "hardened"
cf = 120
height = 4
hexes = [[2, 1]]
"#;

    #[test]
    fn parses_every_layer() {
        let map = MapAsset::parse(SAMPLE).unwrap();
        assert_eq!((map.width, map.height), (6, 2));
        assert_eq!((map.gravity, map.temperature), (80, -10));
        assert_eq!(map.light, Some(Light::MoonlessNight));
        assert_eq!(map.visibility, Some(12));
        assert_eq!(
            map.wind,
            Some(Wind {
                direction: 270,
                speed: 15
            })
        );
        assert_eq!(i64::from(map.flags), 4 | 32);
        let hex = |x, y| map.hex(x, y).unwrap();
        assert_eq!(hex(0, 0), Hex::new(Terrain::Clear, 0));
        assert_eq!(hex(1, 0), Hex::new(Terrain::LightWoods, 1));
        assert_eq!(hex(2, 0), Hex::new(Terrain::HeavyWoods, 2));
        assert_eq!(hex(3, 0), Hex::new(Terrain::DeepSnow, 0));
        assert_eq!(hex(4, 0).deck_clearance(), Some(2));
        assert!(hex(4, 0).is_frozen());
        assert_eq!(hex(4, 0).water_depth(), 2);
        assert_eq!(
            hex(5, 0).water(),
            Some(Water {
                depth: 3,
                flow: Flow::Rapids
            })
        );
        assert_eq!(
            hex(0, 1),
            Hex::new(Terrain::Road, 1).with_condition(Some(Condition::Mud))
        );
        assert_eq!(hex(1, 1).level(), 10);
        assert_eq!(hex(1, 1).ground(), Ground::Pavement);
        let tower = hex(2, 1).structure().unwrap();
        assert_eq!(
            (tower.kind, tower.height, tower.cf),
            (StructureKind::Building, 4, 120)
        );
        assert_eq!(tower.class, ConstructionClass::Hardened);
        assert_eq!(
            hex(3, 1),
            Hex::new(Terrain::UltraRough, 0).with_foliage(Some(Foliage::UltraHeavyJungle))
        );
        assert_eq!(hex(4, 1).ground(), Ground::Swamp);
        assert_eq!(hex(5, 1), Hex::new(Terrain::Water, 1));
    }

    #[test]
    fn writes_what_it_reads() {
        let map = MapAsset::parse(SAMPLE).unwrap();
        let text = map.to_file().unwrap();
        assert_eq!(MapAsset::parse(&text).unwrap(), map);
        assert_eq!(map.to_file().unwrap(), text);
        assert!(text.contains("class = \"hardened\"\ncf = 120\n"), "{text}");
        assert!(text.contains("kind = \"bridge\"\nclass = \"medium\"\nheight"));
        // Three heavy-woods hexes in a row would end a basic multi-line string.
        let woods = MapAsset::from_cells("3 1\n\"0\"0\"0\n").unwrap();
        assert_eq!(MapAsset::parse(&woods.to_file().unwrap()).unwrap(), woods);
        let plain = MapAsset::parse("terrain = \"..\\n\"\nlevel = \"01\\n\"").unwrap();
        let text = plain.to_file().unwrap();
        for absent in [
            "depth",
            "structures",
            "foliage",
            "route",
            "condition",
            "flow",
            "light",
            "wind",
            "visibility",
        ] {
            assert!(!text.contains(absent), "{absent} in {text}");
        }
        assert_eq!(MapAsset::parse(&text).unwrap(), plain);
    }

    /// Every terrain of the compact notation, on every digit, saves and loads unchanged.
    #[test]
    fn every_terrain_survives_a_save() {
        for terrain in Terrain::ALL {
            for digit in [0, 1, 9] {
                let hex = Hex::new(terrain, digit);
                let map = MapAsset {
                    width: 1,
                    height: 1,
                    flags: 0,
                    gravity: 100,
                    temperature: 20,
                    light: None,
                    visibility: None,
                    wind: None,
                    hexes: Arc::new(vec![hex]),
                    points_of_interest: Vec::new(),
                    regions: Vec::new(),
                };
                let text = map.to_file().unwrap();
                assert_eq!(MapAsset::parse(&text).unwrap(), map, "{text}");
            }
        }
    }

    /// A building on high ground keeps both heights; its top is their sum.
    #[test]
    fn structures_on_raised_ground_load_and_validate() {
        let source = "terrain = '.'\nlevel = 'a'\n[[structures]]\nkind = 'building'\nheight = 11\nhexes = [[0, 0]]\n";
        let map = MapAsset::parse(source).unwrap();
        let tower = map.hex(0, 0).unwrap();
        assert_eq!((tower.level(), tower.surface_height()), (10, 21));
        assert_eq!(tower.construction_class(), Some(ConstructionClass::Medium));
        assert_eq!(MapAsset::parse(&map.to_file().unwrap()).unwrap(), map);
    }

    /// A lake and a bridge on a plateau keep their surfaces at the plateau's level.
    #[test]
    fn water_and_bridges_sit_on_raised_ground() {
        let source = "terrain = '~~~'\nlevel = '432'\ndepth = '231'\ncondition = '.-.'\n\n[[structures]]\nkind = 'bridge'\nheight = 2\nhexes = [[2, 0]]\n";
        let map = MapAsset::parse(source).unwrap();
        let lake = map.hex(0, 0).unwrap();
        assert_eq!((lake.water_line(), lake.surface_height()), (4, 2));
        let ice = map.hex(1, 0).unwrap();
        assert_eq!((ice.standing_height(), ice.surface_height()), (3, 0));
        let bridge = map.hex(2, 0).unwrap();
        assert_eq!((bridge.water_line(), bridge.deck_height()), (2, Some(4)));
        assert_eq!(MapAsset::parse(&map.to_file().unwrap()).unwrap(), map);
    }

    /// The overlay grid places permanent fire and smoke over any hex, on top of its layers.
    #[test]
    fn overlay_grid_loads_permanent_fire_and_smoke() {
        let source =
            "terrain = '..~'\nfoliage = '.`.'\nlevel = '120'\ndepth = '..2'\noverlay = '&:.'\n";
        let map = MapAsset::parse(source).unwrap();
        let fire = map.hex(0, 0).unwrap();
        assert_eq!(fire, Hex::new(Terrain::Fire, 1));
        let smoky = map.hex(1, 0).unwrap();
        assert_eq!(smoky.overlay(), Some(DecorationKind::Smoke));
        assert_eq!(smoky.with_overlay(None), Hex::new(Terrain::LightWoods, 2));
        assert_eq!(map.hex(2, 0).unwrap().overlay(), None);
        let text = map.to_file().unwrap();
        assert!(text.contains("overlay = '''\n&:.\n'''"), "{text}");
        assert_eq!(MapAsset::parse(&text).unwrap(), map);
        for bad in [
            "terrain = '&'\nlevel = '0'\n",
            "terrain = '.'\nlevel = '0'\noverlay = '~'\n",
            "terrain = '..'\nlevel = '00'\noverlay = '&'\n",
        ] {
            assert!(MapAsset::parse(bad).is_err(), "{bad}");
        }
        let plain = MapAsset::parse("terrain = '.'\nlevel = '0'\n").unwrap();
        assert!(!plain.to_file().unwrap().contains("overlay"));
    }

    /// Points of interest keep their file order, exact type spelling and optional elevation,
    /// and survive a save and reload.
    #[test]
    fn points_of_interest_round_trip() {
        let source = format!(
            "{SAMPLE}\n[[points_of_interest]]\ntype = \"Objective\"\nname = \"Comms \\\"Tower\\\"\"\nx = 3\ny = 1\nelevation = -2\n\n[[points_of_interest]]\ntype = \"objective\"\nname = \"Ford\"\nx = 4\ny = 0\n"
        );
        let map = MapAsset::parse(&source).unwrap();
        assert_eq!(
            map.points_of_interest,
            [
                MapPointOfInterest {
                    kind: "Objective".into(),
                    name: "Comms \"Tower\"".into(),
                    x: 3,
                    y: 1,
                    elevation: Some(-2),
                },
                MapPointOfInterest {
                    kind: "objective".into(),
                    name: "Ford".into(),
                    x: 4,
                    y: 0,
                    elevation: None,
                },
            ]
        );
        let text = map.to_file().unwrap();
        assert_eq!(MapAsset::parse(&text).unwrap(), map);
        assert_eq!(map.to_file().unwrap(), text);
        assert!(
            !MapAsset::parse(SAMPLE)
                .unwrap()
                .to_file()
                .unwrap()
                .contains("points_of_interest")
        );
    }

    /// Regions keep their file order, type spelling and corner order, and survive a save and
    /// reload; a map without regions writes none.
    #[test]
    fn regions_round_trip() {
        let source = format!(
            "{SAMPLE}\n[[regions]]\ntype = \"deployment\"\nname = \"North \\\"LZ\\\"\"\ncorners = [[4, 0], [0, 1], [3, 1]]\n\n[[regions]]\ntype = \"Deployment\"\nname = \"Ford\"\ncorners = [[4, 0]]\n"
        );
        let map = MapAsset::parse(&source).unwrap();
        assert_eq!(
            map.regions,
            [
                MapRegion {
                    kind: "deployment".into(),
                    name: "North \"LZ\"".into(),
                    corners: vec![[4, 0], [0, 1], [3, 1]],
                },
                MapRegion {
                    kind: "Deployment".into(),
                    name: "Ford".into(),
                    corners: vec![[4, 0]],
                },
            ]
        );
        let text = map.to_file().unwrap();
        assert!(
            text.contains("corners = [[4, 0], [0, 1], [3, 1]]"),
            "{text}"
        );
        assert_eq!(MapAsset::parse(&text).unwrap(), map);
        assert!(
            !MapAsset::parse(SAMPLE)
                .unwrap()
                .to_file()
                .unwrap()
                .contains("regions")
        );
    }

    #[test]
    fn rejects_malformed_regions() {
        let base = "terrain = '..'\nlevel = '00'\n[[regions]]\n";
        for (extra, message) in [
            ("type = 'a'\nname = 'b'\ncorners = [[2, 0]]", "off the map"),
            (
                "type = 'a'\nname = 'b'\ncorners = [[0, 0], [0, 1]]",
                "off the map",
            ),
            (
                "type = 'a'\nname = 'b'\ncorners = []",
                "at least one corner",
            ),
            (
                "type = ''\nname = 'b'\ncorners = [[0, 0]]",
                "type must be non-empty",
            ),
            (
                "type = 'a'\nname = ''\ncorners = [[0, 0]]",
                "name must be non-empty",
            ),
            ("type = 'a'\nname = 'b'", "invalid map file"),
            (
                "type = 'a'\nname = 'b'\ncorners = [[0]]",
                "invalid map file",
            ),
            (
                "type = 'a'\nname = 'b'\ncorners = [[0, -1]]",
                "invalid map file",
            ),
            (
                "type = 'a'\nname = 'b'\ncorners = [[0, 0]]\nhexes = 1",
                "invalid map file",
            ),
        ] {
            let source = format!("{base}{extra}\n");
            let error = format!("{:#}", MapAsset::parse(&source).unwrap_err());
            assert!(error.contains(message), "{message:?} not in {error:?}");
        }
    }

    #[test]
    fn rejects_malformed_points_of_interest() {
        let base = "terrain = '..'\nlevel = '00'\n[[points_of_interest]]\n";
        for (extra, message) in [
            ("type = 'a'\nname = 'b'\nx = 2\ny = 0", "off the map"),
            ("type = 'a'\nname = 'b'\nx = 0\ny = 1", "off the map"),
            (
                "type = ''\nname = 'b'\nx = 0\ny = 0",
                "type must be non-empty",
            ),
            (
                "type = 'a'\nname = ''\nx = 0\ny = 0",
                "name must be non-empty",
            ),
            (
                "type = \"a\\u0000\"\nname = 'b'\nx = 0\ny = 0",
                "type must be non-empty",
            ),
            ("type = 'a'\nx = 0\ny = 0", "invalid map file"),
            ("name = 'b'\nx = 0\ny = 0", "invalid map file"),
            (
                "type = 'a'\nname = 'b'\nx = 0\ny = 0\nelevation = 200",
                "invalid map file",
            ),
            (
                "type = 'a'\nname = 'b'\nx = 0\ny = 0\ncolour = 1",
                "invalid map file",
            ),
        ] {
            let source = format!("{base}{extra}\n");
            let error = format!("{:#}", MapAsset::parse(&source).unwrap_err());
            assert!(error.contains(message), "{message:?} not in {error:?}");
        }
    }

    #[test]
    fn missing_flags_keep_inherited_ones() {
        let source = "terrain = \".\\n\"\nlevel = \"0\\n\"";
        assert_eq!(MapAsset::parse_with_flags(source, 32).unwrap().flags, 32);
        let explicit = format!("flags = []\n{source}");
        assert_eq!(MapAsset::parse_with_flags(&explicit, 32).unwrap().flags, 0);
    }

    #[test]
    fn rejects_malformed_maps() {
        let base = |terrain: &str, level: &str, extra: &str| {
            format!("terrain = \"\"\"\n{terrain}\"\"\"\nlevel = \"\"\"\n{level}\"\"\"\n{extra}")
        };
        let structure = |kind: &str, extra: &str| {
            format!("[[structures]]\nkind = '{kind}'\nheight = 1\n{extra}")
        };
        for (source, message) in [
            (base("..\n.\n", "00\n0\n", ""), "row 1 has 1"),
            (base("..\n", "000\n", ""), "level grid is 3x1"),
            (base("X\n", "0\n", ""), "unknown terrain symbol 'X'"),
            (base(".\n", "!\n", ""), "invalid level"),
            (base("~\n", "0\n", ""), "missing depth"),
            (base(".\n", "0\n", "depth = \"2\\n\""), "without water"),
            (base(".\n", "0\n", "flow = \"r\\n\""), "without water"),
            (
                base(".\n", "0\n", "foliage = \"~\\n\""),
                "unknown foliage symbol",
            ),
            (
                base("~\n", "0\n", "depth = \"1\\n\"\nfoliage = \"`\\n\""),
                "under water",
            ),
            (
                base(".\n", "0\n", &structure("bridge", "hexes = [[0, 0]]")),
                "must span water",
            ),
            (
                base(
                    "~\n",
                    "0\n",
                    &format!(
                        "depth = \"1\\n\"\n{}",
                        structure("bridge", "hexes = [[1, 0]]")
                    ),
                ),
                "off the map",
            ),
            (
                base(
                    ".\n",
                    "0\n",
                    &structure("wall", "class = 'light'\ncf = 30\nhexes = [[0, 0]]"),
                ),
                "between 1 and 15 for a light",
            ),
            (
                base(".\n", "0\n", &structure("wall", "cf = 0\nhexes = [[0, 0]]")),
                "between 1 and",
            ),
            (
                base(".\n", "0\n", &structure("wall", "hexes = []")),
                "at least one hex",
            ),
            (
                base(".\n", "0\n", "flags = [\"bogus\"]"),
                "invalid map file",
            ),
            (base(".\n", "0\n", "visibility = 61"), "visibility"),
            (
                base(".\n", "0\n", "wind = { direction = 400, speed = 1 }"),
                "Wind direction",
            ),
            (base(".\n", "0\n", "light = 'twilight'"), "invalid map file"),
            (base(".\n", "0\n", "colour = 1"), "invalid map file"),
        ] {
            let error = format!("{:#}", MapAsset::parse(&source).unwrap_err());
            assert!(error.contains(message), "{message:?} not in {error:?}");
        }
    }

    /// The example in the map file documentation loads and survives a save, so the docs
    /// cannot drift from the format.
    #[test]
    fn documented_example_loads() {
        let docs = include_str!("../../../docs/content/docs/concepts/map-files.md");
        let example = docs
            .split_once("## Example")
            .and_then(|(_, rest)| rest.split_once("```toml\n"))
            .and_then(|(_, rest)| rest.split_once("```"))
            .map(|(block, _)| block)
            .expect("map-files.md has a toml block under ## Example");
        let map = MapAsset::parse(example).unwrap();
        assert_eq!((map.width, map.height), (8, 2));
        assert!(map.hex(4, 0).unwrap().structure().is_some());
        assert!(map.hex(7, 1).unwrap().structure().is_some());
        assert_eq!(map.hex(2, 0).unwrap().overlay(), Some(DecorationKind::Fire));
        assert_eq!(
            map.hex(3, 1).unwrap().overlay(),
            Some(DecorationKind::Smoke)
        );
        assert_eq!(map.points_of_interest.len(), 1);
        assert_eq!(map.regions.len(), 1);
        assert_eq!(MapAsset::parse(&map.to_file().unwrap()).unwrap(), map);
    }
}
