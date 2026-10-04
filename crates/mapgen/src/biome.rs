//! Biome profiles: the default terrain mix, climate and landform each biome contributes.
//!
//! A [`MapSpec`](crate::MapSpec) field overrides the matching default here. Landform and
//! ground-cover tuning that has no spec field (snow lines, beaches, the shape of the ground)
//! lives only in the profile.
use crate::spec::{Amount, Biome, Relief};
use serde::Serialize;
use stompymux_map::MapFlag;

/// The basic material of open ground in a biome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BaseGround {
    Clear,
    Sand,
    Snow,
}

/// The large-scale shape of the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Landform {
    /// Plain fractal hills and hollows.
    Plain,
    /// Sharp ridgelines.
    Ridged,
    /// Land sloping down to a sea along one edge.
    Coastal,
    /// Flat-topped mesas with steep sides.
    Terraced,
    /// Bowl-shaped craters with raised rims.
    Cratered,
    /// A single large cone with a caldera.
    Volcano,
    /// Low ground pocked with many small hollows, so water collects in scattered pools.
    Marsh,
}

/// Defaults and tuning for one biome.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Profile {
    pub(crate) description: &'static str,
    pub(crate) relief: Relief,
    pub(crate) water: Amount,
    pub(crate) woods: Amount,
    pub(crate) rough: Amount,
    pub(crate) fire: Amount,
    pub(crate) rivers: u8,
    pub(crate) frozen: bool,
    pub(crate) gravity: u8,
    pub(crate) temperature: i8,
    pub(crate) flags: &'static [MapFlag],
    pub(crate) landform: Landform,
    pub(crate) ground: BaseGround,
    /// Share of open ground that is plain clear instead of the base ground.
    pub(crate) clear_patches: f64,
    /// Share of woods that are heavy.
    pub(crate) heavy_share: f64,
    /// Share of the highest land that is mountains.
    pub(crate) mountain_share: f64,
    /// Fraction of the maximum level above which open ground is snow.
    pub(crate) snow_line: Option<f64>,
    /// Deepest water the biome's lakes reach.
    pub(crate) max_depth: u8,
    /// Whether low land touching water is sand.
    pub(crate) beaches: bool,
}

/// Settings shared by most biomes, for struct-update syntax below.
const BASE: Profile = Profile {
    description: "",
    relief: Relief::Rolling,
    water: Amount::Low,
    woods: Amount::Medium,
    rough: Amount::Low,
    fire: Amount::None,
    rivers: 1,
    frozen: false,
    gravity: 100,
    temperature: 20,
    flags: &[],
    landform: Landform::Plain,
    ground: BaseGround::Clear,
    clear_patches: 0.0,
    heavy_share: 0.4,
    mountain_share: 0.0,
    snow_line: None,
    max_depth: 3,
    beaches: false,
};

