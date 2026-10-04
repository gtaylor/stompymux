//! Saved vehicle crew removal of iNarc effects, committed through the ordinary one-second update.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

impl Vehicle {
    /// Remaining seconds of the crew's active iNarc removal attempt.
    pub fn pod_removal(&self) -> Option<u8> {
        self.pod_removal
    }
}

/// Start an eligible crew attempt; ordinary Narc remains attached after completion.
pub fn begin_pod_removal(world: &mut World, id: ObjectId, pilot: ObjectId) -> Result<Notice> {
    super::vehicle_power::controlled(world, id, pilot)?;
    let motion = super::vehicle_driving::readout(world, id, pilot)?;
    let unit = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    ensure!(
        motion.desired_speed <= 0.0 && motion.speed <= 0.0,
        "You can not be moving when attempting to remove iNarc pods!"
    );
    ensure!(
        unit.vtol_flight().is_none_or(|flight| matches!(
            flight.phase,
            VtolFlightPhase::Landed | VtolFlightPhase::Launching { .. }
        )),
        "You must land before attempting to remove iNarc pods!"
    );
    ensure!(
        unit.pod_removal().is_none(),
        "You are already removing iNarc pods!"
    );
    ensure!(
        !unit.crew_stunned(),
        "You're too stunned to remove iNarc pods!"
    );
    ensure!(
        unit.turret_repairs().is_empty(),
        "You're too busy unjamming your turret to remove iNarc pods!"
    );
    ensure!(
        unit.unjam().is_none(),
        "You're too busy unjamming a weapon to remove iNarc pods!"
    );
    ensure!(
        unit.beacons()
            .values()
            .any(|kinds| kinds.iter().any(|kind| *kind != BeaconKind::Narc)),
        "There are no iNarc pods attached to this unit."
    );
    world.btech.vehicles.get_mut(&id).unwrap().pod_removal = Some(60);
    Ok(Notice {
        unit: id,
        text: "You begin to systematically remove all the iNarc pods from your unit.".into(),
    })
}

/// Publish a start in the same checkpoint as its timer so delivery errors leave no active attempt.
pub fn begin_pod_removal_action(
    scripts: &crate::Scripts,
    id: ObjectId,
    pilot: ObjectId,
) -> Result<()> {
    scripts.atomic(|_| {
        let notice = begin_pod_removal(&mut scripts.world.borrow_mut(), id, pilot)?;
        super::notify_unit(scripts, notice)
    })
}

/// Expire attempts even after shutdown; destroyed vehicles finish silently without removing state.
pub(super) fn advance(world: &mut World) -> Vec<Notice> {
    let ids: Vec<_> = world
        .btech
        .vehicles()
        .iter()
        .filter(|(id, unit)| {
            unit.pod_removal.is_some()
                && world
                    .objects
                    .get(id)
                    .is_some_and(|object| !object.flags.contains(Flag::Going))
        })
        .map(|(id, _)| *id)
        .collect();
    let mut notices = Vec::new();
    for id in ids {
        let unit = world.btech.vehicles.get_mut(&id).unwrap();
        let remaining = unit.pod_removal.as_mut().unwrap();
        *remaining -= 1;
        if *remaining > 0 {
            continue;
        }
        unit.pod_removal = None;
        if unit.is_destroyed() {
            continue;
        }
        unit.beacons.retain(|_, kinds| {
            kinds.retain(|kind| *kind == BeaconKind::Narc);
            !kinds.is_empty()
        });
        notices.push(Notice {
            unit: id,
            text: "You remove all the iNARC pods from your unit.".into(),
        });
        notices.extend(super::broadcast::observer_notices(
            world,
            id,
            "'s crew climbs out and knocks off all the attached iNarc pods!",
        ));
    }
    notices
}

/// Native plural removal starts the same crew action as Lua.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    _: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let id = ctx
            .scripts
            .world
            .borrow()
            .objects
            .get(&ctx.player)
            .and_then(|player| player.location)
            .context("Enter a unit first")?;
        begin_pod_removal_action(ctx.scripts, id, ctx.player)
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}
