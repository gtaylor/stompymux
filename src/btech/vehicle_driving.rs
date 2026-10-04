//! Live ground-vehicle controls and traced travel with explicit boundaries for unresolved hazards.
use super::{Motion, MovementRules, Notice, Power, VehicleMovement};
use crate::{Flag, Kind, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// A traced entry retains its terrain hazards until earlier entries have resolved.
struct VehicleStep {
    coordinate: super::HexCoordinate,
    point: super::Point,
    under_bridge: bool,
    change: i32,
    water_check: bool,
    bridge_collision: bool,
    support_height: i16,
    ice_check: bool,
}

/// Read motion for a conscious player physically inside an available, running vehicle.
pub(super) fn readout(world: &World, id: ObjectId, viewer: ObjectId) -> Result<Motion> {
    readout_by_actor(
        world,
        id,
        super::combat_operator::ControlActor::Player(viewer),
    )
}

pub(super) fn readout_by_actor(
    world: &World,
    id: ObjectId,
    actor: super::combat_operator::ControlActor,
) -> Result<Motion> {
    if let super::combat_operator::ControlActor::Autopilot = actor {
        super::power::autopilot_controlled_vehicle(world, id)?;
    }
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    if let super::combat_operator::ControlActor::Player(viewer) = actor {
        ensure!(!world.btech.unconscious(viewer), "You are unconscious");
        ensure!(
            world
                .objects
                .get(&viewer)
                .is_some_and(|object| object.kind == Kind::Player
                    && object.location == Some(id)
                    && !object.flags.contains(Flag::Going)),
            "Enter the unit first"
        );
    }
    let vehicle = &world.btech.vehicles()[&id];
    ensure!(
        vehicle.power() == Power::Running && !vehicle.is_destroyed(),
        "Start the unit first"
    );
    vehicle.motion().context("Unit is not placed")
}

pub(crate) fn set_control_by_actor(
    world: &mut World,
    id: ObjectId,
    actor: super::combat_operator::ControlActor,
    speed: Option<f64>,
    heading: Option<f64>,
    policy: super::SpeedPolicy,
    free_fusion_fuel: bool,
) -> Result<Notice> {
    super::vehicle_power::controlled_by_actor(world, id, actor)?;
    super::fortification::require_mobile(world, id)?;
    let mut motion = readout_by_actor(world, id, actor)?;
    let vehicle = &world.btech.vehicles()[&id];
    ensure!(
        speed.is_none() || (vehicle.free_fall().is_none() && vehicle.orbital_drop().is_none()),
        "Wait until the vehicle lands"
    );
    let maximum = super::motion_controls::throttle_configured(world, id, policy)?;
    if let Some(flight) = vehicle.vtol_flight() {
        // Grounded pilots may select throttle and heading without enabling taxi travel.
        // Heading remains available without fuel; only throttle spends the flight budget.
        ensure!(
            speed.is_none() || vehicle.has_vtol_fuel(free_fusion_fuel),
            "You're out of fuel!"
        );
        ensure!(
            speed.is_none() || flight.phase != super::VtolFlightPhase::Falling,
            "Take off before changing flight controls"
        );
    }
    ensure!(
        vehicle.definition().movement != VehicleMovement::Stationary && maximum > 0.0,
        "Vehicle cannot move"
    );
    let mut text = if let Some(speed) = speed {
        let speed = if let Some(flight) = vehicle.vtol_flight() {
            vehicle.vtol_throttle_at(speed, flight.vertical_speed, maximum)?
        } else {
            speed
        };
        ensure!(
            vehicle.pod_removal().is_none(),
            "You are too busy removing iNARC pods!"
        );
        ensure!(
            speed.is_finite() && speed >= -maximum * 2.0 / 3.0 && speed <= maximum,
            "Speed exceeds unit limits"
        );
        super::motion_controls::require_reverse_allowed(world, id, speed)?;
        ensure!(
            !vehicle.crew_stunned() || speed <= maximum * 2.0 / 3.0 + 0.1,
            "You cannot move faster than cruise speed while stunned!"
        );
        motion.desired_speed = speed;
        super::motion_controls::speed_confirmation(speed)
    } else {
        ensure!(
            !vehicle.dig.dug_in,
            "You are dug in; use speed to leave cover"
        );
        let heading = heading.context("Missing vehicle control request")?;
        ensure!(heading.is_finite(), "Invalid heading");
        motion.desired_heading = heading.rem_euclid(360.0);
        format!("Desired heading: {:.1} degrees.", motion.desired_heading)
    };
    let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
    if speed.is_some_and(|speed| speed.abs() > 0.1) || (heading.is_some() && vehicle.dig.digging) {
        if vehicle.cancel_digging() {
            text = format!("You cease your attempts at digging in.\r\n{text}");
        }
        vehicle.dig = super::DigState::default();
    }
    vehicle.motion = Some(motion);
    Ok(Notice { unit: id, text })
}

/// Resolve supported ground and surface travel in the enclosing movement candidate.
/// Unsupported hazards stop before entry; they must gain their own effects before admission.
pub(super) fn advance(
    world: &mut World,
    rules: MovementRules,
    character: bool,
) -> Result<super::movement_report::MovementReport> {
    let ids: Vec<_> = world
        .btech
        .vehicles()
        .iter()
        .filter_map(|(&id, vehicle)| {
            (!vehicle.definition().is_vtol()
                && vehicle.orbital_drop.is_none()
                && vehicle.free_fall().is_none()
                && vehicle.power() == Power::Running
                && vehicle.motion().is_some_and(Motion::active)
                && world
                    .objects
                    .get(&id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going)))
            .then_some(id)
        })
        .collect();
    let mut report = super::movement_report::MovementReport::default();
    for id in ids {
        let vehicle = &world.btech.vehicles()[&id];
        // Earlier mine blasts can stop another vehicle selected at the beginning of this tick.
        if vehicle.orbital_drop.is_some()
            || vehicle.free_fall().is_some()
            || vehicle.power() != Power::Running
            || vehicle.is_destroyed()
            || !vehicle.motion().is_some_and(Motion::active)
        {
            continue;
        }
        let position = vehicle.position().context("Vehicle is not placed")?;
        let old = vehicle.motion().context("Vehicle lacks motion")?;
        let map = world
            .btech
            .maps()
            .get(&position.map)
            .context("Vehicle map is missing")?;
        let current = map.base_hex(i64::from(position.x), i64::from(position.y))?;
        let mut next = super::propose_vehicle_ground_motion(
            world,
            id,
            old,
            super::HexCoordinate {
                x: i32::from(position.x),
                y: i32::from(position.y),
            },
            rules,
        )?;
        let mut blocked = None;
        let mut edge = false;
        let mut previous_height = vehicle.elevation_level(current);
        let mut previous_tile = current;
        let mut under_bridge = vehicle.under_bridge();
        let reverse_checks = next.speed < 0.0
            && rules.roll_on_backwalk
            && vehicle.definition().movement != VehicleMovement::Tracked;
        let mut entries = Vec::new();
        let traversed = old.point.trace_positions(next.point)?;
        for (hex, point) in traversed {
            let Ok(tile) = map.base_hex(i64::from(hex.x), i64::from(hex.y)) else {
                edge = true;
                blocked = Some("Map edge reached; movement stopped.");
                break;
            };
            if hex.x == i32::from(position.x) && hex.y == i32::from(position.y) {
                continue;
            }
            let bridge_collision = vehicle.definition().movement == VehicleMovement::Hover
                && previous_tile.deck_clearance().is_some_and(|deck| deck != 0)
                && previous_height == i32::from(previous_tile.water_line())
                && tile.deck_clearance() == Some(1);
            let next_under = vehicle.definition().movement == VehicleMovement::Hover
                && previous_height == i32::from(previous_tile.water_line())
                && tile.deck_clearance().is_some_and(|deck| deck >= 2)
                && (under_bridge || previous_tile.is_water_surface());
            let height = if tile.is_ice() && previous_height < i32::from(tile.water_line()) {
                i32::from(tile.surface_height())
            } else {
                vehicle.terrain_elevation(tile, next_under)
            };
            let ice_check = tile.is_ice() && previous_height == i32::from(tile.water_line());
            let change = height - previous_height;
            let water_check = super::vehicle_water::requires_check(vehicle, tile, height);
            entries.push(VehicleStep {
                coordinate: hex,
                point,
                under_bridge: next_under,
                change,
                water_check,
                bridge_collision,
                support_height: i16::try_from(height)?,
                ice_check,
            });
            if change.abs() > 1 || bridge_collision {
                break;
            }
            previous_height = height;
            previous_tile = tile;
            under_bridge = next_under;
        }
        let mut reached = old.point;
        let mut interrupted = false;
        for VehicleStep {
            coordinate: hex,
            point,
            under_bridge: entry_under,
            change,
            water_check,
            bridge_collision,
            support_height,
            ice_check,
        } in entries
        {
            let previous = &world.btech.vehicles()[&id];
            let previous_position = previous.position().unwrap();
            let previous_point = previous.motion().unwrap().point;
            let previous_tile = world.btech.maps()[&position.map].base_hex(
                i64::from(previous_position.x),
                i64::from(previous_position.y),
            )?;
            let previous_height = previous.elevation_level(previous_tile);
            let previous_under = previous.under_bridge();
            reached = point;
            let mut entry_motion = next;
            entry_motion.point = point;
            world.btech.vehicles.get_mut(&id).unwrap().update_motion(
                entry_motion,
                super::Position {
                    map: position.map,
                    x: u16::try_from(hex.x)?,
                    y: u16::try_from(hex.y)?,
                },
                entry_under,
            );
            if world.btech.vehicles()[&id].position() != Some(previous_position) {
                report.notices.extend(super::hiding::movement(world, id));
            }
            let tile =
                world.btech.maps()[&position.map].base_hex(i64::from(hex.x), i64::from(hex.y))?;
            let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
            vehicle.ground_elevation = (i32::from(support_height)
                != vehicle.terrain_elevation(tile, entry_under))
            .then_some(f64::from(support_height));
            let mut fall_rules = rules.fall;
            fall_rules.toughness |= world.btech.vehicles()[&id]
                .pilot()
                .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Toughness"));
            fall_rules.vehicle_impact.criticals.toughness = fall_rules.toughness;
            if ice_check {
                let check = if character {
                    super::surface_break::check_ice_landing_in_action
                } else {
                    super::surface_break::check_ice_landing
                };
                if let Some(fracture) = check(world, id, fall_rules)? {
                    super::piloting::append_feedback(
                        &mut report.pilot_notices,
                        fracture.pilot_notices,
                        report.notices.len(),
                    );
                    report.notices.extend(fracture.notices);
                    report
                        .vehicle_falls
                        .extend(fracture.vehicle_falls.into_iter().map(|(_, fall)| fall));
                    report
                        .falls
                        .extend(fracture.falls.into_iter().map(|(_, fall)| fall));
                    let vehicle = &world.btech.vehicles()[&id];
                    if vehicle.is_destroyed() || vehicle.motion().unwrap().speed == 0.0 {
                        next = vehicle.motion().unwrap();
                        under_bridge = vehicle.under_bridge();
                        interrupted = true;
                        break;
                    }
                    continue;
                }
            }
            if bridge_collision {
                let control = super::terrain_control::check(
                    world,
                    id,
                    super::cliff::modifier(entry_motion.speed, rules.skid_cliff),
                    rules.fall.extended_piloting,
                    character,
                )?;
                report.notices.push(Notice {
                    unit: id,
                    text: "You notice the underside of the bridge in front of you!".into(),
                });
                control.capture_feedback(id, &mut report.notices, &mut report.pilot_notices);
                report
                    .experience_messages
                    .extend(control.experience_messages);
                let (message, broadcast) = if control.success {
                    (
                        "You manage to stop before slamming into the bridge.",
                        "stops suddenly to avoid slamming into the bridge!",
                    )
                } else {
                    (
                        "You drive right into the underside of the bridge.",
                        "drives right into the underside of the bridge.",
                    )
                };
                report.notices.push(Notice {
                    unit: id,
                    text: message.into(),
                });
                report
                    .notices
                    .extend(super::broadcast::observer_notices(world, id, broadcast));
                if !control.success {
                    let fall = super::vehicle_fall::resolve_in_candidate(
                        world, id, 1, fall_rules, character,
                    )?;
                    super::piloting::append_feedback(
                        &mut report.pilot_notices,
                        fall.feedback.pilot_notices.iter().cloned(),
                        report.notices.len(),
                    );
                    report.notices.extend(fall.feedback.notices.iter().cloned());
                    report.vehicle_falls.push(fall);
                }
                let unit = world.btech.vehicles.get_mut(&id).unwrap();
                unit.restore_ground_position(
                    previous_position,
                    previous_point,
                    i16::try_from(previous_height)?,
                    previous_under,
                );
                unit.halt();
                next = unit.motion().unwrap();
                under_bridge = unit.under_bridge();
                interrupted = true;
                break;
            }
            let cliff = change.abs() > 1;
            if cliff || (change != 0 && reverse_checks) {
                let success = if cliff {
                    let (success, movement) =
                        super::vehicle_cliff::check(world, id, change, entry_motion.speed, rules)?;
                    report.extend(movement);
                    success
                } else {
                    let control = super::reverse_slope::check(
                        world,
                        id,
                        i16::try_from(change)?,
                        rules.fall.extended_piloting,
                        character,
                    )?;
                    super::piloting::append_feedback(
                        &mut report.pilot_notices,
                        control.pilot_notices,
                        report.notices.len(),
                    );
                    report.notices.extend(control.notices);
                    report
                        .experience_messages
                        .extend(control.experience_messages);
                    control.success
                };
                if success && !cliff {
                    continue;
                }
                if !success {
                    let levels = if cliff && change > 0 {
                        super::cliff::vehicle_levels(entry_motion.speed, rules.skid_cliff)
                    } else {
                        u8::try_from(change.abs())?
                    };
                    let fall = super::vehicle_fall::resolve_in_candidate(
                        world, id, levels, fall_rules, character,
                    )?;
                    super::piloting::append_feedback(
                        &mut report.pilot_notices,
                        fall.feedback.pilot_notices.iter().cloned(),
                        report.notices.len(),
                    );
                    report.notices.extend(fall.feedback.notices.iter().cloned());
                    report.vehicle_falls.push(fall);
                    let tile = world.btech.maps()[&position.map]
                        .base_hex(i64::from(hex.x), i64::from(hex.y))?;
                    let unit = &world.btech.vehicles()[&id];
                    if cliff
                        && change < 0
                        && tile.is_open_water()
                        && unit.definition().movement != VehicleMovement::Hover
                        && !unit.definition().has_special("Waterproof_Tech")
                    {
                        report
                            .notices
                            .push(super::vehicle_water::flood(world, id, character)?);
                    }
                }
                let unit = world.btech.vehicles.get_mut(&id).unwrap();
                if change > 0 || success {
                    unit.restore_ground_position(
                        previous_position,
                        previous_point,
                        i16::try_from(previous_height)?,
                        previous_under,
                    );
                }
                unit.halt();
                next = unit.motion().unwrap();
                under_bridge = unit.under_bridge();
                interrupted = true;
                break;
            }
            if water_check {
                let entry = super::vehicle_water::enter(
                    world,
                    id,
                    entry_motion.speed,
                    rules.fall,
                    character,
                )?;
                report.extend(entry.movement);
                let unit = world.btech.vehicles.get_mut(&id).unwrap();
                if entry.restore {
                    unit.restore_ground_position(
                        previous_position,
                        previous_point,
                        i16::try_from(previous_height)?,
                        previous_under,
                    );
                }
                unit.halt();
                next = unit.motion().unwrap();
                under_bridge = unit.under_bridge();
                interrupted = true;
                break;
            }
            let tile =
                world.btech.maps()[&position.map].base_hex(i64::from(hex.x), i64::from(hex.y))?;
            let obstacle = super::vehicle_obstacle::resolve(
                world,
                id,
                tile,
                MovementRules {
                    fall: fall_rules,
                    ..rules
                },
                character,
            )?;
            report.extend(obstacle);
            if change.abs() == 1 {
                let motion = world
                    .btech
                    .vehicles
                    .get_mut(&id)
                    .unwrap()
                    .motion
                    .as_mut()
                    .unwrap();
                motion.speed = motion.speed.signum() * (motion.speed.abs() - 21.5).max(0.0);
            }
            let event = super::mine_event::resolve(
                world,
                id,
                super::MineTriggerReason::Step,
                rules.fall,
                character,
            )?;
            super::piloting::append_feedback(
                &mut report.pilot_notices,
                event.pilot_notices.iter().cloned(),
                report.notices.len(),
            );
            report.notices.extend(event.notices.iter().cloned());
            report.mines.push(event);
            // Mines can ignite terrain; inspect the current overlay before its fire check.
            let burning = world.btech.maps()[&position.map]
                .hex(i64::from(hex.x), i64::from(hex.y))?
                .is_burning();
            if burning && rules.fall.vehicle_impact.advanced_fire {
                let before = world.clone();
                let mut criticals = rules.fall.vehicle_impact.criticals;
                criticals.toughness |= world.btech.vehicles()[&id]
                    .pilot()
                    .and_then(|pilot| world.btech.character_values().get(&pilot))
                    .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
                let fire = super::resolve_vehicle_fire_exposure(world, id, criticals)?;
                let previous = &before.btech.vehicles()[&id];
                let current = &world.btech.vehicles()[&id];
                ensure!(
                    character
                        || !world.objects[&id].flags.contains(crate::Flag::InCharacter)
                        || (previous.crew_killed() == current.crew_killed()
                            && previous.character_pilot_status()
                                == current.character_pilot_status()),
                    "Character fire consequences require the movement action"
                );
                for damage in &fire.effects.damage {
                    super::vehicle_injuries::collect_armor(damage, &mut report.character_injuries);
                }
                super::piloting::append_feedback(
                    &mut report.pilot_notices,
                    fire.effects.pilot_notices.iter().cloned(),
                    report.notices.len(),
                );
                report.notices.extend(fire.effects.notices);
                for broadcast in fire.effects.broadcasts {
                    report.notices.extend(super::broadcast::observer_notices(
                        &before,
                        broadcast.unit,
                        &broadcast.text,
                    ));
                }
            }
            let unit = &world.btech.vehicles()[&id];
            let changed = unit.motion().unwrap();
            next.speed = changed.speed;
            next.desired_speed = changed.desired_speed;
            if unit.is_destroyed()
                || unit.immobilized()
                || unit.power() != Power::Running
                || next.speed == 0.0
            {
                next = changed;
                under_bridge = entry_under;
                interrupted = true;
                break;
            }
        }
        if let Some(text) = blocked.filter(|_| !interrupted) {
            if edge {
                report
                    .boundaries
                    .push(super::movement_report::BoundaryCrossing::new(
                        id,
                        position.map,
                        next,
                        text,
                    ));
            }
            next.point = reached;
            next.stop_translation();
            next.desired_heading = next.heading;
            report.notices.push(Notice {
                unit: id,
                text: text.into(),
            });
        }
        let vehicle = world.btech.vehicles.get_mut(&id).unwrap();
        let hex = next.point.containing_hex()?;
        vehicle.update_motion(
            next,
            super::Position {
                map: position.map,
                x: u16::try_from(hex.x)?,
                y: u16::try_from(hex.y)?,
            },
            under_bridge,
        );
        let (notice, experience) = super::building_step::entered(world, id, position)?;
        report.notices.extend(notice);
        report.experience_messages.extend(experience);
    }
    Ok(report)
}
