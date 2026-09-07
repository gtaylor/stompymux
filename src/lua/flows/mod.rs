//! Descriptor-owned interactive flows, with VM-independent state and transactional private output.
mod step;
use super::{Outbox, RuntimeMode, SharedWorld, transactions};
use crate::{config::Config, state::Generation, text::Document, world::ObjectId};
use mlua::{Lua, Table, Value};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use step::validate_string;

/// C scratch, prompt and immediate-transition bounds, excluding string terminators.
const FIELDS: usize = 16;
const KEY_BYTES: usize = 31;
const VALUE_BYTES: usize = 8191;
const STEP_BYTES: usize = 63;
const TRANSITIONS: usize = 32;

/// Player incarnation associated with a currently authenticated connection.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Identity {
    pub player: ObjectId,
    pub generation: Generation,
}

/// Plain flow data survives replacement of the Lua VM.
#[derive(Clone)]
struct Active {
    pub identity: Identity,
    pub module: String,
    pub step: String,
    scratch: BTreeMap<Vec<u8>, Vec<u8>>,
    prompt: String,
}

/// Private output anchored between ordinary outbox messages, retaining script output order.
#[derive(Clone)]
pub struct PrivateOutput {
    pub after: usize,
    pub session: u64,
    pub document: Document,
}

/// Candidate state and output participate in both Lua and native rollback.
#[derive(Clone, Default)]
pub struct Snapshot {
    active: BTreeMap<u64, Active>,
    pub output: Vec<PrivateOutput>,
    /// Script file appends share every native and Lua rollback boundary.
    pub logs: Vec<crate::logging::FileRequest>,
    pub maintenance: Option<crate::dbck::DbCheckReport>,
}

/// Session identity is authoritative and is never restored by world rollback.
#[derive(Default)]
struct State {
    sessions: BTreeMap<u64, Identity>,
    durable: BTreeMap<u64, Active>,
    pending: Snapshot,
    ready: bool,
    depth: usize,
    root: String,
}

