//! Hex geometry in normalized map units: offset coordinates, continuous positions, ranges,
//! bearings and straight-line traces across the column-staggered grid.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Zero-based offset coordinates; signed values allow examining neighbors outside a map.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleHexCoordinate {
    pub x: i32,
    pub y: i32,
}

/// Continuous map position measured in hex heights, with positive y pointing south.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BattlePoint {
    pub x: f64,
    pub y: f64,
}

impl BattleHexCoordinate {
    /// Center of a column-staggered hex; even columns are offset half a hex south.
    pub fn center(self) -> BattlePoint {
        BattlePoint {
            x: (2.0 + 3.0 * f64::from(self.x)) / (2.0 * 3.0_f64.sqrt()),
            y: f64::from(self.y) + if self.x.rem_euclid(2) == 0 { 0.5 } else { 0.0 },
        }
    }

    /// Cube coordinates used only for discrete adjacency and shortest hex-path distance.
    fn cube(self) -> [i64; 3] {
        let q = i64::from(self.x);
        let r = i64::from(self.y) - (q + q.rem_euclid(2)) / 2;
        [q, r, -q - r]
    }

    /// Minimum number of adjacent hex transitions, independent of terrain cost.
    pub fn distance(self, other: Self) -> u64 {
        let a = self.cube();
        let b = other.cube();
        a.into_iter()
            .zip(b)
            .map(|(a, b)| a.abs_diff(b))
            .max()
            .unwrap()
    }

    /// Offset coordinates of the six adjacent cells, clockwise from north, widened so no
    /// neighbor of an `i32` coordinate overflows.
    fn adjacent(self) -> [(i64, i64); 6] {
        let [q, r, _] = self.cube();
        [(0, -1), (1, -1), (1, 0), (0, 1), (-1, 1), (-1, 0)].map(|(dq, dr)| {
            let q = q + dq;
            (q, r + dr + (q + q.rem_euclid(2)) / 2)
        })
    }

    /// The six adjacent cells clockwise from north, on or off any map. Fails only when a
    /// neighbor's coordinates fall outside `i32`; use [`Self::neighbors_within`] to stay on a
    /// map.
    pub fn neighbors(self) -> Result<[Self; 6]> {
        let mut result = [self; 6];
        for (slot, (x, y)) in result.iter_mut().zip(self.adjacent()) {
            *slot = Self {
                x: i32::try_from(x)?,
                y: i32::try_from(y)?,
            };
        }
        Ok(result)
    }

    /// The adjacent cells on a `width` by `height` map, indexed by direction clockwise from
    /// north, with `None` for each direction that leaves the map. Use
    /// `.into_iter().flatten()` to visit only the neighbors that exist.
    pub fn neighbors_within(self, width: u16, height: u16) -> [Option<Self>; 6] {
        self.adjacent().map(|(x, y)| {
            let on_map = (0..i64::from(width)).contains(&x) && (0..i64::from(height)).contains(&y);
            // Both coordinates are below a u16 bound, so they fit in i32.
            on_map.then_some(Self {
                x: x as i32,
                y: y as i32,
            })
        })
    }
}

impl BattlePoint {
    /// Reject non-finite inputs before trigonometry or distance calculations.
    fn validate(self) -> Result<()> {
        ensure!(
            self.x.is_finite() && self.y.is_finite(),
            "Invalid map point"
        );
        Ok(())
    }

