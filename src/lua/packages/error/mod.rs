//! Structured Lua failures and immutable native/custom error-code trees.
mod catalog;
use mlua::{Lua, LuaSerdeExt, MetaMethod, Table, Value};

/// A typed host failure; codes never depend on parsing diagnostic messages.
#[derive(Debug)]
pub(crate) struct Failure {
    pub code: &'static str,
    pub message: Vec<u8>,
    pub detail: Option<serde_json::Value>,
    context: Option<FailureContext>,
}

#[derive(Debug, Clone)]
struct FailureContext {
    name: String,
    method: bool,
    where_: String,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}: {}",
            self.code,
            String::from_utf8_lossy(&self.message)
        )
    }
}
impl std::error::Error for Failure {}

/// Preserve a stable domain code through mlua callback error wrapping.
pub(crate) fn failure(code: &'static str, message: impl ToString) -> mlua::Error {
    mlua::Error::external(Failure {
        code,
        message: message.to_string().into_bytes(),
        detail: None,
        context: None,
    })
}

fn failure_bytes(code: &'static str, message: Vec<u8>) -> mlua::Error {
    mlua::Error::external(Failure {
        code,
        message,
        detail: None,
        context: None,
    })
}

/// Raise a structured failure carrying raw message bytes (non-UTF-8 names).
pub(crate) fn failure_bytes_pub(code: &'static str, message: Vec<u8>) -> mlua::Error {
    failure_bytes(code, message)
}

/// A plain Lua type error that must stay a string through protected calls.
/// C's luaL_checklstring/checkinteger raise ordinary errors named '?' that no
/// wrapper converts to a structured failure.
#[derive(Debug)]
pub(crate) struct PlainError(pub Vec<u8>);

impl std::fmt::Display for PlainError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", String::from_utf8_lossy(&self.0))
    }
}

impl std::error::Error for PlainError {}

/// Raise a C-style plain argument type error.
pub(crate) fn plain_type_error(argument: usize, expected: &str, got: &str) -> mlua::Error {
    mlua::Error::external(PlainError(
        format!("bad argument #{argument} to '?' ({expected} expected, got {got})").into_bytes(),
    ))
}

/// Raise the plain `bad argument #N to '__eq'` error C's luaL_checkudata-based
/// `__eq` handlers produce when one operand is foreign userdata.
pub(crate) fn eq_type_error(argument: usize, metatable: &str, got: &str) -> mlua::Error {
    mlua::Error::external(PlainError(
        format!("bad argument #{argument} to '__eq' ({metatable} expected, got {got})")
            .into_bytes(),
    ))
}

/// Compare one typed userdata family with C's `__eq` semantics: both operands
/// are checked in order and the first foreign value raises the C plain type
/// error naming its position (C checks luaL_checkudata(1) before (2)).
///
/// LuaJIT only dispatches `__eq` when both operands share one metatable, so
/// the error paths stay unreachable while the engine is LuaJIT; they model the
/// C handlers for any engine that dispatches across metatables.
pub(crate) fn typed_eq<T, F>(
    metatable: &'static str,
    left: mlua::Value,
    right: mlua::Value,
    equal: F,
) -> mlua::Result<bool>
where
    T: mlua::UserData + 'static,
    F: FnOnce(&T, &T) -> bool,
{
    let typename = |value: &mlua::Value| match value {
        mlua::Value::Table(_) => "table",
        mlua::Value::UserData(_) => "userdata",
        mlua::Value::Nil => "nil",
        _ => "value",
    };
    let (left_data, right_data) = match (left, right) {
        (mlua::Value::UserData(left), mlua::Value::UserData(right)) => (left, right),
        (mlua::Value::UserData(_), other) => {
            return Err(eq_type_error(2, metatable, typename(&other)));
        }
        (other, _) => return Err(eq_type_error(1, metatable, typename(&other))),
    };
    let (left, right) = match (left_data.borrow::<T>(), right_data.borrow::<T>()) {
        (Ok(left), Ok(right)) => (left, right),
        (Err(_), _) => return Err(eq_type_error(1, metatable, "userdata")),
        _ => return Err(eq_type_error(2, metatable, "userdata")),
    };
    Ok(equal(&left, &right))
}

