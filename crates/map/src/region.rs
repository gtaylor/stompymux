//! Map regions: named multi-hex areas for scripts, outlined by corner hexes and filled in.
//!
//! A region lists its corner hexes in order. Its member hexes are the outline, traced between
//! the centers of consecutive corners and back from the last corner to the first, plus every
//! hex whose center lies inside that outline. One corner makes a single-hex region and two
//! make a line. An outline that crosses itself encloses the areas it winds around an odd
//! number of times, so where it overlaps itself the overlap is left out (but the outline
//! itself always counts).
use crate::{HexCoordinate, Point};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// A scripted region of a map. Like points of interest, regions are map metadata for scripts:
/// they are never shown to units and do not change terrain.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MapRegion {
    /// Case-sensitive category chosen by the map author, such as `deployment`.
    #[serde(rename = "type")]
    pub kind: String,
    /// Display name chosen by the map author.
    pub name: String,
    /// Zero-based `[x, y]` corner hexes in outline order.
    pub corners: Vec<[u16; 2]>,
}

impl MapRegion {
    /// Require a non-empty type and name without NUL characters, and at least one corner,
    /// every one of them inside a `width` by `height` map.
    pub fn validate(&self, width: i64, height: i64) -> Result<()> {
        for (field, value) in [("type", &self.kind), ("name", &self.name)] {
            ensure!(
                !value.is_empty() && !value.contains('\0'),
                "region {field} must be non-empty text without NUL characters"
            );
        }
        ensure!(
            !self.corners.is_empty(),
            "region {:?} must have at least one corner",
            self.name
        );
        for &[x, y] in &self.corners {
            ensure!(
                i64::from(x) < width && i64::from(y) < height,
                "region {:?} corner {x},{y} is off the map",
                self.name
            );
        }
        Ok(())
    }

    /// The region's corners as hex coordinates, in outline order.
    pub fn corner_hexes(&self) -> impl Iterator<Item = HexCoordinate> + '_ {
        self.corners.iter().map(|&[x, y]| HexCoordinate {
            x: i32::from(x),
            y: i32::from(y),
        })
    }

    /// Every member hex, in row-major order (by row, then column) without repeats. Members
    /// never leave the rectangle of rows and columns the corners span, so a region whose
    /// corners all lie on a map has every member on that map too.
    pub fn hexes(&self) -> Result<Vec<HexCoordinate>> {
        let corners: Vec<HexCoordinate> = self.corner_hexes().collect();
        let Some(bounds) = Bounds::around(&corners) else {
            return Ok(Vec::new());
        };
        let outline: Vec<Point> = corners.iter().map(|corner| corner.center()).collect();
        // Twice the outline's signed area; zero when the corners are all on one line.
        let area: f64 = (0..outline.len())
            .map(|index| {
                let (a, b) = (outline[index], outline[(index + 1) % outline.len()]);
                a.x * b.y - b.x * a.y
            })
            .sum();
        let enclosing = corners.len() >= 3 && area.abs() > 1e-9;
        // Each edge, closing back to the first corner when the outline encloses an area.
        let edges = if enclosing {
            corners.len()
        } else {
            corners.len() - 1
        };
        let mut hexes = vec![corners[0]];
        for index in 0..edges {
            let (start, end) = (outline[index], outline[(index + 1) % outline.len()]);
            let (dx, dy) = (end.x - start.x, end.y - start.y);
            let length = dx.hypot(dy);
            if length == 0.0 {
                continue;
            }
            // An edge that runs exactly along hex boundaries touches the hexes on both sides
            // equally. Shifting it a hair toward the inside of the outline (or north, then
            // west, for a line) settles each such tie on the side that stays in the region.
            let (mut nx, mut ny) = (-dy / length, dx / length);
            let flip = if enclosing {
                area < 0.0
            } else {
                ny > 0.0 || (ny == 0.0 && nx > 0.0)
            };
            if flip {
                (nx, ny) = (-nx, -ny);
            }
            let nudge = |point: Point| Point {
                x: point.x + nx * NUDGE,
                y: point.y + ny * NUDGE,
            };
            hexes.extend(nudge(start).trace(nudge(end))?);
        }
        if enclosing {
            hexes.extend(
                bounds
                    .hexes()
                    .filter(|hex| encloses(&outline, hex.center())),
            );
        }
        hexes.retain(|hex| bounds.contains(*hex));
        hexes.sort_by_key(|hex| (hex.y, hex.x));
        hexes.dedup();
        Ok(hexes)
    }

    /// Whether `coordinate` is one of the region's hexes.
    pub fn contains(&self, coordinate: HexCoordinate) -> Result<bool> {
        Ok(self
            .hexes()?
            .binary_search_by_key(&(coordinate.y, coordinate.x), |hex| (hex.y, hex.x))
            .is_ok())
    }
}

