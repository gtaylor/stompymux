//! Bounded, read-only artillery interception prediction using the live chassis motion proposals.
use super::{BattleMovementRules, BattleVehicleMovement, HexCoordinate, Point};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// Predicted horizontal impact point; terrain stops freeze the last valid predicted location.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct BattleArtilleryPrediction {
    pub coordinate: HexCoordinate,
    pub point: Point,
    pub seconds: u16,
    pub stopped: bool,
}

/// Predict until shell travel time catches up with elapsed target motion, without consuming dice.
/// Target controls and damage remain fixed inputs. Terrain stops do not simulate future damage,
/// other units, changes of orders, vertical motion, map transfers or an autopilot.
pub fn predict_artillery_target(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
    rules: BattleMovementRules,
) -> Result<BattleArtilleryPrediction> {
    let source = super::scanner::scanner_unit(world, shooter).context("Shooter is unavailable")?;
    let other = super::scanner::scanner_unit(world, target).context("Target is unavailable")?;
    let origin = source.point.context("Shooter is not placed")?;
    let position = other.position.context("Target is not placed")?;
    ensure!(
        source.position.is_some_and(|p| p.map == position.map),
        "Target is not on this battlefield"
    );
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map is unavailable")?;
    let mech = world.btech.constructed_units().get(&target);
    let vehicle = world.btech.vehicles().get(&target);
    let mut motion = mech
        .and_then(|unit| unit.motion())
        .or_else(|| vehicle.and_then(|unit| unit.motion()))
        .context("Target motion is unavailable")?;
    let mut coordinate = HexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    };
    let mut stopped = false;
    // Valid map points cannot exceed this diagonal, with margin for staggered hex edges.
    let limit = (((map.width as f64).hypot(map.height as f64) + 4.0) / 5.0)
        .ceil()
        .max(10.0) as u16;
    for seconds in 0..=limit {
        if super::artillery::flight_seconds(origin, motion.point)? <= seconds {
            return Ok(BattleArtilleryPrediction {
                coordinate,
                point: motion.point,
                seconds,
                stopped,
            });
        }
        if stopped {
            continue;
        }
        let (next, destination, immobilized) = if mech.is_some() {
            let proposal =
                super::propose_mech_ground_motion(world, target, motion, coordinate, rules)?;
            (proposal.motion, proposal.destination, proposal.immobilized)
        } else {
            let next =
                super::propose_vehicle_ground_motion(world, target, motion, coordinate, rules)?;
            (next, next.point, vehicle.unwrap().maximum_speed() == 0.0)
        };
        if immobilized {
            stopped = true;
            continue;
        }
        let start = motion.point;
        motion = next;
        motion.point = start;
        for (cell, point) in start.trace_positions(destination)? {
            if cell == coordinate {
                continue;
            }
            let Ok(tile) = map.base_hex(i64::from(cell.x), i64::from(cell.y)) else {
                stopped = true;
                break;
            };
            let previous = map.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
            coordinate = cell;
            motion.point = point;
            let movement = vehicle.map(|unit| unit.definition().movement);
            let elevation = |tile: super::Hex| {
                let height = tile.surface_height();
                if movement == Some(BattleVehicleMovement::Hover) {
                    height.max(tile.water_line())
                } else {
                    height
                }
            };
            let floodable = mech.is_some_and(|unit| {
                tile.is_open_water()
                    && tile.water_depth() > 0
                    && unit.sections().iter().any(|(section, state)| {
                        let exposed =
                            tile.water_depth() > 1 || unit.chassis().legs().contains(section);
                        exposed
                            && state.internal > 0
                            && (state.armor == 0
                                || (state.rear == 0
                                    && unit.definition().sections[section].rear > 0))
                    })
            });
            stopped = (tile.woods() == Some(super::Woods::Heavy) && vehicle.is_some())
                || (tile.is_open_water()
                    && matches!(
                        movement,
                        Some(BattleVehicleMovement::Tracked | BattleVehicleMovement::Wheeled)
                    ))
                || floodable
                || (movement != Some(BattleVehicleMovement::Vtol)
                    && (elevation(tile) - elevation(previous)).abs()
                        > if mech.is_some() { 2 } else { 1 });
            if stopped {
                break;
            }
        }
        if !stopped {
            motion.point = destination;
        }
    }
    anyhow::bail!("Artillery prediction exceeds battlefield time bound")
}
