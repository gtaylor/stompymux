//! Native bindings for the existing mux.config package.
use super::bind;
use crate::config::Config;
use anyhow::Result;
use mlua::{Lua, LuaSerdeExt, Table};

/// Register native operations before the embedded facade is evaluated.
pub(super) fn register(lua: &Lua, api: &Table, _config: &Config) -> Result<()> {
    bind!(lua, api, "config", move |lua, key: String| {
        if key.contains('\0') {
            return Err(super::error::failure(
                "mux.arg.invalid",
                "configuration name contains NUL",
            ));
        }
        let c = crate::lua::configuration(lua);
        if let Some(v) = c.effective_value(&key) {
            lua.to_value(v)
        } else {
            Err(super::error::failure(
                "mux.config.not_found",
                format!("unknown configuration key {key}"),
            ))
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
