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
                    "mux.world.flags constants are immutable",
                ))
            },
        );
        methods.add_meta_method(mlua::MetaMethod::Index, |lua, _, key: mlua::Value| {
            let key = lua.coerce_string(key)?.ok_or_else(|| {
                super::super::error::argument_failure(
                    lua,
                    2,
                    "mux.flag.invalid",
                    "constant name must be a string",
                )
            })?;
            let bytes = key.as_bytes();
            let bytes = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
            let name = String::from_utf8_lossy(bytes);
            let flag = Flag::parse(&name).map_err(|_| {
                super::super::error::argument_failure(
                    lua,
                    2,
                    "mux.flag.invalid",
                    format!("unknown flag constant '{name}'"),
                )
            })?;
            if flag.world_name() != name {
                return Err(super::super::error::argument_failure(
                    lua,
                    2,
                    "mux.flag.invalid",
                    format!("unknown flag constant '{name}'"),
                ));
            }
            let value = lua.create_userdata(flag)?;
            super::protect_userdata_metatable(
                lua,
                &value,
                "protected flag or power constant metatable",
            )?;
            Ok(value)
        });
    }
}

impl mlua::UserData for Flag {
    fn add_methods<M: mlua::UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(mlua::MetaMethod::ToString, |_, flag, ()| {
            Ok(flag.world_name())
        });
        methods.add_meta_function(
            mlua::MetaMethod::Eq,
            |_, (left, right): (mlua::Value, mlua::Value)| {
                Ok(
                    named_constant_id(&left)
                        .is_some_and(|id| Some(id) == named_constant_id(&right)),
                )
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

/// Native id C's untyped named-constant `__eq` compares (mux_flag_power_bindings.c
/// lua_mux_constant_equal reads raw `{package, id}` without metatable checks):
/// equal ids compare equal across the same-package families whose second word
/// is an integer — powers, object types (type ordinal), and channel flags
/// (0x100/0x200/0x400 values).
///
/// Both engines are LuaJIT, which only dispatches `__eq` when both operands
/// share one metatable, so cross-family comparisons return false without ever
/// reaching this helper today (namespace_identity.lua pins that behavior).
/// This models the C handler for any engine that dispatches across metatables.
pub(super) fn named_constant_id(value: &mlua::Value) -> Option<i32> {
    let mlua::Value::UserData(userdata) = value else {
        return None;
    };
    if let Ok(flag) = userdata.borrow::<Flag>() {
        return Some(crate::flags::ALL.iter().position(|f| *f == *flag)? as i32 + 1);
    }
    if let Ok(power) = userdata.borrow::<crate::powers::Power>() {
        return Some(crate::powers::ALL.iter().position(|p| *p == *power)? as i32 + 1);
    }
    if let Ok(kind) = userdata.borrow::<crate::world::Kind>() {
        return Some(kind.code() as i32);
    }
    if let Ok(flag) = userdata.borrow::<crate::communication::ChannelFlag>() {
        return Some(flag.bit() as i32);
    }
    None
}
