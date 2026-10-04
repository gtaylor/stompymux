//! Building traversal shares host movement policy, callbacks, rollback and arrival publication.
use super::*;
use crate::{LockInvocation, LockType, ObjectId, Scripts};
use anyhow::{Result, ensure};

/// A committed arrival awaits the tick's normal sensor pass before observer publication.
#[derive(Debug, Clone)]
pub struct BattleBuildingArrival {
    participants: Vec<(ObjectId, crate::state::Generation)>,
    destination: ObjectId,
    message: String,
}

/// Publish only contacts acquired by the ordinary scanner pass, without advancing sensors or dice.
/// Reports belong to the enclosing tick and must be discarded if that tick rolls back.
pub fn publish_building_arrivals(
    scripts: &Scripts,
    arrivals: Vec<BattleBuildingArrival>,
) -> Result<()> {
    transaction(scripts, || {
        for arrival in arrivals {
            for (unit, generation) in arrival.participants {
                let notices = {
                    let world = scripts.world();
                    if world
                        .objects
                        .get(&unit)
                        .is_none_or(|object| object.generation != generation)
                        || super::scanner::scanner_unit(&world, unit)
                            .and_then(|unit| unit.position)
                            .is_none_or(|position| position.map != arrival.destination)
                    {
                        continue;
                    }
                    super::broadcast::observer_notices(&world, unit, &arrival.message)
                };
                for notice in notices {
                    notify_unit(scripts, notice)?;
                }
            }
        }
        Ok(())
    })
}

/// Savepoint for entry callbacks, map state and staged notifications.
fn transaction<T>(scripts: &Scripts, operation: impl FnOnce() -> Result<T>) -> Result<T> {
    scripts.atomic(|_| operation())
}

/// Evaluate the enter lock with explicit command or timer identities.
fn enter_lock(
    scripts: &Scripts,
    id: ObjectId,
    actor: ObjectId,
    destination: ObjectId,
) -> Result<crate::LockOutcome> {
    scripts.evaluate_lock(LockInvocation {
        kind: LockType::Enter,
        object: destination,
        enactor: actor,
        cause: actor,
        subject: id,
        descriptor: None,
        silent: false,
    })
}

/// Read the configured assistance once per host admission or recheck.
fn speed_policy(scripts: &Scripts) -> super::speed_bonus::SpeedPolicy {
    super::speed_bonus::SpeedPolicy::configured(&crate::lua::configuration(&scripts.lua))
}

/// Start the common event after evaluating the live enter lock; denial retains lock feedback.
pub fn begin_building_entry_action(
    scripts: &Scripts,
    id: ObjectId,
    pilot: ObjectId,
    direction: Option<u8>,
) -> Result<bool> {
    transaction(scripts, || {
        let destination = super::building_entry::destination_configured(
            &scripts.world(),
            id,
            pilot,
            direction,
            speed_policy(scripts),
        )?;
        let outcome = enter_lock(scripts, id, pilot, destination.map)?;
        // Lock providers may alter the route or eligibility. Never apply their decision to a new map.
        ensure!(
            super::building_entry::destination_configured(
                &scripts.world(),
                id,
                pilot,
                direction,
                speed_policy(scripts)
            )? == destination,
            "Building route changed during lock evaluation"
        );
        if !building_entry_lock_allows(&scripts.world(), destination.map, outcome.passes)? {
            let ctx = scripts.context(Some(pilot), Some(destination.map), None)?;
            ctx.set("lock", "enter")
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            ctx.set("subject", id.0)
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            ctx.set("cause", pilot.0)
                .map_err(|error| anyhow::anyhow!("{error}"))?;
            scripts.lock_denied(ctx, &outcome, "The hangar is locked.")?;
            return Ok(false);
        }
        super::building_entry::begin_configured(
            &mut scripts.world_mut(),
            id,
            pilot,
            direction,
            outcome.passes,
            speed_policy(scripts),
        )?;
        let notices = {
            let world = scripts.world();
            let position = super::scanner::scanner_unit(&world, id)
                .and_then(|unit| unit.position)
                .expect("admitted placement");
            super::broadcast::hex_notices(
                &world,
                position.map,
                HexCoordinate {
                    x: i32::from(position.x),
                    y: i32::from(position.y),
                },
                false,
                |location| format!("The doors at {location} start to open.."),
            )?
        };
        for notice in notices {
            notify_unit(scripts, notice)?;
        }
        Ok(true)
    })
}

