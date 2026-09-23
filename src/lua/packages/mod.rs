//! Register built-in mux bindings and embedded facades without changing require lookup.
mod btech;
mod comsys;
mod config;
pub(crate) mod error;
mod logging;
mod macros;
mod session;
mod telnet;
mod text;
pub(crate) mod world;

use crate::runtime::{Outbox, SharedWorld};
use crate::{config::Config, text::Palette};
use anyhow::Result;
use mlua::{Function, Lua, Table};
use std::sync::Arc;

/// Keep native closure installation and Lua error conversion consistent across packages.
macro_rules! bind {
    ($lua:ident,$api:ident,$name:literal,$f:expr) => {
        $api.set(
            $name,
            $lua.create_function($f)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    };
}

pub(super) use bind;

/// Register all Rust closures against the existing flat private compatibility table.
pub(super) fn register_native(
    lua: &Lua,
    config: &Config,
    world: &SharedWorld,
    outbox: &Outbox,
    effects: &crate::runtime::Effects,
    palette: &Arc<Palette>,
) -> Result<Table> {
    let api = lua
        .create_table()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    world::register(lua, &api, config, world, outbox, palette)?;
    config::register(lua, &api, config)?;
    macros::register(lua, &api, world).map_err(|e| anyhow::anyhow!("{e}"))?;
    comsys::register(lua, &api, config, world, outbox, effects)?;
    text::register(lua, &api, config, palette)?;
    logging::register(lua, &api).map_err(|e| anyhow::anyhow!("{e}"))?;
    btech::register(lua, &api, world).map_err(|e| anyhow::anyhow!("{e}"))?;
    Ok(api)
}

/// Assemble one mux namespace before game modules can observe it.
pub(super) fn install_facades(lua: &Lua, api: Table) -> Result<()> {
    let install = || -> mlua::Result<()> {
        let mux = lua.create_table()?;
        let id: Function = lua
            .load(include_str!("init.lua"))
            .set_name("@builtin/identity")
            .call(api.clone())?;
        lua.globals().set("mux", mux.clone())?;
        error::install(lua, &mux)?;
        mux.set("macro", api.get::<Table>("macro")?)?;
        btech::install(lua, &api, &mux)?;
        for pair in api.clone().pairs::<String, mlua::Value>() {
            let (name, value) = pair?;
            if let mlua::Value::Function(f) = value {
                let code = match name.as_str() {
                    "config" => "mux.config.not_found",
                    "markup" | "width" | "truncate" | "strip" | "style" | "markdown"
                    | "printable_ascii" => "mux.text.invalid",
                    "state" => "mux.state.invalid",
                    "lock_key" => "mux.access.invalid",
                    "log" => "mux.arg.invalid",
                    _ => "mux.object.invalid",
                };
                api.set(name, error::wrap(lua, f, code)?)?;
            }
        }
        world::install(lua, &api, &mux, &id)?;
        session::install(lua, &api, &mux, &id)?;
        telnet::install(lua, &mux)?;
        config::install(lua, &api, &mux, &id)?;
        text::install(lua, &api, &mux, &id)?;
        comsys::install(lua, &api, &mux, &id)?;
        logging::install(lua, &api, &mux)?;
        error::protected_calls(lua)?;
        let package: Table = lua.globals().get("package")?;
        package.get::<Table>("loaded")?.set("mux", mux)?;
        Ok(())
    };
    install().map_err(|e| anyhow::anyhow!(e.to_string()))
}

/// Remove live services from an isolated checking VM before any game source executes.
pub(super) fn restrict_checking(lua: &Lua, api: &Table) -> Result<()> {
    let apply = || -> mlua::Result<()> {
        let unavailable =
            lua.create_function(|_, _: mlua::MultiValue| -> mlua::Result<mlua::Value> {
                Err(error::failure(
                    "mux.unavailable.checking",
                    "live APIs are unavailable while checking",
                ))
            })?;
        let pure = [
            "config",
            "markup",
            "width",
            "truncate",
            "strip",
            "style",
            "markdown",
            "printable_ascii",
            // Keep message validation ahead of checking-mode rejection, matching luaL_checklstring.
            "pemit",
            "pemit_accepts",
        ];
        // These packages enforce checking-mode availability at their own API boundaries.
        let self_guarding = ["comsys", "macro", "log", "check_db"];
        // C btech entries raise per-entry "btech.<qualified_name> is unavailable
        // during @lua/check" (btech_package.c btech_lua_invoke_native); qualified
        // spellings come from the facade table and contract bindings.
        let btech: mlua::Value = api.get("btech")?;
        let mut btech_qualified = std::collections::HashMap::new();
        if let mlua::Value::Table(native) = &btech {
            if let Some(names) = native.raw_get::<Option<Table>>("__qualified_names")? {
                for pair in names.pairs::<String, String>() {
                    let (key, qualified) = pair?;
                    btech_qualified.insert(key, qualified);
                }
            }
            if let Some(bindings) = native.raw_get::<Option<Table>>("__contract_bindings")? {
                for binding in bindings.sequence_values::<Table>() {
                    let binding = binding?;
                    let key: String = binding.raw_get("key")?;
                    let group: String = binding.raw_get("group")?;
                    let name: String = binding.raw_get("name")?;
                    btech_qualified.insert(key, format!("{group}.{name}"));
                }
            }
        }
        for pair in api.clone().pairs::<String, mlua::Value>() {
            let (name, value) = pair?;
            let self_guarded = self_guarding.contains(&name.as_str());
            if matches!(value, mlua::Value::Function(_))
                && !pure.contains(&name.as_str())
                && !self_guarded
            {
                api.set(name.clone(), unavailable.clone())?;
            }
            if let mlua::Value::Table(table) = value {
                if self_guarded {
                    continue;
                }
                for pair in table.clone().pairs::<String, mlua::Value>() {
                    let (key, value) = pair?;
                    if !matches!(value, mlua::Value::Function(_)) {
                        continue;
                    }
                    if mlua::Value::Table(table.clone()) == btech {
                        let qualified = btech_qualified
                            .get(&key)
                            .map_or(key.as_str(), |qualified| qualified.as_str());
                        let message = format!("btech.{qualified} is unavailable during @lua/check");
                        let entry_unavailable = lua.create_function(
                            move |_, _: mlua::MultiValue| -> mlua::Result<()> {
                                Err(error::failure("mux.unavailable.checking", message.clone()))
                            },
                        )?;
                        table.set(key, entry_unavailable)?;
                    } else {
                        table.set(key, unavailable.clone())?;
                    }
                }
            }
        }
        // mux.session stays live: C's connected_players/who_summary have no checking
        // guard, and flow_start rejects checking itself after its luaL type checks.
        Ok(())
    };
    apply().map_err(|e| anyhow::anyhow!(e.to_string()))
}
