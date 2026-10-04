//! Deterministic startup/shutdown transitions driven by committed one-second simulation steps.
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Engine lifecycle stored with the unit; countdowns measure committed simulation seconds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum Power {
    #[default]
    Off,
    Starting {
        remaining: u8,
    },
    Running,
}

impl Power {
    /// Commit one startup second and record the supplied Unix time only on completion.
    pub(super) fn advance_startup(
        &mut self,
        last_startup: &mut i64,
        preferences: &mut super::auxiliary_preferences::AuxiliaryPreferences,
        now: i64,
    ) -> Option<u8> {
        let Self::Starting { remaining } = *self else {
            return None;
        };
        let remaining = remaining.saturating_sub(1);
        *self = if remaining == 0 {
            *last_startup = now;
            preferences.player_killer = false;
            Self::Running
        } else {
            Self::Starting { remaining }
        };
        Some(remaining)
    }
}

/// Domain notification addressed to the occupants of one unit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Notice {
    pub unit: ObjectId,
    pub text: String,
}

/// Begin normal startup, or the five-second operator override authorized by the adapter.
pub fn start_unit(world: &mut World, id: ObjectId, pilot: ObjectId, fast: bool) -> Result<Notice> {
    start_unit_by_actor(
        world,
        id,
        super::combat_operator::ControlActor::Player(pilot),
        fast,
    )
}

/// Start a unit through an attached autopilot.  Startup still uses the ordinary
/// map, heat, damage, and lifecycle checks; only cockpit ownership is replaced.
pub(crate) fn start_unit_autopilot(world: &mut World, id: ObjectId, fast: bool) -> Result<Notice> {
    start_unit_by_actor(
        world,
        id,
        super::combat_operator::ControlActor::Autopilot,
        fast,
    )
}

fn start_unit_by_actor(
    world: &mut World,
    id: ObjectId,
    actor: super::combat_operator::ControlActor,
    fast: bool,
) -> Result<Notice> {
    ensure!(
        world.btech.towed_by(id).is_none(),
        "Detach tow cables before starting"
    );
    if world.btech.vehicles().contains_key(&id) {
        return super::vehicle_power::start_by_actor(world, id, actor, fast);
    }
    controlled_unit_by_actor(world, id, actor)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(
        unit.power == Power::Off,
        "Unit is already running or starting"
    );
    let position = unit
        .position()
        .context("Place the unit on a battlefield first")?;
    ensure!(
        world
            .objects
            .get(&position.map)
            .is_some_and(|map| !map.flags.contains(Flag::Going)),
        "Battlefield is unavailable"
    );
    world
        .btech
        .maps()
        .get(&position.map)
        .context("Map not found")?
        .base_hex(i64::from(position.x), i64::from(position.y))?;
    ensure!(!unit.is_destroyed(), "Destroyed unit cannot start");
    ensure!(
        unit.heat().excess <= 30.0,
        "This 'Mech is too hot to start back up!"
    );
    world.btech.constructed.get_mut(&id).unwrap().power = Power::Starting {
        remaining: if fast { 5 } else { 30 },
    };
    world
        .btech
        .constructed
        .get_mut(&id)
        .unwrap()
        .limb_recycle
        .clear();
    if let super::combat_operator::ControlActor::Player(pilot) = actor {
        super::pilot_health::synchronize(world, id, pilot);
    }
    Ok(Notice {
        unit: id,
        text: "Startup Cycle commencing...".to_owned(),
    })
}

/// Abort startup or shut down a running unit, releasing the pilot as part of power-down.
pub fn stop_unit(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    rules: super::FallRules,
) -> Result<Vec<Notice>> {
    check_shutdown_control(world, id, pilot)?;
    stop_admitted(world, id, rules)
}

/// Shared cockpit authority for mechanical and host shutdown entry points.
pub(super) fn check_shutdown_control(world: &World, id: ObjectId, pilot: ObjectId) -> Result<()> {
    controlled(world, id, pilot)
}

