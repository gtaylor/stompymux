//! Server-owned Lua runtime and built-in packages; editable game modules stay in game/lua.
mod actions;
pub(crate) mod admin;
pub use admin::AdminRequest;
mod appearance;
pub use appearance::AppearanceMode;
mod callbacks;
pub use actions::ObjectAction;
mod dispatch;
pub mod flows;
mod loading;
mod packages;
mod runtime;
pub use runtime::RuntimeMode;
mod sandbox;
pub mod schedules;
pub mod sources;
pub mod testing;
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
    /// Runtime-only session flows and staged effects.
    pub flows: flows::Engine,
    /// Source identity and package contents captured when this runtime was built.
    pub sources: std::sync::Arc<sources::Sources>,
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
    /// Live queue prerequisite published by the world owner; excluded from rollback.
    pub queue_enabled: std::cell::Cell<bool>,
    /// Object module tables keyed by relative parent path.
    parents: BTreeMap<String, Table>,
    /// Captured, validated schedules, independent of mutable module tables.
    pub schedules: schedules::Catalog,
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
        let args = args
            .into_lua_multi(&self.lua)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let descriptor = match args.front() {
            Some(mlua::Value::Table(ctx)) => ctx
                .get::<Option<u64>>("descriptor")
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            _ => transactions::descriptor(&self.lua),
        };
        let source = f.info().source.unwrap_or_default();
        flows::with_root(&self.lua, &source, || {
            transactions::with_descriptor(&self.lua, descriptor, || {
                transactions::run(&self.lua, &self.world, &self.outbox, || f.call(args))
            })
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    /// Resume interactive input with a fresh budget shared by all immediate transitions.
    pub fn flow_input(&self, session: u64, input: &str) -> anyhow::Result<()> {
        self.budget.reset();
        self.flows
            .input(&self.lua, session, input)
            .map_err(|e| anyhow::anyhow!("{e}"))
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
