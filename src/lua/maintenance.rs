//! Immediate semantic repair with effects retained until the outer world commit.
use super::{Scripts, packages::error::failure, transactions};
use crate::{
    flags::Flag,
    world::{Kind, ObjectId, World},
};

/// Replay repairs from a valid scheduled-destruction state through ordinary movement.
///
/// Planning produces the final tombstones first so references can be repaired as one
/// graph. C evacuates occupants before replacing their GOING containers, however, and
/// therefore runs the complete generic movement provider/event sequence against the
/// still-live source. Reconstruct that intermediate state when it remains valid. Truly
/// malformed databases retain the narrower callback path below because their original
/// graph cannot safely enter the normal movement engine.
pub(crate) fn apply_relocations(
    scripts: &Scripts,
    before: &World,
    report: &crate::dbck::DbCheckReport,
    session: impl Fn(ObjectId) -> Option<u64>,
) -> anyhow::Result<()> {
    let final_world = scripts.world.borrow().clone();
    let mut replay = before.clone();
    for (id, repaired) in &final_world.objects {
        if report.plan.purges.contains(id) {
            continue;
        }
        let target = replay
            .objects
            .get_mut(id)
            .ok_or_else(|| anyhow::anyhow!("repaired object disappeared"))?;
        target.location = repaired.location;
        target.home = repaired.home;
        target.destination = repaired.destination;
        target.dropto = repaired.dropto;
        target.zone = repaired.zone;
        target.affiliation = repaired.affiliation;
        target.flags = repaired.flags.clone();
    }
    for relocation in &report.plan.relocations {
        replay
            .objects
            .get_mut(&relocation.object)
            .ok_or_else(|| anyhow::anyhow!("repaired occupant disappeared"))?
            .location = relocation.source;
    }

    let config = super::configuration(&scripts.lua);
    if replay.validate(&config).is_ok() {
        *scripts.world.borrow_mut() = replay;
        for id in &report.plan.purges {
            let cause = scripts.world.borrow().objects.get(id).and_then(|object| {
                (object.location.is_some() && matches!(object.kind, Kind::Player | Kind::Thing))
                    .then(|| {
                        if object.kind == Kind::Player {
                            object.pending_destroyer.unwrap_or(ObjectId(-1))
                        } else {
                            ObjectId(-1)
                        }
                    })
            });
            if let Some(cause) = cause {
                transactions::with_cause(&scripts.lua, cause, || {
                    crate::movement::depart(scripts, *id, cause, session(*id))
                })?;
            }
        }
        for relocation in &report.plan.relocations {
            // An earlier callback may have already removed or moved a later occupant.
            // C's SAFE_DOLIST verifies that it still belongs to the doomed container
            // before evacuating it, so do not overwrite the callback's live result.
            if scripts
                .world
                .borrow()
                .objects
                .get(&relocation.object)
                .map(|object| object.location)
                != Some(relocation.source)
            {
                continue;
            }
            crate::movement::perform(
                scripts,
                crate::movement::Request {
                    actor: relocation.object,
                    object: relocation.object,
                    cause: ObjectId(-1),
                    destination: relocation.destination,
                    session: session(relocation.object),
                    route: crate::movement::Route::Generic,
                },
            )?;
        }
        let mut world = scripts.world.borrow_mut();
        for id in &report.plan.purges {
            let tombstone = final_world
                .objects
                .get(id)
                .ok_or_else(|| anyhow::anyhow!("planned tombstone disappeared"))?
                .clone();
            world.objects.insert(*id, tombstone);
            world.accounts.remove(id);
        }
        let live: std::collections::BTreeSet<_> = world
            .objects
            .values()
            .filter(|o| o.kind != Kind::Garbage && !report.plan.purges.contains(&o.id))
            .map(|o| o.id)
            .collect();
        world.macros.purge(&report.plan.purges);
        world.retain_battle_rolls(&report.plan.purges)?;
        world.btech.purge(&report.plan.purges);
        world.channel_aliases.retain(|id, _| live.contains(id));
        world.last_pages.retain(|id, _| live.contains(id));
        for recipients in world.last_pages.values_mut() {
            recipients.retain(|id| live.contains(id));
        }
        for channel in world.channels.values_mut() {
            channel.users.retain(|user| live.contains(&user.who));
            if channel.object.is_some_and(|id| !live.contains(&id)) {
                channel.object = None;
            }
        }
        world.links = crate::dbck::rebuild_links(&world, &final_world.links);
        world.validate(&config)?;
        return Ok(());
    }

    for relocation in &report.plan.relocations {
        let movement = crate::movement::Move {
            actor: relocation.object,
            object: relocation.object,
            source: relocation.source,
            destination: relocation.destination,
            session: session(relocation.object),
        };
        if let Some(source) = relocation.source.filter(|id| {
            scripts.world.borrow().objects.get(id).is_some_and(|o| {
                matches!(o.kind, Kind::Room | Kind::Player | Kind::Thing)
                    && !o.flags.contains(Flag::Going)
            })
        }) {
            scripts
                .world
                .borrow_mut()
                .objects
                .get_mut(&relocation.object)
                .ok_or_else(|| anyhow::anyhow!("callback removed repaired occupant"))?
                .location = Some(source);
            scripts.movement_event("on_exit", source, &movement)?;
        }
        scripts
            .world
            .borrow_mut()
            .objects
            .get_mut(&relocation.object)
            .ok_or_else(|| anyhow::anyhow!("callback removed repaired occupant"))?
            .location = Some(relocation.destination);
        scripts.movement_event("on_enter", relocation.destination, &movement)?;
    }
    Ok(())
}

/// Apply C-visible repair and callbacks now; SQL and session effects remain staged.
pub(crate) fn check(lua: &mlua::Lua) -> mlua::Result<()> {
    transactions::require(lua)?;
    let s = Scripts::services(lua)?;
    transactions::run(lua, &s.world, || {
        let config = super::configuration(lua);
        let before = s.world.borrow().clone();
        let links = crate::dbck::rebuild_links(&before, &before.links);
        let (repaired, mut report) =
            crate::dbck::plan(&before, &links, &config).map_err(|e| failure("mux.runtime", e))?;
        *s.world.borrow_mut() = repaired;
        apply_relocations(&s, &before, &report, |_| None).map_err(|e| failure("mux.runtime", e))?;
        s.world
            .borrow()
            .validate(&config)
            .map_err(|e| failure("mux.runtime", e))?;
        report.plan.links = crate::dbck::rebuild_links(&s.world.borrow(), &links);
        s.world.borrow_mut().links = report.plan.links.clone();
        s.effects.stage_maintenance(report);
        Ok(())
    })
}
