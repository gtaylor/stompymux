//! Ground-vehicle startup and shutdown share the world heartbeat and cockpit ownership boundary.
use super::{Notice, Power, VehicleMovement};
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Require a conscious, assigned operator physically inside a live vehicle.
pub(super) fn controlled(world: &World, id: ObjectId, pilot: ObjectId) -> Result<()> {
    controlled_by_actor(
        world,
        id,
        super::combat_operator::ControlActor::Player(pilot),
    )
}

pub(super) fn controlled_by_actor(
    world: &World,
    id: ObjectId,
    actor: super::combat_operator::ControlActor,
) -> Result<()> {
    if let super::combat_operator::ControlActor::Autopilot = actor {
        super::power::autopilot_controlled_vehicle(world, id)?;
        return Ok(());
    }
    let super::combat_operator::ControlActor::Player(pilot) = actor else {
        unreachable!()
    };
    super::power::control_health(world, pilot)?;
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    let unit = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle state is unavailable")?;
    ensure!(
        unit.pilot() == Some(pilot)
            && world.objects.get(&pilot).is_some_and(
                |object| object.location == Some(id) && !object.flags.contains(Flag::Going)
            ),
        "Take the cockpit with pilot first"
    );
    Ok(())
}

pub(super) fn start_by_actor(
    world: &mut World,
    id: ObjectId,
    actor: super::combat_operator::ControlActor,
    fast: bool,
) -> Result<Notice> {
    controlled_by_actor(world, id, actor)?;
    let unit = &world.btech.vehicles()[&id];
    ensure!(
        unit.power() == Power::Off,
        "Unit is already running or starting"
    );
    ensure!(!unit.is_destroyed(), "Destroyed unit cannot start");
    let position = unit
        .position()
        .context("Place the unit on a battlefield first")?;
    ensure!(
        world
            .objects
            .get(&position.map)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Battlefield is unavailable"
    );
    world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?
        .hex(i64::from(position.x), i64::from(position.y))?;
    world.btech.vehicles.get_mut(&id).unwrap().power = Power::Starting {
        remaining: if fast { 5 } else { 30 },
    };
    if let super::combat_operator::ControlActor::Player(pilot) = actor {
        super::pilot_health::synchronize(world, id, pilot);
    }
    Ok(Notice {
        unit: id,
        text: "Startup Cycle commencing...".into(),
    })
}

/// Whether a mobile vehicle is held above the surface that would support it: a flying or
/// falling VTOL, a ground vehicle left unsupported, or one dropping from orbit. Hovercraft and
/// amphibious vehicles are supported by the water they float on.
fn above_support(unit: &super::Vehicle, tile: super::Hex) -> bool {
    unit.definition().movement != super::VehicleMovement::Stationary
        && unit.elevation_level(tile) > unit.terrain_elevation(tile, unit.under_bridge())
}

/// Whether a vehicle is in the air: above the surface that would support it, or dropping from
/// orbit.
fn in_the_air(unit: &super::Vehicle, tile: super::Hex) -> bool {
    unit.orbital_drop().is_some() || above_support(unit, tile)
}

/// A pilot's own shutdown waits until the vehicle has stopped on the ground, as a 'Mech's
/// does (Total Warfare p. 160): a VTOL must land first and a moving vehicle must come to a
/// stop. Neither refusal changes anything; an administrative shutdown is not held back.
pub(super) fn check_shutdown_ready(world: &World, id: ObjectId) -> Result<()> {
    let Some(unit) = world.btech.vehicles().get(&id) else {
        return Ok(());
    };
    if unit.power() == Power::Off {
        return Ok(());
    }
    let position = unit.position().context("Vehicle is not placed")?;
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    ensure!(
        !in_the_air(unit, tile),
        "You must land before you can shut down."
    );
    ensure!(
        unit.motion().is_none_or(|motion| motion.speed == 0.0),
        "You must come to a stop before you can shut down."
    );
    Ok(())
}

