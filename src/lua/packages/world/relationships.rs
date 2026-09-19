//! Validated direct object relationships; trusted setters do not run locks or movement hooks.
use crate::{
    flags::Flag,
    lua::{err, sources::Sources},
    runtime::SharedWorld,
    world::{Kind, ObjectId, World},
};
use mlua::{Lua, MultiValue, Table, Value};
use std::sync::Arc;

fn failure(code: &'static str, message: impl ToString) -> mlua::Error {
    super::super::error::failure(code, message)
}

/// Decode an actual integer identity, accepting the embedded object wrapper.
pub(super) fn identity(
    lua: &Lua,
    value: Value,
    world: &World,
    available: bool,
) -> mlua::Result<ObjectId> {
    let id = super::handles::identity_in(lua, value, world)?;
    let o = world
        .objects
        .get(&id)
        .filter(|o| o.kind != Kind::Garbage)
        .ok_or_else(|| failure("mux.object.invalid", "object does not exist"))?;
    if available && o.flags.contains(Flag::Going) {
        return Err(super::super::error::failure(
            "mux.object.unavailable",
            "object is being destroyed",
        ));
    }
    Ok(id)
}

pub(super) fn register(lua: &Lua, api: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let w = world.clone();
    api.set(
        "relationship",
        lua.create_function(move |lua, (object, key): (Value, String)| {
            let w = w.borrow();
            let id = identity(lua, object, &w, false)?;
            let o = &w.objects[&id];
            if key == "lua_parent" {
                return if o.lua_parent.is_empty() {
                    Ok(Value::Nil)
                } else {
                    Ok(Value::String(lua.create_string(&o.lua_parent)?))
                };
            }
            let target = match key.as_str() {
                "home" | "location" if matches!(o.kind, Kind::Thing | Kind::Player) => {
                    if key == "home" { o.home } else { o.location }
                }
                "destination" if o.kind == Kind::Exit => o.destination,
                "zone" => o.zone,
                "affiliation" => o.affiliation,
                _ => {
                    let message = match key.as_str() {
                        "home" | "location" => "object is not a thing or player",
                        "destination" => "object is not an exit",
                        _ => "unknown relationship",
                    };
                    return Err(failure("mux.object.invalid", message));
                }
            };
            Ok(target.map_or(Value::Nil, |id| Value::Integer(id.0)))
        })?,
    )?;
    let w = world.clone();
    api.set(
        "set_relationship",
        lua.create_function(move |lua, args: MultiValue| {
            if args.len() < 2 {
                return Err(err("invalid relationship"));
            }
            let mut args = args.into_iter();
            let object = args.next().unwrap();
            let Value::String(key) = args.next().unwrap() else {
                return Err(err("invalid relationship"));
            };
            let key = key.to_str()?;
            let Some(value) = args.next() else {
                let detail = match key.as_ref() {
                    "destination" => "destination is required; pass nil to unlink",
                    "home" => "home is required",
                    "zone" => "zone is required; pass nil to clear",
                    "affiliation" => "affiliation is required; pass nil to clear",
                    "lua_parent" => "parent is required; pass nil to clear",
                    _ => "relationship value is required",
                };
                return Err(super::super::error::failure_with_detail(
                    "mux.arg.invalid",
                    detail,
                    serde_json::json!({"argument":2}),
                ));
            };
            let mut world = w.borrow_mut();
            let id = identity(lua, object, &world, false)?;
            let kind = world.objects[&id].kind;
            if (key.as_ref() == "home" && !matches!(kind, Kind::Thing | Kind::Player))
                || (key.as_ref() == "destination" && kind != Kind::Exit)
            {
                return Err(failure(
                    "mux.object.invalid",
                    if key.as_ref() == "home" {
                        "object is not a thing or player"
                    } else {
                        "object is not an exit"
                    },
                ));
            }
            if world.objects[&id].flags.contains(Flag::Going) {
                return Err(failure(
                    "mux.object.unavailable",
                    if key.as_ref() == "destination" {
                        "exit is being destroyed"
                    } else {
                        "object is being destroyed"
                    },
                ));
            }
            if key.as_ref() == "lua_parent" {
                let path = match value {
                    Value::Nil => String::new(),
                    Value::String(s) => {
                        let path = s.to_str()?.to_string();
                        if path.is_empty() {
                            return Err(super::super::error::argument_failure(
                                lua,
                                2,
                                "mux.module.invalid",
                                "module path is empty",
                            ));
                        }
                        path
                    }
                    _ => {
                        return Err(super::super::error::argument_failure(
                            lua,
                            2,
                            "mux.arg.invalid",
                            "parent must be a string or nil",
                        ));
                    }
                };
                if !path.is_empty() {
                    lua.app_data_ref::<Arc<Sources>>()
                        .ok_or_else(|| err("source registry unavailable"))?
                        .contains_parent(&path)
                        .map_err(|error| failure("mux.module.invalid", error))?;
                }
                // An empty string is a malformed path; only explicit nil clears the parent.
                world.objects.get_mut(&id).unwrap().lua_parent = path;
                return Ok(());
            }
            let target = if value.is_nil() {
                if key.as_ref() == "home" {
                    return Err(super::super::error::argument_failure(
                        lua,
                        2,
                        "mux.object.invalid",
                        "new_home must be an object that can contain objects",
                    ));
                }
                None
            } else {
                Some(identity(lua, value, &world, true)?)
            };
            if let Some(target) = target {
                let target_kind = world.objects[&target].kind;
                if matches!(key.as_ref(), "home" | "destination")
                    && !matches!(target_kind, Kind::Room | Kind::Player | Kind::Thing)
                {
                    return Err(super::super::error::argument_failure(
                        lua,
                        2,
                        "mux.object.invalid",
                        if key.as_ref() == "home" {
                            "new_home must be an object that can contain objects"
                        } else {
                            "destination must be an object that can contain objects"
                        },
                    ));
                }
                if key.as_ref() == "home" {
                    if id == target {
                        return Err(super::super::error::argument_failure(
                            lua,
                            2,
                            "mux.object.invalid",
                            "object cannot be its own home",
                        ));
                    }
                }
                if key.as_ref() == "zone" && !matches!(target_kind, Kind::Room | Kind::Thing) {
                    return Err(super::super::error::argument_failure(
                        lua,
                        2,
                        "mux.object.invalid",
                        "zone must be a thing or room",
                    ));
                }
            }
            let o = world.objects.get_mut(&id).unwrap();
            match key.as_ref() {
                "home" => o.home = target,
                "destination" => o.destination = target,
                "zone" => o.zone = target,
                "affiliation" => o.affiliation = target,
                _ => return Err(failure("mux.internal", "unknown relationship")),
            }
            Ok(())
        })?,
    )?;
    Ok(())
}
