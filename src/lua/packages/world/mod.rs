//! Native bindings for the existing mux.world package.
mod flags;
pub(crate) mod handles;
mod locks;
mod powers;
mod relationships;
mod state;
mod types;
use super::bind;
use crate::{
    config::Config,
    text,
    world::{Kind, ObjectId},
};
use crate::{
    lua::err,
    runtime::{Outbox, SharedWorld},
};

pub(super) fn protect_userdata_metatable(
    lua: &mlua::Lua,
    value: &mlua::AnyUserData,
    label: &'static str,
) -> mlua::Result<()> {
    unsafe {
        lua.exec_raw::<()>((value.clone(), label), |state| {
            if mlua::ffi::lua_getmetatable(state, 1) != 0 {
                mlua::ffi::lua_pushvalue(state, 2);
                mlua::ffi::lua_setfield(state, -2, c"__metatable".as_ptr());
            }
        })
    }
}
use anyhow::Result;
use mlua::{Lua, LuaSerdeExt, Table, Value};
use std::sync::Arc;

/// Match the object-list visibility rules used by C's Lua contents binding.
fn contents_visible(
    world: &crate::world::World,
    container: ObjectId,
    object: &crate::world::Object,
    viewer: ObjectId,
) -> bool {
    use crate::flags::Flag;
    if object.kind == Kind::Exit {
        return !object.flags.contains(Flag::Dark)
            && (object.flags.contains(Flag::Light)
                || !world.objects[&container].flags.contains(Flag::Dark));
    }
    if object.kind == Kind::Player && !object.flags.contains(Flag::Connected) {
        return false;
    }
    if object.id == viewer || object.flags.contains(Flag::Dark) {
        return false;
    }
    !world.objects[&container].flags.contains(Flag::Dark) || object.flags.contains(Flag::Light)
}