    /// Locate the closest hex center, with south/east ownership of exact shared boundaries.
    /// The left map border retains the rectangular first-column clipping convention.
    pub fn containing_hex(self) -> Result<BattleHexCoordinate> {
        self.validate()?;
        ensure!(
            self.x.abs() < 1e8 && self.y.abs() < 1e8,
            "Map point exceeds coordinate limits"
        );
        if self.x < 3.0_f64.sqrt() / 6.0 {
            return Ok(BattleHexCoordinate {
                x: if self.x < 0.0 { -1 } else { 0 },
                y: self.y.floor() as i32,
            });
        }
        let column = ((self.x - 1.0 / 3.0_f64.sqrt()) / (3.0_f64.sqrt() / 2.0)).floor() as i32;
        let mut best = None;
        // A center in either enclosing column is at most sqrt(7)/4 hex
        // heights away. Centers outside these columns are at least sqrt(3)/2
        // away, so they cannot win even at a boundary tolerance.
        for x in column..=column + 1 {
            let offset = if x.rem_euclid(2) == 0 { 0.5 } else { 0.0 };
            let row = (self.y - offset).floor() as i32;
            for y in row..=row + 1 {
                let hex = BattleHexCoordinate { x, y };
                let center = hex.center();
                let dx = center.x - self.x;
                let dy = center.y - self.y;
                let squared = dx * dx + dy * dy;
                if best.is_none_or(|(old_squared, old): (f64, BattleHexCoordinate)| {
                    // All four candidate centers are less than three hex heights
                    // away. Outside this conservative squared-distance margin,
                    // ordering cannot be affected by the 1e-12 boundary tolerance.
                    // Near a shared edge retain the exact hypot-based tie rule.
                    if (squared - old_squared).abs() > 1e-10 {
                        return squared < old_squared;
                    }
                    let distance = dx.hypot(dy);
                    let old_center = old.center();
                    let old_distance = (old_center.x - self.x).hypot(old_center.y - self.y);
                    distance < old_distance - 1e-12
                        || ((distance - old_distance).abs() <= 1e-12 && (y, x) > (old.y, old.x))
                }) {
                    best = Some((squared, hex));
                }
            }
        }
        Ok(best.unwrap().1)
    }

    /// Ordered cells occupied along a straight segment, including both endpoint owners.
    /// Intersections with the three families of hex-edge lines partition the segment
    /// exactly; midpoint classification avoids fixed sampling that can miss narrow crossings.
    /// The bound covers any segment inside a supported 1000-by-1000 battlefield.
    pub fn trace(self, end: Self) -> Result<Vec<BattleHexCoordinate>> {
        Ok(self
            .trace_positions(end)?
            .into_iter()
            .map(|(hex, _)| hex)
            .collect())
    }

    /// A representative point on the segment inside each crossed hex, for movement-triggered effects.
    pub fn trace_positions(self, end: Self) -> Result<Vec<(BattleHexCoordinate, BattlePoint)>> {
        let cuts = self.trace_cuts(end)?;
        let dx = end.x - self.x;
        let dy = end.y - self.y;
        let mut cells = vec![(self.containing_hex()?, self)];
        for interval in cuts.windows(2) {
            let t = (interval[0] + interval[1]) / 2.0;
            let point = Self {
                x: self.x + t * dx,
                y: self.y + t * dy,
            };
            let cell = point.containing_hex()?;
            if cells.last().map(|(hex, _)| hex) != Some(&cell) {
                cells.push((cell, point));
            }
        }
        let last = end.containing_hex()?;
        if cells.last().map(|(hex, _)| hex) != Some(&last) {
            cells.push((last, end));
        }
        Ok(cells)
    }

    /// Shared geometric partition at every candidate hex edge, independent of traversal effects.
    fn trace_cuts(self, end: Self) -> Result<Vec<f64>> {
        self.containing_hex()?;
        end.containing_hex()?;
        ensure!(
            self.range(end)? <= 4096.0,
            "Map trace exceeds distance limit"
        );
        let dx = end.x - self.x;
        let mut cuts = vec![0.0, 1.0];
        for (start, finish, spacing, offset) in [
            (self.y, end.y, 0.5, 0.0),
            (
                self.y + 3.0_f64.sqrt() * self.x,
                end.y + 3.0_f64.sqrt() * end.x,
                1.0,
                0.5,
            ),
            (
                self.y - 3.0_f64.sqrt() * self.x,
                end.y - 3.0_f64.sqrt() * end.x,
                1.0,
                0.5,
            ),
        ] {
            if start == finish {
                continue;
            }
            let first = ((start.min(finish) - offset) / spacing).ceil() as i64;
            let last = ((start.max(finish) - offset) / spacing).floor() as i64;
            for line in first..=last {
                let t = (line as f64 * spacing + offset - start) / (finish - start);
                if t > 0.0 && t < 1.0 {
                    cuts.push(t);
                }
            }
        }
        // The first column has a rectangular clipped border instead of infinite hex tiling.
        if dx != 0.0 {
            for border in [0.0, 3.0_f64.sqrt() / 6.0] {
                let t = (border - self.x) / dx;
                if t > 0.0 && t < 1.0 {
                    cuts.push(t);
                }
            }
        }
        cuts.sort_by(f64::total_cmp);
        cuts.dedup();
        Ok(cuts)
    }

