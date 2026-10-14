//! MASC and supercharger controls and their once-a-turn activation checks (Total Warfare
//! p. 137; Tactical Operations: Advanced Units and Equipment p. 345). A booster that was in
//! use during a turn rolls 2d6 at the turn-clock boundary against an escalating threshold;
//! a failed MASC check puts one critical hit on each surviving leg and the MASC keeps
//! working, while a failed supercharger check rolls on the Determining Critical Hits Table
//! against the upper center-torso slots and destroys the supercharger.
use super::{CriticalLocation, MechSection, Notice, Power, System};
use crate::{Config, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Durable activation history and the interval of the next check; supercharger failure
/// survives shutdown.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoosterState {
    /// Pilot-selected operating mode.
    pub enabled: bool,
    /// Interval of the last activation check still in effect, 1 to 5; 0 for a fresh system.
    #[serde(default)]
    pub counter: u8,
    /// The booster was active at some point in the current turn.
    #[serde(default)]
    pub used: bool,
    /// The last turn passed without using the booster, so the next check repeats the
    /// decayed interval instead of advancing to the next one.
    #[serde(default)]
    pub rested: bool,
    /// Permanent failure independent of still-installed critical slots (supercharger only).
    #[serde(default)]
    pub failed: bool,
}

/// The highest activation interval; the fifth consecutive turn fails automatically.
const LAST_INTERVAL: u8 = 5;

/// A 2d6 result at or below this fails the check at `interval` (Total Warfare p. 137): 2 on
/// the first turn of use, 4 on the second, 6 on the third, 10 on the fourth, and any result
/// on the fifth.
fn failure_threshold(interval: u8) -> u8 {
    match interval {
        0 | 1 => 2,
        2 => 4,
        3 => 6,
        4 => 10,
        _ => 12,
    }
}

impl BoosterState {
    /// Reject impossible overload combinations on load.
    pub(super) fn validate(self) -> Result<()> {
        ensure!(
            self.counter <= LAST_INTERVAL && (!self.enabled || !self.failed),
            "Invalid booster state"
        );
        Ok(())
    }

    /// Clear operating history while retaining failed hardware.
    pub(super) fn shutdown(&mut self) {
        self.enabled = false;
        self.counter = 0;
        self.used = false;
        self.rested = false;
    }

    /// The interval the next check rolls at: the next one up after a turn of use, or the
    /// decayed one after a rest (TW p. 137: one interval down per unused turn, never below
    /// the first).
    pub(super) fn next_interval(self) -> u8 {
        if self.rested {
            self.counter.clamp(1, LAST_INTERVAL)
        } else {
            (self.counter + 1).min(LAST_INTERVAL)
        }
    }

    /// Record a turn that passed without using the booster.
    fn rest(&mut self) {
        if self.counter > 0 {
            self.counter -= 1;
            self.rested = true;
        }
        self.used = false;
    }
}

impl super::Mech {
    /// Owned activation and overload state; hardware inspection remains separate.
    pub fn masc(&self) -> BoosterState {
        self.masc
    }

    /// A working powered booster contributes to the current movement ceiling.
    pub fn masc_active(&self) -> bool {
        self.masc.enabled
            && self.power() == Power::Running
            && self.masc_operational().unwrap_or(false)
    }
}

