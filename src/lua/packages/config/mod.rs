//! Native bindings for the existing mux.config package.
use super::bind;
use crate::config::Config;
use anyhow::Result;
use mlua::{Lua, LuaSerdeExt, Table, Value};

/// Register native operations before the embedded facade is evaluated.
pub(super) fn register(lua: &Lua, api: &Table, config: &Config) -> Result<()> {
    let c = config.clone();
    bind!(lua, api, "config", move |lua, key: String| {
        if let Some(v) = c.effective_value(&key) {
            lua.to_value(v)
        } else {
            Ok(Value::Nil)
        }
    });
    Ok(())
}

/// Install the embedded Lua facade with explicit shared table and identity dependencies.
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    mux: &Table,
    id: &mlua::Function,
) -> mlua::Result<()> {
    lua.load(include_str!("api.lua"))
        .set_name("@builtin/config")
        .call((api, mux, id))
}
