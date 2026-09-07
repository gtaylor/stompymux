//! Embedded session facade over runtime-published connection snapshots.
use mlua::{Function, Lua, Table};

/// Register session snapshots and the transaction-aware flow engine.
pub(super) fn install(lua: &Lua, api: &Table, mux: &Table, id: &Function) -> mlua::Result<()> {
    api.set(
        "flow_start",
        lua.create_function(|lua, (descriptor, module, step): (u64, String, String)| {
            let engine = lua
                .app_data_ref::<crate::lua::flows::Engine>()
                .expect("flow engine installed")
                .clone();
            engine.start(lua, descriptor, &module, &step)
        })?,
    )?;
    lua.load(include_str!("api.lua"))
        .set_name("@builtin/session")
        .call((api, mux, id))
}
