//! Explicit embedding and inspection access to the server-owned Lua runtime.

use super::Scripts;
use crate::{commands::CommandRegistry, help::HelpIndex, text, world::World};
use std::cell::{Ref, RefMut};

/// Read-only live-world handle for diagnostics that outlive a moved `Scripts` owner.
pub struct WorldInspection {
    world: crate::runtime::SharedWorld,
}

impl WorldInspection {
    /// Borrow the current live world without mutation access.
    pub fn world(&self) -> Ref<'_, World> {
        self.world.borrow()
    }
}

impl Scripts {
    /// Borrow the live world for read-only embedding or test inspection.
    pub fn world(&self) -> Ref<'_, World> {
        self.world.borrow()
    }

    /// Borrow the live world for an embedding-owned mutation.
    ///
    /// The caller is responsible for persisting intentional changes. Command
    /// handlers should continue to use transactional operations instead.
    pub fn world_mut(&self) -> RefMut<'_, World> {
        self.world.borrow_mut()
    }

    /// Retain read-only live-world inspection when the runtime owner is moved.
    pub fn inspect_world(&self) -> WorldInspection {
        WorldInspection {
            world: self.world.clone(),
        }
    }

    /// Build an alternate VM over the same live world for integration inspection.
    ///
    /// This does not perform the server's atomic reload and publication process.
    pub fn rebuild_for_inspection(&self, config: &crate::config::Config) -> anyhow::Result<Self> {
        Self::new(config, self.world.clone())
    }

    /// Build an alternate runtime mode over the same live world for tests.
    pub fn from_sources_for_inspection(
        &self,
        config: &crate::config::Config,
        help: HelpIndex,
        sources: std::sync::Arc<super::sources::Sources>,
        mode: super::RuntimeMode,
    ) -> anyhow::Result<Self> {
        Self::from_sources(config, self.world.clone(), help, sources, mode)
    }

    /// Drain transaction-staged messages for delivery or command-level tests.
    pub fn drain_outbox(&self) -> Vec<(crate::world::ObjectId, text::Document)> {
        self.outbox.borrow_mut().drain(..).collect()
    }

    /// Remove the most recently staged message.
    pub fn pop_outbox(&self) -> Option<(crate::world::ObjectId, text::Document)> {
        self.outbox.borrow_mut().pop()
    }

    /// Stage one message while testing output-capacity and rollback boundaries.
    pub fn stage_message_for_inspection(
        &self,
        recipient: crate::world::ObjectId,
        message: impl Into<text::Document>,
    ) {
        self.outbox.borrow_mut().push((recipient, message.into()));
    }

    /// Route one embedding-originated notification through the runtime outbox.
    pub fn send_notification(
        &self,
        config: &crate::config::Config,
        request: crate::notification::Request,
    ) -> anyhow::Result<()> {
        crate::notification::send(&self.world.borrow(), &self.outbox, config, request)
    }

    /// Borrow staged messages for read-only embedding or test inspection.
    pub fn outbox(&self) -> Ref<'_, Vec<(crate::world::ObjectId, text::Document)>> {
        self.outbox.borrow()
    }

    /// Restore staged messages during an embedding-managed rollback.
    pub fn replace_outbox(&self, messages: Vec<(crate::world::ObjectId, text::Document)>) {
        *self.outbox.borrow_mut() = messages;
    }

    /// Inspect the immutable command registry captured for this runtime.
    pub fn commands(&self) -> &CommandRegistry {
        &self.commands
    }

    /// Mutate registry metadata in command-registration integration tests.
    pub fn commands_mut_for_inspection(&mut self) -> &mut CommandRegistry {
        &mut self.commands
    }

    /// Inspect the rendering palette captured for this runtime.
    pub fn palette(&self) -> &text::Palette {
        &self.palette
    }

    /// Clone a live low-level Lua handle for embedding diagnostics and tests.
    ///
    /// This bypasses transactional command APIs and does not provide a sandbox
    /// boundary. Production game behavior should use `call` or `eval_callback`.
    pub fn inspect_lua(&self) -> mlua::Lua {
        self.lua.clone()
    }

    /// Inspect the source snapshot used to construct the runtime.
    pub fn sources(&self) -> &std::sync::Arc<super::sources::Sources> {
        &self.sources
    }

    /// Inspect registered schedules without exposing mutable catalog ownership.
    pub fn schedules(&self) -> &super::schedules::Catalog {
        &self.schedules
    }

    /// Inspect deferred diagnostics discovered during module loading.
    pub fn warnings(&self) -> &[String] {
        &self.warnings
    }

    /// Inspect the immutable game help index.
    pub fn help(&self) -> &HelpIndex {
        &self.help
    }

    /// Publish the live queue prerequisite from an embedding or server owner.
    pub fn set_queue_enabled(&self, enabled: bool) {
        self.queue_enabled.set(enabled);
    }

    /// Discard all staged transaction effects during an embedding rollback probe.
    pub fn rollback_effects_for_inspection(&self) {
        self.effects.rollback();
    }

    /// Drain categorized diagnostics during embedding transaction inspection.
    pub fn drain_records_for_inspection(&self) -> Vec<crate::logging::Record> {
        self.effects.drain_records()
    }

    /// Drain staged transaction log effects during integration inspection.
    pub fn drain_logs_for_inspection(&self) -> Vec<crate::logging::FileRequest> {
        self.effects.drain_logs()
    }
}
