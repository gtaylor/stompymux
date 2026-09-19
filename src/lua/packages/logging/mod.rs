//! Transaction-staged mux.log binding; no filesystem writes run in the Lua VM.
use crate::lua::{RuntimeMode, transactions};
use mlua::{Lua, Table};

/// Mirror the synchronous admission checks of C log_to_file (server/log.c).
/// C accepts a request only when the file already exists under `logs/` and is
/// readable and writable by the process; everything else returns false.
fn permitted(lua: &Lua, filename: &str, message: &[u8]) -> bool {
    // C returns true for an empty message before validating the filename.
    if message.is_empty() {
        return true;
    }
    if filename.is_empty()
        || filename.len() > 200
        || filename.contains("..")
        || filename.contains('/')
    {
        return false;
    }
    let path = crate::lua::configuration(lua).path(format!("logs/{filename}"));
    let Ok(metadata) = std::fs::metadata(&path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = metadata.permissions().mode();
        mode & 0o444 != 0 && mode & 0o222 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Register argument validation and bounded admission to the callback's pending effects.
pub(super) fn register(lua: &Lua, api: &Table) -> mlua::Result<()> {
    api.set(
        "log",
        lua.create_function(move |lua, values: mlua::MultiValue| {
            let filename = super::error::check_string_arg(lua, &values, 1)?;
            let message = super::error::check_string_arg(lua, &values, 2)?;
            if lua
                .app_data_ref::<RuntimeMode>()
                .is_some_and(|m| *m == RuntimeMode::Checking)
            {
                return Err(super::error::failure(
                    "mux.unavailable.checking",
                    "mux.log is unavailable during @lua/check",
                ));
            }
            transactions::require(lua)?;
            if filename.contains(&0) {
                return Err(super::error::argument_failure(
                    lua,
                    1,
                    "mux.arg.invalid",
                    "filename contains an embedded NUL byte",
                ));
            }
            if message.contains(&0) {
                return Err(super::error::argument_failure(
                    lua,
                    2,
                    "mux.arg.invalid",
                    "message contains an embedded NUL byte",
                ));
            }
            let Ok(filename) = std::str::from_utf8(&filename) else {
                return Ok(false);
            };
            // C returns true for an empty message before touching the log.
            if message.is_empty() {
                return Ok(true);
            }
            if !permitted(lua, filename, &message) {
                return Ok(false);
            }
            let request = crate::logging::FileRequest::new_bytes(filename, &message)
                .map_err(mlua::Error::external)?;
            let config = crate::lua::configuration(lua);
            let effects = lua.app_data_ref::<crate::runtime::Effects>().unwrap();
            Ok(effects.stage_log(request, &config))
        })?,
    )
}
/// Install at the C top-level mux namespace, without a new require interface.
pub(super) fn install(lua: &Lua, api: &Table, mux: &Table) -> mlua::Result<()> {
    lua.load(include_str!("api.lua"))
        .set_name("@builtin/logging")
        .call((api, mux))
}
