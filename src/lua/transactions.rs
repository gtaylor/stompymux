//! Callback boundaries keep state availability and rollback independent of Lua pcall.
use crate::runtime::SharedWorld;
use mlua::Lua;
use std::{cell::Cell, rc::Rc};

/// Install a private nesting counter before any game module executes.
pub fn install(lua: &Lua) {
    lua.set_app_data(Rc::new(Cell::new(0usize)));
}

/// Whether a host invocation is nested inside the current Lua transaction.
pub(crate) fn active(lua: &Lua) -> bool {
    lua.app_data_ref::<Rc<Cell<usize>>>()
        .is_some_and(|depth| depth.get() > 0)
}

/// State enumeration and mutations require an active game callback.
pub fn require(lua: &Lua) -> mlua::Result<()> {
    if lua
        .app_data_ref::<super::RuntimeMode>()
        .is_some_and(|m| *m == super::RuntimeMode::Checking)
    {
        return Err(super::packages::error::failure(
            "mux.unavailable.checking",
            "live API is unavailable while checking",
        ));
    }

    if lua
        .app_data_ref::<Rc<Cell<usize>>>()
        .is_some_and(|depth| depth.get() > 0)
    {
        Ok(())
    } else {
        Err(super::packages::error::failure(
            "mux.state.unavailable",
            "state unavailable outside a callback transaction",
        ))
    }
}

/// Restore world and staged output on a protected callback error, including nested callbacks.
pub fn run<T>(
    lua: &Lua,
    world: &SharedWorld,
    work: impl FnOnce() -> mlua::Result<T>,
) -> mlua::Result<T> {
    let depth = lua
        .app_data_ref::<Rc<Cell<usize>>>()
        .expect("callback counter installed")
        .clone();
    let before = world.borrow().clone();
    let effects = lua
        .app_data_ref::<crate::runtime::Effects>()
        .expect("transaction effects installed")
        .clone();
    let checkpoint = effects.checkpoint();
    depth.set(depth.get() + 1);
    let result = work().and_then(|value| {
        effects
            .validate()
            .map_err(|error| mlua::Error::runtime(error.to_string()))?;
        Ok(value)
    });
    depth.set(depth.get() - 1);
    if result.is_err() {
        *world.borrow_mut() = before;
        effects.restore(checkpoint);
    }
    result
}

/// Descriptor dynamically scoped to the current callback, never stored in game state.
#[derive(Clone, Copy)]
pub struct Descriptor(pub Option<u64>);
pub fn descriptor(lua: &Lua) -> Option<u64> {
    lua.app_data_ref::<Descriptor>().and_then(|d| d.0)
}
pub fn with_descriptor<T>(lua: &Lua, value: Option<u64>, work: impl FnOnce() -> T) -> T {
    let before = descriptor(lua);
    lua.set_app_data(Descriptor(value));
    let result = work();
    lua.set_app_data(Descriptor(before));
    result
}

/// Test invocations retain valid mutations even when Lua cannot construct an error report.
/// The caller must validate and persist, rolling back only an invalid or unsaved result.
pub(super) fn live_test<T>(lua: &Lua, work: impl FnOnce() -> mlua::Result<T>) -> mlua::Result<T> {
    let depth = lua
        .app_data_ref::<Rc<Cell<usize>>>()
        .expect("callback counter installed")
        .clone();
    depth.set(depth.get() + 1);
    let result = work();
    depth.set(depth.get() - 1);
    result
}

/// Causal actor dynamically scoped to a native or Lua command invocation.
#[derive(Clone, Copy)]
struct Cause(Option<crate::world::ObjectId>);

/// Read the causal actor without changing execution authority.
pub fn cause(lua: &Lua) -> Option<crate::world::ObjectId> {
    lua.app_data_ref::<Cause>().and_then(|c| c.0)
}

/// Restore causal context after nested command/callback execution.
pub fn with_cause<T>(lua: &Lua, value: crate::world::ObjectId, work: impl FnOnce() -> T) -> T {
    let before = cause(lua);
    lua.set_app_data(Cause(Some(value)));
    let result = work();
    lua.set_app_data(Cause(before));
    result
}
