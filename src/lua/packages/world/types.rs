//! Immutable native object-kind constants and strict dense type filters.
use crate::world::Kind;
fn err(message: impl ToString) -> mlua::Error {
    super::super::error::failure("mux.arg.invalid", message)
}
use mlua::{AnyUserData, MetaMethod, Table, UserData, UserDataMethods, Value};

pub(super) struct Types;
impl UserData for Types {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(
            MetaMethod::NewIndex,
            |_, _, _: (Value, Value)| -> mlua::Result<()> {
                Err(err("object types are immutable"))
            },
        );
        m.add_meta_method(MetaMethod::Index, |lua, _, key: String| {
            let kind = match key.as_str() {
                "ROOM" => Kind::Room,
                "THING" => Kind::Thing,
                "EXIT" => Kind::Exit,
                "PLAYER" => Kind::Player,
                _ => return Err(err("unknown object type constant")),
            };
            lua.create_userdata(kind)
        });
    }
}
impl UserData for Kind {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::ToString, |_, value, ()| {
            Ok(format!("{value:?}").to_uppercase())
        });
        m.add_meta_method(MetaMethod::Eq, |_, value, other: AnyUserData| {
            Ok(other.borrow::<Kind>().is_ok_and(|other| *value == *other))
        });
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

pub(super) fn filter(value: Value) -> mlua::Result<Option<Vec<Kind>>> {
    if value.is_nil() {
        return Ok(None);
    }
    let Value::Table(table) = value else {
        return Err(err("types must be a dense array"));
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
            return Err(err("types must be a dense array"));
        }
        seen += 1;
    }
    if count != seen {
        return Err(err("types must be a dense array"));
    }
    (1..=count)
        .map(|i| kind(table.raw_get(i)?))
        .collect::<mlua::Result<Vec<_>>>()
        .map(Some)
}

pub(super) fn options(table: &Table, allowed: &[&str]) -> mlua::Result<()> {
    for pair in table.clone().pairs::<Value, Value>() {
        let (key, _) = pair?;
        let Value::String(key) = key else {
            return Err(super::super::error::failure(
                "mux.arg.invalid",
                "option names must be strings",
            ));
        };
        if !allowed.contains(&key.to_str()?.as_ref()) {
            return Err(super::super::error::failure(
                "mux.arg.invalid",
                format!("unknown option: {}", key.to_str()?),
            ));
        }
    }
    Ok(())
}
