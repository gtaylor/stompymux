//! Native bindings for the existing mux.config package.
use super::bind;
use crate::config::Config;
use anyhow::Result;
use mlua::{Lua, LuaSerdeExt, Table};

/// Register native operations before the embedded facade is evaluated.
pub(super) fn register(lua: &Lua, api: &Table, _config: &Config) -> Result<()> {
    bind!(lua, api, "config", move |lua, values: mlua::MultiValue| {
        let key = match values.front() {
            Some(mlua::Value::String(key)) => key.as_bytes().to_vec(),
            _ => {
                return Err(super::error::argument_failure(
                    lua,
                    1,
                    "mux.arg.invalid",
                    "configuration name must be a string",
                ));
            }
        };
        if key.contains(&0) {
            return Err(super::error::argument_failure(
                lua,
                1,
                "mux.arg.invalid",
                "configuration name contains an embedded NUL byte",
            ));
        }
        let c = crate::lua::configuration(lua);
        // Only names in the legacy catalog are known directives; other spellings
        // (including TOML section paths like 'mux') are unknown in C.
        let known = crate::config::catalog::KEYS
            .iter()
            .any(|spec| spec.legacy.as_bytes() == key.as_slice());
        let Some(value) = c.effective_value(&String::from_utf8_lossy(&key)) else {
            return Err(not_found(&key));
        };
        if !known {
            return Err(not_found(&key));
        }
        match value {
            toml::Value::String(_)
            | toml::Value::Integer(_)
            | toml::Value::Float(_)
            | toml::Value::Boolean(_) => lua.to_value(value),
            _ => {
                let mut message = b"configuration directive '".to_vec();
                message.extend_from_slice(&key);
                message.extend_from_slice(b"' is not a readable scalar");
                Err(super::error::failure_bytes_pub(
                    "mux.config.unsupported",
                    message,
                ))
            }
        }
    });
    Ok(())
}

/// The C not-found diagnostic carries the raw requested name bytes.
fn not_found(key: &[u8]) -> mlua::Error {
    let mut message = b"unknown configuration directive '".to_vec();
    message.extend_from_slice(key);
    message.push(b'\'');
    super::error::failure_bytes_pub("mux.config.not_found", message)
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