/// Pending entries keep otherwise stationary or shut-down units on the host clock.
pub fn building_entries_pending(world: &crate::World) -> bool {
    world
        .btech
        .constructed_units()
        .values()
        .any(|unit| unit.building_entry.is_some())
        || world
            .btech
            .vehicles()
            .values()
            .any(|unit| unit.building_entry.is_some())
}

/// Advance entry events once, rechecking current routes and locks before ordinary teleport movement.
/// Invalid eligibility consumes the event; callback errors restore the entire entry step for retry.
pub fn advance_building_entries_action(scripts: &Scripts) -> Result<Vec<BattleBuildingArrival>> {
    transaction(scripts, || {
        let mut arrivals = Vec::new();
        let ids: Vec<_> = {
            let world = scripts.world();
            world
                .btech
                .units()
                .keys()
                .copied()
                .filter(|id| building_entry(&world, *id).is_some())
                .collect()
        };
        for id in ids {
            let Some(event) = advance_building_entry(&mut scripts.world_mut(), id)? else {
                continue;
            };
            clear_building_entry(&mut scripts.world_mut(), id)?;
            let pilot = {
                let world = scripts.world();
                world
                    .btech
                    .vehicles()
                    .get(&id)
                    .and_then(BattleVehicle::pilot)
                    .or_else(|| {
                        world
                            .btech
                            .constructed_units()
                            .get(&id)
                            .and_then(BattleUnit::pilot)
                    })
            };
            let Some(pilot) = pilot else {
                continue;
            };
            let destination = super::building_entry::destination_configured(
                &scripts.world(),
                id,
                pilot,
                event.direction,
                speed_policy(scripts),
            );
            let Ok(destination) = destination else {
                continue;
            };
            let outcome = enter_lock(scripts, id, id, destination.map)?;
            if !building_entry_lock_allows(&scripts.world(), destination.map, outcome.passes)? {
                notify_unit(
                    scripts,
                    BattleNotice {
                        unit: id,
                        text: outcome
                            .enactor_message
                            .unwrap_or_else(|| "The hangar is locked.".into()),
                    },
                )?;
                continue;
            }
            if let Some(arrival) = transfer_building(
                scripts,
                id,
                destination,
                BuildingDirection::Entry {
                    pilot,
                    direction: event.direction,
                },
            )? {
                arrivals.push(arrival);
            }
        }
        Ok(arrivals)
    })
}

/// Entry and exit differ in route admission and placement, while host movement remains common.
#[derive(Clone, Copy)]
enum BuildingDirection {
    Entry {
        pilot: ObjectId,
        direction: Option<u8>,
    },
    Exit,
}

impl BuildingDirection {
    /// Recheck the current route after lock and departure callbacks.
    fn destination(
        self,
        world: &crate::World,
        id: ObjectId,
        policy: super::speed_bonus::SpeedPolicy,
    ) -> Result<BattlePosition> {
        match self {
            Self::Entry { pilot, direction } => {
                super::building_entry::destination_configured(world, id, pilot, direction, policy)
            }
            Self::Exit => building_exit_for_unit(world, id),
        }
    }
}

