//! The generation report: what was built and where, so a person, an editor or an LLM can
//! check the result against what they asked for without reading the hex grids.
use crate::map::{HexMap, Terrain};
use crate::spec::{Biome, SettlementKind, SettlementLayout, SettlementSize};
use serde::Serialize;

/// A summary of a generated map.
#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub seed: u64,
    pub biome: Biome,
    pub width: u16,
    pub height: u16,
    /// Percent of hexes of each terrain.
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
}

/// Percent of the map's hexes holding each terrain.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Coverage {
    pub clear: f64,
    pub road: f64,
    pub rough: f64,
    pub mountains: f64,
    pub snow: f64,
    pub sand: f64,
    pub light_woods: f64,
    pub heavy_woods: f64,
    pub water: f64,
    pub ice: f64,
    pub building: f64,
    pub wall: f64,
}

impl Coverage {
    /// Measure the terrain mix of `map`.
    pub fn of(map: &HexMap) -> Self {
        let mut coverage = Self::default();
        for hex in &map.hexes {
            let slot = match hex.terrain {
                Terrain::Clear => &mut coverage.clear,
                Terrain::Road => &mut coverage.road,
                Terrain::Rough => &mut coverage.rough,
                Terrain::Mountains => &mut coverage.mountains,
                Terrain::Snow => &mut coverage.snow,
                Terrain::Sand => &mut coverage.sand,
                Terrain::LightWoods => &mut coverage.light_woods,
                Terrain::HeavyWoods => &mut coverage.heavy_woods,
                Terrain::Water { .. } => &mut coverage.water,
                Terrain::Ice { .. } => &mut coverage.ice,
                Terrain::Building { .. } => &mut coverage.building,
                Terrain::Wall { .. } => &mut coverage.wall,
            };
            *slot += 1.0;
        }
        let total = map.hexes.len().max(1) as f64;
        for slot in [
            &mut coverage.clear,
            &mut coverage.road,
            &mut coverage.rough,
            &mut coverage.mountains,
            &mut coverage.snow,
            &mut coverage.sand,
            &mut coverage.light_woods,
            &mut coverage.heavy_woods,
            &mut coverage.water,
            &mut coverage.ice,
            &mut coverage.building,
            &mut coverage.wall,
        ] {
            *slot = (*slot * 1000.0 / total).round() / 10.0;
        }
        coverage
    }
}
