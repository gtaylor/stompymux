//! Settlements: choosing a site, levelling it, and laying out streets, buildings and walls.
//!
//! Each settlement gets a footprint (a rectangle for planned layouts, a ragged disc for grown
//! ones) on the flattest dry ground near its requested position. The footprint is levelled
//! and blended into the land around it, then filled by its layout. Streets that reach the
//! footprint's edge become gates, which the road network links to.
use crate::Params;
use crate::map::{HexMap, Terrain};
use crate::noise::Noise;
use crate::report::SettlementReport;
use crate::rng::Rng;
use crate::spec::{SettlementKind, SettlementLayout, SettlementSize, SettlementSpec};
use crate::terrain::open_ground;
use std::collections::VecDeque;
use stompymux_map::{BattleDecorationKind, BattleHexCoordinate, BattlePoint};

/// A settlement as built, for routing roads and reporting.
#[derive(Debug, Clone)]
pub(crate) struct Settlement {
    pub(crate) name: String,
    pub(crate) center: (i32, i32),
    pub(crate) radius: i32,
    /// Hexes where roads may enter, on the footprint's edge.
    pub(crate) gates: Vec<(i32, i32)>,
    /// Row-major indices of every hex inside the footprint.
    pub(crate) footprint: Vec<usize>,
    pub(crate) report: SettlementReport,
}

/// Dimensions and building heights for a settlement size.
struct Scale {
    radius: i32,
    lowest: u8,
    tallest: u8,
    density: f64,
}

/// The scale of a settlement of `size`.
const fn scale(size: SettlementSize) -> Scale {
    let (radius, lowest, tallest, density) = match size {
        SettlementSize::Outpost => (2, 1, 2, 0.5),
        SettlementSize::Hamlet => (3, 1, 2, 0.4),
        SettlementSize::Village => (5, 1, 3, 0.55),
        SettlementSize::Town => (8, 1, 5, 0.7),
        SettlementSize::City => (12, 2, 10, 0.8),
        SettlementSize::Metropolis => (18, 3, 20, 0.85),
    };
    Scale {
        radius,
        lowest,
        tallest,
        density,
    }
}

/// What a layout puts in one footprint hex.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Plot {
    Open,
    Park,
    Street,
    Building(u8),
    Wall(u8),
    /// Collapsed buildings, walls or debris, laid as rough ground.
    Rubble,
}

/// Place and build every requested settlement in order, expanding `count`s.
pub(crate) fn build_all(
    map: &mut HexMap,
    specs: &[SettlementSpec],
    params: &Params,
    warnings: &mut Vec<String>,
) -> Vec<Settlement> {
    let mut rng = Rng::stream(params.seed, "settlements");
    let mut built: Vec<Settlement> = Vec::new();
    for spec in specs {
        for copy in 0..spec.count.unwrap_or(1) {
            let number = built
                .iter()
                .filter(|other| other.report.size == spec.size)
                .count()
                + 1;
            let name = match (&spec.name, spec.count.unwrap_or(1)) {
                (Some(name), 1) => name.clone(),
                (Some(name), _) => format!("{name} {}", copy + 1),
                (None, _) => format!("{} {number}", size_name(spec.size)),
            };
            let settlement = build(map, spec, name, &built, params, &mut rng, warnings);
            built.push(settlement);
        }
    }
    built
}

/// The lower-case name of a settlement size.
fn size_name(size: SettlementSize) -> &'static str {
    match size {
        SettlementSize::Outpost => "outpost",
        SettlementSize::Hamlet => "hamlet",
        SettlementSize::Village => "village",
        SettlementSize::Town => "town",
        SettlementSize::City => "city",
        SettlementSize::Metropolis => "metropolis",
    }
}

