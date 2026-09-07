//! Register built-in mux bindings and embedded facades without changing require lookup.
mod comsys;
mod config;
mod error;
mod session;
mod text;
mod world;

use super::{Outbox, SharedWorld};
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
    palette: &Arc<Palette>,
) -> Result<Table> {
    let api = lua
        .create_table()
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    world::register(lua, &api, config, world, outbox, palette)?;
    config::register(lua, &api, config)?;
    comsys::register(lua, &api, config, world, outbox)?;
    text::register(lua, &api, config, palette)?;
    Ok(api)
}

/// Assemble one mux namespace before game modules can observe it.
pub(super) fn install_facades(lua: &Lua, api: Table) -> Result<()> {
    let install = || -> mlua::Result<()> {
        let mux = lua.create_table()?;
        let id: Function = lua
            .load(include_str!("init.lua"))
            .set_name("@builtin/identity")
            .eval()?;
        lua.globals().set("mux", mux.clone())?;
        error::install(lua, &mux)?;
        world::install(lua, &api, &mux, &id)?;
        session::install(lua, &api, &mux, &id)?;
        config::install(lua, &api, &mux, &id)?;
        text::install(lua, &api, &mux, &id)?;
        comsys::install(lua, &api, &mux, &id)?;
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
                Err(mlua::Error::external(
                    "mux.unavailable.checking: live APIs are unavailable while checking",
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
        ];
        for pair in api.clone().pairs::<String, mlua::Value>() {
            let (name, value) = pair?;
            if matches!(value, mlua::Value::Function(_)) && !pure.contains(&name.as_str()) {
                api.set(name, unavailable.clone())?;
            }
            if let mlua::Value::Table(table) = value {
                for pair in table.clone().pairs::<String, mlua::Value>() {
                    let (key, value) = pair?;
                    if matches!(value, mlua::Value::Function(_)) {
                        table.set(key, unavailable.clone())?;
                    }
                }
            }
        }
        let mux: Table = lua.globals().get("mux")?;
        let session: Table = mux.get("session")?;
        for pair in session.clone().pairs::<String, mlua::Value>() {
            let (key, value) = pair?;
            if matches!(value, mlua::Value::Function(_)) {
                session.set(key, unavailable.clone())?;
            }
        }
        Ok(())
    };
    apply().map_err(|e| anyhow::anyhow!("{e}"))
}
