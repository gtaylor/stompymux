//! Descriptor-owned interactive flows, with VM-independent state and transactional private output.
mod step;
use super::{RuntimeMode, transactions};
use crate::{
    runtime::transaction::{ActiveFlow, Effects, SharedWorld},
    text::Document,
};
use mlua::{Lua, Table, Value};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use step::validate_string;

use crate::runtime::transaction::FlowIdentity as Identity;

/// C scratch, prompt and immediate-transition bounds, excluding string terminators.
const FIELDS: usize = 16;
const KEY_BYTES: usize = 31;
const VALUE_BYTES: usize = 8191;
const STEP_BYTES: usize = 63;
const TRANSITIONS: usize = 32;

#[derive(Default)]
struct Control {
    ready: bool,
    depth: usize,
    root: String,
}

/// Bindings share the world-thread flow engine without borrowing server sessions during callbacks.
#[derive(Clone)]
pub struct Engine {
    control: Rc<RefCell<Control>>,
    pub(crate) effects: Effects,
    modules: Rc<RefCell<BTreeMap<String, Table>>>,
    world: SharedWorld,
    live: bool,
    lazy: bool,
}

/// Preserve an error code for conversion to a structured error by the session facade.
fn error(_lua: &Lua, code: &str, message: impl Into<String>) -> mlua::Error {
    super::packages::error::failure(
        match code {
            "connection.invalid" => "mux.connection.invalid",
            "connection.unavailable" => "mux.connection.unavailable",
            "module.invalid" => "mux.module.invalid",
            "unavailable.checking" => "mux.unavailable.checking",
            _ => "mux.runtime",
        },
        message.into(),
    )
}

impl Engine {
    /// Install before game modules; initialization cannot start a live flow.
    pub(super) fn install(
        lua: &Lua,
        world: &SharedWorld,
        effects: &Effects,
        mode: RuntimeMode,
    ) -> Self {
        let engine = Self {
            control: Default::default(),
            effects: effects.clone(),
            modules: Default::default(),
            world: world.clone(),
            live: mode == RuntimeMode::Live,
            lazy: false,
        };
        lua.set_app_data(engine.clone());
        engine
    }

    /// Register loaded module tables; steps are resolved dynamically, including after reload.
    pub(super) fn register(&self, path: &str, module: &Table) -> mlua::Result<()> {
        match module.raw_get::<Value>("flows")? {
            Value::Nil => {}
            Value::Table(flows) => {
                for entry in flows.pairs::<Value, Value>() {
                    let (key, value) = entry?;
                    let Value::String(key) = key else {
                        return Err(mlua::Error::runtime(format!(
                            "{path}: flow names must be strings"
                        )));
                    };
                    let key = key.to_str()?;
                    validate_string(&key, STEP_BYTES, true)
                        .map_err(|e| mlua::Error::runtime(format!("{path}: flow {key:?}: {e}")))?;
                    if !matches!(value, Value::Function(_)) {
                        return Err(mlua::Error::runtime(format!(
                            "{path}: flow {key:?} must be a function"
                        )));
                    }
                }
            }
            _ => {
                return Err(mlua::Error::runtime(format!(
                    "{path}: flows must be a table"
                )));
            }
        }
        self.modules
            .borrow_mut()
            .insert(path.into(), module.clone());
        Ok(())
    }

    /// A server-hosted test VM shares session effects but loads its own callback functions.
    pub(crate) fn hosted(&self, host: &Self) -> Self {
        Self {
            effects: self.effects.hosted(&host.effects),
            live: true,
            lazy: true,
            ..self.clone()
        }
    }

    /// Prevent session effects while evaluating an initial module chunk.
    pub(crate) fn initializing<T>(&self, work: impl FnOnce() -> T) -> T {
        let before = std::mem::replace(&mut self.control.borrow_mut().ready, false);
        let result = work();
        self.control.borrow_mut().ready = before;
        result
    }

    pub(super) fn ready(&self) {
        self.control.borrow_mut().ready = true;
    }

    /// Shutdown discards interactive work without calling completion steps.
    pub fn stop(&self) {
        self.control.borrow_mut().ready = false;
        self.effects.stop();
    }

    /// Publish actual session identities, removing flows whose connections or incarnations disappeared.
    pub fn sessions(&self, sessions: BTreeMap<u64, Identity>) {
        self.effects.sessions(sessions);
    }

    /// Whether this session currently consumes input through a flow.
    pub fn active(&self, session: u64) -> bool {
        self.effects.active(session)
    }
    /// Abandon one flow without invoking script completion handlers.
    pub fn cancel(&self, session: u64) {
        self.effects.cancel(session);
    }
    fn output(&self, session: u64, text: &str, bold: bool) -> mlua::Result<()> {
        let document: Document = if bold {
            format!("[bold]{text}[reset]").into()
        } else {
            text.to_owned().into()
        };
        self.effects
            .output(session, document)
            .map_err(|error| mlua::Error::runtime(error.to_string()))
    }