/// Choose a site for one settlement and build it.
fn build(
    map: &mut HexMap,
    spec: &SettlementSpec,
    name: String,
    existing: &[Settlement],
    params: &Params,
    rng: &mut Rng,
    warnings: &mut Vec<String>,
) -> Settlement {
    let kind = spec.kind.unwrap_or(SettlementKind::Civilian);
    let layout = spec.layout.unwrap_or(match (kind, spec.size) {
        (SettlementKind::Military, _) => SettlementLayout::Compound,
        (SettlementKind::Industrial, _) => SettlementLayout::Grid,
        (_, SettlementSize::Outpost | SettlementSize::Hamlet) => SettlementLayout::Scattered,
        (_, SettlementSize::Village) => SettlementLayout::Organic,
        _ => SettlementLayout::Grid,
    });
    let walled = spec.walled.unwrap_or(kind == SettlementKind::Military);
    let mut scale = scale(spec.size);
    if matches!(kind, SettlementKind::Industrial | SettlementKind::Military) {
        scale.tallest = scale.tallest.clamp(2, 4);
        scale.lowest = scale.lowest.min(scale.tallest);
    }
    if kind == SettlementKind::Industrial {
        scale.density = (scale.density + 0.1).min(0.95);
    }
    let largest = (i32::from(map.width.min(map.height)) / 2 - 2).max(1);
    if scale.radius > largest {
        warnings.push(format!(
            "{name} was shrunk from radius {} to {largest} to fit the {}x{} map",
            scale.radius, map.width, map.height
        ));
        scale.radius = largest;
    }
    let site = choose_site(map, spec, scale.radius, existing, rng);
    if existing
        .iter()
        .any(|other| HexMap::distance(site, other.center) < scale.radius + other.radius + 1)
    {
        warnings.push(format!(
            "{name} overlaps another settlement; the map is crowded"
        ));
    }
    let rectangular = matches!(layout, SettlementLayout::Grid | SettlementLayout::Compound);
    let footprint = footprint(map, site, scale.radius, rectangular, params.seed);
    level_site(map, &footprint);
    let mut plots = match layout {
        SettlementLayout::Grid => grid_plots(map, site, &scale, kind, &footprint, rng),
        SettlementLayout::Organic => organic_plots(map, site, &scale, &footprint, rng),
        SettlementLayout::Scattered => scattered_plots(map, site, &scale, &footprint, rng),
        SettlementLayout::Compound => compound_plots(map, site, &scale, &footprint, rng),
    };
    let edge = edge_hexes(map, &footprint);
    let gate_slots = if walled {
        let height = if spec.size >= SettlementSize::City || kind == SettlementKind::Military {
            3
        } else {
            2
        };
        wall_in(
            map,
            site,
            scale.radius,
            &footprint,
            &edge,
            &mut plots,
            height,
        )
    } else {
        let on_edge = membership(map, &edge);
        (0..footprint.len())
            .filter(|&slot| plots[slot] == Plot::Street && on_edge[footprint[slot]])
            .collect()
    };
    if kind == SettlementKind::Ruins {
        ruin(&mut plots, &gate_slots, rng);
    }
    let mut gates: Vec<(i32, i32)> = gate_slots
        .iter()
        .map(|&slot| map.coordinate(footprint[slot]))
        .collect();
    if gates.is_empty() {
        gates.push(site);
    }
    let (buildings, tallest) = apply(map, &footprint, &plots, params, kind, rng);
    Settlement {
        report: SettlementReport {
            name: name.clone(),
            size: spec.size,
            kind,
            layout,
            walled,
            center: [site.0 as u16, site.1 as u16],
            radius: scale.radius as u16,
            buildings,
            tallest,
            gates: gates.iter().map(|&(x, y)| [x as u16, y as u16]).collect(),
        },
        name,
        center: site,
        radius: scale.radius,
        gates,
        footprint,
    }
}

