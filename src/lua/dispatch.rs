//! Scoped Lua command invocation; catalog and permission metadata belong to commands.
use super::Scripts;
use crate::world::ObjectId;
use anyhow::Result;
use mlua::{Table, Value};

impl Scripts {
    /// Try eligible nearby object commands before global Lua commands.
    pub fn dispatch(
        &self,
        player: ObjectId,
        session: impl Into<Option<u64>>,
        line: &str,
    ) -> Result<bool> {
        let session = session.into();
        let room = self.world.borrow().objects[&player].location;
        let objects: Vec<ObjectId> = {
            let w = self.world.borrow();
            w.objects
                .values()
                .filter(|o| o.id == player || Some(o.id) == room || o.location == room)
                .filter(|o| {
                    !o.flags.contains(crate::flags::Flag::NoCommand)
                        && !o.flags.contains(crate::flags::Flag::Halted)
                })
                .map(|o| o.id)
                .collect()
        };
        for id in objects {
            let parent = self.world.borrow().objects[&id].lua_parent.clone();
            if self.dispatch_scope(
                player,
                session,
                line,
                Some(id),
                &crate::commands::CommandScope::Object(parent),
            )? {
                return Ok(true);
            }
        }
        self.dispatch_scope(
            player,
            session,
            line,
            None,
            &crate::commands::CommandScope::Global,
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
    ) -> Result<bool> {
        use crate::commands::CommandHandler;
        let mut source = None;
        let mut context: Option<Table> = None;
        for definition in self.commands.definitions().filter(|d| &d.scope == scope) {
            if !definition.permission.allows(&self.world.borrow(), player) {
                continue;
            }
            if source != Some(definition.source.as_str()) {
                self.budget.reset();
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
            if let CommandHandler::Lua(handler) = &definition.handler
                && super::flows::with_root(&self.lua, &definition.source, || {
                    self.call::<bool>(handler, (ctx, line))
                })
                .map_err(|error| {
                    anyhow::anyhow!(
                        "{}: command {}: {}",
                        definition.source,
                        definition.declaration.unwrap_or(0),
                        error
                    )
                })?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
