//! LuaJIT budgets, game-only module search paths and operating-system access restrictions.
use super::err;
use crate::config::Config;
use anyhow::Result;
use mlua::{HookTriggers, Lua, Table, Value, VmState};
use std::{cell::Cell, rc::Rc};

/// One shared instruction counter, replenished at existing callback and module boundaries.
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
    let lua = Lua::new();
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
pub(super) fn configure_search(lua: &Lua, config: &Config) -> Result<()> {
    let packages = config.lua_dir().join("packages").canonicalize()?;
    let package: Table = lua
        .globals()
        .get("package")
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    package
        .set("path", format!("{}/?.lua", packages.display()))
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    package
        .set("cpath", "")
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    Ok(())
}

/// Seal runtime access after installing built-ins and before evaluating game modules.
pub(super) fn restrict(lua: &Lua) -> Result<()> {
    lua.load(include_str!("sandbox.lua"))
        .exec()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    // Scripts are game extensions, not operating-system extensions.
    for name in ["io", "os", "debug", "ffi", "jit", "dofile", "loadfile"] {
        lua.globals()
            .set(name, Value::Nil)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    }
    Ok(())
}
