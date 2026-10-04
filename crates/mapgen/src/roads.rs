//! The road network: roads linking settlements and highways crossing the map, routed around
//! steep climbs, deep water and buildings, with bridges wherever they cross water.
use crate::Params;
use crate::map::{HexMap, Terrain};
use crate::path::find_path;
use crate::report::RoadReport;
use crate::rng::Rng;
use crate::settlement::Settlement;
use stompymux_map::BattleHexCoordinate;

/// Cheapest possible step cost, which keeps the routing heuristic admissible.
const ROAD_STEP: u32 = 3;

/// Price of stepping from `from` onto `to`, in tenths of an open-ground step, or `None` when a
/// road cannot go there. Existing roads and bridges are cheapest so routes share them.
fn step_cost(map: &HexMap, reserved: &[bool], from: (i32, i32), to: (i32, i32)) -> Option<u32> {
    let target = map.hex(to.0, to.1)?;
    let source = map.hex(from.0, from.1)?;
    let mut cost = match target.terrain {
        Terrain::Building { .. } | Terrain::Wall { .. } => return None,
        Terrain::Road => ROAD_STEP,
        _ if target.bridge.is_some() => ROAD_STEP,
        Terrain::Clear | Terrain::Sand | Terrain::Snow => 10,
        Terrain::LightWoods => 18,
        Terrain::Rough => 22,
        Terrain::HeavyWoods => 30,
        Terrain::Mountains => 60,
        Terrain::Water { depth } | Terrain::Ice { depth } => 50 + 15 * u32::from(depth),
    };
    // A row is a connected east-west line, so a small charge for changing rows keeps
    // east-west roads straight instead of zigzagging between two equally short rows.
    if to.1 != from.1 {
        cost += 1;
    }
    let climb = u32::from(target.level.abs_diff(source.level));
    cost += 12 * climb + if climb > 2 { 80 } else { 0 };
    if reserved[map.index(to.0, to.1)] && target.terrain != Terrain::Road {
        cost += 25;
    }
    Some(cost)
}

/// Pave `path`, bridging water, and widen it to `width` hexes on the side `across` points to.
fn pave(map: &mut HexMap, path: &[(i32, i32)], width: u8, across: (i32, i32)) -> usize {
    let mut bridges = 0;
    for &(x, y) in path {
        for lane in 0..i32::from(width) {
            // Lanes alternate sides: the path itself, then one side, then the other.
            let offset = match lane {
                0 => 0,
                1 => 1,
                _ => -1,
            };
            let Some(hex) = map.hex_mut(x + across.0 * offset, y + across.1 * offset) else {
                continue;
            };
            if hex.terrain.is_structure() {
                continue;
            }
            if hex.terrain.is_water() {
                if hex.bridge.is_none() {
                    hex.bridge = Some(1);
                    bridges += 1;
                }
                continue;
            }
            hex.terrain = Terrain::Road;
        }
    }
    bridges
}

/// The lane offset that widens a road heading from `from` to `to`: down a column for a road
/// running east-west, across to the next column for one running north-south.
fn widening(from: (i32, i32), to: (i32, i32)) -> (i32, i32) {
    let from = BattleHexCoordinate {
        x: from.0,
        y: from.1,
    }
    .center();
    let to = BattleHexCoordinate { x: to.0, y: to.1 }.center();
    if (to.x - from.x).abs() >= (to.y - from.y).abs() {
        (0, 1)
    } else {
        (1, 0)
    }
}

