//! The map specification: every knob a person, a tool or an LLM can turn to describe the
//! battlefield they want.
//!
//! Every field is optional. Anything left out falls back to the chosen biome's defaults, and
//! [`MapSpec::resolve`] fills those defaults in so the resolved spec reproduces the map exactly.
//! The JSON Schema from [`spec_schema`] is generated from these types and their doc comments, so
//! keep the comments written for a reader who only sees the schema.
use anyhow::{Context, Result, bail, ensure};
use schemars::{JsonSchema, Schema, SchemaGenerator, json_schema};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::borrow::Cow;
use std::str::FromStr;
use stompymux_map::MapFlag;

/// The smallest width or height a generated map may have.
pub const MIN_DIMENSION: u16 = 8;

/// The largest width or height a map file may have.
pub const MAX_DIMENSION: u16 = 1000;

/// A description of the battlefield to generate. Every field is optional; omitted fields take
/// the biome's defaults.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MapSpec {
    /// Random seed. The same spec and seed always produce the same map. Omit for a random map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
    /// Overall landscape, which sets the default terrain mix, climate and environment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub biome: Option<Biome>,
    /// Map size preset. `width` and `height` override it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub size: Option<MapSize>,
    /// Map width in hexes (8 to 1000). Overrides `size`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u16>,
    /// Map height in hexes (8 to 1000). Overrides `size`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u16>,
    /// How much the ground rises and falls.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub relief: Option<Relief>,
    /// How much of the map is lakes, seas or swamp water.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub water: Option<Amount>,
    /// How much of the land is woods, or jungle in the jungle biome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub woods: Option<Amount>,
    /// How much of the land is rough ground (rubble, scree, broken rock).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rough: Option<Amount>,
    /// How much permanent fire and smoke burns on the map.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fire: Option<Amount>,
    /// Number of rivers crossing the map (0 to 8).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rivers: Option<u8>,
    /// Whether lakes and rivers are frozen over with ice.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frozen: Option<bool>,
    /// Cities, towns, bases and other clusters of buildings, placed in order.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub settlements: Vec<SettlementSpec>,
    /// The road network.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub roads: Option<RoadSpec>,
    /// Gravity, temperature and map flags.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub environment: Option<EnvironmentSpec>,
}

/// The overall landscape of a map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "snake_case")]
pub enum Biome {
    /// Rolling grassland, scattered woods, lakes and rivers.
    Temperate,
    /// Dense woods broken by clearings, with stands of ultra-heavy old growth.
    Forest,
    /// Hot, wet and choked with jungle and rivers.
    Jungle,
    /// Hot sand and hardpan with rocky outcrops and rare oases.
    Desert,
    /// Snowfields over frozen tundra, frozen lakes and bitter cold.
    Arctic,
    /// High ridges, ultra-rough peaks and deep snow above the snow line.
    Mountains,
    /// Terraced mesas, canyons and broken rock.
    Badlands,
    /// Flat, waterlogged swamp with shallow pools and heavy woods.
    Swamp,
    /// Land running down to a sea along one edge of the map, with beaches.
    Coastal,
    /// Airless, low-gravity cratered rock.
    Lunar,
    /// A volcano with rough slopes, fire and smoke.
    Volcanic,
}

impl Biome {
    /// Every biome, in catalog order.
    pub const ALL: [Self; 11] = [
        Self::Temperate,
        Self::Forest,
        Self::Jungle,
        Self::Desert,
        Self::Arctic,
        Self::Mountains,
        Self::Badlands,
        Self::Swamp,
        Self::Coastal,
        Self::Lunar,
        Self::Volcanic,
    ];
}

/// Map size presets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "snake_case")]
pub enum MapSize {
    /// 30 by 30 hexes: a skirmish between a few units.
    Small,
    /// 50 by 50 hexes: a lance-on-lance fight.
    Medium,
    /// 80 by 80 hexes: a company-sized battle.
    Large,
    /// 120 by 120 hexes: a campaign theater.
    Huge,
}

impl MapSize {
    /// Width and height in hexes.
    pub const fn dimensions(self) -> (u16, u16) {
        match self {
            Self::Small => (30, 30),
            Self::Medium => (50, 50),
            Self::Large => (80, 80),
            Self::Huge => (120, 120),
        }
    }
}

