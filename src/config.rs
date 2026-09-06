use anyhow::{Context, Result, bail, ensure};
use std::{
    collections::BTreeSet,
    path::{Path, PathBuf},
};

#[derive(Clone, Debug)]
pub struct Config {
    pub root: PathBuf,
    pub values: toml::Value,
    pub warnings: Vec<String>,
}
fn merge(base: &mut toml::Value, overlay: toml::Value) {
    match (base, overlay) {
        (toml::Value::Table(a), toml::Value::Table(b)) => {
            for (k, v) in b {
                if let Some(old) = a.get_mut(&k) {
                    merge(old, v)
                } else {
                    a.insert(k, v);
                }
            }
        }
        (a, b) => *a = b,
    }
}
fn read(path: &Path, stack: &mut BTreeSet<PathBuf>) -> Result<toml::Value> {
    let path = path
        .canonicalize()
        .with_context(|| format!("reading {}", path.display()))?;
    ensure!(
        stack.insert(path.clone()),
        "configuration include cycle: {}",
        path.display()
    );
    let mut value: toml::Value = toml::from_str(&std::fs::read_to_string(&path)?)?;
    let mut result = toml::Value::Table(Default::default());
    if let Some(includes) = value.as_table_mut().unwrap().remove("include") {
        for item in includes.as_array().context("include must be an array")? {
            merge(
                &mut result,
                read(
                    &path
                        .parent()
                        .unwrap()
                        .join(item.as_str().context("include must contain paths")?),
                    stack,
                )?,
            );
        }
    }
    merge(&mut result, value);
    stack.remove(&path);
    Ok(result)
}
impl Config {
    pub fn load(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().canonicalize()?;
        let values = read(&root.join("stompymux.toml"), &mut BTreeSet::new())?;
        let mut c = Self {
            root,
            values,
            warnings: Vec::new(),
        };
        for (key, default, min, max) in [
            ("server.port", 5555, 0, 65535),
            ("names.maximum_length", 30, 2, 256),
            ("security.player_password_length_limit", 64, 1, 4096),
            ("security.password_hash_opslimit", 3, 1, 100),
            (
                "security.password_hash_memlimit",
                12582912,
                1048576,
                1073741824,
            ),
            ("security.login_attempt_burst", 3, 1, 10000),
            ("security.login_attempt_refill", 10, 1, 86400),
            ("security.login_hash_limit", 5, 1, 1000),
            ("lua.memory_limit", 67108864, 1048576, 1073741824),
            ("lua.state_value_limit", 65536, 1, 10485760),
            ("lua.state_entry_limit", 1024, 1, 1000000),
            ("lua.state_object_limit", 1048576, 1, 1073741824),
            ("mux.idle_timeout", 3600, 1, i64::MAX),
            ("mux.command_quota_increment", 1000, 1, 100000),
            ("mux.command_quota_interval", 50, 1, 60000),
            ("mux.player_starting_room", 4, 0, i64::MAX),
            ("mux.player_starting_home", 4, 0, i64::MAX),
        ] {
            if let Some(v) = c.get(key) {
                let n = v
                    .as_integer()
                    .with_context(|| format!("{key} must be an integer"))?;
                ensure!((min..=max).contains(&n), "{key} outside supported range");
            } else {
                let _ = default;
            }
        }
        for key in [
            "database.game_database",
            "lua.directory",
            "server.mud_name",
            "lua.error_reporting",
        ] {
            if let Some(v) = c.get(key) {
                ensure!(v.is_str(), "{key} must be a string");
            }
        }
        for kind in ["player", "room", "thing", "exit"] {
            let flags = format!("mux.default_{kind}_flags");
            if let Some(v) = c.get(&flags) {
                ensure!(
                    v.as_array()
                        .is_some_and(|a| a.iter().all(|v| v.as_str().is_some())),
                    "{flags} must be an array of flag names"
                );
            }
            let parent = format!("mux.default_{kind}_lua_parent");
            if let Some(v) = c.get(&parent) {
                ensure!(v.is_str(), "{parent} must be a string");
            }
        }
        if let Some(v) = c.get("names.bad") {
            ensure!(
                v.as_array().is_some_and(|a| a.iter().all(|v| v.is_str())),
                "names.bad must be an array of patterns"
            );
        }
        if let Some(v) = c.get("mux.player_name_spaces") {
            ensure!(v.is_bool(), "mux.player_name_spaces must be boolean");
        }
        if let Some(v) = c.get("aliases.commands") {
            ensure!(
                v.as_table().is_some_and(|t| t.values().all(|v| v.is_str())),
                "command aliases must be strings"
            );
        }
        for key in [
            "aliases.flags",
            "colors",
            "mux.idle_interval",
            "mux.command_queue_active_chunk",
            "mux.command_queue_idle_chunk",
            "database.mech_database",
            "database.map_database",
        ] {
            if c.get(key).is_some() {
                c.warnings
                    .push(format!("{key}: deferred in the foundation milestone"));
            }
        }
        ensure!(
            ["off", "wizards", "all"]
                .contains(&c.string("lua.error_reporting", "wizards").as_str()),
            "invalid lua.error_reporting"
        );
        for section in ["battletech", "logging", "osc8"] {
            if c.get(section).is_some() {
                c.warnings
                    .push(format!("{section}: deferred in the foundation milestone"));
            }
        }
        for key in ["database.dump_interval", "database.fork_dump"] {
            if c.get(key).is_some() {
                c.warnings.push(format!(
                    "{key}: deferred; Rust storage commits each mutation"
                ));
            }
        }
        if c.get("sites")
            .and_then(|v| v.as_table())
            .is_some_and(|t| !t.is_empty())
        {
            bail!("site rules are not supported yet; refusing to ignore access restrictions");
        }
        if c.get("access.commands")
            .and_then(|v| v.as_table())
            .is_some_and(|t| !t.is_empty())
        {
            bail!("custom command access rules are not supported yet");
        }
        Ok(c)
    }
    pub fn get(&self, key: &str) -> Option<&toml::Value> {
        let mut v = &self.values;
        for k in key.split('.') {
            v = v.get(k)?;
        }
        Some(v)
    }
    pub fn int(&self, key: &str, default: i64) -> i64 {
        self.get(key)
            .and_then(|v| v.as_integer())
            .unwrap_or(default)
    }
    pub fn string(&self, key: &str, default: &str) -> String {
        self.get(key)
            .and_then(|v| v.as_str())
            .unwrap_or(default)
            .into()
    }
    pub fn strings(&self, key: &str) -> Vec<String> {
        self.get(key)
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    }
    pub fn database(&self) -> PathBuf {
        self.root.join("data/stompymux-rs.db")
    }
    pub fn lua_dir(&self) -> PathBuf {
        self.root.join(self.string("lua.directory", "lua"))
    }
    pub fn start(&self) -> i64 {
        self.int("mux.player_starting_room", 4)
    }
    pub fn home(&self) -> i64 {
        self.int("mux.player_starting_home", 4)
    }
}
