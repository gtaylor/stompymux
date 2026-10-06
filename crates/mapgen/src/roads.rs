//! The road network: roads linking settlements and highways crossing the map, routed around
//! steep climbs, deep water and buildings, with bridges wherever they cross water.
//!
//! A road is a route laid through the hexes it crosses: woods, rough ground and fields stay
//! where they are, and units travelling along the road pay for the road instead. Roads are
//! kept clear of snow. Highways are paved; a road linking two settlements is gravel when
//! both are towns or larger, or one is a military base, and dirt otherwise. Where roads share
//! hexes the better surface wins. Bridges are built to suit the road: heavy for paved roads,
//! medium for gravel and light for dirt.
use crate::Params;
use crate::map::HexMap;
use crate::path::find_path;
use crate::report::RoadReport;
use crate::rng::Rng;
use crate::settlement::Settlement;
use crate::spec::{SettlementKind, SettlementSize};
use stompymux_map::{
    ConstructionClass, Foliage, Ground, HexCoordinate, Route, Structure, StructureKind,
};

/// Cheapest possible step cost, which keeps the routing heuristic admissible.
const ROAD_STEP: u32 = 3;

/// Price of stepping from `from` onto `to`, in tenths of an open-ground step, or `None` when a
/// road cannot go there. Existing roads and bridges are cheapest so routes share them.
fn step_cost(map: &HexMap, reserved: &[bool], from: (i32, i32), to: (i32, i32)) -> Option<u32> {
    let target = map.hex(to.0, to.1)?;
    let source = map.hex(from.0, from.1)?;
    if target.has_standing_structure() {
        return None;
    }
    let mut cost = if target.is_road() || target.has_bridge() {
        ROAD_STEP
    } else if let Some(water) = target.water() {
        50 + 15 * u32::from(water.depth)
    } else {
        let ground = match target.ground() {
            Ground::Rough | Ground::Rubble => 22,
            Ground::Swamp => 30,
            Ground::UltraRough | Ground::UltraRubble | Ground::Magma => 60,
            _ => 10,
        };
        let foliage = match target.foliage() {
            Some(Foliage::LightWoods) => 18,
            Some(Foliage::HeavyWoods | Foliage::LightJungle) => 30,
            Some(Foliage::UltraHeavyWoods | Foliage::HeavyJungle) => 45,
            Some(Foliage::UltraHeavyJungle) => 60,
            Some(Foliage::PlantedFields) | None => 10,
        };
        ground.max(foliage)
    };
    // A row is a connected east-west line, so a small charge for changing rows keeps
    // east-west roads straight instead of zigzagging between two equally short rows.
    if to.1 != from.1 {
        cost += 1;
    }
    let climb = u32::from(target.level().abs_diff(source.level()));
    cost += 12 * climb + if climb > 2 { 80 } else { 0 };
    if reserved[map.index(to.0, to.1)] && !target.is_road() {
        cost += 25;
    }
    Some(cost)
}

/// The construction class of a bridge carrying a road of `surface`.
const fn bridge_class(surface: Route) -> ConstructionClass {
    match surface {
        Route::PavedRoad => ConstructionClass::Heavy,
        Route::GravelRoad | Route::Rail => ConstructionClass::Medium,
        Route::DirtRoad => ConstructionClass::Light,
    }
}

/// The surface of a road linking settlements `a` and `b`.
fn link_surface(a: &Settlement, b: &Settlement) -> Route {
    let military = |settlement: &Settlement| settlement.report.kind == SettlementKind::Military;
    if military(a) || military(b) || a.report.size.min(b.report.size) >= SettlementSize::Town {
        Route::GravelRoad
    } else {
        Route::DirtRoad
    }
}

/// Lay a road of `surface` along `path`, bridging water, and widen it to `width` hexes on the
/// side `across` points to. Returns the bridge hexes added.
fn pave(
    map: &mut HexMap,
    path: &[(i32, i32)],
    width: u8,
    across: (i32, i32),
    surface: Route,
) -> usize {
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
            if hex.has_standing_structure() {
                continue;
            }
            if hex.holds_water() {
                if !hex.has_bridge() {
                    let bridge = Structure::new(StructureKind::Bridge, 1, bridge_class(surface));
                    *hex = hex.with_structure(Some(bridge));
                    bridges += 1;
                }
                continue;
            }
            // Route variants run from the best surface to the worst.
            let route = hex
                .route()
                .filter(|route| route.is_road())
                .map_or(surface, |existing| existing.min(surface));
            *hex = hex.with_route(Some(route)).with_condition(None);
        }
    }
    bridges
}

/// The lane offset that widens a road heading from `from` to `to`: down a column for a road
/// running east-west, across to the next column for one running north-south.
fn widening(from: (i32, i32), to: (i32, i32)) -> (i32, i32) {
    let from = HexCoordinate {
        x: from.0,
        y: from.1,
    }
    .center();
    let to = HexCoordinate { x: to.0, y: to.1 }.center();
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
            let surface = link_surface(from, to);
            let bridges = pave(
                map,
                &path,
                params.road_width,
                widening(start, goal),
                surface,
            );
            roads.push(RoadReport {
                from: from.name.clone(),
                to: to.name.clone(),
                hexes: path.len(),
                bridges,
                surface,
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
                .is_some_and(|hex| !hex.has_standing_structure() && !hex.holds_water())
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
        let bridges = pave(
            map,
            &path,
            params.road_width,
            widening(start, end),
            Route::PavedRoad,
        );
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
            surface: Route::PavedRoad,
        });
    }
    None
}
