//! Natural terrain: the elevation field and ground levels, seas and lakes, rivers, ground cover,
//! permanent fire and smoke, and freezing.
//!
//! Amounts in the spec are fractions of the map, so each feature takes the hexes that score
//! highest on its own smooth noise field until that fraction is covered. That keeps a
//! requested "high woods" close to the same share of the map whatever the seed.
use crate::Params;
use crate::biome::{BaseGround, Landform};
use crate::map::{HexMap, Terrain};
use crate::noise::Noise;
use crate::path::find_path;
use crate::rng::Rng;
use crate::spec::Relief;
use stompymux_map::{DecorationKind, HexCoordinate, Point};

/// Size in hexes of the largest hills and valleys.
const FEATURE_SCALE: f64 = 16.0;

/// The value at or above which `fraction` of `values` lie: infinite for nothing, negative
/// infinity for everything.
fn threshold(values: impl Iterator<Item = f64>, fraction: f64) -> f64 {
    if fraction <= 0.0 {
        return f64::INFINITY;
    }
    let mut sorted: Vec<f64> = values.collect();
    if fraction >= 1.0 || sorted.is_empty() {
        return f64::NEG_INFINITY;
    }
    sorted.sort_by(f64::total_cmp);
    let index = ((1.0 - fraction) * sorted.len() as f64) as usize;
    sorted[index.min(sorted.len() - 1)]
}

/// Rescale `values` to span exactly `[0, 1]`.
fn normalize(values: &mut [f64]) {
    let (low, high) = values
        .iter()
        .fold((f64::INFINITY, f64::NEG_INFINITY), |(low, high), &value| {
            (low.min(value), high.max(value))
        });
    let span = (high - low).max(f64::EPSILON);
    for value in values {
        *value = (*value - low) / span;
    }
}

/// The smooth elevation field in `[0, 1]` for every hex, shaped by the biome's landform.
pub(crate) fn elevation(map: &HexMap, params: &Params) -> Vec<f64> {
    let base = Noise::new(params.seed, "elevation");
    let detail = Noise::new(params.seed, "elevation-detail");
    let ridges = Noise::new(params.seed, "ridges");
    let mut rng = Rng::stream(params.seed, "landform");
    let (width, height) = (f64::from(map.width), f64::from(map.height));
    let span = HexCoordinate {
        x: i32::from(map.width) - 1,
        y: i32::from(map.height) - 1,
    }
    .center();
    let coast_side = rng.index(4);
    let mut field: Vec<f64> = (0..map.hexes.len())
        .map(|index| {
            let (x, y) = map.coordinate(index);
            let Point { x: cx, y: cy } = HexCoordinate { x, y }.center();
            let plain =
                base.fractal(cx, cy, FEATURE_SCALE, 4) + 0.08 * detail.fractal(cx, cy, 4.0, 2);
            match params.profile.landform {
                Landform::Plain | Landform::Volcano => plain,
                Landform::Cratered => 0.5 * plain,
                Landform::Marsh => 0.2 * plain + 0.8 * ridges.fractal(cx, cy, 3.0, 2),
                Landform::Ridged => {
                    let ridge = 1.0 - (2.0 * ridges.fractal(cx, cy, FEATURE_SCALE, 3) - 1.0).abs();
                    0.4 * plain + 0.6 * ridge * ridge
                }
                Landform::Coastal => {
                    let inland = match coast_side {
                        0 => cy / span.y,
                        1 => 1.0 - cx / span.x,
                        2 => 1.0 - cy / span.y,
                        _ => cx / span.x,
                    };
                    0.6 * inland + 0.4 * plain
                }
                Landform::Terraced => {
                    let steps = 4.0 * plain * 1.5;
                    (steps.floor() + steps.fract().powi(6)) / 6.0
                }
            }
        })
        .collect();
    match params.profile.landform {
        Landform::Cratered => {
            let craters = (width * height / 300.0).max(2.0) as usize;
            let largest = (width.min(height) / 6.0).max(3.0) as i32;
            for _ in 0..craters {
                let middle = HexCoordinate {
                    x: rng.between(0, i32::from(map.width) - 1),
                    y: rng.between(0, i32::from(map.height) - 1),
                }
                .center();
                let radius = f64::from(rng.between(2, largest));
                for (index, value) in field.iter_mut().enumerate() {
                    let (x, y) = map.coordinate(index);
                    let Point { x: cx, y: cy } = HexCoordinate { x, y }.center();
                    let distance = ((cx - middle.x).powi(2) + (cy - middle.y).powi(2)).sqrt();
                    let ratio = distance / radius;
                    if ratio < 1.0 {
                        *value -= 0.35 * (1.0 - ratio * ratio);
                    } else if ratio < 1.4 {
                        *value += 0.12 * (1.0 - (ratio - 1.2).abs() / 0.2).max(0.0);
                    }
                }
            }
        }
        Landform::Volcano => {
            let peak = HexCoordinate {
                x: (width * (0.3 + 0.4 * rng.unit())) as i32,
                y: (height * (0.3 + 0.4 * rng.unit())) as i32,
            }
            .center();
            let radius = 0.45 * span.x.min(span.y);
            for (index, value) in field.iter_mut().enumerate() {
                let (x, y) = map.coordinate(index);
                let Point { x: cx, y: cy } = HexCoordinate { x, y }.center();
                let ratio = ((cx - peak.x).powi(2) + (cy - peak.y).powi(2)).sqrt() / radius;
                *value = 0.35 * *value + 0.9 * (1.0 - ratio).max(0.0).powf(1.6);
                if ratio < 0.12 {
                    *value -= 0.5 * (1.0 - ratio / 0.12);
                }
            }
        }
        _ => {}
    }
    normalize(&mut field);
    field
}

