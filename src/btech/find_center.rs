//! Precise cockpit navigation from continuous motion to the current hex center.
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// A read-only navigation measurement and its shared native/Lua text.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleHexCenterReport {
    pub coordinate: super::HexCoordinate,
    pub elevation: i32,
    pub range: f64,
    pub bearing: u16,
    pub text: String,
}

/// Measure horizontal distance and clockwise bearing without requiring scanner hardware.
/// A coincident position uses the established south-facing (180 degree) readout convention.
pub fn find_center(
    world: &World,
    unit: ObjectId,
    pilot: ObjectId,
) -> Result<BattleHexCenterReport> {
    let source = super::brief::display_source(world, unit, pilot)?;
    let unit = source.unit;
    super::combat_operator::controlled(world, unit, pilot)?;
    let record = super::scanner::scanner_unit(world, unit).context("Unit is unavailable")?;
    ensure!(
        record.power == super::BattlePower::Running && !record.destroyed,
        "Start the unit first"
    );
    let position = record.position.context("Unit is not on a battlefield")?;
    let coordinate = super::HexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y),
    };
    let point = record.point.context("Unit has no motion state")?;
    let (range, bearing) = measurement(point, coordinate)?;
    let elevation = super::unit_elevation(world, unit)?.context("Unit has no elevation")?;
    Ok(BattleHexCenterReport {
        coordinate,
        elevation,
        range,
        bearing,
        text: format!(
            "Current hex: ({},{},{})\tRange to center: {:.2}\tBearing to center: {}",
            coordinate.x, coordinate.y, elevation, range, bearing
        ),
    })
}

/// Geometry shared by cockpit navigation and administrative fields, without access checks.
pub(super) fn measurement(
    point: super::Point,
    coordinate: super::HexCoordinate,
) -> Result<(f64, u16)> {
    let range = point.range(coordinate.center())?;
    let bearing = point
        .bearing(coordinate.center())?
        .unwrap_or(180.0)
        .round()
        .rem_euclid(360.0) as u16;
    Ok((range, bearing))
}

/// Native cockpit entry point; extra arguments have no effect on the current-hex measurement.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    _input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let world = ctx.scripts.world.borrow();
        let unit = world
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        find_center(&world, unit, ctx.player)
    })();
    Ok(crate::CommandAction::Report(match result {
        Ok(report) => crate::CommandReport::Reply(report.text),
        Err(error) => crate::CommandReport::Reply(format!("{error:#}")),
    }))
}
