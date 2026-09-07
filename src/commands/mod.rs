//! Registry-driven commands with transactional native/Lua handlers and exit fallback.
mod native;
mod objects;
mod registry;
pub(crate) mod target;
use crate::{
    config::Config,
    lua::Scripts,
    world::{Kind, ObjectId},
};
use anyhow::{Context, Result};
pub use registry::*;
/// Result interpreted by the world/session owner.
pub enum Action {
    /// Commit callback changes and flush player-directed output.
    Continue,
    /// Commit mutations before delivering a private confirmation.
    CommitReply(String),
    /// Read-only session diagnostics.
    Sessions(String),
    Telnet(String),
    /// Captured schedule metadata, delivered only to the invoking session.
    LuaSchedules(String),
    /// Session-local rendering preferences and read-only help.
    Color(String),
    Help(String),
    /// Rebuild the immutable help metadata snapshot.
    HelpReload,
    /// Request common graceful shutdown.
    Shutdown,
    /// Run transactional database maintenance.
    DbCheck,
    /// Disconnect the invoking session.
    Quit,
    /// Read-only session-scoped object search.
    Find(crate::find::FindRequest),
    /// Bounded session-private response without persistence.
    Reply(String),
}
/// Shared inputs available to registered native handlers.
pub struct CommandContext<'a> {
    /// Lua and world services owned by the world thread.
    pub scripts: &'a Scripts,
    /// Effective server configuration.
    pub config: &'a Config,
    /// Authenticated invoking player.
    pub player: ObjectId,
    /// Invoking session identifier.
    pub session: u64,
}
impl CommandContext<'_> {
    /// Resolve a location only for commands that need one.
    pub fn location(&self) -> Result<ObjectId> {
        self.scripts
            .world
            .borrow()
            .objects
            .get(&self.player)
            .context("player missing")?
            .location
            .context("player has no location")
    }
}
/// Resolve the registry first, then Lua scopes and exit-name matching.
pub fn run(s: &Scripts, c: &Config, player: ObjectId, session: u64, line: &str) -> Result<Action> {
    crate::lua::transactions::with_descriptor(&s.lua, Some(session), || {
        run_inner(s, c, player, session, line)
    })
}
fn run_inner(
    s: &Scripts,
    c: &Config,
    player: ObjectId,
    session: u64,
    line: &str,
) -> Result<Action> {
    let ctx = CommandContext {
        scripts: s,
        config: c,
        player,
        session,
    };
    if line.trim().starts_with('.')
        && !s
            .world
            .borrow()
            .objects
            .get(&player)
            .is_some_and(|o| o.kind == Kind::Player)
    {
        return Ok(Action::Reply(
            "MACRO: Only players may use macro sets.".into(),
        ));
    }
    let direct = CommandInput::parse(c, line);
    if let Some((definition, input)) = s.commands.native_match(direct)
        && line.trim().starts_with('.')
        && definition.direct_input_only
    {
        return definition.invoke_native(&ctx, &input);
    }
    let expanded =
        match s
            .world
            .borrow()
            .macros
            .expand(player, line.trim(), c.runtime.input_line_limit)
        {
            Ok(expanded) => expanded,
            Err(error) => return Ok(Action::Reply(error.to_string())),
        };
    let line = expanded.as_deref().unwrap_or(line);
    if line.is_empty() {
        return Ok(Action::Reply(String::new()));
    }
    if let Some(action) = crate::communication::alias(s, c, player, line)? {
        return Ok(action);
    }
    let input = CommandInput::parse(c, line);
    if let Some((definition, input)) = s.commands.native_match(input.clone())
        && !definition.direct_input_only
    {
        return definition.invoke_native(&ctx, &input);
    }
    if s.dispatch(player, session, &input.line)? {
        return Ok(Action::Continue);
    }
    let room = ctx.location()?;
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
    let preferred = s.prefer_matches(player, exits.iter().map(|e| e.0).collect(), Some(session))?;
    let exits: Vec<_> = exits
        .into_iter()
        .filter(|e| preferred.contains(&e.0))
        .collect();
    match exits.as_slice() {
        [(exit, Some(destination))] => {
            if s.traversal(player, *exit, session)? {
                crate::movement::perform(
                    s,
                    player,
                    player,
                    *destination,
                    Some(session),
                    crate::movement::Route::Exit,
                )?;
            }
        }
        [(_, None)] => s
            .outbox
            .borrow_mut()
            .push((player, "You can't go that way.".into())),
        [] => s.outbox.borrow_mut().push((
            player,
            "Huh? (Type look, say <message>, WHO, an exit name, or quit.)".into(),
        )),
        _ => s
            .outbox
            .borrow_mut()
            .push((player, "I don't know which exit you mean.".into())),
    }
    Ok(Action::Continue)
}