    /// Exact parameter intervals occupied by each hex along a movement segment.
    /// Zero-length endpoint intervals retain boundary ownership used by ordinary tracing.
    pub fn trace_intervals(self, end: Self) -> Result<Vec<(BattleHexCoordinate, f64, f64)>> {
        let cuts = self.trace_cuts(end)?;
        let mut result = vec![(self.containing_hex()?, 0.0, 0.0)];
        for pair in cuts.windows(2) {
            let t = (pair[0] + pair[1]) / 2.0;
            let point = Self {
                x: self.x + (end.x - self.x) * t,
                y: self.y + (end.y - self.y) * t,
            };
            let hex = point.containing_hex()?;
            if let Some((last, _, finish)) = result.last_mut()
                && *last == hex
            {
                *finish = pair[1];
                continue;
            }
            result.push((hex, pair[0], pair[1]));
        }
        let last = end.containing_hex()?;
        if result.last().is_none_or(|(hex, _, _)| *hex != last) {
            result.push((last, 1.0, 1.0));
        }
        Ok(result)
    }

    /// Euclidean horizontal range, in hex heights rather than graph steps.
    pub fn range(self, other: Self) -> Result<f64> {
        self.validate()?;
        other.validate()?;
        let range = (other.x - self.x).hypot(other.y - self.y);
        ensure!(range.is_finite(), "Map range overflow");
        Ok(range)
    }

    /// Clockwise bearing from north, absent for coincident points.
    pub fn bearing(self, other: Self) -> Result<Option<f64>> {
        self.range(other)?;
        Ok(self.bearing_after_range(other))
    }

    /// Both points and their range must already have passed `range` validation.
    /// [`BattlePoint::bearing`] is the checked entry point; this skips repeating the checks.
    pub fn bearing_after_range(self, other: Self) -> Option<f64> {
        if self == other {
            return None;
        }
        Some(
            (other.x - self.x)
                .atan2(self.y - other.y)
                .to_degrees()
                .rem_euclid(360.0),
        )
    }

    /// Move a continuous position along a clockwise bearing for a nonnegative hex distance.
    pub fn project(self, bearing: f64, distance: f64) -> Result<Self> {
        self.validate()?;
        ensure!(
            bearing.is_finite() && distance.is_finite() && distance >= 0.0,
            "Invalid map projection"
        );
        let radians = bearing.rem_euclid(360.0).to_radians();
        let result = Self {
            x: self.x + distance * radians.sin(),
            y: self.y - distance * radians.cos(),
        };
        result.validate()?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Independent Euclidean-distance oracle for the optimized nearest-center query.
    fn reference_containment(point: BattlePoint) -> BattleHexCoordinate {
        if point.x < 3.0_f64.sqrt() / 6.0 {
            return BattleHexCoordinate {
                x: if point.x < 0.0 { -1 } else { 0 },
                y: point.y.floor() as i32,
            };
        }
        let column = ((point.x - 1.0 / 3.0_f64.sqrt()) / (3.0_f64.sqrt() / 2.0)).floor() as i32;
        let mut best: Option<(f64, BattleHexCoordinate)> = None;
        for x in column - 1..=column + 2 {
            let row = (point.y - if x.rem_euclid(2) == 0 { 0.5 } else { 0.0 }).floor() as i32;
            for y in row..=row + 1 {
                let hex = BattleHexCoordinate { x, y };
                let distance = point.range(hex.center()).unwrap();
                if best.is_none_or(|(old_distance, old)| {
                    distance < old_distance - 1e-12
                        || ((distance - old_distance).abs() <= 1e-12 && (y, x) > (old.y, old.x))
                }) {
                    best = Some((distance, hex));
                }
            }
        }
        best.unwrap().1
    }

    #[test]
    fn fast_containment_preserves_euclidean_boundary_ownership() {
        let check = |point: BattlePoint| {
            assert_eq!(
                point.containing_hex().unwrap(),
                reference_containment(point),
                "{point:?}"
            );
        };
        let mut seed = 0x5eed_u64;
        for _ in 0..25_000 {
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let x = (seed >> 32) as u32;
            seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1);
            let y = (seed >> 32) as u32;
            check(BattlePoint {
                x: f64::from(x) / 43.0,
                y: f64::from(y) / 43.0 - 5e7,
            });
        }
        for x in [0, 1, 2, 99, 999, 10_000_000] {
            let center = BattleHexCoordinate { x, y: 10 }.center();
            for neighbor in (BattleHexCoordinate { x, y: 10 }).neighbors().unwrap() {
                let other = neighbor.center();
                for offset in [-1e-10, -1e-12, -1e-14, 0.0, 1e-14, 1e-12, 1e-10] {
                    check(BattlePoint {
                        x: (center.x + other.x) / 2.0 + offset,
                        y: (center.y + other.y) / 2.0 + offset,
                    });
                }
            }
        }
    }