/// Lua type spelling used by C type-error messages, distinguishing omitted values.
pub(crate) fn lua_type_at(values: &mlua::MultiValue, index: usize) -> &'static str {
    values
        .get(index - 1)
        .map_or("no value", |value| lua_type_name(value))
}

/// Lua type spelling for a present value.
pub(crate) fn lua_type_name(value: &mlua::Value) -> &'static str {
    match value {
        mlua::Value::Nil => "nil",
        mlua::Value::Boolean(_) => "boolean",
        mlua::Value::LightUserData(_) => "light userdata",
        mlua::Value::Integer(_) | mlua::Value::Number(_) => "number",
        mlua::Value::String(_) => "string",
        mlua::Value::Table(_) => "table",
        mlua::Value::Function(_) => "function",
        mlua::Value::Thread(_) => "thread",
        _ => "userdata",
    }
}

/// luaL_checklstring equivalent: strings pass through and numbers coerce to
/// their string form; anything else raises the C plain type error.
pub(crate) fn check_string_arg(
    lua: &Lua,
    values: &mlua::MultiValue,
    argument: usize,
) -> mlua::Result<Vec<u8>> {
    check_string_arg_at(lua, values, argument, argument)
}

/// Like check_string_arg with a separate frame index for method calls, where
/// the public argument number counts the self slot the callback never sees.
pub(crate) fn check_string_arg_at(
    lua: &Lua,
    values: &mlua::MultiValue,
    index: usize,
    argument: usize,
) -> mlua::Result<Vec<u8>> {
    match values.get(index - 1).cloned() {
        Some(mlua::Value::String(value)) => Ok(value.as_bytes().to_vec()),
        Some(value @ (mlua::Value::Integer(_) | mlua::Value::Number(_))) => {
            let converted = lua
                .coerce_string(value)?
                .expect("numbers coerce to strings");
            Ok(converted.as_bytes().to_vec())
        }
        _ => Err(plain_type_error(
            argument,
            "string",
            lua_type_at(values, argument),
        )),
    }
}

/// luaL_checkinteger equivalent: integers pass, numbers and numeric strings
/// truncate toward zero; anything else raises the C plain type error.
pub(crate) fn check_integer_arg(
    lua: &Lua,
    values: &mlua::MultiValue,
    argument: usize,
) -> mlua::Result<i64> {
    let number = match values.get(argument - 1).cloned() {
        Some(mlua::Value::Integer(i)) => return Ok(i),
        Some(v) => lua.coerce_number(v)?,
        None => None,
    };
    match number {
        Some(n) if n.is_finite() => Ok(n.trunc() as i64),
        _ => Err(plain_type_error(
            argument,
            "number",
            lua_type_at(values, argument),
        )),
    }
}

/// Create a typed host failure with detached structured detail for Lua callers.
pub(crate) fn failure_with_detail(
    code: &'static str,
    message: impl ToString,
    detail: serde_json::Value,
) -> mlua::Error {
    mlua::Error::external(Failure {
        code,
        message: message.to_string().into_bytes(),
        detail: Some(detail),
        context: None,
    })
}

/// Format a native argument rejection with Lua's function name, method offset and caller site.
pub(crate) fn argument_failure(
    lua: &Lua,
    argument: usize,
    code: &'static str,
    detail: impl ToString,
) -> mlua::Error {
    mlua::Error::external(Failure {
        code,
        message: detail.to_string().into_bytes(),
        detail: Some(serde_json::json!({"argument":argument})),
        context: Some(stack_context(lua, 0, 1)),
    })
}

