//! Generation-checked object identities; callers cannot manufacture handles from tables.
use super::super::error::{failure, failure_with_detail};
use crate::{
    runtime::SharedWorld,
    state::Generation,
    world::{Kind, ObjectId},
};
use mlua::{Lua, MetaMethod, Table, UserData, UserDataMethods, Value};

fn lua_type_name(value: Option<&Value>) -> &'static str {
    match value {
        None => "no value",
        Some(Value::Nil) => "nil",
        Some(Value::Boolean(_)) => "boolean",
        Some(Value::LightUserData(_)) => "light userdata",
        Some(Value::Integer(_) | Value::Number(_)) => "number",
        Some(Value::String(_)) => "string",
        Some(Value::Table(_)) => "table",
        Some(Value::Function(_)) => "function",
        Some(Value::Thread(_)) => "thread",
        Some(Value::UserData(_)) => "userdata",
        Some(Value::Error(_)) => "userdata",
        Some(_) => "userdata",
    }
}

pub(super) fn native_type_error(
    argument: usize,
    expected: &str,
    value: Option<&Value>,
) -> mlua::Error {
    mlua::Error::RuntimeError(native_type_error_message(argument, expected, value))
}

fn native_type_error_message(argument: usize, expected: &str, value: Option<&Value>) -> String {
    format!(
        "bad argument #{argument} to '?' ({expected} expected, got {})",
        lua_type_name(value)
    )
}

#[derive(Clone)]
pub(crate) struct Object {
    world: SharedWorld,
    id: ObjectId,
    generation: Generation,
}
impl Object {
    pub(crate) fn id(&self) -> mlua::Result<ObjectId> {
        let w = self.world.borrow();
        if w.objects
            .get(&self.id)
            .is_none_or(|o| o.kind == Kind::Garbage || o.generation != self.generation)
        {
            return Err(failure("mux.object.invalid", "object handle is stale"));
        }
        Ok(self.id)
    }

    /// Report whether this generation-checked handle has lost its object in the supplied world.
    pub(crate) fn is_stale_in(&self, world: &crate::World) -> bool {
        world.objects.get(&self.id).is_none_or(|object| {
            object.kind == Kind::Garbage || object.generation != self.generation
        })
    }
}
impl UserData for Object {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_function(MetaMethod::Eq, |_, (left, right): (Value, Value)| {
            // C's object __eq luaL_checkudatas both operands
            // (mux_object_bindings.c lua_mux_object_equal).
            super::super::error::typed_eq::<Self, _>("btmux.object", left, right, |a, b| {
                std::rc::Rc::ptr_eq(&a.world, &b.world)
                    && a.id == b.id
                    && a.generation == b.generation
            })
        });
        m.add_meta_method(MetaMethod::ToString, |_, o, ()| {
            Ok(format!("object(#{})", o.id()?.0))
        });
        m.add_meta_method(MetaMethod::Index, |lua, _, key: Value| {
            lua.named_registry_value::<Table>("mux.object.methods")?
                .raw_get::<Value>(key)
        });
    }
}
/// Decode a real handle or a Lua-coercible numeric dbref, never a forged table.
pub(crate) fn identity(lua: &Lua, value: Value) -> mlua::Result<ObjectId> {
    match value {
        Value::UserData(u) => u
            .borrow::<Object>()
            .map_err(|_| failure("mux.object.invalid", "expected an Object handle"))?
            .id(),
        value => match lua.coerce_number(value)? {
            Some(n) if n.is_finite() && n >= i64::MIN as f64 && n < 9_223_372_036_854_775_808.0 => {
                Ok(ObjectId(n as i64))
            }
            _ => Err(super::super::error::argument_failure(
                lua,
                1,
                "mux.object.invalid",
                "object must be a dbref or Object",
            )),
        },
    }
}

/// Construct a generation-checked handle for one live object in this runtime.
pub(crate) fn push(lua: &Lua, world: &SharedWorld, id: ObjectId) -> mlua::Result<Value> {
    let generation = world
        .borrow()
        .objects
        .get(&id)
        .filter(|object| object.kind != Kind::Garbage)
        .ok_or_else(|| failure("mux.object.invalid", "object does not exist"))?
        .generation;
    Ok(Value::UserData(lua.create_userdata(Object {
        world: world.clone(),
        id,
        generation,
    })?))
}