/// Set every hex's ground level from the elevation field, flooding the lowest ground as water.
pub(crate) fn shape(map: &mut HexMap, elevation: &[f64], params: &Params) {
    // Elevation spans [0, 1], so a dry map's sea sits at the lowest point.
    let sea = (-threshold(
        elevation.iter().map(|value| -value),
        params.water.water_fraction(),
    ))
    .max(0.0);
    let max_level = f64::from(params.relief.max_level());
    let curve = match params.relief {
        Relief::Flat => 1.0,
        Relief::Rolling => 1.2,
        Relief::Hilly => 1.3,
        Relief::Mountainous => 1.5,
    };
    let max_depth = f64::from(params.profile.max_depth);
    for (hex, &value) in map.hexes.iter_mut().zip(elevation) {
        if value < sea {
            let depth = 1.0 + ((sea - value) / sea.max(f64::EPSILON)) * (max_depth - 1.0);
            hex.level = 0;
            hex.terrain = Terrain::Water {
                depth: depth.round().clamp(1.0, max_depth) as u8,
            };
            continue;
        }
        let height = ((value - sea) / (1.0 - sea).max(f64::EPSILON)).clamp(0.0, 1.0);
        hex.level = (height.powf(curve) * max_level).round() as u8;
    }
}

/// Trace rivers from high ground on one edge down to the far edge, carving their beds and
/// banks into the ground. Returns how many rivers were drawn.
pub(crate) fn rivers(map: &mut HexMap, elevation: &[f64], params: &Params) -> usize {
    let mut rng = Rng::stream(params.seed, "rivers");
    let meander = Noise::new(params.seed, "meander");
    let depth = if params.relief >= Relief::Hilly { 2 } else { 1 };
    let (width, height) = (i32::from(map.width), i32::from(map.height));
    let mut drawn = 0;
    for river in 0..params.rivers {
        let across = (river + rng.index(2) as u8).is_multiple_of(2);
        let flip = rng.chance(0.5);
        // Edge hexes as (start side, far side) pairs, indexed by position along the edge.
        let edge = |along: i32, far: bool| -> (i32, i32) {
            let far = far != flip;
            if across {
                (if far { width - 1 } else { 0 }, along)
            } else {
                (along, if far { height - 1 } else { 0 })
            }
        };
        let length = if across { height } else { width };
        let start = (0..12)
            .map(|_| edge(rng.between(length / 8, length - 1 - length / 8), false))
            .max_by(|a, b| {
                let value = |(x, y)| elevation[map.index(x, y)];
                value(*a).total_cmp(&value(*b))
            })
            .expect("candidates");
        let reached = |(x, y): (i32, i32)| {
            let far = edge(0, true);
            if across { x == far.0 } else { y == far.1 }
        };
        let remaining = |(x, y): (i32, i32)| {
            let far = edge(0, true);
            if across {
                (x - far.0).unsigned_abs()
            } else {
                (y - far.1).unsigned_abs()
            }
        };
        let map_ref = &*map;
        let Some(path) = find_path(map_ref, start, reached, remaining, |_, (x, y)| {
            let index = map_ref.index(x, y);
            if map_ref.hexes[index].terrain.is_water() {
                return Some(1);
            }
            let Point { x: cx, y: cy } = HexCoordinate { x, y }.center();
            Some(4 + (40.0 * elevation[index] + 20.0 * meander.fractal(cx, cy, 6.0, 2)) as u32)
        }) else {
            continue;
        };
        drawn += 1;
        let mut surface = map.hexes[map.index(start.0, start.1)].level;
        for &(x, y) in &path {
            let hex = map.hex_mut(x, y).expect("path on map");
            surface = surface.min(hex.level);
            hex.level = surface;
            if !hex.terrain.is_water() {
                hex.terrain = Terrain::Water { depth };
            }
            for (nx, ny) in neighbors_on(map, x, y) {
                let bank = map.hex_mut(nx, ny).expect("neighbor on map");
                if !bank.terrain.is_water() {
                    bank.level = bank.level.min(surface + 2);
                }
            }
        }
    }
    drawn
}

