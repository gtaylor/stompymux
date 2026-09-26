//! Live computer faults and durable display recovery share chassis adapters and host checkpoints.
use super::{BattleComputerFailure as Failure, BattleNotice, BattlePower};
use crate::{Config, Flag, ObjectId, Scripts, World};
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Typed display identity avoids storing encoded event payloads in domain state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Display {
    Tactical,
    LongRange,
    Scanner,
}

/// Events retain insertion order, including overlapping outages of the same display.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct SensorRecovery {
    unit: ObjectId,
    display: Display,
    value: u8,
    remaining: u8,
}

impl SensorRecovery {
    /// Validate durable countdowns and references before accepting a saved world.
    pub(crate) fn validate(&self, state: &super::BtechState) -> Result<()> {
        ensure!(
            self.remaining > 0 && self.remaining <= 200 && self.value <= 127,
            "Invalid computer recovery"
        );
        ensure!(
            state.constructed_units().contains_key(&self.unit)
                || state.vehicles().contains_key(&self.unit),
            "Computer recovery requires a unit"
        );
        Ok(())
    }
    /// Object deletion removes its events while map removal leaves independent hardware timers alive.
    pub(crate) fn unit(&self) -> ObjectId {
        self.unit
    }
}

/// Only started mobile chassis participate, independently of their actual current speed.
fn eligible(world: &World, id: ObjectId) -> bool {
    if world
        .objects
        .get(&id)
        .is_none_or(|o| o.flags.contains(Flag::Going))
    {
        return false;
    }
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        return unit.power() == BattlePower::Running;
    }
    world.btech.vehicles().get(&id).is_some_and(|unit| {
        unit.power() == BattlePower::Running
            && unit.definition().movement != super::BattleVehicleMovement::Stationary
    })
}

/// Running mobile vehicles need ticks even when stopped and without observers.
pub(crate) fn pending(world: &World, config: &Config) -> bool {
    !world.btech.sensor_recoveries.is_empty()
        || config.battletech.parts != 0
            && world.btech.vehicles().keys().any(|&id| eligible(world, id))
}

/// Sample the owned quality, ranges and unit target without refreshing contacts.
fn input(world: &World, id: ObjectId) -> super::BattleComputerFailureInput {
    let (ranges, quality, target) = if let Some(unit) = world.btech.constructed_units().get(&id) {
        (
            unit.sensor_ranges(),
            super::scan::computer_quality(
                &unit.definition().attributes,
                unit.definition().has_special("Clan"),
            ),
            unit.target_lock(),
        )
    } else {
        let unit = &world.btech.vehicles()[&id];
        (
            unit.sensor_ranges(),
            super::scan::computer_quality(
                &unit.definition().attributes,
                unit.definition().has_special("Clan"),
            ),
            unit.target_lock(),
        )
    };
    super::BattleComputerFailureInput {
        parts_enabled: true,
        quality,
        has_target: target.is_some_and(|t| t.target.0 > 0),
        tactical_range: ranges.tactical,
        long_range: ranges.long_range,
        scanner_range: ranges.scan,
    }
}

/// Assign a display at its current damage baseline, using the ordinary hardware field service.
fn set_range(world: &mut World, id: ObjectId, display: Display, value: u8) -> Result<()> {
    let field = match display {
        Display::Tactical => "tacrange",
        Display::LongRange => "lrsrange",
        Display::Scanner => "scanrange",
    };
    if let Some(unit) = world.btech.constructed.get_mut(&id) {
        let hits = unit.system_hits(super::BattleSystem::Sensors);
        return unit.hardware.set(field, &value.to_string(), hits);
    }
    if let Some(unit) = world.btech.vehicles.get_mut(&id) {
        return unit.hardware.set(field, &value.to_string(), 0);
    }
    anyhow::bail!("Unit is unavailable")
}

/// Draw independent nested delays in tactical, long-range, scanner order before disabling displays.
fn outage(
    world: &mut World,
    id: ObjectId,
    effect: Failure,
    input: super::BattleComputerFailureInput,
) -> Result<()> {
    let displays: &[(Display, u8)] = match effect {
        Failure::Tactical => &[(Display::Tactical, input.tactical_range)],
        Failure::LongRange => &[(Display::LongRange, input.long_range)],
        Failure::Scanner => &[(Display::Scanner, input.scanner_range)],
        Failure::AllDisplays => &[
            (Display::Tactical, input.tactical_range),
            (Display::LongRange, input.long_range),
            (Display::Scanner, input.scanner_range),
        ],
        _ => return Ok(()),
    };
    for &(display, captured) in displays {
        let dice = super::dice::unit_dice_mut(world, id)?;
        let upper = dice.die(161)? + 39;
        let remaining = (dice.die(upper - 29)? + 29) as u8;
        // Encoded long-range/scanner recovery values saturate the reference's signed-char setter.
        let value = if display == Display::Tactical {
            captured
        } else {
            127
        };
        Arc::make_mut(&mut world.btech.sensor_recoveries).push(SensorRecovery {
            unit: id,
            display,
            value,
            remaining,
        });
    }
    for &(display, _) in displays {
        set_range(world, id, display, 0)?;
    }
    Ok(())
}