fn format_argument_failure(
    message: &[u8],
    detail: &mut serde_json::Value,
    context: &FailureContext,
) -> Vec<u8> {
    let Some(raw_argument) = detail.get("argument").and_then(serde_json::Value::as_u64) else {
        return message.to_vec();
    };
    let method = context.method;
    let argument = raw_argument
        .saturating_sub(u64::from(method))
        .min(i64::MAX as u64);
    if let Some(fields) = detail.as_object_mut() {
        fields.insert("argument".into(), serde_json::Value::from(argument));
    }
    let name = &context.name;
    let where_ = &context.where_;
    let detail = &message[..message.len().min(2047)];
    let where_ = &where_.as_bytes()[..where_.len().min(255)];
    let mut formatted = Vec::with_capacity(2303);
    formatted.extend_from_slice(where_);
    if argument == 0 {
        formatted.extend_from_slice(format!("calling '{name}' on bad self (").as_bytes());
    } else {
        formatted.extend_from_slice(format!("bad argument #{argument} to '{name}' (").as_bytes());
    }
    formatted.extend_from_slice(detail);
    formatted.push(b')');
    formatted.truncate(2303);
    formatted
}

fn stack_context(lua: &Lua, function_level: usize, caller_level: usize) -> FailureContext {
    let (name, method) = lua
        .inspect_stack(function_level, |frame| {
            let names = frame.names();
            (
                names.name.as_deref().unwrap_or("?").to_owned(),
                names.name_what == Some("method"),
            )
        })
        .unwrap_or_else(|| ("?".into(), false));
    let where_ = lua
        .inspect_stack(caller_level, |frame| {
            let line = frame.current_line().unwrap_or(0);
            if line > 0 {
                format!(
                    "{}:{line}: ",
                    frame.source().short_src.as_deref().unwrap_or("?")
                )
            } else {
                String::new()
            }
        })
        .unwrap_or_default();
    FailureContext {
        name,
        method,
        where_,
    }
}

fn table_context(context: &Table) -> FailureContext {
    FailureContext {
        name: context
            .raw_get::<String>("name")
            .unwrap_or_else(|_| "?".into()),
        method: context.raw_get::<bool>("method").unwrap_or(false),
        where_: context.raw_get::<String>("where").unwrap_or_default(),
    }
}

fn valid(code: &str) -> bool {
    code.split('.').all(|part| {
        !part.is_empty()
            && part.as_bytes()[0].is_ascii_lowercase()
            && part
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    })
}

fn checked_string(lua: &Lua, value: Value, to: &'static str) -> mlua::Result<String> {
    lua.coerce_string(value.clone())?
        .map(|value| {
            let bytes = value.as_bytes();
            let end = bytes
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(bytes.len());
            String::from_utf8_lossy(&bytes[..end]).into_owned()
        })
        .ok_or_else(|| mlua::Error::FromLuaConversionError {
            from: value.type_name(),
            to: to.to_owned(),
            message: None,
        })
}

fn checked_bytes(lua: &Lua, value: Value, to: &'static str) -> mlua::Result<Vec<u8>> {
    lua.coerce_string(value.clone())?
        .map(|value| {
            let bytes = value.as_bytes();
            let end = bytes
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(bytes.len());
            bytes[..end].to_vec()
        })
        .ok_or_else(|| mlua::Error::FromLuaConversionError {
            from: value.type_name(),
            to: to.to_owned(),
            message: None,
        })
}

fn code_node(lua: &Lua, metatable: &Table, code: &str) -> mlua::Result<Table> {
    let node = lua.create_table()?;
    node.raw_set("code", code)?;
    node.set_metatable(Some(metatable.clone()))?;
    Ok(node)
}

