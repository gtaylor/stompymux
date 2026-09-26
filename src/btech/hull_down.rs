//! Quad hull-down transitions, shared movement admission, and terrain-cover contributions.
use super::*;
use crate::{Flag, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Completed posture and an optional timed transition; cancellation retains the completed posture.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BattleHullDownState {
    pub active: bool,
    pub pending: Option<bool>,
    pub remaining: u8,
}

impl BattleHullDownState {
    /// Cancel only the pending change, including on shutdown.
    pub(super) fn cancel(&mut self) {
        self.pending = None;
        self.remaining = 0;
    }

    /// Controls that require mobile legs use one gate for active and changing posture.
    pub(super) fn require_mobile(self) -> Result<()> {
        ensure!(!self.active, "You cannot move while hull-down");
        ensure!(
            self.pending.is_none(),
            "You are busy changing your hull-down mode"
        );
        Ok(())
    }
}

impl BattleUnit {
    /// Current completed posture and remaining transition time.
    pub fn hull_down(&self) -> BattleHullDownState {
        self.hull_down
    }

    /// Validate saved stance independently of timers for other actions.
    pub(super) fn validate_hull_down(&self) -> Result<()> {
        let state = self.hull_down;
        ensure!(
            state.remaining <= 30 && state.pending.is_some() == (state.remaining > 0),
            "Invalid hull-down countdown"
        );
        ensure!(
            state.pending != Some(state.active),
            "Redundant hull-down transition"
        );
        if !state.active && state.pending.is_none() {
            return Ok(());
        }
        ensure!(
            self.chassis() == BattleMechChassis::Quad
                && self.position().is_some()
                && self.posture() == BattlePosture::Standing
                && !self.airborne()
                && self.free_fall().is_none()
                && self.stand_timer().is_none(),
            "Invalid hull-down posture"
        );
        ensure!(
            state.pending.is_none()
                || (self.power() == BattlePower::Running && !self.is_destroyed()),
            "Hull-down transition requires a running unit"
        );
        Ok(())
    }
}

/// Begin lowering, begin raising (`-`), or cancel (`stop`) using the assigned pilot.
pub fn set_hull_down(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
    argument: &str,
) -> Result<Vec<BattleNotice>> {
    super::targeting::controlled(world, id, pilot)?;
    super::fortification::require_mobile(world, id)?;
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Only quads can go hull-down")?;
    ensure!(
        unit.chassis() == BattleMechChassis::Quad,
        "Only quads can go hull-down"
    );
    ensure!(unit.crew_recovery().remaining == 0, "You are unconscious");
    ensure!(
        unit.posture() == BattlePosture::Standing,
        "You cannot go hull-down from a fallen position"
    );
    ensure!(
        !unit.airborne() && unit.free_fall().is_none(),
        "Land before changing hull-down posture"
    );
    ensure!(
        unit.jump_stabilization() == 0 && unit.stand_timer().is_none(),
        "Finish stabilizing or standing first"
    );
    let motion = unit.motion().context("Unit is not placed")?;
    ensure!(motion.speed <= 0.5, "You cannot go hull-down while moving");
    let state = unit.hull_down;
    let argument = argument.trim();
    if argument.eq_ignore_ascii_case("stop") {
        ensure!(
            state.pending.is_some(),
            "You are not changing hull-down mode"
        );
        Arc::make_mut(&mut world.btech.constructed)
            .get_mut(&id)
            .unwrap()
            .hull_down
            .cancel();
        return Ok(vec![BattleNotice {
            unit: id,
            text: "You stop changing your hull-down mode.".into(),
        }]);
    }
    ensure!(
        argument.is_empty() || argument == "-",
        "Usage: hulldown [-|stop]"
    );
    let active = argument.is_empty();
    ensure!(
        state.pending.is_none(),
        "You are busy changing your hull-down mode"
    );
    ensure!(
        state.active != active,
        "Unit is already in that hull-down mode"
    );
    let remaining = (30.0 / (unit.mobility().maximum_speed / 10.75).clamp(1.0, 30.0)) as u8;
    let unit = Arc::make_mut(&mut world.btech.constructed)
        .get_mut(&id)
        .unwrap();
    unit.hull_down = BattleHullDownState {
        active: state.active,
        pending: Some(active),
        remaining,
    };
    if active {
        unit.motion.as_mut().unwrap().desired_speed = 0.0;
    }
    Ok(notices(world, id, active, false))
}

/// Format occupant and visibility-filtered observer messages once for both command adapters.
fn notices(world: &World, id: ObjectId, active: bool, complete: bool) -> Vec<BattleNotice> {
    let (text, observer) = match (active, complete) {
        (true, false) => (
            "You start to lower yourself to the ground.",
            "begins to lower itself to the ground.",
        ),
        (false, false) => (
            "You start to lift yourself up.",
            "begins to raise up on its legs.",
        ),
        (true, true) => (
            "You finish lowering yourself to the ground.",
            "finishes lowering itself to the ground.",
        ),
        (false, true) => (
            "You finish lifting yourself up.",
            "finishes lifting itself up.",
        ),
    };
    let mut notices = vec![BattleNotice {
        unit: id,
        text: text.into(),
    }];
    notices.extend(
        super::observer_messages(world, id, observer)
            .into_iter()
            .map(|(unit, text)| BattleNotice { unit, text }),
    );
    notices
}

/// Advance through the ordinary committed heartbeat, preserving completed posture on cancellation.
pub(super) fn advance(world: &mut World) -> Vec<BattleNotice> {
    let mut completed = Vec::new();
    for (&id, unit) in Arc::make_mut(&mut world.btech.constructed) {
        let Some(active) = unit.hull_down.pending else {
            continue;
        };
        if world
            .objects
            .get(&id)
            .is_none_or(|object| object.flags.contains(Flag::Going))
        {
            continue;
        }
        if unit.power() != BattlePower::Running || unit.is_destroyed() {
            unit.hull_down.cancel();
            continue;
        }
        unit.hull_down.remaining -= 1;
        if unit.hull_down.remaining != 0 {
            continue;
        }
        unit.hull_down.pending = None;
        unit.hull_down.active = active;
        completed.push((id, active));
    }
    completed
        .into_iter()
        .flat_map(|(id, active)| notices(world, id, active, true))
        .collect()
}

/// Hull-down adds two to any aim that must clear existing partial cover.
pub(super) fn cover_modifier(world: &World, target: ObjectId, partial_cover: bool) -> i16 {
    if partial_cover
        && world
            .btech
            .constructed_units()
            .get(&target)
            .is_some_and(|unit| unit.hull_down.active)
    {
        2
    } else {
        0
    }
}

/// Commit posture and all notifications together, also usable inside a Lua transaction.
pub fn hull_down_action(
    scripts: &Scripts,
    id: ObjectId,
    pilot: ObjectId,
    argument: &str,
) -> Result<()> {
    let before = scripts.world().clone();
    let checkpoint = scripts.effects.checkpoint();
    let result = (|| {
        let notices = set_hull_down(&mut scripts.world_mut(), id, pilot, argument)?;
        for notice in notices {
            super::notify_unit(scripts, notice)?;
        }
        Ok(())
    })();
    if result.is_err() {
        *scripts.world_mut() = before;
        scripts.effects.restore(checkpoint);
    }
    result
}

/// Native control derives the cockpit from the invoking player.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|object| object.location)
            .context("Enter a quad first")?;
        hull_down_action(ctx.scripts, id, ctx.player, &input.args)
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
