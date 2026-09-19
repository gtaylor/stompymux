//! Embedded session facade over runtime-published connection snapshots.
use mlua::{Function, Lua, MultiValue, Table};

/// Register session snapshots and the transaction-aware flow engine.
pub(super) fn install(lua: &Lua, api: &Table, mux: &Table, id: &Function) -> mlua::Result<()> {
    api.set(
        "connected_players",
        lua.create_function(|lua, ()| {
            // C pushes object handles directly without a checking guard.
            let snapshot = lua.app_data_ref::<crate::lua::sessions::Sessions>();
            let players = snapshot
                .as_ref()
                .map(|s| s.players.as_slice())
                .unwrap_or(&[]);
            let world = crate::Scripts::services(lua)?.world;
            let result = lua.create_table()?;
            for player in players {
                let entry = lua.create_table()?;
                entry.set(
                    "object",
                    super::world::handles::push(lua, &world, crate::world::ObjectId(player.dbref))?,
                )?;
                entry.set("name", player.name.clone())?;
                entry.set("connected_for", player.connected_for)?;
                entry.set("idle_for", player.idle_for)?;
                result.push(entry)?;
            }
            Ok(result)
        })?,
    )?;
    api.set(
        "who_summary",
        lua.create_function(|lua, ()| {
            let t = lua.create_table()?;
            let s = lua.app_data_ref::<crate::lua::sessions::Sessions>();
            t.set("hidden", 0)?;
            t.set("record", s.as_ref().map_or(0, |s| s.record))?;
            let maximum = s.as_ref().and_then(|s| s.maximum).or_else(|| {
                let configured = crate::lua::configuration(lua).mux.max_players;
                (configured != -1).then_some(configured)
            });
            if let Some(maximum) = maximum {
                t.set("maximum", maximum)?;
            }
            Ok(t)
        })?,
    )?;
    api.set(
        "flow_start",
        lua.create_function(|lua, values: MultiValue| {
            // C validates all three arguments with luaL checks before any host work.
            // The descriptor integer is narrowed through C's (int) cast.
            let descriptor = super::error::check_integer_arg(lua, &values, 1)? as u32 as u64;
            let module = super::error::check_string_arg(lua, &values, 2)?;
            let step = super::error::check_string_arg(lua, &values, 3)?;
            let truncate = |bytes: &[u8]| {
                let end = bytes
                    .iter()
                    .position(|byte| *byte == 0)
                    .unwrap_or(bytes.len());
                String::from_utf8_lossy(&bytes[..end]).into_owned()
            };
            let module = truncate(&module);
            let step = truncate(&step);
            let engine = lua
                .app_data_ref::<crate::lua::flows::Engine>()
                .expect("flow engine installed")
                .clone();
            engine.start(lua, descriptor, &module, &step)
        })?,
    )?;
    let f = api.get("flow_start")?;
    api.set("flow_start", super::error::wrap(lua, f, "mux.runtime")?)?;
    lua.load(include_str!("api.lua"))
        .set_name("@builtin/session")
        .call((api, mux, id))
}
