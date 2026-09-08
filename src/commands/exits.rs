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

    travel_from(ctx, line, invocation, room)
}

/// Try exits in a room-valued location zone after every scoped local fallback.
pub(super) fn travel_zone(ctx: &CommandContext<'_>, line: &str) -> Result<bool> {
    let world = ctx.scripts.world.borrow();
    let Some(location) = world.objects.get(&ctx.player).and_then(|o| o.location) else {
        return Ok(false);
    };
    let Some(zone) = world.objects.get(&location).and_then(|o| o.zone) else {
        return Ok(false);
    };
    let Some(exit_root) = world.objects.get(&ctx.player).and_then(|o| o.zone) else {
        return Ok(false);
    };
    if location == exit_root
        || world.objects.get(&zone).map(|o| o.kind) != Some(Kind::Room)
        || !world
            .objects
            .get(&exit_root)
            .is_some_and(|o| matches!(o.kind, Kind::Room | Kind::Thing | Kind::Player))
    {
        return Ok(false);
    }
    drop(world);
    travel_from(ctx, line, Invocation::Bare, exit_root)
}

/// Match and traverse exits rooted at one explicit room.
fn travel_from(
    ctx: &CommandContext<'_>,
    line: &str,
    invocation: Invocation,
    room: crate::world::ObjectId,
) -> Result<bool> {
    let s = ctx.scripts;
    let player = ctx.player;
    let session = ctx.session;
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
    if exits.len() > 1 && matches!(invocation, Invocation::Explicit) {
        s.outbox
            .borrow_mut()
            .push((player, "I don't know which way you mean!".into()));
        return Ok(true);
    }
    let selected = match exits.len() {
        0 => None,
        1 => exits.first(),
        n => Some(&exits[rand::random_range(0..n)]),
    };
    match selected {
        Some((exit, Some(destination))) => {
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
        Some((_, None)) => s
            .outbox
            .borrow_mut()
            .push((player, "You can't go that way.".into())),
        None => {
            if matches!(invocation, Invocation::Bare) {
                return Ok(false);
            }
            s.outbox
                .borrow_mut()
                .push((player, "You can't go that way.".into()));
        }
    }
    Ok(true)
}
