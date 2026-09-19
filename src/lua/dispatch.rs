//! Scoped Lua command invocation; catalog and permission metadata belong to commands.
use super::Scripts;
use crate::world::ObjectId;
use anyhow::Result;
use mlua::{Table, Value};

/// Tracks callback participation when dispatch continues into a read-only native action.
#[derive(Clone, Copy)]
pub struct CommandCallbackInvoked(pub bool);
impl Scripts {
    /// Begin callback participation tracking for one command transaction.
    pub fn reset_command_callbacks(&self) {
        self.lua.set_app_data(CommandCallbackInvoked(false));
    }
    /// Whether a matched Lua handler participated before a native action.
    pub fn command_callbacks_invoked(&self) -> bool {
        self.lua
            .app_data_ref::<CommandCallbackInvoked>()
            .is_some_and(|v| v.0)
    }

    /// Enumerate actual object command sources without running Lua or matching patterns.
    pub fn command_objects(&self, player: ObjectId) -> Vec<ObjectId> {
        crate::commands::sources::sources(&self.world.borrow(), player)
            .into_iter()
            .map(|s| s.object)
            .collect()
    }

    /// Try eligible nearby object commands before global Lua commands.
    pub fn dispatch(
        &self,
        player: ObjectId,
        session: impl Into<Option<u64>>,
        line: &str,
    ) -> Result<bool> {
        let session = session.into();
        if self.dispatch_local(player, session, line)? {
            return Ok(true);
        }
        self.dispatch_global(player, session, line)
    }

    /// Search captured local sources, checking live attachments before invocation.
    pub fn dispatch_local(
        &self,
        player: ObjectId,
        session: Option<u64>,
        line: &str,
    ) -> Result<bool> {
        let sources = crate::commands::sources::dispatch_sources(&self.world.borrow(), player);
        for stage in [
            None,
            Some("location-zone fallback"),
            Some("player-zone fallback"),
        ] {
            let stage_sources: Vec<_> = sources
                .iter()
                .filter(|source| match stage {
                    None => !source.fallback(),
                    Some(stage) => source.stage == stage,
                })
                .copied()
                .collect();
            if self.dispatch_local_sources(player, session, line, &stage_sources)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Use the command's captured source list without adding objects during callbacks.
    ///
    /// Object command scopes accumulate every handled callback, as in the C
    /// dispatcher. Global modules retain their first-handled short circuit.
    pub fn dispatch_local_sources(
        &self,
        player: ObjectId,
        session: Option<u64>,
        line: &str,
        sources: &[crate::commands::sources::CommandSource],
    ) -> Result<bool> {
        let mut handled = false;
        for &source in sources {
            let id = source.object;
            let parent = {
                let world = self.world.borrow();
                if !source.eligible(&world) {
                    continue;
                }
                world.objects[&id].lua_parent.clone()
            };
            handled |= self.dispatch_scope(
                player,
                session,
                line,
                Some(id),
                &crate::commands::CommandScope::Object(parent),
                false,
            )?;
        }
        Ok(handled)
    }

    /// Global Lua modules follow local native handlers.
    pub fn dispatch_global(
        &self,
        player: ObjectId,
        session: Option<u64>,
        line: &str,
    ) -> Result<bool> {
        self.dispatch_scope(
            player,
            session,
            line,
            None,
            &crate::commands::CommandScope::Global,
            true,
        )
    }

    /// Execute eligible captured Lua handlers in declaration order with per-module budgets.
    fn dispatch_scope(
        &self,
        player: ObjectId,
        session: Option<u64>,
        line: &str,
        object: Option<ObjectId>,
        scope: &crate::commands::CommandScope,
        stop_on_handled: bool,
    ) -> Result<bool> {
        use crate::commands::CommandHandler;
        let mut source = None;
        let mut context: Option<Table> = None;
        let mut any_handled = false;
        for definition in self.commands.definitions().filter(|d| {
            &d.scope == scope && matches!(d.handler, crate::commands::CommandHandler::Lua(_))
        }) {
            if let Some(id) = object {
                let world = self.world.borrow();
                if !world.objects.contains_key(&id)
                    || scope
                        != &crate::commands::CommandScope::Object(
                            world.objects[&id].lua_parent.clone(),
                        )
                {
                    break;
                }
            }
            if !definition.lua_access.allows(&self.world.borrow(), player) {
                continue;
            }
            if definition
                .permission
                .denial(
                    &self.world.borrow(),
                    player,
                    crate::access::Context {
                        queue_enabled: self.queue_enabled.get(),
                    },
                )
                .is_some()
            {
                continue;
            }
            if source != Some(definition.source.as_str()) {
                self.reset_callback_budget();
                source = Some(definition.source.as_str());
                let ctx = self.context(Some(player), object, session)?;
                ctx.set(
                    "scope",
                    if object.is_none() {
                        Value::String(
                            self.lua
                                .create_string("global")
                                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
                        )
                    } else {
                        Value::Nil
                    },
                )
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                ctx.set("command", line)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                context = Some(ctx);
            }
            let ctx = context
                .as_ref()
                .expect("context initialized for eligible module")
                .clone();
            if let CommandHandler::Lua(handler) = &definition.handler {
                let (handled, invoked) =
                    super::flows::with_root(&self.lua, &definition.source, || {
                        self.call::<(bool, bool)>(handler, (ctx, line))
                    })
                    .map_err(|error| {
                        anyhow::anyhow!(
                            "{}: command {}: {}",
                            definition.source,
                            definition.declaration.unwrap_or(0),
                            error
                        )
                    })?;
                if invoked {
                    self.lua.set_app_data(CommandCallbackInvoked(true));
                }
                if handled && stop_on_handled {
                    return Ok(true);
                }
                any_handled |= handled;
            }
        }
        Ok(any_handled)
    }
}