fn add_code(
    lua: &Lua,
    metatable: &Table,
    root: &Table,
    code: &str,
    start: usize,
) -> mlua::Result<()> {
    let mut node = root.clone();
    let mut end = start;
    while end < code.len() {
        let segment_end = code[end..]
            .find('.')
            .map_or(code.len(), |offset| end + offset);
        let segment = &code[end..segment_end];
        let child = match node.raw_get::<Value>(segment)? {
            Value::Table(child) => child,
            _ => {
                let child = code_node(lua, metatable, &code[..segment_end])?;
                node.raw_set(segment, child.clone())?;
                child
            }
        };
        node = child;
        end = segment_end + 1;
    }
    Ok(())
}

/// Convert host failures without changing arbitrary Lua error values.
pub(crate) fn value(lua: &Lua, value: Value, fallback: &'static str) -> mlua::Result<Value> {
    value_with_context(lua, value, fallback, None)
}

fn value_with_context(
    lua: &Lua,
    value: Value,
    fallback: &'static str,
    context: Option<Table>,
) -> mlua::Result<Value> {
    let Value::Error(error) = value else {
        return Ok(value);
    };
    let mut error = error.as_ref();
    while let mlua::Error::CallbackError { cause, .. } = error {
        error = cause;
    }
    // Lua argument/type failures remain ordinary errors, as with luaL_check* in C.
    if matches!(
        error,
        mlua::Error::FromLuaConversionError { .. } | mlua::Error::BadArgument { .. }
    ) {
        return Ok(Value::String(lua.create_string(error.to_string())?));
    }
    // C-style plain type errors stay plain strings through every wrapper.
    if let Some(plain) = error.downcast_ref::<PlainError>() {
        return Ok(Value::String(lua.create_string(plain.0.clone())?));
    }
    let (code, message, detail) = match error.downcast_ref::<Failure>() {
        Some(f) => {
            let mut detail = f.detail.clone();
            // Argument validators capture the C callback's name and caller before mlua
            // unwinds. Prefer that evidence: facade wrappers can otherwise replace the
            // native `?`/no-line context with their internal Lua source location.
            let context = f
                .context
                .clone()
                .or_else(|| context.as_ref().map(table_context));
            let message = match (detail.as_mut(), context.as_ref()) {
                (Some(detail), Some(context)) if detail.get("argument").is_some() => {
                    format_argument_failure(&f.message, detail, context)
                }
                _ => f.message.clone(),
            };
            (f.code, message, detail)
        }
        None => (fallback, error.to_string().into_bytes(), None),
    };
    let mux: Table = lua.globals().get("mux")?;
    let api: Table = mux.get("error")?;
    let fields = lua.create_table()?;
    fields.set("code", code)?;
    fields.set("message", lua.create_string(message)?)?;
    if let Some(detail) = detail.as_ref() {
        fields.set("detail", lua.to_value(detail)?)?;
    }
    api.get::<mlua::Function>("new")?.call(fields)
}

/// Wrap native closures before facades capture them, retaining Lua-origin structured errors.
pub(crate) fn wrap(
    lua: &Lua,
    function: mlua::Function,
    code: &'static str,
) -> mlua::Result<mlua::Function> {
    let convert = lua.create_function(move |lua, (v, context): (Value, Option<Table>)| {
        value_with_context(lua, v, code, context)
    })?;
    let capture = lua.create_function(|lua, ()| {
        let context = lua.create_table()?;
        if let Some((name, method)) = lua.inspect_stack(1, |frame| {
            let names = frame.names();
            (
                names.name.as_deref().unwrap_or("?").to_owned(),
                names.name_what == Some("method"),
            )
        }) {
            context.raw_set("name", name)?;
            context.raw_set("method", method)?;
        } else {
            context.raw_set("name", "?")?;
            context.raw_set("method", false)?;
        }
        let where_ = lua
            .inspect_stack(2, |frame| {
                let line = frame.current_line().unwrap_or(0);
                if line > 0 {
                    format!(
                        "{}:{line}: ",
                        frame.source().short_src.as_deref().unwrap_or("?")
                    )
                } else {
                    String::new()
                }
            })
            .unwrap_or_default();
        context.raw_set("where", where_)?;
        Ok(context)
    })?;
    lua.load(
        r#"
        local f,convert,capture=...
        local pcall,unpack,select=pcall,unpack,select
        local function pack(...) return {n=select('#',...),...} end
        return function(...)
            local context=capture()
            local r=pack(pcall(f,...))
            if not r[1] then error(convert(r[2],context),0) end
            return unpack(r,2,r.n)
        end
        "#,
    )
    .call((function, convert, capture))
}

