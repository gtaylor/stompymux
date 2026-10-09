//! Script annotations for generated maps: each settlement gets a point of interest at its
//! center and a region covering its footprint, both of type [`SETTLEMENT_TYPE`] and named
//! after the settlement.
//!
//! A region is outlined by corner hexes (see [`MapRegion`]), so the footprint's outline is
//! traced hex by hex around its edge, clockwise, and then thinned to the corners that matter:
//! a corner is dropped whenever the region holds exactly the same hexes without it. A
//! footprint is connected and has no holes, so the outline through the centers of its edge
//! hexes holds exactly the footprint.
use crate::map::HexMap;
use crate::settlement::Settlement;
use std::collections::HashMap;
use stompymux_map::{HexCoordinate, MapPointOfInterest, MapRegion};

/// The type of the points of interest and regions generated for settlements.
pub const SETTLEMENT_TYPE: &str = "settlement";

/// Add a point of interest and a region for each settlement to `map`. A footprint whose
/// region cannot match it exactly, which a connected footprint without holes never is, gets
/// the closest region the tracing finds, with a warning.
pub(crate) fn annotate(map: &mut HexMap, settlements: &[Settlement], warnings: &mut Vec<String>) {
    for settlement in settlements {
        let [x, y] = settlement.report.center;
        map.points_of_interest.push(MapPointOfInterest {
            kind: SETTLEMENT_TYPE.into(),
            name: settlement.name.clone(),
            x,
            y,
            elevation: None,
        });
        let footprint: Vec<(i32, i32)> = settlement
            .footprint
            .iter()
            .map(|&index| map.coordinate(index))
            .collect();
        let region = MapRegion {
            kind: SETTLEMENT_TYPE.into(),
            name: settlement.name.clone(),
            corners: outline(map, &footprint),
        };
        if region.hexes().ok() != Some(sorted(&footprint)) {
            warnings.push(format!(
                "the {} region does not match the settlement's footprint exactly",
                settlement.name
            ));
        }
        map.regions.push(region);
    }
}

/// The corners of a region holding exactly the connected, hole-free `footprint` of hexes on
/// `map`, or as close as a single outline gets otherwise.
pub(crate) fn outline(map: &HexMap, footprint: &[(i32, i32)]) -> Vec<[u16; 2]> {
    let corners: Vec<[u16; 2]> = trace(map, footprint)
        .into_iter()
        .map(|(x, y)| [x as u16, y as u16])
        .collect();
    thin(corners)
}

/// The footprint's edge hexes in clockwise order around it, starting from its topmost hex.
///
/// This follows the edge the way a hand on a wall does: from each hex, the neighbors are
/// checked clockwise starting just past one known to be outside, and the walk steps to the
/// first one inside. Stepping in direction `d` leaves the last outside neighbor checked in
/// direction `d + 4` from the new hex, where the next check starts. The walk ends when it
/// returns to a hex it has already left in the same way, so a hex on a one-hex-wide neck
/// appears once for each side.
fn trace(map: &HexMap, footprint: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let mut inside = vec![false; map.hexes.len()];
    for &(x, y) in footprint {
        inside[map.index(x, y)] = true;
    }
    let is_inside = |hex: Option<(i32, i32)>| hex.is_some_and(|(x, y)| inside[map.index(x, y)]);
    // The hex whose center is highest, which has nothing inside to its north.
    let Some(&start) = footprint.iter().min_by(|a, b| {
        let (a, b) = (
            HexCoordinate { x: a.0, y: a.1 }.center(),
            HexCoordinate { x: b.0, y: b.1 }.center(),
        );
        a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x))
    }) else {
        return Vec::new();
    };
    let mut seen: HashMap<((i32, i32), usize), usize> = HashMap::new();
    let mut walk = Vec::new();
    let (mut hex, mut back) = (start, 0);
    loop {
        if let Some(&first) = seen.get(&(hex, back)) {
            return walk.split_off(first);
        }
        seen.insert((hex, back), walk.len());
        walk.push(hex);
        let neighbors = map.adjacent(hex.0, hex.1);
        let Some(step) = (1..=6)
            .map(|turn| (back + turn) % 6)
            .find(|&direction| is_inside(neighbors[direction]))
        else {
            // A footprint of one hex.
            return walk;
        };
        hex = neighbors[step].expect("inside hexes are on the map");
        back = (step + 4) % 6;
    }
}

/// `corners` without every corner whose removal leaves the region holding the same hexes.
fn thin(mut corners: Vec<[u16; 2]>) -> Vec<[u16; 2]> {
    let hexes = |corners: &[[u16; 2]]| {
        MapRegion {
            kind: SETTLEMENT_TYPE.into(),
            name: SETTLEMENT_TYPE.into(),
            corners: corners.to_vec(),
        }
        .hexes()
        .ok()
    };
    let target = hexes(&corners);
    loop {
        let before = corners.len();
        let mut index = 0;
        while index < corners.len() && corners.len() > 1 {
            let mut without = corners.clone();
            without.remove(index);
            if hexes(&without) == target {
                corners = without;
            } else {
                index += 1;
            }
        }
        if corners.len() == before {
            return corners;
        }
    }
}