/// Apply shutdown after the enclosing action establishes authority over the unit.
/// Pickup uses carrier authority; ordinary shutdown requires the unit's own pilot.
pub(super) fn stop_admitted(
    world: &mut World,
    id: ObjectId,
    rules: super::FallRules,
) -> Result<Vec<Notice>> {
    stop_inner(world, id, rules, None)
}

/// Shutdown admitted by an enclosing host action that owns secondary consequences.
pub(super) fn stop_admitted_in_action(
    world: &mut World,
    id: ObjectId,
    rules: super::FallRules,
    effects: &mut super::shutdown::ShutdownEffects,
) -> Result<Vec<Notice>> {
    stop_inner(world, id, rules, Some(effects))
}

/// One shutdown transition for both mechanical callers and character-capable host actions.
fn stop_inner(
    world: &mut World,
    id: ObjectId,
    mut rules: super::FallRules,
    mut effects: Option<&mut super::shutdown::ShutdownEffects>,
) -> Result<Vec<Notice>> {
    if world.btech.vehicles().contains_key(&id) {
        return super::vehicle_power::stop_admitted(world, id, rules, effects);
    }
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit is unavailable")?;
    let free_fall = if unit.flight().is_some() {
        let position = unit.position().context("Airborne unit is not placed")?;
        let tile = world.btech.maps()[&position.map]
            .base_hex(i64::from(position.x), i64::from(position.y))?;
        Some(super::FreeFall::new(unit.elevation_level(tile)))
    } else {
        None
    };
    ensure!(
        unit.power != Power::Off,
        "The unit has not been started yet"
    );
    let starting = matches!(unit.power, Power::Starting { .. });
    let moving = unit.motion().is_some_and(|motion| motion.speed > 10.75);
    let twisted = unit.facing().torso != super::Torso::Center;
    rules.toughness = unit
        .pilot()
        .and_then(|pilot| world.btech.character_values().get(&pilot))
        .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
    world.attempt(|world| {
        let mut notices = vec![Notice {
            unit: id,
            text: (if starting {
                "The startup sequence has been aborted."
            } else {
                "All systems shut down."
            })
            .to_owned(),
        }];
        if !starting {
            world.btech.constructed.get_mut(&id).unwrap().facing.torso = super::Torso::Center;
            if twisted {
                notices.push(Notice {
                    unit: id,
                    text: "Torso rotated back to center for shutdown".to_owned(),
                });
            }
            if let Some(fall) = free_fall {
                let unit = world.btech.constructed.get_mut(&id).unwrap();
                unit.flight = None;
                unit.free_fall = Some(fall);
                notices.push(Notice {
                    unit: id,
                    text: "You start free-fall.. Enjoy the ride!".to_owned(),
                });
            } else if moving {
                notices.push(Notice {
                    unit: id,
                    text: "Your systems stop in mid-motion!".to_owned(),
                });
                notices.extend(
                    super::observer_messages(world, id, "stops in mid-motion, and falls!")
                        .into_iter()
                        .map(|(unit, text)| Notice { unit, text }),
                );
                let fall = if effects.is_some()
                    && world.objects[&id].flags.contains(crate::Flag::InCharacter)
                {
                    super::fall::resolve_character_fall(world, id, 1, rules)?
                } else {
                    super::resolve_fall(world, id, 1, rules)?
                };
                if let Some(effects) = effects.as_deref_mut() {
                    fall.append_notices(id, &mut notices, &mut effects.pilot_notices);
                } else {
                    notices.extend(fall.notices(id));
                }
                if let Some(effects) = effects.as_deref_mut() {
                    effects.falls.push(fall);
                }
                let input = super::stacking::physical_input(world, id, super::StackingEntry::Fall)?;
                let collisions = if let Some(effects) = effects {
                    super::stacking::resolve_in_action(
                        world,
                        id,
                        input,
                        rules.stacking,
                        rules,
                        &mut effects.stacking,
                        (&mut effects.pilot_notices, notices.len()),
                    )?
                } else {
                    super::resolve_stacking(world, id, input, rules.stacking, rules)?
                };
                notices.extend(collisions);
            }
        }
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        let dropped = finish_shutdown(unit, id, &mut notices);
        if dropped {
            notices.extend(super::club::dropped_notices(world, id));
        }
        world.btech.validate_action(world)?;
        Ok(notices)
    })
}

