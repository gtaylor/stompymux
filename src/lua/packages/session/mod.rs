//! Embedded session facade over runtime-published connection snapshots.
use mlua::{Function, Lua, Table};

/// Session APIs read the same snapshot globals; they register no native closures.
pub(super) fn install(lua: &Lua, api: &Table, mux: &Table, id: &Function) -> mlua::Result<()> {
    lua.load(include_str!("api.lua"))
        .set_name("@builtin/session")
        .call((api, mux, id))
}
