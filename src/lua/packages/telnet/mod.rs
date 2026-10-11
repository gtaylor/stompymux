//! Binary-safe NEW-ENVIRON lookup and MXP client reports against live connection snapshots.
use super::error::{check_integer_arg, check_string_arg, failure, wrap};
use mlua::{Lua, MultiValue, Table, Value};

/// Resolve a live descriptor argument the way the NEW-ENVIRON lookups do.
fn descriptor_arg(lua: &Lua, values: &MultiValue) -> mlua::Result<u64> {
    let descriptor = check_integer_arg(lua, values, 1)? as i32 as i64;
    if lua
        .app_data_ref::<crate::lua::RuntimeMode>()
        .is_some_and(|m| *m == crate::lua::RuntimeMode::Checking)
    {
        return Err(failure(
            "mux.unavailable.checking",
            "mux.telnet is unavailable during @lua/check",
        ));
    }
    let known = lua
        .app_data_ref::<crate::lua::sessions::Sessions>()
        .is_some_and(|s| s.environments.contains_key(&(descriptor as u64)));
    if !known {
        return Err(super::error::argument_failure(
            lua,
            1,
            "mux.connection.invalid",
            "no such descriptor",
        ));
    }
    Ok(descriptor as u64)
}

/// Install `mxp_client` and `mxp_supports`, which read the negotiated MXP report.
fn install_mxp(lua: &Lua, package: &Table) -> mlua::Result<()> {
    let client = lua.create_function(|lua, values: MultiValue| {
        let descriptor = descriptor_arg(lua, &values)?;
        let snapshot = lua.app_data_ref::<crate::lua::sessions::Sessions>();
        let Some(client) = snapshot.as_ref().and_then(|s| s.mxp.get(&descriptor)) else {
            return Ok(Value::Nil);
        };
        let table = lua.create_table()?;
        table.set("name", lua.create_string(&client.name)?)?;
        table.set("version", lua.create_string(&client.version)?)?;
        table.set("mxp_version", lua.create_string(&client.mxp_version)?)?;
        if let Some(registered) = client.registered {
            table.set("registered", registered)?;
        }
        let supports = lua.create_table()?;
        for (index, tag) in client.supports.iter().enumerate() {
            supports.set(index + 1, lua.create_string(tag)?)?;
        }
        table.set("supports", supports)?;
        Ok(Value::Table(table))
    })?;
    package.set("mxp_client", wrap(lua, client, "mux.connection.invalid")?)?;
    let supports = lua.create_function(|lua, values: MultiValue| {
        let descriptor = descriptor_arg(lua, &values)?;
        let tag = check_string_arg(lua, &values, 2)?;
        let tag = std::str::from_utf8(&tag).map_err(|_| {
            super::error::argument_failure(
                lua,
                2,
                "mux.connection.invalid",
                "tag must be valid UTF-8",
            )
        })?;
        let snapshot = lua.app_data_ref::<crate::lua::sessions::Sessions>();
        Ok(snapshot
            .as_ref()
            .and_then(|s| s.mxp.get(&descriptor))
            .is_some_and(|client| client.supports(tag)))
    })?;
    package.set(
        "mxp_supports",
        wrap(lua, supports, "mux.connection.invalid")?,
    )
}

pub(super) fn install(lua: &Lua, mux: &Table) -> mlua::Result<()> {
    let package = lua.create_table()?;
    install_mxp(lua, &package)?;
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
