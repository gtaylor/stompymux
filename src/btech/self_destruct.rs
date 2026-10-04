//! Cockpit self-destruction uses a shared durable countdown and existing blast/casualty actions.
use super::*;
use crate::{
    CommandAction, CommandContext, CommandInput, CommandReport, Config, ObjectId, Scripts, World,
};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// An admitted sequence; advertised intent is separate from the event's actual detonation mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelfDestruct {
    pub remaining: u16,
    /// Mech detonation mode and the shared ammunition-presence cancellation condition.
    pub ammunition: bool,
    /// Nonpositive configured delays display negative intermediate countdowns.
    negative: bool,
    /// Stable order among pending sequences, including after restart.
    order: u64,
}

impl SelfDestruct {
    /// The event expires at the next lower 256-second boundary, including nonpositive settings.
    fn scheduled(delay: i64, order: u64) -> Self {
        let remaining = delay.rem_euclid(256) as u16;
        Self {
            remaining: if remaining == 0 { 256 } else { remaining },
            ammunition: delay > 256,
            negative: delay <= 0,
            order,
        }
    }

    /// Reject malformed timers independently of current power and map placement.
    pub(super) fn validate(self) -> Result<()> {
        ensure!(
            (1..=256).contains(&self.remaining),
            "Invalid self-destruct countdown"
        );
        ensure!(
            !self.negative || !self.ammunition,
            "Invalid self-destruct mode"
        );
        Ok(())
    }

    /// Feedback reflects the signed countdown while admission bounds the actual duration.
    fn notice(self, unit: ObjectId) -> Notice {
        let seconds = i32::from(self.remaining) - if self.negative { 256 } else { 0 };
        let plural = self.ammunition || (!self.negative && self.remaining > 1);
        Notice {
            unit,
            text: format!(
                "Self-destruction in {seconds} second{}..",
                if plural { "s" } else { "" }
            ),
        }
    }
}

impl Mech {
    /// Current admitted self-destruct sequence, independent of pilot reassignment.
    pub fn self_destruct(&self) -> Option<SelfDestruct> {
        self.self_destruct
    }

    /// Scenario protection from ammunition self-destruct admission.
    pub fn self_destruct_safe(&self) -> bool {
        self.self_destruct_safe
    }
}

impl Vehicle {
    /// The same sequence drives ground-vehicle and rotorcraft immolation.
    pub fn self_destruct(&self) -> Option<SelfDestruct> {
        self.self_destruct
    }

    /// Scenario protection from ammunition self-destruct admission.
    pub fn self_destruct_safe(&self) -> bool {
        self.self_destruct_safe
    }
}

/// Trusted scenario flag; it gates new ammunition requests and does not cancel admitted events.
pub fn set_battle_self_destruct_safe(world: &mut World, id: ObjectId, safe: bool) -> Result<()> {
    ensure!(
        world
            .objects
            .get(&id)
            .is_some_and(|o| !o.flags.contains(crate::Flag::Going)),
        "Unit is unavailable"
    );
    super::with_unit_mut!(
        world.btech.unit_mut(id).context("Unit is unavailable")?,
        |unit| {
            unit.self_destruct_safe = safe;
            Ok(())
        }
    )
}

/// Active sequences retain command order even when their unit identifiers differ.
fn pending(state: &BtechState) -> Vec<(ObjectId, SelfDestruct)> {
    let mut timers: Vec<_> = state
        .constructed_units()
        .iter()
        .filter_map(|(&id, u)| u.self_destruct().map(|s| (id, s)))
        .chain(
            state
                .vehicles()
                .iter()
                .filter_map(|(&id, u)| u.self_destruct().map(|s| (id, s))),
        )
        .collect();
    timers.sort_by_key(|(id, s)| (s.order, *id));
    timers
}

/// Duplicate scheduling order is invalid persisted state, not a reason to reorder events.
pub(super) fn validate(state: &BtechState) -> Result<()> {
    let mut orders = std::collections::BTreeSet::new();
    for (_, timer) in pending(state) {
        timer.validate()?;
        ensure!(
            orders.insert(timer.order),
            "Duplicate self-destruct event order"
        );
    }
    Ok(())
}

/// Mutable anatomy adapter; no countdown or detonation decisions are duplicated here.
fn timer_mut(world: &mut World, id: ObjectId) -> &mut Option<SelfDestruct> {
    crate::btech::with_unit_mut!(world.btech.unit_mut(id).unwrap(), |unit| {
        &mut unit.self_destruct
    })
}

/// Largest destructive live bin, retaining canonical order for equal hazards.
fn ammunition(world: &World, id: ObjectId) -> Result<Option<usize>> {
    world
        .btech
        .unit(id)
        .context("Unit is unavailable")?
        .largest_ammunition_hazard_bin()
}

