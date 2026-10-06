//! The generation report: what was built and where, so a person, an editor or an LLM can
//! check the result against what they asked for without reading the hex grids.
use crate::map::HexMap;
use crate::spec::{Biome, SettlementKind, SettlementLayout, SettlementSize};
use serde::Serialize;
use stompymux_map::{Condition, Foliage, Ground, Route, StructureKind};

/// A summary of a generated map.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub seed: u64,
    pub biome: Biome,
    pub width: u16,
    pub height: u16,
    /// Percent of hexes holding each kind of ground, water, foliage, road, structure and snow.
    pub coverage: Coverage,
    pub rivers: usize,
    pub settlements: Vec<SettlementReport>,
    pub roads: Vec<RoadReport>,
    pub fire_hexes: usize,
    pub smoke_hexes: usize,
    /// Requests that could not be met exactly, such as a settlement shrunk to fit.
    pub warnings: Vec<String>,
}

/// A settlement as built.
#[derive(Debug, Clone, Serialize)]
pub struct SettlementReport {
    pub name: String,
    pub size: SettlementSize,
    pub kind: SettlementKind,
    pub layout: SettlementLayout,
    pub walled: bool,
    /// `[x, y]` of its center hex.
    pub center: [u16; 2],
    /// Distance in hexes from the center to the edge of its footprint.
    pub radius: u16,
    pub buildings: usize,
    pub tallest: u8,
    /// `[x, y]` hexes where streets leave the settlement.
    pub gates: Vec<[u16; 2]>,
}

/// A road as built.
#[derive(Debug, Clone, Serialize)]
pub struct RoadReport {
    /// Settlement name or map edge the road starts from.
    pub from: String,
    /// Settlement name or map edge the road leads to.
    pub to: String,
    /// Hexes along the road's center line.
    pub hexes: usize,
    /// Bridge hexes it added.
    pub bridges: usize,
    /// The road's surface where it does not share a better road.
    pub surface: Route,
}

/// Percent of the map's hexes holding each feature. A hex has several layers, so one hex can
/// count toward several fields: a snowy wood counts toward its ground, its woods and its snow.
/// The ground fields count dry hexes by their ground and add up to 100 with `water`.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Coverage {
    pub clear: f64,
    pub pavement: f64,
    pub rough: f64,
    pub ultra_rough: f64,
    pub rubble: f64,
    pub sand: f64,
    pub tundra: f64,
    pub swamp: f64,
    /// Lakes, seas and rivers, frozen or not.
    pub water: f64,
    /// Water frozen over.
    pub ice: f64,
    /// Water running as rapids or torrents.
    pub rapids: f64,
    pub light_woods: f64,
    pub heavy_woods: f64,
    pub ultra_heavy_woods: f64,
    pub light_jungle: f64,
    pub heavy_jungle: f64,
    pub ultra_heavy_jungle: f64,
    pub planted_fields: f64,
    pub paved_road: f64,
    pub gravel_road: f64,
    pub dirt_road: f64,
    pub bridge: f64,
    pub building: f64,
    pub wall: f64,
    pub thin_snow: f64,
    pub deep_snow: f64,
}

impl Coverage {
    /// Measure the terrain mix of `map`.
    pub fn of(map: &HexMap) -> Self {
        let mut coverage = Self::default();
        let count = |slot: &mut f64| *slot += 1.0;
        for hex in &map.hexes {
            if let Some(water) = hex.water() {
                count(&mut coverage.water);
                if hex.is_frozen() {
                    count(&mut coverage.ice);
                }
                if !water.flow.is_still() {
                    count(&mut coverage.rapids);
                }
            } else {
                match hex.ground() {
                    Ground::Clear => count(&mut coverage.clear),
                    Ground::Pavement => count(&mut coverage.pavement),
                    Ground::Rough => count(&mut coverage.rough),
                    Ground::UltraRough => count(&mut coverage.ultra_rough),
                    Ground::Rubble | Ground::UltraRubble => count(&mut coverage.rubble),
                    Ground::Sand => count(&mut coverage.sand),
                    Ground::Tundra => count(&mut coverage.tundra),
                    Ground::Swamp => count(&mut coverage.swamp),
                    Ground::MagmaCrust | Ground::Magma | Ground::HeavyIndustrial => {}
                }
            }
            match hex.foliage() {
                Some(Foliage::LightWoods) => count(&mut coverage.light_woods),
                Some(Foliage::HeavyWoods) => count(&mut coverage.heavy_woods),
                Some(Foliage::UltraHeavyWoods) => count(&mut coverage.ultra_heavy_woods),
                Some(Foliage::LightJungle) => count(&mut coverage.light_jungle),
                Some(Foliage::HeavyJungle) => count(&mut coverage.heavy_jungle),
                Some(Foliage::UltraHeavyJungle) => count(&mut coverage.ultra_heavy_jungle),
                Some(Foliage::PlantedFields) => count(&mut coverage.planted_fields),
                None => {}
            }
            match hex.route() {
                Some(Route::PavedRoad) => count(&mut coverage.paved_road),
                Some(Route::GravelRoad) => count(&mut coverage.gravel_road),
                Some(Route::DirtRoad) => count(&mut coverage.dirt_road),
                Some(Route::Rail) | None => {}
            }
            match hex.structure().map(|structure| structure.kind) {
                Some(StructureKind::Building) => count(&mut coverage.building),
                Some(StructureKind::Wall) => count(&mut coverage.wall),
                Some(StructureKind::Bridge) => count(&mut coverage.bridge),
                None => {}
            }
            match hex.condition() {
                Some(Condition::ThinSnow) => count(&mut coverage.thin_snow),
                Some(Condition::DeepSnow) => count(&mut coverage.deep_snow),
                Some(Condition::Ice | Condition::Mud) | None => {}
            }
        }
        let total = map.hexes.len().max(1) as f64;
        let percent = |slot: &mut f64| *slot = (*slot * 1000.0 / total).round() / 10.0;
        for slot in [
            &mut coverage.clear,
            &mut coverage.pavement,
            &mut coverage.rough,
            &mut coverage.ultra_rough,
            &mut coverage.rubble,
            &mut coverage.sand,
            &mut coverage.tundra,
            &mut coverage.swamp,
            &mut coverage.water,
            &mut coverage.ice,
            &mut coverage.rapids,
            &mut coverage.light_woods,
            &mut coverage.heavy_woods,
            &mut coverage.ultra_heavy_woods,
            &mut coverage.light_jungle,
            &mut coverage.heavy_jungle,
            &mut coverage.ultra_heavy_jungle,
            &mut coverage.planted_fields,
            &mut coverage.paved_road,
            &mut coverage.gravel_road,
            &mut coverage.dirt_road,
            &mut coverage.bridge,
            &mut coverage.building,
            &mut coverage.wall,
            &mut coverage.thin_snow,
            &mut coverage.deep_snow,
        ] {
            percent(slot);
        }
        coverage
    }

    /// Percent of hexes carrying a road of any surface.
    pub fn road(&self) -> f64 {
        self.paved_road + self.gravel_road + self.dirt_road
    }

    /// Percent of hexes under thin or deep snow.
    pub fn snow(&self) -> f64 {
        self.thin_snow + self.deep_snow
    }

    /// Percent of hexes covered by woods or jungle of any density.
    pub fn trees(&self) -> f64 {
        self.light_woods
            + self.heavy_woods
            + self.ultra_heavy_woods
            + self.light_jungle
            + self.heavy_jungle
            + self.ultra_heavy_jungle
    }
}