/// Shared control cleanup after the caller has resolved any location-dependent consequences.
pub(super) fn finish_shutdown(
    unit: &mut super::Mech,
    id: ObjectId,
    notices: &mut Vec<Notice>,
) -> bool {
    if let Some(motion) = &mut unit.motion {
        motion.speed = 0.0;
        motion.desired_speed = 0.0;
        motion.desired_heading = motion.heading;
    }
    let dropped = unit.carried_club.take().is_some();
    unit.power = Power::Off;
    if unit.searchlight.shutdown() {
        notices.push(Notice {
            unit: id,
            text: "Your searchlight shuts off.".into(),
        });
    }
    unit.hide_elapsed = None;
    unit.masc.shutdown();
    unit.supercharger.shutdown();
    if unit.tag.stop() {
        notices.push(Notice {
            unit: id,
            text: "Your TAG connection has been broken.".into(),
        });
    }
    unit.reconcile_electronics();
    unit.charge.target = None;
    unit.jump_stabilization = 0;
    unit.target_lock = None;
    unit.stand_timer = None;
    unit.hull_down.cancel();
    unit.pilot = None;
    dropped
}

/// Advance one simulation second at the supplied Unix time. The caller commits state and notices atomically.
pub fn advance_units(world: &mut World, now: i64) -> Vec<Notice> {
    super::radio_experience::advance(world);
    let mut notices = super::scenario_map::advance_detached(world);
    notices.extend(super::vehicle_power::advance(world, now));
    notices.extend(super::dig::advance(world));
    notices.extend(super::hull_down::advance(world));
    notices.extend(super::vehicle_pods::advance(world));
    notices.extend(super::crew_recovery::advance(world));
    notices.extend(super::vehicle_control_damage::advance(world));
    notices.extend(super::vehicle_turret::advance(world));
    notices.extend(super::lateral::advance(world));
    let pending: Vec<_> = world
        .btech
        .constructed_units()
        .iter()
        .filter_map(|(&id, unit)| {
            let Power::Starting { .. } = unit.power else {
                return None;
            };
            world
                .objects
                .get(&id)
                .filter(|object| !object.flags.contains(Flag::Going))
                .map(|_| {
                    let perception = unit
                        .pilot
                        .filter(|pilot| {
                            world
                                .objects
                                .get(pilot)
                                .is_some_and(|object| object.kind == crate::Kind::Player)
                        })
                        .map_or(6, |pilot| {
                            super::perception_target(world, pilot).expect("validated player")
                        });
                    let radio_skill = super::radio::startup_skill(world, unit.pilot);
                    (
                        id,
                        perception,
                        radio_skill,
                        super::sixth_sense::startup(world, unit.pilot),
                    )
                })
        })
        .collect();
    if pending.is_empty() {
        return notices;
    }
    for (id, perception, radio_skill, sixth_sense) in pending {
        let remaining;
        {
            let unit = world.btech.constructed.get_mut(&id).unwrap();
            remaining = unit
                .power
                .advance_startup(&mut unit.last_startup, &mut unit.auxiliary_preferences, now)
                .expect("pending startup");
            if remaining == 0 {
                unit.scanner_perception = perception;
                unit.sixth_sense.enabled = sixth_sense;
                unit.radio_skill = radio_skill;
                unit.radio_experience_remaining = 0;
            }
        }
        let text = match remaining {
            25 => "Main reactor is now online.",
            20 => "Gyros are now stable.",
            15 => "Main computer system is now online.",
            10 => "Scanners are now operational.",
            5 => "Targeting system is now operational.",
            0 => "All systems operational!",
            _ => continue,
        };
        notices.push(Notice {
            unit: id,
            text: text.to_owned(),
        });
        if remaining == 0 {
            notices.extend(
                super::observer_messages(world, id, "powers up!")
                    .into_iter()
                    .map(|(unit, text)| Notice { unit, text }),
            );
            // Shutdown extinguishes the lamp; restore whatever its mode asks for.
            super::searchlight::reconcile(world, id);
        }
    }
    notices
}

