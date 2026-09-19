//! Immutable object-state handles and explicit, binary-safe Lua scalar conversion.
use crate::{
    config::Config,
    lua::transactions,
    runtime::SharedWorld,
    state::{self, Generation},
    world::{Kind, ObjectId},
};
use mlua::{Lua, MetaMethod, MultiValue, Table, UserData, UserDataMethods, Value};

fn err(e: impl std::fmt::Display) -> mlua::Error {
    super::super::error::failure("mux.state.invalid", e)
}

fn checked_name(lua: &Lua, value: Value, argument: usize, namespace: bool) -> mlua::Result<String> {
    let value = lua.coerce_string(value)?.ok_or_else(|| {
        super::super::error::argument_failure(
            lua,
            argument,
            "mux.state.invalid",
            if namespace {
                "invalid state namespace"
            } else {
                "invalid state key"
            },
        )
    })?;
    let bytes = value.as_bytes();
    if bytes.contains(&0) || !bytes.is_ascii() {
        return Err(super::super::error::argument_failure(
            lua,
            argument,
            "mux.state.invalid",
            if namespace {
                "invalid state namespace"
            } else {
                "invalid state key"
            },
        ));
    }
    let name = std::str::from_utf8(bytes.as_ref())
        .expect("ASCII is valid UTF-8")
        .to_owned();
    (if namespace {
        state::address(&name, None)
    } else {
        // Use a known-valid namespace while applying the shared key rules.
        state::address("state", Some(&name))
    })
    .map_err(|_| {
        super::super::error::argument_failure(
            lua,
            argument,
            "mux.state.invalid",
            if namespace {
                "invalid state namespace"
            } else {
                "invalid state key"
            },
        )
    })?;
    Ok(name)
}

/// A handle is tied to an incarnation, not just a recyclable provisional dbref.
struct Handle {
    world: SharedWorld,
    object: ObjectId,
    generation: Generation,
    namespace: String,
}
impl Handle {
    /// Reject deleted, tombstoned and rolled-back provisional objects.
    fn check(&self) -> mlua::Result<()> {
        if self
            .world
            .borrow()
            .objects
            .get(&self.object)
            .is_some_and(|o| o.kind != Kind::Garbage && o.generation == self.generation)
        {
            Ok(())
        } else {
            Err(super::super::error::failure(
                "mux.object.invalid",
                "invalid state object",
            ))
        }
    }
    /// Validate even absent read keys.
    fn get(&self, key: &str) -> mlua::Result<Option<state::Value>> {
        self.check()?;
        state::address(&self.namespace, Some(key)).map_err(err)?;
        Ok(self.world.borrow().objects[&self.object]
            .state
            .get(&self.namespace)
            .and_then(|v| v.get(key))
            .cloned())
    }
    /// Publish a fully validated candidate, never an intermediate batch.
    fn change<T>(
        &self,
        lua: &Lua,
        work: impl FnOnce(&mut state::State) -> anyhow::Result<T>,
    ) -> mlua::Result<T> {
        self.check()?;
        transactions::require(lua)?;
        state::change(
            &mut self.world.borrow_mut(),
            &crate::lua::configuration(lua),
            self.object,
            work,
        )
        .map_err(|e| super::super::error::failure("mux.state.value_too_large", e))
    }
}

