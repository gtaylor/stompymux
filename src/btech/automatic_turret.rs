//! Persistent pilot-selected turret tracking follows live unit or coordinate target selections.
use crate::{CommandAction, CommandContext, CommandInput, CommandReport, ObjectId, World};
use anyhow::{Context, Result, ensure};
use std::sync::Arc;

impl super::BattleVehicle {
    /// Selected automatic tracking mode, independent of current power and mechanical availability.
    pub fn automatic_turret(&self) -> bool {
        self.automatic_turret
    }
}

/// Toggle mode while on a map with a conscious assigned pilot and a surviving turret.
pub fn toggle_battle_automatic_turret(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
) -> Result<super::BattleNotice> {
    super::vehicle_power::controlled(world, id, pilot)?;
    let unit = &world.btech.vehicles()[&id];
    ensure!(unit.position().is_some(), "Unit must be on a map");
    ensure!(
        unit.turret_heading().is_some(),
        "You have no turret to autoturn!"
    );
    let enabled = !unit.automatic_turret;
    Arc::make_mut(&mut world.btech.vehicles)
        .get_mut(&id)
        .unwrap()
        .automatic_turret = enabled;
    Ok(super::BattleNotice {
        unit: id,
        text: format!(
            "Automatic turret turning is now {}",
            if enabled { "ON" } else { "OFF" }
        ),
    })
}

/// Compute only a desired change, allowing idle heartbeat admission to stay read-only.
fn desired(world: &World, id: ObjectId) -> Option<f64> {
    let unit = world.btech.vehicles().get(&id)?;
    if !unit.automatic_turret
        || unit.power() != super::BattlePower::Running
        || unit.is_destroyed()
        || unit.turret_locked()
        || unit.turret_jammed()
        || unit.crew_recovery().remaining > 0
        || unit.pilot().is_some_and(|p| world.btech.unconscious(p))
        || world
            .objects
            .get(&id)
            .is_none_or(|o| o.flags.contains(crate::Flag::Going))
    {
        return None;
    }
    unit.turret_heading()?;
    let position = unit.position()?;
    let motion = unit.motion()?;
    let point = match unit.target_selection()? {
        super::BattleTargetSelection::Unit(lock) => {
            if world
                .objects
                .get(&lock.target)
                .is_none_or(|o| o.flags.contains(crate::Flag::Going))
            {
                return None;
            }
            let target = super::scanner::scanner_unit(world, lock.target)?;
            if target.position?.map != position.map {
                return None;
            }
            target.point?
        }
        super::BattleTargetSelection::Hex(lock) => {
            world
                .btech
                .maps()
                .get(&position.map)?
                .hex(i64::from(lock.hex.x), i64::from(lock.hex.y))
                .ok()?;
            lock.hex.center()
        }
    };
    let heading = integer_bearing(motion.point, point)?;
    Some((heading - motion.heading).rem_euclid(360.0))
}

/// Quantize signed tenth-degrees before producing the integer compass bearing.
fn integer_bearing(start: super::BattlePoint, end: super::BattlePoint) -> Option<f64> {
    if start.x == end.x {
        return Some(if end.y < start.y { 0.0 } else { 180.0 });
    }
    let bearing = start.bearing(end).ok()??;
    Some(f64::from((((bearing - 180.0) * 10.0).trunc() as i32 + 5) / 10 + 180).rem_euclid(360.0))
}

/// A moving target or hull can wake tracking even after the selection countdown settles.
pub fn battle_automatic_turrets_pending(world: &World) -> bool {
    world
        .btech
        .vehicles()
        .iter()
        .any(|(&id, unit)| desired(world, id).is_some_and(|heading| heading != unit.turret_offset))
}

/// Apply tracking after motion and before scanner refresh in the host's world transaction.
pub fn advance_battle_automatic_turrets(world: &mut World) {
    let updates: Vec<_> = world
        .btech
        .vehicles()
        .iter()
        .filter_map(|(&id, unit)| {
            desired(world, id)
                .filter(|heading| *heading != unit.turret_offset)
                .map(|heading| (id, heading))
        })
        .collect();
    if updates.is_empty() {
        return;
    }
    let vehicles = Arc::make_mut(&mut world.btech.vehicles);
    for (id, heading) in updates {
        vehicles.get_mut(&id).unwrap().turret_offset = heading;
    }
}

/// Trailing text is ignored; the domain operation owns the toggle and cockpit authority.
pub(crate) fn command(ctx: &CommandContext<'_>, _input: &CommandInput) -> Result<CommandAction> {
    let before = ctx.scripts.world.borrow().clone();
    let checkpoint = ctx.scripts.effects.checkpoint();
    let result = (|| {
        let id = before
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        let notice =
            toggle_battle_automatic_turret(&mut ctx.scripts.world.borrow_mut(), id, ctx.player)?;
        super::notify_unit(ctx.scripts, notice)
    })();
    Ok(match result {
        Ok(()) => CommandAction::Continue,
        Err(error) => {
            *ctx.scripts.world.borrow_mut() = before;
            ctx.scripts.effects.restore(checkpoint);
            CommandAction::Report(CommandReport::Reply(format!("{error:#}")))
        }
    })
}
