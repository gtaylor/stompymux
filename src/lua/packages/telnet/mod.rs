//! Binary-safe NEW-ENVIRON lookup against live connection snapshots.
use super::error::{check_integer_arg, check_string_arg, failure, wrap};
use mlua::{Lua, MultiValue, Table, Value};

pub(super) fn install(lua: &Lua, mux: &Table) -> mlua::Result<()> {
    let package = lua.create_table()?;
    for (name, has) in [("environment_has", true), ("environment_get", false)] {
        let f = lua.create_function(move |lua, values: MultiValue| {
            // C validates the descriptor integer, then rejects checking mode,
            // then resolves the descriptor (mux_telnet_bindings.c require_descriptor).
            let descriptor = check_integer_arg(lua, &values, 1)? as i32 as i64;
            if lua
                .app_data_ref::<crate::lua::RuntimeMode>()
                .is_some_and(|m| *m == crate::lua::RuntimeMode::Checking)
            {
                return Err(failure(
                    "mux.unavailable.checking",
                    "mux.telnet.environment is unavailable during @lua/check",
                ));
            }
            let snapshot = lua.app_data_ref::<crate::lua::sessions::Sessions>();
            let env = snapshot
                .as_ref()
                .and_then(|s| s.environments.get(&(descriptor as u64)))
                .ok_or_else(|| {
                    super::error::argument_failure(
                        lua,
                        1,
                        "mux.connection.invalid",
                        "no such descriptor",
                    )
                })?;
            let kind_bytes = check_string_arg(lua, &values, 2)?;
            // C compares with strcmp, so an embedded NUL truncates the kind.
            let kind_end = kind_bytes
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(kind_bytes.len());
            let kind = match &kind_bytes[..kind_end] {
                b"var" => crate::telnet::environment::Kind::Var,
                b"uservar" => crate::telnet::environment::Kind::UserVar,
                _ => {
                    return Err(super::error::argument_failure(
                        lua,
                        2,
                        "mux.connection.invalid",
                        "kind must be 'var' or 'uservar'",
                    ));
                }
            };
            let name = check_string_arg(lua, &values, 3)?;
            let value = env.0.get(&(kind, name));
            if has {
                Ok(Value::Boolean(value.is_some()))
            } else {
                value
                    .map(|v| lua.create_string(v).map(Value::String))
                    .transpose()
                    .map(|v| v.unwrap_or(Value::Nil))
            }
        })?;
        package.set(name, wrap(lua, f, "mux.connection.invalid")?)?;
    }
    mux.set("telnet", package)
}