/// Toggle a booster, keeping a forward throttle's share of the running ceiling.
fn toggle(world: &mut World, id: ObjectId, pilot: ObjectId, kind: Booster) -> Result<Notice> {
    super::power::controlled_unit(world, id, pilot)?;
    let unit = &world.btech.constructed_units()[&id];
    ensure!(unit.power() == Power::Running, "Start the unit first");
    ensure!(
        kind.operational(unit)?,
        "Your toy ain't prepared for what you're askin' it!"
    );
    ensure!(unit.motion().is_some(), "Unit is not placed");
    let enabled = !kind.state(unit).enabled;
    ensure!(
        !enabled || unit.movement_maximum_speed() >= 10.75,
        "{}",
        if kind == Booster::Masc {
            "You can't move. How is MASC going to work?"
        } else {
            "How much can you Supercharge if you can't move?"
        }
    );
    let before = unit.movement_maximum_speed();
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    kind.state_mut(unit).enabled = enabled;
    if enabled {
        // Activation is declared for the turn; the check comes at its end.
        kind.state_mut(unit).used = true;
    }
    unit.supercharger_scheduled_last = kind == Booster::Supercharger;
    let after = unit.movement_maximum_speed();
    let motion = unit.motion.as_mut().unwrap();
    if motion.desired_speed > 0.0 && before > 0.0 {
        motion.desired_speed *= after / before;
    }
    Ok(Notice {
        unit: id,
        text: format!(
            "{} has been turned {}.",
            kind.name(),
            if enabled { "on" } else { "off" }
        ),
    })
}

/// Cockpit activation shares notification rollback with other Lua/native controls.
fn control(scripts: &Scripts, id: ObjectId, pilot: ObjectId, kind: Booster) -> Result<Notice> {
    scripts.atomic(|_| {
        let notice = toggle(&mut scripts.world_mut(), id, pilot, kind)?;
        super::notify_unit(scripts, notice.clone())?;
        Ok(notice)
    })
}

/// Note which boosters were in use this second and, on the turn-clock boundary, make the
/// activation checks of the turn just ended, publishing every consequence in one host
/// transaction.
pub fn advance_boosters_action(scripts: &Scripts, config: &Config) -> Result<Vec<Notice>> {
    let ids: Vec<_> = scripts
        .world()
        .btech
        .constructed_units()
        .iter()
        .flat_map(|(&id, unit)| {
            let kinds = if unit.supercharger_scheduled_last {
                [Booster::Masc, Booster::Supercharger]
            } else {
                [Booster::Supercharger, Booster::Masc]
            };
            kinds
                .into_iter()
                .filter(move |kind| {
                    let state = kind.state(unit);
                    state.enabled || state.counter > 0 || state.used
                })
                .map(move |kind| (id, kind))
        })
        .collect();
    if ids.is_empty() {
        return Ok(Vec::new());
    }
    scripts.atomic(|before| {
        let boundary = scripts.world().btech.turn_clock.starts_turn();
        let mut notices = Vec::new();
        let mut private = Vec::new();
        let mut impacts = Vec::new();
        for (id, kind) in ids {
            let mut world = scripts.world_mut();
            let available = world
                .objects
                .get(&id)
                .is_some_and(|object| !object.flags.contains(crate::Flag::Going));
            let unit = &world.btech.constructed_units()[&id];
            if !available || unit.power() != Power::Running || unit.is_destroyed() {
                kind.state_mut(world.btech.constructed.get_mut(&id).unwrap())
                    .shutdown();
                continue;
            }
            if kind.state(unit).enabled && !kind.operational(unit)? {
                let unit = world.btech.constructed.get_mut(&id).unwrap();
                kind.state_mut(unit).enabled = false;
                unit.supercharger_scheduled_last = kind == Booster::Supercharger;
            }
            let unit = world.btech.constructed.get_mut(&id).unwrap();
            kind.state(unit).validate()?;
            if kind.active(unit) {
                kind.state_mut(unit).used = true;
            }
            if !boundary {
                continue;
            }
            if !kind.state(unit).used {
                kind.state_mut(unit).rest();
                continue;
            }
            let interval = kind.state(unit).next_interval();
            let threshold = failure_threshold(interval);
            let roll = unit.dice.generic_roll();
            {
                let state = kind.state_mut(unit);
                state.counter = interval;
                state.rested = false;
                state.used = false;
            }
            notices.push(Notice {
                unit: id,
                text: format!(
                    "{}: turn {interval}, fails on {threshold} or less, Roll: {roll}",
                    kind.name()
                ),
            });
            if roll > threshold {
                continue;
            }
            let pilot = unit.pilot();
            let toughness = pilot
                .and_then(|pilot| world.btech.character_values().get(&pilot))
                .is_some_and(|values| super::advantages::enabled(values, "Toughness"));
            let settings = &config.battletech;
            let rules = super::FallRules {
                vehicle_impact: crate::VehicleImpactRules::configured(settings, false),
                extended_piloting: settings.extended_piloting != 0,
                toughness,
            };
            let report = match kind {
                Booster::Masc => fail_masc(&mut world, id, rules, &mut notices)?,
                Booster::Supercharger => fail_supercharger(&mut world, id, rules, &mut notices)?,
            };
            if let Some(report) = report {
                super::piloting::append_feedback(
                    &mut private,
                    report.pilot_notices.iter().cloned(),
                    notices.len(),
                );
                notices.extend(report.notices.iter().cloned());
                impacts.push(report);
            }
            world
                .btech
                .constructed
                .get_mut(&id)
                .unwrap()
                .reconcile_damage();
        }
        super::piloting::publish_ordered_notices(scripts, &notices, &private)?;
        for impact in &impacts {
            super::evacuation::publish_impact_consequences(scripts, config, impact)?;
        }
        super::evacuation::publish_new_casualties(scripts, config, before)?;
        scripts.world().validate_action(config)?;
        Ok(notices)
    })
}