/// How far, in hex heights, an outline edge shifts to settle ties between hexes it runs
/// between: far too little to move it into any hex it does not already touch.
const NUDGE: f64 = 1e-7;

/// The rectangle of columns and rows a region's corners span.
#[derive(Debug, Clone, Copy)]
struct Bounds {
    left: i32,
    right: i32,
    top: i32,
    bottom: i32,
}

impl Bounds {
    /// The rectangle around `corners`, or `None` when there are none.
    fn around(corners: &[HexCoordinate]) -> Option<Self> {
        let first = corners.first()?;
        let mut bounds = Self {
            left: first.x,
            right: first.x,
            top: first.y,
            bottom: first.y,
        };
        for corner in corners {
            bounds.left = bounds.left.min(corner.x);
            bounds.right = bounds.right.max(corner.x);
            bounds.top = bounds.top.min(corner.y);
            bounds.bottom = bounds.bottom.max(corner.y);
        }
        Some(bounds)
    }

    fn contains(self, hex: HexCoordinate) -> bool {
        (self.left..=self.right).contains(&hex.x) && (self.top..=self.bottom).contains(&hex.y)
    }

    /// Every hex in the rectangle. A center inside the outline lies between the corners'
    /// centers, so every hex the outline encloses is among them.
    fn hexes(self) -> impl Iterator<Item = HexCoordinate> {
        (self.top..=self.bottom)
            .flat_map(move |y| (self.left..=self.right).map(move |x| HexCoordinate { x, y }))
    }
}