/// Pick the settlement's center: the exact hex asked for, or the flattest, driest site near
/// the requested region that keeps clear of other settlements.
fn choose_site(
    map: &HexMap,
    spec: &SettlementSpec,
    radius: i32,
    existing: &[Settlement],
    rng: &mut Rng,
) -> (i32, i32) {
    let (width, height) = (i32::from(map.width), i32::from(map.height));
    let keep_on_map = |(x, y): (i32, i32)| {
        let clamp = |value: i32, size: i32| {
            if size > 2 * radius {
                value.clamp(radius, size - 1 - radius)
            } else {
                size / 2
            }
        };
        (clamp(x, width), clamp(y, height))
    };
    if let Some([x, y]) = spec.at {
        return (i32::from(x), i32::from(y));
    }
    let anchor = spec.position.and_then(|position| position.anchor());
    let target = anchor.map(|(fx, fy)| {
        (
            (fx * f64::from(width - 1)).round() as i32,
            (fy * f64::from(height - 1)).round() as i32,
        )
    });
    let reach = (width.max(height) / 5).max(radius);
    let mut candidates: Vec<(i32, i32)> = target.into_iter().map(keep_on_map).collect();
    for _ in 0..160 {
        let point = match target {
            Some((tx, ty)) => (
                tx + rng.between(-reach, reach),
                ty + rng.between(-reach, reach),
            ),
            None => (rng.between(0, width - 1), rng.between(0, height - 1)),
        };
        candidates.push(keep_on_map(point));
    }
    let score = |site: (i32, i32)| -> f64 {
        let mut levels = Vec::new();
        let mut wet = 0;
        let mut total = 0;
        for y in site.1 - radius..=site.1 + radius {
            for x in site.0 - radius..=site.0 + radius {
                let Some(hex) = map.hex(x, y) else {
                    continue;
                };
                if HexMap::distance(site, (x, y)) > radius {
                    continue;
                }
                total += 1;
                if hex.terrain.is_water() {
                    wet += 1;
                } else {
                    levels.push(f64::from(hex.level));
                }
            }
        }
        let mean = levels.iter().sum::<f64>() / levels.len().max(1) as f64;
        let spread = (levels
            .iter()
            .map(|level| (level - mean).powi(2))
            .sum::<f64>()
            / levels.len().max(1) as f64)
            .sqrt();
        let mut score = 10.0 * f64::from(wet) / f64::from(total.max(1)) + spread;
        if let Some(target) = target {
            score += 0.6 * f64::from(HexMap::distance(site, target)) / f64::from(radius.max(1));
        }
        for other in existing {
            let gap = HexMap::distance(site, other.center) - (radius + other.radius + 2);
            if gap < 0 {
                score += 50.0 - f64::from(gap);
            }
        }
        score
    };
    candidates
        .into_iter()
        .map(|site| (score(site), site))
        .min_by(|a, b| a.0.total_cmp(&b.0))
        .map(|(_, site)| site)
        .expect("candidates")
}

/// Every on-map hex in the footprint: a rectangle for planned layouts, a ragged disc for
/// grown ones.
fn footprint(
    map: &HexMap,
    site: (i32, i32),
    radius: i32,
    rectangular: bool,
    seed: u64,
) -> Vec<usize> {
    let ragged = Noise::new(seed, "footprint");
    let rows = ((f64::from(radius) * 0.87).round() as i32).max(1);
    let mut hexes = Vec::new();
    for y in site.1 - radius - 2..=site.1 + radius + 2 {
        for x in site.0 - radius - 2..=site.0 + radius + 2 {
            if !map.contains(x, y) {
                continue;
            }
            let inside = if rectangular {
                (x - site.0).abs() <= radius && (y - site.1).abs() <= rows
            } else {
                let BattlePoint { x: cx, y: cy } = BattleHexCoordinate { x, y }.center();
                let wobble = 2.0 * (ragged.value(cx / 2.0, cy / 2.0) - 0.5);
                f64::from(HexMap::distance(site, (x, y))) <= f64::from(radius) + wobble
            };
            if inside {
                hexes.push(map.index(x, y));
            }
        }
    }
    if hexes.is_empty() {
        hexes.push(map.index(site.0, site.1));
    }
    hexes
}

/// Footprint hexes with a neighbor outside the footprint or off the map.
fn edge_hexes(map: &HexMap, footprint: &[usize]) -> Vec<usize> {
    let inside = membership(map, footprint);
    footprint
        .iter()
        .copied()
        .filter(|&index| {
            let (x, y) = map.coordinate(index);
            map.adjacent(x, y)
                .iter()
                .any(|neighbor| neighbor.is_none_or(|(nx, ny)| !inside[map.index(nx, ny)]))
        })
        .collect()
}

