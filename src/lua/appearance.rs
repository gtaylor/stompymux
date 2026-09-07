//! Explicit internal/external render selection and read-only attached behavior metadata.
use super::Scripts;
use crate::world::ObjectId;
use anyhow::{Context, Result};
use mlua::{Function, Value};

/// Which face of an object the viewer is looking at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AppearanceMode {
    Internal,
    External,
}

impl Scripts {
    /// A missing callback permits native fallback; a defined empty renderer suppresses fallback.
    pub fn render_appearance(
        &self,
        viewer: ObjectId,
        object: ObjectId,
        session: Option<u64>,
        mode: AppearanceMode,
    ) -> Result<Option<String>> {
        self.sync_parents()?;
        let path = self
            .world
            .borrow()
            .objects
            .get(&object)
            .context("Appearance object missing")?
            .lua_parent
            .clone();
        if path.is_empty() {
            return Ok(None);
        }
        let parent = self
            .lua
            .named_registry_value::<mlua::Table>("mux.parents")
            .and_then(|parents| parents.get::<Option<mlua::Table>>(path.as_str()))
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .ok_or_else(|| anyhow::anyhow!("appearance parent missing: {path}"))?;
        let name = match mode {
            AppearanceMode::Internal => "internal_appearance",
            AppearanceMode::External => "external_appearance",
        };
        let renderer = parent
            .get::<Option<Function>>(name)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let Some(renderer) = renderer else {
            return Ok(None);
        };
        self.reset_callback_budget();
        let value: Value = self.call(
            &renderer,
            self.context(Some(viewer), Some(object), session)?,
        )?;
        let Value::String(text) = value else {
            anyhow::bail!("{path}: {name} must return a string");
        };
        Ok(Some(
            text.to_str()
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .to_string(),
        ))
    }

    /// Enumerate metadata using raw Lua table reads, never executing module callbacks/metamethods.
    pub fn inspect_parent(&self, object: ObjectId) -> Result<Vec<String>> {
        let path = self
            .world
            .borrow()
            .objects
            .get(&object)
            .context("Appearance object missing")?
            .lua_parent
            .clone();
        if path.is_empty() {
            return Ok(Vec::new());
        }
        let mut lines = vec![format!("Lua parent: object_logic/{path}")];
        let Some(parent) = self.parents.get(&path) else {
            lines.push("Lua behaviors unavailable.".into());
            return Ok(lines);
        };
        let read = || -> mlua::Result<Vec<String>> {
            let mut result = Vec::new();
            for name in ["internal_appearance", "external_appearance"] {
                if matches!(parent.raw_get::<Value>(name)?, Value::Function(_)) {
                    result.push(format!("Lua appearance: {name}"));
                }
            }
            let module = format!("object_logic/{path}");
            for command in self.commands.definitions().filter(|d| d.source == module) {
                if let crate::commands::CommandMatcher::LuaPattern(pattern) = &command.matcher {
                    result.push(format!("Lua commands: {pattern}"));
                }
            }
            for schedule in self.schedules.definitions().filter(|d| d.module == module) {
                result.push(format!("Lua schedules: {}", schedule.name));
            }
            for section in ["events", "messages", "locks"] {
                if let Value::Table(table) = parent.raw_get::<Value>(section)? {
                    let mut names = Vec::new();
                    for pair in table.pairs::<Value, Value>() {
                        if let (Value::String(name), Value::Function(_)) = pair? {
                            names.push(name.to_str()?.to_string());
                        }
                    }
                    names.sort();
                    result.extend(
                        names
                            .into_iter()
                            .map(|name| format!("Lua {section}: {name}")),
                    );
                }
            }
            Ok(result)
        };
        lines.extend(read().map_err(|e| anyhow::anyhow!("{e}"))?);
        Ok(lines)
    }
}
