//! Immutable Lua powers constants backed by the shared domain catalog.
use crate::powers::Power;

/// Immutable Lua power namespace, separate from flag identities.
pub(super) struct LuaPowers;

impl mlua::UserData for LuaPowers {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(
            mlua::MetaMethod::NewIndex,
            |_, _, _: (mlua::Value, mlua::Value)| -> mlua::Result<()> {
                Err(super::super::error::failure(
                    "mux.power.invalid",
                    "mux.world.powers constants are immutable",
                ))
            },
        );
        methods.add_meta_method(mlua::MetaMethod::Index, |lua, _, key: mlua::Value| {
            let key = lua.coerce_string(key)?.ok_or_else(|| {
                super::super::error::argument_failure(
                    lua,
                    2,
                    "mux.power.invalid",
                    "constant name must be a string",
                )
            })?;
            let bytes = key.as_bytes();
            let bytes = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
            let name = String::from_utf8_lossy(bytes);
            let power = Power::parse(&name).map_err(|_| {
                super::super::error::argument_failure(
                    lua,
                    2,
                    "mux.power.invalid",
                    format!("unknown power constant '{name}'"),
                )
            })?;
            if power.name() != name {
                return Err(super::super::error::argument_failure(
                    lua,
                    2,
                    "mux.power.invalid",
                    format!("unknown power constant '{name}'"),
                ));
            }
            let value = lua.create_userdata(power)?;
            super::protect_userdata_metatable(
                lua,
                &value,
                "protected flag or power constant metatable",
            )?;
            Ok(value)
        });
    }
}

impl mlua::UserData for Power {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::ToString, |_, power, ()| Ok(power.name()));
        methods.add_meta_function(
            mlua::MetaMethod::Eq,
            |_, (left, right): (mlua::Value, mlua::Value)| {
                // Same untyped native comparison as flags (lua_mux_constant_equal).
                Ok(super::flags::named_constant_id(&left)
                    .is_some_and(|id| Some(id) == super::flags::named_constant_id(&right)))
            },
        );
        methods.add_meta_method(
            mlua::MetaMethod::NewIndex,
            |_, _, _: (mlua::Value, mlua::Value)| -> mlua::Result<()> {
                Err(super::super::error::failure(
                    "mux.arg.invalid",
                    "flag and power constants are immutable",
                ))
            },
        );
    }
}
