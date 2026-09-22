//! Deterministic, resumable A* search over a bounded hexagonal map.
//!
//! The search is deliberately independent of the battle model.  Callers provide
//! the map dimensions and a traversal-cost callback (or use [`HexGrid`]); later
//! layers can therefore apply unit mobility, terrain, occupancy, and hazard
//! rules without making the planner depend on those rules.  A search can be
//! advanced in small budgets and only stores discovered records, which keeps it
//! suitable for a heartbeat that serves many autonomous units.

use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};
use std::error::Error;
use std::fmt;

/// The maximum number of records used by a search when a caller does not supply
/// a tighter per-search limit.
pub const DEFAULT_MAX_RECORDS: usize = 1_000_000;

/// A zero-based column-staggered hex coordinate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Hex {
    /// The map column.
    pub x: u16,
    /// The map row within the column.
    pub y: u16,
}

impl Hex {
    /// Create a coordinate without checking whether it belongs to a map.
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }

    /// Return the six neighboring coordinates in clockwise order from north.
    ///
    /// The returned coordinates may be outside the map.  [`AStarSearch`] filters
    /// them using its dimensions before asking the traversal provider about a
    /// transition.
    pub fn neighbors(self) -> [Self; 6] {
        // Convert the column-staggered coordinates to cube coordinates before
        // adding directions.  This matches the geometry used by the battle map
        // while keeping this module independent of the battle model.
        let q = i64::from(self.x);
        let r = i64::from(self.y) - (q + q.rem_euclid(2)) / 2;
        let directions = [(0_i64, -1_i64), (1, -1), (1, 0), (0, 1), (-1, 1), (-1, 0)];

        directions.map(|(dq, dr)| {
            let q = q + dq;
            let row = r + dr + (q + q.rem_euclid(2)) / 2;
            // The source coordinate is u16, so every in-range neighbor fits in
            // i32.  Keep a signed intermediate for the two off-map directions.
            let x = u16::try_from(q).unwrap_or(u16::MAX);
            let y = u16::try_from(row).unwrap_or(u16::MAX);
            Self { x, y }
        })
    }

    /// Return the shortest number of hex transitions between two coordinates.
    pub fn distance(self, other: Self) -> u32 {
        let a = self.cube();
        let b = other.cube();
        a.into_iter()
            .zip(b)
            .map(|(left, right)| left.abs_diff(right))
            .max()
            .map(|distance| u32::try_from(distance).expect("u16 hex distance fits in u32"))
            .unwrap_or(0)
    }

    fn cube(self) -> [i64; 3] {
        let q = i64::from(self.x);
        let r = i64::from(self.y) - (q + q.rem_euclid(2)) / 2;
        [q, r, -q - r]
    }
}

/// A goal region.  Reaching any hex whose distance from `center` is at most
/// `radius` completes the search.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Goal {
    /// Center of the goal region.
    pub center: Hex,
    /// Inclusive radius in hex transitions.
    pub radius: u32,
}

impl Goal {
    /// Construct a goal region around one map hex.
    pub const fn new(center: Hex, radius: u32) -> Self {
        Self { center, radius }
    }

    fn contains(self, position: Hex) -> bool {
        position.distance(self.center) <= self.radius
    }
}

/// A positive traversal cost for an adjacent transition.
pub trait Traversal {
    /// Return the cost of entering `to` from `from`, or `None` when the
    /// transition is unavailable.  Costs must be positive; a zero cost is
    /// treated as unavailable by the planner.
    fn traversal_cost(&self, from: Hex, to: Hex) -> Option<u32>;
}

/// Allow a plain closure to provide unit-specific traversal rules.
impl<F> Traversal for F
where
    F: Fn(Hex, Hex) -> Option<u32>,
{
    fn traversal_cost(&self, from: Hex, to: Hex) -> Option<u32> {
        self(from, to)
    }
}