/// A failed MASC check assigns one critical hit to each leg not yet destroyed, located by
/// the ordinary slot roll, and the MASC keeps working (Total Warfare p. 137).
fn fail_masc(
    world: &mut World,
    id: ObjectId,
    rules: super::FallRules,
    notices: &mut Vec<Notice>,
) -> Result<Option<super::TacticalImpact>> {
    notices.push(Notice {
        unit: id,
        text: "Your MASC overloads and damages your legs!".into(),
    });
    notices.extend(super::broadcast::observer_notices(
        world,
        id,
        "'s legs shudder as its MASC overloads!",
    ));
    let unit = &world.btech.constructed_units()[&id];
    let targets: Vec<_> = unit
        .chassis()
        .legs()
        .iter()
        .filter(|&&leg| !unit.leg_unavailable(leg))
        .map(|&leg| (leg, None))
        .collect();
    if targets.is_empty() {
        return Ok(None);
    }
    super::impact::resolve_chosen_criticals(world, id, &targets, rules).map(Some)
}

/// A failed supercharger check rolls on the Determining Critical Hits Table, applies the
/// hits to the upper-most undamaged center-torso slots and destroys the supercharger
/// (Tactical Operations: Advanced Units and Equipment p. 345).
fn fail_supercharger(
    world: &mut World,
    id: ObjectId,
    rules: super::FallRules,
    notices: &mut Vec<Notice>,
) -> Result<Option<super::TacticalImpact>> {
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    unit.supercharger.enabled = false;
    unit.supercharger.failed = true;
    let roll = unit.dice.generic_roll();
    let count = super::impact::critical_count(roll);
    notices.push(Notice {
        unit: id,
        text: format!("Your supercharger overloads and explodes!  Critical roll: {roll}"),
    });
    notices.extend(super::broadcast::observer_notices(
        world,
        id,
        "'s center torso spews black smoke!",
    ));
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    let loadout = unit.loadout()?;
    for part in loadout.systems.iter().filter(|part| {
        part.location.section == MechSection::CenterTorso && part.system == System::Supercharger
    }) {
        unit.destroy_critical(part.location)?;
    }
    let mut targets: Vec<_> = unit.definition().sections[&MechSection::CenterTorso]
        .criticals
        .keys()
        .copied()
        .map(|slot| CriticalLocation {
            section: MechSection::CenterTorso,
            slot,
        })
        .filter(|&location| !unit.critical_destroyed(location))
        .collect();
    targets.sort_by_key(|location| location.slot);
    let targets: Vec<_> = targets
        .into_iter()
        .take(usize::from(count))
        .map(|location| (MechSection::CenterTorso, Some(location)))
        .collect();
    if targets.is_empty() {
        return Ok(None);
    }
    super::impact::resolve_chosen_criticals(world, id, &targets, rules).map(Some)
}

