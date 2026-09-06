//! Callback boundaries keep state availability and rollback independent of Lua pcall.
use super::{Outbox, SharedWorld};
use mlua::Lua;
use std::{cell::Cell, rc::Rc};

/// Install a private nesting counter before any game module executes.
pub fn install(lua: &Lua) {
    lua.set_app_data(Rc::new(Cell::new(0usize)));
}

/// State enumeration and mutations require an active game callback.
pub fn require(lua: &Lua) -> mlua::Result<()> {
    if lua
        .app_data_ref::<Rc<Cell<usize>>>()
        .is_some_and(|depth| depth.get() > 0)
    {
        Ok(())
    } else {
        Err(super::err(
            "state unavailable outside a callback transaction",
        ))
    }
}

/// Restore world and staged output on a protected callback error, including nested callbacks.
pub fn run<T>(
    lua: &Lua,
    world: &SharedWorld,
    outbox: &Outbox,
    work: impl FnOnce() -> mlua::Result<T>,
) -> mlua::Result<T> {
    let depth = lua
        .app_data_ref::<Rc<Cell<usize>>>()
        .expect("callback counter installed")
        .clone();
    let before = world.borrow().clone();
    let pending = outbox.borrow().clone();
    depth.set(depth.get() + 1);
    let result = work();
    depth.set(depth.get() - 1);
    if result.is_err() {
        *world.borrow_mut() = before;
        *outbox.borrow_mut() = pending;
    }
    result
}