/// A dense destination-cost grid useful for simple maps and tests.
///
/// Each cell stores the cost of entering it.  `None` blocks entry.  The source
/// coordinate is accepted by the implementation so callers can later replace
/// this grid with a provider that applies directional or unit-specific rules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HexGrid {
    width: u16,
    height: u16,
    costs: Vec<Option<u32>>,
}

impl HexGrid {
    /// Build a grid from row-major destination costs.
    pub fn from_costs(
        width: u16,
        height: u16,
        costs: Vec<Option<u32>>,
    ) -> Result<Self, NavigationError> {
        if width == 0 || height == 0 {
            return Err(NavigationError::EmptyMap);
        }
        let expected = usize::from(width) * usize::from(height);
        if costs.len() != expected {
            return Err(NavigationError::GridSizeMismatch {
                expected,
                actual: costs.len(),
            });
        }
        if costs.iter().flatten().any(|cost| *cost == 0) {
            return Err(NavigationError::NonPositiveTraversalCost);
        }
        Ok(Self {
            width,
            height,
            costs,
        })
    }

    /// Build a fully traversable grid with one entry cost for every cell.
    pub fn uniform(width: u16, height: u16, cost: u32) -> Result<Self, NavigationError> {
        if cost == 0 {
            return Err(NavigationError::NonPositiveTraversalCost);
        }
        let cells = usize::from(width)
            .checked_mul(usize::from(height))
            .ok_or(NavigationError::EmptyMap)?;
        Self::from_costs(width, height, vec![Some(cost); cells])
    }

    /// Return this grid's dimensions.
    pub const fn dimensions(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    /// Change the destination cost for one cell.
    pub fn set_cost(&mut self, position: Hex, cost: Option<u32>) -> Result<(), NavigationError> {
        if cost == Some(0) {
            return Err(NavigationError::NonPositiveTraversalCost);
        }
        let index = self
            .index(position)
            .ok_or(NavigationError::OutOfBounds(position))?;
        self.costs[index] = cost;
        Ok(())
    }

    fn index(&self, position: Hex) -> Option<usize> {
        (position.x < self.width && position.y < self.height)
            .then(|| usize::from(position.y) * usize::from(self.width) + usize::from(position.x))
    }
}

impl Traversal for HexGrid {
    fn traversal_cost(&self, _from: Hex, to: Hex) -> Option<u32> {
        self.index(to).and_then(|index| self.costs[index])
    }
}

/// Errors raised while constructing a search or dense grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NavigationError {
    /// A map dimension was zero.
    EmptyMap,
    /// A start or goal coordinate was outside the supplied dimensions.
    OutOfBounds(Hex),
    /// A search cannot run without at least one record slot.
    ZeroRecordLimit,
    /// A dense grid did not contain exactly one entry for every cell.
    GridSizeMismatch { expected: usize, actual: usize },
    /// Traversal costs must be positive integers.
    NonPositiveTraversalCost,
}

impl fmt::Display for NavigationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyMap => formatter.write_str("navigation map dimensions must be non-zero"),
            Self::OutOfBounds(position) => {
                write!(
                    formatter,
                    "hex ({}, {}) is outside the navigation map",
                    position.x, position.y
                )
            }
            Self::ZeroRecordLimit => {
                formatter.write_str("navigation record limit must be positive")
            }
            Self::GridSizeMismatch { expected, actual } => write!(
                formatter,
                "navigation grid contains {actual} cells, expected {expected}"
            ),
            Self::NonPositiveTraversalCost => {
                formatter.write_str("navigation traversal costs must be positive")
            }
        }
    }
}

impl Error for NavigationError {}

/// A completed route, including the cost accumulated by the traversal
/// provider.  Cells are ordered from the start through the reached goal cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationPath {
    /// Ordered route cells, including both endpoints.
    pub cells: Vec<Hex>,
    /// Sum of the provider's edge costs along the route.
    pub cost: u64,
}