/// Register native operations before the embedded facade is evaluated.
pub(super) fn register(
    lua: &Lua,
    api: &Table,
    config: &Config,
    world: &SharedWorld,
    outbox: &Outbox,
    palette: &Arc<text::Palette>,
) -> Result<()> {
    crate::lua::command_access::install(lua, api).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    bind!(lua, api, "parent", |lua, path: String| {
        lua.named_registry_value::<Table>("mux.parents")?
            .get::<Value>(path)
    });
    bind!(
        lua,
        api,
        "register_lock_result",
        |lua, function: mlua::Function| {
            lua.set_named_registry_value("mux.lock_result", function)
        }
    );
    handles::register(lua, api, world).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let w = world.clone();
    bind!(
        lua,
        api,
        "destroy_object",
        move |lua, arguments: mlua::MultiValue| {
            crate::lua::transactions::require(lua)?;
            let value = arguments.front().cloned().unwrap_or(Value::Nil);
            let mut w = w.borrow_mut();
            let id = relationships::identity(lua, value, &w, false)?;
            let override_safe = if let Some(options) = arguments.get(1).cloned() {
                if options.is_nil() {
                    false
                } else {
                    let Value::Table(t) = options else {
                        return Err(super::error::argument_failure(
                            lua,
                            2,
                            "mux.arg.invalid",
                            "options must be a table",
                        ));
                    };
                    types::options(lua, &t, 2, &["override"])?;
                    match t.get::<Value>("override")? {
                        Value::Nil => false,
                        Value::Boolean(b) => b,
                        _ => {
                            return Err(super::error::argument_failure(
                                lua,
                                2,
                                "mux.arg.invalid",
                                "options.override must be a boolean",
                            ));
                        }
                    }
                }
            } else {
                false
            };
            crate::destruction::schedule(
                &mut w,
                &crate::lua::configuration(lua),
                ObjectId(1),
                id,
                override_safe,
            )
            .map_err(|e| super::error::failure("mux.object.unavailable", e))?;
            Ok(())
        }
    );
    bind!(lua, api, "check_db", |lua, ()| {
        if lua
            .app_data_ref::<crate::lua::RuntimeMode>()
            .is_some_and(|mode| *mode == crate::lua::RuntimeMode::Checking)
        {
            return Err(super::error::failure(
                "mux.unavailable.checking",
                "mux.check_db is unavailable during @lua/check",
            ));
        }
        crate::lua::maintenance::check(lua)
    });
    bind!(lua, api, "teleport_object", |lua, value: Value| {
        crate::lua::transactions::require(lua)?;
        let Value::Table(t) = value else {
            return Err(super::error::argument_failure(
                lua,
                1,
                "mux.arg.invalid",
                "options must be a table",
            ));
        };
        types::options(lua, &t, 1, &["object", "destination"])?;
        let s = crate::lua::Scripts::services(lua)?;
        let (id, dest, source) = {
            let w = s.world.borrow();
            let id = relationships::identity(lua, t.get("object")?, &w, true)?;
            let dest = relationships::identity(lua, t.get("destination")?, &w, true)?;
            if !matches!(w.objects[&id].kind, Kind::Thing | Kind::Player) {
                return Err(super::error::failure(
                    "mux.object.invalid",
                    "options.object must be a thing or player",
                ));
            }
            (id, dest, w.objects[&id].location)
        };
        if source == Some(dest) {
            return Ok(());
        }
        crate::lua::transactions::run(lua, &s.world, || {
            crate::lua::transactions::with_cause(lua, ObjectId(1), || {
                crate::movement::perform(
                    &s,
                    crate::movement::Request {
                        actor: ObjectId(1),
                        object: id,
                        cause: ObjectId(1),
                        destination: dest,
                        session: None,
                        route: crate::movement::Route::Teleport,
                    },
                )
            })
            .map_err(|e| super::error::failure("mux.object.unavailable", e))?;
            if s.world.borrow().objects[&id].location != Some(dest) {
                return Err(super::error::failure(
                    "mux.object.unavailable",
                    "teleport was denied",
                ));
            }
            Ok(())
        })
    });
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
            "type" => {
                let value = lua.create_userdata(o.kind)?;
                protect_userdata_metatable(
                    lua,
                    &value,
                    "protected object type constant metatable",
                )?;
                Ok(Value::UserData(value))
            }
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
    let text_limit = config.runtime.output_message_limit.min(8191);
    bind!(lua, api, "set", move |lua, arguments: mlua::MultiValue| {
        let id = match arguments.front() {
            Some(Value::Integer(id)) => *id,
            _ => return Err(err("object does not exist")),
        };
        let key = match arguments.get(1) {
            Some(Value::String(key)) => key.to_str()?.to_string(),
            _ => return Err(err("unsupported object mutation")),
        };
        let mut w = w.borrow_mut();
        let o = w
            .objects
            .get_mut(&ObjectId(id))
            .ok_or_else(|| err("object does not exist"))?;
        if o.flags.contains(crate::flags::Flag::Going) {
            return Err(super::error::failure(
                "mux.object.unavailable",
                "object is being destroyed",
            ));
        }
        let Some(v) = arguments.get(2).cloned() else {
            let detail = match key.as_str() {
                "name" => "name is required",
                "description" | "internal_description" => {
                    "description is required; pass nil to clear it"
                }
                _ => "value is required",
            };
            return Err(super::error::failure_with_detail(
                "mux.arg.invalid",
                detail,
                serde_json::json!({"argument":2}),
            ));
        };
        match key.as_str() {
            "name" => {
                let Value::String(value) = v else {
                    return Err(super::error::failure(
                        "mux.arg.invalid",
                        "name must be a string",
                    ));
                };
                let name = value
                    .to_str()
                    .map_err(|e| super::error::failure("mux.arg.invalid", e))?
                    .to_string();
                if name.contains('\0') {
                    return Err(super::error::failure(
                        "mux.arg.invalid",
                        "name contains NUL",
                    ));
                }
                if name.len() > text_limit {
                    return Err(super::error::failure(
                        "mux.arg.invalid",
                        "object name exceeds text limit",
                    ));
                }
                o.name = text::validate(&p, &name)
                    .map_err(|e| super::error::failure("mux.arg.invalid", e))?;
            }
            "description" | "internal_description" => {
                let mut value = match v {
                    Value::Nil => None,
                    Value::String(value) => Some(value),
                    Value::Integer(value) => lua.coerce_string(Value::Integer(value))?,
                    Value::Number(value) => lua.coerce_string(Value::Number(value))?,
                    _ => {
                        return Err(super::error::argument_failure(
                            lua,
                            2,
                            "mux.arg.invalid",
                            "string expected",
                        ));
                    }
                };
                let mut value = value
                    .map(|value| {
                        if value.as_bytes().contains(&0) || value.to_str().is_err() {
                            return Err(super::error::argument_failure(
                                lua, 2, "mux.arg.invalid",
                                "description must be valid UTF-8 without embedded NUL bytes and shorter than LBUF_SIZE",
                            ));
                        }
                        Ok(value.to_str().expect("checked UTF-8").to_string())
                    })
                    .transpose()?;
                if value.as_deref() == Some("") {
                    value = None;
                }
                if let Some(value) = &value {
                    if value.len() > text_limit {
                        return Err(super::error::argument_failure(
                            lua,
                            2,
                            "mux.arg.invalid",
                            "description must be valid UTF-8 without embedded NUL bytes and shorter than LBUF_SIZE",
                        ));
                    }
                    text::validate(&p, value).map_err(|e| {
                        super::error::argument_failure(
                            lua,
                            2,
                            "mux.arg.invalid",
                            format!("description has invalid styled-text markup: {e}"),
                        )
                    })?;
                }
                if key == "description" {
                    o.description = value;
                } else {
                    o.internal_description = value;
                }
            }
            "location" => {
                let destination = relationships::identity(lua, v, &w, true)?;
                relationships::identity(lua, Value::Integer(id), &w, true)?;
                w.validate_move(ObjectId(id), destination).map_err(err)?;
                w.objects.get_mut(&ObjectId(id)).unwrap().location = Some(destination);
                crate::btech::player_moved(&mut w, ObjectId(id));
            }
            _ => return Err(err("unsupported object mutation")),
        }
        Ok(())
    });
    let w = world.clone();
    let p = palette.clone();
    bind!(lua, api, "create", move |lua, value: Value| {
        let c = crate::lua::configuration(lua);
        let Value::Table(t) = value else {
            return Err(super::error::argument_failure(
                lua,
                1,
                "mux.arg.invalid",
                "options must be a table",
            ));
        };
        let mut w = w.borrow_mut();
        let type_value = t.get::<Value>("type")?;
        if type_value.is_nil() {
            return Err(super::error::argument_failure(
                lua,
                1,
                "mux.arg.invalid",
                "options.type is required",
            ));
        }
        let kind = if let Value::UserData(value) = type_value {
            *value.borrow::<Kind>().map_err(|_| {
                super::error::argument_failure(
                    lua,
                    1,
                    "mux.arg.invalid",
                    "options.type must be a mux.world.types constant from this runtime",
                )
            })?
        } else {
            return Err(super::error::argument_failure(
                lua,
                1,
                "mux.arg.invalid",
                "options.type must be a mux.world.types constant from this runtime",
            ));
        };
        if kind == Kind::Player || kind == Kind::Garbage {
            return Err(super::error::argument_failure(
                lua,
                1,
                "mux.arg.invalid",
                "options.type must be ROOM, THING, or EXIT",
            ));
        }
        let Value::String(value) = t.get::<Value>("name")? else {
            return Err(super::error::argument_failure(
                lua,
                1,
                "mux.arg.invalid",
                "options.name must be a string",
            ));
        };
        let name = value
            .to_str()
            .map_err(|_| {
                super::error::argument_failure(
                    lua,
                    1,
                    "mux.arg.invalid",
                    "options.name is not valid UTF-8",
                )
            })?
            .to_string();
        if name.contains('\0') {
            return Err(super::error::argument_failure(
                lua,
                1,
                "mux.arg.invalid",
                "options.name contains an embedded NUL byte",
            ));
        }
        if name.len() > text_limit {
            return Err(super::error::argument_failure(
                lua,
                1,
                "mux.arg.invalid",
                "options.name is too long",
            ));
        }
        let name =
            text::validate(&p, &name).map_err(|e| super::error::failure("mux.arg.invalid", e))?;
        types::options(
            lua,
            &t,
            1,
            match kind {
                Kind::Room => &["type", "name", "zone"],
                Kind::Thing => &["type", "name", "location", "home", "zone"],
                _ => &["type", "name", "location", "destination", "zone"],
            },
        )?;
        let mut references = std::collections::BTreeMap::new();
        for key in ["location", "home", "zone", "destination"] {
            let value: Value = t.get(key)?;
            if value.is_nil() {
                continue;
            }
            let target = relationships::identity(lua, value, &w, true)?;
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
        if matches!(kind, Kind::Thing | Kind::Exit) && !references.contains_key("location") {
            return Err(super::error::argument_failure(
                lua,
                1,
                "mux.arg.invalid",
                "options.location is required",
            ));
        }
        if kind == Kind::Thing && !references.contains_key("home") {
            references.insert("home", references["location"]);
        }
        let id = w
            .create_with(
                &c,
                name,
                kind,
                crate::CreationContext::Object {
                    creator: ObjectId(1),
                    zone: references.get("zone").copied(),
                },
            )
            .map_err(err)?;
        let o = w.objects.get_mut(&id).unwrap();
        o.location = references.get("location").copied();
        o.home = references.get("home").copied();
        o.destination = references.get("destination").copied();
        Ok(id.0)
    });
    let w = world.clone();
    bind!(lua, api, "contents", move |lua,
                                      (id, options): (
        Value,
        Value
    )| {
        let w = w.borrow();
        let id = relationships::identity(lua, id, &w, false)?;
        if !matches!(w.objects[&id].kind, Kind::Room | Kind::Thing | Kind::Player) {
            return Err(err("object can hold neither contents nor exits"));
        }
        let Value::Table(options) = options else {
            return Err(super::error::argument_failure(
                lua,
                2,
                "mux.arg.invalid",
                "options must be a table",
            ));
        };
        types::options(lua, &options, 2, &["types", "visible_to"])?;
        let types = types::filter(lua, options.get("types")?, 2)?;
        let viewer = match options.get::<Value>("visible_to")? {
            Value::Nil => None,
            v => Some(relationships::identity(lua, v, &w, false)?),
        };
        // C walks the head-inserted contents chain and then the exit chain
        // (mux_object_bindings.c lua_mux_contents DOLIST), so members come back
        // newest-first and exits follow every other member.
        let mut members: Vec<_> = w
            .objects
            .values()
            .filter(|o| {
                o.kind != Kind::Garbage
                    && o.location == Some(id)
                    && o.kind != Kind::Exit
                    && types.as_ref().is_none_or(|types| types.contains(&o.kind))
                    && viewer.is_none_or(|v| contents_visible(&w, id, o, v))
            })
            .map(|o| o.id.0)
            .collect();
        members.reverse();
        let mut exits: Vec<_> = w
            .objects
            .values()
            .filter(|o| {
                o.kind == Kind::Exit
                    && o.location == Some(id)
                    && types.as_ref().is_none_or(|types| types.contains(&o.kind))
                    && viewer.is_none_or(|v| contents_visible(&w, id, o, v))
            })
            .map(|o| o.id.0)
            .collect();
        exits.reverse();
        members.extend(exits);
        lua.to_value(&members)
    });
    let w = world.clone();
    bind!(lua, api, "list_objects", move |lua, options: Value| {
        let w = w.borrow();
        let mut types = None;
        let mut zone = None;
        if !options.is_nil() {
            let Value::Table(options) = options else {
                return Err(super::error::argument_failure(
                    lua,
                    1,
                    "mux.arg.invalid",
                    "options must be a table",
                ));
            };
            types::options(lua, &options, 1, &["types", "in_zone"])?;
            types = types::filter(lua, options.get("types")?, 1)?;
            let value = options.get::<Value>("in_zone")?;
            if !value.is_nil() {
                zone = Some(relationships::identity(lua, value, &w, true)?);
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
    });
    let types = lua
        .create_userdata(types::Types)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    protect_userdata_metatable(lua, &types, "protected object type namespace metatable")
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    api.set("types", types)
        .map_err(|e| anyhow::anyhow!("{e}"))?;
    relationships::register(lua, api, world).map_err(|e| anyhow::anyhow!("{e}"))?;
    let flags = lua
        .create_userdata(flags::LuaFlags)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    protect_userdata_metatable(lua, &flags, "protected flag or power namespace metatable")
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    api.set("flags", flags)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    let powers = lua
        .create_userdata(powers::LuaPowers)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    protect_userdata_metatable(lua, &powers, "protected flag or power namespace metatable")
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    api.set("powers", powers)
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
    locks::register(lua, api, world).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    state::register(lua, api, config, world).map_err(|e| anyhow::anyhow!(e.to_string()))?;
    bind!(lua, api, "pemit_accepts", |_, value: Value| {
        Ok(matches!(
            value,
            Value::String(_) | Value::Integer(_) | Value::Number(_)
        ) || matches!(value, Value::UserData(ref value) if value.borrow::<super::text::LuaDocument>().is_ok()))
    });
    let o = outbox.clone();
    let w = world.clone();
    bind!(
        lua,
        api,
        "pemit",
        move |lua, (object, value): (Value, Value)| {
            let config = crate::lua::configuration(lua);
            let document = match value {
                Value::String(s) => {
                    if s.as_bytes().contains(&0) {
                        return Err(super::error::argument_failure(
                            lua,
                            2,
                            "mux.connection.invalid",
                            "message contains an embedded NUL byte",
                        ));
                    }
                    let message = s.to_str().map_err(|_| {
                        super::error::argument_failure(
                            lua,
                            2,
                            "mux.connection.invalid",
                            "message is not valid UTF-8",
                        )
                    })?;
                    text::Document::Styled(message.to_string())
                }
                Value::Integer(value) => text::Document::Styled(value.to_string()),
                Value::Number(value) => text::Document::Styled(
                    lua.coerce_string(Value::Number(value))?
                        .expect("numbers coerce to strings")
                        .to_str()?
                        .to_string(),
                ),
                Value::UserData(u) => u.borrow::<super::text::LuaDocument>()?.document.clone(),
                _ => {
                    return Err(super::error::argument_failure(
                        lua,
                        2,
                        "mux.connection.invalid",
                        "string expected",
                    ));
                }
            };
            if lua
                .app_data_ref::<crate::lua::RuntimeMode>()
                .is_some_and(|mode| *mode == crate::lua::RuntimeMode::Checking)
            {
                return Err(super::error::failure(
                    "mux.unavailable.checking",
                    "mux.world.pemit is unavailable during @lua/check",
                ));
            }
            let world = w.borrow();
            let id = handles::identity_at(lua, object, &world, 1, "object")?;
            crate::notification::send(
                &world,
                &o,
                &config,
                crate::notification::Request {
                    target: id,
                    sender: id,
                    document,
                    policy: crate::notification::Policy::DIRECT,
                    exclusions: None,
                },
            )
            .map_err(err)
        }
    );
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
