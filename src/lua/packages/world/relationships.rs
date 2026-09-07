//! Validated direct object relationships; trusted setters do not run locks or movement hooks.
use crate::{
    flags::Flag,
    lua::{SharedWorld, err, sources::Sources},
    world::{Kind, ObjectId, World},
};
use mlua::{Lua, MultiValue, Table, Value};
use std::sync::Arc;

/// Decode an actual integer identity, accepting the embedded object wrapper.
pub(super) fn identity(value: Value, world: &World, available: bool) -> mlua::Result<ObjectId> {
    let value = if let Value::Table(t) = value {
        t.raw_get("_id")?
    } else {
        value
    };
    let id = match value {
        Value::Integer(n) => ObjectId(n),
        Value::Number(n)
            if n.is_finite() && n.fract() == 0.0 && n >= 0.0 && n < i64::MAX as f64 =>
        {
            ObjectId(n as i64)
        }
        _ => return Err(err("expected an object or integer dbref")),
    };
    let o = world
        .objects
        .get(&id)
        .filter(|o| o.kind != Kind::Garbage)
        .ok_or_else(|| err("object does not exist"))?;
    if available && o.flags.contains(Flag::Going) {
        return Err(err("object is being destroyed"));
    }
    Ok(id)
}

pub(super) fn register(lua: &Lua, api: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let w = world.clone();
    api.set(
        "relationship",
        lua.create_function(move |lua, (object, key): (Value, String)| {
            let w = w.borrow();
            let id = identity(object, &w, false)?;
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
                _ => return Err(err("relationship is not valid for this object type")),
            };
            Ok(target.map_or(Value::Nil, |id| Value::Integer(id.0)))
        })?,
    )?;
    let w = world.clone();
    api.set(
        "set_relationship",
        lua.create_function(move |lua, args: MultiValue| {
            if args.len() != 3 {
                return Err(err(
                    "relationship value is required; supply nil explicitly to clear",
                ));
            }
            let mut args = args.into_iter();
            let object = args.next().unwrap();
            let Value::String(key) = args.next().unwrap() else {
                return Err(err("invalid relationship"));
            };
            let key = key.to_str()?;
            let value = args.next().unwrap();
            let mut world = w.borrow_mut();
            let id = identity(object, &world, true)?;
            let kind = world.objects[&id].kind;
            if key.as_ref() == "lua_parent" {
                let path = match value {
                    Value::Nil => String::new(),
                    Value::String(s) => {
                        let path = s.to_str()?.to_string();
                        if path.is_empty() {
                            return Err(err("parent must not be empty; use nil to clear"));
                        }
                        path
                    }
                    _ => return Err(err("parent must be a string or nil")),
                };
                if !path.is_empty() {
                    lua.app_data_ref::<Arc<Sources>>()
                        .ok_or_else(|| err("source registry unavailable"))?
                        .contains_parent(&path)
                        .map_err(err)?;
                }
                // An empty string is a malformed path; only explicit nil clears the parent.
                world.objects.get_mut(&id).unwrap().lua_parent = path;
                return Ok(());
            }
            if (key.as_ref() == "home" && !matches!(kind, Kind::Thing | Kind::Player))
                || (key.as_ref() == "destination" && kind != Kind::Exit)
            {
                return Err(err("relationship is not valid for this object type"));
            }
            let target = if value.is_nil() {
                if key.as_ref() == "home" {
                    return Err(err("home is required"));
                }
                None
            } else {
                Some(identity(value, &world, true)?)
            };
            if let Some(target) = target {
                let target_kind = world.objects[&target].kind;
                if matches!(key.as_ref(), "home" | "destination")
                    && !matches!(target_kind, Kind::Room | Kind::Player | Kind::Thing)
                {
                    return Err(err("target cannot contain objects"));
                }
                if key.as_ref() == "home" {
                    world.validate_move(id, target).map_err(err)?;
                }
                if key.as_ref() == "zone" && !matches!(target_kind, Kind::Room | Kind::Thing) {
                    return Err(err("zone must be a room or thing"));
                }
            }
            let o = world.objects.get_mut(&id).unwrap();
            match key.as_ref() {
                "home" => o.home = target,
                "destination" => o.destination = target,
                "zone" => o.zone = target,
                "affiliation" => o.affiliation = target,
                _ => return Err(err("unknown relationship")),
            }
            Ok(())
        })?,
    )?;
    Ok(())
}