/// Build every road the spec asks for, returning what was built.
pub(crate) fn build(
    map: &mut HexMap,
    settlements: &[Settlement],
    params: &Params,
    warnings: &mut Vec<String>,
) -> Vec<RoadReport> {
    let mut reserved = vec![false; map.hexes.len()];
    for settlement in settlements {
        for &index in &settlement.footprint {
            reserved[index] = true;
        }
    }
    let mut roads = Vec::new();
    if params.connect_settlements {
        for (a, b) in spanning_links(settlements) {
            let (from, to) = (&settlements[a], &settlements[b]);
            let start = nearest(&from.gates, to.center);
            let goal = nearest(&to.gates, start);
            let path = find_path(
                map,
                start,
                |hex| hex == goal,
                |hex| HexMap::distance(hex, goal) as u32 * ROAD_STEP,
                |here, next| step_cost(map, &reserved, here, next),
            );
            let Some(path) = path else {
                warnings.push(format!(
                    "no road could reach {} from {}",
                    to.name, from.name
                ));
                continue;
            };
            let bridges = pave(map, &path, params.road_width, widening(start, goal));
            roads.push(RoadReport {
                from: from.name.clone(),
                to: to.name.clone(),
                hexes: path.len(),
                bridges,
            });
        }
    }
    let mut rng = Rng::stream(params.seed, "roads");
    for highway in 0..params.through_roads {
        if let Some(road) = through_road(map, &reserved, params, highway, &mut rng) {
            roads.push(road);
        } else {
            warnings.push(format!(
                "through road {} could not cross the map",
                highway + 1
            ));
        }
    }
    roads
}

/// The gate in `gates` nearest `target`.
fn nearest(gates: &[(i32, i32)], target: (i32, i32)) -> (i32, i32) {
    *gates
        .iter()
        .min_by_key(|&&gate| HexMap::distance(gate, target))
        .expect("every settlement has a gate")
}

/// The shortest set of links joining every settlement (a minimum spanning tree by distance).
fn spanning_links(settlements: &[Settlement]) -> Vec<(usize, usize)> {
    if settlements.len() < 2 {
        return Vec::new();
    }
    let mut joined = vec![false; settlements.len()];
    joined[0] = true;
    let mut links = Vec::new();
    while links.len() < settlements.len() - 1 {
        let link = (0..settlements.len())
            .filter(|&a| joined[a])
            .flat_map(|a| {
                (0..settlements.len())
                    .filter(|&b| !joined[b])
                    .map(move |b| (a, b))
            })
            .min_by_key(|&(a, b)| HexMap::distance(settlements[a].center, settlements[b].center))
            .expect("an unjoined settlement remains");
        joined[link.1] = true;
        links.push(link);
    }
    links
}

/// A highway from one map edge to the opposite edge. Even-numbered highways prefer to run
/// west-east and odd ones north-south; each tries both directions, then the other axis, until
/// it finds dry land in the middle of both edges to start from and arrive on.
fn through_road(
    map: &mut HexMap,
    reserved: &[bool],
    params: &Params,
    number: u8,
    rng: &mut Rng,
) -> Option<RoadReport> {
    let (width, height) = (i32::from(map.width), i32::from(map.height));
    let preferred = number.is_multiple_of(2);
    for (east_west, reversed) in [
        (preferred, false),
        (preferred, true),
        (!preferred, false),
        (!preferred, true),
    ] {
        let (length, far) = if east_west {
            (height, width - 1)
        } else {
            (width, height - 1)
        };
        let (from_line, to_line) = if reversed { (far, 0) } else { (0, far) };
        let at = |along: i32, line: i32| {
            if east_west {
                (line, along)
            } else {
                (along, line)
            }
        };
        let dry = |(x, y): (i32, i32)| {
            map.hex(x, y)
                .is_some_and(|hex| !hex.terrain.is_structure() && !hex.terrain.is_water())
        };
        let middle = length / 6..length - length / 6;
        if !middle.clone().any(|along| dry(at(along, to_line))) {
            continue;
        }
        let Some(start) = (0..16)
            .map(|_| at(rng.between(middle.start, middle.end - 1), from_line))
            .find(|&hex| dry(hex))
        else {
            continue;
        };
        let line = |(x, y): (i32, i32)| if east_west { x } else { y };
        let Some(path) = find_path(
            map,
            start,
            |hex| line(hex) == to_line && dry(hex),
            |hex| (line(hex) - to_line).unsigned_abs() * ROAD_STEP,
            |here, next| step_cost(map, reserved, here, next),
        ) else {
            continue;
        };
        let end = *path.last().expect("path has a start");
        let bridges = pave(map, &path, params.road_width, widening(start, end));
        let edge = |line: i32| match (east_west, line == 0) {
            (true, true) => "west edge",
            (true, false) => "east edge",
            (false, true) => "north edge",
            (false, false) => "south edge",
        };
        return Some(RoadReport {
            from: edge(from_line).to_owned(),
            to: edge(to_line).to_owned(),
            hexes: path.len(),
            bridges,
        });
    }
    None
}
