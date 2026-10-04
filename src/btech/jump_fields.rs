//! Atomic administrative jump-course edits reuse the saved flight cursor and landing services.
use super::BattleJumpPath;
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Edit retained course values and, when airborne, redirect only the remaining flight.
/// The enclosing unit-field transaction owns complete world validation and rollback.
pub(super) fn set(world: &mut World, id: ObjectId, heading: bool, value: &str) -> Result<()> {
    let value = value
        .trim()
        .parse::<i16>()
        .context("Invalid jump field integer")?;
    if heading {
        ensure!(
            (0..360).contains(&value),
            "Jump heading must be between 0 and 359"
        );
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Jump course fields require a Mech")?;
    let mut course = unit.last_jump;
    let original = if heading {
        course.heading as i16
    } else {
        course.length
    };
    if original == value {
        return Ok(());
    }
    if heading {
        course.heading = value as u16;
    } else {
        course.length = value;
    }
    let mut flight = unit.flight();
    if let Some(cursor) = flight.as_mut() {
        let remaining = if heading {
            cursor.path().distance() - cursor.travelled()
        } else {
            f64::from(value) / 322.5 - cursor.total_travelled()
        };
        if course.length <= 0 || remaining <= 0.0 || (heading && cursor.landing_requested()) {
            cursor.request_landing();
        } else {
            let position = unit.position().context("Airborne unit is not placed")?;
            let map = world
                .btech
                .maps()
                .get(&position.map)
                .context("Jump map is unavailable")?;
            let origin = cursor.sample();
            let end = origin.point.project(f64::from(course.heading), remaining)?;
            let destination = end.containing_hex()?;
            let elevation = map
                .base_hex(i64::from(destination.x), i64::from(destination.y))?
                .standing_height();
            let path = BattleJumpPath::continuation(
                origin,
                end,
                elevation,
                cursor.path().movement_points(),
            )?;
            super::jumping::validate_route(map, path)?;
            cursor.redirect(path)?;
        }
    }
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    unit.last_jump = course;
    unit.flight = flight;
    Ok(())
}
