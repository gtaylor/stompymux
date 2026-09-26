//! Ground-vehicle digging state and controls; combat consumers share this saved state.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};
use serde::{Deserialize, Serialize};

/// Cover and preparation are independent of scheduled completion events.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BattleDigState {
    pub dug_in: bool,
    pub digging: bool,
    /// Distinct pending completion deadlines; raw flags do not create or cancel these events.
    pub completion: std::collections::BTreeSet<u8>,
}

impl BattleDigState {
    /// Begin ordinary timed preparation without existing cover.
    pub fn preparing(remaining: u8) -> Self {
        Self {
            dug_in: false,
            digging: true,
            completion: [remaining].into_iter().collect(),
        }
    }

    /// Established cover with no remaining preparation event.
    pub fn covered() -> Self {
        Self {
            dug_in: true,
            ..Self::default()
        }
    }

    /// Seconds until the next pending completion, or zero when no event is scheduled.
    pub fn remaining(&self) -> u8 {
        self.completion.first().copied().unwrap_or(0)
    }

    /// No cover or preparation condition, irrespective of an independently pending timer.
    pub fn exposed(&self) -> bool {
        !self.dug_in && !self.digging
    }
}

impl BattleVehicle {
    /// Current digging preparation or established cover.
    pub fn dig_state(&self) -> BattleDigState {
        self.dig.clone()
    }

    /// Reject impossible construction and countdowns in both saved and live state.
    pub(super) fn validate_dig(&self) -> Result<()> {
        ensure!(
            self.dig
                .completion
                .iter()
                .all(|remaining| (1..=20).contains(remaining))
                && (self.dig.completion.is_empty() || !self.is_destroyed()),
            "Invalid digging countdown"
        );
        ensure!(
            (self.dig.exposed() && self.dig.completion.is_empty())
                || (matches!(
                    self.definition().movement,
                    BattleVehicleMovement::Tracked | BattleVehicleMovement::Wheeled
                ) && self.position().is_some()),
            "Digging requires a placed tracked or wheeled vehicle"
        );
        Ok(())
    }
}

/// Begin cover preparation for a stationary, conscious operator on a diggable surface.
pub fn dig_unit(world: &mut World, id: ObjectId, pilot: ObjectId) -> Result<BattleNotice> {
    super::targeting::controlled(world, id, pilot)?;
    super::fortification::require_mobile(world, id)?;
    let unit = world
        .btech
        .vehicles()
        .get(&id)
        .context("Only tracked and wheeled vehicles can dig in")?;
    ensure!(
        matches!(
            unit.definition().movement,
            BattleVehicleMovement::Tracked | BattleVehicleMovement::Wheeled
        ),
        "Only tracked and wheeled vehicles can dig in"
    );
    ensure!(unit.crew_recovery().remaining == 0, "You are unconscious");
    ensure!(unit.free_fall().is_none(), "You cannot dig while falling");
    ensure!(unit.dig.exposed(), "You are already digging or dug in");
    let motion = unit.motion().context("Vehicle is not placed")?;
    ensure!(motion.speed == 0.0, "You are moving!");
    ensure!(motion.heading == motion.desired_heading, "You are turning!");
    let position = unit.position().context("Vehicle is not placed")?;
    let tile =
        world.btech.maps()[&position.map].hex(i64::from(position.x), i64::from(position.y))?;
    ensure!(
        !matches!(
            tile.terrain,
            Terrain::Road | Terrain::Bridge | Terrain::Building | Terrain::Wall | Terrain::Water
        ),
        "You cannot dig into this surface"
    );
    let unit = world.btech.vehicles.get_mut(&id).unwrap();
    unit.dig.dug_in = false;
    unit.dig.digging = true;
    unit.dig.completion.insert(20);
    Ok(BattleNotice {
        unit: id,
        text: "You start digging yourself in a nice hole..".into(),
    })
}

/// The ordinary heartbeat owns countdown advancement and persistence; no private timer is scheduled.
pub(super) fn advance(world: &mut World) -> Vec<BattleNotice> {
    let mut notices = Vec::new();
    for (&id, unit) in world.btech.vehicles.iter_mut() {
        if unit.dig.completion.is_empty()
            || world
                .objects
                .get(&id)
                .is_none_or(|object| object.flags.contains(Flag::Going))
        {
            continue;
        }
        let due = unit.dig.completion.contains(&1);
        unit.dig.completion = unit
            .dig
            .completion
            .iter()
            .filter_map(|remaining| remaining.checked_sub(1).filter(|remaining| *remaining > 0))
            .collect();
        if !due
            || !unit.dig.digging
            || unit.power() != BattlePower::Running
            || unit.is_destroyed()
            || unit.free_fall().is_some()
        {
            continue;
        }
        unit.dig.digging = false;
        unit.dig.dug_in = true;
        notices.push(BattleNotice {
            unit: id,
            text: "You finish burrowing for cover - only turret weapons are available now.".into(),
        });
    }
    notices
}

/// Cover applies from the configured arc only when the attacker is no higher than the defender.
pub(super) fn cover_modifier(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
    rules: BattleAimRules,
) -> Result<i16> {
    if !world
        .btech
        .vehicles()
        .get(&target)
        .is_some_and(|unit| unit.dig.dug_in)
    {
        return Ok(0);
    }
    let source_height = super::unit_elevation(world, shooter)?.context("Shooter is not placed")?;
    let target_height = super::unit_elevation(world, target)?.context("Target is not placed")?;
    if source_height > target_height {
        return Ok(0);
    }
    if rules.dig_only_front
        && (super::hit_direction::HitDirection::Direct {
            shooter,
            mode: rules.hit_arc_mode,
        })
        .current(world, target)?
            != BattleHitArc::Front
    {
        return Ok(0);
    }
    Ok(rules.dig_bonus)
}

impl BattleVehicle {
    /// Cancel preparation without removing cover already completed before shutdown or destruction.
    pub(super) fn cancel_digging(&mut self) -> bool {
        let was_digging = self.dig.digging;
        self.dig.digging = false;
        self.dig.completion.clear();
        was_digging
    }
}

/// Share the same world/effect checkpoint for native and Lua initiation.
pub fn dig_action(scripts: &crate::Scripts, id: ObjectId, pilot: ObjectId) -> Result<()> {
    scripts.atomic(|_| {
        let notice = dig_unit(&mut scripts.world_mut(), id, pilot)?;
        super::notify_unit(scripts, notice)
    })
}

/// Native digging takes no arguments and uses the caller's current cockpit.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        ensure!(input.args.trim().is_empty(), "Usage: dig");
        let id = ctx
            .scripts
            .world()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a vehicle first")?;
        dig_action(ctx.scripts, id, ctx.player)
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
