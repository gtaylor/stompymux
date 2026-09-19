//! Native bindings for the existing mux.text package.
mod document;
use super::bind;
use crate::{config::Config, lua::err, text};
use anyhow::Result;
pub(super) use document::LuaDocument;
use mlua::{FromLua, Lua, MultiValue, Table, Value};
use std::{cell::Cell, rc::Rc, sync::Arc};

/// Register native operations before the embedded facade is evaluated.
pub(super) fn register(
    lua: &Lua,
    api: &Table,
    config: &Config,
    palette: &Arc<text::Palette>,
) -> Result<()> {
    let message_limit = config.runtime.output_message_limit;
    let p = palette.clone();
    bind!(lua, api, "markup", move |lua, values: mlua::MultiValue| {
        let value = super::error::check_string_arg(lua, &values, 1)?;
        let s = std::str::from_utf8(&value).map_err(|_| {
            super::error::failure(
                "mux.text.invalid",
                "invalid styled-text markup: text is not valid UTF-8",
            )
        })?;
        text::validate(&p, s).map_err(|error| {
            super::error::failure(
                "mux.text.invalid",
                format!("invalid styled-text markup: {error}"),
            )
        })?;
        Ok(Value::String(lua.create_string(&s)?))
    });
    let p = palette.clone();
    bind!(lua, api, "width", move |lua, values: mlua::MultiValue| {
        let s = super::error::check_string_arg(lua, &values, 1)?;
        // C measures the byte length of the plain render, whose invalid UTF-8
        // bytes each become one U+FFFD (styled_append_utf8_codepoint).
        Ok(text::mux_plain(&p, &text::c_utf8_lossy(&s)).len())
    });
    let p = palette.clone();
    bind!(
        lua,
        api,
        "truncate",
        move |lua, values: mlua::MultiValue| {
            let s = super::error::check_string_arg(lua, &values, 1)?;
            let width = super::error::check_integer_arg(lua, &values, 2)?;
            // C raises a structured text.invalid rejection for negative widths.
            if width < 0 {
                return Err(super::error::argument_failure(
                    lua,
                    2,
                    "mux.text.invalid",
                    "width must not be negative",
                ));
            }
            Ok(text::mux_truncate(
                &p,
                &text::utf8_valid_prefix(&s),
                width as usize,
            ))
        }
    );
    let p = palette.clone();
    bind!(lua, api, "strip", move |lua, values: mlua::MultiValue| {
        let s = super::error::check_string_arg(lua, &values, 1)?;
        // C replaces each invalid UTF-8 byte with one U+FFFD while rendering.
        Ok(text::mux_plain(&p, &text::c_utf8_lossy(&s)))
    });
    let p = palette.clone();
    bind!(lua, api, "style", move |lua, values: MultiValue| {
        let bytes = super::error::check_string_arg(lua, &values, 1)?;
        if bytes.contains(&0) {
            return Err(super::error::argument_failure(
                lua,
                1,
                "mux.text.invalid",
                "value contains an embedded NUL byte",
            ));
        }
        let s = std::str::from_utf8(&bytes).map_err(|_| {
            super::error::failure(
                "mux.text.invalid",
                "invalid style: invalid UTF-8 styled text",
            )
        })?;
        // C luaL_checktype(2, LUA_TTABLE) raises a plain '?'-named type error.
        let Value::Table(t) = values.get(1).cloned().unwrap_or(Value::Nil) else {
            return Err(super::error::plain_type_error(
                2,
                "table",
                super::error::lua_type_at(&values, 2),
            ));
        };
        let mut tags = Vec::new();
        for (field, tag) in [("foreground", "fg"), ("background", "bg")] {
            let value = t.get::<Value>(field)?;
            if !matches!(value, Value::Nil) {
                let Some(v) = lua.coerce_string(value)? else {
                    return Err(super::error::failure(
                        "mux.text.invalid",
                        "style fields have invalid types",
                    ));
                };
                let bytes = v.as_bytes();
                let end = bytes
                    .iter()
                    .position(|byte| *byte == 0)
                    .unwrap_or(bytes.len());
                let v = std::str::from_utf8(&bytes[..end]).map_err(|_| {
                    super::error::failure("mux.text.invalid", "style fields have invalid types")
                })?;
                if v.contains('[') || v.contains(']') {
                    return Err(super::error::failure(
                        "mux.text.invalid",
                        "style fields have invalid types",
                    ));
                }
                tags.push(format!("{tag}={v}"));
            }
        }
        for field in ["bold", "underline", "inverse"] {
            match t.get::<Value>(field)? {
                Value::Nil | Value::Boolean(false) => {}
                Value::Boolean(true) => tags.push(field.into()),
                _ => {
                    return Err(super::error::failure(
                        "mux.text.invalid",
                        "style fields have invalid types",
                    ));
                }
            }
        }
        let value = if tags.is_empty() {
            s.to_owned()
        } else {
            let mut value = tags
                .iter()
                .map(|tag| format!("[{tag}]"))
                .collect::<String>();
            value.push_str(s);
            for _ in &tags {
                value.push_str("[/]");
            }
            value
        };
        if value.len() > message_limit {
            return Err(super::error::failure(
                "mux.text.invalid",
                "styled text output limit exceeded",
            ));
        }
        text::validate(&p, &value).map_err(|error| {
            super::error::failure("mux.text.invalid", format!("invalid style: {error}"))
        })
    });
    let markdown_usage = Rc::new(Cell::new(0usize));
    bind!(lua, api, "markdown", move |lua, s: String| {
        let config = crate::lua::configuration(lua);
        let markdown_memory_limit = config.lua.memory_limit.min(config.lua.output_byte_limit);
        let allocation = s.capacity();
        if allocation > markdown_memory_limit.saturating_sub(markdown_usage.get()) {
            return Err(err("Markdown document memory limit exceeded"));
        }
        let document = text::Document::markdown(s, message_limit).map_err(err)?;
        markdown_usage.set(markdown_usage.get() + allocation);
        Ok(LuaDocument {
            document,
            allocation,
            usage: markdown_usage.clone(),
        })
    });
    bind!(
        lua,
        api,
        "printable_ascii",
        |_, values: mlua::MultiValue| {
            // C luaL_checktype raises the ordinary '?'-named string type error.
            let Some(Value::String(s)) = values.get(0) else {
                return Err(super::error::plain_type_error(
                    1,
                    "string",
                    super::error::lua_type_at(&values, 1),
                ));
            };
            Ok(s.as_bytes().iter().all(|b| (0x20..=0x7e).contains(b)))
        }
    );
    Ok(())
}

/// Install the embedded Lua facade with explicit shared table and identity dependencies.
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    mux: &Table,
    id: &mlua::Function,
) -> mlua::Result<()> {
    lua.load(include_str!("api.lua"))
        .set_name("@builtin/text")
        .call((api, mux, id))
}