/// How much the ground rises and falls.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "snake_case")]
pub enum Relief {
    /// Almost level: ground from level 0 to 2.
    Flat,
    /// Gentle hills: ground from level 0 to 4.
    Rolling,
    /// Steep hills: ground from level 0 to 8.
    Hilly,
    /// Ridges and peaks: ground from level 0 to 16.
    Mountainous,
}

impl Relief {
    /// The highest ground level this relief produces.
    pub const fn max_level(self) -> u8 {
        match self {
            Self::Flat => 2,
            Self::Rolling => 4,
            Self::Hilly => 8,
            Self::Mountainous => 16,
        }
    }
}

/// A qualitative quantity for a terrain feature.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "snake_case")]
pub enum Amount {
    /// None at all.
    None,
    /// A little.
    Low,
    /// A fair amount.
    Medium,
    /// A lot.
    High,
    /// Most of the map.
    Extreme,
}

impl Amount {
    /// Fraction of the map covered by water.
    pub(crate) const fn water_fraction(self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Low => 0.05,
            Self::Medium => 0.15,
            Self::High => 0.3,
            Self::Extreme => 0.5,
        }
    }

    /// Fraction of the land covered by woods or jungle.
    pub(crate) const fn woods_fraction(self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Low => 0.1,
            Self::Medium => 0.25,
            Self::High => 0.45,
            Self::Extreme => 0.7,
        }
    }

    /// Fraction of the land that is rough ground.
    pub(crate) const fn rough_fraction(self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Low => 0.05,
            Self::Medium => 0.12,
            Self::High => 0.25,
            Self::Extreme => 0.4,
        }
    }

    /// Fraction of the map burning or smoking.
    pub(crate) const fn fire_fraction(self) -> f64 {
        match self {
            Self::None => 0.0,
            Self::Low => 0.01,
            Self::Medium => 0.03,
            Self::High => 0.06,
            Self::Extreme => 0.12,
        }
    }
}

/// A settlement, base or other cluster of buildings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SettlementSpec {
    /// Label used in the generation report. Defaults to the size and a number, like `town 2`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// How large the settlement is.
    pub size: SettlementSize,
    /// What the settlement is for, which shapes its buildings.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kind: Option<SettlementKind>,
    /// How its streets and buildings are arranged. Defaults by size and kind.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub layout: Option<SettlementLayout>,
    /// Where on the map to put it. Defaults to anywhere suitable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub position: Option<Position>,
    /// Exact `[x, y]` hex for its center, counting from 0 at the top left. Overrides `position`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<[u16; 2]>,
    /// Whether a wall with gates surrounds it. Military bases are walled by default.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub walled: Option<bool>,
    /// How many identical settlements to place (1 to 32).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub count: Option<u8>,
}

impl SettlementSpec {
    /// A settlement of `size` with every other choice left to the generator.
    pub fn new(size: SettlementSize) -> Self {
        Self {
            name: None,
            size,
            kind: None,
            layout: None,
            position: None,
            at: None,
            walled: None,
            count: None,
        }
    }
}

/// How large a settlement is.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "snake_case")]
pub enum SettlementSize {
    /// Two to four low buildings, about 5 hexes across.
    Outpost,
    /// A handful of low buildings, about 7 hexes across.
    Hamlet,
    /// A small cluster of streets, about 11 hexes across.
    Village,
    /// A street grid with mid-rise buildings, about 17 hexes across.
    Town,
    /// Dense blocks with towers downtown, about 25 hexes across.
    City,
    /// A sprawling city with skyscrapers, about 37 hexes across.
    Metropolis,
}

/// What a settlement is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "snake_case")]
pub enum SettlementKind {
    /// Homes and shops; heights fall off from a downtown core.
    Civilian,
    /// Large, low, closely packed factory and warehouse blocks.
    Industrial,
    /// A walled compound with barracks, hangars and an open landing pad.
    Military,
    /// A wrecked settlement: rubble, broken walls, smoke and stubs of buildings.
    Ruins,
}

