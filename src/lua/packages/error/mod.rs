//! Structured Lua failures and immutable native/custom error-code trees.
mod catalog;
use mlua::{AnyUserData, Lua, MetaMethod, Table, UserData, UserDataMethods, Value};
use std::sync::Arc;

/// A typed host failure; codes never depend on parsing diagnostic messages.
#[derive(Debug)]
pub(crate) struct Failure {
    pub code: &'static str,
    pub message: String,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.message)
    }
}
impl std::error::Error for Failure {}

/// Preserve a stable domain code through mlua callback error wrapping.
pub(crate) fn failure(code: &'static str, message: impl ToString) -> mlua::Error {
    mlua::Error::external(Failure {
        code,
        message: message.to_string(),
    })
}

#[derive(Clone)]
struct Node {
    code: String,
    names: Arc<Vec<String>>,
}
impl UserData for Node {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(
            MetaMethod::NewIndex,
            |_, _, _: (Value, Value)| -> mlua::Result<()> {
                Err(failure("mux.arg.invalid", "error codes are immutable"))
            },
        );
        m.add_meta_method(MetaMethod::Eq, |_, a, b: AnyUserData| {
            Ok(b.borrow::<Node>().is_ok_and(|b| a.code == b.code))
        });
        m.add_meta_method(MetaMethod::ToString, |_, n, ()| Ok(n.code.clone()));
        m.add_meta_method(MetaMethod::Index, |lua, n, key: String| {
            if key == "code" {
                return Ok(Value::String(lua.create_string(&n.code)?));
            }
            let code = format!("{}.{}", n.code, key);
            if !n
                .names
                .iter()
                .any(|s| s == &code || s.starts_with(&(code.clone() + ".")))
            {
                return Err(failure(
                    "mux.arg.invalid",
                    format!("unknown error code {code}"),
                ));
            }
            Ok(Value::UserData(lua.create_userdata(Node {
                code,
                names: n.names.clone(),
            })?))
        });
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

/// Convert host failures without changing arbitrary Lua error values.
pub(crate) fn value(lua: &Lua, value: Value, fallback: &'static str) -> mlua::Result<Value> {
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
    let (code, message) = match error.downcast_ref::<Failure>() {
        Some(f) => (f.code, f.message.clone()),
        None => (fallback, error.to_string()),
    };
    let mux: Table = lua.globals().get("mux")?;
    let api: Table = mux.get("error")?;
    let fields = lua.create_table()?;
    fields.set("code", code)?;
    fields.set("message", message)?;
    api.get::<mlua::Function>("new")?.call(fields)
}

/// Wrap native closures before facades capture them, retaining Lua-origin structured errors.
pub(crate) fn wrap(
    lua: &Lua,
    function: mlua::Function,
    code: &'static str,
) -> mlua::Result<mlua::Function> {
    let convert = lua.create_function(move |lua, v: Value| value(lua, v, code))?;
    lua.load("local f,convert=...; local pcall=pcall; local function pack(...) return {n=select('#',...),...} end; return function(...) local r=pack(pcall(f,...)); if not r[1] then error(convert(r[2]),0) end; return unpack(r,2,r.n) end").call((function,convert))
}

pub(super) fn install(lua: &Lua, mux: &Table) -> mlua::Result<()> {
    let names = Arc::new(
        catalog::CODES
            .iter()
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
    );
    let native = lua.create_table()?;
    let roots = lua.create_table()?;
    for root in ["mux", "testing", "btech"] {
        roots.set(
            root,
            lua.create_userdata(Node {
                code: root.into(),
                names: names.clone(),
            })?,
        )?;
    }
    native.set(
        "code_tree",
        lua.create_function(move |_, root: String| {
            roots
                .get::<Option<AnyUserData>>(root.clone())?
                .ok_or_else(|| {
                    failure(
                        "mux.arg.invalid",
                        format!("unknown Lua error code root '{root}'"),
                    )
                })
        })?,
    )?;
    native.set(
        "namespace",
        lua.create_function(|lua, (prefix, names): (String, Table)| {
            if !valid(&prefix)
                || ["mux", "btech", "testing"].contains(&prefix.split('.').next().unwrap())
            {
                return Err(failure(
                    "mux.arg.invalid",
                    "invalid or reserved namespace prefix",
                ));
            }
            let mut symbols = Vec::new();
            for name in names.sequence_values::<String>() {
                let name = name?;
                if !valid(&name) {
                    return Err(failure("mux.arg.invalid", "invalid namespace name"));
                }
                symbols.push(format!("{prefix}.{name}"));
            }
            lua.create_userdata(Node {
                code: prefix,
                names: Arc::new(symbols),
            })
        })?,
    )?;
    let trace = lua.create_function(|lua, _: mlua::MultiValue| {
        let mut rows = vec!["stack traceback:".to_owned()];
        for level in 1..32 {
            let Some(row) = lua.inspect_stack(level, |f| {
                format!(
                    "  {}:{}",
                    f.source().short_src.as_deref().unwrap_or("?"),
                    f.current_line().unwrap_or(0)
                )
            }) else {
                break;
            };
            rows.push(row);
        }
        Ok(rows.join("\n"))
    })?;
    let api: Table = lua
        .load(include_str!("api.lua"))
        .call((native.clone(), trace))?;
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
            if e.downcast_ref::<Failure>().is_some() {
                return value(lua, v, "mux.runtime");
            }
        }
        Ok(v)
    })?;
    lua.load("local convert=...; local raw_pcall,raw_xpcall=pcall,xpcall; local function pack(...) return {n=select('#',...),...} end; pcall=function(...) local r=pack(raw_pcall(...)); if not r[1] then r[2]=convert(r[2]) end; return unpack(r,1,r.n) end; xpcall=function(fn,handler) return raw_xpcall(fn,function(e) return handler(convert(e)) end) end").call::<()>(convert)
}
