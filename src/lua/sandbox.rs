//! LuaJIT budgets, game-only module search paths and operating-system access restrictions.
use super::err;
use crate::config::Config;
use anyhow::Result;
use mlua::{HookTriggers, Lua, LuaOptions, StdLib, Table, Value, VmState};
use std::{cell::Cell, rc::Rc};

/// One shared instruction counter, replenished at existing callback and module boundaries.
#[derive(Clone)]
pub(super) struct InstructionBudget {
    remaining: Rc<Cell<usize>>,
    limit: usize,
}

impl InstructionBudget {
    /// Restore the configured allowance without replacing the counter captured by the VM hook.
    pub(super) fn reset(&self) {
        self.remaining.set(self.limit);
    }
}

/// Create the VM and install resource limits before any built-in facade runs.
pub(super) fn create(config: &Config) -> Result<(Lua, InstructionBudget)> {
    // Retain LuaJIT's native debug.traceback closure for mux.error before the
    // sandbox removes the debug table. No game source executes in this window.
    let lua =
        unsafe { Lua::unsafe_new_with(StdLib::ALL_SAFE | StdLib::DEBUG, LuaOptions::default()) };
    lua.set_memory_limit(config.lua.memory_limit)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    // JIT traces can bypass instruction hooks. Keep the LuaJIT runtime, with tracing off,
    // so an operator-supplied script cannot monopolize the world owner indefinitely.
    lua.load("if jit then jit.off() end")
        .exec()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let budget = InstructionBudget {
        remaining: Rc::new(Cell::new(config.lua.instruction_limit)),
        limit: config.lua.instruction_limit,
    };
    let b = budget.remaining.clone();
    // Sampling granularity is an implementation detail; the budget is configured.
    const HOOK_QUANTUM: usize = 1000;
    let quantum = config.lua.instruction_limit.min(HOOK_QUANTUM);
    lua.set_global_hook(
        HookTriggers::new().every_nth_instruction(quantum as u32),
        move |_, _| {
            let n = b.get();
            if n <= quantum {
                return Err(err("Lua instruction budget exceeded"));
            }
            b.set(n - quantum);
            Ok(VmState::Continue)
        },
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok((lua, budget))
}

/// Keep require resolution restricted to the configured editable game packages.
pub(super) fn configure_search(lua: &Lua, _sources: &super::sources::Sources) -> Result<()> {
    let install = || -> mlua::Result<()> {
        let package: Table = lua.globals().get("package")?;
        // Retain the preload loader only; require must never consult mutable disk files.
        let loaders: Table = package.get("loaders")?;
        let only = lua.create_table()?;
        only.set(1, loaders.get::<Value>(1)?)?;
        package.set("loaders", only)?;
        package.set("path", "")?;
        package.set("cpath", "")?;
        Ok(())
    };
    install().map_err(|e| anyhow::anyhow!("{e}"))
}

#[derive(Clone)]
struct ModuleRoot(Rc<Cell<u8>>);

fn root_name(code: u8) -> &'static str {
    match code {
        0 => "object_logic",
        1 => "global_logic",
        2 => "packages",
        3 => "tests",
        _ => "packages",
    }
}

fn root_code(root: &str) -> u8 {
    match root {
        "object_logic" => 0,
        "global_logic" => 1,
        "packages" => 2,
        "tests" => 3,
        _ => 2,
    }
}

/// C reads only the immediate Lua caller's mutable environment field. C callers
/// (including pcall) have no environment and therefore use the active load root.
fn caller_root(lua: &Lua) -> String {
    let fallback = lua.app_data_ref::<ModuleRoot>().map_or(2, |v| v.0.get());
    let code = lua
        .inspect_stack(1, |frame| frame.function().environment())
        .flatten()
        .and_then(|environment| environment.raw_get::<Value>("__mux_module_root").ok())
        .and_then(|value| lua.coerce_integer(value).ok().flatten())
        .and_then(|value| u8::try_from(value).ok())
        .filter(|value| *value < 4)
        .unwrap_or(fallback);
    root_name(code).to_owned()
}

