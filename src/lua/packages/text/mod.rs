//! Native bindings for the existing mux.text package.
mod document;
use super::bind;
use crate::{config::Config, lua::err, text};
use anyhow::Result;
pub(super) use document::LuaDocument;
use mlua::{Lua, Table, Value};
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
    bind!(lua, api, "markup", move |_, s: String| text::validate(
        &p, &s
    )
    .map_err(err));
    let p = palette.clone();
    bind!(lua, api, "width", move |_, s: String| {
        let plain: String = text::Document::Styled(s)
            .spans(&p, &text::RenderOptions::default())
            .iter()
            .map(|s| s.text.as_str())
            .collect();
        Ok(unicode_width::UnicodeWidthStr::width(plain.as_str()))
    });
    let p = palette.clone();
    bind!(lua, api, "truncate", move |_, (s, n): (String, usize)| Ok(
        text::truncate_with(&p, &s, n)
    ));
    let p = palette.clone();
    bind!(lua, api, "strip", move |_, s: String| Ok(
        text::Document::Styled(s)
            .spans(&p, &text::RenderOptions::default())
            .iter()
            .map(|s| s.text.as_str())
            .collect::<String>()
    ));
    let p = palette.clone();
    bind!(lua, api, "style", move |_, (s, t): (String, Table)| {
        let mut tags = Vec::new();
        for (field, tag) in [("foreground", "fg"), ("background", "bg")] {
            if let Some(v) = t.get::<Option<String>>(field)? {
                tags.push(format!("{tag}={v}"));
            }
        }
        for field in ["bold", "underline", "inverse"] {
            match t.get::<Value>(field)? {
                Value::Nil | Value::Boolean(false) => {}
                Value::Boolean(true) => tags.push(field.into()),
                _ => return Err(err("style fields have invalid types")),
            }
        }
        let value = if tags.is_empty() {
            s
        } else {
            format!("[{}]{s}[/]", tags.join(" "))
        };
        if value.len() > message_limit {
            return Err(err("styled text output limit exceeded"));
        }
        text::validate(&p, &value).map_err(err)
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
    bind!(lua, api, "printable_ascii", |_, value: Value| {
        let Value::String(s) = value else {
            return Err(mlua::Error::BadArgument {
                to: Some("mux.text.is_printable_ascii".into()),
                pos: 1,
                name: None,
                cause: mlua::Error::FromLuaConversionError {
                    from: value.type_name(),
                    to: "string".into(),
                    message: Some("expected an actual Lua string".into()),
                }
                .into(),
            });
        };
        Ok(s.as_bytes().iter().all(|b| (0x20..=0x7e).contains(b)))
    });
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
