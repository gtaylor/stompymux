//! Native bindings for the existing mux.world package.
mod flags;
mod locks;
mod powers;
mod state;
use super::bind;
use crate::lua::{Outbox, SharedWorld, err};
use crate::{
    config::Config,
    text,
    world::{Kind, ObjectId},
};
use anyhow::Result;
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::sync::Arc;

/// Register native operations before the embedded facade is evaluated.
pub(super) fn register(
    lua: &Lua,
    api: &Table,
    config: &Config,
    world: &SharedWorld,
    outbox: &Outbox,
    palette: &Arc<text::Palette>,
) -> Result<()> {
    let w = world.clone();
    bind!(lua, api, "get", move |lua, (id, key): (i64, String)| {
        let w = w.borrow();
        let o = w
            .objects
            .get(&ObjectId(id))
            .ok_or_else(|| err("object does not exist"))?;
        match key.as_str() {
            "name" => lua.to_value(&o.name),
            "description" => lua.to_value_with(
                &o.description,
                mlua::serde::SerializeOptions::new().serialize_none_to_null(false),
            ),
            "internal_description" => lua.to_value_with(
                &o.internal_description,
                mlua::serde::SerializeOptions::new().serialize_none_to_null(false),
            ),
            "type" => Ok(Value::Integer(o.kind.code())),
            "location" => lua.to_value_with(
                &o.location,
                mlua::serde::SerializeOptions::new().serialize_none_to_null(false),
            ),
            "home" => lua.to_value_with(
                &o.home,
                mlua::serde::SerializeOptions::new().serialize_none_to_null(false),
            ),
            "affiliation" => lua.to_value_with(
                &o.affiliation,
                mlua::serde::SerializeOptions::new().serialize_none_to_null(false),
            ),
            "flags" => lua.to_value(&o.flags),
            _ => Err(err("unsupported object field")),
        }
    });
    let w = world.clone();
    let p = palette.clone();
    let text_limit = config.runtime.output_message_limit;
    bind!(lua, api, "set", move |lua,
                                 (id, key, v): (
        i64,
        String,
        Value
    )| {
        let mut w = w.borrow_mut();
        let o = w
            .objects
            .get_mut(&ObjectId(id))
            .ok_or_else(|| err("object does not exist"))?;
        match key.as_str() {
            "name" => {
                let name: String = lua.from_value(v)?;
                if name.len() > text_limit {
                    return Err(err("object name exceeds text limit"));
                }
                o.name = text::validate(&p, &name).map_err(err)?;
            }
            "description" | "internal_description" => {
                let value: Option<String> = lua.from_value(v)?;
                if let Some(value) = &value {
                    if value.len() > text_limit {
                        return Err(err("description exceeds text limit"));
                    }
                    text::validate(&p, value).map_err(err)?;
                }
                if key == "description" {
                    o.description = value;
                } else {
                    o.internal_description = value;
                }
            }
            "home" => o.home = lua.from_value(v)?,
            "location" => o.location = lua.from_value(v)?,
            _ => return Err(err("unsupported object mutation")),
        }
        Ok(())
    });
    let w = world.clone();
    let c = config.clone();
    let p = palette.clone();
    bind!(lua, api, "create", move |_, t: Table| {
        let mut w = w.borrow_mut();
        let kind = Kind::from_code(t.get("type")?).map_err(err)?;
        if kind == Kind::Player || kind == Kind::Garbage {
            return Err(err("Use account registration to create players"));
        }
        let name: String = t.get("name")?;
        if name.len() > text_limit {
            return Err(err("object name exceeds text limit"));
        }
        let name = text::validate(&p, &name).map_err(err)?;
        let id = w.create(&c, name, kind);
        let o = w.objects.get_mut(&id).unwrap();
        for key in ["location", "zone", "destination"] {
            let value: Option<i64> = t.get(key)?;
            match key {
                "location" => o.location = value.map(ObjectId),
                "zone" => o.zone = value.map(ObjectId),
                _ => o.destination = value.map(ObjectId),
            }
        }
        Ok(id.0)
    });
    let w = world.clone();
    bind!(lua, api, "contents", move |lua,
                                      (id, types, viewer): (
        i64,
        Vec<i64>,
        Option<i64>
    )| {
        let w = w.borrow();
        lua.to_value(
            &w.objects
                .values()
                .filter(|o| {
                    o.location == Some(ObjectId(id))
                        && (types.is_empty() || types.contains(&o.kind.code()))
                        && viewer.is_none_or(|v| w.visible(o, ObjectId(v)))
                })
                .map(|o| o.id.0)
                .collect::<Vec<_>>(),
        )
    });
    api.set(
        "flags",
        lua.create_userdata(flags::LuaFlags)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?,
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let w = world.clone();
    bind!(lua, api, "has_flag", move |_,
                                      (id, flag): (
        i64,
        mlua::AnyUserData
    )| {
        let flag = *flag.borrow::<crate::flags::Flag>()?;
        let world = w.borrow();
        let o = world
            .objects
            .get(&ObjectId(id))
            .filter(|o| o.kind != Kind::Garbage)
            .ok_or_else(|| err("object does not exist"))?;
        Ok(o.flags.contains(flag))
    });
    let w = world.clone();
    bind!(lua, api, "flag", move |_,
                                  (id, flag, add): (
        i64,
        mlua::AnyUserData,
        bool
    )| {
        let flag = *flag.borrow::<crate::flags::Flag>()?;
        crate::flags::change(&mut w.borrow_mut(), ObjectId(1), ObjectId(id), flag, add).map_err(err)
    });
    api.set(
        "powers",
        lua.create_userdata(powers::LuaPowers)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?,
    )
    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let w = world.clone();
    bind!(lua, api, "has_power", move |_,
                                       (id, power): (
        i64,
        mlua::AnyUserData
    )| {
        let power = *power.borrow::<crate::powers::Power>()?;
        let world = w.borrow();
        let o = world
            .objects
            .get(&ObjectId(id))
            .filter(|o| o.kind != Kind::Garbage)
            .ok_or_else(|| err("object does not exist"))?;
        Ok(o.powers.contains(power))
    });
    let w = world.clone();
    bind!(lua, api, "power", move |_,
                                   (id, power, value): (
        i64,
        mlua::AnyUserData,
        bool
    )| {
        let power = *power.borrow::<crate::powers::Power>()?;
        crate::powers::change(&mut w.borrow_mut(), ObjectId(1), ObjectId(id), power, value)
            .map_err(err)
    });
    locks::register(lua, api, world, outbox).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    state::register(lua, api, config, world).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let o = outbox.clone();
    let output_settings = config.lua.clone();
    let message_limit = config.runtime.output_message_limit;
    bind!(lua, api, "pemit", move |_, (id, value): (i64, Value)| {
        let s = match value {
            Value::String(s) => text::Document::Styled(s.to_str()?.to_string()),
            Value::UserData(u) => u.borrow::<super::text::LuaDocument>()?.document.clone(),
            _ => return Err(err("output must be text or a Markdown document")),
        };
        let mut out = o.borrow_mut();
        if s.len() > message_limit
            || out.len() >= output_settings.output_entry_limit
            || out.iter().map(|(_, s)| s.len()).sum::<usize>() + s.len()
                > output_settings.output_byte_limit
        {
            return Err(err("Lua output limit exceeded"));
        }
        out.push((ObjectId(id), s));
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
        .set_name("@builtin/world")
        .call((api, mux, id))
}