/// Install the private source-snapshot loader after the standard package tables are hidden.
fn install_require(lua: &Lua) -> mlua::Result<()> {
    let cache = lua.create_table()?;
    lua.set_app_data(ModuleRoot(Rc::new(Cell::new(2))));
    lua.set_named_registry_value("mux.modules", cache.clone())?;
    let loading = lua.create_table()?;
    lua.set_named_registry_value("mux.modules.loading", loading.clone())?;
    let loader = lua.create_function(move |lua, value: Value| {
        let name =
            lua.coerce_string(value)?
                .ok_or_else(|| mlua::Error::FromLuaConversionError {
                    from: "value",
                    to: "string".into(),
                    message: Some("module name must be a string".into()),
                })?;
        // The reference uses luaL_checkstring followed by strlen, so an embedded NUL
        // terminates the module name before validation and lookup.
        let bytes = name.as_bytes();
        let bytes = &bytes[..bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len())];
        let name = std::str::from_utf8(bytes).map_err(|_| {
            super::packages::error::failure("mux.module.invalid", "invalid module name")
        })?;
        if name.is_empty()
            || name.starts_with('.')
            || name.ends_with('.')
            || !name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_' || byte == b'.')
        {
            return Err(super::packages::error::failure(
                "mux.module.invalid",
                "invalid module name",
            ));
        }
        if name.len() + 4 >= 4096 {
            return Err(super::packages::error::failure(
                "mux.module.invalid",
                "module name is too long",
            ));
        }
        if name == "mux" || name == "btech" {
            return lua.globals().get::<Table>(name);
        }
        let relative = format!("{}.lua", name.replace('.', "/"));
        let sources = lua
            .app_data_ref::<std::sync::Arc<super::sources::Sources>>()
            .expect("source snapshot installed before require");
        let mut candidates = Vec::with_capacity(2);
        let root = caller_root(lua);
        candidates.push(format!("{root}/{relative}"));
        if !candidates
            .iter()
            .any(|path| path == &format!("packages/{relative}"))
        {
            candidates.push(format!("packages/{relative}"));
        }
        let Some(path) = candidates
            .into_iter()
            .find(|path| sources.files.contains_key(path))
        else {
            return Err(super::packages::error::failure(
                "mux.module.unavailable",
                format!("Lua module {name} is unavailable"),
            ));
        };
        if let Some(module) = cache.get::<Option<Table>>(path.as_str())? {
            return Ok(module);
        }
        if loading.get::<Option<bool>>(path.as_str())?.unwrap_or(false) {
            return Err(super::packages::error::failure(
                "mux.module.unavailable",
                format!("recursive load of Lua module {name}"),
            ));
        }
        let source = sources.files[&path].clone();
        drop(sources);
        loading.set(path.as_str(), true)?;
        let function = lua.load(&source).set_name(&path).into_function()?;
        let root = root_code(path.split('/').next().unwrap_or("packages"));
        function.set_environment(module_environment(lua, root_name(root))?)?;
        let state = lua.app_data_ref::<ModuleRoot>().unwrap().clone();
        let previous = state.0.replace(root);
        let result = function.call::<Table>(()).map_err(|error| {
            super::packages::error::failure("mux.module.unavailable", error.to_string())
        });
        state.0.set(previous);
        loading.set(path.as_str(), Value::Nil)?;
        let module = result?;
        cache.set(path, module.clone())?;
        Ok(module)
    })?;
    lua.globals().set("require", loader)
}

/// Give every game module C's isolated write scope while inheriting safe globals.
pub(super) fn module_environment(lua: &Lua, root: &str) -> mlua::Result<Table> {
    let environment = lua.create_table()?;
    environment.raw_set("__mux_module_root", root_code(root))?;
    let metatable = lua.create_table()?;
    metatable.raw_set("__index", lua.globals())?;
    environment.set_metatable(Some(metatable))?;
    Ok(environment)
}

pub(super) fn with_module_root<T>(lua: &Lua, root: &str, work: impl FnOnce() -> T) -> T {
    let state = lua.app_data_ref::<ModuleRoot>().map(|state| state.clone());
    let Some(state) = state else { return work() };
    let previous = state.0.replace(root_code(root));
    let result = work();
    state.0.set(previous);
    result
}

/// Seal runtime access after installing built-ins and before evaluating game modules.
pub(super) fn restrict(lua: &Lua) -> Result<()> {
    lua.load(include_str!("sandbox.lua"))
        .exec()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    // Scripts are game extensions, not operating-system extensions.
    for name in [
        "io",
        "os",
        "debug",
        "package",
        "coroutine",
        "ffi",
        "jit",
        "dofile",
        "loadfile",
        "loadstring",
        "load",
        "collectgarbage",
        "module",
        "getfenv",
        "setfenv",
        "require",
    ] {
        lua.globals()
            .set(name, Value::Nil)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    install_require(lua).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(())
}
