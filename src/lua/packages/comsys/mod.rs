//! Native bindings for the existing mux.comsys package.
use super::bind;
use crate::{
    lua::SharedWorld,
    world::{Channel, ObjectId},
};
use anyhow::Result;
use mlua::{Lua, Table};

/// Register native operations before the embedded facade is evaluated.
pub(super) fn register(lua: &Lua, api: &Table, world: &SharedWorld) -> Result<()> {
    let w = world.clone();
    bind!(lua, api, "channel", move |_,
                                     (name, object, flag): (
        String,
        Option<i64>,
        Option<i64>
    )| {
        let mut w = w.borrow_mut();
        let channel = w.channels.entry(name.clone()).or_insert(Channel {
            name,
            object: None,
            flags: 0,
            messages: 0,
        });
        if let Some(id) = object {
            channel.object = Some(ObjectId(id));
        }
        if let Some(f) = flag {
            channel.flags |= f;
        }
        Ok(())
    });
    Ok(())
}

/// Install the embedded Lua facade with explicit shared table and identity dependencies.
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    mux: &Table,
    id: &mlua::Function,
) -> mlua::Result<()> {
    lua.load(include_str!("api.lua"))
        .set_name("@builtin/comsys")
        .call((api, mux, id))
}
