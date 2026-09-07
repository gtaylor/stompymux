//! Private callback-boundary session snapshots, independent of durable world rollback.
use mlua::{Lua, LuaSerdeExt, Table};
use std::collections::BTreeMap;
#[derive(Clone, serde::Serialize)]
pub(crate) struct Player {
    pub name: String,
    pub dbref: i64,
    pub session: u64,
    pub terminal_width: u16,
    pub connected_for: u64,
    pub idle_for: u64,
}
#[derive(Clone, Default)]
pub(crate) struct Sessions {
    pub players: Vec<Player>,
    pub hidden: usize,
    pub record: i64,
    pub maximum: Option<i64>,
    pub environments: BTreeMap<u64, crate::telnet::environment::Environment>,
}
/// Read a fresh Lua value; editing it cannot corrupt the host's snapshot.
pub(crate) fn players(lua: &Lua) -> mlua::Result<Table> {
    let snapshot = lua.app_data_ref::<Sessions>();
    let players = snapshot
        .as_ref()
        .map(|s| s.players.as_slice())
        .unwrap_or(&[]);
    let mlua::Value::Table(table) = lua.to_value(players)? else {
        unreachable!()
    };
    Ok(table)
}
