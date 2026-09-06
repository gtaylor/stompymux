//! Server-owned Lua runtime and built-in packages; editable game modules stay in game/lua.
mod callbacks;
mod dispatch;
mod loading;
mod packages;
mod runtime;
mod sandbox;
pub(crate) mod transactions;

use crate::{
    text,
    world::{ObjectId, World},
};
use mlua::{Lua, Table};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

/// World ownership remains on the serial world thread shared with Lua closures.
pub type SharedWorld = Rc<RefCell<World>>;

/// Transaction-staged messages, rendered separately for each receiving session.
pub type Outbox = Rc<RefCell<Vec<(ObjectId, text::Document)>>>;

/// Lua owner and loaded game modules with runtime-only shared resources.
pub struct Scripts {
    /// VM handle for world-thread execution and session snapshot publication.
    pub lua: Lua,
    /// Shared world mutated by built-in operations and callbacks.
    pub world: SharedWorld,
    /// Pending messages owned by the surrounding world transaction.
    pub outbox: Outbox,
    /// Global module tables retained in lexical loading order.
    globals: Vec<Table>,
    /// Immutable native and Lua command catalog captured during module loading.
    pub commands: crate::commands::CommandRegistry,
    /// Object module tables keyed by relative parent path.
    parents: BTreeMap<String, Table>,
    /// Shared instruction budget also captured by the VM hook.
    budget: sandbox::InstructionBudget,
    /// Deferred capability diagnostics discovered during module loading.
    pub warnings: Vec<String>,
    /// Shared immutable rendering catalogs and startup help index.
    pub palette: std::sync::Arc<text::Palette>,
    /// Immutable, permission-filtered game help index.
    pub help: crate::help::HelpIndex,
}

/// Preserve the Lua-facing runtime error representation used by all built-in packages.
fn err(e: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::RuntimeError(e.to_string())
}

impl Scripts {
    /// Begin a native communication operation with one shared callback budget.
    pub fn communication<'a>(
        &'a self,
        config: &'a crate::config::Config,
    ) -> crate::communication::Service<'a> {
        self.budget.reset();
        crate::communication::Service {
            world: &self.world,
            outbox: &self.outbox,
            config,
            lua: &self.lua,
        }
    }
}

impl Scripts {
    /// Invoke a protected game callback with rollback and state availability.
    pub fn call<T: mlua::FromLuaMulti>(
        &self,
        f: &mlua::Function,
        args: impl mlua::IntoLuaMulti,
    ) -> anyhow::Result<T> {
        transactions::run(&self.lua, &self.world, &self.outbox, || f.call(args))
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    /// Execute a callback chunk with the same transaction and budget as game handlers.
    pub fn eval_callback<T: mlua::FromLuaMulti>(&self, source: &str) -> anyhow::Result<T> {
        self.budget.reset();
        let f = self
            .lua
            .load(source)
            .into_function()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        self.call(&f, ())
    }
}
