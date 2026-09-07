//! Immutable Lua flags constants backed by the shared domain catalog.
use crate::flags::Flag;

/// Immutable Lua catalog namespace; aliases are a command/configuration concern.
pub(super) struct LuaFlags;

impl mlua::UserData for LuaFlags {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(
            mlua::MetaMethod::NewIndex,
            |_, _, _: (mlua::Value, mlua::Value)| -> mlua::Result<()> {
                Err(super::super::error::failure(
                    "mux.flag.invalid",
                    "constants are immutable",
                ))
            },
        );
        methods.add_meta_method(mlua::MetaMethod::Index, |lua, _, key: String| {
            let flag = Flag::parse(&key)
                .map_err(|e| super::super::error::failure("mux.flag.invalid", e))?;
            if flag.world_name() != key {
                return Err(super::super::error::failure(
                    "mux.flag.invalid",
                    "flag constants require canonical uppercase names",
                ));
            }
            lua.create_userdata(flag)
        });
    }
}

impl mlua::UserData for Flag {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::ToString, |_, flag, ()| {
            Ok(flag.world_name())
        });
        methods.add_meta_method(mlua::MetaMethod::Eq, |_, flag, other: mlua::AnyUserData| {
            Ok(other.borrow::<Flag>().is_ok_and(|other| *flag == *other))
        });
    }
}