    /// Both column parities have six unit-distance neighbors at the expected compass bearings.
    #[test]
    fn staggered_neighbors_and_projection_share_a_compass() {
        for x in -3..=3 {
            let origin = BattleHexCoordinate { x, y: 4 };
            for (index, neighbor) in origin.neighbors().unwrap().into_iter().enumerate() {
                assert_eq!(origin.distance(neighbor), 1);
                assert!(neighbor.neighbors().unwrap().contains(&origin));
                assert!((origin.center().range(neighbor.center()).unwrap() - 1.0).abs() < 1e-12);
                let bearing = origin.center().bearing(neighbor.center()).unwrap().unwrap();
                assert!((bearing - index as f64 * 60.0).abs() < 1e-10);
                let projected = origin.center().project(bearing, 1.0).unwrap();
                assert!(projected.range(neighbor.center()).unwrap() < 1e-12);
            }
        }
    }

    /// Bounded neighbors keep their directions, drop the ones off the map and agree with the
    /// unbounded list everywhere else.
    #[test]
    fn neighbors_within_keep_directions_and_drop_off_map_cells() {
        // Even columns sit half a hex south of odd ones.
        let corner = BattleHexCoordinate { x: 0, y: 0 };
        let east = BattleHexCoordinate { x: 1, y: 0 };
        assert_eq!(
            corner.neighbors_within(3, 3),
            [
                None,
                Some(east),
                Some(BattleHexCoordinate { x: 1, y: 1 }),
                Some(BattleHexCoordinate { x: 0, y: 1 }),
                None,
                None,
            ]
        );
        assert_eq!(
            east.neighbors_within(3, 3),
            [
                None,
                None,
                Some(BattleHexCoordinate { x: 2, y: 0 }),
                Some(BattleHexCoordinate { x: 1, y: 1 }),
                Some(corner),
                None,
            ]
        );
        for x in -2..=6 {
            for y in -2..=6 {
                let hex = BattleHexCoordinate { x, y };
                for (bounded, unbounded) in hex
                    .neighbors_within(5, 4)
                    .into_iter()
                    .zip(hex.neighbors().unwrap())
                {
                    let on_map = (0..5).contains(&unbounded.x) && (0..4).contains(&unbounded.y);
                    assert_eq!(bounded, on_map.then_some(unbounded), "{hex:?}");
                }
            }
        }
        let far = BattleHexCoordinate {
            x: i32::MAX,
            y: i32::MIN,
        };
        assert_eq!(far.neighbors_within(u16::MAX, u16::MAX), [None; 6]);
        assert_eq!(
            BattleHexCoordinate { x: 0, y: 0 }.neighbors_within(0, 0),
            [None; 6]
        );
    }

    #[test]
    fn graph_distance_and_euclidean_range_are_distinct() {
        let a = BattleHexCoordinate { x: 0, y: 0 };
        let b = BattleHexCoordinate { x: 2, y: 0 };
        assert_eq!(a.distance(b), 2);
        assert!((a.center().range(b.center()).unwrap() - 3.0_f64.sqrt()).abs() < 1e-12);
        assert_eq!(a.center().bearing(a.center()).unwrap(), None);
        assert_eq!(a.center().project(720.0, 0.0).unwrap(), a.center());
        assert!(a.center().project(f64::NAN, 1.0).is_err());
        assert!(a.center().project(0.0, -1.0).is_err());
        assert!(
            BattlePoint {
                x: f64::INFINITY,
                y: 0.0
            }
            .range(a.center())
            .is_err()
        );
        assert!(
            BattleHexCoordinate { x: i32::MAX, y: 0 }
                .neighbors()
                .is_err()
        );
        assert!(
            BattleHexCoordinate {
                x: i32::MIN,
                y: i32::MIN
            }
            .distance(BattleHexCoordinate {
                x: i32::MAX,
                y: i32::MAX
            }) > u64::from(u32::MAX)
        );
    }
}
