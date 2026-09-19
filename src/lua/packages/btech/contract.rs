//! Shared validation, native registration, facade dispatch, and error helpers for C-compatible bindings.

use super::error;
use crate::{Flag, ObjectId, SharedWorld, World, world::Kind};
use mlua::{Function, Lua, MultiValue, Table, Value};

const BINDINGS_KEY: &str = "__contract_bindings";

fn argument_failure(argument: usize, message: impl ToString) -> mlua::Error {
    error::failure_with_detail(
        "mux.arg.invalid",
        message,
        serde_json::json!({ "argument": argument }),
    )
}

/// Register a C-contract native function and remember its public namespace mapping.
pub(super) fn bind(
    lua: &Lua,
    native: &Table,
    native_key: &'static str,
    group: &'static str,
    public_name: &'static str,
    function: Function,
) -> mlua::Result<()> {
    native.set(native_key, function)?;
    let bindings = match native.raw_get::<Value>(BINDINGS_KEY)? {
        Value::Table(bindings) => bindings,
        _ => {
            let bindings = lua.create_table()?;
            native.raw_set(BINDINGS_KEY, bindings.clone())?;
            bindings
        }
    };
    let binding = lua.create_table()?;
    binding.raw_set("key", native_key)?;
    binding.raw_set("group", group)?;
    binding.raw_set("name", public_name)?;
    bindings.raw_set(bindings.raw_len() + 1, binding)
}

/// Install every binding registered with [`bind`], allowing C contracts to replace old facades.
pub(super) fn install(lua: &Lua, native: &Table, package: &Table) -> mlua::Result<()> {
    let Some(bindings) = native.raw_get::<Option<Table>>(BINDINGS_KEY)? else {
        return Ok(());
    };
    for binding in bindings.sequence_values::<Table>() {
        let binding = binding?;
        let key: String = binding.raw_get("key")?;
        let group: String = binding.raw_get("group")?;
        let name: String = binding.raw_get("name")?;
        let namespace = match package.raw_get::<Value>(group.as_str())? {
            Value::Table(namespace) => namespace,
            _ => lua.create_table()?,
        };
        let dispatch: Function = lua
            .load("local native,key=...; return function(...) return native[key](...) end")
            .call((native.clone(), key))?;
        namespace.raw_set(name, error::wrap(lua, dispatch, "btech.operation.failed")?)?;
        package.raw_set(group, namespace)?;
    }
    Ok(())
}

/// Require at least the C contract's positional argument count.
pub(super) fn check_arity(arguments: &MultiValue, expected: usize) -> mlua::Result<()> {
    if arguments.len() >= expected {
        return Ok(());
    }
    Err(error::failure(
        "mux.arg.invalid",
        format!("expected at least {expected} arguments"),
    ))
}

/// Reject non-string and unknown option keys without invoking table metamethods.
pub(super) fn check_options(table: &Table, allowed: &[&str], argument: usize) -> mlua::Result<()> {
    for pair in table.clone().pairs::<Value, Value>() {
        let (field, _) = pair?;
        let Value::String(field) = field else {
            return Err(argument_failure(argument, "unknown field '<non-string>'"));
        };
        let bytes = field.as_bytes();
        let end = bytes
            .iter()
            .position(|byte| *byte == 0)
            .unwrap_or(bytes.len());
        let field = &bytes[..end];
        if !allowed.iter().any(|allowed| allowed.as_bytes() == field) {
            return Err(argument_failure(
                argument,
                format!("unknown field '{}'", String::from_utf8_lossy(field)),
            ));
        }
    }
    Ok(())
}

/// Read a raw field so option records cannot synthesize values through `__index`.
pub(super) fn field(table: &Table, name: &str) -> mlua::Result<Value> {
    table.raw_get(name)
}

/// Validate a strict integral Lua number in an inclusive range.
pub(super) fn integer(
    value: Value,
    label: &str,
    minimum: i64,
    maximum: i64,
    argument: usize,
) -> mlua::Result<i64> {
    let number = match value {
        Value::Integer(value) => return range_integer(value, label, minimum, maximum, argument),
        Value::Number(value) => value,
        _ => {
            return Err(argument_failure(
                argument,
                format!("{label} must be an integer"),
            ));
        }
    };
    if !number.is_finite()
        || number.fract() != 0.0
        || number < minimum as f64
        || number > maximum as f64
    {
        return Err(argument_failure(
            argument,
            format!("{label} must be an integer from {minimum} to {maximum}"),
        ));
    }
    Ok(number as i64)
}

fn range_integer(
    value: i64,
    label: &str,
    minimum: i64,
    maximum: i64,
    argument: usize,
) -> mlua::Result<i64> {
    if (minimum..=maximum).contains(&value) {
        return Ok(value);
    }
    Err(argument_failure(
        argument,
        format!("{label} must be an integer from {minimum} to {maximum}"),
    ))
}

/// Validate a finite Lua number in an inclusive range.
pub(super) fn number(
    value: Value,
    label: &str,
    minimum: f64,
    maximum: f64,
    argument: usize,
) -> mlua::Result<f64> {
    let number = match value {
        Value::Integer(value) => value as f64,
        Value::Number(value) => value,
        _ => {
            return Err(argument_failure(
                argument,
                format!("{label} must be a number"),
            ));
        }
    };
    if number.is_finite() && (minimum..=maximum).contains(&number) {
        return Ok(number);
    }
    Err(argument_failure(
        argument,
        format!("{label} must be a number from {minimum} to {maximum}"),
    ))
}