/// A map-sized mask marking `hexes`.
fn membership(map: &HexMap, hexes: &[usize]) -> Vec<bool> {
    let mut inside = vec![false; map.hexes.len()];
    for &index in hexes {
        inside[index] = true;
    }
    inside
}

/// Level the footprint to its median dry ground and ease the three rings around it toward
/// that level, so the site does not sit in a pit or on a pillar.
fn level_site(map: &mut HexMap, footprint: &[usize]) {
    let mut levels: Vec<u8> = footprint
        .iter()
        .filter(|&&index| !map.hexes[index].terrain.is_water())
        .map(|&index| map.hexes[index].level)
        .collect();
    if levels.is_empty() {
        return;
    }
    levels.sort_unstable();
    let target = levels[levels.len() / 2];
    let mut ring = vec![u8::MAX; map.hexes.len()];
    let mut queue = VecDeque::new();
    for &index in footprint {
        ring[index] = 0;
        queue.push_back(index);
        let hex = &mut map.hexes[index];
        hex.level = if hex.terrain.is_water() {
            hex.level.min(target)
        } else {
            target
        };
    }
    while let Some(index) = queue.pop_front() {
        let distance = ring[index];
        if distance >= 3 {
            continue;
        }
        let (x, y) = map.coordinate(index);
        for (nx, ny) in map.neighbors(x, y) {
            let next = map.index(nx, ny);
            if ring[next] != u8::MAX {
                continue;
            }
            ring[next] = distance + 1;
            queue.push_back(next);
            let slack = 2 * (distance + 1);
            let hex = &mut map.hexes[next];
            if !hex.terrain.is_water() {
                hex.level = hex
                    .level
                    .clamp(target.saturating_sub(slack), target.saturating_add(slack));
            }
        }
    }
}

/// A building height that rises toward the settlement's center.
fn downtown_height(scale: &Scale, site: (i32, i32), hex: (i32, i32), rng: &mut Rng) -> u8 {
    let closeness =
        1.0 - (f64::from(HexMap::distance(site, hex)) / f64::from(scale.radius.max(1))).min(1.0);
    let span = f64::from(scale.tallest - scale.lowest);
    let height =
        f64::from(scale.lowest) + span * closeness.powf(1.5) + f64::from(rng.between(-1, 1));
    height
        .round()
        .clamp(f64::from(scale.lowest.max(1)), f64::from(scale.tallest)) as u8
}

/// A rectangular street grid with building lots between; industrial blocks are larger.
fn grid_plots(
    map: &HexMap,
    site: (i32, i32),
    scale: &Scale,
    kind: SettlementKind,
    footprint: &[usize],
    rng: &mut Rng,
) -> Vec<Plot> {
    let pitch = if kind == SettlementKind::Industrial {
        4
    } else {
        3
    };
    let parks = Noise::new(rng.next_u64(), "parks");
    footprint
        .iter()
        .map(|&index| {
            let (x, y) = map.coordinate(index);
            let (dx, dy) = (x - site.0, y - site.1);
            if dx.rem_euclid(pitch) == 0 || dy.rem_euclid(pitch) == 0 {
                return Plot::Street;
            }
            let block = (dx.div_euclid(pitch), dy.div_euclid(pitch));
            if scale.radius >= 8
                && parks.value(f64::from(block.0) * 7.3, f64::from(block.1) * 7.3) < 0.08
            {
                return Plot::Park;
            }
            if !rng.chance(scale.density) {
                return Plot::Open;
            }
            let height = if kind == SettlementKind::Industrial {
                rng.between(i32::from(scale.lowest), i32::from(scale.tallest)) as u8
            } else {
                downtown_height(scale, site, (x, y), rng)
            };
            Plot::Building(height)
        })
        .collect()
}

