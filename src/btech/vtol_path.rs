//! Ordered aircraft terrain traversal and atomic clear-path movement commits.
use super::{
    BattleVehicle, BattleVtolMotionStep, BattleVtolSurfaceContact, HexCoordinate, MapAsset, Point,
};
use anyhow::{Context, Result};
use serde::Serialize;

/// A blocked path leaves the aircraft unchanged so the host can resolve consequences atomically.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
#[must_use = "Publish movement or resolve the first obstruction in the enclosing flight action"]
pub enum BattleVtolPath {
    Advanced {
        step: BattleVtolMotionStep,
    },
    MapEdge {
        hex: HexCoordinate,
        /// Last traced in-bounds hex, before the first attempted exit.
        last: HexCoordinate,
        /// Continuous altitude at the first attempted exit.
        altitude: f64,
    },
    Contact {
        hex: HexCoordinate,
        /// Representative point at or just inside the contact band.
        point: Point,
        altitude: f64,
        contact: BattleVtolSurfaceContact,
    },
}

impl BattleVtolMotionStep {
    /// Find the first map edge or terrain contact, including altitude changes within a hex.
    /// Integer-altitude boundaries are sampled on both sides to retain truncation semantics.
    fn first_obstruction(
        self,
        lookup: &impl Fn(HexCoordinate) -> Result<Option<super::Hex>>,
    ) -> Result<Option<BattleVtolPath>> {
        let start = self.origin.0.point;
        let end = self.motion.point;
        let initial = self.origin.1.altitude;
        let change = self.altitude - initial;
        let mut last = None;
        for (hex, from, to) in start.trace_intervals(end)? {
            let Some(tile) = lookup(hex)? else {
                return Ok(Some(BattleVtolPath::MapEdge {
                    hex,
                    last: last.context("Aircraft starts outside the map")?,
                    altitude: initial + change * from,
                }));
            };
            let entered = last.is_some_and(|previous| previous != hex);
            last = Some(hex);
            let entry_altitude = initial + change * from;
            let blocks_elevation = if tile.is_ice() {
                (entry_altitude as i32) < i32::from(tile.water_line())
            } else {
                tile.blocks_jump_entry(entry_altitude as i32)
            };
            let entry_contact = if tile.is_woods()
                && (entry_altitude as i32) < i32::from(tile.surface_height()) + 2
            {
                Some(BattleVtolSurfaceContact::Forest)
            } else if blocks_elevation {
                Some(BattleVtolSurfaceContact::Elevation)
            } else {
                None
            };
            if let Some(contact) = entry_contact.filter(|_| entered) {
                return Ok(Some(BattleVtolPath::Contact {
                    hex,
                    point: Point {
                        x: start.x + (end.x - start.x) * from,
                        y: start.y + (end.y - start.y) * from,
                    },
                    altitude: entry_altitude,
                    contact,
                }));
            }
            let mut times = vec![from, to];
            if change != 0.0 {
                let surface = f64::from(tile.surface_height());
                let water_line = f64::from(tile.water_line());
                for height in [
                    water_line - 1.0,
                    water_line,
                    surface,
                    surface - 1.0,
                    surface - 2.0,
                ] {
                    let t = (height - initial) / change;
                    // Work in parameter space: a next-representable height may round away
                    // when converted back to time near the start of a long movement segment.
                    for t in [t.next_down(), t, t.next_up()] {
                        if (from..=to).contains(&t) {
                            times.push(t);
                        }
                    }
                }
            }
            times.sort_by(f64::total_cmp);
            times.dedup();
            // Interior representatives cover open altitude bands even when boundary
            // interpolation rounds to an adjacent integer elevation.
            let interiors: Vec<_> = times
                .windows(2)
                .map(|pair| (pair[0] + pair[1]) / 2.0)
                .collect();
            times.extend(interiors);
            times.sort_by(f64::total_cmp);
            for time in times {
                let mut sample = self;
                sample.altitude = initial + change * time;
                let contact = sample.surface_contact(tile);
                if contact == BattleVtolSurfaceContact::Clear {
                    continue;
                }
                return Ok(Some(BattleVtolPath::Contact {
                    hex,
                    point: Point {
                        x: start.x + (end.x - start.x) * time,
                        y: start.y + (end.y - start.y) * time,
                    },
                    altitude: sample.altitude,
                    contact,
                }));
            }
        }
        Ok(None)
    }
}

impl BattleVehicle {
    /// Advance an unobstructed aircraft event against the supplied current map asset.
    /// Map identity, horizontal control updates, fuel, effects and collision resolution belong to the host.
    /// Any obstruction returns before mutation; failures also preserve position and saved altitude.
    pub fn advance_vtol_clear_path(
        &mut self,
        map: &MapAsset,
        movement_modifier: i64,
    ) -> Result<BattleVtolPath> {
        self.advance_vtol_clear_path_with(&|hex| Ok(map.hex(hex.x, hex.y)), movement_modifier, None)
    }

    /// Trace either decoded assets or current world terrain without copying a map.
    pub(super) fn advance_vtol_clear_path_with(
        &mut self,
        lookup: &impl Fn(HexCoordinate) -> Result<Option<super::Hex>>,
        movement_modifier: i64,
        boundary: Option<&super::StoredBattleMap>,
    ) -> Result<BattleVtolPath> {
        let step = self.vtol_motion_step(movement_modifier)?;
        if let Some(mut obstruction) = step.first_obstruction(lookup)? {
            if let Some(map) = boundary
                && let BattleVtolPath::Contact { hex, point, .. } = &mut obstruction
            {
                let resolved = map.motion_hex(*hex)?;
                if resolved != *hex {
                    *hex = resolved;
                    *point = resolved.center();
                }
            }
            return Ok(obstruction);
        }
        let point = match boundary {
            Some(map) => map.motion_destination(step.motion.point)?,
            None => step.motion.point,
        };
        self.commit_vtol_motion_at(step, point)?;
        Ok(BattleVtolPath::Advanced { step })
    }
}