/// Ordered cockpit/observer feedback and diagnostic-channel records from admission.
enum Feedback {
    Notice(Notice),
    Debug(String),
}

/// Diagnostic channels are optional and share the enclosing world/effects transaction.
fn debug(scripts: &Scripts, config: &Config, text: String) -> Result<()> {
    super::channels::publish(
        scripts,
        config,
        &[DiagnosticMessage::new(DiagnosticChannel::Debug, text)],
    )
}

/// Admit controls once; the event does not require the original pilot to remain assigned.
fn control(
    world: &mut World,
    config: &Config,
    id: ObjectId,
    pilot: ObjectId,
    text: &str,
) -> Result<Vec<Feedback>> {
    let vehicle = world.btech.vehicles().contains_key(&id);
    let unit = super::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    ensure!(!unit.destroyed, "You are destroyed!");
    ensure!(unit.power == Power::Running, "Reactor is not online!");
    super::brief::display_access(world, id, pilot, true)?;
    let assigned = if vehicle {
        world.btech.vehicles()[&id].pilot()
    } else {
        world.btech.constructed_units()[&id].pilot()
    };
    let wizard = crate::authority::is_wizard(world, pilot);
    ensure!(
        wizard
            || !world.objects[&id].flags.contains(crate::Flag::InCharacter)
            || assigned == Some(pilot),
        "Now now, only the pilot can push that button."
    );
    ensure!(unit.position.is_some(), "Unit must be on a map");
    let override_enabled = text.contains("override") && wizard;
    let argument = text
        .split([' ', '\t'])
        .find(|word| !word.is_empty())
        .context("Invalid number of arguments!")?;
    if !override_enabled {
        let recycling = if vehicle {
            !world.btech.vehicles()[&id].weapon_recycle().is_empty()
        } else {
            let unit = &world.btech.constructed_units()[&id];
            let loadout = unit.loadout()?;
            unit.weapon_recycle().keys().any(|index| {
                unit.sections()[&loadout.weapons[*index].criticals[0].section].internal > 0
            })
        };
        ensure!(!recycling, "You have weapons recycling!");
        if !vehicle {
            ensure!(
                world.btech.constructed_units()[&id]
                    .limb_recycle()
                    .is_empty(),
                "You are still recovering from your last attack."
            );
        }
    }
    let existing = *timer_mut(world, id);
    if argument.eq_ignore_ascii_case("stop") {
        ensure!(
            override_enabled || config.battletech.explode_stop != 0,
            "It's too late to turn back now!"
        );
        ensure!(
            existing.is_some(),
            "Your mech isn't undergoing a self-destruct sequence!"
        );
        *timer_mut(world, id) = None;
        let mut notices = vec![Notice {
            unit: id,
            text: "Self-destruction sequence aborted.".into(),
        }];
        notices.extend(super::broadcast::observer_notices(
            world,
            id,
            "regains control over itself.",
        ));
        let mut feedback: Vec<_> = notices.into_iter().map(Feedback::Notice).collect();
        feedback.insert(
            1,
            Feedback::Debug(format!(
                "#{} in #{} stopped the self-destruction sequence.",
                pilot.0, id.0
            )),
        );
        return Ok(feedback);
    }
    ensure!(
        existing.is_none(),
        "Your mech is already undergoing a self-destruct sequence!"
    );
    let advertised_ammunition = argument.eq_ignore_ascii_case("ammo");
    if advertised_ammunition {
        ensure!(
            override_enabled || config.battletech.explode_ammo != 0,
            "You can't bring yourself to do it!"
        );
        let safe = if vehicle {
            world.btech.vehicles()[&id].self_destruct_safe()
        } else {
            world.btech.constructed_units()[&id].self_destruct_safe()
        };
        ensure!(override_enabled || !safe, "That's not a possibility here.");
        ensure!(
            ammunition(world, id)?.is_some(),
            "There is no 'damaging' ammo on your 'mech!"
        );
    } else if !override_enabled {
        ensure!(
            config.battletech.explode_reactor != 0,
            "You can't bring yourself to do it!"
        );
        ensure!(!vehicle, "Only mechs can do the 'big boom' effect.");
    }
    let delay = if override_enabled {
        3
    } else if advertised_ammunition {
        config.battletech.explode_time / 2
    } else {
        config.battletech.explode_time
    };
    let order = pending(&world.btech).last().map_or(Ok(0), |(_, timer)| {
        timer
            .order
            .checked_add(1)
            .context("Self-destruct event order is exhausted")
    })?;
    *timer_mut(world, id) = Some(SelfDestruct::scheduled(delay, order));
    if let Some(assigned) = assigned {
        super::crew::release_pilot(world, id, assigned)?;
    }
    let mut notices = super::broadcast::observer_notices(
        world,
        id,
        if advertised_ammunition {
            "starts billowing smoke!"
        } else {
            "loses reactions containment!"
        },
    );
    notices.push(Notice {
        unit: id,
        text: "Self-destruction sequence engaged ; please stand by.".into(),
    });
    notices.push(Notice {
        unit: id,
        text: format!(
            "{} in {delay} seconds.",
            if advertised_ammunition {
                "The ammunition will explode"
            } else {
                "The reactor will blow up"
            }
        ),
    });
    let mut feedback: Vec<_> = notices.into_iter().map(Feedback::Notice).collect();
    feedback.insert(
        0,
        Feedback::Debug(format!(
            "#{} in #{} initiates the {} explosion sequence.",
            pilot.0,
            id.0,
            if advertised_ammunition {
                "ammo"
            } else {
                "reactor"
            }
        )),
    );
    Ok(feedback)
}

