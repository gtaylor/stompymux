//! A callback service view without owning its Lua VM, used by native world operations.
use super::*;
#[derive(Clone)]
struct View {
    world: SharedWorld,
    outbox: Outbox,
    effects: crate::runtime::Effects,
    flows: flows::Engine,
    sources: std::sync::Arc<sources::Sources>,
    palette: std::sync::Arc<text::Palette>,
    help: crate::help::HelpIndex,
    parents: BTreeMap<String, Table>,
    globals: Vec<Table>,
    budget: sandbox::InstructionBudget,
}
impl Scripts {
    pub(crate) fn publish_services(&self) {
        self.lua.set_app_data(self.effects.clone());
        self.lua.set_app_data(View {
            world: self.world.clone(),
            outbox: self.outbox.clone(),
            effects: self.effects.clone(),
            flows: self.flows.clone(),
            sources: self.sources.clone(),
            palette: self.palette.clone(),
            help: self.help.clone(),
            parents: self.parents.clone(),
            globals: self.globals.clone(),
            budget: self.budget.clone(),
        });
    }
    pub(crate) fn services(lua: &Lua) -> mlua::Result<Self> {
        let v = lua
            .app_data_ref::<View>()
            .ok_or_else(|| {
                packages::error::failure(
                    "mux.state.unavailable",
                    "runtime services are not initialized",
                )
            })?
            .clone();
        Ok(Self {
            lua: lua.clone(),
            world: v.world,
            outbox: v.outbox,
            effects: v.effects,
            flows: v.flows,
            sources: v.sources,
            palette: v.palette,
            help: v.help,
            parents: v.parents,
            globals: v.globals,
            budget: v.budget,
            commands: crate::commands::CommandRegistry::new(),
            queue_enabled: std::cell::Cell::new(true),
            schedules: Default::default(),
            warnings: Vec::new(),
        })
    }
}