/// Bindings share the world-thread flow engine without borrowing server sessions during callbacks.
#[derive(Clone)]
pub struct Engine {
    state: Rc<RefCell<State>>,
    modules: Rc<RefCell<BTreeMap<String, Table>>>,
    world: SharedWorld,
    outbox: Outbox,
    config: Config,
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
        config: &Config,
        world: &SharedWorld,
        outbox: &Outbox,
        mode: RuntimeMode,
    ) -> Self {
        let engine = Self {
            state: Default::default(),
            modules: Default::default(),
            world: world.clone(),
            outbox: outbox.clone(),
            config: config.clone(),
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
            state: host.state.clone(),
            live: true,
            lazy: true,
            ..self.clone()
        }
    }

    /// Prevent session effects while evaluating an initial module chunk.
    pub(crate) fn initializing<T>(&self, work: impl FnOnce() -> T) -> T {
        let before = std::mem::replace(&mut self.state.borrow_mut().ready, false);
        let result = work();
        self.state.borrow_mut().ready = before;
        result
    }

    pub(super) fn ready(&self) {
        self.state.borrow_mut().ready = true;
    }

    /// Shutdown discards interactive work without calling completion steps.
    pub fn stop(&self) {
        let mut state = self.state.borrow_mut();
        state.ready = false;
        state.durable.clear();
        state.pending = Snapshot::default();
    }

    /// Publish actual session identities, removing flows whose connections or incarnations disappeared.
    pub fn sessions(&self, sessions: BTreeMap<u64, Identity>) {
        let mut state = self.state.borrow_mut();
        state.sessions = sessions;
        let sessions = state.sessions.clone();
        state
            .durable
            .retain(|id, f| sessions.get(id) == Some(&f.identity));
        state
            .pending
            .active
            .retain(|id, f| sessions.get(id) == Some(&f.identity));
        state
            .pending
            .output
            .retain(|o| sessions.contains_key(&o.session));
    }

    /// Capture candidate effects for nested rollback without capturing the session registry.
    pub fn snapshot(&self) -> Snapshot {
        self.state.borrow().pending.clone()
    }
    /// Restore candidate effects while excluding detached or replaced session identities.
    pub fn restore(&self, mut snapshot: Snapshot) {
        let mut state = self.state.borrow_mut();
        snapshot
            .active
            .retain(|id, active| state.sessions.get(id) == Some(&active.identity));
        snapshot
            .output
            .retain(|o| state.sessions.contains_key(&o.session));
        state.pending = snapshot;
    }
    /// Publish flow state only after durable world mutations succeed.
    pub fn commit(&self) {
        let mut s = self.state.borrow_mut();
        s.durable = s.pending.active.clone();
    }
    /// Restore the last committed step after a failed world transaction.
    pub fn rollback(&self) {
        let mut s = self.state.borrow_mut();
        s.pending = Snapshot {
            active: s.durable.clone(),
            output: Vec::new(),
            logs: Vec::new(),
            maintenance: None,
        };
    }
    /// Whether this session currently consumes input through a flow.
    pub fn active(&self, session: u64) -> bool {
        self.state.borrow().pending.active.contains_key(&session)
    }
    /// Abandon one flow without invoking script completion handlers.
    pub fn cancel(&self, session: u64) {
        let mut s = self.state.borrow_mut();
        s.pending.active.remove(&session);
        s.durable.remove(&session);
        s.pending.output.retain(|p| p.session != session);
    }
    /// Admit a bounded log request without touching the filesystem.
    pub(crate) fn stage_log(
        &self,
        request: crate::logging::FileRequest,
        config: &Config,
    ) -> mlua::Result<bool> {
        let mut state = self.state.borrow_mut();
        let pending = &mut state.pending.logs;
        let bytes = pending
            .iter()
            .map(|r| r.filename.len() + r.message.len())
            .sum::<usize>();
        if pending.len() >= config.lua.output_entry_limit
            || bytes.saturating_add(request.filename.len() + request.message.len())
                > config.lua.output_byte_limit
        {
            return Ok(false);
        }
        pending.push(request);
        Ok(true)
    }
    pub(crate) fn maintenance(&self) -> Option<crate::dbck::DbCheckReport> {
        self.state.borrow().pending.maintenance.clone()
    }
    pub(crate) fn stage_maintenance(&self, mut report: crate::dbck::DbCheckReport) {
        let mut state = self.state.borrow_mut();
        if let Some(old) = state.pending.maintenance.take() {
            report.findings.splice(0..0, old.findings);
            report.plan.purges.extend(old.plan.purges);
            report.plan.detachments.extend(old.plan.detachments);
        }
        // Repeated synchronous checks must not accumulate unbounded diagnostic strings.
        let mut bytes = 0usize;
        let mut entries = 0usize;
        report.findings.retain(|finding| {
            bytes = bytes.saturating_add(finding.len());
            entries += 1;
            entries <= self.config.lua.output_entry_limit
                && bytes <= self.config.lua.output_byte_limit
        });
        state.pending.maintenance = Some(report);
    }
    pub(crate) fn drain_maintenance(&self) -> Option<crate::dbck::DbCheckReport> {
        self.state.borrow_mut().pending.maintenance.take()
    }
    /// Consume appends only after their enclosing transaction commits.
    pub fn drain_logs(&self) -> Vec<crate::logging::FileRequest> {
        std::mem::take(&mut self.state.borrow_mut().pending.logs)
    }
    /// Consume staged private messages after their transaction commits.
    pub fn drain(&self) -> Vec<PrivateOutput> {
        std::mem::take(&mut self.state.borrow_mut().pending.output)
    }
    /// Carry committed plain data into a successfully published replacement VM.
    pub fn inherit(&self, previous: &Self) {
        let previous = previous.state.borrow();
        let mut s = self.state.borrow_mut();
        let logs = std::mem::take(&mut s.pending.logs);
        let maintenance = s.pending.maintenance.take();
        s.sessions = previous.sessions.clone();
        s.durable = previous.durable.clone();
        s.pending = Snapshot {
            active: s.durable.clone(),
            output: Vec::new(),
            logs,
            maintenance,
        };
    }

    /// Enforce one aggregate output allowance even when ordinary messages follow prompts.
    pub(crate) fn validate_output(&self) -> mlua::Result<()> {
        let normal = self.outbox.borrow();
        let state = self.state.borrow();
        let private = &state.pending.output;
        let bytes = normal
            .iter()
            .map(|(_, d)| d.len())
            .sum::<usize>()
            .saturating_add(private.iter().map(|o| o.document.len()).sum::<usize>())
            .saturating_add(
                state
                    .pending
                    .logs
                    .iter()
                    .map(|r| r.filename.len() + r.message.len())
                    .sum::<usize>(),
            );
        if normal
            .len()
            .saturating_add(private.len())
            .saturating_add(state.pending.logs.len())
            > self.config.lua.output_entry_limit
            || bytes > self.config.lua.output_byte_limit
        {
            return Err(mlua::Error::runtime("Lua output limit exceeded"));
        }
        Ok(())
    }

    fn output(&self, session: u64, text: &str, bold: bool) -> mlua::Result<()> {
        let document: Document = if bold {
            format!("[bold]{text}[reset]").into()
        } else {
            text.to_owned().into()
        };
        let normal = self.outbox.borrow();
        let mut s = self.state.borrow_mut();
        let bytes = normal.iter().map(|(_, d)| d.len()).sum::<usize>()
            + s.pending
                .output
                .iter()
                .map(|p| p.document.len())
                .sum::<usize>();
        if document.len() > self.config.runtime.output_message_limit
            || normal.len() + s.pending.output.len() >= self.config.lua.output_entry_limit
            || bytes.saturating_add(document.len()) > self.config.lua.output_byte_limit
        {
            return Err(mlua::Error::runtime("Flow output limit exceeded"));
        }
        s.pending.output.push(PrivateOutput {
            after: normal.len(),
            session,
            document,
        });
        Ok(())
    }

    /// Validate and prime the target synchronously; nested Lua pcall cannot retain failed effects.
    pub fn start(&self, lua: &Lua, session: u64, module: &str, step: &str) -> mlua::Result<()> {
        if !self.live || !self.state.borrow().ready {
            return Err(error(
                lua,
                "unavailable.checking",
                "mux.session.flow_start is unavailable during initialization, checking or isolated testing",
            ));
        }
        transactions::require(lua)?;
        transactions::run(lua, &self.world, &self.outbox, || {
            let identity = *self.state.borrow().sessions.get(&session).ok_or_else(|| {
                error(
                    lua,
                    "connection.invalid",
                    "no such authenticated descriptor",
                )
            })?;
            if self.active(session) {
                return Err(error(
                    lua,
                    "connection.unavailable",
                    "descriptor already has an active flow",
                ));
            }
            validate_string(step, STEP_BYTES, true)
                .map_err(|e| error(lua, "module.invalid", e.to_string()))?;
            let root = calling_root(lua).unwrap_or_else(|| self.state.borrow().root.clone());
            let module = module_path(&root, module)
                .map_err(|e| error(lua, "module.invalid", e.to_string()))?;
            self.handler(lua, &module, step)?;
            self.state.borrow_mut().pending.active.insert(
                session,
                Active {
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

    fn handler(&self, lua: &Lua, module: &str, step: &str) -> mlua::Result<mlua::Function> {
        if self.lazy && !self.modules.borrow().contains_key(module) {
            let sources = lua
                .app_data_ref::<std::sync::Arc<super::sources::Sources>>()
                .expect("source snapshot installed")
                .clone();
            let source = sources.files.get(module).ok_or_else(|| {
                error(
                    lua,
                    "module.invalid",
                    format!("Unknown flow module {module}"),
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
                format!("Unknown flow module {module}"),
            ));
        };
        module_table
            .raw_get::<Table>("flows")
            .and_then(|t| t.raw_get(step))
            .map_err(|e| {
                error(
                    lua,
                    "module.invalid",
                    format!("{module}: flow {step:?}: {e}"),
                )
            })
    }

    /// A submitted line and all immediate transitions share one rollback boundary.
    pub fn input(&self, lua: &Lua, session: u64, input: &str) -> mlua::Result<()> {
        transactions::run(lua, &self.world, &self.outbox, || {
            self.drive(lua, session, Some(input))
        })
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
    let before = std::mem::replace(&mut engine.state.borrow_mut().root, root.into());
    let result = work();
    engine.state.borrow_mut().root = before;
    result
}

/// Native and Lua transaction helpers can capture effects without depending on the server.
pub(crate) fn snapshot(lua: &Lua) -> Option<Snapshot> {
    lua.app_data_ref::<Engine>().map(|e| e.snapshot())
}
pub(crate) fn restore(lua: &Lua, snapshot: Option<Snapshot>) {
    if let (Some(engine), Some(snapshot)) = (lua.app_data_ref::<Engine>(), snapshot) {
        engine.restore(snapshot);
    }
}

#[cfg(test)]
mod tests;
