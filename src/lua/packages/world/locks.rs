//! Checked lock results retain denial metadata and roll back malformed callback results.
use crate::lua::{Outbox, SharedWorld, err, transactions};
use mlua::{Function, Lua, Table, Value};

/// C LBUF_SIZE bounds optional lock messages, excluding their terminator.
const MESSAGE_LIMIT: usize = 8192;

/// Install a private lock invoker; public Lua callers still receive a boolean.
pub(super) fn register(
    lua: &Lua,
    api: &Table,
    world: &SharedWorld,
    outbox: &Outbox,
) -> mlua::Result<()> {
    api.set(
        "lock_error",
        lua.create_function(|lua, message: String| {
            crate::lua::configuration(lua).log(
                &[crate::logging::Category::Bugs],
                "LUA",
                "ERROR",
                format!("Lua lock failed: {message}"),
            );
            Ok(())
        })?,
    )?;
    api.set(
        "callback_descriptor",
        lua.create_function(|lua, ()| Ok(crate::lua::transactions::descriptor(lua)))?,
    )?;
    api.set("locks", lua.create_userdata(LockNamespace)?)?;
    api.set(
        "lock_key",
        lua.create_function(|_, value: mlua::AnyUserData| {
            Ok(value.borrow::<crate::LockType>()?.key())
        })?,
    )?;
    let parents = world.clone();
    api.set(
        "lock_parent",
        lua.create_function(move |_, n: i64| {
            let w = parents.borrow();
            let o = w
                .objects
                .get(&crate::world::ObjectId(n))
                .ok_or_else(|| err("invalid lock object"))?;
            Ok(if o.lua_parent.is_empty() {
                None
            } else {
                Some(o.lua_parent.clone())
            })
        })?,
    )?;
    let identities = world.clone();
    api.set(
        "lock_identity",
        lua.create_function(move |_, value: Value| {
            let n = match value {
                Value::Integer(n) => n,
                Value::Number(n)
                    if n.is_finite() && n.fract() == 0.0 && n >= 0.0 && n < i64::MAX as f64 =>
                {
                    n as i64
                }
                _ => return Err(err("lock identity must be an integer dbref or object")),
            };
            if !identities
                .borrow()
                .objects
                .get(&crate::world::ObjectId(n))
                .is_some_and(|o| o.kind != crate::world::Kind::Garbage)
            {
                return Err(err("invalid lock object"));
            }
            Ok(n)
        })?,
    )?;
    let w = world.clone();
    let out = outbox.clone();
    api.set(
        "evaluate_lock",
        lua.create_function(move |lua, (f, ctx): (Function, Table)| {
            transactions::run(lua, &w, &out, || {
                let value = f.call::<Value>(ctx)?;
                let result = lua.create_table()?;
                match value {
                    Value::Boolean(v) => result.set("passes", v)?,
                    Value::Table(t) => {
                        for pair in t.pairs::<Value, Value>() {
                            let (key, _) = pair?;
                            match key {
                                Value::String(k)
                                    if matches!(
                                        k.to_str()?.as_ref(),
                                        "passes" | "enactor_message" | "other_message"
                                    ) => {}
                                _ => return Err(err("invalid lock result field")),
                            }
                        }
                        let Value::Boolean(passes) = t.get::<Value>("passes")? else {
                            return Err(err("lock result passes must be boolean"));
                        };
                        result.set("passes", passes)?;
                        for key in ["enactor_message", "other_message"] {
                            match t.get::<Value>(key)? {
                                Value::Nil => {}
                                Value::String(v) if v.as_bytes().len() < MESSAGE_LIMIT => {
                                    v.to_str()?;
                                    result.set(key, v)?;
                                }
                                _ => {
                                    return Err(err(
                                        "lock message must be a string shorter than 8192 bytes",
                                    ));
                                }
                            }
                        }
                    }
                    _ => return Err(err("lock must return a boolean or result table")),
                }
                Ok(result)
            })
        })?,
    )
}

/// Opaque constants cannot be changed with ordinary assignment or rawset.
struct LockNamespace;
impl mlua::UserData for LockNamespace {
    fn add_methods<M: mlua::UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(mlua::MetaMethod::Index, |lua, _, value: Value| {
            let Value::String(name) = value else {
                return Err(err("lock name must be a string"));
            };
            let name = name.to_str()?;
            let lock = crate::locks::LOCKS
                .into_iter()
                .find(|k| k.name() == name.as_ref())
                .ok_or_else(|| err("unknown lock constant"))?;
            lua.create_userdata(lock)
        });
    }
}
impl mlua::UserData for crate::LockType {
    fn add_methods<M: mlua::UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(mlua::MetaMethod::ToString, |_, lock, ()| Ok(lock.name()));
        m.add_meta_method(mlua::MetaMethod::Eq, |_, lock, other: mlua::AnyUserData| {
            Ok(other.borrow::<Self>().is_ok_and(|v| *lock == *v))
        });
    }
}