/// Whether `point` lies strictly inside the closed `outline`, by the even-odd rule. Points on
/// the outline itself are left to the outline's trace.
fn encloses(outline: &[Point], point: Point) -> bool {
    let mut inside = false;
    for (index, a) in outline.iter().enumerate() {
        let b = outline[(index + 1) % outline.len()];
        if (a.y > point.y) != (b.y > point.y) {
            let crossing = a.x + (point.y - a.y) * (b.x - a.x) / (b.y - a.y);
            if point.x < crossing {
                inside = !inside;
            }
        }
    }
    inside
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A region named `Zone` with the given corners.
    fn region(corners: &[[u16; 2]]) -> MapRegion {
        MapRegion {
            kind: "deployment".into(),
            name: "Zone".into(),
            corners: corners.to_vec(),
        }
    }

    /// Coordinates as `(x, y)` pairs, for compact assertions.
    fn pairs(hexes: &[HexCoordinate]) -> Vec<(i32, i32)> {
        hexes.iter().map(|hex| (hex.x, hex.y)).collect()
    }

    /// One corner is a single hex; two are the traced line between them.
    #[test]
    fn single_corners_and_lines() {
        assert_eq!(pairs(&region(&[[3, 2]]).hexes().unwrap()), [(3, 2)]);
        let line = region(&[[1, 1], [4, 1]]).hexes().unwrap();
        let traced = HexCoordinate { x: 1, y: 1 }
            .center()
            .trace(HexCoordinate { x: 4, y: 1 }.center())
            .unwrap();
        let mut expected = pairs(&traced);
        expected.sort_by_key(|&(x, y)| (y, x));
        assert_eq!(pairs(&line), expected);
    }

    /// A rectangle of corners fills every hex inside it, and nothing outside.
    #[test]
    fn outlines_fill_their_interior() {
        let hexes = region(&[[1, 1], [7, 1], [7, 5], [1, 5]]).hexes().unwrap();
        for x in 1..=7 {
            for y in 2..=4 {
                assert!(hexes.contains(&HexCoordinate { x, y }), "{x},{y} missing");
            }
        }
        for hex in &hexes {
            assert!((1..=7).contains(&hex.x), "{hex:?} outside the columns");
            assert!((0..=6).contains(&hex.y), "{hex:?} outside the rows");
        }
        for (x, y) in [(0, 3), (8, 3), (4, 7)] {
            assert!(!hexes.contains(&HexCoordinate { x, y }), "{x},{y} included");
        }
    }

    /// Members are sorted row-major without repeats, and repeating a corner changes nothing.
    #[test]
    fn hexes_are_sorted_and_unique() {
        let triangle = region(&[[2, 0], [8, 4], [0, 6]]).hexes().unwrap();
        let mut sorted = triangle.clone();
        sorted.sort_by_key(|hex| (hex.y, hex.x));
        sorted.dedup();
        assert_eq!(triangle, sorted);
        let repeated = region(&[[2, 0], [2, 0], [8, 4], [0, 6], [0, 6]])
            .hexes()
            .unwrap();
        assert_eq!(repeated, triangle);
        let region = region(&[[2, 0], [8, 4], [0, 6]]);
        assert!(region.contains(HexCoordinate { x: 3, y: 3 }).unwrap());
        assert!(!region.contains(HexCoordinate { x: 9, y: 0 }).unwrap());
    }

    /// Every member of a region whose corners touch the map's edges stays on the map.
    #[test]
    fn members_stay_on_the_map() {
        let (width, height) = (9, 7);
        let region = region(&[[0, 0], [8, 0], [8, 6], [0, 6]]);
        region.validate(width, height).unwrap();
        let hexes = region.hexes().unwrap();
        assert!(region.corner_hexes().all(|corner| hexes.contains(&corner)));
        assert!(hexes.iter().all(|hex| {
            (0..width as i32).contains(&hex.x) && (0..height as i32).contains(&hex.y)
        }));
    }

    /// Edges that run along hex boundaries keep to the region's side: a rectangle through the
    /// centers of even columns keeps its top and bottom edges on the rows of its corners, and
    /// a line along a map's bottom row stays on that row, unbroken.
    #[test]
    fn boundary_edges_settle_inward() {
        let rectangle = region(&[[0, 0], [8, 0], [8, 6], [0, 6]]).hexes().unwrap();
        for x in 0..=8 {
            assert!(
                rectangle.contains(&HexCoordinate { x, y: 6 }),
                "{x},6 missing"
            );
        }
        let line = region(&[[0, 6], [8, 6]]).hexes().unwrap();
        assert_eq!(pairs(&line), (0..=8).map(|x| (x, 6)).collect::<Vec<_>>());
        let reversed = region(&[[8, 6], [0, 6]]).hexes().unwrap();
        assert_eq!(reversed, line);
        let clockwise = region(&[[0, 6], [0, 0], [8, 0], [8, 6]]).hexes().unwrap();
        assert_eq!(clockwise, rectangle);
    }

    #[test]
    fn rejects_malformed_regions() {
        let off = region(&[[0, 0], [9, 0]]);
        let message = format!("{:#}", off.validate(9, 7).unwrap_err());
        assert!(message.contains("off the map"), "{message}");
        let empty = region(&[]);
        let message = format!("{:#}", empty.validate(9, 7).unwrap_err());
        assert!(message.contains("at least one corner"), "{message}");
        let unnamed = MapRegion {
            name: String::new(),
            ..region(&[[0, 0]])
        };
        let message = format!("{:#}", unnamed.validate(9, 7).unwrap_err());
        assert!(message.contains("name must be non-empty"), "{message}");
    }
}