pub(super) fn install(lua: &Lua, mux: &Table) -> mlua::Result<()> {
    let node_metatable = lua.create_table()?;
    node_metatable.set(
        MetaMethod::ToString.name(),
        // C reads .code with lua_getfield, so a removed field re-enters __index.
        lua.create_function(|_, node: Table| node.get::<Value>("code"))?,
    )?;
    node_metatable.set(
        MetaMethod::Index.name(),
        lua.create_function(|_, (_node, key): (Table, Value)| {
            let key = match key {
                Value::String(value) => String::from_utf8_lossy(&value.as_bytes()).into_owned(),
                Value::Integer(value) => value.to_string(),
                Value::Number(value) => value.to_string(),
                _ => "<non-string>".into(),
            };
            Err::<Value, _>(failure(
                "mux.arg.invalid",
                format!("unknown Lua error code segment '{key}'"),
            ))
        })?,
    )?;
    node_metatable.set(
        MetaMethod::NewIndex.name(),
        lua.create_function(|_, _: (Table, Value, Value)| {
            Err::<(), _>(failure(
                "mux.arg.invalid",
                "Lua error code nodes are immutable",
            ))
        })?,
    )?;
    let native = lua.create_table()?;
    let roots = lua.create_table()?;
    for root in ["mux", "testing", "btech"] {
        roots.raw_set(root, code_node(lua, &node_metatable, root)?)?;
    }
    for &code in catalog::CODES {
        let (root, _) = code.split_once('.').unwrap_or((code, ""));
        let tree: Table = roots.raw_get(root)?;
        add_code(lua, &node_metatable, &tree, code, root.len() + 1)?;
    }
    native.set(
        "code_tree",
        lua.create_function(
            move |lua, values: mlua::MultiValue| -> mlua::Result<Table> {
                // C's luaL_checkstring raises the ordinary '?'-named type error.
                let root = check_string_arg(lua, &values, 1)?;
                for name in ["mux", "testing", "btech"] {
                    if root == name.as_bytes() {
                        return roots.raw_get(name);
                    }
                }
                let mut message = b"unknown Lua error code root '".to_vec();
                message.extend_from_slice(&root);
                message.push(b'\'');
                Err(failure_bytes("mux.arg.invalid", message))
            },
        )?,
    )?;
    let namespace_metatable = node_metatable.clone();
    native.set(
        "namespace",
        lua.create_function(move |lua, (prefix, names): (Value, Value)| {
            let prefix = checked_string(lua, prefix, "string")?;
            let Value::Table(names) = names else {
                return Err(mlua::Error::FromLuaConversionError {
                    from: names.type_name(),
                    to: "table".into(),
                    message: None,
                });
            };
            if !valid(&prefix) {
                return Err(failure(
                    "mux.arg.invalid",
                    "namespace prefix must contain dotted lower-case segments",
                ));
            }
            if ["mux", "btech", "testing"].contains(&prefix.split('.').next().unwrap()) {
                return Err(failure(
                    "mux.arg.invalid",
                    format!("namespace prefix '{prefix}' is reserved"),
                ));
            }
            let root = code_node(lua, &namespace_metatable, &prefix)?;
            for index in 1..=names.raw_len() {
                let name = checked_string(lua, names.raw_get(index)?, "string")?;
                if !valid(&name) {
                    return Err(failure(
                        "mux.arg.invalid",
                        "namespace names must contain dotted lower-case segments",
                    ));
                }
                let code = format!("{prefix}.{name}");
                add_code(lua, &namespace_metatable, &root, &code, prefix.len() + 1)?;
            }
            Ok(root)
        })?,
    )?;
    native.set(
        "capture",
        lua.create_function(|lua, ()| {
            let context = lua.create_table()?;
            let (name, method) = lua
                .inspect_stack(1, |frame| {
                    let names = frame.names();
                    (
                        names.name.as_deref().unwrap_or("?").to_owned(),
                        names.name_what == Some("method"),
                    )
                })
                .unwrap_or_else(|| ("?".into(), false));
            context.raw_set("name", name)?;
            context.raw_set("method", method)?;
            let where_ = lua
                .inspect_stack(2, |frame| {
                    let line = frame.current_line().unwrap_or(0);
                    if line > 0 {
                        format!(
                            "{}:{line}: ",
                            frame.source().short_src.as_deref().unwrap_or("?")
                        )
                    } else {
                        String::new()
                    }
                })
                .unwrap_or_default();
            context.raw_set("where", where_)?;
            Ok(context)
        })?,
    )?;
    native.set(
        "convert",
        lua.create_function(|lua, error_value: Value| value(lua, error_value, "mux.runtime"))?,
    )?;
    // Retain LuaJIT's own traceback closure before the sandbox hides `debug`.
    // The C runtime saves this same function in its registry.
    let debug: Table = lua.globals().get("debug")?;
    let trace: mlua::Function = debug.get("traceback")?;
    let getinfo: mlua::Function = debug.get("getinfo")?;
    let api_value: Value =
        lua.load(include_str!("api.lua"))
            .call((native.clone(), trace, getinfo))?;
    let Value::Table(api) = api_value else {
        return Err(mlua::Error::runtime("error facade did not return a table"));
    };
    mux.set("error", api.clone())?;
    for name in ["code_tree", "namespace"] {
        api.set(name, wrap(lua, native.get(name)?, "mux.arg.invalid")?)?;
    }
    Ok(())
}

