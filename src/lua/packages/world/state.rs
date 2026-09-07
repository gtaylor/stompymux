//! Immutable object-state handles and explicit, binary-safe Lua scalar conversion.
use crate::{
    config::Config,
    lua::{SharedWorld, transactions},
    state::{self, Generation},
    world::{Kind, ObjectId},
};
use mlua::{Lua, MetaMethod, Table, UserData, UserDataMethods, Value};

fn err(e: impl std::fmt::Display) -> mlua::Error {
    super::super::error::failure("mux.state.invalid", e)
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
        .map_err(err)
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
fn checked_value(lua: &Lua, v: Value) -> mlua::Result<state::Value> {
    let v = from_lua(v)?;
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
        m.add_method("get", |lua, h, (key, default): (String, Value)| {
            match h.get(&key)? {
                Some(v) => to_lua(lua, v),
                None => Ok(default),
            }
        });
        m.add_method("has", |_, h, key: String| Ok(h.get(&key)?.is_some()));
        m.add_method("set", |lua, h, (key, value): (String, Value)| {
            let value = if value.is_nil() {
                None
            } else {
                Some(checked_value(lua, value)?)
            };
            h.change(lua, |s| state::set(s, &h.namespace, &key, value))
        });
        m.add_method("delete", |lua, h, key: String| {
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
                let key: String = keys.raw_get(index)?;
                if let Some(v) = h.get(&key)? {
                    result.set(key, to_lua(lua, v)?)?;
                }
            }
            Ok(result)
        });
        m.add_method("set_many", |lua, h, values: Table| {
            let mut batch = Vec::new();
            for pair in values.pairs::<String, Value>() {
                let (key, v) = pair?;
                state::address(&h.namespace, Some(&key)).map_err(err)?;
                batch.push((key, from_lua(v)?));
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
    }
}
/// Install the constructor; all subsequent operations validate the captured incarnation.
pub(super) fn register(lua: &Lua, api: &Table, _c: &Config, w: &SharedWorld) -> mlua::Result<()> {
    let world = w.clone();
    api.set(
        "state",
        lua.create_function(move |lua, (id, namespace): (i64, String)| {
            state::address(&namespace, None).map_err(err)?;
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
