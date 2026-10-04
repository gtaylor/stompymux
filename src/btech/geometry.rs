//! Measurements between placed units: range, bearing, elevation and altitude. The hex
//! geometry they build on lives in `stompymux-map`.
use super::HexCoordinate;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

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
    let a = HexCoordinate {
        x: i32::from(first.x),
        y: i32::from(first.y),
    };
    let b = HexCoordinate {
        x: i32::from(second.x),
        y: i32::from(second.y),
    };
    let horizontal = first_point.range(second_point)?;
    let dz = first_height.unwrap_or(height(first.x, first.y)?)
        - second_height.unwrap_or(height(second.x, second.y)?);
    Ok(BattleRange {
        horizontal,
        // Horizontal range is finite and nonnegative: hypot(horizontal, ±0) is exact.
        spatial: if dz == 0.0 {
            horizontal
        } else {
            horizontal.hypot(dz)
        },
        bearing: first_point.bearing_after_range(second_point),
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

/// Whether a placed unit is below the water surface of its hex.
pub(super) fn unit_submerged(world: &World, id: ObjectId) -> Result<bool> {
    let position = super::scanner::scanner_unit(world, id)
        .context("Unit construction state is unavailable")?
        .position
        .context("Unit is not placed")?;
    let tile = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?
        .base_hex(i64::from(position.x), i64::from(position.y))?;
    let elevation = unit_elevation(world, id)?.context("Unit is not placed")?;
    Ok(tile.immerses(elevation))
}

impl super::BattleUnit {
    /// Continuous altitude for geometry and external transport; terrain effects retain their integer resolver.
    pub(super) fn altitude(&self, tile: super::Hex) -> f64 {
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
    pub(super) fn altitude(&self, tile: super::Hex) -> f64 {
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