/// Convert stored strings without UTF-8 decoding or serde's tagged representation.
fn to_lua(lua: &Lua, v: state::Value) -> mlua::Result<Value> {
    Ok(match v {
        state::Value::Boolean(v) => Value::Boolean(v),
        state::Value::Integer(v) => Value::Integer(v),
        state::Value::Number(v) => Value::Number(v),
        state::Value::String(v) => Value::String(lua.create_string(v)?),
    })
}
/// Match LuaJIT's integer classification while rejecting unsupported and nonfinite values.
fn from_lua(v: Value) -> mlua::Result<state::Value> {
    Ok(match v {
        Value::Boolean(v) => state::Value::Boolean(v),
        Value::Integer(v) => state::Value::Integer(v),
        Value::Number(v) if v.is_finite() => {
            if v.fract() == 0.0 && v >= i64::MIN as f64 && v < 9223372036854775808.0 {
                state::Value::Integer(v as i64)
            } else {
                state::Value::Number(v)
            }
        }
        Value::String(v) => state::Value::String(v.as_bytes().to_vec()),
        _ => {
            return Err(err(
                "state value must be a string, boolean or finite number",
            ));
        }
    })
}
fn checked_value(lua: &Lua, v: Value, argument: Option<usize>) -> mlua::Result<state::Value> {
    let v = from_lua(v).map_err(|_| {
        if let Some(argument) = argument {
            super::super::error::argument_failure(
                lua,
                argument,
                "mux.state.invalid",
                "state values must be strings, booleans, or finite numbers",
            )
        } else {
            super::super::error::failure(
                "mux.state.invalid",
                "state values must be strings, booleans, or finite numbers",
            )
        }
    })?;
    if v.bytes() > crate::lua::configuration(lua).lua.state_value_limit {
        return Err(super::super::error::failure(
            "mux.state.value_too_large",
            "state value exceeds configured byte limit",
        ));
    }
    Ok(v)
}
impl UserData for Handle {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_method("get", |lua, h, arguments: MultiValue| {
            let key = checked_name(
                lua,
                arguments.front().cloned().unwrap_or(Value::Nil),
                2,
                false,
            )?;
            let default = arguments.get(1).cloned().unwrap_or(Value::Nil);
            match h.get(&key)? {
                Some(v) => to_lua(lua, v),
                None => Ok(default),
            }
        });
        m.add_method("has", |lua, h, key: Value| {
            let key = checked_name(lua, key, 2, false)?;
            Ok(h.get(&key)?.is_some())
        });
        m.add_method("set", |lua, h, arguments: MultiValue| {
            let key = checked_name(
                lua,
                arguments.front().cloned().unwrap_or(Value::Nil),
                2,
                false,
            )?;
            let Some(value) = arguments.get(1).cloned() else {
                return Err(super::super::error::argument_failure(
                    lua,
                    3,
                    "mux.state.invalid",
                    "state values must be strings, booleans, or finite numbers",
                ));
            };
            let value = if value.is_nil() {
                None
            } else {
                Some(checked_value(lua, value, Some(3))?)
            };
            h.change(lua, |s| state::set(s, &h.namespace, &key, value))
        });
        m.add_method("delete", |lua, h, key: Value| {
            let key = checked_name(lua, key, 2, false)?;
            let existed = h.get(&key)?.is_some();
            h.change(lua, |s| state::set(s, &h.namespace, &key, None))?;
            Ok(existed)
        });
        m.add_method("keys", |lua, h, (): ()| {
            h.check()?;
            transactions::require(lua)?;
            // Release the world borrow before allocating Lua values (which may run GC).
            let values = h.world.borrow().objects[&h.object]
                .state
                .get(&h.namespace)
                .cloned();
            let t = lua.create_table()?;
            if let Some(values) = values {
                for (i, key) in values.keys().enumerate() {
                    t.set(i + 1, key.clone())?;
                }
            }
            Ok(t)
        });
        m.add_method("entries", |lua, h, (): ()| {
            h.check()?;
            transactions::require(lua)?;
            // Release the world borrow before allocating Lua values (which may run GC).
            let values = h.world.borrow().objects[&h.object]
                .state
                .get(&h.namespace)
                .cloned();
            let t = lua.create_table()?;
            if let Some(values) = values {
                for (i, (key, value)) in values.iter().enumerate() {
                    let entry = lua.create_table()?;
                    entry.set("key", key.clone())?;
                    entry.set("value", to_lua(lua, value.clone())?)?;
                    t.set(i + 1, entry)?;
                }
            }
            Ok(t)
        });
        m.add_method("get_many", |lua, h, keys: Table| {
            h.check()?;
            let result = lua.create_table()?;
            for index in 1..=keys.raw_len() {
                let key = checked_name(lua, keys.raw_get(index)?, 2, false)?;
                if let Some(v) = h.get(&key)? {
                    result.set(key, to_lua(lua, v)?)?;
                }
            }
            Ok(result)
        });
        m.add_method("set_many", |lua, h, values: Table| {
            let mut batch = Vec::new();
            for pair in values.pairs::<Value, Value>() {
                let (key, v) = pair?;
                let Value::String(key) = key else {
                    return Err(super::super::error::failure(
                        "mux.state.invalid",
                        "state update keys must be strings",
                    ));
                };
                let key = checked_name(lua, Value::String(key), 2, false)?;
                batch.push((key, checked_value(lua, v, None)?));
            }
            h.change(lua, |s| {
                for (key, v) in batch {
                    state::set(s, &h.namespace, &key, Some(v))?;
                }
                Ok(())
            })
        });
        m.add_meta_method(MetaMethod::ToString, |_, h, (): ()| {
            h.check()?;
            Ok(format!("state(#{}, {})", h.object.0, h.namespace))
        });
        m.add_function("__tostring", |_, args: MultiValue| {
            let value = args.get(0);
            let Some(Value::UserData(value)) = value else {
                return Err(super::handles::native_type_error(
                    1,
                    "btmux.object_state",
                    value,
                ));
            };
            let handle = value.borrow::<Handle>().map_err(|_| {
                super::handles::native_type_error(1, "btmux.object_state", args.get(0))
            })?;
            handle.check()?;
            Ok(format!("state(#{}, {})", handle.object.0, handle.namespace))
        });
    }
}
/// Install the constructor; all subsequent operations validate the captured incarnation.
pub(super) fn register(lua: &Lua, api: &Table, _c: &Config, w: &SharedWorld) -> mlua::Result<()> {
    let world = w.clone();
    api.set(
        "state",
        lua.create_function(move |lua, (id, namespace): (i64, Value)| {
            let namespace = checked_name(lua, namespace, 2, true)?;
            let object = ObjectId(id);
            let generation = world
                .borrow()
                .objects
                .get(&object)
                .filter(|o| o.kind != Kind::Garbage)
                .ok_or_else(|| err("invalid state object"))?
                .generation;
            lua.create_userdata(Handle {
                world: world.clone(),
                object,
                generation,
                namespace,
            })
        })?,
    )
}
