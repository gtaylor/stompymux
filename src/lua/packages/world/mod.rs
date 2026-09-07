//! Native bindings for the existing mux.world package.
mod flags;
mod locks;
mod powers;
mod relationships;
mod state;
mod types;
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
            .filter(|o| o.kind != Kind::Garbage)
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
            "type" => Ok(Value::UserData(lua.create_userdata(o.kind)?)),
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
            "location" => {
                let destination = relationships::identity(v, &w, true)?;
                relationships::identity(Value::Integer(id), &w, true)?;
                w.validate_move(ObjectId(id), destination).map_err(err)?;
                w.objects.get_mut(&ObjectId(id)).unwrap().location = Some(destination);
            }
            _ => return Err(err("unsupported object mutation")),
        }
        Ok(())
    });
    let w = world.clone();
    let p = palette.clone();
    bind!(lua, api, "create", move |lua, t: Table| {
        let c = crate::lua::configuration(lua);
        let mut w = w.borrow_mut();
        let kind = types::kind(t.get("type")?)?;
        if kind == Kind::Player || kind == Kind::Garbage {
            return Err(err("Use account registration to create players"));
        }
        let name: String = t.get("name")?;
        if name.len() > text_limit {
            return Err(err("object name exceeds text limit"));
        }
        let name = text::validate(&p, &name).map_err(err)?;
        types::options(
            &t,
            match kind {
                Kind::Room => &["type", "name", "zone"],
                Kind::Thing => &["type", "name", "location", "home", "zone"],
                _ => &["type", "name", "location", "destination", "zone"],
            },
        )?;
        let mut references = std::collections::BTreeMap::new();
        for key in ["location", "home", "zone", "destination"] {
            let value: Value = t.raw_get(key)?;
            if value.is_nil() {
                continue;
            }
            let target = relationships::identity(value, &w, true)?;
            let target_kind = w.objects[&target].kind;
            if key == "zone" {
                if !matches!(target_kind, Kind::Room | Kind::Thing) {
                    return Err(err("zone must be a room or thing"));
                }
            } else if !matches!(target_kind, Kind::Room | Kind::Thing | Kind::Player) {
                return Err(err("target cannot contain objects"));
            }
            references.insert(key, target);
        }
        let id = w.create(&c, name, kind);
        let o = w.objects.get_mut(&id).unwrap();
        o.location = references.get("location").copied();
        o.home = references.get("home").copied();
        o.zone = references.get("zone").copied();
        o.destination = references.get("destination").copied();
        Ok(id.0)
    });
    let w = world.clone();
    bind!(lua, api, "contents", move |lua,
                                      (id, options): (
        Value,
        Table
    )| {
        let w = w.borrow();
        let id = relationships::identity(id, &w, false)?;
        types::options(&options, &["types", "visible_to"])?;
        let types = types::filter(options.raw_get("types")?)?;
        let viewer = match options.raw_get::<Value>("visible_to")? {
            Value::Nil => None,
            v => Some(relationships::identity(v, &w, false)?),
        };
        lua.to_value(
            &w.objects
                .values()
                .filter(|o| {
                    o.kind != Kind::Garbage
                        && o.location == Some(id)
                        && types.as_ref().is_none_or(|types| types.contains(&o.kind))
                        && viewer.is_none_or(|v| w.visible(o, v))
                })
                .map(|o| o.id.0)
                .collect::<Vec<_>>(),
        )
    });
    let w = world.clone();
    bind!(
        lua,
        api,
        "list_objects",
        move |lua, options: Option<Table>| {
            let w = w.borrow();
            let mut types = None;
            let mut zone = None;
            if let Some(options) = options {
                types::options(&options, &["types", "in_zone"])?;
                types = types::filter(options.raw_get("types")?)?;
                if let value @ (Value::Integer(_) | Value::Number(_) | Value::Table(_)) =
                    options.raw_get::<Value>("in_zone")?
                {
                    zone = Some(relationships::identity(value, &w, true)?);
                } else if !options.raw_get::<Value>("in_zone")?.is_nil() {
                    return Err(err("invalid zone object"));
                }
            }
            lua.to_value(
                &w.objects
                    .values()
                    .filter(|o| {
                        o.kind != Kind::Garbage
                            && types.as_ref().is_none_or(|types| types.contains(&o.kind))
                            && zone.is_none_or(|zone| o.zone == Some(zone))
                    })
                    .map(|o| o.id.0)
                    .collect::<Vec<_>>(),
            )
        }
    );
    api.set(
        "types",
        lua.create_userdata(types::Types)
            .map_err(|e| anyhow::anyhow!("{e}"))?,
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    relationships::register(lua, api, world).map_err(|e| anyhow::anyhow!("{e}"))?;
    let w = world.clone();
    bind!(lua, api, "list_flags", move |lua, id: Value| {
        let w = w.borrow();
        let id = relationships::identity(id, &w, false)?;
        lua.create_sequence_from(
            crate::flags::ALL
                .into_iter()
                .filter(|f| w.objects[&id].flags.contains(*f)),
        )
    });
    let w = world.clone();
    bind!(lua, api, "list_powers", move |lua, id: Value| {
        let w = w.borrow();
        let id = relationships::identity(id, &w, false)?;
        lua.create_sequence_from(
            crate::powers::ALL
                .into_iter()
                .filter(|p| w.objects[&id].powers.contains(*p)),
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
    let w = world.clone();
    bind!(lua, api, "pemit", move |lua, (id, value): (i64, Value)| {
        let config = crate::lua::configuration(lua);
        let document = match value {
            Value::String(s) => text::Document::Styled(s.to_str()?.to_string()),
            Value::UserData(u) => u.borrow::<super::text::LuaDocument>()?.document.clone(),
            _ => return Err(err("output must be text or a Markdown document")),
        };
        let world = w.borrow();
        if !world
            .objects
            .get(&ObjectId(id))
            .is_some_and(|o| o.kind != Kind::Garbage)
        {
            return Err(err("object does not exist"));
        }
        crate::notification::send(
            &world,
            &o,
            &config,
            crate::notification::Request {
                target: ObjectId(id),
                sender: ObjectId(id),
                document,
                policy: crate::notification::Policy::DIRECT,
                exclusions: None,
            },
        )
        .map_err(err)
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
