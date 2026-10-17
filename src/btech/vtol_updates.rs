//! Aircraft movement events reuse launch, fuel, vehicle controls and contact consequences.
//! Flight follows Total Warfare p. 54: a VTOL on the ground may only climb, horizontal and
//! vertical movement share one budget, flanking turns risk a sideslip (p. 67), and flying
//! into terrain or treetops crashes the VTOL (p. 68).
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Append feedback through the ordinary movement publication boundary.
fn notice(report: &mut super::movement_report::MovementReport, id: ObjectId, text: &str) {
    report.notices.push(Notice {
        unit: id,
        text: text.into(),
    });
}

/// Advance descent first, then launch and powered flight; new lift loss waits until the next tick.
pub(super) fn advance_all(
    world: &mut World,
    rules: MovementRules,
    character: bool,
) -> Result<super::movement_report::MovementReport> {
    let mut report = super::vtol_crash::advance_all(world, rules, character)?;
    let ids: Vec<_> = world
        .btech
        .vehicles()
        .iter()
        .filter_map(|(&id, unit)| {
            (unit.vtol_flight().is_some_and(|flight| {
                matches!(
                    flight.phase,
                    VtolFlightPhase::Launching { .. } | VtolFlightPhase::Airborne
                )
            }) && world
                .objects
                .get(&id)
                .is_some_and(|object| !object.flags.contains(Flag::Going)))
            .then_some(id)
        })
        .collect();
    for id in ids {
        ensure!(
            character || !world.objects[&id].flags.contains(Flag::InCharacter),
            "Character flight requires the host movement transaction"
        );
        let unit = &world.btech.vehicles()[&id];
        let flight = unit
            .vtol_flight()
            .context("Aircraft flight state is unavailable")?;
        let position = unit.position().context("Aircraft is not placed")?;
        ensure!(
            world
                .objects
                .get(&position.map)
                .is_some_and(|object| !object.flags.contains(Flag::Going)),
            "Aircraft map is unavailable"
        );
        let map = world
            .btech
            .maps()
            .get(&position.map)
            .context("Aircraft map is unavailable")?;
        if matches!(flight.phase, VtolFlightPhase::Launching { .. }) {
            let underground = map.has_flag(super::MapFlag::Underground);
            let result = world
                .btech
                .vehicles
                .get_mut(&id)
                .unwrap()
                .advance_vtol_takeoff(underground)?;
            match result {
                VtolTakeoff::Aborted { reason } => {
                    notice(&mut report, id, &format!("Takeoff aborted: {reason}"))
                }
                VtolTakeoff::LiftedOff => {
                    notice(&mut report, id, "You lift off!");
                    report.notices.extend(super::broadcast::observer_notices(
                        world,
                        id,
                        "lifts off!",
                    ));
                }
                _ => {}
            }
            continue;
        }
        if flight.phase != VtolFlightPhase::Airborne {
            continue;
        }
        // An administratively inserted, stopped aircraft waits aloft until startup finishes.
        if unit.power() != Power::Running {
            continue;
        }
        if unit.is_destroyed() || unit.rotor_destroyed() {
            world.btech.vehicles.get_mut(&id).unwrap().lose_vtol_lift();
            continue;
        }
        let mut motion = unit.motion().context("Aircraft motion is unavailable")?;
        let envelope = super::motion_controls::throttle_envelope(
            world,
            id,
            super::SpeedPolicy {
                tsm_tow_bonus: rules.tsm_tow_bonus,
            },
        )?;
        if super::load::carries_load(world, id) {
            motion.limit_load(envelope, envelope);
        }
        if !motion.active() && flight.vertical_speed == 0.0 {
            world.btech.vehicles.get_mut(&id).unwrap().motion = Some(motion);
            continue;
        }
        let speed_demon = unit
            .pilot()
            .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Speed_Demon"));
        let mut fall_rules = rules.fall;
        fall_rules.toughness |= unit
            .pilot()
            .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Toughness"));
        let movement = VehicleMotionRules {
            speed_demon,
            // Fog and buildings slow ground movement, not rotorcraft in flight.
            extra_cost: 0,
        };
        let current = map.base_hex(i64::from(position.x), i64::from(position.y))?;
        let current_hex = super::HexCoordinate {
            x: i32::from(position.x),
            y: i32::from(position.y),
        };
        // A VTOL whose elevation is its hex's level is on the ground: the only movement it
        // may make is to climb at least one elevation above the terrain (TW p. 54).
        let ground = if current.is_water_surface() {
            i32::from(current.water_line())
        } else {
            i32::from(current.surface_height())
        };
        let grounded = (flight.altitude as i32) <= ground;
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        let fuel = unit.consume_vtol_fuel(flight.vertical_speed, flight.altitude as i32, false)?;
        if let VtolFuelUse::Exhausted { newly } = fuel {
            if newly {
                notice(&mut report, id, "You run out of fuel and begin to fall!");
            }
            continue;
        }
        let mut controlled = unit.definition().control_within(
            motion,
            super::Hex::at_level(0),
            movement,
            envelope,
        )?;
        if grounded {
            controlled.speed = 0.0;
        }
        unit.motion = Some(controlled);
        // A flanking VTOL that turns and then goes on into a new hex may sideslip (TW p. 67).
        unit.slick
            .observe(true, controlled.speed != 0.0, controlled.heading);
        let held = unit.slick.heading;
        let leaving = unit.vtol_motion_step()?.motion.point.containing_hex()? != current_hex;
        let cruise =
            super::vtol_speed::horizontal_envelope(envelope, flight.vertical_speed)?.walking;
        if leaving && unit.slick.turned && controlled.speed.abs() > cruise + 0.1 {
            unit.slick.reset();
            let slip = super::vtol_sideslip::check(
                world,
                id,
                held.unwrap_or(controlled.heading),
                fall_rules,
                character,
            )?;
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                slip.pilot_notices,
                report.notices.len(),
            );
            report.notices.extend(slip.notices);
            report.experience_messages.extend(slip.experience_messages);
            report.vehicle_falls.extend(slip.vehicle_falls);
            if slip.ended {
                continue;
            }
        }
        let unit = &world.btech.vehicles()[&id];
        let edge_motion = unit.motion().expect("controlled flight");
        let edge_map = unit.position().expect("placed flight").map;
        let outcome =
            super::vtol_environment::advance_in_candidate(world, id, fall_rules, character)?;
        match outcome {
            VtolEnvironment::Boundary { .. } => {
                report
                    .boundaries
                    .push(super::movement_report::BoundaryCrossing::new(
                        id,
                        edge_map,
                        edge_motion,
                        "You cannot move off this map!",
                    ));
                notice(&mut report, id, "You cannot move off this map!")
            }
            VtolEnvironment::Movement {
                path: VtolPath::Advanced { step },
            } if step.ceiling_reached => {
                notice(
                    &mut report,
                    id,
                    "You cannot climb any higher! Vertical movement halted!",
                );
            }
            VtolEnvironment::Landed { landing, .. } => {
                let effects = super::vtol_controls::landing_consequences(
                    world, id, landing, fall_rules, character,
                )?;
                super::piloting::append_feedback(
                    &mut report.pilot_notices,
                    effects.pilot_notices.iter().cloned(),
                    report.notices.len(),
                );
                report.notices.extend(effects.notices);
                report.mines.extend(effects.mines);
            }
            VtolEnvironment::Flooded { newly: true, .. } => {
                notice(&mut report, id, "You crash your vehicle into the water!");
                notice(
                    &mut report,
                    id,
                    "Water pours into the cockpit....glub glub!",
                );
                report.notices.extend(super::broadcast::observer_notices(
                    world,
                    id,
                    "splashes into the water!",
                ));
            }
            VtolEnvironment::Crashed { fall, path } => {
                let (own, seen) = match path {
                    VtolPath::Contact {
                        contact: VtolSurfaceContact::Forest,
                        ..
                    } => (
                        "CRASH! You fly into the trees!",
                        "flies into the trees and crashes!",
                    ),
                    VtolPath::Contact {
                        contact: VtolSurfaceContact::Elevation,
                        ..
                    } => (
                        "CRASH! You fly straight into the terrain!",
                        "flies into an obstacle and crashes!",
                    ),
                    _ => (
                        "CRASH! You smash your toy into the ground!",
                        "crashes into the ground!",
                    ),
                };
                notice(&mut report, id, own);
                report
                    .notices
                    .extend(super::broadcast::observer_notices(world, id, seen));
                super::piloting::append_feedback(
                    &mut report.pilot_notices,
                    fall.feedback.pilot_notices.iter().cloned(),
                    report.notices.len(),
                );
                report.notices.extend(fall.feedback.notices.iter().cloned());
                report.vehicle_falls.push(*fall);
            }
            VtolEnvironment::CrashRequired { .. } => {
                unreachable!("World contacts resolve crash damage")
            }
            _ => {}
        }
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        if unit.position() != Some(position) {
            unit.slick.reset();
        }
        let (notice, experience) = super::building_step::entered(world, id, position)?;
        report.notices.extend(notice);
        report.experience_messages.extend(experience);
    }
    Ok(report)
}
