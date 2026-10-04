//! Structure inspection with hidden-building perception, durable dice and atomic experience output.
use super::{DiagnosticMessage, HexCoordinate};
use crate::{Config, Flag, ObjectId, Scripts, World};
use anyhow::{Context, Result};
use serde::Serialize;

/// Captured structure scan output and accepted perception diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BuildingScan {
    /// Cockpit text; undiscovered and missing structures use the same reply.
    pub text: String,
    /// Accepted perception XP diagnostics to publish with the report.
    pub experience_messages: Vec<DiagnosticMessage>,
}

/// Resolve a building scan atomically, including any eligible concealment roll and XP award.
/// Explicit building coordinates use scanner range even for observers. Invisible,
/// missing and unavailable interiors never roll or disclose construction state.
pub fn scan_building(
    world: &mut World,
    observer: ObjectId,
    pilot: ObjectId,
    coordinate: HexCoordinate,
    now: i64,
) -> Result<BuildingScan> {
    scan_with_range(world, observer, pilot, coordinate, now, false)
}

/// Resolve a selected or explicit coordinate without changing any target lock.
pub(super) fn scan_with_range(
    world: &mut World,
    observer: ObjectId,
    pilot: ObjectId,
    coordinate: HexCoordinate,
    now: i64,
    observer_range: bool,
) -> Result<BuildingScan> {
    let observer = super::combat_operator::for_owner(world, observer, pilot)?
        .source
        .unit;
    world.attempt(|world| {
        let report = resolve(world, observer, pilot, coordinate, now, observer_range)?;
        Ok(report)
    })
}

/// Inspect the first entrance only; a concealed first entry does not expose later duplicates.
fn resolve(
    world: &mut World,
    observer: ObjectId,
    pilot: ObjectId,
    coordinate: HexCoordinate,
    now: i64,
    observer_range: bool,
) -> Result<BuildingScan> {
    let map = super::scan::check_coordinate(world, observer, pilot, coordinate, observer_range)?;
    let mut report = BuildingScan {
        text: "The sensors detect no building in the hex!".into(),
        experience_messages: Vec::new(),
    };
    let Some(entrance) = world.btech.maps()[&map].building_at(coordinate)? else {
        return Ok(report);
    };
    let Some(interior) = world.btech.maps().get(&entrance.interior) else {
        return Ok(report);
    };
    let Some(object) = world
        .objects
        .get(&entrance.interior)
        .filter(|o| !o.flags.contains(Flag::Going))
    else {
        return Ok(report);
    };
    let building = interior.building;
    let name = object.name.clone();
    if building.is_invisible() {
        return Ok(report);
    }
    if building.is_hidden() {
        let unit =
            super::scanner::scanner_unit(world, observer).context("Scanner is unavailable")?;
        let position = unit.position.context("Scanner is not placed")?;
        let range = (f64::from(position.x) - f64::from(coordinate.x))
            .hypot(f64::from(position.y) - f64::from(coordinate.y))
            .hypot(f64::from(
                super::unit_elevation(world, observer)?.unwrap_or(0),
            ));
        let difficulty = (range + 0.95).floor() as i64;
        let (success, message) =
            super::perception_check::attempt(world, observer, pilot, difficulty, now)?;
        if !success {
            return Ok(report);
        }
        report.experience_messages.extend(message);
    }
    report.text = format!("The {name}'s CF is {}.", building.integrity);
    Ok(report)
}

/// Publish structure results to the cockpit and diagnostics together; restore all effects on failure.
pub fn scan_building_action(
    scripts: &Scripts,
    config: &Config,
    observer: ObjectId,
    pilot: ObjectId,
    coordinate: HexCoordinate,
) -> Result<BuildingScan> {
    action_with_range(scripts, config, observer, pilot, coordinate, false)
}

/// Publish a selected or explicit building scan with the matching observer distance policy.
pub(super) fn action_with_range(
    scripts: &Scripts,
    config: &Config,
    observer: ObjectId,
    pilot: ObjectId,
    coordinate: HexCoordinate,
    observer_range: bool,
) -> Result<BuildingScan> {
    let source = super::combat_operator::for_owner(&scripts.world(), observer, pilot)?.source;
    scripts.atomic(|_| {
        let report = scan_with_range(
            &mut scripts.world.borrow_mut(),
            observer,
            pilot,
            coordinate,
            crate::clock::wall_time(),
            observer_range,
        )?;
        super::channels::publish(scripts, config, &report.experience_messages)?;
        super::notify_unit_text(scripts, source.unit, &report.text)?;
        Ok(report)
    })
}
