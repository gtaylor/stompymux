//! Least-cost paths across the hex grid, used to trace rivers down valleys and to route roads
//! around obstacles.
use crate::map::HexMap;
use std::{cmp::Reverse, collections::BinaryHeap};

/// The cheapest path from `start` to any hex where `is_goal` holds, or `None` if every route
/// is blocked. `cost(from, to)` prices one step, with `None` for impassable hexes, and
/// `heuristic` must never overestimate the remaining cost.
pub(crate) fn find_path(
    map: &HexMap,
    start: (i32, i32),
    is_goal: impl Fn((i32, i32)) -> bool,
    heuristic: impl Fn((i32, i32)) -> u32,
    cost: impl Fn((i32, i32), (i32, i32)) -> Option<u32>,
) -> Option<Vec<(i32, i32)>> {
    if !map.contains(start.0, start.1) {
        return None;
    }
    let mut best = vec![u32::MAX; map.hexes.len()];
    let mut came_from = vec![usize::MAX; map.hexes.len()];
    let mut open = BinaryHeap::new();
    let start_index = map.index(start.0, start.1);
    best[start_index] = 0;
    open.push(Reverse((heuristic(start), 0_u32, start_index)));
    while let Some(Reverse((_, spent, index))) = open.pop() {
        if spent > best[index] {
            continue;
        }
        let here = map.coordinate(index);
        if is_goal(here) {
            let mut path = vec![here];
            let mut step = index;
            while came_from[step] != usize::MAX {
                step = came_from[step];
                path.push(map.coordinate(step));
            }
            path.reverse();
            return Some(path);
        }
        for next in map.neighbors(here.0, here.1) {
            let Some(step_cost) = cost(here, next) else {
                continue;
            };
            let next_index = map.index(next.0, next.1);
            let total = spent.saturating_add(step_cost);
            if total >= best[next_index] {
                continue;
            }
            best[next_index] = total;
            came_from[next_index] = index;
            open.push(Reverse((
                total.saturating_add(heuristic(next)),
                total,
                next_index,
            )));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use stompymux_map::{Hex, Structure};

    /// A wall two levels tall.
    const WALL: Hex = Hex::at_level(0).with_structure(Some(Structure::wall(2)));

    #[test]
    fn routes_around_walls() {
        let mut map = HexMap::new(9, 9);
        for y in 0..8 {
            *map.hex_mut(4, y).unwrap() = WALL;
        }
        let goal = (8, 0);
        let path = find_path(
            &map,
            (0, 0),
            |hex| hex == goal,
            |hex| HexMap::distance(hex, goal) as u32,
            |_, to| (!map.hex(to.0, to.1)?.has_standing_structure()).then_some(1),
        )
        .unwrap();
        assert_eq!((path[0], *path.last().unwrap()), ((0, 0), goal));
        assert!(path.contains(&(4, 8)));
        for pair in path.windows(2) {
            assert_eq!(HexMap::distance(pair[0], pair[1]), 1);
        }
    }

    #[test]
    fn reports_blocked_routes() {
        let mut map = HexMap::new(5, 5);
        for y in 0..5 {
            *map.hex_mut(2, y).unwrap() = WALL;
        }
        let blocked = find_path(
            &map,
            (0, 0),
            |hex| hex == (4, 4),
            |_| 0,
            |_, to| (!map.hex(to.0, to.1)?.has_standing_structure()).then_some(1),
        );
        assert!(blocked.is_none());
    }
}
