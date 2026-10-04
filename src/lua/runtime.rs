//! Construct the Lua owner in dependency order before evaluating editable game modules.
use super::{Scripts, packages, sandbox};
use crate::{
    config::Config,
    runtime::{Effects, Outbox, SharedWorld},
    text,
};
use anyhow::Result;
use std::{collections::BTreeMap, sync::Arc};

/// Checking VMs cannot access live world or session APIs.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum RuntimeMode {
    Live,
    Checking,
    /// Separate test VM with live services, without loading game root modules.
    Testing,
}

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
        let sources = Arc::new(super::sources::Sources::read(config)?);
        Self::from_sources(config, world, help, sources, RuntimeMode::Live)
    }

    /// Build an unpublished VM from a source snapshot without filesystem access.
    pub fn from_sources(
        config: &Config,
        world: SharedWorld,
        help: crate::help::HelpIndex,
        sources: Arc<super::sources::Sources>,
        mode: RuntimeMode,
    ) -> Result<Self> {
        Self::from_sources_with(config, world, help, sources, mode, |_| Ok(()))
    }

    /// Seed runtime-only session snapshots before evaluating candidate modules.
    pub(crate) fn from_sources_with(
        config: &Config,
        world: SharedWorld,
        help: crate::help::HelpIndex,
        sources: Arc<super::sources::Sources>,
        mode: RuntimeMode,
        setup: impl FnOnce(&Scripts) -> Result<()>,
    ) -> Result<Self> {
        let palette = std::sync::Arc::new(text::Palette::from_config(config)?);
        world.borrow_mut().palette = palette.clone();
        let (lua, budget) = sandbox::create(config)?;
        lua.set_app_data(std::sync::Arc::new(config.clone()));
        lua.set_app_data(mode);
        let event_telemetry = std::rc::Rc::new(std::cell::Cell::new(crate::EventTelemetry {
            process_start: crate::clock::wall_time(),
            ticks: 0,
        }));
        lua.set_app_data(event_telemetry.clone());
        super::transactions::install(&lua);
        super::testing::install(&lua).map_err(|e| anyhow::anyhow!(e.to_string()))?;
        lua.set_app_data(sources.clone());
        let outbox: Outbox = Default::default();
        let effects = Effects::new(config, &outbox);
        lua.set_app_data(effects.clone());
        let flows = super::flows::Engine::install(&lua, &world, &effects, mode);
        let api = packages::register_native(&lua, config, &world, &outbox, &effects, &palette)?;
        sandbox::configure_search(&lua, &sources)?;
        packages::install_facades(&lua, api.clone())?;
        sandbox::restrict(&lua)?;
        if mode == RuntimeMode::Checking {
            packages::restrict_checking(&lua, &api)?;
        }
        let mut scripts = Self {
            event_telemetry,
            flows,
            sources,
            palette,
            help,
            lua,
            world,
            outbox,
            effects,
            globals: Vec::new(),
            commands: crate::commands::CommandRegistry::new(),
            queue_enabled: std::cell::Cell::new(true),
            progress: tokio::sync::watch::Sender::new(Default::default()),
            parents: BTreeMap::new(),
            schedules: Default::default(),
            budget,
            warnings: Vec::new(),
        };
        scripts.publish_services();
        setup(&scripts)?;
        if mode != RuntimeMode::Testing {
            scripts.load_game_modules()?;
            scripts.commands.configure_access(config)?;
        } else {
            super::testing::install_parents(&scripts)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        if mode == RuntimeMode::Checking {
            super::testing::check(&scripts)?;
        }
        scripts.publish_services();
        scripts.flows.ready();
        crate::configure_battle_perception(
            &mut scripts.world.borrow_mut(),
            config.battletech.sensor_range,
        );
        crate::configure_battle_reactor_policy(
            &mut scripts.world.borrow_mut(),
            config.battletech.stackpole != 0,
            config.battletech.explode_reactor > 1,
        );
        Ok(scripts)
    }
}