/// The result of one bounded search step.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SearchStatus {
    /// More work remains.  `expanded` is the number of records expanded by this
    /// call, while `records` is the number discovered by the whole search.
    Pending {
        expanded: usize,
        total_expanded: u64,
        records: usize,
    },
    /// A route reached the goal region.
    Found { path: NavigationPath },
    /// The open set was exhausted without reaching the goal region.
    Unreachable,
    /// A caller invalidated the search after its terrain, mobility, or goal changed.
    Invalidated,
    /// The search needed a record beyond its configured cap.
    ResourceLimit { records: usize, limit: usize },
}

impl SearchStatus {
    /// Whether this result finishes the search.
    pub const fn is_terminal(&self) -> bool {
        !matches!(self, Self::Pending { .. })
    }
}

#[derive(Debug, Clone, Copy)]
struct Record {
    cost: u64,
    parent: Option<Hex>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct OpenEntry {
    estimated_total: u64,
    heuristic: u32,
    cost: u64,
    position: Hex,
}

impl Ord for OpenEntry {
    fn cmp(&self, other: &Self) -> Ordering {
        // BinaryHeap is a max-heap.  Reverse every priority field so the
        // smallest estimated route cost, then the smallest coordinate, wins.
        other
            .estimated_total
            .cmp(&self.estimated_total)
            .then_with(|| other.heuristic.cmp(&self.heuristic))
            .then_with(|| other.cost.cmp(&self.cost))
            .then_with(|| other.position.cmp(&self.position))
    }
}

impl PartialOrd for OpenEntry {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

/// A resumable A* search over a bounded column-staggered hex map.
#[derive(Debug, Clone)]
pub struct AStarSearch {
    width: u16,
    height: u16,
    start: Hex,
    goal: Goal,
    max_records: usize,
    records: HashMap<Hex, Record>,
    open: BinaryHeap<OpenEntry>,
    total_expanded: u64,
    terminal: Option<SearchStatus>,
}

impl AStarSearch {
    /// Create a search using [`DEFAULT_MAX_RECORDS`].
    pub fn new(width: u16, height: u16, start: Hex, goal: Goal) -> Result<Self, NavigationError> {
        Self::with_record_limit(width, height, start, goal, DEFAULT_MAX_RECORDS)
    }

    /// Create a search with an explicit cap on discovered records.
    pub fn with_record_limit(
        width: u16,
        height: u16,
        start: Hex,
        goal: Goal,
        max_records: usize,
    ) -> Result<Self, NavigationError> {
        if width == 0 || height == 0 {
            return Err(NavigationError::EmptyMap);
        }
        if max_records == 0 {
            return Err(NavigationError::ZeroRecordLimit);
        }
        if !Self::contains(width, height, start) {
            return Err(NavigationError::OutOfBounds(start));
        }
        if !Self::contains(width, height, goal.center) {
            return Err(NavigationError::OutOfBounds(goal.center));
        }

        let heuristic = start.distance(goal.center).saturating_sub(goal.radius);
        let mut records = HashMap::with_capacity(1.min(max_records));
        records.insert(
            start,
            Record {
                cost: 0,
                parent: None,
            },
        );
        let mut open = BinaryHeap::new();
        open.push(OpenEntry {
            estimated_total: u64::from(heuristic),
            heuristic,
            cost: 0,
            position: start,
        });

        Ok(Self {
            width,
            height,
            start,
            goal,
            max_records,
            records,
            open,
            total_expanded: 0,
            terminal: None,
        })
    }

    /// Return the dimensions used by the search.
    pub const fn dimensions(&self) -> (u16, u16) {
        (self.width, self.height)
    }

    /// Return the search's start coordinate.
    pub const fn start(&self) -> Hex {
        self.start
    }

    /// Return the configured goal region.
    pub const fn goal(&self) -> Goal {
        self.goal
    }

    /// Return the number of discovered records, including the start record.
    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    /// Return the cumulative number of expanded nodes across all search steps.
    pub const fn total_expanded(&self) -> u64 {
        self.total_expanded
    }

    /// Return the configured discovered-record cap.
    pub const fn record_limit(&self) -> usize {
        self.max_records
    }

