//! Unit command bindings split into topic slices that share one private native table.

mod ammunition;
mod displays;
mod equipment;
mod firing;
mod movement;
mod physical;
mod radio;
mod self_destruct;
mod sensors;
mod settings;
mod targeting;
mod weapons;

use super::*;

/// Register every unit command slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    physical::register(lua, native, world)?;
    equipment::register(lua, native, world)?;
    settings::register(lua, native, world)?;
    movement::register(lua, native, world)?;
    targeting::register(lua, native, world)?;
    sensors::register(lua, native, world)?;
    displays::register(lua, native, world)?;
    radio::register(lua, native, world)?;
    weapons::register(lua, native, world)?;
    ammunition::register(lua, native, world)?;
    firing::register(lua, native, world)?;
    self_destruct::register(lua, native, world)?;
    Ok(())
}
