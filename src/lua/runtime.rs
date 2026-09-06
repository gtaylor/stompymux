//! Construct the Lua owner in dependency order before evaluating editable game modules.
use super::{Outbox, Scripts, SharedWorld, packages, sandbox};
use crate::{config::Config, text};
use anyhow::Result;
use std::collections::BTreeMap;

impl Scripts {
    /// Initialize budgets, built-ins and sandbox restrictions before loading game scripts.
    pub fn new(config: &Config, world: SharedWorld) -> Result<Self> {
        Self::with_help(config, world, crate::help::HelpIndex::load(config)?)
    }

    /// Accept a help index loaded on a blocking worker during asynchronous server startup.
    pub(crate) fn with_help(
        config: &Config,
        world: SharedWorld,
        help: crate::help::HelpIndex,
    ) -> Result<Self> {
        let palette = std::sync::Arc::new(text::Palette::from_config(config)?);
        world.borrow_mut().palette = palette.clone();
        let (lua, budget) = sandbox::create(config)?;
        super::transactions::install(&lua);
        let outbox: Outbox = Default::default();
        let api = packages::register_native(&lua, config, &world, &outbox, &palette)?;
        lua.globals()
            .set("_native", api.clone())
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        sandbox::configure_search(&lua, config)?;
        packages::install_facades(&lua, api)?;
        sandbox::restrict(&lua)?;
        let mut scripts = Self {
            palette,
            help,
            lua,
            world,
            outbox,
            globals: Vec::new(),
            commands: crate::commands::CommandRegistry::new(),
            parents: BTreeMap::new(),
            schedules: Default::default(),
            budget,
            warnings: Vec::new(),
        };
        scripts.load_game_modules(config)?;
        Ok(scripts)
    }
}