pub(super) fn register(lua: &Lua, api: &Table, world: &SharedWorld) -> mlua::Result<()> {
    api.set(
        "object_tostring",
        lua.create_function(|lua, args: mlua::MultiValue| {
            let value = args.front();
            let Some(Value::UserData(value)) = value else {
                return Ok((
                    false,
                    Value::String(lua.create_string(native_type_error_message(
                        1,
                        "btmux.object",
                        value,
                    ))?),
                ));
            };
            let Ok(object) = value.borrow::<Object>() else {
                return Ok((
                    false,
                    Value::String(lua.create_string(native_type_error_message(
                        1,
                        "btmux.object",
                        args.front(),
                    ))?),
                ));
            };
            Ok((
                true,
                Value::String(lua.create_string(format!("object(#{})", object.id()?.0))?),
            ))
        })?,
    )?;
    api.set(
        "object_equal",
        lua.create_function(|lua, args: mlua::MultiValue| {
            let left_value = args.front();
            let Some(Value::UserData(left_value)) = left_value else {
                return Ok((
                    false,
                    Value::String(lua.create_string(native_type_error_message(
                        1,
                        "btmux.object",
                        left_value,
                    ))?),
                ));
            };
            let Ok(left) = left_value.borrow::<Object>() else {
                return Ok((
                    false,
                    Value::String(lua.create_string(native_type_error_message(
                        1,
                        "btmux.object",
                        args.front(),
                    ))?),
                ));
            };
            let right_value = args.get(1);
            let Some(Value::UserData(right_value)) = right_value else {
                return Ok((
                    false,
                    Value::String(lua.create_string(native_type_error_message(
                        2,
                        "btmux.object",
                        right_value,
                    ))?),
                ));
            };
            let Ok(right) = right_value.borrow::<Object>() else {
                return Ok((
                    false,
                    Value::String(lua.create_string(native_type_error_message(
                        2,
                        "btmux.object",
                        args.get(1),
                    ))?),
                ));
            };
            Ok((
                true,
                Value::Boolean(
                    std::rc::Rc::ptr_eq(&left.world, &right.world)
                        && left.id == right.id
                        && left.generation == right.generation,
                ),
            ))
        })?,
    )?;
    for (name, powers) in [("flag_set", false), ("power_set", true)] {
        api.set(
            name,
            lua.create_function(move |lua, value: mlua::AnyUserData| {
                let owner = value.borrow::<Object>()?.clone();
                owner.id()?;
                lua.create_userdata(Set { owner, powers })
            })?,
        )?;
    }
    let w = world.clone();
    api.set(
        "object",
        lua.create_function(move |lua, v: Value| {
            if lua
                .app_data_ref::<crate::lua::RuntimeMode>()
                .is_some_and(|m| *m == crate::lua::RuntimeMode::Checking)
            {
                return Err(failure(
                    "mux.unavailable.checking",
                    "world is unavailable while checking",
                ));
            }
            let id = identity(lua, v)?;
            push(lua, &w, id)
        })?,
    )?;
    api.set(
        "object_id",
        lua.create_function(|lua, v: Value| Ok(identity(lua, v)?.0))?,
    )?;
    api.set(
        "object_methods",
        lua.create_function(|lua, methods: Table| {
            lua.set_named_registry_value("mux.object.methods", methods)
        })?,
    )?;
    Ok(())
}

/// Validate under an existing world borrow, avoiding nested RefCell borrowing during mutations.
pub(crate) fn identity_in(
    lua: &Lua,
    value: Value,
    world: &crate::world::World,
) -> mlua::Result<ObjectId> {
    if let Value::UserData(u) = value {
        let h = u
            .borrow::<Object>()
            .map_err(|_| failure("mux.object.invalid", "expected an Object handle"))?;
        if world
            .objects
            .get(&h.id)
            .is_none_or(|o| o.kind == Kind::Garbage || o.generation != h.generation)
        {
            return Err(failure("mux.object.invalid", "object handle is stale"));
        }
        Ok(h.id)
    } else {
        identity(lua, value)
    }
}

/// Decode a C-contract object argument with its public argument number and label.
pub(crate) fn identity_at(
    lua: &Lua,
    value: Value,
    world: &crate::world::World,
    argument: usize,
    label: &str,
) -> mlua::Result<ObjectId> {
    let invalid = |message: String| {
        failure_with_detail(
            "mux.object.invalid",
            message,
            serde_json::json!({ "argument": argument }),
        )
    };
    let id = if let Value::UserData(userdata) = value {
        let handle = userdata
            .borrow::<Object>()
            .map_err(|_| invalid(format!("{label} must be a dbref or Object")))?;
        let same_runtime = std::ptr::eq(handle.world.as_ptr().cast_const(), world as *const _);
        if !same_runtime {
            return Err(invalid(format!("{label} belongs to another Lua runtime")));
        }
        if world.objects.get(&handle.id).is_none_or(|object| {
            object.kind == Kind::Garbage || object.generation != handle.generation
        }) {
            return Err(invalid(format!("{label} no longer exists")));
        }
        handle.id
    } else {
        let number = lua
            .coerce_number(value)?
            .ok_or_else(|| invalid(format!("{label} must be a dbref or Object")))?;
        // LuaJIT's lua_tointeger truncates finite values toward zero. Values it cannot
        // represent (including NaN and infinities) produce LONG_MIN, which is not a dbref.
        if !number.is_finite() || number <= i64::MIN as f64 || number >= i64::MAX as f64 {
            return Err(invalid(format!("{label} is invalid")));
        }
        ObjectId(number as i64)
    };
    if world
        .objects
        .get(&id)
        .is_none_or(|object| object.kind == Kind::Garbage)
    {
        return Err(invalid(format!("{label} is invalid")));
    }
    Ok(id)
}

