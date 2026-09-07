//! Embedded session facade over runtime-published connection snapshots.
use mlua::{Function, Lua, Table};

/// Register session snapshots and the transaction-aware flow engine.
pub(super) fn install(lua: &Lua, api: &Table, mux: &Table, id: &Function) -> mlua::Result<()> {
    api.set(
        "connected_players",
        lua.create_function(|lua, ()| crate::lua::sessions::players(lua))?,
    )?;
    api.set(
        "who_summary",
        lua.create_function(|lua, ()| {
            let t = lua.create_table()?;
            let s = lua.app_data_ref::<crate::lua::sessions::Sessions>();
            t.set("hidden", s.as_ref().map_or(0, |s| s.hidden))?;
            t.set("record", s.as_ref().map_or(0, |s| s.record))?;
            if let Some(maximum) = s.as_ref().and_then(|s| s.maximum) {
                t.set("maximum", maximum)?;
            }
            Ok(t)
        })?,
    )?;
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
    let f = api.get("flow_start")?;
    api.set("flow_start", super::error::wrap(lua, f, "mux.runtime")?)?;
    lua.load(include_str!("api.lua"))
        .set_name("@builtin/session")
        .call((api, mux, id))
}