/// Native toggle resolves the invoking player's cockpit.
fn command_for(ctx: &crate::CommandContext<'_>, kind: Booster) -> Result<crate::CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        control(ctx.scripts, id, ctx.player, kind)
    })();
    Ok(match result {
        Ok(_) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Toggle MASC through the pure world control.
pub fn toggle_masc(world: &mut World, id: ObjectId, pilot: ObjectId) -> Result<Notice> {
    toggle(world, id, pilot, Booster::Masc)
}
/// Toggle a supercharger through the pure world control.
pub fn toggle_supercharger(world: &mut World, id: ObjectId, pilot: ObjectId) -> Result<Notice> {
    toggle(world, id, pilot, Booster::Supercharger)
}
/// Toggle and publish MASC atomically.
pub fn masc(scripts: &Scripts, id: ObjectId, pilot: ObjectId) -> Result<Notice> {
    control(scripts, id, pilot, Booster::Masc)
}
/// Toggle and publish the supercharger atomically.
pub fn supercharger(scripts: &Scripts, id: ObjectId, pilot: ObjectId) -> Result<Notice> {
    control(scripts, id, pilot, Booster::Supercharger)
}
/// Native MASC control.
pub(crate) fn masc_command(
    ctx: &crate::CommandContext<'_>,
    _: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, Booster::Masc)
}
/// Native supercharger control.
pub(crate) fn supercharger_command(
    ctx: &crate::CommandContext<'_>,
    _: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command_for(ctx, Booster::Supercharger)
}

/// The two independently checked booster devices.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Booster {
    Masc,
    Supercharger,
}
impl Booster {
    fn name(self) -> &'static str {
        match self {
            Self::Masc => "MASC",
            Self::Supercharger => "Supercharger",
        }
    }
    fn state(self, unit: &super::Mech) -> &BoosterState {
        match self {
            Self::Masc => &unit.masc,
            Self::Supercharger => &unit.supercharger,
        }
    }
    fn state_mut(self, unit: &mut super::Mech) -> &mut BoosterState {
        match self {
            Self::Masc => &mut unit.masc,
            Self::Supercharger => &mut unit.supercharger,
        }
    }
    fn operational(self, unit: &super::Mech) -> Result<bool> {
        match self {
            Self::Masc => unit.masc_operational(),
            Self::Supercharger => Ok(unit.supercharger_operational()),
        }
    }
    fn active(self, unit: &super::Mech) -> bool {
        match self {
            Self::Masc => unit.masc_active(),
            Self::Supercharger => unit.supercharger_active(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The thresholds climb 2, 4, 6, 10 and then fail automatically; a rested turn repeats
    /// the decayed interval and two rests return to the first (Total Warfare p. 137).
    #[test]
    fn intervals_escalate_with_use_and_decay_with_rest() {
        assert_eq!([1, 2, 3, 4, 5].map(failure_threshold), [2, 4, 6, 10, 12]);
        let mut state = BoosterState::default();
        for expected in [1, 2, 3] {
            assert_eq!(state.next_interval(), expected);
            state.counter = expected;
            state.rested = false;
        }
        state.rest();
        assert_eq!(state.next_interval(), 2);
        state.rest();
        assert_eq!(state.next_interval(), 1);
        state.rest();
        assert_eq!(state.next_interval(), 1);
        state.counter = 5;
        state.rested = false;
        assert_eq!(state.next_interval(), 5);
        assert!(
            BoosterState {
                counter: 6,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            BoosterState {
                enabled: true,
                failed: true,
                ..Default::default()
            }
            .validate()
            .is_err()
        );
    }
}