/// Flag and power collections carry their owning object's generation, not a bare dbref.
struct Set {
    owner: Object,
    powers: bool,
}
impl Set {
    fn change(&self, lua: &Lua, value: Value, add: bool) -> mlua::Result<bool> {
        if lua
            .app_data_ref::<crate::lua::RuntimeMode>()
            .is_some_and(|m| *m == crate::lua::RuntimeMode::Checking)
        {
            return Err(failure(
                "mux.unavailable.checking",
                "flags and powers are unavailable while checking",
            ));
        }
        let id = self.owner.id()?;
        let mut w = self.owner.world.borrow_mut();
        if self.powers {
            let Value::UserData(value) = value else {
                return Err(super::super::error::argument_failure(
                    lua,
                    2,
                    "mux.power.invalid",
                    "expected a mux.world.powers constant",
                ));
            };
            let power = *value.borrow::<crate::powers::Power>().map_err(|_| {
                super::super::error::argument_failure(
                    lua,
                    2,
                    "mux.power.invalid",
                    "expected a mux.world.powers constant",
                )
            })?;
            crate::powers::change(&mut w, ObjectId(1), id, power, add)
                .map_err(|e| failure("mux.power.invalid", e))
        } else {
            let Value::UserData(value) = value else {
                return Err(super::super::error::argument_failure(
                    lua,
                    2,
                    "mux.flag.invalid",
                    "expected a mux.world.flags constant",
                ));
            };
            let flag = *value.borrow::<crate::flags::Flag>().map_err(|_| {
                super::super::error::argument_failure(
                    lua,
                    2,
                    "mux.flag.invalid",
                    "expected a mux.world.flags constant",
                )
            })?;
            crate::flags::change(&mut w, ObjectId(1), id, flag, add)
                .map_err(|e| failure("mux.flag.invalid", e))
        }
    }
}
impl UserData for Set {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::ToString, |_, s, ()| {
            let id = s.owner.id()?;
            Ok(format!(
                "{}(#{})",
                if s.powers { "powers" } else { "flags" },
                id.0
            ))
        });
        m.add_function("__tostring", |_, args: mlua::MultiValue| {
            let value = args.front();
            let Some(Value::UserData(value)) = value else {
                return Err(native_type_error(1, "btmux.object_flags", value));
            };
            let set = value
                .borrow::<Set>()
                .map_err(|_| native_type_error(1, "btmux.object_flags", args.front()))?;
            let id = set.owner.id()?;
            Ok(format!(
                "{}(#{})",
                if set.powers { "powers" } else { "flags" },
                id.0
            ))
        });
        m.add_method("list", |lua, s, ()| {
            let id = s.owner.id()?;
            let w = s.owner.world.borrow();
            if s.powers {
                lua.create_sequence_from(
                    crate::powers::ALL
                        .into_iter()
                        .filter(|p| w.objects[&id].powers.contains(*p)),
                )
            } else {
                lua.create_sequence_from(
                    crate::flags::ALL
                        .into_iter()
                        .filter(|p| w.objects[&id].flags.contains(*p)),
                )
            }
        });
        m.add_method("has", |lua, s, v: Value| {
            let id = s.owner.id()?;
            let w = s.owner.world.borrow();
            if s.powers {
                let Value::UserData(v) = v else {
                    return Err(super::super::error::argument_failure(
                        lua,
                        2,
                        "mux.power.invalid",
                        "expected a mux.world.powers constant",
                    ));
                };
                Ok(w.objects[&id]
                    .powers
                    .contains(*v.borrow::<crate::powers::Power>().map_err(|_| {
                        super::super::error::argument_failure(
                            lua,
                            2,
                            "mux.power.invalid",
                            "expected a mux.world.powers constant",
                        )
                    })?))
            } else {
                let Value::UserData(v) = v else {
                    return Err(super::super::error::argument_failure(
                        lua,
                        2,
                        "mux.flag.invalid",
                        "expected a mux.world.flags constant",
                    ));
                };
                Ok(w.objects[&id]
                    .flags
                    .contains(*v.borrow::<crate::flags::Flag>().map_err(|_| {
                        super::super::error::argument_failure(
                            lua,
                            2,
                            "mux.flag.invalid",
                            "expected a mux.world.flags constant",
                        )
                    })?))
            }
        });
        m.add_method("add", |lua, s, v: Value| s.change(lua, v, true));
        m.add_method("remove", |lua, s, v: Value| s.change(lua, v, false));
    }
}
