//! Shared opposite-edge coordinate resolution preserves traversal order across map seams.
use super::{HexCoordinate, Point, StoredBattleMap};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

/// Authored wrapping marker; its presence enables wrapping independently of its payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BattleLinkedMarker {
    pub coordinate: HexCoordinate,
    pub object: ObjectId,
    pub data_char: u8,
    pub data_short: i16,
    pub data_int: i64,
}

impl BattleLinkedMarker {
    /// Create the marker used by SETLINKED, with the reference's initialized payload.
    pub fn new(coordinate: HexCoordinate) -> Self {
        Self {
            coordinate,
            object: ObjectId(0),
            data_char: 1,
            data_short: 0,
            data_int: 0,
        }
    }
}

impl StoredBattleMap {
    /// Any linked marker enables wrapping; positions are retained for map-object selection.
    pub fn wrapping(&self) -> bool {
        !self.linked_markers.is_empty()
    }

    /// Stable marker slots and their authored coordinates, which need not lie on the map.
    pub fn linked_markers(&self) -> &std::collections::BTreeMap<u32, BattleLinkedMarker> {
        &self.linked_markers
    }

    /// Resolve a virtual tile to its opposite-edge tile when wrapping is enabled.
    pub(super) fn motion_hex(&self, hex: HexCoordinate) -> Result<HexCoordinate> {
        match self.wrapping_dimensions()? {
            Some(wrapping) => Ok(wrapping.hex(hex)),
            None => Ok(hex),
        }
    }

    /// Snapshot dimensions for a saved flight sample; movement refreshes this each tick.
    pub(super) fn wrapping_dimensions(&self) -> Result<Option<MapWrapping>> {
        if !self.wrapping() {
            return Ok(None);
        }
        Ok(Some(MapWrapping::new(self.width, self.height)?))
    }

    /// Boundary crossings settle at the destination hex center; ordinary motion stays continuous.
    pub(super) fn motion_destination(&self, point: Point) -> Result<Point> {
        match self.wrapping_dimensions()? {
            Some(wrapping) => wrapping.point(point),
            None => Ok(point),
        }
    }

    /// Trace in virtual coordinates before resolving tiles, avoiding a false path across the map interior.
    pub(super) fn trace_motion(
        &self,
        start: Point,
        end: Point,
    ) -> Result<Vec<(HexCoordinate, Point)>> {
        start
            .trace_positions(end)?
            .into_iter()
            .map(|(hex, point)| {
                let resolved = self.motion_hex(hex)?;
                Ok((
                    resolved,
                    if resolved == hex {
                        point
                    } else {
                        resolved.center()
                    },
                ))
            })
            .collect()
    }
}

/// Set saved opposite-edge wrapping without changing terrain, membership or unit positions.
pub fn set_map_wrapping(world: &mut World, id: ObjectId, wrapping: bool) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Map is unavailable"
    );
    let mut candidate = world
        .btech
        .maps()
        .get(&id)
        .context("Map not found")?
        .clone();
    if wrapping && !candidate.wrapping() {
        Arc::make_mut(&mut candidate.linked_markers)
            .insert(0, BattleLinkedMarker::new(HexCoordinate { x: 0, y: 0 }));
    } else if !wrapping {
        candidate.linked_markers = Default::default();
    }
    install_markers(world, id, candidate)
}

/// Set or remove one linked marker without discarding other markers or their payloads.
pub fn set_linked_marker(
    world: &mut World,
    id: ObjectId,
    ordinal: u32,
    coordinate: Option<HexCoordinate>,
) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Map is unavailable"
    );
    let mut candidate = world
        .btech
        .maps()
        .get(&id)
        .context("Map not found")?
        .clone();
    if let Some(coordinate) = coordinate {
        let mut marker = candidate
            .linked_markers
            .get(&ordinal)
            .copied()
            .unwrap_or_else(|| BattleLinkedMarker::new(coordinate));
        marker.coordinate = coordinate;
        Arc::make_mut(&mut candidate.linked_markers).insert(ordinal, marker);
    } else {
        Arc::make_mut(&mut candidate.linked_markers).remove(&ordinal);
    }
    install_markers(world, id, candidate)
}

/// Preserve active jump admission when a marker edit changes boundary behavior.
fn install_markers(world: &mut World, id: ObjectId, candidate: StoredBattleMap) -> Result<()> {
    candidate.validate()?;
    for unit in world.btech.constructed_units().values() {
        if unit.position().is_some_and(|position| position.map == id)
            && let Some(flight) = unit.flight()
        {
            super::jumping::validate_route(&candidate, flight.path())
                .context("Wrapping change would invalidate an active jump")?;
        }
    }
    world.btech.maps.get_mut(&id).unwrap().linked_markers = candidate.linked_markers;
    Ok(())
}

/// Dimensions used to resolve a saved virtual flight sample without copying terrain.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(try_from = "WrappingRecord")]
pub(super) struct MapWrapping {
    width: i32,
    height: i32,
}

/// Validate persisted divisors before coordinate arithmetic.
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct WrappingRecord {
    width: i64,
    height: i64,
}

impl TryFrom<WrappingRecord> for MapWrapping {
    type Error = anyhow::Error;
    fn try_from(record: WrappingRecord) -> Result<Self> {
        Self::new(record.width, record.height)
    }
}

impl MapWrapping {
    /// Match the representable battlefield dimensions.
    fn new(width: i64, height: i64) -> Result<Self> {
        ensure!(
            (1..=i64::from(u16::MAX)).contains(&width)
                && (1..=i64::from(u16::MAX)).contains(&height),
            "Invalid wrapping map dimensions"
        );
        Ok(Self {
            width: width as i32,
            height: height as i32,
        })
    }

    /// Resolve both axes, including negative coordinates and multiple crossings.
    fn hex(self, hex: HexCoordinate) -> HexCoordinate {
        HexCoordinate {
            x: hex.x.rem_euclid(self.width),
            y: hex.y.rem_euclid(self.height),
        }
    }

    /// Keep continuous positions until a virtual sample crosses a boundary.
    pub(super) fn point(self, point: Point) -> Result<Point> {
        let hex = point.containing_hex()?;
        let resolved = self.hex(hex);
        Ok(if resolved == hex {
            point
        } else {
            resolved.center()
        })
    }

    /// A saved sample must use its actual battlefield dimensions.
    pub(super) fn matches(self, map: &StoredBattleMap) -> bool {
        i64::from(self.width) == map.width && i64::from(self.height) == map.height
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_dimensions_validate_and_resolve_negative_and_repeated_crossings() -> Result<()> {
        for (width, height) in [(0, 1), (1, 0), (-1, 3), (1, 65536)] {
            assert!(
                serde_json::from_value::<MapWrapping>(
                    serde_json::json!({"width":width,"height":height})
                )
                .is_err()
            );
        }
        let wrapping = MapWrapping::new(3, 4)?;
        for hex in [
            HexCoordinate { x: -1, y: -1 },
            HexCoordinate { x: 7, y: 9 },
            HexCoordinate { x: -7, y: -9 },
        ] {
            let expected = HexCoordinate {
                x: hex.x.rem_euclid(3),
                y: hex.y.rem_euclid(4),
            };
            assert_eq!(wrapping.point(hex.center())?, expected.center());
            let restored: MapWrapping = serde_json::from_value(serde_json::to_value(wrapping)?)?;
            assert_eq!(restored.point(hex.center())?, expected.center());
        }
        Ok(())
    }
}
