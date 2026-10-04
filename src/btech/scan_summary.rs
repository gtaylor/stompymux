//! Shared read-only SCAN and REPORT information rows with reference disclosure and spacing.
use super::{ContactView, LateralMode, MechChassis, VehicleMovement};
use crate::{ObjectId, World, text};
use anyhow::{Context, Result};

/// Render an already-admitted clear contact without acquiring targets or changing unit state.
pub(super) fn render(world: &World, observer: ObjectId, view: &ContactView) -> Result<String> {
    let target = view.target;
    let source = super::scanner::scanner_unit(world, observer).context("Scanner is unavailable")?;
    let unit = super::scanner::scanner_unit(world, target).context("Target is unavailable")?;
    let position = unit.position.context("Target is not placed")?;
    let heading = unit.heading.context("Target has no motion state")?;
    let mech = world.btech.constructed_units().get(&target);
    let vehicle = world.btech.vehicles().get(&target);
    let tons = mech.map_or_else(
        || vehicle.unwrap().definition().tons,
        |unit| unit.definition().tons,
    );
    let heat = mech.map_or(0.0, |unit| unit.heat().excess * 10.0);
    let travel = mech
        .and_then(|unit| unit.travel_heading())
        .unwrap_or(heading)
        .trunc()
        .rem_euclid(360.0) as u16;
    let bearing = view
        .range
        .bearing
        .unwrap_or(180.0)
        .round()
        .rem_euclid(360.0) as u16;
    let name: String = view.name.chars().take(25).collect();
    let mut label = unit.label().context("Target has no battlefield identity")?;
    if view.friendly {
        label.make_ascii_lowercase();
    }
    let mut lines = vec![
        format!("[{label}]  {name:<25} Tonnage: {tons}"),
        format!(
            "      Range: {:.1} hex\t\tBearing: {bearing} degrees",
            view.range.spatial
        ),
        format!(
            "      Speed: {:.1} KPH\t\tHeading: {travel} degrees",
            unit.speed
        ),
    ];
    if let Some(flight) = vehicle.and_then(|unit| unit.vtol_flight()) {
        lines.push(format!(
            "      Vertical speed: {:.1} KPH",
            flight.vertical_speed
        ));
    }
    lines.push(format!(
        "      X, Y, Z: {:3}, {:3}, {:3}\tHeat: {heat:.0} deg C.",
        position.x,
        position.y,
        super::unit_elevation(world, target)?.unwrap_or(0)
    ));
    if let Some(mech) = mech
        && mech.lateral().active != LateralMode::None
    {
        lines.push(format!(
            "      Mech is moving laterally {}",
            mech.lateral().active.description()
        ));
    }
    if let Some(vehicle) = vehicle
        && let Some(turret) = vehicle.turret_heading()
    {
        lines.push(super::status::turret_line(
            turret,
            heading,
            vehicle.definition().movement == VehicleMovement::Stationary,
        ));
    }
    let (kind, movement) = if let Some(mech) = mech {
        (
            "MECH",
            if mech.chassis() == MechChassis::Quad {
                "QUAD"
            } else {
                "BIPED"
            },
        )
    } else {
        match vehicle.unwrap().definition().movement {
            VehicleMovement::Tracked => ("VEHICLE", "TRACKED"),
            VehicleMovement::Wheeled => ("VEHICLE", "WHEELED"),
            VehicleMovement::Hover => ("VEHICLE", "HOVER"),
            VehicleMovement::Stationary => ("INSTALLATION", ""),
            VehicleMovement::Vtol => ("VTOL", "VTOL"),
        }
    };
    lines.push(if movement.is_empty() {
        "      Type: INSTALLATION".into()
    } else {
        format!("      Type: {kind:<20}Movement: {movement}")
    });
    if let Some(turret) = world
        .btech
        .vehicles()
        .get(&observer)
        .and_then(|unit| unit.turret_heading())
        && super::vehicle_arcs::turret_arc(turret, f64::from(bearing))
    {
        lines.push("      In Turret Arc".into());
    }
    let arc = source.facing.contact_arc(
        source.heading.context("Scanner has no heading")?,
        f64::from(bearing),
    )?;
    lines.push(format!(
        "      In {} Weapons Arc",
        arc.description(source.vehicle)
    ));
    let mut lines: Vec<_> = lines.into_iter().map(|line| text::escape(&line)).collect();
    lines.extend(super::status::scan_conditions(world, target)?);
    if let Some(flight) = mech.and_then(|unit| unit.flight()) {
        lines.push(format!(
            "      Mech is Jumping!\tJump Heading: {}",
            flight.path().heading()?
        ));
    }
    lines.push(" ".into());
    Ok(lines.join("\r\n"))
}
