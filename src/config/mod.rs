//! Complete legacy TOML catalog plus Rust operational settings.
pub mod administration;
pub mod catalog;
pub mod directives;
mod loader;
mod model;
mod types;
use anyhow::{Context, Result, ensure};
pub use model::*;
use std::{
    collections::BTreeMap,
    net::{IpAddr, SocketAddr},
    ops::Deref,
    path::{Path, PathBuf},
};
pub use types::*;

#[derive(Debug, Clone)]
/// Validated effective settings together with game-root and source diagnostics.
pub struct Config {
    pub root: PathBuf,
    /// Bounded output service shared by effective snapshots and Lua reloads.
    pub logger: crate::logging::Logger,
    settings: Settings,
    /// Exact pre-edit states permitted after a live quota reduction; never serialized.
    retained_state: std::sync::Arc<
        BTreeMap<crate::world::ObjectId, (crate::state::Generation, crate::state::State)>,
    >,
    effective: toml::Value,
    pub warnings: Vec<String>,
    /// Ordered site rules compiled from the merged document.
    pub site_policy: crate::sites::Policy,
    /// Ordered command/list access edits, resolved after module registration.
    pub access_rules: Vec<crate::access::Rule>,
    origins: BTreeMap<String, PathBuf>,
    /// Effective directive permissions, including runtime edits.
    /// Runtime aliases must still resolve in a candidate Lua registry.
    pub runtime_aliases: Vec<String>,
    pub directive_permissions: BTreeMap<String, crate::access::Permissions>,
}
impl Deref for Config {
    type Target = Settings;
    fn deref(&self) -> &Self::Target {
        &self.settings
    }
}
impl Config {
    /// Merge recursive includes, apply defaults and validate recognized configuration.
    pub fn load(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().canonicalize()?;
        let mut doc = loader::read(&root.join("stompymux.toml"))?;
        loader::resolve_flags(&mut doc)?;
        let site_policy = crate::sites::Policy::compile(
            doc.values.get("sites"),
            &doc.origins,
            &mut doc.warnings,
        )?;
        let access_rules = crate::access::rules(doc.values.get("access"), &doc.origins)?;
        let settings: Settings = toml::Value::Table(doc.values).try_into().with_context(|| {
            format!(
                "{}: typed configuration",
                root.join("stompymux.toml").display()
            )
        })?;
        let effective = toml::Value::try_from(&settings)?;
        let mut config = Self {
            root,
            logger: Default::default(),
            settings,
            retained_state: Default::default(),
            effective,
            warnings: doc.warnings,
            site_policy,
            access_rules,
            origins: doc.origins,
            directive_permissions: BTreeMap::new(),
            runtime_aliases: Vec::new(),
        };
        config.compile_directive_permissions()?;
        config.validate()?;
        crate::text::Palette::from_config(&config).with_context(|| {
            let origins = config
                .origins
                .iter()
                .filter(|(k, _)| k.starts_with("colors") || k.starts_with("osc8"))
                .map(|(k, p)| format!("{k} in {}", p.display()))
                .collect::<Vec<_>>()
                .join(", ");
            format!("rendering configuration: {origins}")
        })?;
        config.warnings.push("Configuration parsed completely; BattleTech and remaining legacy command-system settings are retained for future implementation.".into());
        Ok(config)
    }
    /// Check structural invariants independently of implemented server capabilities.
    fn validate(&self) -> Result<()> {
        self.validate_values(true)
    }
    /// Runtime edits validate live values independently of the unused bootstrap template.
    fn validate_values(&self, bootstrap: bool) -> Result<()> {
        let fail = |key: &str| {
            format!(
                "{}: {key}",
                self.origins
                    .get(key)
                    .unwrap_or(&self.root.join("stompymux.toml"))
                    .display()
            )
        };
        for key in catalog::KEYS {
            if let Some(v) = self.effective_value(key.path) {
                loader::validate(key, v).with_context(|| fail(key.path))?;
            }
        }
        for (key, value) in [
            ("mux.command_queue_limit", self.mux.command_queue_limit),
            (
                "mux.command_queue_active_chunk",
                self.mux.command_queue_active_chunk,
            ),
            (
                "mux.command_queue_idle_chunk",
                self.mux.command_queue_idle_chunk,
            ),
        ] {
            ensure!(value >= 0, "{}: must be nonnegative", fail(key));
        }
        for key in [
            "database.game_database",
            "lua.directory",
            "database.bootstrap.credentials_file",
        ] {
            ensure!(
                !self
                    .effective_value(key)
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .is_empty(),
                "{}: path must not be empty",
                fail(key)
            );
        }
        if !bootstrap {
            return Ok(());
        }
        let objects = &self.settings.database.bootstrap.objects;
        for (id, obj) in objects {
            ensure!(
                id.0 < i32::MAX as i64 && !obj.name.is_empty(),
                "{}: invalid bootstrap object #{}",
                fail("database.bootstrap.objects"),
                id.0
            );
            ensure!(
                !obj.wizard || obj.r#type == BootstrapKind::Player,
                "{}: rooms cannot be wizards",
                fail("database.bootstrap.objects")
            );
            ensure!(
                obj.r#type != BootstrapKind::Player || [1, 2].contains(&id.0),
                "{}: bootstrap players must be #1 or #2",
                fail("database.bootstrap.objects")
            );
        }
        let god = objects.get(&BootstrapId(1));
        ensure!(
            god.is_some_and(|o| o.r#type == BootstrapKind::Player && o.wizard && o.name == "GOD"),
            "{}: #1 must be GOD with type player and wizard=true",
            fail("database.bootstrap.objects")
        );
        ensure!(
            objects
                .get(&BootstrapId(2))
                .is_some_and(|o| o.r#type == BootstrapKind::Player && o.name == "Wizard"),
            "{}: #2 must be the Wizard player",
            fail("database.bootstrap.objects")
        );
        for id in [
            0,
            self.mux.player_starting_room,
            self.mux.player_starting_home,
            self.battletech.usedmechstore,
            self.battletech.afterlife_dbref,
        ] {
            ensure!(
                objects
                    .get(&BootstrapId(id))
                    .is_some_and(|o| o.r#type == BootstrapKind::Room),
                "{}: required room #{id} missing",
                fail("database.bootstrap.objects")
            );
        }
        Ok(())
    }
    /// Reject policies the server cannot enforce before any serving side effects.
    pub fn validate_for_serve(&self) -> Result<()> {
        ensure!(
            self.mux.check_interval > 0,
            "mux.check_interval must be positive"
        );
        ensure!(
            self.mux.check_offset >= 0,
            "mux.check_offset must be nonnegative"
        );
        for (key, value) in [
            ("mux.check_interval", self.mux.check_interval),
            ("mux.check_offset", self.mux.check_offset),
        ] {
            ensure!(
                std::time::Instant::now()
                    .checked_add(std::time::Duration::from_secs(value as u64))
                    .is_some(),
                "{key} is too large for a monotonic deadline"
            );
        }
        self.site_policy
            .validate_listener(self.server.listen_address)?;

        Ok(())
    }
    /// Apply optional CLI overrides and refresh the Lua-visible effective snapshot.
    pub fn with_listener_overrides(
        mut self,
        address: Option<IpAddr>,
        port: Option<u16>,
    ) -> Result<Self> {
        if let Some(address) = address {
            self.settings.server.listen_address = address;
        }
        if let Some(port) = port {
            self.settings.server.port = port;
        }
        self.effective = toml::Value::try_from(&self.settings)?;
        Ok(self)
    }
    /// Construct the effective IPv4 or IPv6 listener endpoint.
    pub fn listener(&self) -> SocketAddr {
        SocketAddr::new(self.server.listen_address, self.server.port)
    }
    /// Look up a defaulted value by dotted TOML path or legacy directive name.
    pub fn effective_value(&self, key: &str) -> Option<&toml::Value> {
        let path = catalog::KEYS
            .iter()
            .find(|s| s.legacy == key && !key.is_empty())
            .map_or(key, |s| s.path);
        let mut value = &self.effective;
        for part in path.split('.') {
            value = value.get(part)?;
        }
        Some(value)
    }
    /// Resolve content paths against the game root, preserving absolute paths.
    pub fn path(&self, path: impl AsRef<Path>) -> PathBuf {
        self.root.join(path)
    }
    /// Resolve the live schema-32 database path.
    pub fn database(&self) -> PathBuf {
        self.path(&self.settings.database.game_database)
    }
    /// Resolve the Lua module directory.
    pub fn lua_dir(&self) -> PathBuf {
        self.path(&self.lua.directory)
    }
    /// Return the configured starting room dbref.
    pub fn start(&self) -> i64 {
        self.mux.player_starting_room
    }
    /// Return the configured initial home dbref.
    pub fn home(&self) -> i64 {
        self.mux.player_starting_home
    }
}
