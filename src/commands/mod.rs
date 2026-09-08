//! C-ordered native and scoped command dispatch with transactional handlers.
mod context;
pub mod discovery;
mod exits;
pub(crate) mod inspection;
mod native;
mod objects;
pub mod queue;
mod registry;
mod result;
pub mod sources;
pub(crate) mod target;
use crate::{
    config::Config,
    lua::Scripts,
    world::{Kind, ObjectId},
};
use anyhow::Result;
pub use context::{CommandContext, ExecutionContext, InputOrigin, executable};
pub use registry::*;
pub use result::{Action, Report, ServerRequest};
/// Dispatch an authenticated interactive command.
pub fn run(s: &Scripts, c: &Config, player: ObjectId, session: u64, line: &str) -> Result<Action> {
    execute(
        s,
        c,
        ExecutionContext {
            executor: player,
            cause: player,
            session: Some(session),
            origin: InputOrigin::Interactive,
        },
        line,
    )
}

/// Render the connecting player's current location without command matching or audit/cost effects.
pub(crate) fn look_in(
    s: &Scripts,
    c: &Config,
    player: ObjectId,
    session: u64,
) -> Result<Option<String>> {
    let ctx = CommandContext {
        scripts: s,
        config: c,
        player,
        object: None,
        session: Some(session),
        cause: player,
        origin: InputOrigin::Interactive,
    };
    let input = CommandInput {
        name: "look".into(),
        args: String::new(),
        switch: None,
        line: "look".into(),
    };
    match objects::look::look(&ctx, &input)? {
        Action::Continue => Ok(None),
        Action::Report(crate::commands::Report::Reply(error)) => Ok(Some(error)),
        _ => unreachable!("look handler returned a server-only action"),
    }
}

/// Dispatch with an explicit executor, causal actor and optional connection.
pub fn execute(s: &Scripts, c: &Config, execution: ExecutionContext, line: &str) -> Result<Action> {
    anyhow::ensure!(
        execution.origin != InputOrigin::Queued || execution.session.is_none(),
        "Queued commands cannot borrow an interactive session."
    );
    let before = s.world.borrow().clone();
    let effects = s.effects.checkpoint();
    s.reset_command_callbacks();
    let result = crate::lua::transactions::with_cause(&s.lua, execution.cause, || {
        crate::lua::transactions::with_descriptor(&s.lua, execution.session, || {
            run_inner(s, c, execution, line)
        })
    });
    if result.is_err() {
        *s.world.borrow_mut() = before;
        s.effects.restore(effects);
    }
    result
}