/// Hex coordinates of `(x, y)` pairs in row-major order, as [`MapRegion::hexes`] gives them.
fn sorted(hexes: &[(i32, i32)]) -> Vec<HexCoordinate> {
    let mut sorted: Vec<HexCoordinate> =
        hexes.iter().map(|&(x, y)| HexCoordinate { x, y }).collect();
    sorted.sort_by_key(|hex| (hex.y, hex.x));
    sorted.dedup();
    sorted
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The region the corners describe.
    fn region_hexes(corners: Vec<[u16; 2]>) -> Vec<HexCoordinate> {
        MapRegion {
            kind: SETTLEMENT_TYPE.into(),
            name: "test".into(),
            corners,
        }
        .hexes()
        .unwrap()
    }

    /// Every hex within `radius` steps of `center` on `map`.
    fn disc(map: &HexMap, center: (i32, i32), radius: i32) -> Vec<(i32, i32)> {
        let mut hexes = Vec::new();
        for y in 0..i32::from(map.height) {
            for x in 0..i32::from(map.width) {
                if HexMap::distance(center, (x, y)) <= radius {
                    hexes.push((x, y));
                }
            }
        }
        hexes
    }

    /// Outlines of discs, rectangles, single hexes, lines and ragged shapes, whole or clipped
    /// by the map's edges, hold exactly their footprints with few corners.
    #[test]
    fn outlines_hold_exactly_their_footprints() {
        let map = HexMap::new(24, 20);
        let rectangle: Vec<_> = (3..=14)
            .flat_map(|y| (4..=19).map(move |x| (x, y)))
            .collect();
        let notched: Vec<_> = disc(&map, (12, 10), 6)
            .into_iter()
            .filter(|&(x, y)| !(x > 12 && (9..=11).contains(&y)))
            .collect();
        let neck: Vec<_> = disc(&map, (5, 5), 2)
            .into_iter()
            .chain([(8, 5), (9, 5)])
            .chain(disc(&map, (12, 5), 2))
            .collect();
        for (name, footprint) in [
            ("disc", disc(&map, (12, 10), 5)),
            ("clipped disc", disc(&map, (1, 2), 6)),
            ("rectangle", rectangle),
            ("whole map", disc(&map, (12, 10), 40)),
            ("single hex", vec![(7, 7)]),
            ("pair", vec![(7, 7), (7, 8)]),
            ("notched disc", notched),
            ("neck", neck),
        ] {
            let corners = outline(&map, &footprint);
            assert_eq!(
                region_hexes(corners.clone()),
                sorted(&footprint),
                "{name}: {corners:?}"
            );
            assert!(corners.len() <= 24, "{name} kept {} corners", corners.len());
        }
    }

    /// Every settlement of a generated map has a point of interest at its center and a region
    /// holding exactly its footprint, both named after it.
    #[test]
    fn settlements_get_points_and_regions() {
        let generated = crate::generate(&crate::MapSpec {
            seed: Some(11),
            biome: Some(crate::Biome::Temperate),
            size: Some(crate::MapSize::Medium),
            settlements: [
                crate::SettlementSize::Town,
                crate::SettlementSize::Village,
                crate::SettlementSize::Hamlet,
            ]
            .into_iter()
            .map(crate::SettlementSpec::new)
            .collect(),
            ..crate::MapSpec::default()
        })
        .unwrap();
        let map = &generated.map;
        let settlements = &generated.report.settlements;
        assert_eq!(map.points_of_interest.len(), settlements.len());
        assert_eq!(map.regions.len(), settlements.len());
        for ((settlement, point), region) in settlements
            .iter()
            .zip(&map.points_of_interest)
            .zip(&map.regions)
        {
            assert_eq!(
                (point.kind.as_str(), point.name.as_str()),
                (SETTLEMENT_TYPE, settlement.name.as_str())
            );
            assert_eq!([point.x, point.y], settlement.center);
            assert_eq!(
                (region.kind.as_str(), region.name.as_str()),
                (SETTLEMENT_TYPE, settlement.name.as_str())
            );
            let [x, y] = settlement.center;
            let center = HexCoordinate {
                x: i32::from(x),
                y: i32::from(y),
            };
            assert!(region.contains(center).unwrap(), "{}", settlement.name);
        }
        assert!(
            !generated
                .report
                .warnings
                .iter()
                .any(|warning| warning.contains("footprint exactly")),
            "{:?}",
            generated.report.warnings
        );
        let asset = map.to_asset().unwrap();
        assert_eq!(asset.points_of_interest, map.points_of_interest);
        assert_eq!(asset.regions, map.regions);
        let file = generated.to_toml().unwrap();
        let reloaded = stompymux_map::MapAsset::parse(&file).unwrap();
        assert_eq!(reloaded.regions, map.regions);
    }
}