/// Validate an exact Lua string with a byte-length bound.
pub(super) fn string(
    value: Value,
    label: &str,
    maximum: usize,
    argument: usize,
) -> mlua::Result<String> {
    let Value::String(value) = value else {
        return Err(argument_failure(
            argument,
            format!("{label} must be a string"),
        ));
    };
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(argument_failure(
            argument,
            format!("{label} must contain 1 to {maximum} bytes"),
        ));
    }
    value
        .to_str()
        .map(|value| value.to_owned())
        .map_err(|_| argument_failure(argument, format!("{label} must be valid UTF-8")))
}

/// Read and validate an integral record field.
pub(super) fn integer_field(
    table: &Table,
    name: &str,
    minimum: i64,
    maximum: i64,
    argument: usize,
) -> mlua::Result<i64> {
    integer(field(table, name)?, name, minimum, maximum, argument)
}

/// Read and validate a finite numeric record field.
pub(super) fn number_field(
    table: &Table,
    name: &str,
    minimum: f64,
    maximum: f64,
    argument: usize,
) -> mlua::Result<f64> {
    number(field(table, name)?, name, minimum, maximum, argument)
}

/// Read and validate a boolean record field.
pub(super) fn boolean_field(table: &Table, name: &str, argument: usize) -> mlua::Result<bool> {
    match field(table, name)? {
        Value::Boolean(value) => Ok(value),
        _ => Err(argument_failure(
            argument,
            format!("{name} must be a boolean"),
        )),
    }
}

/// Read and validate a bounded string record field.
pub(super) fn string_field(
    table: &Table,
    name: &str,
    maximum: usize,
    argument: usize,
) -> mlua::Result<String> {
    string(field(table, name)?, name, maximum, argument)
}

/// Resolve a dbref or generation-checked Object and reject garbage or GOING objects.
pub(super) fn require_object(
    lua: &Lua,
    world: &World,
    value: Value,
    argument: usize,
) -> mlua::Result<ObjectId> {
    let id =
        crate::lua::packages::world::handles::identity_at(lua, value, world, argument, "object")?;
    if world.objects[&id].flags.contains(Flag::Going) {
        return Err(error::failure_with_detail(
            "mux.object.unavailable",
            "object is going away",
            serde_json::json!({ "argument": argument }),
        ));
    }
    Ok(id)
}

/// Resolve an object and verify its registered BattleTech special family.
pub(super) fn require_special(
    lua: &Lua,
    world: &World,
    value: Value,
    argument: usize,
    registration: &str,
    label: &str,
) -> mlua::Result<ObjectId> {
    let id = require_object(lua, world, value, argument)?;
    if world
        .btech
        .registrations()
        .get(&id)
        .is_some_and(|kind| kind.eq_ignore_ascii_case(registration))
    {
        return Ok(id);
    }
    Err(error::failure_with_detail(
        "mux.object.invalid",
        format!("object is not a registered BTech {label}"),
        serde_json::json!({ "argument": argument }),
    ))
}

/// Read a required object field with the outer record's argument position.
pub(super) fn require_object_field(
    lua: &Lua,
    world: &World,
    table: &Table,
    name: &str,
    argument: usize,
) -> mlua::Result<ObjectId> {
    let value = field(table, name)?;
    if value == Value::Nil {
        return Err(argument_failure(argument, format!("{name} is required")));
    }
    let id = crate::lua::packages::world::handles::identity_at(lua, value, world, argument, name)?;
    if world.objects[&id].flags.contains(Flag::Going) {
        return Err(error::failure_with_detail(
            "mux.object.unavailable",
            format!("{name} is going away"),
            serde_json::json!({ "argument": argument }),
        ));
    }
    Ok(id)
}

/// Create the canonical generation-checked Lua Object handle.
pub(super) fn push_object(lua: &Lua, world: &SharedWorld, id: ObjectId) -> mlua::Result<Value> {
    crate::lua::packages::world::handles::push(lua, world, id)
}

/// Push nil for absent or retiring relationships and reject corrupt stored dbrefs.
pub(super) fn push_optional_object(
    lua: &Lua,
    shared: &SharedWorld,
    id: Option<ObjectId>,
) -> mlua::Result<Value> {
    let Some(id) = id else {
        return Ok(Value::Nil);
    };
    let world = shared.borrow();
    let Some(object) = world
        .objects
        .get(&id)
        .filter(|object| object.kind != Kind::Garbage)
    else {
        return Err(error::failure(
            "mux.object.invalid",
            "stored object relationship is corrupt",
        ));
    };
    if object.flags.contains(Flag::Going) {
        return Ok(Value::Nil);
    }
    drop(world);
    push_object(lua, shared, id)
}

/// Reject traversal and directory syntax in script-supplied asset names.
pub(super) fn validate_resource_name(name: &str, label: &str, argument: usize) -> mlua::Result<()> {
    if name.contains("..") || name.contains('/') || name.contains('\\') {
        return Err(argument_failure(
            argument,
            format!("{label} must not contain path components"),
        ));
    }
    Ok(())
}

/// Build a stable BattleTech operation failure with `detail.reason`.
pub(super) fn operation_failure(reason: &str, message: impl ToString) -> mlua::Error {
    error::failure_with_detail(
        "btech.operation.failed",
        message,
        serde_json::json!({ "reason": reason }),
    )
}

/// Borrow the live world captured by contract registration without exposing the container type.
pub(super) fn world(shared: &SharedWorld) -> std::cell::Ref<'_, World> {
    shared.borrow()
}