/// Abort startup or shut down, releasing the cockpit assignment. A moving vehicle simply
/// stops; one an administrative shutdown catches in the air starts an unpowered descent.
pub(super) fn stop_admitted(world: &mut World, id: ObjectId) -> Result<Vec<Notice>> {
    let unit = &world.btech.vehicles()[&id];
    ensure!(
        unit.power() != Power::Off,
        "The unit has not been started yet"
    );
    let starting = matches!(unit.power(), Power::Starting { .. });
    let position = unit.position().context("Vehicle is not placed")?;
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    let airborne = above_support(unit, tile) && unit.elevation_level(tile) < 300;
    let mut candidate = world.clone();
    let mut notices = vec![Notice {
        unit: id,
        text: if starting {
            "The startup sequence has been aborted."
        } else {
            "All systems shut down."
        }
        .into(),
    }];
    if !starting && airborne {
        let already_falling = unit.free_fall().is_some()
            || unit
                .vtol_flight()
                .is_some_and(|flight| flight.phase == super::VtolFlightPhase::Falling);
        if !already_falling {
            super::vtol_crash::begin_descent(&mut candidate, id)?;
        }
        notices.push(Notice {
            unit: id,
            text: "You start free-fall.. Enjoy the ride!".into(),
        });
    }
    let unit = candidate.btech.vehicles.get_mut(&id).unwrap();
    if unit.searchlight.on {
        notices.push(Notice {
            unit: id,
            text: "Your searchlight shuts off.".into(),
        });
    }
    if unit.tag.stop() {
        notices.push(Notice {
            unit: id,
            text: "Your TAG connection has been broken.".into(),
        });
    }
    finish_shutdown(unit);
    candidate.btech.validate_action(&candidate)?;
    *world = candidate;
    Ok(notices)
}

/// Shared control cleanup after location-dependent shutdown consequences have been resolved.
pub(super) fn finish_shutdown(unit: &mut super::Vehicle) {
    unit.target_lock = None;
    unit.power = Power::Off;
    unit.tag.stop();
    unit.searchlight.shutdown();
    unit.hide_elapsed = None;
    unit.lose_vtol_lift();
    unit.reconcile_electronics();
    unit.halt();
    unit.pilot = None;
}

/// Advance countdowns and capture the operator's perception when startup completes.
pub(super) fn advance(world: &mut World, now: i64) -> Vec<Notice> {
    let perception: std::collections::BTreeMap<_, _> = world
        .btech
        .vehicles()
        .iter()
        .filter(|(_, unit)| matches!(unit.power(), Power::Starting { remaining: 1 }))
        .map(|(&id, unit)| {
            let target = unit
                .pilot()
                .filter(|pilot| {
                    world
                        .objects
                        .get(pilot)
                        .is_some_and(|object| object.kind == crate::Kind::Player)
                })
                .map_or(super::scanner::default_perception(), |pilot| {
                    super::perception_target(world, pilot).expect("validated player")
                });
            (
                id,
                (
                    target,
                    super::radio::startup_skill(world, unit.pilot()),
                    super::sixth_sense::startup(world, unit.pilot()),
                ),
            )
        })
        .collect();
    let mut notices = Vec::new();
    let mut online = Vec::new();
    for (&id, unit) in world.btech.vehicles.iter_mut() {
        let Power::Starting { .. } = unit.power else {
            continue;
        };
        if world
            .objects
            .get(&id)
            .is_none_or(|object| object.flags.contains(Flag::Going))
        {
            continue;
        }
        let remaining = unit
            .power
            .advance_startup(&mut unit.last_startup, &mut unit.auxiliary_preferences, now)
            .expect("pending startup");
        if remaining == 0 {
            unit.scanner_perception = perception[&id].0;
            unit.sixth_sense.enabled = perception[&id].2;
            unit.radio_skill = perception[&id].1;
            unit.radio_experience_remaining = 0;
            online.push(id);
        }
        let text = match (remaining, unit.definition().movement) {
            (25, VehicleMovement::Stationary) => "Main reactor is now online.",
            (25, _) => "Powerplant initialized and online.",
            (20, VehicleMovement::Tracked) => "Auto-aligning drive wheels.",
            (15, VehicleMovement::Tracked) => "Adjusting track tension.",
            (20, VehicleMovement::Wheeled) => "Performing steering system checks.",
            (15, VehicleMovement::Wheeled) => "Checking wheel status.",
            (20, VehicleMovement::Hover) => "Checking plenum chamber status.",
            (15, VehicleMovement::Hover) => "Verifying fan status.",
            (20, VehicleMovement::Stationary) => "Gyros are now stable.",
            (15, VehicleMovement::Stationary) => "Main computer system is now online.",
            (10, _) => "Scanners are now operational.",
            (5, _) => "Targeting system is now operational.",
            (0, _) => "All systems operational!",
            _ => continue,
        };
        notices.push(Notice {
            unit: id,
            text: text.into(),
        });
    }
    // Shutdown extinguishes the lamp; restore whatever its mode asks for.
    for id in online {
        super::searchlight::reconcile(world, id);
    }
    notices
}