/// Require the assigned, physically present pilot of either chassis. The checks are
/// those of [`controlled_unit`] and its vehicle counterpart, which also guard a caller
/// that handles only one chassis.
pub(super) fn controlled(world: &World, id: ObjectId, pilot: ObjectId) -> Result<()> {
    if world.btech.vehicles().contains_key(&id) {
        return super::vehicle_power::controlled(world, id, pilot);
    }
    controlled_unit(world, id, pilot)
}

/// Admit a cockpit operator or the autopilot to either chassis.
pub(super) fn controlled_by_actor(
    world: &World,
    id: ObjectId,
    actor: super::combat_operator::ControlActor,
) -> Result<()> {
    if world.btech.vehicles().contains_key(&id) {
        return super::vehicle_power::controlled_by_actor(world, id, actor);
    }
    controlled_unit_by_actor(world, id, actor)
}

/// Admit either chassis while the ground autopilot holds it.
pub(super) fn autopilot_controlled(world: &World, id: ObjectId) -> Result<()> {
    if world.btech.vehicles().contains_key(&id) {
        return autopilot_controlled_vehicle(world, id);
    }
    autopilot_controlled_unit(world, id)
}

/// Require the currently assigned, physically present pilot of an available unit.
pub(super) fn controlled_unit(world: &World, id: ObjectId, pilot: ObjectId) -> Result<()> {
    controlled_unit_by_actor(
        world,
        id,
        super::combat_operator::ControlActor::Player(pilot),
    )
}

/// Shared cockpit/autopilot admission for ordinary mechanical actions.
pub(super) fn controlled_unit_by_actor(
    world: &World,
    id: ObjectId,
    actor: super::combat_operator::ControlActor,
) -> Result<()> {
    if let super::combat_operator::ControlActor::Autopilot = actor {
        return autopilot_controlled_unit(world, id);
    }
    let super::combat_operator::ControlActor::Player(pilot) = actor else {
        unreachable!()
    };
    control_health(world, pilot)?;
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?;
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    ensure!(
        unit.pilot() == Some(pilot)
            && world.objects.get(&pilot).is_some_and(
                |player| player.location == Some(id) && !player.flags.contains(Flag::Going)
            ),
        "Take the cockpit with pilot first"
    );
    Ok(())
}

/// Admit a unit attached to the ground autopilot.  The controller is the
/// internal actor; it does not need a player object in the cockpit.
pub(super) fn autopilot_controlled_unit(world: &World, id: ObjectId) -> Result<()> {
    ensure!(
        world.btech.controllers().contains_key(&id),
        "Unit has no attached autopilot"
    );
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Unit construction state is unavailable")?;
    ensure!(!unit.is_destroyed(), "Unit is destroyed");
    // Autopilot replaces cockpit ownership, but it does not bypass the
    // ordinary pilot-health and crew-recovery gates.  An assigned pilot can
    // still become unconscious while the controller remains attached; an
    // uncrewed unit continues to use the normal no-pilot skill path.
    if let Some(pilot) = unit.pilot() {
        control_health(world, pilot)?;
    }
    ensure!(unit.crew_recovery().remaining == 0, "Crew is unconscious");
    Ok(())
}