    /// Clamp future discoveries to a shared runtime budget. A caller must
    /// defer this search when no record slots remain; shrinking below current
    /// usage would incorrectly turn a temporary global shortage into failure.
    pub fn set_record_limit(&mut self, limit: usize) -> bool {
        if limit < self.records.len() {
            return false;
        }
        self.max_records = limit;
        true
    }

    /// Mark the frontier stale. A new request must be created for the revised
    /// terrain, mobility, or goal; stepping this search cannot use old records.
    pub fn invalidate(&mut self) -> SearchStatus {
        self.terminal = Some(SearchStatus::Invalidated);
        SearchStatus::Invalidated
    }

    /// Advance the search by at most `expansion_budget` node expansions.
    ///
    /// A budget of zero leaves the search untouched and returns [`SearchStatus::Pending`]
    /// unless a prior call already reached a terminal status.  Stale queue
    /// entries created by an improved route do not consume expansion budget.
    pub fn step<T: Traversal + ?Sized>(
        &mut self,
        expansion_budget: usize,
        traversal: &T,
    ) -> SearchStatus {
        if let Some(status) = &self.terminal {
            return status.clone();
        }

        if self.goal.contains(self.start) {
            let status = SearchStatus::Found {
                path: NavigationPath {
                    cells: vec![self.start],
                    cost: 0,
                },
            };
            self.terminal = Some(status.clone());
            return status;
        }

        if expansion_budget == 0 {
            return self.pending(0);
        }

        let mut expanded = 0;
        while expanded < expansion_budget {
            let Some(entry) = self.open.pop() else {
                let status = SearchStatus::Unreachable;
                self.terminal = Some(status.clone());
                return status;
            };

            let Some(record) = self.records.get(&entry.position).copied() else {
                continue;
            };
            if record.cost != entry.cost {
                continue;
            }

            if self.goal.contains(entry.position) {
                let status = SearchStatus::Found {
                    path: self.path_to(entry.position),
                };
                self.terminal = Some(status.clone());
                return status;
            }

            expanded += 1;
            self.total_expanded += 1;
            for neighbor in entry.position.neighbors() {
                if !Self::contains(self.width, self.height, neighbor) {
                    continue;
                }
                let Some(edge_cost) = traversal.traversal_cost(entry.position, neighbor) else {
                    continue;
                };
                if edge_cost == 0 {
                    continue;
                }
                let Some(candidate_cost) = record.cost.checked_add(u64::from(edge_cost)) else {
                    let status = SearchStatus::ResourceLimit {
                        records: self.records.len(),
                        limit: self.max_records,
                    };
                    self.terminal = Some(status.clone());
                    return status;
                };

                let should_update = self
                    .records
                    .get(&neighbor)
                    .is_none_or(|known| candidate_cost < known.cost);
                if !should_update {
                    continue;
                }
                if !self.records.contains_key(&neighbor) && self.records.len() >= self.max_records {
                    let status = SearchStatus::ResourceLimit {
                        records: self.records.len(),
                        limit: self.max_records,
                    };
                    self.terminal = Some(status.clone());
                    return status;
                }

                self.records.insert(
                    neighbor,
                    Record {
                        cost: candidate_cost,
                        parent: Some(entry.position),
                    },
                );
                let heuristic = neighbor
                    .distance(self.goal.center)
                    .saturating_sub(self.goal.radius);
                let Some(estimated_total) = candidate_cost.checked_add(u64::from(heuristic)) else {
                    let status = SearchStatus::ResourceLimit {
                        records: self.records.len(),
                        limit: self.max_records,
                    };
                    self.terminal = Some(status.clone());
                    return status;
                };
                self.open.push(OpenEntry {
                    estimated_total,
                    heuristic,
                    cost: candidate_cost,
                    position: neighbor,
                });
            }
        }

        if self.open.is_empty() {
            let status = SearchStatus::Unreachable;
            self.terminal = Some(status.clone());
            return status;
        }
        self.pending(expanded)
    }

    fn pending(&self, expanded: usize) -> SearchStatus {
        SearchStatus::Pending {
            expanded,
            total_expanded: self.total_expanded,
            records: self.records.len(),
        }
    }