/// Native and Lua admission share one world/effects checkpoint, including observer publication.
pub fn self_destruct_action(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    pilot: ObjectId,
    text: &str,
) -> Result<()> {
    scripts.atomic(|_| {
        let notices = control(&mut scripts.world.borrow_mut(), config, unit, pilot, text)?;
        for feedback in notices {
            match feedback {
                Feedback::Notice(notice) => super::notify_unit(scripts, notice)?,
                Feedback::Debug(text) => debug(scripts, config, text)?,
            }
        }
        scripts.world.borrow().validate_action(config)
    })
}

/// Idle shutdown cancellation must be processed even with no other battlefield activity.
pub fn self_destructs_pending(world: &World) -> bool {
    !pending(&world.btech).is_empty()
}

/// One committed timer outcome, retaining nested damage for trusted callers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SelfDestructOutcome {
    Cancelled {
        unit: ObjectId,
    },
    Countdown {
        unit: ObjectId,
        remaining: u16,
    },
    Reactor {
        report: Box<ReactorExplosion>,
    },
    Ammunition {
        unit: ObjectId,
        impacts: Vec<TacticalImpact>,
    },
    Vehicle {
        unit: ObjectId,
        damage: DamageResult<VehicleSection>,
        crew_injury: TacticalPilotInjury,
    },
}

/// Process all due timers in scheduling order; a failed blast/callback rolls back the whole step.
pub fn advance_battle_self_destructs_action(
    scripts: &Scripts,
    config: &Config,
) -> Result<Vec<SelfDestructOutcome>> {
    scripts.atomic(|before| {
        let mut reports = Vec::new();
        for (id, _) in pending(&before.btech) {
            let current = {
                let world = scripts.world.borrow();
                world
                    .btech
                    .constructed_units()
                    .get(&id)
                    .and_then(Mech::self_destruct)
                    .or_else(|| {
                        world
                            .btech
                            .vehicles()
                            .get(&id)
                            .and_then(Vehicle::self_destruct)
                    })
            };
            let Some(mut timer) = current else {
                continue;
            };
            let cancel = {
                let world = scripts.world.borrow();
                let unit =
                    super::scanner::scanner_unit(&world, id).context("Unit is unavailable")?;
                unit.destroyed
                    || unit.power != Power::Running
                    || world
                        .objects
                        .get(&id)
                        .is_none_or(|o| o.flags.contains(crate::Flag::Going))
                    || (timer.ammunition && ammunition(&world, id)?.is_none())
            };
            if cancel {
                *timer_mut(&mut scripts.world.borrow_mut(), id) = None;
                reports.push(SelfDestructOutcome::Cancelled { unit: id });
                continue;
            }
            timer.remaining -= 1;
            if timer.remaining > 0 {
                *timer_mut(&mut scripts.world.borrow_mut(), id) = Some(timer);
                super::notify_unit(scripts, timer.notice(id))?;
                reports.push(SelfDestructOutcome::Countdown {
                    unit: id,
                    remaining: timer.remaining,
                });
                continue;
            }
            *timer_mut(&mut scripts.world.borrow_mut(), id) = None;
            debug(scripts, config, format!("#{} explodes.", id.0))?;
            if scripts.world.borrow().btech.vehicles().contains_key(&id) {
                reports.push(immolate(scripts, id)?);
            } else if timer.ammunition {
                debug(scripts, config, format!("#{} explodes [ammo]", id.0))?;
                super::notify_unit(
                    scripts,
                    Notice {
                        unit: id,
                        text: "All your ammo explodes!".into(),
                    },
                )?;
                let mut impacts = Vec::new();
                loop {
                    let index = ammunition(&scripts.world.borrow(), id)?;
                    let Some(index) = index else {
                        break;
                    };
                    impacts.push(super::evacuation::ammunition_explosion_action(
                        scripts,
                        config,
                        id,
                        index,
                        FallRules::configured(config),
                    )?);
                }
                reports.push(SelfDestructOutcome::Ammunition { unit: id, impacts });
            } else {
                debug(scripts, config, format!("#{} explodes [reactor]", id.0))?;
                reports.push(SelfDestructOutcome::Reactor {
                    report: Box::new(super::reactor_explosion_action(scripts, config, id)?),
                });
            }
        }
        super::evacuation::publish_new_casualties(scripts, config, before)?;
        scripts.world.borrow().validate_action(config)?;
        Ok(reports)
    })
}

