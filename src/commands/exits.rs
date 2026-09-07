//! Shared native and shorthand exit travel, preserving executor and cause identity.
use super::CommandContext;
use crate::world::Kind;
use anyhow::Result;

/// Explicit invocation reports missing exits; shorthand permits dispatch fallback.
pub(super) enum Invocation {
    Bare,
    Explicit,
}

/// Exit matches consume the command even when traversal fails.
pub(super) fn travel(ctx: &CommandContext<'_>, line: &str, invocation: Invocation) -> Result<bool> {
    let s = ctx.scripts;
    if matches!(invocation, Invocation::Bare) {
        let definition = s
            .commands
            .definitions()
            .find(|d| {
                d.name == "goto"
                    && d.scope == super::CommandScope::Global
                    && matches!(d.handler, super::CommandHandler::Native(_))
            })
            .expect("goto is a registered native command");
        if definition
            .permission
            .denial(
                &s.world.borrow(),
                ctx.player,
                crate::access::Context {
                    queue_enabled: s.queue_enabled.get(),
                },
            )
            .is_some()
        {
            return Ok(false);
        }
    }
    let player = ctx.player;
    let session = ctx.session;
    let Some(room) = s
        .world
        .borrow()
        .objects
        .get(&player)
        .and_then(|o| o.location)
    else {
        if matches!(invocation, Invocation::Explicit) {
            s.outbox
                .borrow_mut()
                .push((player, "You can't go that way.".into()));
            return Ok(true);
        }
        return Ok(false);
    };

    let line = line.trim();
    let exits: Vec<_> = s
        .world
        .borrow()
        .objects
        .values()
        .filter(|o| {
            o.kind == Kind::Exit
                && o.location == Some(room)
                && crate::text::plain_with(&s.palette, &o.name)
                    .split(';')
                    .any(|n| n.eq_ignore_ascii_case(line))
        })
        .map(|o| (o.id, o.destination))
        .collect();
    let preferred = s.prefer_matches(player, exits.iter().map(|e| e.0).collect(), session)?;
    let exits: Vec<_> = exits
        .into_iter()
        .filter(|e| preferred.contains(&e.0))
        .collect();
    match exits.as_slice() {
        [(exit, Some(destination))] => {
            if s.traversal(player, *exit, session)? {
                crate::movement::perform(
                    s,
                    crate::movement::Request {
                        actor: player,
                        object: player,
                        cause: ctx.cause,
                        destination: *destination,
                        session,
                        route: crate::movement::Route::Exit { exit: *exit },
                    },
                )?;
            }
        }
        [(_, None)] => s
            .outbox
            .borrow_mut()
            .push((player, "You can't go that way.".into())),
        [] => {
            if matches!(invocation, Invocation::Bare) {
                return Ok(false);
            }
            s.outbox
                .borrow_mut()
                .push((player, "You can't go that way.".into()));
        }
        _ => s.outbox.borrow_mut().push((
            player,
            match invocation {
                Invocation::Bare => "I don't know which exit you mean.",
                Invocation::Explicit => "I don't know which way you mean!",
            }
            .into(),
        )),
    }
    Ok(true)
}
