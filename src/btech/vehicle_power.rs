//! Ground-vehicle startup and shutdown share the world heartbeat and cockpit ownership boundary.
use super::{BattleNotice, BattlePower, BattleVehicleMovement};
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

/// Begin the normal thirty-second sequence or the adapter-authorized five-second override.
pub(super) fn start(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    fast: bool,
) -> Result<BattleNotice> {
    start_by_actor(
        world,
        id,
        super::combat_operator::ControlActor::Player(pilot),
        fast,
    )
}

pub(super) fn start_by_actor(
    world: &mut World,
    id: ObjectId,
    actor: super::combat_operator::ControlActor,
    fast: bool,
) -> Result<BattleNotice> {
    controlled_by_actor(world, id, actor)?;
    let unit = &world.btech.vehicles()[&id];
    ensure!(
        unit.power() == BattlePower::Off,
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
    world.btech.vehicles.get_mut(&id).unwrap().power = BattlePower::Starting {
        remaining: if fast { 5 } else { 30 },
    };
    if let super::combat_operator::ControlActor::Player(pilot) = actor {
        super::pilot_health::synchronize(world, id, pilot);
    }
    Ok(BattleNotice {
        unit: id,
        text: "Startup Cycle commencing...".into(),
    })
}

/// Abort startup or shut down, releasing the cockpit assignment.
pub(super) fn stop_admitted(
    world: &mut World,
    id: ObjectId,
    mut rules: super::BattleFallRules,
    effects: Option<&mut super::shutdown::ShutdownEffects>,
) -> Result<Vec<BattleNotice>> {
    let unit = &world.btech.vehicles()[&id];
    ensure!(
        unit.power() != BattlePower::Off,
        "The unit has not been started yet"
    );
    let starting = matches!(unit.power(), BattlePower::Starting { .. });
    let position = unit.position().context("Vehicle is not placed")?;
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    let altitude = unit.elevation_level(tile);
    let airborne = unit.definition().movement != super::BattleVehicleMovement::Stationary
        && altitude > i32::from(tile.standing_height())
        && altitude < 300;
    let moving = unit.motion().is_some_and(|motion| motion.speed > 10.75);
    rules.toughness |= unit
        .pilot()
        .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Toughness"));
    let mut candidate = world.clone();
    let mut notices = vec![BattleNotice {
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
                .is_some_and(|flight| flight.phase == super::BattleVtolFlightPhase::Falling);
        if !already_falling {
            super::vtol_crash::begin_descent(&mut candidate, id)?;
        }
        notices.push(BattleNotice {
            unit: id,
            text: "You start free-fall.. Enjoy the ride!".into(),
        });
    } else if !starting && moving {
        notices.push(BattleNotice {
            unit: id,
            text: "Your systems stop in mid-motion!".into(),
        });
        notices.push(BattleNotice {
            unit: id,
            text: "You tumble end over end and come to a crashing halt!".into(),
        });
        notices.extend(super::broadcast::observer_notices(
            &candidate,
            id,
            "tumbles end over end and comes to a crashing halt!",
        ));
        let fall = super::vehicle_fall::resolve_in_candidate(
            &mut candidate,
            id,
            1,
            rules,
            effects.is_some(),
        )?;
        notices.extend(fall.notices.iter().cloned());
        if let Some(effects) = effects {
            super::piloting::append_feedback(
                &mut effects.pilot_notices,
                fall.pilot_notices.iter().cloned(),
                notices.len() - fall.notices.len(),
            );
            effects.vehicle_falls.push(fall);
        }
    }
    let unit = candidate.btech.vehicles.get_mut(&id).unwrap();
    if unit.searchlight.on {
        notices.push(BattleNotice {
            unit: id,
            text: "Your searchlight shuts off.".into(),
        });
    }
    if unit.tag.stop() {
        notices.push(BattleNotice {
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
pub(super) fn finish_shutdown(unit: &mut super::BattleVehicle) {
    unit.cancel_digging();
    unit.target_lock = None;
    unit.power = BattlePower::Off;
    unit.tag.stop();
    unit.searchlight.shutdown();
    unit.hide_elapsed = None;
    unit.lose_vtol_lift();
    unit.reconcile_electronics();
    unit.halt();
    unit.pilot = None;
}

/// Advance countdowns and capture the operator's perception when startup completes.
pub(super) fn advance(world: &mut World, now: i64) -> Vec<BattleNotice> {
    let perception: std::collections::BTreeMap<_, _> = world
        .btech
        .vehicles()
        .iter()
        .filter(|(_, unit)| matches!(unit.power(), BattlePower::Starting { remaining: 1 }))
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
    for (&id, unit) in world.btech.vehicles.iter_mut() {
        let BattlePower::Starting { .. } = unit.power else {
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
        }
        let text = match (remaining, unit.definition().movement) {
            (25, BattleVehicleMovement::Stationary) => "Main reactor is now online.",
            (25, _) => "Powerplant initialized and online.",
            (20, BattleVehicleMovement::Tracked) => "Auto-aligning drive wheels.",
            (15, BattleVehicleMovement::Tracked) => "Adjusting track tension.",
            (20, BattleVehicleMovement::Wheeled) => "Performing steering system checks.",
            (15, BattleVehicleMovement::Wheeled) => "Checking wheel status.",
            (20, BattleVehicleMovement::Hover) => "Checking plenum chamber status.",
            (15, BattleVehicleMovement::Hover) => "Verifying fan status.",
            (20, BattleVehicleMovement::Stationary) => "Gyros are now stable.",
            (15, BattleVehicleMovement::Stationary) => "Main computer system is now online.",
            (10, _) => "Scanners are now operational.",
            (5, _) => "Targeting system is now operational.",
            (0, _) => "All systems operational!",
            _ => continue,
        };
        notices.push(BattleNotice {
            unit: id,
            text: text.into(),
        });
    }
    notices
}
