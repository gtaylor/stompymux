//! Bounded tactical and long-range viewport dimensions and edge clipping for map renderers.
use super::{BattleHexCoordinate, BattleViewKind};
use crate::{ObjectId, World};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};

/// Requested display dimensions; explicit renderer values override saved player preferences.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct BattleViewDimensions {
    /// Requested tactical columns, 5 through 40.
    pub tactical_width: u16,
    /// Requested tactical rows, 5 through 24.
    pub tactical_height: u16,
    /// Requested long-range rows, 10 through 40, before odd-height adjustment.
    pub long_range_height: u16,
}

impl Default for BattleViewDimensions {
    fn default() -> Self {
        Self {
            tactical_width: 21,
            tactical_height: 14,
            long_range_height: 11,
        }
    }
}

impl BattleViewDimensions {
    /// Validate supported user dimensions before hardware and small-map clipping.
    pub fn validate(self) -> Result<()> {
        ensure!(
            (5..=40).contains(&self.tactical_width),
            "Tactical width must be between 5 and 40"
        );
        ensure!(
            (5..=24).contains(&self.tactical_height),
            "Tactical height must be between 5 and 24"
        );
        ensure!(
            (10..=40).contains(&self.long_range_height),
            "Long-range height must be between 10 and 40"
        );
        Ok(())
    }
}

/// A fully in-bounds rectangle. Width and height are counts, never inclusive end coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleViewport {
    /// Scanner battlefield.
    pub map: ObjectId,
    /// Selected center; map-only views clamp it before resolving the rectangle.
    /// Scanner views may retain a center outside map bounds.
    pub requested_center: BattleHexCoordinate,
    /// Upper-left in-bounds coordinate.
    pub origin: BattleHexCoordinate,
    /// Number of visible columns.
    pub width: u16,
    /// Number of visible rows.
    pub height: u16,
    /// Current display hardware radius; zero for a map-side view without a scanner.
    pub maximum_range: u8,
}

/// Resolve display admission and clip its entire rectangle to the scanner's map.
/// Long-range horizontal labels include both ends of a 70-column span (71 cells).
/// Centering reveals no terrain or contacts beyond those needed to select a contact center.
pub fn resolve_viewport(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    kind: BattleViewKind,
    arguments: &str,
    dimensions: BattleViewDimensions,
) -> Result<BattleViewport> {
    dimensions.validate()?;
    let view = super::parse_view_center(world, observer, pilot, kind, arguments)?;
    let map = &world.btech.maps()[&view.map];
    let (x, width, y, height) = rectangle(
        view.center,
        map.width,
        map.height,
        i64::from(view.maximum_range),
        kind,
        dimensions,
    );
    Ok(BattleViewport {
        map: view.map,
        requested_center: view.center,
        origin: BattleHexCoordinate {
            x: i32::try_from(x)?,
            y: i32::try_from(y)?,
        },
        width: u16::try_from(width)?,
        height: u16::try_from(height)?,
        maximum_range: view.maximum_range,
    })
}

/// Compute in widened arithmetic so extreme projected centers cannot overflow clipping.
fn rectangle(
    center: BattleHexCoordinate,
    map_width: i64,
    map_height: i64,
    radius: i64,
    kind: BattleViewKind,
    dimensions: BattleViewDimensions,
) -> (i64, i64, i64, i64) {
    let (width, height) = match kind {
        BattleViewKind::Tactical => (
            i64::from(dimensions.tactical_width)
                .min(2 * radius)
                .min(map_width),
            i64::from(dimensions.tactical_height)
                .min(2 * radius)
                .min(map_height),
        ),
        BattleViewKind::LongRange => {
            let height = i64::from(dimensions.long_range_height)
                .min(2 * radius)
                .min(map_height);
            (
                71.min(map_width),
                (height + i64::from(height % 2 == 0)).min(map_height),
            )
        }
    };
    clip_rectangle(center, map_width, map_height, width, height)
}

