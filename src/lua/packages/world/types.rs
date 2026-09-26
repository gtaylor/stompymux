//! Immutable native object-kind constants and strict dense type filters.
use crate::world::Kind;
fn err(message: impl ToString) -> mlua::Error {
    super::super::error::failure("mux.arg.invalid", message)
}
use mlua::{MetaMethod, Table, UserData, UserDataMethods, Value};

pub(super) struct Types;
impl UserData for Types {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(
            MetaMethod::NewIndex,
            |_, _, _: (Value, Value)| -> mlua::Result<()> {
                Err(err("mux.world.types constants are immutable"))
            },
        );
        m.add_meta_method(MetaMethod::Index, |lua, _, key: Value| {
            let Value::String(key) = key else {
                return Err(super::super::error::argument_failure(
                    lua,
                    2,
                    "mux.arg.invalid",
                    "object type name must be a string",
                ));
            };
            let bytes = key.as_bytes();
            let key = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
            let kind = match key {
                b"ROOM" => Kind::Room,
                b"THING" => Kind::Thing,
                b"EXIT" => Kind::Exit,
                b"PLAYER" => Kind::Player,
                _ => {
                    return Err(super::super::error::argument_failure(
                        lua,
                        2,
                        "mux.arg.invalid",
                        format!("unknown object type '{}'", String::from_utf8_lossy(key)),
                    ));
                }
            };
            let value = lua.create_userdata(kind)?;
            super::protect_userdata_metatable(
                lua,
                &value,
                "protected object type constant metatable",
            )?;
            Ok(value)
        });
    }
}
impl UserData for Kind {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::ToString, |_, value, ()| {
            Ok(format!("{value:?}").to_uppercase())
        });
        m.add_meta_function(MetaMethod::Eq, |_, (left, right): (Value, Value)| {
            // C's object type __eq luaL_checkudatas both operands
            // (mux_object_type_bindings.c lua_mux_object_type_equal).
            super::super::error::typed_eq::<Self, _>("btmux.object_type", left, right, |a, b| {
                a == b
            })
        });
        m.add_meta_method(
            MetaMethod::NewIndex,
            |_, _, _: (Value, Value)| -> mlua::Result<()> {
                Err(err("mux.world.types constants are immutable"))
            },
        );
    }
}

pub(super) fn kind(value: Value) -> mlua::Result<Kind> {
    let Value::UserData(value) = value else {
        return Err(err("expected typed object kind"));
    };
    Ok(*value
        .borrow::<Kind>()
        .map_err(|_| err("expected typed object kind"))?)
}

pub(super) fn filter(
    lua: &mlua::Lua,
    value: Value,
    argument: usize,
) -> mlua::Result<Option<Vec<Kind>>> {
    if value.is_nil() {
        return Ok(None);
    }
    let Value::Table(table) = value else {
        return Err(super::super::error::argument_failure(
            lua,
            argument,
            "mux.arg.invalid",
            "options.types must be an array",
        ));
    };
    let count = table.raw_len();
    let mut seen = 0;
    for pair in table.clone().pairs::<Value, Value>() {
        let (key, _) = pair?;
        let valid = match key {
            Value::Integer(n) => n >= 1 && n as usize <= count,
            Value::Number(n) => n.fract() == 0.0 && n >= 1.0 && n <= count as f64,
            _ => false,
        };
        if !valid {
            return Err(super::super::error::argument_failure(
                lua,
                argument,
                "mux.arg.invalid",
                "options.types must be a dense array",
            ));
        }
        seen += 1;
    }
    if count != seen {
        return Err(super::super::error::argument_failure(
            lua,
            argument,
            "mux.arg.invalid",
            "options.types must be a dense array",
        ));
    }
    (1..=count)
        .map(|i| kind(table.raw_get(i)?))
        .collect::<mlua::Result<Vec<_>>>()
        .map(Some)
}

pub(super) fn options(
    lua: &mlua::Lua,
    table: &Table,
    argument: usize,
    allowed: &[&str],
) -> mlua::Result<()> {
    for pair in table.clone().pairs::<Value, Value>() {
        let (key, _) = pair?;
        let Value::String(key) = key else {
            return Err(super::super::error::argument_failure(
                lua,
                argument,
                "mux.arg.invalid",
                "unknown options field '<non-string>'",
            ));
        };
        let bytes = key.as_bytes();
        let bytes = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
        let name = String::from_utf8_lossy(bytes);
        if !allowed.contains(&name.as_ref()) {
            return Err(super::super::error::argument_failure(
                lua,
                argument,
                "mux.arg.invalid",
                format!("unknown options field '{name}'"),
            ));
        }
    }
    Ok(())
}