/// Ground vehicles and rotorcraft lose their rear section and receive the shared terminal injury.
fn immolate(scripts: &Scripts, id: ObjectId) -> Result<SelfDestructOutcome> {
    let notices =
        super::broadcast::observer_notices(&scripts.world.borrow(), id, "suddenly explodes!");
    for notice in notices {
        super::notify_unit(scripts, notice)?;
    }
    super::notify_unit(scripts,Notice{unit:id,text:"Your life flashes before your eyes as your vehicle immolates itself... you faint.. (and die)".into()})?;
    let mut world = scripts.world.borrow_mut();
    let unit = &world.btech.vehicles()[&id];
    let position = unit.position().context("Unit must be on a map")?;
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    let falling = unit.elevation_level(tile) > i32::from(tile.standing_height());
    // Destruction places the wreck on the selected surface before the explosion raises it.
    // Beneath a bridge the destruction surface is the bed, including for hovercraft.
    let surface = if tile
        .deck_height()
        .is_some_and(|deck| unit.elevation_level(tile) < i32::from(deck))
    {
        i32::from(tile.bottom_height())
    } else {
        i32::from(tile.surface_height())
    };
    let height = surface
        .checked_add(6)
        .context("Explosion altitude overflow")?;
    let unit = world.btech.vehicles.get_mut(&id).unwrap();
    let amount = unit.sections()[&VehicleSection::Rear].internal;
    let damage = unit.damage_phase(VehicleSection::Rear, amount, DamagePhase::Internal)?;
    if let Some(flight) = &mut unit.vtol_flight {
        flight.altitude = f64::from(height);
        flight.vertical_speed = 0.0;
        flight.fall = falling.then(|| FreeFall::new(height));
        flight.phase = if falling {
            super::VtolFlightPhase::Falling
        } else {
            super::VtolFlightPhase::Landed
        };
    } else if falling {
        unit.orbital_drop = None;
        unit.ground_elevation = None;
        unit.free_fall = Some(FreeFall::new(height));
    } else {
        unit.orbital_drop = None;
        unit.free_fall = None;
        unit.ground_elevation = Some(f64::from(height));
    }
    let crew_injury = super::pilot_injury::injure_terminal_crew(&mut world, id, 4)?;
    drop(world);
    if let Some(notice) = crew_injury.notice(id) {
        super::notify_unit(scripts, notice)?;
    }
    Ok(SelfDestructOutcome::Vehicle {
        unit: id,
        damage,
        crew_injury,
    })
}

/// All argument interpretation and wizard override checks belong to the shared action.
pub(crate) fn command(ctx: &CommandContext<'_>, input: &CommandInput) -> Result<CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        self_destruct_action(ctx.scripts, ctx.config, id, ctx.player, &input.args)
    })();
    Ok(match result {
        Ok(()) => CommandAction::Continue,
        Err(e) => CommandAction::Report(CommandReport::Reply(format!("{e:#}"))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Observable boundary cases include zero/negative settings and mode changes above 256.
    #[test]
    fn configured_countdown_boundaries() {
        for (delay, remaining, ammunition, negative) in [
            (-257, 255, false, true),
            (-256, 256, false, true),
            (-1, 255, false, true),
            (0, 256, false, true),
            (1, 1, false, false),
            (255, 255, false, false),
            (256, 256, false, false),
            (257, 1, true, false),
            (512, 256, true, false),
            (513, 1, true, false),
            (1000, 232, true, false),
        ] {
            let timer = SelfDestruct::scheduled(delay, 7);
            assert_eq!(
                (timer.remaining, timer.ammunition, timer.negative),
                (remaining, ammunition, negative)
            );
            timer.validate().unwrap();
        }
        assert_eq!(
            SelfDestruct::scheduled(-1, 0).notice(ObjectId(1)).text,
            "Self-destruction in -1 second.."
        );
        assert_eq!(
            SelfDestruct::scheduled(257, 0).notice(ObjectId(1)).text,
            "Self-destruction in 1 seconds.."
        );
        assert_eq!(
            SelfDestruct::scheduled(1, 0).notice(ObjectId(1)).text,
            "Self-destruction in 1 second.."
        );
    }
}
