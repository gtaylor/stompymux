//! Deterministic game-module discovery, Lua parents and declaration registration.
use super::Scripts;
use anyhow::{Context, Result, ensure};
use mlua::Table;

impl Scripts {
    /// Register object modules and their parent map before evaluating global modules.
    pub(super) fn load_game_modules(&mut self) -> Result<()> {
        let sources = self.sources.clone();
        for (path, source) in sources
            .files
            .iter()
            .filter(|(p, _)| p.starts_with("packages/"))
        {
            self.lua
                .load(source)
                .set_name(path)
                .into_function()
                .map_err(|e| anyhow::anyhow!("checking {path}: {e}"))?;
        }
        for (path, source) in sources
            .files
            .iter()
            .filter(|(p, _)| p.starts_with("object_logic/"))
        {
            let name = path.strip_prefix("object_logic/").unwrap().to_string();
            let t = self.load_module(path, source)?;
            self.flows
                .register(path, &t)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Self::validate_locks(&t, &name, true)?;
            Self::validate_handlers(&t, &name)?;
            self.commands.register_lua(
                &self.lua,
                &t,
                &format!("object_logic/{name}"),
                crate::commands::CommandScope::Object(name.clone()),
            )?;
            self.schedules
                .register(&format!("object_logic/{name}"), &t)?;
            self.parents.insert(name, t);
        }
        let table = self
            .lua
            .create_table()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for (name, t) in &self.parents {
            table
                .set(name.as_str(), t.clone())
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        self.lua
            .globals()
            .set("_parents", table)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let parent_map = self
            .lua
            .create_table()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for o in self.world.borrow().objects.values() {
            if !o.lua_parent.is_empty() {
                ensure!(
                    self.parents.contains_key(&o.lua_parent),
                    "missing Lua parent {}",
                    o.lua_parent
                );
                parent_map
                    .set(o.id.0, o.lua_parent.as_str())
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            }
        }
        self.lua
            .globals()
            .set("_object_parents", parent_map)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for (source, text) in sources
            .files
            .iter()
            .filter(|(p, _)| p.starts_with("global_logic/"))
        {
            let t = self.load_module(source, text)?;
            self.flows
                .register(source, &t)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            Self::validate_locks(&t, source, false)?;
            Self::validate_handlers(&t, source)?;
            self.commands.register_lua(
                &self.lua,
                &t,
                source,
                crate::commands::CommandScope::Global,
            )?;
            self.schedules.register(source, &t)?;
            self.globals.push(t);
        }
        Ok(())
    }

    /// Load one named source with a fresh instruction budget and contextual errors.
    fn load_module(&self, path: &str, source: &str) -> Result<Table> {
        self.budget.reset();
        let function = self
            .lua
            .load(source)
            .set_name(path)
            .into_function()
            .map_err(|e| anyhow::anyhow!("loading {path}: {e}"))?;
        self.call(&function, ())
            .with_context(|| format!("loading {path}"))
    }

    /// Refresh Lua parent references after world mutations.
    pub fn sync_parents(&self) -> Result<()> {
        let t: Table = self
            .lua
            .globals()
            .get("_object_parents")
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for o in self.world.borrow().objects.values() {
            t.set(o.id.0, o.lua_parent.as_str())
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        Ok(())
    }
}

impl Scripts {
    /// Check callable declaration shapes without invoking callbacks.
    fn validate_handlers(module: &Table, path: &str) -> Result<()> {
        let validate = || -> mlua::Result<()> {
            for name in ["internal_appearance", "external_appearance"] {
                let value = module.raw_get::<mlua::Value>(name)?;
                if !matches!(value, mlua::Value::Nil | mlua::Value::Function(_)) {
                    return Err(mlua::Error::external(format!("{name} must be a function")));
                }
            }
            for name in ["events", "messages"] {
                let value = module.raw_get::<mlua::Value>(name)?;
                if value.is_nil() {
                    continue;
                }
                let mlua::Value::Table(table) = value else {
                    return Err(mlua::Error::external(format!("{name} must be a table")));
                };
                for pair in table.pairs::<mlua::Value, mlua::Value>() {
                    let (key, value) = pair?;
                    if !matches!(
                        (key, value),
                        (mlua::Value::String(_), mlua::Value::Function(_))
                    ) {
                        return Err(mlua::Error::external(format!(
                            "{name} entries must be named functions"
                        )));
                    }
                }
            }
            Ok(())
        };
        validate().map_err(|e| anyhow::anyhow!("{path}: {e}"))
    }

    /// Reject declaration mistakes before any module can receive lock traffic.
    fn validate_locks(module: &Table, path: &str, object: bool) -> Result<()> {
        let value = module
            .get::<mlua::Value>("locks")
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        if value == mlua::Value::Nil {
            return Ok(());
        }
        ensure!(object, "{path}: locks are only valid in object modules");
        let mlua::Value::Table(locks) = value else {
            anyhow::bail!("{path}: locks must be a table");
        };
        for pair in locks.pairs::<mlua::Value, mlua::Value>() {
            let (key, value) = pair.map_err(|e| anyhow::anyhow!("{e}"))?;
            let mlua::Value::String(key) = key else {
                anyhow::bail!("{path}: lock keys must be strings");
            };
            let key = key.to_str().map_err(|e| anyhow::anyhow!("{e}"))?;
            ensure!(
                crate::LockType::from_key(&key).is_some(),
                "{path}: unknown lock key {key}"
            );
            ensure!(
                matches!(value, mlua::Value::Function(_)),
                "{path}: lock {key} must be a function"
            );
        }
        Ok(())
    }
}
