//! Transactional container movement shared by home, teleport and exit travel.
use crate::{
    flags::Flag,
    lua::Scripts,
    world::{Kind, ObjectId},
};
use anyhow::{Result, ensure};
/// Movement route determines policy checks and user feedback.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Route {
    /// Stored home, bypassing movement locks.
    Home,
    /// Administrative movement with teleport policies.
    Teleport,
    /// Exit travel, after its traversal lock has passed.
    Exit { exit: ObjectId },
    /// Ordinary inventory/container relocation with policies evaluated by its caller.
    Generic,
}

/// Explicit execution identity and route for one transactional relocation.
#[derive(Clone, Copy)]
pub struct Request {
    /// Object executing the operation, whose descriptor the caller may supply.
    pub actor: ObjectId,
    /// Object to relocate; never inferred from the causal actor.
    pub object: ObjectId,
    /// Original causal actor retained across forced and queued execution.
    pub cause: ObjectId,
    /// Immediate destination container.
    pub destination: ObjectId,
    /// Initiating session; stripped when moving a different object.
    pub session: Option<u64>,
    /// Policy and callback sequence, including the matched exit when traversing.
    pub route: Route,
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
pub fn perform(s: &Scripts, request: Request) -> Result<()> {
    let before = s.world.borrow().clone();
    let checkpoint = s.effects.checkpoint();
    let result = crate::lua::transactions::with_cause(&s.lua, request.cause, || apply(s, request));
    if result.is_err() {
        *s.world.borrow_mut() = before;
        s.effects.restore(checkpoint);
    }
    result
}

/// Run the generic departure used when a GOING player or thing moves to NOTHING.
pub(crate) fn depart(
    s: &Scripts,
    object: ObjectId,
    cause: ObjectId,
    session: Option<u64>,
) -> Result<()> {
    let (source, generation, source_generation, hear) = {
        let world = s.world.borrow();
        let value = world
            .objects
            .get(&object)
            .ok_or_else(|| anyhow::anyhow!("Departing object missing"))?;
        ensure!(
            matches!(value.kind, Kind::Player | Kind::Thing),
            "Only players and things depart during purge."
        );
        let source = value.location;
        (
            source,
            value.generation,
            source.and_then(|id| world.objects.get(&id).map(|source| (id, source.generation))),
            value.flags.contains(Flag::Connected),
        )
    };
    let Some(source_id) = source else {
        return Ok(());
    };
    let validate = || -> Result<()> {
        let world = s.world.borrow();
        let value = world
            .objects
            .get(&object)
            .ok_or_else(|| anyhow::anyhow!("Departing object disappeared"))?;
        ensure!(
            value.generation == generation
                && value.kind != Kind::Garbage
                && value.location == Some(source_id),
            "Departing object changed during callbacks."
        );
        if let Some((id, generation)) = source_generation {
            ensure!(
                world.objects.get(&id).is_some_and(|source| {
                    source.generation == generation && source.kind != Kind::Garbage
                }),
                "Departure source changed during callbacks."
            );
        }
        Ok(())
    };
    validate()?;
    let movement = Move {
        actor: object,
        object,
        source,
        destination: ObjectId(-1),
        session,
    };
    let transitions = crate::lua::TransitionContext {
        cause,
        hear,
        excluded: cause,
    };
    s.transition_action_context(&movement, false, false, transitions)?;
    validate()?;
    s.world
        .borrow_mut()
        .objects
        .get_mut(&object)
        .unwrap()
        .location = None;
    s.action_message(
        crate::lua::ObjectAction {
            object,
            enactor: object,
            cause,
            descriptor: session,
            source,
            destination: None,
            operation: "move",
            silent: false,
        },
        "move",
        Some("on_move"),
        None,
        None,
    )?;
    s.transition_action_context(&movement, true, false, transitions)?;
    s.world
        .borrow()
        .validate(&crate::lua::configuration(&s.lua))?;
    Ok(())
}

/// Validate first, then run transition callbacks and render the resulting location.
fn apply(s: &Scripts, request: Request) -> Result<()> {
    let Request {
        actor,
        object,
        cause,
        destination,
        session,
        route,
    } = request;
    let session = session.filter(|_| actor == object);
    s.world.borrow().validate_move(object, destination)?;
    let source = s.world.borrow().objects[&object].location;
    if source == Some(destination) {
        s.outbox.borrow_mut().push((actor, "Already there.".into()));
        return Ok(());
    }
    let kind = s.world.borrow().objects[&object].kind;
    // Retain incarnation identities across callback-driven object changes.
    let identities = {
        let world = s.world.borrow();
        let mut ids = vec![object, destination];
        if let Route::Exit { exit } = route {
            ids.push(exit);
        }
        ids.into_iter()
            .map(|id| {
                let value = world
                    .objects
                    .get(&id)
                    .ok_or_else(|| anyhow::anyhow!("Movement object {id:?} missing"))?;
                ensure!(
                    value.kind != Kind::Garbage,
                    "Movement object is unavailable."
                );
                Ok((id, value.generation))
            })
            .collect::<Result<Vec<_>>>()?
    };
    let validate = || -> Result<()> {
        let world = s.world.borrow();
        for (id, generation) in &identities {
            let value = world
                .objects
                .get(id)
                .ok_or_else(|| anyhow::anyhow!("Movement object missing"))?;
            ensure!(
                value.generation == *generation && value.kind != Kind::Garbage,
                "Movement object changed during callbacks."
            );
        }
        if let Route::Exit { exit } = route {
            let exit = &world.objects[&exit];
            let location_zone = source
                .and_then(|id| world.objects.get(&id))
                .and_then(|o| o.zone);
            let zone_origin = location_zone
                .is_some_and(|zone| world.objects.get(&zone).map(|o| o.kind) == Some(Kind::Room))
                && source != world.objects[&actor].zone
                && world.objects[&actor].zone == exit.location
                && exit
                    .location
                    .and_then(|id| world.objects.get(&id))
                    .is_some_and(|o| matches!(o.kind, Kind::Room | Kind::Thing | Kind::Player));
            ensure!(
                exit.kind == Kind::Exit
                    && (exit.location == source || zone_origin)
                    && exit.destination == Some(destination),
                "Exit changed during traversal."
            );
        }
        world.validate_move(object, destination)
    };
    validate()?;
    let movement = Move {
        actor,
        object,
        source,
        destination,
        session,
    };
    if route == Route::Teleport {
        let mut policies = vec![(
            destination,
            crate::LockType::Teleport,
            actor,
            "You can't teleport there!",
        )];
        if kind != Kind::Exit {
            for location in s.world.borrow().containment_chain(source)? {
                policies.push((
                    location,
                    crate::LockType::TeleportOut,
                    object,
                    "You can't teleport out!",
                ));
            }
        }
        for (location, lock, subject, default) in policies {
            let outcome = s.movement_lock_outcome(location, lock, subject, &movement)?;
            if !outcome.passes {
                let ctx = s.context(Some(object), Some(location), session)?;
                ctx.set("lock", lock.key())
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                ctx.set("cause", cause.0)
                    .map_err(|e| anyhow::anyhow!("{e}"))?;
                if actor != object {
                    s.outbox
                        .borrow_mut()
                        .push((actor, "Permission denied.".into()));
                }
                s.lock_denied(ctx, &outcome, default)?;
                return Ok(());
            }
            validate()?;
        }
    }

    if route == Route::Home {
        let w = s.world.borrow();
        let o = &w.objects[&object];
        if let Some(source) = source
            && !o.flags.contains(Flag::Dark)
            && !w.objects[&source].flags.contains(Flag::Dark)
        {
            crate::notification::send(
                &w,
                &s.outbox,
                &crate::lua::configuration(&s.lua),
                crate::notification::Request {
                    target: source,
                    sender: object,
                    document: format!("{} goes home.", o.name).into(),
                    policy: crate::notification::Policy::ROOM,
                    exclusions: Some(vec![object]),
                },
            )?;
        }
        for _ in 0..3 {
            s.outbox
                .borrow_mut()
                .push((object, "There's no place like home...".into()));
        }
    }
    let generic = matches!(route, Route::Generic);
    let (hear, dark, dark_wizard) = {
        let w = s.world.borrow();
        let flags = &w.objects[&object].flags;
        (
            flags.contains(Flag::Connected),
            flags.contains(Flag::Dark),
            flags.contains(Flag::Dark) && crate::authority::is_wizard(&w, object),
        )
    };
    let hush = route == Route::Teleport && dark;
    // Home and teleport transitions use NOTHING; ordinary travel retains its cause.
    let transition_cause = if matches!(route, Route::Home | Route::Teleport) {
        ObjectId(-1)
    } else {
        cause
    };
    let transitions = crate::lua::TransitionContext {
        cause: transition_cause,
        hear,
        excluded: transition_cause,
    };
    let action = crate::lua::ObjectAction {
        object,
        enactor: object,
        cause: if route == Route::Home {
            ObjectId(-1)
        } else {
            cause
        },
        descriptor: session,
        source,
        destination: Some(destination),
        operation: if route == Route::Teleport {
            "teleport"
        } else {
            "move"
        },
        silent: false,
    };
    if route == Route::Teleport && kind != Kind::Exit && !hush {
        s.action_message(action, "teleport_source", None, None, None)?;
        validate()?;
    }
    let exit_action = if let Route::Exit { exit } = route {
        Some(crate::lua::ObjectAction {
            object: exit,
            operation: "traverse",
            silent: dark_wizard,
            ..action
        })
    } else {
        None
    };
    if let Some(exit_action) = exit_action {
        s.action_message(exit_action, "success", Some("on_success"), None, None)?;
        validate()?;
    }
    if kind != Kind::Exit {
        s.transition_action_context(&movement, false, hush, transitions)?;
    }
    // Providers can mutate the world; recheck before assigning the location.
    validate()?;
    s.world
        .borrow_mut()
        .objects
        .get_mut(&object)
        .unwrap()
        .location = Some(destination);
    let render = || -> Result<()> {
        if kind == Kind::Player
            && (session.is_some()
                || s.world.borrow().objects[&object]
                    .flags
                    .contains(Flag::Connected))
        {
            let text = s.appearance_for(object, destination, session)?;
            s.outbox.borrow_mut().push((object, text.into()));
        }
        Ok(())
    };
    // Appearance precedes all post-relocation actions, for every movement route.
    render()?;
    validate()?;
    if let Some(exit_action) = exit_action {
        s.action_message(exit_action, "drop", Some("on_drop"), None, None)?;
        validate()?;
    }
    if kind != Kind::Exit {
        if route == Route::Teleport && !hush {
            s.action_message(action, "teleport", Some("on_teleport"), None, None)?;
            validate()?;
        }
        s.action_message(action, "move", Some("on_move"), None, None)?;
        validate()?;
        s.transition_action_context(&movement, true, hush, transitions)?;
    }
    validate()?;
    s.world
        .borrow()
        .validate(&crate::lua::configuration(&s.lua))?;
    if object != actor && !generic {
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