    /// Validate and prime the target synchronously; nested Lua pcall cannot retain failed effects.
    pub fn start(&self, lua: &Lua, session: u64, module: &str, step: &str) -> mlua::Result<()> {
        if !self.live || !self.control.borrow().ready {
            let checking = lua
                .app_data_ref::<super::RuntimeMode>()
                .is_some_and(|mode| *mode == super::RuntimeMode::Checking);
            // C rejects checking after its luaL type checks with this exact message.
            return Err(error(
                lua,
                "unavailable.checking",
                if checking {
                    "mux.session.flow_start is unavailable during @lua/check"
                } else {
                    "mux.session.flow_start is unavailable during initialization, checking or isolated testing"
                },
            ));
        }
        transactions::require(lua)?;
        transactions::run(lua, &self.world, || {
            let identity = self
                .effects
                .session(session)
                .ok_or_else(|| error(lua, "connection.invalid", "no such descriptor"))?;
            if self.active(session) {
                return Err(error(
                    lua,
                    "connection.unavailable",
                    "descriptor already has an active flow",
                ));
            }
            let root = calling_root(lua).unwrap_or_else(|| self.control.borrow().root.clone());
            let module_name = module;
            let module = module_path(&root, module)
                .map_err(|e| error(lua, "module.invalid", e.to_string()))?;
            self.handler(lua, &module, module_name, step)?;
            self.effects.insert_flow(
                session,
                ActiveFlow {
                    identity,
                    module,
                    step: step.into(),
                    scratch: Default::default(),
                    prompt: String::new(),
                },
            );
            self.drive(lua, session, None)
        })
    }

    fn handler(
        &self,
        lua: &Lua,
        module: &str,
        name: &str,
        step: &str,
    ) -> mlua::Result<mlua::Function> {
        if self.lazy && !self.modules.borrow().contains_key(module) {
            let sources = lua
                .app_data_ref::<std::sync::Arc<super::sources::Sources>>()
                .expect("source snapshot installed")
                .clone();
            let source = sources.files.get(module).ok_or_else(|| {
                error(
                    lua,
                    "module.invalid",
                    format!("Lua file {name} is unavailable"),
                )
            })?;
            let result = self.initializing(|| lua.load(source).set_name(module).eval::<Table>());
            self.register(module, &result?)?;
        }
        let modules = self.modules.borrow();
        let Some(module_table) = modules.get(module) else {
            return Err(error(
                lua,
                "module.invalid",
                format!("Lua file {name} is unavailable"),
            ));
        };
        // C reports any missing flows table or step with one shared message.
        module_table
            .raw_get::<Table>("flows")
            .ok()
            .and_then(|flows| flows.raw_get::<mlua::Function>(step).ok())
            .ok_or_else(|| {
                error(
                    lua,
                    "module.invalid",
                    format!("{name} has no flow step '{step}'"),
                )
            })
    }

    /// A submitted line and all immediate transitions share one rollback boundary.
    pub fn input(&self, lua: &Lua, session: u64, input: &str) -> mlua::Result<()> {
        transactions::run(lua, &self.world, || self.drive(lua, session, Some(input)))
    }
}

fn module_path(root: &str, module: &str) -> mlua::Result<String> {
    let module = module
        .strip_suffix(".lua")
        .unwrap_or(module)
        .replace('.', "/");
    let path = format!("{module}.lua");
    super::sources::parent_path(&path).map_err(super::err)?;
    if !["global_logic", "object_logic"].contains(&root) {
        return Err(mlua::Error::runtime(
            "Flow requires a calling game-module root",
        ));
    }
    Ok(format!("{root}/{path}"))
}

/// Resolve the nearest game caller through built-in facades and package helper frames.
fn calling_root(lua: &Lua) -> Option<String> {
    for level in 0..64 {
        let source =
            lua.inspect_stack(level, |frame| frame.source().source.map(|s| s.into_owned()))?;
        if let Some(source) = source {
            let root = source
                .trim_start_matches(['@', '='])
                .split('/')
                .next()
                .unwrap_or("");
            if ["global_logic", "object_logic"].contains(&root) {
                return Some(root.into());
            }
        }
    }
    None
}

/// Establish a game root during dispatch; helper packages inherit the calling root.
pub(crate) fn with_root<T>(lua: &Lua, source: &str, work: impl FnOnce() -> T) -> T {
    let engine = lua.app_data_ref::<Engine>().map(|e| e.clone());
    let Some(engine) = engine else {
        return work();
    };
    let root = source
        .trim_start_matches(['@', '='])
        .split('/')
        .next()
        .unwrap_or("");
    if !["global_logic", "object_logic"].contains(&root) {
        return work();
    }
    let before = std::mem::replace(&mut engine.control.borrow_mut().root, root.into());
    let result = work();
    engine.control.borrow_mut().root = before;
    result
}

#[cfg(test)]
mod tests;