/// The on-map neighbors of `(x, y)`, collected so the map can be changed while visiting them.
fn neighbors_on(map: &HexMap, x: i32, y: i32) -> Vec<(i32, i32)> {
    map.neighbors(x, y).collect()
}

/// Whether any neighbor of `(x, y)` is water.
fn touches_water(map: &HexMap, x: i32, y: i32) -> bool {
    map.neighbors(x, y)
        .any(|(nx, ny)| map.hexes[map.index(nx, ny)].terrain.is_water())
}

/// Cover the land with mountains, snow, woods, rough ground, beaches and the biome's open
/// ground.
pub(crate) fn cover(map: &mut HexMap, elevation: &[f64], params: &Params) {
    let profile = &params.profile;
    let moisture = Noise::new(params.seed, "moisture");
    let roughness = Noise::new(params.seed, "roughness");
    let patches = Noise::new(params.seed, "patches");
    let land: Vec<usize> = (0..map.hexes.len())
        .filter(|&index| !map.hexes[index].terrain.is_water())
        .collect();
    let sample = |index: usize, noise: Noise, scale: f64| {
        let (x, y) = map.coordinate(index);
        let Point { x: cx, y: cy } = HexCoordinate { x, y }.center();
        noise.fractal(cx, cy, scale, 3)
    };
    let wet: Vec<f64> = (0..map.hexes.len())
        .map(|index| {
            let (x, y) = map.coordinate(index);
            sample(index, moisture, 10.0) + if touches_water(map, x, y) { 0.1 } else { 0.0 }
        })
        .collect();
    let rugged: Vec<f64> = (0..map.hexes.len())
        .map(|index| {
            let (x, y) = map.coordinate(index);
            let level = i32::from(map.hexes[index].level);
            let slope = map
                .neighbors(x, y)
                .map(|(nx, ny)| (i32::from(map.hexes[map.index(nx, ny)].level) - level).abs())
                .max()
                .unwrap_or(0);
            sample(index, roughness, 6.0) + 0.05 * f64::from(slope)
        })
        .collect();
    let woods_fraction = params.woods.woods_fraction();
    let mountains = threshold(land.iter().map(|&i| elevation[i]), profile.mountain_share);
    let woods = threshold(land.iter().map(|&i| wet[i]), woods_fraction);
    let heavy = threshold(
        land.iter().map(|&i| wet[i]),
        woods_fraction * profile.heavy_share,
    );
    let rough = threshold(
        land.iter().map(|&i| rugged[i]),
        params.rough.rough_fraction(),
    );
    let snow_level = profile
        .snow_line
        .map(|line| (line * f64::from(params.relief.max_level())).ceil() as u8);
    let mut terrain = Vec::with_capacity(land.len());
    for &index in &land {
        let (x, y) = map.coordinate(index);
        let level = map.hexes[index].level;
        terrain.push(
            if elevation[index] >= mountains && params.relief >= Relief::Hilly {
                Terrain::Mountains
            } else if snow_level.is_some_and(|snow| level >= snow.max(1)) {
                Terrain::Snow
            } else if wet[index] >= heavy {
                Terrain::HeavyWoods
            } else if wet[index] >= woods {
                Terrain::LightWoods
            } else if rugged[index] >= rough {
                Terrain::Rough
            } else if profile.beaches && level <= 1 && touches_water(map, x, y) {
                Terrain::Sand
            } else if sample(index, patches, 5.0) < profile.clear_patches {
                Terrain::Clear
            } else {
                open_ground(profile.ground)
            },
        );
    }
    for (&index, terrain) in land.iter().zip(terrain) {
        map.hexes[index].terrain = terrain;
    }
}

