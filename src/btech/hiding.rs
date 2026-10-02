//! Shared camouflage preparation, cached observer checks and cover loss for supported units.
use super::{BattleNotice, BattlePower, Ground};
use crate::{CommandAction, CommandContext, CommandInput, CommandReport, Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

impl super::BattleUnit {
    /// Elapsed committed hide checks; zero is the initial scheduled event.
    pub fn hide_elapsed(&self) -> Option<u16> {
        self.hide_elapsed
    }
}

impl super::BattleVehicle {
    /// Elapsed committed hide checks, shared by ground vehicles and rotorcraft.
    pub fn hide_elapsed(&self) -> Option<u16> {
        self.hide_elapsed
    }
}

/// Borrow only the common hiding facts, retaining anatomy at the storage boundary.
fn facts(world: &World, id: ObjectId) -> Option<(Option<u16>, bool, bool)> {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return Some((
            unit.hide_elapsed,
            unit.definition().has_special("Camo_Tech"),
            unit.definition().is_vtol(),
        ));
    }
    world.btech.constructed_units().get(&id).map(|unit| {
        (
            unit.hide_elapsed,
            unit.definition().has_special("Camo_Tech"),
            false,
        )
    })
}

/// Mutable timer and signature fields are the only chassis-specific hiding state.
fn state_mut(world: &mut World, id: ObjectId) -> (&mut Option<u16>, &mut bool) {
    if world.btech.vehicles().contains_key(&id) {
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        return (&mut unit.hide_elapsed, &mut unit.signature.hidden);
    }
    let unit = world.btech.constructed.get_mut(&id).unwrap();
    (&mut unit.hide_elapsed, &mut unit.signature.hidden)
}

/// Begin hiding with wizard authority or installed camouflage, without acquiring any contacts.
pub fn begin_battle_hiding(
    world: &mut World,
    id: ObjectId,
    pilot: ObjectId,
) -> Result<BattleNotice> {
    super::power::controlled_running_unit(world, id, pilot)?;
    let (elapsed, camouflage, vtol) = facts(world, id).context("Unit is unavailable")?;
    ensure!(
        camouflage || crate::authority::is_wizard(world, pilot),
        "You aren't capable of such curious things."
    );
    let unit = super::scanner::scanner_unit(world, id).unwrap();
    let position = unit.position.unwrap();
    if let Some(mech) = world.btech.constructed_units().get(&id) {
        ensure!(
            !mech.airborne() && mech.free_fall().is_none(),
            "Hide where? Up here?"
        );
    } else {
        ensure!(
            (world.btech.vehicles()[&id].free_fall().is_none()
                && world.btech.vehicles()[&id].orbital_drop().is_none()),
            "Hide where? Up here?"
        );
    }
    ensure!(unit.speed.abs() <= 10.75, "Come to a complete stop first.");
    ensure!(elapsed.is_none(), "You are looking for cover already!");
    if vtol {
        ensure!(
            world.btech.vehicles()[&id]
                .vtol_flight()
                .is_none_or(|flight| flight.phase == super::BattleVtolFlightPhase::Landed),
            "You must be landed!"
        );
    }
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    let text = match (tile.is_woods(), tile.is_bare(), tile.ground()) {
        (true, _, _) => "You start to hide amongst the trees...",
        (false, true, Ground::Mountains) => "You start to hide behind some rocky outcroppings...",
        (false, true, Ground::Rough) => "You find some boulders to try to hide behind...",
        _ => anyhow::bail!(
            "You begin to hide in this terrain...\n... then realize that just isn't going to work!"
        ),
    };
    *state_mut(world, id).0 = Some(0);
    Ok(BattleNotice {
        unit: id,
        text: text.into(),
    })
}

/// A pending hide wakes otherwise idle unit processing and survives restart.
pub fn battle_hiding_pending(world: &World) -> bool {
    world
        .btech
        .constructed_units()
        .values()
        .any(|u| u.hide_elapsed.is_some())
        || world
            .btech
            .vehicles()
            .values()
            .any(|u| u.hide_elapsed.is_some())
}

/// Reject hostile acquired observations and height above the terrain without consuming detection dice.
fn exposed(world: &World, id: ObjectId) -> Result<bool> {
    let unit = super::scanner::scanner_unit(world, id).context("Unit is unavailable")?;
    let position = unit.position.context("Unit is not on a map")?;
    let tile =
        world.btech.maps()[&position.map].base_hex(i64::from(position.x), i64::from(position.y))?;
    let elevation = if let Some(vehicle) = world.btech.vehicles().get(&id) {
        vehicle.elevation_level(tile)
    } else {
        world.btech.constructed_units()[&id].elevation_level(tile)
    };
    if elevation != i32::from(super::fall_profile::surface(tile, elevation)) {
        return Ok(true);
    }
    for observer in super::map_slots::all_unit_order(world, position.map)? {
        let Some(other) = super::scanner::scanner_unit(world, observer) else {
            continue;
        };
        if other.observer
            || other.visibility.clairvoyant
            || other.visibility.invisible
            || other.destroyed
            || other.power != BattlePower::Running
            || other.signature.team == unit.signature.team
            || world
                .objects
                .get(&observer)
                .is_none_or(|o| o.flags.contains(Flag::Going))
        {
            continue;
        }
        if other.contacts.contains_key(&id) {
            return Ok(true);
        }
    }
    Ok(false)
}

