//! Server-owned Lua runtime and built-in packages; editable game modules stay in game/lua.
mod access;
pub use access::WorldInspection;
mod actions;
pub(crate) mod admin;
pub(crate) mod maintenance;
pub(crate) mod sessions;
mod view;
pub use admin::AdminRequest;
mod appearance;
pub use appearance::AppearanceMode;
mod callbacks;
pub(crate) mod command_access;
mod communication;
pub(crate) use actions::ActionContent;
pub use actions::{ObjectAction, TransitionContext};
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
    runtime::{Effects, Outbox, SharedWorld},
    text,
};
use mlua::{Lua, Table};
use std::collections::BTreeMap;

/// Lua owner and loaded game modules with runtime-only shared resources.
pub struct Scripts {
    /// Process-local tick timing shared by reloads of this Lua owner.
    pub(crate) event_telemetry: std::rc::Rc<std::cell::Cell<crate::BattleEventTelemetry>>,
    /// Runtime-only session flows and staged effects.
    pub(crate) flows: flows::Engine,
    /// Source identity and package contents captured when this runtime was built.
    pub(crate) sources: std::sync::Arc<sources::Sources>,
    /// VM handle for world-thread execution and session snapshot publication.
    pub(crate) lua: Lua,
    /// Shared world mutated by built-in operations and callbacks.
    pub(crate) world: SharedWorld,
    /// Pending messages owned by the surrounding world transaction.
    pub(crate) outbox: Outbox,
    /// Runtime-owned candidate effects shared by native and Lua operations.
    pub(crate) effects: Effects,
    /// Global module tables retained in lexical loading order.
    globals: Vec<Table>,
    /// Immutable native and Lua command catalog captured during module loading.
    pub(crate) commands: crate::commands::CommandRegistry,
    /// Live queue prerequisite published by the world owner; excluded from rollback.
    pub(crate) queue_enabled: std::cell::Cell<bool>,
    /// Periodic-work counters the world owner publishes; reloads inherit the channel.
    pub(crate) progress: tokio::sync::watch::Sender<crate::RuntimeProgress>,
    /// Object module tables keyed by relative parent path.
    parents: BTreeMap<String, Table>,
    /// Captured, validated schedules, independent of mutable module tables.
    pub(crate) schedules: schedules::Catalog,
    /// Shared instruction budget also captured by the VM hook.
    budget: sandbox::InstructionBudget,
    /// Deferred capability diagnostics discovered during module loading.
    pub(crate) warnings: Vec<String>,
    /// Shared immutable rendering catalogs and startup help index.
    pub(crate) palette: std::sync::Arc<text::Palette>,
    /// Immutable, permission-filtered game help index.
    pub(crate) help: crate::help::HelpIndex,
}

/// Preserve the Lua-facing runtime error representation used by all built-in packages.
fn err(e: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::RuntimeError(e.to_string())
}

impl Scripts {
    /// Override process-local event timing for diagnostics and contract tests.
    pub fn configure_battle_event_telemetry(&self, process_start: i64, ticks: u64) {
        self.event_telemetry.set(crate::BattleEventTelemetry {
            process_start,
            ticks,
        });
    }

    /// Subscribe to the counters the world owner publishes after each heartbeat and
    /// maintenance tick. Take the receiver before handing these scripts to the server.
    pub fn progress(&self) -> tokio::sync::watch::Receiver<crate::RuntimeProgress> {
        self.progress.subscribe()
    }

    /// Publish one finished step's counters to every progress receiver.
    pub(crate) fn record_progress(&self, update: impl FnOnce(&mut crate::RuntimeProgress)) {
        self.progress.send_modify(update);
    }

    pub(crate) fn record_battle_event_tick(&self) {
        let mut telemetry = self.event_telemetry.get();
        telemetry.ticks = telemetry.ticks.saturating_add(1);
        self.event_telemetry.set(telemetry);
    }

    /// Nested native-to-Lua callbacks share the caller's budget rather than replenishing it.
    fn reset_callback_budget(&self) {
        if !transactions::active(&self.lua) {
            self.budget.reset();
        }
    }
}

impl Scripts {
    /// Begin a native communication operation with one shared callback budget.
    pub fn communication<'a>(
        &'a self,
        config: &'a crate::config::Config,
    ) -> crate::communication::Service<'a> {
        self.reset_callback_budget();
        crate::communication::Service {
            world: &self.world,
            outbox: &self.outbox,
            effects: &self.effects,
            config: crate::communication::ServiceConfig::Borrowed(config),
            host: &self.lua,
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
                transactions::run(&self.lua, &self.world, || f.call(args))
            })
        })
        .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    /// Resume interactive input with a fresh budget shared by all immediate transitions.
    pub fn flow_input(&self, session: u64, input: &str) -> anyhow::Result<()> {
        self.reset_callback_budget();
        self.flows
            .input(&self.lua, session, input)
            .map_err(|e| anyhow::anyhow!("{e}"))
    }

    /// Execute a callback chunk with the same transaction and budget as game handlers.
    pub fn eval_callback<T: mlua::FromLuaMulti>(&self, source: &str) -> anyhow::Result<T> {
        self.reset_callback_budget();
        let f = self
            .lua
            .load(source)
            .into_function()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        self.call(&f, ())
    }
}

/// Read one effective configuration snapshot at a Lua operation boundary.
///
/// The snapshot is shared: cloning the handle is a reference-count increment, so hot paths
/// such as per-message notification can take it freely. Reconfiguration installs a new
/// snapshot rather than mutating this one.
pub(crate) fn configuration(lua: &Lua) -> std::sync::Arc<crate::config::Config> {
    lua.app_data_ref::<std::sync::Arc<crate::config::Config>>()
        .expect("configuration installed before packages")
        .clone()
}
impl Scripts {
    /// Prepare access and VM resource changes before publishing a live configuration.
    pub fn configure(&mut self, config: &crate::config::Config) -> anyhow::Result<()> {
        for parent in [
            &config.mux.default_player_lua_parent,
            &config.mux.default_thing_lua_parent,
            &config.mux.default_room_lua_parent,
            &config.mux.default_exit_lua_parent,
        ] {
            anyhow::ensure!(
                parent.is_empty() || self.parents.contains_key(parent),
                "Unknown default Lua parent {parent}"
            );
        }
        let mut commands = self.commands.clone();
        commands.configure_access(config)?;
        self.lua
            .set_memory_limit(config.lua.memory_limit)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        self.commands = commands;
        self.lua.set_app_data(std::sync::Arc::new(config.clone()));
        crate::configure_battle_perception(
            &mut self.world.borrow_mut(),
            config.battletech.sensor_range,
        );
        Ok(())
    }
}
