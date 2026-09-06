//! Registry-driven commands with transactional native/Lua handlers and exit fallback.
mod native;
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
    /// Read-only session diagnostics.
    Sessions(String),
    Telnet(String),
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
    if let Some(action) = crate::communication::alias(s, c, player, line)? {
        return Ok(action);
    }
    let input = CommandInput::parse(c, line);
    let ctx = CommandContext {
        scripts: s,
        config: c,
        player,
        session,
    };
    if let Some((definition, input)) = s.commands.native_match(input.clone()) {
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
