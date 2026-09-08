//! Generation-checked object identities; callers cannot manufacture handles from tables.
use super::super::error::failure;
use crate::{
    runtime::SharedWorld,
    state::Generation,
    world::{Kind, ObjectId},
};
use mlua::{Lua, MetaMethod, Table, UserData, UserDataMethods, Value};

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
}
impl UserData for Object {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Eq, |_, a, b: mlua::AnyUserData| {
            Ok(b.borrow::<Object>().is_ok_and(|b| {
                std::rc::Rc::ptr_eq(&a.world, &b.world)
                    && a.id == b.id
                    && a.generation == b.generation
            }))
        });
        m.add_meta_method(MetaMethod::ToString, |_, o, ()| {
            Ok(format!("object(#{})", o.id()?.0))
        });
        m.add_meta_method(MetaMethod::Index, |lua, _, key: String| {
            lua.named_registry_value::<Table>("mux.object.methods")?
                .get::<Value>(key)
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
            _ => Err(failure(
                "mux.object.invalid",
                "expected a finite in-range numeric dbref",
            )),
        },
    }
}
pub(super) fn register(lua: &Lua, api: &Table, world: &SharedWorld) -> mlua::Result<()> {
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
            let generation = w
                .borrow()
                .objects
                .get(&id)
                .filter(|o| o.kind != Kind::Garbage)
                .ok_or_else(|| failure("mux.object.invalid", "object does not exist"))?
                .generation;
            lua.create_userdata(Object {
                world: w.clone(),
                id,
                generation,
            })
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

/// Flag and power collections carry their owning object's generation, not a bare dbref.
struct Set {
    owner: Object,
    powers: bool,
}
impl Set {
    fn change(&self, lua: &Lua, value: mlua::AnyUserData, add: bool) -> mlua::Result<bool> {
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
            let power = *value
                .borrow::<crate::powers::Power>()
                .map_err(|_| failure("mux.power.invalid", "expected a typed Power"))?;
            crate::powers::change(&mut w, ObjectId(1), id, power, add)
                .map_err(|e| failure("mux.power.invalid", e))
        } else {
            let flag = *value
                .borrow::<crate::flags::Flag>()
                .map_err(|_| failure("mux.flag.invalid", "expected a typed Flag"))?;
            crate::flags::change(&mut w, ObjectId(1), id, flag, add)
                .map_err(|e| failure("mux.flag.invalid", e))
        }
    }
}
impl UserData for Set {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
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
        m.add_method("has", |_, s, v: mlua::AnyUserData| {
            let id = s.owner.id()?;
            let w = s.owner.world.borrow();
            if s.powers {
                Ok(w.objects[&id].powers.contains(
                    *v.borrow::<crate::powers::Power>()
                        .map_err(|_| failure("mux.power.invalid", "expected a typed Power"))?,
                ))
            } else {
                Ok(w.objects[&id].flags.contains(
                    *v.borrow::<crate::flags::Flag>()
                        .map_err(|_| failure("mux.flag.invalid", "expected a typed Flag"))?,
                ))
            }
        });
        m.add_method("add", |lua, s, v: mlua::AnyUserData| s.change(lua, v, true));
        m.add_method("remove", |lua, s, v: mlua::AnyUserData| {
            s.change(lua, v, false)
        });
    }
}