/// Winding streets branching out from a central square, with buildings fronting them.
fn organic_plots(
    map: &HexMap,
    site: (i32, i32),
    scale: &Scale,
    footprint: &[usize],
    rng: &mut Rng,
) -> Vec<Plot> {
    let inside = membership(map, footprint);
    let mut street = vec![false; map.hexes.len()];
    street[map.index(site.0, site.1)] = true;
    for (x, y) in map.neighbors(site.0, site.1) {
        street[map.index(x, y)] = inside[map.index(x, y)];
    }
    let branches = 3 + scale.radius as usize / 3;
    let mut walks: Vec<((i32, i32), usize, i32)> = (0..branches)
        .map(|branch| {
            (
                site,
                (branch * 6 / branches + rng.index(2)) % 6,
                scale.radius + 3,
            )
        })
        .collect();
    while let Some((mut here, mut direction, steps)) = walks.pop() {
        for _ in 0..steps {
            if !rng.chance(0.7) {
                direction = (direction + if rng.chance(0.5) { 1 } else { 5 }) % 6;
            }
            let Some(next) = map.adjacent(here.0, here.1)[direction] else {
                break;
            };
            if !inside[map.index(next.0, next.1)] {
                break;
            }
            here = next;
            street[map.index(here.0, here.1)] = true;
            if steps > 3 && rng.chance(0.12) {
                walks.push((here, (direction + 2) % 6, steps / 2));
            }
        }
    }
    footprint
        .iter()
        .map(|&index| {
            if street[index] {
                return Plot::Street;
            }
            let (x, y) = map.coordinate(index);
            let fronting = map
                .neighbors(x, y)
                .any(|(nx, ny)| street[map.index(nx, ny)]);
            let chance = if fronting {
                scale.density
            } else {
                scale.density * 0.25
            };
            if rng.chance(chance) {
                Plot::Building(downtown_height(scale, site, (x, y), rng))
            } else if rng.chance(0.15) {
                Plot::Park
            } else {
                Plot::Open
            }
        })
        .collect()
}

/// Buildings spread at least two hexes apart, sometimes in pairs, around a central crossroads.
fn scattered_plots(
    map: &HexMap,
    site: (i32, i32),
    scale: &Scale,
    footprint: &[usize],
    rng: &mut Rng,
) -> Vec<Plot> {
    let mut plots = vec![Plot::Open; footprint.len()];
    let site_index = map.index(site.0, site.1);
    if let Some(slot) = footprint.iter().position(|&index| index == site_index) {
        plots[slot] = Plot::Street;
    }
    let wanted = ((footprint.len() as f64 * scale.density * 0.18).round() as usize).max(2);
    let mut placed: Vec<(i32, i32)> = Vec::new();
    for _ in 0..wanted * 12 {
        if placed.len() >= wanted {
            break;
        }
        let slot = rng.index(footprint.len());
        let hex = map.coordinate(footprint[slot]);
        if plots[slot] != Plot::Open
            || map.hexes[footprint[slot]].terrain.is_water()
            || placed.iter().any(|&other| HexMap::distance(hex, other) < 2)
        {
            continue;
        }
        let height = rng.between(i32::from(scale.lowest), i32::from(scale.tallest)) as u8;
        plots[slot] = Plot::Building(height);
        placed.push(hex);
        if rng.chance(0.35)
            && let Some((nx, ny)) = map.adjacent(hex.0, hex.1)[rng.index(6)]
            && let Some(pair) = footprint.iter().position(|&i| i == map.index(nx, ny))
            && plots[pair] == Plot::Open
        {
            plots[pair] = Plot::Building(height);
            placed.push((nx, ny));
        }
    }
    plots
}

/// A rectangular compound: a central road, and in larger compounds a perimeter road, a landing
/// pad in the middle and rows of barracks and hangars between service roads.
fn compound_plots(
    map: &HexMap,
    site: (i32, i32),
    scale: &Scale,
    footprint: &[usize],
    rng: &mut Rng,
) -> Vec<Plot> {
    let edge = membership(map, &edge_hexes(map, footprint));
    // Small compounds have no room for a perimeter road, service rows or a landing pad.
    let roomy = scale.radius >= 4;
    let pad = if roomy { scale.radius / 3 } else { -1 };
    footprint
        .iter()
        .map(|&index| {
            let (x, y) = map.coordinate(index);
            let (dx, dy) = (x - site.0, y - site.1);
            if edge[index] {
                return if dx == 0 || dy == 0 {
                    Plot::Street
                } else {
                    Plot::Open
                };
            }
            let ring = map.neighbors(x, y).any(|(nx, ny)| edge[map.index(nx, ny)]);
            if dx == 0 || (roomy && (ring || dy.rem_euclid(3) == 0)) {
                return Plot::Street;
            }
            if HexMap::distance(site, (x, y)) <= pad {
                return Plot::Open;
            }
            if rng.chance(scale.density) {
                Plot::Building(rng.between(i32::from(scale.lowest), i32::from(scale.tallest)) as u8)
            } else {
                Plot::Open
            }
        })
        .collect()
}