/// The profile for `biome`.
pub(crate) const fn profile(biome: Biome) -> Profile {
    match biome {
        Biome::Temperate => Profile {
            description: "Rolling grassland, scattered woods, lakes and rivers.",
            ..BASE
        },
        Biome::Forest => Profile {
            description: "Dense light and heavy woods broken by clearings.",
            woods: Amount::High,
            heavy_share: 0.5,
            ..BASE
        },
        Biome::Jungle => Profile {
            description: "Hot, wet and choked with heavy woods and rivers.",
            relief: Relief::Hilly,
            water: Amount::Medium,
            woods: Amount::Extreme,
            rivers: 2,
            temperature: 35,
            heavy_share: 0.7,
            ..BASE
        },
        Biome::Desert => Profile {
            description: "Hot sand and hardpan with rocky outcrops and rare oases.",
            water: Amount::None,
            woods: Amount::None,
            rough: Amount::Medium,
            rivers: 0,
            temperature: 45,
            ground: BaseGround::Sand,
            clear_patches: 0.3,
            mountain_share: 0.04,
            ..BASE
        },
        Biome::Arctic => Profile {
            description: "Snowfields, frozen lakes and bitter cold.",
            water: Amount::Medium,
            woods: Amount::Low,
            rivers: 1,
            frozen: true,
            temperature: -30,
            ground: BaseGround::Snow,
            clear_patches: 0.15,
            heavy_share: 0.2,
            ..BASE
        },
        Biome::Mountains => Profile {
            description: "High ridges, rocky peaks and snow above the tree line.",
            relief: Relief::Mountainous,
            woods: Amount::Medium,
            rough: Amount::Medium,
            rivers: 1,
            temperature: 5,
            landform: Landform::Ridged,
            mountain_share: 0.15,
            snow_line: Some(0.75),
            max_depth: 4,
            ..BASE
        },
        Biome::Badlands => Profile {
            description: "Terraced mesas, canyons and broken rock.",
            relief: Relief::Hilly,
            water: Amount::None,
            woods: Amount::None,
            rough: Amount::High,
            rivers: 1,
            temperature: 35,
            landform: Landform::Terraced,
            ground: BaseGround::Sand,
            clear_patches: 0.6,
            mountain_share: 0.05,
            ..BASE
        },
        Biome::Swamp => Profile {
            description: "Flat, waterlogged ground with shallow pools and heavy woods.",
            relief: Relief::Flat,
            water: Amount::High,
            woods: Amount::High,
            rough: Amount::None,
            rivers: 2,
            temperature: 28,
            heavy_share: 0.6,
            max_depth: 1,
            landform: Landform::Marsh,
            ..BASE
        },
        Biome::Coastal => Profile {
            description: "Land running down to a sea along one edge of the map, with beaches.",
            water: Amount::High,
            woods: Amount::Low,
            rivers: 1,
            landform: Landform::Coastal,
            max_depth: 6,
            beaches: true,
            ..BASE
        },
        Biome::Lunar => Profile {
            description: "Airless, low-gravity cratered rock.",
            relief: Relief::Rolling,
            water: Amount::None,
            woods: Amount::None,
            rough: Amount::High,
            rivers: 0,
            gravity: 17,
            temperature: -120,
            flags: &[MapFlag::SpecialRules, MapFlag::Vacuum],
            landform: Landform::Cratered,
            mountain_share: 0.03,
            ..BASE
        },
        Biome::Volcanic => Profile {
            description: "A volcano with rough slopes, fire and smoke.",
            relief: Relief::Hilly,
            water: Amount::None,
            woods: Amount::Low,
            rough: Amount::High,
            fire: Amount::Medium,
            rivers: 0,
            temperature: 40,
            landform: Landform::Volcano,
            mountain_share: 0.08,
            ..BASE
        },
    }
}

/// A biome's description and defaults, for catalogs shown to people and LLMs.
#[derive(Debug, Clone, Serialize)]
pub struct BiomeInfo {
    pub biome: Biome,
    pub description: &'static str,
    pub relief: Relief,
    pub water: Amount,
    pub woods: Amount,
    pub rough: Amount,
    pub fire: Amount,
    pub rivers: u8,
    pub frozen: bool,
    pub gravity: u8,
    pub temperature: i8,
    pub flags: Vec<MapFlag>,
}

/// Every biome with its description and defaults.
pub fn biome_catalog() -> Vec<BiomeInfo> {
    Biome::ALL
        .into_iter()
        .map(|biome| {
            let profile = profile(biome);
            BiomeInfo {
                biome,
                description: profile.description,
                relief: profile.relief,
                water: profile.water,
                woods: profile.woods,
                rough: profile.rough,
                fire: profile.fire,
                rivers: profile.rivers,
                frozen: profile.frozen,
                gravity: profile.gravity,
                temperature: profile.temperature,
                flags: profile.flags.to_vec(),
            }
        })
        .collect()
}
