//! Transaction-staged mux.log binding; no filesystem operations run in the Lua VM.
use crate::lua::{RuntimeMode, flows, transactions};
use mlua::{Lua, Table};
/// Register argument validation and bounded admission to the callback's pending effects.
pub(super) fn register(lua: &Lua, api: &Table) -> mlua::Result<()> {
    api.set(
        "log",
        lua.create_function(
            |lua, (filename, message): (mlua::LuaString, mlua::LuaString)| {
                if lua
                    .app_data_ref::<RuntimeMode>()
                    .is_some_and(|m| *m == RuntimeMode::Checking)
                {
                    return Err(mlua::Error::runtime(
                        "mux.unavailable.checking: log is unavailable while checking",
                    ));
                }
                transactions::require(lua)?;
                let filename = filename.as_bytes();
                let message = message.as_bytes();
                if filename.contains(&0) || message.contains(&0) {
                    return Err(mlua::Error::runtime(
                        "mux.arg.invalid: filename/message contains an embedded NUL byte",
                    ));
                }
                if message.is_empty() {
                    return Ok(true);
                }
                let Ok(filename) = std::str::from_utf8(&filename) else {
                    return Ok(false);
                };
                let message = std::str::from_utf8(&message).map_err(|_| {
                    mlua::Error::runtime("mux.arg.invalid: log message must be UTF-8")
                })?;
                let Ok(request) = crate::logging::FileRequest::new(filename, message) else {
                    return Ok(false);
                };
                let config = crate::lua::configuration(lua);
                let engine = lua.app_data_ref::<flows::Engine>().unwrap();
                engine.stage_log(request, &config)
            },
        )?,
    )
}
/// Install at the C top-level mux namespace, without a new require interface.
pub(super) fn install(lua: &Lua, api: &Table, mux: &Table) -> mlua::Result<()> {
    lua.load(include_str!("api.lua"))
        .set_name("@builtin/logging")
        .call((api, mux))
}
