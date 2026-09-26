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
    /// Host teleport with quiet transitions, retaining source teleport-out policies.
    SilentTeleport,
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
    perform_with_relocation(s, request, |_| Ok(()))
}

/// Commit domain placement after departure callbacks and before arrival callbacks.
/// The hook shares the movement rollback boundary and is skipped on policy denial.
pub(crate) fn perform_with_relocation(
    s: &Scripts,
    request: Request,
    relocate: impl FnOnce(&mut crate::World) -> Result<()>,
) -> Result<()> {
    s.atomic(|_| {
        crate::lua::transactions::with_cause(&s.lua, request.cause, || {
            apply(s, request, || relocate(&mut s.world.borrow_mut()))
        })
    })
}

/// A denied participant cancels the entire movement batch without publishing staged effects.
#[derive(Debug)]
struct BatchDenied;

impl std::fmt::Display for BatchDenied {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Movement batch denied")
    }
}

impl std::error::Error for BatchDenied {}

/// Run paired domain movement through the ordinary policy and callback pipeline.
/// All departure hooks precede one domain commit; arrival hooks unwind in reverse order.
/// Denial, no-op, or callback failure restores both world state and staged effects.
pub(crate) fn perform_pair_with_relocation(
    s: &Scripts,
    requests: [Request; 2],
    relocate: impl FnOnce(&mut crate::World) -> Result<()>,
) -> Result<bool> {
    ensure!(
        requests[0].object != requests[1].object,
        "Duplicate movement participant"
    );
    let result = s.atomic(|_| {
        let mut first = false;
        crate::lua::transactions::with_cause(&s.lua, requests[0].cause, || {
            apply(s, requests[0], || {
                let mut second = false;
                crate::lua::transactions::with_cause(&s.lua, requests[1].cause, || {
                    apply(s, requests[1], || {
                        relocate(&mut s.world.borrow_mut())?;
                        second = true;
                        Ok(())
                    })
                })?;
                ensure!(second, BatchDenied);
                first = true;
                Ok(())
            })
        })?;
        ensure!(first, BatchDenied);
        Ok(())
    });
    match result {
        Ok(()) => Ok(true),
        Err(error) => {
            if error.is::<BatchDenied>() {
                return Ok(false);
            }
            Err(error)
        }
    }
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
    // Purged units leave tactical membership after departure callbacks and before losing containment.
    // Retain their final rolls before removing the simulation identity from this transaction.
    {
        let mut world = s.world.borrow_mut();
        if world.btech.units().contains_key(&object) {
            world.retain_battle_rolls(&[object].into_iter().collect())?;
            crate::btech::wreck_cleanup::forget(&mut world.btech, object);
        }
    }
    s.world
        .borrow_mut()
        .objects
        .get_mut(&object)
        .unwrap()
        .location = None;
    crate::btech::player_moved(&mut s.world.borrow_mut(), object);
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
fn apply(s: &Scripts, request: Request, relocate: impl FnOnce() -> Result<()>) -> Result<()> {
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
    if matches!(route, Route::Teleport | Route::SilentTeleport) {
        let mut policies = vec![(
            destination,
            crate::LockType::Teleport,
            actor,
            "You can't teleport there!",
        )];
        if route == Route::SilentTeleport {
            policies.clear();
        }
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
    let generic = matches!(route, Route::Generic | Route::SilentTeleport);
    let teleport = matches!(route, Route::Teleport | Route::SilentTeleport);
    let (hear, dark, dark_wizard) = {
        let w = s.world.borrow();
        let flags = &w.objects[&object].flags;
        (
            flags.contains(Flag::Connected),
            flags.contains(Flag::Dark),
            flags.contains(Flag::Dark) && crate::authority::is_wizard(&w, object),
        )
    };
    let hush = route == Route::SilentTeleport || teleport && dark;
    // Home and teleport transitions use NOTHING; ordinary travel retains its cause.
    let transition_cause = if matches!(route, Route::Home | Route::Teleport | Route::SilentTeleport)
    {
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
        operation: if teleport { "teleport" } else { "move" },
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
    relocate()?;
    s.world
        .borrow_mut()
        .objects
        .get_mut(&object)
        .unwrap()
        .location = Some(destination);
    crate::btech::player_moved(&mut s.world.borrow_mut(), object);
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