/// Execute either traversal with one teleport-policy and callback sequence.
fn transfer_building(
    scripts: &Scripts,
    id: ObjectId,
    destination: BattlePosition,
    direction: BuildingDirection,
) -> Result<Option<BattleBuildingArrival>> {
    let target = scripts.world().btech.tows().get(&id).copied();
    let participants: Vec<_> = std::iter::once(id)
        .chain(target)
        .map(|unit| (unit, scripts.world().objects[&unit].generation))
        .collect();
    let request = |unit| crate::movement::Request {
        actor: unit,
        object: unit,
        cause: ObjectId(1),
        destination: destination.map,
        session: None,
        route: crate::movement::Route::Teleport,
    };
    let mut feedback = None;
    let relocate = |world: &mut crate::World| {
        ensure!(
            world.btech.tows().get(&id).copied() == target
                && participants.iter().all(|(unit, generation)| world
                    .objects
                    .get(unit)
                    .is_some_and(|object| object.generation == *generation)),
            "Tow participants changed during movement callbacks"
        );
        ensure!(
            direction.destination(world, id, speed_policy(scripts))? == destination,
            "Building route changed during movement callbacks"
        );
        let position = super::scanner::scanner_unit(world, id)
            .and_then(|unit| unit.position)
            .expect("admitted placement");
        let interior = match direction {
            BuildingDirection::Entry { .. } => destination.map,
            BuildingDirection::Exit => position.map,
        };
        let name = super::building_entrance::structure_name(world, interior)?;
        let (departure, arrival, cockpit) = match direction {
            BuildingDirection::Entry { .. } => {
                let message = format!("has entered {name} at {},{}.", position.x, position.y);
                (message.clone(), message, format!("You enter {name}."))
            }
            BuildingDirection::Exit => (
                "has left the hangar.".into(),
                format!("has left {name} at {},{}.", destination.x, destination.y),
                format!("You have left {name}."),
            ),
        };
        let notices = participants
            .iter()
            .flat_map(|(unit, _)| super::broadcast::observer_notices(world, *unit, &departure))
            .collect::<Vec<_>>();
        match direction {
            BuildingDirection::Entry { .. } => transfer_unit(world, id, destination)?,
            BuildingDirection::Exit => {
                super::building_exit::exit_configured(world, id, speed_policy(scripts))?;
            }
        }
        feedback = Some((arrival, cockpit, notices));
        Ok(())
    };
    if let Some(target) = target {
        if !crate::movement::perform_pair_with_relocation(
            scripts,
            [request(id), request(target)],
            relocate,
        )? {
            return Ok(None);
        }
    } else {
        crate::movement::perform_with_relocation(scripts, request(id), relocate)?;
    }
    let Some((message, cockpit, notices)) = feedback else {
        return Ok(None);
    };
    for notice in notices {
        notify_unit(scripts, notice)?;
    }
    notify_unit(
        scripts,
        BattleNotice {
            unit: id,
            text: cockpit,
        },
    )?;
    Ok(Some(BattleBuildingArrival {
        participants,
        destination: destination.map,
        message,
    }))
}

/// Leave through the building's reciprocal route using the common movement transaction.
/// A denied teleport returns no arrival report; callback errors restore state and effects.
/// The caller publishes a successful report after its ordinary scanner pass.
pub fn exit_building_action(
    scripts: &Scripts,
    id: ObjectId,
) -> Result<Option<BattleBuildingArrival>> {
    transaction(scripts, || {
        let destination = building_exit_for_unit(&scripts.world(), id)?;
        let arrival = transfer_building(scripts, id, destination, BuildingDirection::Exit)?;
        if arrival.is_some() {
            warn_unpiloted_exit(scripts, id)?;
        } else {
            notify_unit(
                scripts,
                BattleNotice {
                    unit: id,
                    text: "Unable to leave the hangar: teleportation was denied.".into(),
                },
            )?;
        }
        Ok(arrival)
    })
}

/// Successful in-character exits warn occupants when the cockpit has no present operator.
/// Pilot assignments are cleared on departure; an unassigned cockpit does not execute shutdown.
fn warn_unpiloted_exit(scripts: &Scripts, id: ObjectId) -> Result<()> {
    let missing = {
        let world = scripts.world();
        if !world.objects[&id].flags.contains(crate::Flag::InCharacter) {
            return Ok(());
        }
        let pilot = world
            .btech
            .vehicles()
            .get(&id)
            .and_then(BattleVehicle::pilot)
            .or_else(|| {
                world
                    .btech
                    .constructed_units()
                    .get(&id)
                    .and_then(BattleUnit::pilot)
            });
        pilot
            .and_then(|pilot| world.objects.get(&pilot))
            .is_none_or(|pilot| pilot.location != Some(id))
    };
    if !missing {
        return Ok(());
    }
    for text in [
        "[fg=red bold blink inverse]INTRUDER ALERT! INTRUDER ALERT![reset]",
        "[fg=red bold blink]Automatic self-destruct sequence initiated...[reset]",
    ] {
        notify_unit(
            scripts,
            BattleNotice {
                unit: id,
                text: text.into(),
            },
        )?;
    }
    Ok(())
}

