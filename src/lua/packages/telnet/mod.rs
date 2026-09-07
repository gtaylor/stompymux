//! Binary-safe NEW-ENVIRON lookup against live connection snapshots.
use super::error::{failure, wrap};
use mlua::{Lua, Table, Value};
pub(super) fn install(lua: &Lua, mux: &Table) -> mlua::Result<()> {
    let package = lua.create_table()?;
    for (name, has) in [("environment_has", true), ("environment_get", false)] {
        let f = lua.create_function(
            move |lua, (descriptor, kind, name): (u64, String, mlua::LuaString)| {
                if lua
                    .app_data_ref::<crate::lua::RuntimeMode>()
                    .is_some_and(|m| *m == crate::lua::RuntimeMode::Checking)
                {
                    return Err(failure(
                        "mux.unavailable.checking",
                        "telnet is unavailable while checking",
                    ));
                }
                let kind = match kind.as_str() {
                    "var" => crate::telnet::environment::Kind::Var,
                    "uservar" => crate::telnet::environment::Kind::UserVar,
                    _ => {
                        return Err(failure(
                            "mux.connection.invalid",
                            "kind must be 'var' or 'uservar'",
                        ));
                    }
                };
                let snapshot = lua.app_data_ref::<crate::lua::sessions::Sessions>();
                let env = snapshot
                    .as_ref()
                    .and_then(|s| s.environments.get(&descriptor))
                    .ok_or_else(|| {
                        failure(
                            "mux.connection.invalid",
                            "descriptor is not a live connection",
                        )
                    })?;
                let value = env.0.get(&(kind, name.as_bytes().to_vec()));
                if has {
                    Ok(Value::Boolean(value.is_some()))
                } else {
                    value
                        .map(|v| lua.create_string(v).map(Value::String))
                        .transpose()
                        .map(|v| v.unwrap_or(Value::Nil))
                }
            },
        )?;
        package.set(name, wrap(lua, f, "mux.connection.invalid")?)?;
    }
    mux.set("telnet", package)
}
