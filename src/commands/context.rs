//! Stable identity and origin passed through command dispatch.

use crate::{
    config::Config,
    lua::Scripts,
    world::{Kind, ObjectId, World},
};
use anyhow::{Context, Result};

/// Shared inputs available to registered native handlers.
#[derive(Clone)]
pub struct CommandContext<'a> {
    /// Lua and world services owned by the world thread.
    pub scripts: &'a Scripts,
    /// Effective server configuration.
    pub config: &'a Config,
    /// Authenticated invoking player.
    pub player: ObjectId,
    /// Object providing a local native handler; absent for global built-ins.
    pub object: Option<ObjectId>,
    /// Invoking session identifier.
    pub session: Option<u64>,
    /// Original causal actor, separate from execution authority.
    pub cause: ObjectId,
    /// Interactive or background dispatch.
    pub origin: InputOrigin,
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

/// Origin controls session-only commands without borrowing another connection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputOrigin {
    /// Authenticated connection input.
    Interactive,
    /// Descriptor-free execution owned by the runtime command queue.
    Queued,
}

/// Identity and connection information shared by every dispatch path.
#[derive(Clone, Copy, Debug)]
pub struct ExecutionContext {
    /// Object whose permissions and surroundings govern dispatch.
    pub executor: ObjectId,
    /// Causal actor; never a source of elevated command authority.
    pub cause: ObjectId,
    /// Real invoking connection, absent for background execution.
    pub session: Option<u64>,
    /// How this command entered the dispatcher.
    pub origin: InputOrigin,
}

/// Whether the executor may enter command dispatch under the C lifecycle guard.
pub fn executable(world: &World, execution: ExecutionContext) -> bool {
    let Some(object) = world.objects.get(&execution.executor) else {
        return false;
    };
    if object.kind == Kind::Garbage || object.flags.contains(crate::flags::Flag::Going) {
        return false;
    }
    !object.flags.contains(crate::flags::Flag::Halted)
        || object.kind == Kind::Player && execution.origin == InputOrigin::Interactive
}