    fn path_to(&self, destination: Hex) -> NavigationPath {
        let mut cells = Vec::new();
        let mut current = destination;
        let cost = self.records[&destination].cost;
        loop {
            cells.push(current);
            let Some(parent) = self.records[&current].parent else {
                break;
            };
            current = parent;
        }
        cells.reverse();
        NavigationPath { cells, cost }
    }

    fn contains(width: u16, height: u16, position: Hex) -> bool {
        position.x < width && position.y < height
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cmp::Reverse;
    use std::collections::{BinaryHeap, HashMap};

    fn all_open(width: u16, height: u16) -> HexGrid {
        HexGrid::uniform(width, height, 1).unwrap()
    }

    #[test]
    fn shared_record_limit_cannot_drop_below_discovered_usage() {
        let mut search =
            AStarSearch::new(5, 5, Hex::new(0, 0), Goal::new(Hex::new(4, 4), 0)).unwrap();
        assert!(matches!(
            search.step(1, &all_open(5, 5)),
            SearchStatus::Pending { .. }
        ));
        let used = search.record_count();
        assert!(used > 1);
        assert!(!search.set_record_limit(used - 1));
        assert_eq!(search.record_limit(), DEFAULT_MAX_RECORDS);
        assert!(search.set_record_limit(used + 4));
        assert_eq!(search.record_limit(), used + 4);
    }

    fn dijkstra(grid: &HexGrid, start: Hex, goal: Goal) -> Option<u64> {
        let mut distances = HashMap::from([(start, 0_u64)]);
        let mut queue = BinaryHeap::from([Reverse((0_u64, start))]);
        while let Some(Reverse((cost, position))) = queue.pop() {
            if distances.get(&position) != Some(&cost) {
                continue;
            }
            if position.distance(goal.center) <= goal.radius {
                return Some(cost);
            }
            for neighbor in position.neighbors() {
                let Some(edge) = grid.traversal_cost(position, neighbor) else {
                    continue;
                };
                let Some(next) = cost.checked_add(u64::from(edge)) else {
                    continue;
                };
                if distances.get(&neighbor).is_none_or(|known| next < *known) {
                    distances.insert(neighbor, next);
                    queue.push(Reverse((next, neighbor)));
                }
            }
        }
        None
    }

    #[test]
    fn coordinate_neighbors_and_distance_match_column_staggered_geometry() {
        let center = Hex::new(2, 2);
        assert_eq!(center.neighbors()[0], Hex::new(2, 1));
        assert_eq!(center.neighbors()[1], Hex::new(3, 2));
        assert_eq!(center.neighbors()[2], Hex::new(3, 3));
        assert_eq!(center.neighbors()[3], Hex::new(2, 3));
        assert_eq!(center.neighbors()[4], Hex::new(1, 3));
        assert_eq!(center.neighbors()[5], Hex::new(1, 2));
        assert_eq!(center.distance(Hex::new(5, 4)), 3);
    }

    #[test]
    fn route_cost_matches_dijkstra_on_weighted_obstacle_map() {
        let width = 9;
        let height = 7;
        let mut grid = all_open(width, height);
        for y in 1..6 {
            grid.set_cost(Hex::new(4, y), None).unwrap();
        }
        grid.set_cost(Hex::new(4, 3), Some(3)).unwrap();
        for x in 0..width {
            grid.set_cost(Hex::new(x, 0), Some(2)).unwrap();
        }
        let start = Hex::new(1, 3);
        let goal = Goal::new(Hex::new(7, 3), 0);
        let expected = dijkstra(&grid, start, goal).unwrap();
        let mut search = AStarSearch::new(width, height, start, goal).unwrap();
        let status = loop {
            let status = search.step(3, &grid);
            if status.is_terminal() {
                break status;
            }
        };
        let SearchStatus::Found { path } = status else {
            panic!("expected a route, got {status:?}");
        };
        assert_eq!(path.cost, expected);
        assert_eq!(path.cells.first(), Some(&start));
        assert!(
            path.cells
                .last()
                .is_some_and(|position| position.distance(goal.center) <= goal.radius)
        );
    }

    #[test]
    fn budgeted_search_resumes_without_changing_the_route() {
        let width = 8;
        let height = 8;
        let grid = all_open(width, height);
        let start = Hex::new(0, 0);
        let goal = Goal::new(Hex::new(7, 7), 0);
        let mut search = AStarSearch::new(width, height, start, goal).unwrap();
        let mut pending_calls = 0;
        let path = loop {
            match search.step(1, &grid) {
                SearchStatus::Pending { .. } => pending_calls += 1,
                SearchStatus::Found { path } => break path,
                status => panic!("expected a pending search followed by a route, got {status:?}"),
            }
        };
        assert!(pending_calls > 1);
        assert_eq!(path.cells.first(), Some(&start));
        assert_eq!(path.cells.last(), Some(&goal.center));
        assert_eq!(path.cost, u64::from(start.distance(goal.center)));
        assert_eq!(search.step(0, &grid), SearchStatus::Found { path });
    }

    #[test]
    fn invalidated_search_cannot_resume_old_frontier() {
        let grid = all_open(8, 8);
        let mut search =
            AStarSearch::new(8, 8, Hex::new(0, 0), Goal::new(Hex::new(7, 7), 0)).unwrap();
        assert!(matches!(
            search.step(1, &grid),
            SearchStatus::Pending { .. }
        ));
        assert_eq!(search.invalidate(), SearchStatus::Invalidated);
        assert_eq!(search.step(256, &grid), SearchStatus::Invalidated);
    }

    #[test]
    fn goal_radius_completes_at_the_first_region_cell() {
        let grid = all_open(8, 8);
        let start = Hex::new(0, 0);
        let goal = Goal::new(Hex::new(5, 4), 2);
        let mut search = AStarSearch::new(8, 8, start, goal).unwrap();
        let status = search.step(200, &grid);
        let SearchStatus::Found { path } = status else {
            panic!("expected a route, got {status:?}");
        };
        assert!(
            path.cells
                .last()
                .is_some_and(|position| position.distance(goal.center) <= goal.radius)
        );
        assert_eq!(
            path.cost,
            u64::from(start.distance(goal.center) - goal.radius)
        );
    }

    #[test]
    fn record_limit_reports_resource_limit_and_preserves_cap() {
        let grid = all_open(4, 4);
        let mut search =
            AStarSearch::with_record_limit(4, 4, Hex::new(0, 0), Goal::new(Hex::new(3, 3), 0), 1)
                .unwrap();
        assert_eq!(
            search.step(1, &grid),
            SearchStatus::ResourceLimit {
                records: 1,
                limit: 1,
            }
        );
        assert_eq!(search.record_count(), 1);
        assert_eq!(
            search.step(10, &grid),
            SearchStatus::ResourceLimit {
                records: 1,
                limit: 1,
            }
        );
    }

    #[test]
    fn blocked_map_is_unreachable() {
        let mut grid = all_open(5, 5);
        for y in 0..5 {
            grid.set_cost(Hex::new(2, y), None).unwrap();
        }
        let mut search =
            AStarSearch::new(5, 5, Hex::new(0, 2), Goal::new(Hex::new(4, 2), 0)).unwrap();
        assert_eq!(search.step(100, &grid), SearchStatus::Unreachable);
    }

    #[test]
    fn callback_provider_matches_dense_grid_provider() {
        let grid = all_open(6, 6);
        let callback = |_: Hex, to: Hex| (to.x < 6 && to.y < 6).then_some(1);
        let mut from_grid =
            AStarSearch::new(6, 6, Hex::new(0, 0), Goal::new(Hex::new(5, 5), 0)).unwrap();
        let mut from_callback = from_grid.clone();
        let first = loop {
            let status = from_grid.step(8, &grid);
            if status.is_terminal() {
                break status;
            }
        };
        let second = loop {
            let status = from_callback.step(8, &callback);
            if status.is_terminal() {
                break status;
            }
        };
        assert_eq!(first, second);
    }
}