/// How a settlement's streets and buildings are arranged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "snake_case")]
pub enum SettlementLayout {
    /// A rectangular street grid with building blocks between.
    Grid,
    /// Winding streets branching from a central square.
    Organic,
    /// Buildings spread apart without streets, like farmsteads.
    Scattered,
    /// A rectangular compound with a perimeter road and rows of buildings.
    Compound,
}

/// A region of the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "cli", derive(clap::ValueEnum))]
#[serde(rename_all = "snake_case")]
pub enum Position {
    /// Anywhere suitable.
    Random,
    /// The middle of the map.
    Center,
    /// The top of the map.
    North,
    /// The top-right corner.
    Northeast,
    /// The right side.
    East,
    /// The bottom-right corner.
    Southeast,
    /// The bottom of the map.
    South,
    /// The bottom-left corner.
    Southwest,
    /// The left side.
    West,
    /// The top-left corner.
    Northwest,
}

impl Position {
    /// Where this region's center lies as fractions of the map's width and height, or `None`
    /// for anywhere.
    pub(crate) const fn anchor(self) -> Option<(f64, f64)> {
        Some(match self {
            Self::Random => return None,
            Self::Center => (0.5, 0.5),
            Self::North => (0.5, 0.2),
            Self::Northeast => (0.8, 0.2),
            Self::East => (0.8, 0.5),
            Self::Southeast => (0.8, 0.8),
            Self::South => (0.5, 0.8),
            Self::Southwest => (0.2, 0.8),
            Self::West => (0.2, 0.5),
            Self::Northwest => (0.2, 0.2),
        })
    }
}

/// The road network.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RoadSpec {
    /// Link every settlement into one network with the shortest set of roads. Default true.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub connect_settlements: Option<bool>,
    /// Highways crossing the whole map from edge to edge, passing through settlements on the
    /// way (0 to 8). Defaults to 1 when there are settlements, otherwise 0.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub through_roads: Option<u8>,
    /// Road width in hexes (1 to 3). Default 1.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u8>,
}

/// Gravity, temperature and flags for the battlefield.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentSpec {
    /// Gravity in percent of standard (0 to 255). Defaults by biome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub gravity: Option<u8>,
    /// Temperature in degrees Celsius (-128 to 127). Defaults by biome.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temperature: Option<i8>,
    /// Map flags. Defaults by biome; give an empty list for none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<Vec<MapFlagSchema>>")]
    pub flags: Option<Vec<MapFlag>>,
}

/// The JSON Schema of a [`MapFlag`], built from the flags' own names and
/// descriptions. `stompymux-map` carries no schema support of its own, so spec fields that
/// hold flags borrow this one with `#[schemars(with = ...)]`.
struct MapFlagSchema;

impl JsonSchema for MapFlagSchema {
    fn schema_name() -> Cow<'static, str> {
        "MapFlag".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        let flags: Vec<_> = MapFlag::ALL
            .into_iter()
            .map(|flag| {
                json!({
                    "const": flag.name(),
                    "description": flag.description(),
                    "type": "string",
                })
            })
            .collect();
        json_schema!({
            "description": "A battlefield rule flag, as named in map files.",
            "oneOf": flags,
        })
    }
}

impl MapSpec {
    /// Read a spec from JSON or TOML text, whichever it is.
    pub fn parse(text: &str) -> Result<Self> {
        let trimmed = text.trim_start();
        if trimmed.starts_with('{') {
            return serde_json::from_str(text).context("invalid JSON map spec");
        }
        toml::from_str(text).context("invalid TOML map spec")
    }

    /// Check every value is in range, so mistakes come back as clear messages.
    pub fn validate(&self) -> Result<()> {
        for (name, value) in [("width", self.width), ("height", self.height)] {
            if let Some(value) = value {
                ensure!(
                    (MIN_DIMENSION..=MAX_DIMENSION).contains(&value),
                    "{name} must be between {MIN_DIMENSION} and {MAX_DIMENSION}, not {value}"
                );
            }
        }
        if let Some(rivers) = self.rivers {
            ensure!(rivers <= 8, "rivers must be between 0 and 8, not {rivers}");
        }
        if let Some(roads) = &self.roads {
            if let Some(width) = roads.width {
                ensure!(
                    (1..=3).contains(&width),
                    "roads.width must be between 1 and 3, not {width}"
                );
            }
            if let Some(through) = roads.through_roads {
                ensure!(
                    through <= 8,
                    "roads.through_roads must be between 0 and 8, not {through}"
                );
            }
        }
        let (width, height) = self.dimensions();
        for (index, settlement) in self.settlements.iter().enumerate() {
            if let Some(count) = settlement.count {
                ensure!(
                    (1..=32).contains(&count),
                    "settlements[{index}].count must be between 1 and 32, not {count}"
                );
            }
            if let Some([x, y]) = settlement.at {
                ensure!(
                    x < width && y < height,
                    "settlements[{index}].at [{x}, {y}] is off the {width}x{height} map"
                );
            }
        }
        Ok(())
    }

