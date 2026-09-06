//! Transactional container movement shared by home, teleport and exit travel.
use crate::{
    flags::Flag,
    lua::Scripts,
    world::{Kind, ObjectId},
};
use anyhow::Result;
/// Movement route determines policy checks and user feedback.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Stored home, bypassing movement locks.
    Home,
    /// Administrative movement with teleport policies.
    Teleport,
    /// Exit travel, after its traversal lock has passed.
    Exit,
}
/// Context shared by movement hooks and lock callbacks.
pub struct Move {
    /// Wizard or traveler initiating the move.
    pub actor: ObjectId,
    /// Object whose immediate location changes.
    pub object: ObjectId,
    /// Immediate source, absent for an unplaced object.
    pub source: Option<ObjectId>,
    /// Immediate destination container.
    pub destination: ObjectId,
    /// Initiating descriptor only when it belongs to the moved player.
    pub session: Option<u64>,
}
/// Apply movement as one command mutation, restoring callback changes on failure.
pub fn perform(
    s: &Scripts,
    actor: ObjectId,
    object: ObjectId,
    destination: ObjectId,
    session: Option<u64>,
    route: Route,
) -> Result<()> {
    let before = s.world.borrow().clone();
    let pending = s.outbox.borrow().len();
    let result = apply(s, actor, object, destination, session, route);
    if result.is_err() {
        *s.world.borrow_mut() = before;
        s.outbox.borrow_mut().truncate(pending);
    }
    result
}
/// Validate first, then run transition callbacks and render the resulting location.
fn apply(
    s: &Scripts,
    actor: ObjectId,
    object: ObjectId,
    destination: ObjectId,
    session: Option<u64>,
    route: Route,
) -> Result<()> {
    s.world.borrow().validate_move(object, destination)?;
    let source = s.world.borrow().objects[&object].location;
    if source == Some(destination) {
        s.outbox.borrow_mut().push((actor, "Already there.".into()));
        return Ok(());
    }
    let kind = s.world.borrow().objects[&object].kind;
    let movement = Move {
        actor,
        object,
        source,
        destination,
        session,
    };
    if route == Route::Teleport {
        let mut policies = vec![(destination, "teleport", actor, "You can't teleport there!")];
        if kind != Kind::Exit {
            for location in s.world.borrow().containment_chain(source)? {
                policies.push((location, "teleport_out", object, "You can't teleport out!"));
            }
        }
        for (location, lock, subject, default) in policies {
            let outcome = s.movement_lock_outcome(location, lock, subject, &movement)?;
            if !outcome.passes {
                let ctx = s.context(Some(object), Some(location), session)?;
                ctx.set("lock", lock).map_err(|e| anyhow::anyhow!("{e}"))?;
                ctx.set("cause", actor.0)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                if actor != object {
                    s.outbox
                        .borrow_mut()
                        .push((actor, "Permission denied.".into()));
                }
                s.lock_denied(ctx, &outcome, default)?;
                return Ok(());
            }
        }
    }

    if route == Route::Home {
        let w = s.world.borrow();
        let o = &w.objects[&object];
        if let Some(source) = source
            && !o.flags.contains(Flag::Dark)
            && !w.objects[&source].flags.contains(Flag::Dark)
        {
            for recipient in w
                .objects
                .values()
                .filter(|p| p.kind == Kind::Player && p.id != object && p.location == Some(source))
            {
                s.outbox
                    .borrow_mut()
                    .push((recipient.id, format!("{} goes home.", o.name).into()));
            }
        }
        for _ in 0..3 {
            s.outbox
                .borrow_mut()
                .push((object, "There's no place like home...".into()));
        }
    }
    if kind != Kind::Exit
        && let Some(source) = source
    {
        s.movement_event("on_exit", source, &movement)?;
    }
    // Callbacks may change containment; check again before committing the location.
    s.world.borrow().validate_move(object, destination)?;
    s.world
        .borrow_mut()
        .objects
        .get_mut(&object)
        .unwrap()
        .location = Some(destination);
    if kind != Kind::Exit {
        s.movement_event("on_enter", destination, &movement)?;
    }
    if kind == Kind::Player
        && (session.is_some()
            || s.world.borrow().objects[&object]
                .flags
                .contains(Flag::Connected))
    {
        let text = s.appearance_for(object, destination, session)?;
        s.outbox.borrow_mut().push((object, text.into()));
    }
    if object != actor {
        s.outbox.borrow_mut().push((
            actor,
            if kind == Kind::Exit {
                "Exit teleported."
            } else {
                "Teleported."
            }
            .into(),
        ));
    }
    Ok(())
}