/// Recover existing events before scheduling new faults, so a new timer retains its full delay.
fn recover(world: &mut World) -> Result<Vec<BattleNotice>> {
    let mut due = Vec::new();
    Arc::make_mut(&mut world.btech.sensor_recoveries).retain_mut(|event| {
        event.remaining -= 1;
        if event.remaining == 0 {
            due.push(event.clone());
            false
        } else {
            true
        }
    });
    let mut notices = Vec::new();
    for event in due {
        set_range(world, event.unit, event.display, event.value)?;
        let destroyed = world
            .btech
            .constructed_units()
            .get(&event.unit)
            .map(|u| u.is_destroyed())
            .or_else(|| {
                world
                    .btech
                    .vehicles()
                    .get(&event.unit)
                    .map(|u| u.is_destroyed())
            })
            .unwrap_or(true);
        if destroyed {
            continue;
        }
        let text = match event.display {
            Display::Tactical => "Your tactical scanners are operational again.",
            Display::LongRange => "Your long-range scanners are operational again.",
            Display::Scanner => "Your scanners are operational again.",
        };
        notices.push(BattleNotice {
            unit: event.unit,
            text: text.into(),
        });
    }
    Ok(notices)
}

/// Reference cockpit wording is selected independently of the applied chassis effect.
fn message(effect: Failure) -> &'static str {
    match effect {
        Failure::LoseTarget => "Computer Glitch!  Target lost, please reacquire!",
        Failure::Tactical => "Tactical shorts out! Fixing .. Please stand by.",
        Failure::LongRange => "Long Range Sensors short out! .. Fixing .. Please stand by.",
        Failure::Scanner => "Scanners short out! Fixing .. Please stand by.",
        Failure::AllDisplays => "A sudden *SNAP* echos in your cockpit then all your displays die!",
        Failure::Shutdown => "You hear a loud *SNAP* *CRACKLE* and then everything powers down!",
    }
}

/// Apply one committed second of recovery and turn-boundary failure checks atomically.
pub fn advance_battle_computer_failures_action(scripts: &Scripts, config: &Config) -> Result<()> {
    scripts.atomic(|before| {
        let notices = recover(&mut scripts.world_mut())?;
        super::piloting::publish_ordered_notices(scripts, &notices, &[])?;
        if config.battletech.parts != 0 && scripts.world().btech.turn_clock.due() {
            let ids: Vec<_> = scripts
                .world()
                .btech
                .constructed_units()
                .keys()
                .chain(scripts.world().btech.vehicles().keys())
                .copied()
                .collect();
            for id in ids {
                if !eligible(&scripts.world(), id) {
                    continue;
                }
                let facts = input(&scripts.world(), id);
                let effect = super::select_computer_failure(
                    super::dice::unit_dice_mut(&mut scripts.world_mut(), id)?,
                    facts,
                )?;
                let Some(effect) = effect else {
                    continue;
                };
                let notice = BattleNotice {
                    unit: id,
                    text: format!("[fg=red bold]{}[reset]", message(effect)),
                };
                super::piloting::publish_ordered_notices(scripts, &[notice], &[])?;
                match effect {
                    Failure::LoseTarget => {
                        super::targeting::set_selection(&mut scripts.world_mut(), id, None)
                    }
                    Failure::Shutdown => {
                        let mut effects = super::shutdown::ShutdownEffects::default();
                        let notices = super::power::stop_admitted_in_action(
                            &mut scripts.world_mut(),
                            id,
                            super::BattleFallRules::configured(config),
                            &mut effects,
                        )?;
                        super::piloting::publish_ordered_notices(
                            scripts,
                            &notices,
                            &effects.pilot_notices,
                        )?;
                        super::shutdown::publish(scripts, config, &effects)?;
                    }
                    _ => outage(&mut scripts.world_mut(), id, effect, facts)?,
                }
            }
        }
        super::evacuation::publish_new_casualties(scripts, config, &before)?;
        scripts.world().validate_action(config)?;
        scripts.effects.validate()?;
        Ok(())
    })
}
