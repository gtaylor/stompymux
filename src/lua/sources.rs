//! Bounded immutable game-source snapshots shared by startup, checking and reload.
use crate::config::Config;
use anyhow::{Context, Result, ensure};
use std::{
    collections::BTreeMap,
    path::{Component, Path},
};

/// UTF-8 Lua files keyed by root-qualified relative paths; contains no VM handles.
#[derive(Clone, Default)]
pub struct Sources {
    pub files: BTreeMap<String, String>,
}

/// Accept only normalized object_logic-relative Lua paths.
pub fn parent_path(path: &str) -> Result<()> {
    ensure!(
        !path.is_empty()
            && !path.contains(['\0', '\\'])
            && path.ends_with(".lua")
            && path
                .split('/')
                .all(|p| !p.is_empty() && p != "." && p != "..")
            && Path::new(path)
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
            && !["object_logic/", "global_logic/", "packages/"]
                .iter()
                .any(|p| path.starts_with(p)),
        "Lua parent paths must be relative .lua paths within object_logic"
    );
    Ok(())
}

impl Sources {
    /// Read a lexical snapshot without following symlinks outside a source root or through cycles.
    pub fn read(config: &Config) -> Result<Self> {
        let mut result = Self::default();
        let mut remaining = config.lua.memory_limit;
        for root in ["object_logic", "global_logic", "packages"] {
            let base = config
                .lua_dir()
                .join(root)
                .canonicalize()
                .with_context(|| format!("Lua root {root}"))?;
            let mut visited = Vec::new();
            Self::walk(
                &base,
                &base,
                root,
                &mut visited,
                &mut remaining,
                &mut result.files,
            )?;
        }
        Ok(result)
    }

    /// Add test sources only for explicit checking or test execution.
    pub fn with_tests(mut self, config: &Config) -> Result<Self> {
        let root = config.lua_dir().join("tests");
        if !root.exists() {
            return Ok(self);
        }
        let base = root.canonicalize()?;
        let used: usize = self.files.values().map(String::len).sum();
        let mut remaining = config.lua.memory_limit.saturating_sub(used);
        Self::walk(
            &base,
            &base,
            "tests",
            &mut Vec::new(),
            &mut remaining,
            &mut self.files,
        )?;
        Ok(self)
    }

    fn walk(
        base: &Path,
        dir: &Path,
        prefix: &str,
        visited: &mut Vec<std::path::PathBuf>,
        remaining: &mut usize,
        files: &mut BTreeMap<String, String>,
    ) -> Result<()> {
        let resolved = dir.canonicalize()?;
        ensure!(
            resolved.starts_with(base),
            "Lua path escapes source root: {}",
            dir.display()
        );
        ensure!(
            !visited.contains(&resolved),
            "Lua directory cycle: {}",
            dir.display()
        );
        visited.push(resolved);
        let mut entries = std::fs::read_dir(dir)?.collect::<std::io::Result<Vec<_>>>()?;
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            let name = entry
                .file_name()
                .into_string()
                .map_err(|_| anyhow::anyhow!("Lua path is not UTF-8"))?;
            let key = format!("{prefix}/{name}");
            let resolved = path.canonicalize()?;
            ensure!(
                resolved.starts_with(base),
                "Lua path escapes source root: {}",
                path.display()
            );
            if path.is_dir() {
                Self::walk(base, &path, &key, visited, remaining, files)?;
                continue;
            }
            if path.extension().is_none_or(|e| e != "lua") {
                continue;
            }
            use std::io::Read;
            let mut data = Vec::new();
            std::fs::File::open(&resolved)?
                .take((*remaining as u64).saturating_add(1))
                .read_to_end(&mut data)?;
            ensure!(
                data.len() <= *remaining,
                "Lua source snapshot exceeds configured memory limit at {key}"
            );
            *remaining -= data.len();
            files.insert(
                key.clone(),
                String::from_utf8(data).with_context(|| format!("Lua source {key}"))?,
            );
        }
        visited.pop();
        Ok(())
    }

    /// Parent assignment never loads a file that was absent from the active snapshot.
    pub fn contains_parent(&self, path: &str) -> Result<()> {
        parent_path(path)?;
        ensure!(
            self.files.contains_key(&format!("object_logic/{path}")),
            "Lua parent is not loaded: {path}; use @lua/reload after adding modules"
        );
        Ok(())
    }

    /// Read current source for viewing without requiring unrelated files to be valid.
    pub fn view(config: &Config, path: &str) -> Result<String> {
        parent_path(path)?;
        let root = config.lua_dir().join("object_logic").canonicalize()?;
        let file = root.join(path).canonicalize()?;
        ensure!(file.starts_with(root), "Lua parent escapes object_logic");
        use std::io::Read;
        let mut text = String::new();
        std::fs::File::open(file)?
            .take(config.lua.memory_limit as u64 + 1)
            .read_to_string(&mut text)?;
        ensure!(
            text.len() <= config.lua.memory_limit,
            "Lua source exceeds configured memory limit"
        );
        Ok(text)
    }
}