    /// The map's width and height in hexes.
    pub fn dimensions(&self) -> (u16, u16) {
        let (width, height) = self.size.unwrap_or(MapSize::Medium).dimensions();
        (self.width.unwrap_or(width), self.height.unwrap_or(height))
    }

    /// This spec with a seed and every biome default filled in. Generating from the resolved
    /// spec produces the same map, even if biome defaults later change.
    pub fn resolve(&self) -> Result<Self> {
        self.validate()?;
        let biome = self.biome.unwrap_or(Biome::Temperate);
        let profile = crate::biome::profile(biome);
        let (width, height) = self.dimensions();
        let environment = self.environment.clone().unwrap_or_default();
        let roads = self.roads.clone().unwrap_or_default();
        Ok(Self {
            seed: Some(self.seed.unwrap_or_else(random_seed)),
            biome: Some(biome),
            size: None,
            width: Some(width),
            height: Some(height),
            relief: Some(self.relief.unwrap_or(profile.relief)),
            water: Some(self.water.unwrap_or(profile.water)),
            woods: Some(self.woods.unwrap_or(profile.woods)),
            rough: Some(self.rough.unwrap_or(profile.rough)),
            fire: Some(self.fire.unwrap_or(profile.fire)),
            rivers: Some(self.rivers.unwrap_or(profile.rivers)),
            frozen: Some(self.frozen.unwrap_or(profile.frozen)),
            settlements: self.settlements.clone(),
            roads: Some(RoadSpec {
                connect_settlements: Some(roads.connect_settlements.unwrap_or(true)),
                through_roads: Some(
                    roads
                        .through_roads
                        .unwrap_or(u8::from(!self.settlements.is_empty())),
                ),
                width: Some(roads.width.unwrap_or(1)),
            }),
            environment: Some(EnvironmentSpec {
                gravity: Some(environment.gravity.unwrap_or(profile.gravity)),
                temperature: Some(environment.temperature.unwrap_or(profile.temperature)),
                flags: Some(environment.flags.unwrap_or_else(|| profile.flags.to_vec())),
            }),
        })
    }
}

/// A seed from the clock and process, for specs that leave the seed out.
fn random_seed() -> u64 {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_nanos() as u64);
    crate::rng::mix(nanos ^ u64::from(std::process::id()).rotate_left(32))
}

/// Parse the compact settlement notation used on the command line:
/// `SIZE[@POSITION][,OPTION...]`, where each option is `walled`, `open`, `kind=KIND`,
/// `layout=LAYOUT`, `count=N`, `name=NAME` or `at=X:Y`. For example
/// `city@center,walled` or `village,count=3,kind=ruins`.
impl FromStr for SettlementSpec {
    type Err = anyhow::Error;

    fn from_str(text: &str) -> Result<Self> {
        let mut parts = text.split(',').map(str::trim);
        let head = parts.next().unwrap_or_default();
        let (size, position) = match head.split_once('@') {
            Some((size, position)) => (size, Some(position)),
            None => (head, None),
        };
        let mut spec = Self::new(named("settlement size", size)?);
        spec.position = position
            .map(|position| named("settlement position", position))
            .transpose()?;
        for option in parts {
            let (key, value) = option.split_once('=').unwrap_or((option, ""));
            match key {
                "walled" => spec.walled = Some(true),
                "open" => spec.walled = Some(false),
                "kind" => spec.kind = Some(named("settlement kind", value)?),
                "layout" => spec.layout = Some(named("settlement layout", value)?),
                "name" => spec.name = Some(value.to_owned()),
                "count" => {
                    spec.count = Some(value.parse().with_context(|| {
                        format!("settlement count must be a number, not {value:?}")
                    })?);
                }
                "at" => {
                    let (x, y) = value
                        .split_once(':')
                        .context("settlement at must be X:Y, like at=12:30")?;
                    let coordinate = |text: &str| {
                        text.parse::<u16>()
                            .with_context(|| format!("settlement at={value} needs numbers"))
                    };
                    spec.at = Some([coordinate(x)?, coordinate(y)?]);
                }
                _ => bail!(
                    "unknown settlement option {option:?}; use walled, open, kind=, layout=, \
                     name=, count= or at=X:Y"
                ),
            }
        }
        Ok(spec)
    }
}

