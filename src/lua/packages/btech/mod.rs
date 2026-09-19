//! BattleTech Lua bindings, split by package responsibility while sharing one native table.

mod character_contract;
mod characters;
mod constants;
pub(super) mod contract;
mod facade;
mod inspection_admin;
mod inspection_records;
mod inventory;
mod map_contract;
mod maps;
mod parts_contract;
mod player_contract;
mod repair_contract;
mod system_contract;
mod systems;
mod template_contract;
mod templates;
mod unit;
mod unit_admin_contract;
mod unit_contract;
mod unit_inspection;
mod unit_operations_contract;
mod unit_state;

use super::error;
use crate::{ObjectId, SharedWorld};
use mlua::{Lua, LuaSerdeExt, Table, Value};

/// Register closures before facade installation; no live data escapes checking mode.
pub(super) fn register(lua: &Lua, api: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let native = lua.create_table()?;
    inventory::register(lua, &native, world)?;
    systems::register(lua, &native, world)?;
    characters::register(lua, &native, world)?;
    character_contract::register(lua, &native, world)?;
    templates::register(lua, &native, world)?;
    template_contract::register(lua, &native, world)?;
    inspection_admin::register(lua, &native, world)?;
    maps::register(lua, &native, world)?;
    map_contract::register(lua, &native, world)?;
    unit_operations_contract::register(lua, &native, world)?;
    parts_contract::register(lua, &native, world)?;
    player_contract::register(lua, &native, world)?;
    repair_contract::register(lua, &native, world)?;
    system_contract::register(lua, &native, world)?;
    unit_state::register(lua, &native, world)?;
    unit::register(lua, &native, world)?;
    unit_inspection::register(lua, &native, world)?;
    unit_contract::register(lua, &native, world)?;
    unit_admin_contract::register(lua, &native, world)?;
    api.set("btech", native)
}

pub(super) use facade::install;

/// Convert an owned Rust value into a Lua value without exposing live world data.
pub(super) fn detached<T: serde::Serialize + ?Sized>(lua: &Lua, value: &T) -> mlua::Result<Value> {
    lua.to_value_with(
        value,
        mlua::serde::SerializeOptions::new()
            .serialize_none_to_null(false)
            .serialize_unit_to_null(false),
    )
}

/// Decode the target grammar shared by direct firing and TIC commands.
pub(super) fn firing_target(lua: &Lua, value: Value) -> mlua::Result<crate::BattleFireTarget> {
    match value {
        Value::Nil => Ok(crate::BattleFireTarget::Selected),
        Value::Table(_) => Ok(crate::BattleFireTarget::Hex {
            coordinate: lua.from_value(value)?,
        }),
        _ => Ok(crate::BattleFireTarget::Unit {
            unit: ObjectId(lua.from_value(value)?),
        }),
    }
}