/// Shared text adapter: one byte selects a direction; longer selectors use the first entrance.
pub(crate) fn enterbase(
    scripts: &Scripts,
    id: ObjectId,
    pilot: ObjectId,
    arguments: &str,
) -> Result<bool> {
    let mut arguments = arguments.split_whitespace();
    let selector = arguments.next();
    ensure!(arguments.next().is_none(), "Invalid arguments to command!");
    let direction = selector
        .filter(|value| value.len() == 1)
        .map(|value| value.as_bytes()[0]);
    begin_building_entry_action(scripts, id, pilot, direction)
}

/// Native hangar entry uses the same host action as Lua and the delayed clock.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let id = ctx.scripts.world().objects[&ctx.player]
            .location
            .ok_or_else(|| anyhow::anyhow!("Enter a unit first"))?;
        enterbase(ctx.scripts, id, ctx.player, &input.args)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Try configured building exits after material movement has settled at an unlinked edge.
/// A failed route or policy keeps the stopped boundary state; callback errors abort the host step.
pub(super) fn dispatch_boundary_exits(
    scripts: &Scripts,
    report: &mut super::movement_report::MovementReport,
) -> Result<Vec<BattleBuildingArrival>> {
    let mut arrivals = Vec::new();
    for boundary in std::mem::take(&mut report.boundaries) {
        let eligible = {
            let world = scripts.world();
            super::scanner::scanner_unit(&world, boundary.unit)
                .and_then(|unit| unit.position)
                .is_some_and(|position| position.map == boundary.map)
                && world
                    .btech
                    .maps()
                    .get(&boundary.map)
                    .is_some_and(|map| !map.wrapping() && !map.building_exits().is_empty())
        };
        if !eligible {
            continue;
        }
        if let Err(error) = building_exit_for_unit(&scripts.world(), boundary.unit) {
            report.notices.push(BattleNotice {
                unit: boundary.unit,
                text: error.to_string(),
            });
            continue;
        }
        let stopped = {
            let world = scripts.world();
            world
                .btech
                .vehicles()
                .get(&boundary.unit)
                .and_then(BattleVehicle::motion)
                .or_else(|| {
                    world
                        .btech
                        .constructed_units()
                        .get(&boundary.unit)
                        .and_then(BattleUnit::motion)
                })
                .expect("placed boundary unit")
        };
        let mut running = boundary.motion;
        running.point = stopped.point;
        let previous = super::scanner::scanner_unit(&scripts.world(), boundary.unit)
            .and_then(|unit| unit.position)
            .expect("admitted boundary placement");
        set_boundary_motion(&mut scripts.world_mut(), boundary.unit, running);
        let arrival = exit_building_action(scripts, boundary.unit)?;
        if let Some(arrival) = arrival {
            let (notice, experience) =
                super::building_step::entered(&mut scripts.world_mut(), boundary.unit, previous)?;
            report.notices.extend(notice);
            report.experience_messages.extend(experience);
            if let Some(index) = report
                .notices
                .iter()
                .position(|notice| *notice == boundary.notice)
            {
                report.notices.remove(index);
            }
            arrivals.push(arrival);
        } else {
            set_boundary_motion(&mut scripts.world_mut(), boundary.unit, stopped);
        }
    }
    Ok(arrivals)
}

/// Replace only motion controls while retaining the boundary's last valid coordinate.
fn set_boundary_motion(world: &mut crate::World, id: ObjectId, motion: BattleMotion) {
    if let Some(unit) = world.btech.vehicles.get_mut(&id) {
        unit.motion = Some(motion);
    } else if let Some(unit) = world.btech.constructed.get_mut(&id) {
        unit.motion = Some(motion);
    }
}