/// Turn the footprint's edge into a wall of `height` with a gate on each side, where the edge
/// meets the settlement's main axes. Each gate is the edge street nearest that point, or the
/// nearest edge hex when no street reaches it. Returns the gates' footprint slots.
fn wall_in(
    map: &HexMap,
    site: (i32, i32),
    radius: i32,
    footprint: &[usize],
    edge: &[usize],
    plots: &mut [Plot],
    height: u8,
) -> Vec<usize> {
    let on_edge = membership(map, edge);
    let edge_slots: Vec<usize> = (0..footprint.len())
        .filter(|&slot| on_edge[footprint[slot]])
        .collect();
    let mut gates = Vec::new();
    for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
        let target = (site.0 + dx * radius, site.1 + dy * radius);
        let distance = |slot: usize| HexMap::distance(map.coordinate(footprint[slot]), target);
        let Some(nearest) = edge_slots
            .iter()
            .copied()
            .min_by_key(|&slot| distance(slot))
        else {
            continue;
        };
        let gate = edge_slots
            .iter()
            .copied()
            .filter(|&slot| plots[slot] == Plot::Street)
            .min_by_key(|&slot| distance(slot))
            .filter(|&slot| distance(slot) <= distance(nearest) + 2)
            .unwrap_or(nearest);
        if !gates.contains(&gate) {
            gates.push(gate);
        }
    }
    for &slot in &edge_slots {
        plots[slot] = if gates.contains(&slot) {
            Plot::Street
        } else {
            Plot::Wall(height)
        };
    }
    gates
}

/// Wreck a settlement: rubble where buildings and walls fell, stubs of the rest, and debris
/// in the streets. Plots marked in `keep` (the gates) stay passable.
fn ruin(plots: &mut [Plot], keep: &[usize], rng: &mut Rng) {
    for (slot, plot) in plots.iter_mut().enumerate() {
        if keep.contains(&slot) {
            continue;
        }
        *plot = match *plot {
            Plot::Building(_) if rng.chance(0.35) => Plot::Rubble,
            Plot::Building(height) => Plot::Building((height / 2).max(1)),
            Plot::Wall(_) if rng.chance(0.4) => Plot::Rubble,
            Plot::Street if rng.chance(0.15) => Plot::Rubble,
            Plot::Open if rng.chance(0.1) => Plot::Park,
            other => other,
        };
    }
}

/// Write the plots into the map, returning the number of buildings and the tallest. Streets
/// over water become bridges; buildings and walls are not built on water.
fn apply(
    map: &mut HexMap,
    footprint: &[usize],
    plots: &[Plot],
    params: &Params,
    kind: SettlementKind,
    rng: &mut Rng,
) -> (usize, u8) {
    let (mut buildings, mut tallest) = (0, 0);
    let open = open_ground(params.profile.ground);
    for (&index, &plot) in footprint.iter().zip(plots) {
        let hex = &mut map.hexes[index];
        if hex.terrain.is_water() {
            if plot == Plot::Street {
                hex.bridge = Some(1);
            }
            continue;
        }
        hex.terrain = match plot {
            Plot::Open => open,
            Plot::Park => {
                if rng.chance(0.6) {
                    Terrain::LightWoods
                } else {
                    open
                }
            }
            Plot::Street => Terrain::Road,
            Plot::Rubble => Terrain::Rough,
            Plot::Wall(height) => Terrain::Wall { height },
            Plot::Building(height) => {
                buildings += 1;
                tallest = tallest.max(height);
                if kind == SettlementKind::Ruins && rng.chance(0.15) {
                    hex.overlay = Some(BattleDecorationKind::Smoke);
                }
                Terrain::Building { height }
            }
        };
    }
    (buildings, tallest)
}
