//! Checked lock results retain denial metadata and roll back malformed callback results.
use crate::{
    lua::{err, transactions},
    runtime::SharedWorld,
};
use mlua::{Function, Lua, Table, Value};

/// C LBUF_SIZE bounds optional lock messages, excluding their terminator.
const MESSAGE_LIMIT: usize = 8192;

/// Install a private lock invoker; public Lua callers still receive a boolean.
pub(super) fn register(lua: &Lua, api: &Table, world: &SharedWorld) -> mlua::Result<()> {
    api.set(
        "lock_error",
        lua.create_function(|_, message: String| {
            tracing::error!("Lua lock failed: {message}");
            Ok(())
        })?,
    )?;
    api.set(
        "callback_descriptor",
        lua.create_function(|lua, ()| Ok(crate::lua::transactions::descriptor(lua)))?,
    )?;
    let locks = lua.create_userdata(LockNamespace)?;
    super::protect_userdata_metatable(lua, &locks, "protected lock namespace metatable")?;
    api.set("locks", locks)?;
    api.set(
        "lock_key",
        lua.create_function(|_, value: mlua::AnyUserData| {
            Ok(value
                .borrow::<crate::LockType>()
                .map_err(|_| {
                    super::super::error::failure("mux.access.invalid", "expected a typed lock")
                })?
                .key())
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
    api.set(
        "evaluate_lock",
        lua.create_function(move |lua, (f, ctx): (Function, Table)| {
            transactions::run(lua, &w, || {
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
            let name = lua.coerce_string(value)?.ok_or_else(|| {
                super::super::error::argument_failure(
                    lua,
                    2,
                    "mux.arg.invalid",
                    "lock name must be a string",
                )
            })?;
            let bytes = name.as_bytes();
            let bytes = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
            let name = String::from_utf8_lossy(bytes);
            let lock = crate::locks::LOCKS
                .into_iter()
                .find(|k| k.name() == name.as_ref())
                .ok_or_else(|| {
                    super::super::error::argument_failure(
                        lua,
                        2,
                        "mux.arg.invalid",
                        format!("unknown lock '{name}'"),
                    )
                })?;
            let value = lua.create_userdata(lock)?;
            super::protect_userdata_metatable(lua, &value, "protected lock constant metatable")?;
            Ok(value)
        });
        m.add_meta_method(
            mlua::MetaMethod::NewIndex,
            |_, _, _: (Value, Value)| -> mlua::Result<()> {
                Err(super::super::error::failure(
                    "mux.arg.invalid",
                    "mux.world.locks constants are immutable",
                ))
            },
        );
    }
}
impl mlua::UserData for crate::LockType {
    fn add_methods<M: mlua::UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(mlua::MetaMethod::ToString, |_, lock, ()| Ok(lock.name()));
        m.add_meta_function(mlua::MetaMethod::Eq, |_, (left, right): (Value, Value)| {
            // C's lock __eq luaL_checkudatas both operands (mux_lock_bindings.c).
            super::super::error::typed_eq::<Self, _>("btmux.lock", left, right, |a, b| a == b)
        });
        m.add_meta_method(
            mlua::MetaMethod::NewIndex,
            |_, _, _: (Value, Value)| -> mlua::Result<()> {
                Err(super::super::error::failure(
                    "mux.arg.invalid",
                    "mux.world.locks constants are immutable",
                ))
            },
        );
    }
}