/// Preserve native typed errors through Lua's ordinary protected-call entry points.
/// Non-host errors retain the original Lua value and standard pcall/xpcall behavior.
pub(super) fn protected_calls(lua: &Lua) -> mlua::Result<()> {
    let convert = lua.create_function(|lua, v: Value| {
        if let Value::Error(e) = &v {
            let mut e = e.as_ref();
            while let mlua::Error::CallbackError { cause, .. } = e {
                e = cause;
            }
            if let Some(plain) = e.downcast_ref::<PlainError>() {
                return Ok(Value::String(lua.create_string(plain.0.clone())?));
            }
            if e.downcast_ref::<Failure>().is_some() {
                return value(lua, v, "mux.runtime");
            }
            if matches!(
                e,
                mlua::Error::FromLuaConversionError { .. } | mlua::Error::BadArgument { .. }
            ) {
                return Ok(Value::String(lua.create_string(e.to_string())?));
            }
            if let mlua::Error::RuntimeError(message) = e {
                let message = message
                    .strip_prefix("runtime error: ")
                    .unwrap_or(message)
                    .split("\nstack traceback:")
                    .next()
                    .unwrap_or(message);
                return Ok(Value::String(lua.create_string(message)?));
            }
        }
        Ok(v)
    })?;
    lua.load("local convert=...; local raw_pcall,raw_xpcall=pcall,xpcall; local function pack(...) return {n=select('#',...),...} end; pcall=function(...) local r=pack(raw_pcall(...)); if not r[1] then r[2]=convert(r[2]) end; return unpack(r,1,r.n) end; xpcall=function(fn,handler) return raw_xpcall(fn,function(e) return handler(convert(e)) end) end").call::<()>(convert)
}
