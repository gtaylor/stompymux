//! Immutable Lua powers constants backed by the shared domain catalog.
use crate::powers::Power;

/// Immutable Lua power namespace, separate from flag identities.
pub(super) struct LuaPowers;

impl mlua::UserData for LuaPowers {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::Index, |lua, _, key: String| {
            let power = Power::parse(&key).map_err(mlua::Error::external)?;
            if power.name() != key {
                return Err(mlua::Error::external(
                    "power constants require canonical uppercase names",
                ));
            }
            lua.create_userdata(power)
        });
    }
}

impl mlua::UserData for Power {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::ToString, |_, power, ()| Ok(power.name()));
        methods.add_meta_method(
            mlua::MetaMethod::Eq,
            |_, power, other: mlua::AnyUserData| {
                Ok(other.borrow::<Power>().is_ok_and(|other| *power == *other))
            },
        );
    }
}