/// Advance one event per unit in stable order, committing all timers and notices atomically.
pub fn advance_battle_hiding(world: &mut World) -> Result<Vec<BattleNotice>> {
    let mut candidate = world.clone();
    let ids: Vec<_> = super::scanner::scanner_ids(world)
        .into_iter()
        .filter(|&id| facts(world, id).is_some_and(|(elapsed, _, _)| elapsed.is_some()))
        .collect();
    let mut notices = Vec::new();
    for id in ids {
        let (elapsed, camouflage, vtol) = facts(&candidate, id).unwrap();
        if candidate
            .objects
            .get(&id)
            .is_none_or(|o| o.flags.contains(Flag::Going))
            || super::scanner::scanner_unit(&candidate, id)
                .and_then(|u| u.position)
                .is_none()
        {
            *state_mut(&mut candidate, id).0 = None;
            continue;
        }
        if exposed(&candidate, id)? {
            *state_mut(&mut candidate, id).0 = None;
            notices.push(BattleNotice {
                unit: id,
                text: "Your spidey sense tingles, telling you this isn't going to work......"
                    .into(),
            });
            continue;
        }
        let threshold = if vtol { 40 } else { 50 } * if camouflage { 1 } else { 2 };
        let (timer, hidden) = state_mut(&mut candidate, id);
        if elapsed.unwrap() < threshold {
            *timer = Some(elapsed.unwrap() + 1);
            continue;
        }
        *timer = None;
        *hidden = true;
        notices.push(BattleNotice {
            unit: id,
            text: "You are now hidden!".into(),
        });
    }
    candidate.btech.validate_action(&candidate)?;
    *world = candidate;
    Ok(notices)
}

/// Clear active cover and optionally cancel preparation, preserving observer visibility for feedback.
fn reveal(
    world: &mut World,
    id: ObjectId,
    cancel: bool,
    text: &str,
    broadcast: &str,
) -> Vec<BattleNotice> {
    let Some(unit) = super::scanner::scanner_unit(world, id) else {
        return Vec::new();
    };
    let mut notices = Vec::new();
    if unit.signature.hidden {
        notices.push(BattleNotice {
            unit: id,
            text: text.into(),
        });
        notices.extend(super::broadcast::observer_notices(world, id, broadcast));
    }
    let (timer, hidden) = state_mut(world, id);
    if cancel {
        *timer = None;
    }
    *hidden = false;
    notices
}

/// Attempted firing reveals cover and cancels preparation before mechanical or target rejection.
pub(super) fn firing(world: &mut World, id: ObjectId) -> Vec<BattleNotice> {
    reveal(
        world,
        id,
        true,
        "You break out of your cover to initiate weapons fire!",
        "breaks out of its cover and begins firing rabidly!",
    )
}

/// Armor-directed damage ruins current cover; it does not cancel a pending hide event.
pub(super) fn damage(world: &mut World, id: ObjectId) -> Vec<BattleNotice> {
    reveal(
        world,
        id,
        false,
        "Your cover is ruined as you take damage!",
        "loses its cover as it takes damage.",
    )
}

/// A committed map/hex crossing cancels hiding; motion within the same hex does not.
pub(super) fn movement(world: &mut World, id: ObjectId) -> Vec<BattleNotice> {
    reveal(
        world,
        id,
        true,
        "You move too much and break your cover!",
        "breaks from its cover.",
    )
}

/// Cover changes across movement adapters that relocate units after their local terrain step.
pub(super) fn movement_changes(world: &mut World, before: &World) -> Vec<BattleNotice> {
    let ids: Vec<_> = super::scanner::scanner_ids(before)
        .into_iter()
        .filter(|&id| {
            // Towing mirrors placement; the target did not execute a movement event.
            if before.btech.towed_by(id).is_some() {
                return false;
            }
            let Some(old) = super::scanner::scanner_unit(before, id) else {
                return false;
            };
            (old.signature.hidden || facts(before, id).is_some_and(|f| f.0.is_some()))
                && super::scanner::scanner_unit(world, id)
                    .is_some_and(|new| new.position != old.position)
        })
        .collect();
    ids.into_iter().flat_map(|id| movement(world, id)).collect()
}

/// Native hiding ignores trailing text, sharing the same mutation and cockpit publication as Lua.
pub(crate) fn command(ctx: &CommandContext<'_>, _: &CommandInput) -> Result<CommandAction> {
    let result = ctx.scripts.atomic(|before| {
        let id = before
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        let notice = begin_battle_hiding(&mut ctx.scripts.world.borrow_mut(), id, ctx.player)?;
        super::notify_unit(ctx.scripts, notice)
    });
    Ok(match result {
        Ok(()) => CommandAction::Continue,
        Err(error) => CommandAction::Report(CommandReport::Reply(format!("{error:#}"))),
    })
}