/// The terrain for open ground of a biome's base material.
pub(crate) fn open_ground(ground: BaseGround) -> Terrain {
    match ground {
        BaseGround::Clear => Terrain::Clear,
        BaseGround::Sand => Terrain::Sand,
        BaseGround::Snow => Terrain::Snow,
    }
}

/// Lay permanent fire and smoke in clusters: fire at each cluster's heart, smoke around it.
/// Volcanic maps burn hottest near the summit.
pub(crate) fn burn(map: &mut HexMap, elevation: &[f64], params: &Params) {
    let fraction = params.fire.fire_fraction();
    if fraction <= 0.0 {
        return;
    }
    let heat = Noise::new(params.seed, "fire");
    let summit_bias = if params.profile.landform == Landform::Volcano {
        0.6
    } else {
        0.0
    };
    let score: Vec<f64> = (0..map.hexes.len())
        .map(|index| {
            if map.hexes[index].terrain.is_water() {
                return f64::NEG_INFINITY;
            }
            let (x, y) = map.coordinate(index);
            let Point { x: cx, y: cy } = HexCoordinate { x, y }.center();
            heat.fractal(cx, cy, 5.0, 2) + summit_bias * elevation[index]
        })
        .collect();
    let smoke = threshold(score.iter().copied(), fraction);
    let fire = threshold(score.iter().copied(), fraction * 0.35);
    for (hex, &value) in map.hexes.iter_mut().zip(&score) {
        if value >= fire {
            hex.overlay = Some(DecorationKind::Fire);
        } else if value >= smoke {
            hex.overlay = Some(DecorationKind::Smoke);
        }
    }
}

/// Freeze every lake and river over.
pub(crate) fn freeze(map: &mut HexMap) {
    for hex in &mut map.hexes {
        if let Terrain::Water { depth } = hex.terrain {
            hex.terrain = Terrain::Ice { depth };
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn threshold_selects_the_requested_share() {
        let values = (0..100).map(f64::from);
        assert_eq!(threshold(values.clone(), 0.25), 75.0);
        assert_eq!(threshold(values.clone(), 0.0), f64::INFINITY);
        assert_eq!(threshold(values, 1.0), f64::NEG_INFINITY);
    }
}
