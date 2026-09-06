//! Deterministic game-module discovery, Lua parents and declaration registration.
use super::Scripts;
use crate::config::Config;
use anyhow::{Context, Result, ensure};
use mlua::Table;
use std::path::{Path, PathBuf};

/// Recursively discover Lua sources in lexical path order.
fn files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            paths.extend(files(&p)?)
        } else if p.extension().is_some_and(|s| s == "lua") {
            paths.push(p);
        }
    }
    paths.sort();
    Ok(paths)
}

impl Scripts {
    /// Register object modules and their parent map before evaluating global modules.
    pub(super) fn load_game_modules(&mut self, config: &Config) -> Result<()> {
        let dir = config.lua_dir();
        for p in files(&dir.join("object_logic"))? {
            let name = p
                .strip_prefix(dir.join("object_logic"))?
                .to_string_lossy()
                .replace('\\', "/");
            let t = self.load_module(&p)?;
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
        for p in files(&dir.join("global_logic"))? {
            let t = self.load_module(&p)?;
            let source = p.strip_prefix(&dir)?.to_string_lossy().replace('\\', "/");
            self.commands.register_lua(
                &self.lua,
                &t,
                &source,
                crate::commands::CommandScope::Global,
            )?;
            self.schedules.register(&source, &t)?;
            self.globals.push(t);
        }
        Ok(())
    }

    /// Load one named source with a fresh instruction budget and contextual errors.
    fn load_module(&self, p: &Path) -> Result<Table> {
        self.budget.reset();
        self.lua
            .load(std::fs::read_to_string(p)?)
            .set_name(p.to_string_lossy())
            .eval()
            .map_err(|e| anyhow::anyhow!(e.to_string()))
            .with_context(|| format!("loading {}", p.display()))
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