pub(super) fn autopilot_controlled_vehicle(world: &World, id: ObjectId) -> Result<()> {
    ensure!(
        world.btech.controllers().contains_key(&id),
        "Unit has no attached autopilot"
    );
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|object| !object.flags.contains(Flag::Going)),
        "Unit is unavailable"
    );
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle state is unavailable")?;
    ensure!(!vehicle.is_destroyed(), "Unit is destroyed");
    // Keep autopilot admission aligned with the staffed vehicle controls:
    // replacing the driver does not make an assigned unconscious pilot or
    // recovering crew operational.  With no assigned pilot, the existing
    // uncrewed-vehicle behavior remains available.
    if let Some(pilot) = vehicle.pilot() {
        control_health(world, pilot)?;
    }
    ensure!(
        vehicle.crew_recovery().remaining == 0,
        "Crew is unconscious"
    );
    Ok(())
}

/// Shared operator health gate for cockpit control.
pub(super) fn control_health(world: &World, pilot: ObjectId) -> Result<()> {
    ensure!(!world.btech.unconscious(pilot), "You are unconscious");
    Ok(())
}

/// Cockpit authority and live placement precede native target decoding for every chassis.
pub(super) fn controlled_running_unit(
    world: &World,
    shooter: ObjectId,
    pilot: ObjectId,
) -> Result<()> {
    controlled(world, shooter, pilot)?;
    require_running_unit(world, shooter)
}

/// Physical combat availability shared by cockpit and autopilot operators.
pub(super) fn require_running_unit(world: &World, shooter: ObjectId) -> Result<()> {
    let unit = super::scanner::scanner_unit(world, shooter).context("Unit is unavailable")?;
    ensure!(!unit.destroyed, "Unit is destroyed");
    ensure!(unit.power == super::Power::Running, "Unit must be started");
    ensure!(unit.position.is_some(), "Unit must be on a map");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::UnitTemplateExt;
    use crate::{Config, Kind, Recovery, UnitTemplate};

    fn config() -> Config {
        Config::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"))
            .expect("test configuration")
    }

    fn attach(world: &mut World, unit: ObjectId) {
        world
            .btech
            .controllers
            .insert(unit, super::super::autopilot::AutopilotController::new());
    }

    fn unconscious_pilot(world: &mut World, config: &Config) -> ObjectId {
        let pilot = world.create(config, "Unconscious autopilot pilot".into(), Kind::Player);
        let mut recovery = Recovery::fresh();
        recovery.remaining = 1;
        world.btech.recoveries.insert(pilot, recovery);
        pilot
    }

    #[test]
    fn autopilot_unit_admission_retains_pilot_health_and_uncrewed_behavior() {
        let config = config();
        let mut world = World::default();
        let unit = world.create(&config, "Autopilot health mech".into(), Kind::Thing);
        UnitTemplate::parse("JR7-D", include_str!("../../game/mechs/JR7-D.toml"))
            .unwrap()
            .create(&mut world, unit)
            .unwrap();
        attach(&mut world, unit);

        // An uncrewed unit follows the ordinary no-pilot skill path.
        assert!(autopilot_controlled_unit(&world, unit).is_ok());

        let pilot = unconscious_pilot(&mut world, &config);
        world.btech.constructed.get_mut(&unit).unwrap().pilot = Some(pilot);
        let error = autopilot_controlled_unit(&world, unit).unwrap_err();
        assert!(error.to_string().contains("unconscious"));
    }

    #[test]
    fn autopilot_vehicle_admission_retains_pilot_health_and_uncrewed_behavior() {
        let config = config();
        let mut world = World::default();
        let unit = world.create(&config, "Autopilot health vehicle".into(), Kind::Thing);
        UnitTemplate::parse(
            "Demolisher",
            include_str!("../../game/mechs/Demolisher.toml"),
        )
        .unwrap()
        .create(&mut world, unit)
        .unwrap();
        attach(&mut world, unit);

        assert!(autopilot_controlled_vehicle(&world, unit).is_ok());

        let pilot = unconscious_pilot(&mut world, &config);
        world.btech.vehicles.get_mut(&unit).unwrap().pilot = Some(pilot);
        let error = autopilot_controlled_vehicle(&world, unit).unwrap_err();
        assert!(error.to_string().contains("unconscious"));
    }
}