/// Clip an already bounded rectangle without applying a scanner hardware limit.
fn clip_rectangle(
    center: BattleHexCoordinate,
    map_width: i64,
    map_height: i64,
    width: i64,
    height: i64,
) -> (i64, i64, i64, i64) {
    let x = (i64::from(center.x) - width / 2).clamp(0, map_width - width);
    let y = (i64::from(center.y) - height / 2).clamp(0, map_height - height);
    (x, width, y, height)
}

/// Resolve a map-side viewport from player preferences; zero range denotes no scanner.
pub(super) fn map_viewport(
    map: ObjectId,
    record: &super::StoredBattleMap,
    center: BattleHexCoordinate,
    dimensions: BattleViewDimensions,
) -> Result<BattleViewport> {
    dimensions.validate()?;
    record.validate()?;
    ensure!(record.terrain_ready(), "Map terrain is unavailable");
    let center = BattleHexCoordinate {
        x: i32::try_from(i64::from(center.x).clamp(0, record.width - 1))?,
        y: i32::try_from(i64::from(center.y).clamp(0, record.height - 1))?,
    };
    let (x, width, y, height) = clip_rectangle(
        center,
        record.width,
        record.height,
        i64::from(dimensions.tactical_width).min(record.width),
        i64::from(dimensions.tactical_height).min(record.height),
    );
    Ok(BattleViewport {
        map,
        requested_center: center,
        origin: BattleHexCoordinate {
            x: i32::try_from(x)?,
            y: i32::try_from(y)?,
        },
        width: u16::try_from(width)?,
        height: u16::try_from(height)?,
        maximum_range: 0,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// LRS makes the requested height odd before clipping visible rows to the map.
    #[test]
    fn long_range_odd_height_respects_short_maps_and_edges() {
        for (requested, map_height, radius, expected) in [
            (10, 100, 30, 11),
            (11, 100, 30, 11),
            (40, 100, 30, 41),
            (40, 100, 8, 17),
            (40, 10, 30, 10),
            (40, 11, 30, 11),
            (10, 1, 30, 1),
        ] {
            for y in [0, map_height / 2, map_height - 1] {
                let (_, _, top, height) = rectangle(
                    BattleHexCoordinate { x: 0, y: y as i32 },
                    100,
                    map_height,
                    radius,
                    BattleViewKind::LongRange,
                    BattleViewDimensions {
                        long_range_height: requested,
                        ..BattleViewDimensions::default()
                    },
                );
                assert_eq!(height, expected);
                assert!(top >= 0 && top + height <= map_height);
            }
        }
    }

    #[test]
    fn dimensions_clip_at_edges_and_preserve_long_range_inclusive_span() {
        let dimensions = BattleViewDimensions {
            tactical_width: 40,
            tactical_height: 24,
            long_range_height: 40,
        };
        for center in [
            BattleHexCoordinate {
                x: i32::MIN,
                y: i32::MIN,
            },
            BattleHexCoordinate {
                x: i32::MAX,
                y: i32::MAX,
            },
        ] {
            for kind in [BattleViewKind::Tactical, BattleViewKind::LongRange] {
                let (x, width, y, height) = rectangle(center, 3, 2, 8, kind, dimensions);
                assert_eq!((x, width, y, height), (0, 3, 0, 2));
            }
        }
        let center = BattleHexCoordinate { x: 100, y: 100 };
        assert_eq!(
            rectangle(center, 300, 300, 8, BattleViewKind::Tactical, dimensions),
            (92, 16, 92, 16)
        );
        assert_eq!(
            rectangle(center, 300, 300, 8, BattleViewKind::LongRange, dimensions),
            (65, 71, 92, 17)
        );
        assert_eq!(
            rectangle(center, 1, 1, 8, BattleViewKind::LongRange, dimensions),
            (0, 1, 0, 1)
        );
    }
}
