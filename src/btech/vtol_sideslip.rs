//! VTOL sideslips (Total Warfare pp. 54 and 67). A VTOL using flank speed that changes facing
//! and then goes on into a new hex makes an unmodified Driving Skill Roll first. On a failure
//! it drifts along its old course for as many hexes as it missed the roll by, never more than
//! one less than the hexes it has entered, without changing elevation; it takes no damage
//! unless the drift carries it into terrain, a building or treetops at or above its elevation,
//! or down onto water, in which case it crashes there (TW p. 68).
use super::{FallRules, Notice, PilotNotice, VtolFlightPhase, VtolSurfaceContact};
use crate::{ObjectId, World};
use anyhow::{Context, Result};

/// What a sideslip check produced; the flight tick publishes notices and nested crashes.
#[derive(Default)]
pub(super) struct SideslipReport {
    pub notices: Vec<Notice>,
    pub pilot_notices: Vec<PilotNotice>,
    pub experience_messages: Vec<super::DiagnosticMessage>,
    pub vehicle_falls: Vec<super::VehicleFallReport>,
    /// The drift crashed or drowned the VTOL, so its movement this tick is over.
    pub ended: bool,
}

/// Roll to avoid sideslipping as the VTOL leaves the hex it turned in, drifting along
/// `heading`, its course before the turn, when the roll fails.
pub(super) fn check(
    world: &mut World,
    id: ObjectId,
    heading: f64,
    rules: FallRules,
    character: bool,
) -> Result<SideslipReport> {
    let mut report = SideslipReport::default();
    report.notices.push(Notice {
        unit: id,
        text: "You bank hard at flank speed and fight to hold your line!".into(),
    });
    let control = super::terrain_control::check(world, id, 0, rules.extended_piloting, character)?;
    let margin = control.margin_of_failure();
    control.capture_feedback(id, &mut report.notices, &mut report.pilot_notices);
    report
        .experience_messages
        .extend(control.experience_messages);
    if control.success {
        report.notices.push(Notice {
            unit: id,
            text: "You hold your course.".into(),
        });
        return Ok(report);
    }
    let (position, point, speed, altitude) = {
        let unit = &world.btech.vehicles()[&id];
        let motion = unit.motion().context("Sideslipping VTOL has no motion")?;
        (
            unit.position().context("Sideslipping VTOL is not placed")?,
            motion.point,
            motion.speed.abs(),
            unit.vtol_flight()
                .context("Sideslip requires a VTOL")?
                .altitude,
        )
    };
    // A sideslip drifts one hex per point missed by, never farther than one less than the
    // hexes already entered this turn (TW p. 67).
    let hexes = margin.min(((speed / 10.75) as u8).saturating_sub(1));
    report.notices.push(Notice {
        unit: id,
        text: "You lose your line and sideslip!".into(),
    });
    report
        .notices
        .extend(super::broadcast::observer_notices(world, id, "sideslips!"));
    for (hex, _) in super::skid::path(point, heading, hexes)? {
        let Ok(tile) =
            world.btech.maps()[&position.map].base_hex(i64::from(hex.x), i64::from(hex.y))
        else {
            break;
        };
        let contact = super::vtol_path::horizontal_entry_contact(tile, altitude as i32);
        world
            .btech
            .vehicles
            .get_mut(&id)
            .unwrap()
            .place_at(hex, hex.center(), altitude)?;
        report.notices.push(Notice {
            unit: id,
            text: format!("You sideslip into {},{}.", hex.x, hex.y),
        });
        report.notices.extend(super::hiding::movement(world, id));
        let Some(contact) = contact else {
            continue;
        };
        report.ended = true;
        if contact == VtolSurfaceContact::Water {
            report.notices.push(Notice {
                unit: id,
                text: "You sideslip down onto the water and go under!".into(),
            });
            report.notices.extend(super::broadcast::observer_notices(
                world,
                id,
                "sideslips into the water!",
            ));
            let unit = world.btech.vehicles.get_mut(&id).unwrap();
            let flight = unit.vtol_flight.as_mut().unwrap();
            flight.phase = VtolFlightPhase::Landed;
            flight.vertical_speed = 0.0;
            flight.fall = None;
            unit.destroy_by_flooding();
            break;
        }
        let (own, seen) = if contact == VtolSurfaceContact::Forest {
            (
                "CRASH! You sideslip into the trees!",
                "sideslips into the trees and crashes!",
            )
        } else {
            (
                "CRASH! You sideslip into the terrain!",
                "sideslips into an obstacle and crashes!",
            )
        };
        report.notices.push(Notice {
            unit: id,
            text: own.into(),
        });
        report
            .notices
            .extend(super::broadcast::observer_notices(world, id, seen));
        let fall = super::vtol_crash::resolve_signed_in_candidate(world, id, 1, rules, character)?;
        super::piloting::append_feedback(
            &mut report.pilot_notices,
            fall.feedback.pilot_notices.iter().cloned(),
            report.notices.len(),
        );
        report.notices.extend(fall.feedback.notices.iter().cloned());
        report.vehicle_falls.push(fall);
        break;
    }
    Ok(report)
}
