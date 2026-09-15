//! Hex geometry in normalized map units, shared by movement, range and navigation.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
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

    /// Adjacent cells clockwise from north. The caller applies actual map bounds.
    pub fn neighbors(self) -> Result<[Self; 6]> {
        let [q, r, _] = self.cube();
        let mut result = [self; 6];
        for (index, (dq, dr)) in [(0, -1), (1, -1), (1, 0), (0, 1), (-1, 1), (-1, 0)]
            .into_iter()
            .enumerate()
        {
            let q = q + dq;
            result[index] = Self {
                x: i32::try_from(q)?,
                y: i32::try_from(r + dr + (q + q.rem_euclid(2)) / 2)?,
            };
        }
        Ok(result)
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
        for x in column - 1..=column + 2 {
            let offset = if x.rem_euclid(2) == 0 { 0.5 } else { 0.0 };
            let row = (self.y - offset).floor() as i32;
            for y in row..=row + 1 {
                let hex = BattleHexCoordinate { x, y };
                let distance = self.range(hex.center())?;
                if best.is_none_or(|(old_distance, old): (f64, BattleHexCoordinate)| {
                    distance < old_distance - 1e-12
                        || ((distance - old_distance).abs() <= 1e-12 && (y, x) > (old.y, old.x))
                }) {
                    best = Some((distance, hex));
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
    pub(super) fn trace_positions(
        self,
        end: Self,
    ) -> Result<Vec<(BattleHexCoordinate, BattlePoint)>> {
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
    pub(super) fn trace_intervals(self, end: Self) -> Result<Vec<(BattleHexCoordinate, f64, f64)>> {
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
        if self == other {
            return Ok(None);
        }
        Ok(Some(
            (other.x - self.x)
                .atan2(self.y - other.y)
                .to_degrees()
                .rem_euclid(360.0),
        ))
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

/// Geometric measurement between placed units; it does not imply visibility or weapon reach.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BattleRange {
    pub horizontal: f64,
    pub spatial: f64,
    pub bearing: Option<f64>,
    pub hex_distance: u64,
}

/// Measure continuous unit positions on the same battlefield, including signed ground elevation.
pub fn unit_range(world: &World, first: ObjectId, second: ObjectId) -> Result<BattleRange> {
    let sample = |id| -> Result<_> {
        if let Some(vehicle) = world.btech.vehicles().get(&id) {
            let position = vehicle.position().context("Unit is not on a battlefield")?;
            let point = vehicle
                .motion()
                .context("Unit is not on a battlefield")?
                .point;
            let tile = world
                .btech
                .maps()
                .get(&position.map)
                .context("Map not found")?
                .base_hex(i64::from(position.x), i64::from(position.y))?;
            return Ok((position, point, Some(vehicle.altitude(tile) / 5.0)));
        }
        let unit = world
            .btech
            .constructed_units()
            .get(&id)
            .context("Unit construction state is unavailable")?;
        let position = unit.position().context("Unit is not on a battlefield")?;
        let point = unit.motion().context("Unit is not on a battlefield")?.point;
        let height = unit.retained_altitude().map(|height| height / 5.0);
        Ok((position, point, height))
    };
    let (first, first_point, first_height) = sample(first)?;
    let (second, second_point, second_height) = sample(second)?;
    ensure!(
        first.map == second.map,
        "Units are on different battlefields"
    );
    let map = world
        .btech
        .maps()
        .get(&first.map)
        .context("Map not found")?;
    let height = |x, y| -> Result<f64> {
        Ok(f64::from(map.base_hex(i64::from(x), i64::from(y))?.standing_height()) / 5.0)
    };
    let a = BattleHexCoordinate {
        x: i32::from(first.x),
        y: i32::from(first.y),
    };
    let b = BattleHexCoordinate {
        x: i32::from(second.x),
        y: i32::from(second.y),
    };
    let horizontal = first_point.range(second_point)?;
    let dz = first_height.unwrap_or(height(first.x, first.y)?)
        - second_height.unwrap_or(height(second.x, second.y)?);
    Ok(BattleRange {
        horizontal,
        spatial: horizontal.hypot(dz),
        bearing: first_point.bearing(second_point)?,
        hex_distance: a.distance(b),
    })
}

/// Current signed integer altitude used by terrain effects, or None when unplaced.
/// Ice surfaces, retained terrain-break heights and committed jump rounding share one resolver.
pub fn unit_elevation(world: &World, id: ObjectId) -> Result<Option<i32>> {
    if let Some(vehicle) = world.btech.vehicles().get(&id) {
        let Some(position) = vehicle.position() else {
            return Ok(None);
        };
        let map = world
            .btech
            .maps()
            .get(&position.map)
            .context("Map not found")?;
        let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
        return Ok(Some(vehicle.elevation_level(tile)));
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?;
    let Some(position) = unit.position() else {
        return Ok(None);
    };
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?;
    let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    Ok(Some(unit.elevation_level(tile)))
}

impl super::BattleUnit {
    /// Continuous altitude for geometry and external transport; terrain effects retain their integer resolver.
    pub(super) fn altitude(&self, tile: super::BattleHex) -> f64 {
        self.retained_altitude()
            .unwrap_or_else(|| f64::from(tile.standing_height()))
    }

    /// Optional physical height override, shared by LOS and range without changing terrain-cover semantics.
    pub(super) fn retained_altitude(&self) -> Option<f64> {
        self.flight()
            .map(|flight| flight.sample().elevation)
            .or_else(|| self.orbital_drop().map(|drop| f64::from(drop.elevation())))
            .or_else(|| {
                self.free_fall()
                    .filter(|fall| !fall.grounded())
                    .map(|fall| fall.altitude())
            })
            .or(self.ground_elevation)
    }
}

impl super::BattleVehicle {
    /// Continuous aircraft or carried height, falling back to the chassis-specific terrain support.
    pub(super) fn altitude(&self, tile: super::BattleHex) -> f64 {
        self.vtol_flight()
            .map(|flight| flight.altitude)
            .or_else(|| self.orbital_drop().map(|drop| f64::from(drop.elevation())))
            .or_else(|| self.free_fall().map(|fall| fall.altitude()))
            .or(self.ground_elevation)
            .unwrap_or_else(|| f64::from(self.terrain_elevation(tile, self.under_bridge())))
    }
}

/// Continuous height in terrain levels for any placed unit; unlike terrain-rule elevation this preserves fractions.
pub fn unit_altitude(world: &World, id: ObjectId) -> Result<Option<f64>> {
    let position = super::scanner::scanner_unit(world, id)
        .context("Unit construction state is unavailable")?
        .position;
    let Some(position) = position else {
        return Ok(None);
    };
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?
        .base_hex(i64::from(position.x), i64::from(position.y))?;
    Ok(Some(world.btech.vehicles().get(&id).map_or_else(
        || world.btech.constructed_units()[&id].altitude(tile),
        |unit| unit.altitude(tile),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

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