fn run_inner(s: &Scripts, c: &Config, execution: ExecutionContext, line: &str) -> Result<Action> {
    let player = execution.executor;
    let session = execution.session;
    if !executable(&s.world.borrow(), execution) {
        if s.world
            .borrow()
            .objects
            .get(&player)
            .is_some_and(|object| object.kind != Kind::Garbage)
        {
            s.outbox.borrow_mut().push((
                player,
                format!("Attempt to execute command by halted object #{}", player.0).into(),
            ));
        }
        return Ok(Action::Continue);
    }
    let ctx = CommandContext {
        scripts: s,
        object: None,
        config: c,
        player,
        session,
        cause: execution.cause,
        origin: execution.origin,
    };
    if execution.origin == InputOrigin::Interactive
        && line.trim().starts_with('.')
        && !s
            .world
            .borrow()
            .objects
            .get(&player)
            .is_some_and(|o| o.kind == Kind::Player)
    {
        return Ok(Action::Report(crate::commands::Report::Reply(
            "MACRO: Only players may use macro sets.".into(),
        )));
    }
    let direct = CommandInput::parse(c, line);
    if execution.origin == InputOrigin::Interactive
        && let Some((definition, input)) = s.commands.native_match(direct)
        && line.trim().starts_with('.')
        && definition.direct_input_only
    {
        return definition.invoke_native(&ctx, &input);
    }
    let expanded = if execution.origin == InputOrigin::Interactive {
        match s
            .world
            .borrow()
            .macros
            .expand(player, line.trim(), c.runtime.input_line_limit)
        {
            Ok(expanded) => expanded,
            Err(error) => {
                return Ok(Action::Report(crate::commands::Report::Reply(
                    error.to_string(),
                )));
            }
        }
    } else {
        None
    };
    let line = expanded.as_deref().unwrap_or(line);
    if line.is_empty() {
        return Ok(Action::Report(
            crate::commands::Report::Reply(String::new()),
        ));
    }
    if let Some(action) = crate::communication::alias(s, c, player, line)? {
        return Ok(action);
    }
    let input = CommandInput::parse(c, line);
    if exits::travel(&ctx, line, exits::Invocation::Bare)? {
        return Ok(Action::Continue);
    }
    if let Some((definition, input)) = s.commands.native_match(input.clone())
        && !definition.direct_input_only
    {
        if expanded.is_some() && definition.no_macro {
            return Ok(Action::Report(crate::commands::Report::Reply(
                "This command is unavailable as macro. Please use an alias instead.".into(),
            )));
        }
        return definition.invoke_native(&ctx, &input);
    }
    let sources = sources::dispatch_sources(&s.world.borrow(), player);
    let local: Vec<_> = sources
        .iter()
        .filter(|source| !source.fallback())
        .copied()
        .collect();
    let location_fallback: Vec<_> = sources
        .iter()
        .filter(|source| source.stage == "location-zone fallback")
        .copied()
        .collect();
    let player_fallback: Vec<_> = sources
        .iter()
        .filter(|source| source.stage == "player-zone fallback")
        .copied()
        .collect();
    if s.dispatch_local_sources(player, session, &input.line, &local)? {
        return Ok(Action::Continue);
    }
    for source in sources.iter().filter(|source| !source.fallback()) {
        let parent = {
            let world = s.world.borrow();
            if !sources::eligible(&world, source.object) {
                continue;
            }
            world.objects[&source.object].lua_parent.clone()
        };
        if let Some((definition, input)) = s
            .commands
            .native_match_scope(input.clone(), &CommandScope::Object(parent))
        {
            let local = CommandContext {
                object: Some(source.object),
                ..ctx.clone()
            };
            if expanded.is_some() && definition.no_macro {
                return Ok(Action::Report(crate::commands::Report::Reply(
                    "This command is unavailable as macro. Please use an alias instead.".into(),
                )));
            }
            return definition.invoke_native(&local, &input);
        }
    }
    if s.dispatch_local_sources(player, session, &input.line, &location_fallback)? {
        return Ok(Action::Continue);
    }
    for source in sources
        .iter()
        .filter(|source| source.stage == "location-zone fallback")
    {
        let parent = {
            let world = s.world.borrow();
            if !sources::eligible(&world, source.object) {
                continue;
            }
            world.objects[&source.object].lua_parent.clone()
        };
        if let Some((definition, input)) = s
            .commands
            .native_match_scope(input.clone(), &CommandScope::Object(parent))
        {
            let local = CommandContext {
                object: Some(source.object),
                ..ctx.clone()
            };
            if expanded.is_some() && definition.no_macro {
                return Ok(Action::Report(crate::commands::Report::Reply(
                    "This command is unavailable as macro. Please use an alias instead.".into(),
                )));
            }
            return definition.invoke_native(&local, &input);
        }
    }
    if s.dispatch_local_sources(player, session, &input.line, &player_fallback)? {
        return Ok(Action::Continue);
    }
    for source in sources
        .iter()
        .filter(|source| source.stage == "player-zone fallback")
    {
        let parent = {
            let world = s.world.borrow();
            if !sources::eligible(&world, source.object) {
                continue;
            }
            world.objects[&source.object].lua_parent.clone()
        };
        if let Some((definition, input)) = s
            .commands
            .native_match_scope(input.clone(), &CommandScope::Object(parent))
        {
            let local = CommandContext {
                object: Some(source.object),
                ..ctx.clone()
            };
            if expanded.is_some() && definition.no_macro {
                return Ok(Action::Report(crate::commands::Report::Reply(
                    "This command is unavailable as macro. Please use an alias instead.".into(),
                )));
            }
            return definition.invoke_native(&local, &input);
        }
    }
    if exits::travel_zone(&ctx, line)? {
        return Ok(Action::Continue);
    }
    if s.dispatch_global(player, session, &input.line)? {
        return Ok(Action::Continue);
    }
    c.log(
        &[crate::logging::Category::BadCommands],
        "CMD",
        "BAD",
        crate::logging::audit::message(c, &s.world.borrow(), execution, line),
    );
    s.outbox.borrow_mut().push((
        player,
        "Huh? (Type look, say <message>, WHO, an exit name, or quit.)".into(),
    ));
    Ok(Action::Continue)
}