/// Decode a snake_case enum value by its serde name. The error names the valid values.
fn named<T: serde::de::DeserializeOwned>(what: &str, name: &str) -> Result<T> {
    serde_json::from_value(serde_json::Value::String(name.to_owned()))
        .map_err(|error| anyhow::anyhow!("invalid {what}: {error}"))
}

/// The JSON Schema describing [`MapSpec`], for tool definitions and editor validation.
pub fn spec_schema() -> serde_json::Value {
    serde_json::to_value(schemars::schema_for!(MapSpec)).expect("schema serializes")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_json_and_toml() {
        let json = r#"{"biome": "desert", "size": "small", "settlements": [{"size": "town"}]}"#;
        let toml = "biome = \"desert\"\nsize = \"small\"\n[[settlements]]\nsize = \"town\"\n";
        let spec = MapSpec::parse(json).unwrap();
        assert_eq!(spec, MapSpec::parse(toml).unwrap());
        assert_eq!(spec.biome, Some(Biome::Desert));
        assert_eq!(spec.dimensions(), (30, 30));
    }

    #[test]
    fn rejects_unknown_fields_and_bad_ranges() {
        assert!(MapSpec::parse(r#"{"biomes": "desert"}"#).is_err());
        let wide = MapSpec {
            width: Some(2000),
            ..MapSpec::default()
        };
        assert!(wide.validate().unwrap_err().to_string().contains("width"));
        let off_map = MapSpec {
            size: Some(MapSize::Small),
            settlements: vec![SettlementSpec {
                at: Some([40, 1]),
                ..SettlementSpec::new(SettlementSize::Town)
            }],
            ..MapSpec::default()
        };
        assert!(off_map.validate().unwrap_err().to_string().contains("off"));
    }

    #[test]
    fn resolve_fills_every_default_and_keeps_choices() {
        let spec = MapSpec {
            biome: Some(Biome::Lunar),
            woods: Some(Amount::High),
            ..MapSpec::default()
        };
        let resolved = spec.resolve().unwrap();
        assert!(resolved.seed.is_some());
        assert_eq!(resolved.woods, Some(Amount::High));
        let environment = resolved.environment.clone().unwrap();
        assert!(environment.gravity.unwrap() < 50);
        assert!(environment.flags.unwrap().contains(&MapFlag::Vacuum));
        assert_eq!(resolved.resolve().unwrap(), resolved);
    }

    #[test]
    fn parses_compact_settlement_notation() {
        let spec: SettlementSpec = "city@northeast,walled,kind=industrial,count=2,at=3:4"
            .parse()
            .unwrap();
        assert_eq!(spec.size, SettlementSize::City);
        assert_eq!(spec.position, Some(Position::Northeast));
        assert_eq!(spec.walled, Some(true));
        assert_eq!(spec.kind, Some(SettlementKind::Industrial));
        assert_eq!(spec.count, Some(2));
        assert_eq!(spec.at, Some([3, 4]));
        assert!("megacity".parse::<SettlementSpec>().is_err());
        assert!("town,tall".parse::<SettlementSpec>().is_err());
    }

    #[test]
    fn schema_describes_fields() {
        let schema = spec_schema().to_string();
        assert!(schema.contains("settlements") && schema.contains("metropolis"));
        assert!(schema.contains("Overall landscape"));
        for flag in MapFlag::ALL {
            assert!(schema.contains(flag.name()) && schema.contains(flag.description()));
        }
    }
}
