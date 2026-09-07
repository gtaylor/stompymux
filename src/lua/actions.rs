//! Transaction-staged action messages and operation-specific object events.
use super::Scripts;
use crate::{LockInvocation, LockOutcome, world::ObjectId};
use anyhow::{Context, Result, ensure};
use mlua::{Function, Table, Value};

/// Live stored content that takes precedence after a message provider executes.
#[derive(Clone, Copy)]
pub(crate) enum ActionContent {
    None,
    Description,
    InternalDescription,
}

/// Explicit action context shared by message and event handlers.
#[derive(Clone, Copy)]
pub struct ObjectAction<'a> {
    pub object: ObjectId,
    pub enactor: ObjectId,
    pub cause: ObjectId,
    pub descriptor: Option<u64>,
    pub source: Option<ObjectId>,
    pub destination: Option<ObjectId>,
    pub operation: &'a str,
    pub silent: bool,
}

impl Scripts {
    fn action_context(&self, action: ObjectAction<'_>) -> Result<Table> {
        let ctx = self.context(Some(action.enactor), Some(action.object), action.descriptor)?;
        ctx.set("cause", action.cause.0)
            .map_err(super::err)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        for (k, v) in [
            ("source", action.source),
            ("destination", action.destination),
        ] {
            ctx.set(k, v.map(|id| id.0))
                .map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        ctx.set("operation", action.operation)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        ctx.set("silent", action.silent)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        ctx.set(
            "args",
            self.lua
                .create_table()
                .map_err(|e| anyhow::anyhow!("{e}"))?,
        )
        .map_err(|e| anyhow::anyhow!("{e}"))?;
        Ok(ctx)
    }

    /// Look up a live attachment without treating no attachment as a missing module.
    fn action_parent(&self, object: ObjectId) -> Result<Option<&Table>> {
        let path = self
            .world
            .borrow()
            .objects
            .get(&object)
            .ok_or_else(|| anyhow::anyhow!("Action object missing"))?
            .lua_parent
            .clone();
        if path.is_empty() {
            return Ok(None);
        }
        Ok(Some(self.parents.get(&path).ok_or_else(|| {
            anyhow::anyhow!("Missing action parent {path}")
        })?))
    }

    /// Determine whether use has an actual message or event implementation.
    pub fn has_action(&self, object: ObjectId, message: &str, event: &str) -> Result<bool> {
        let Some(parent) = self.action_parent(object)? else {
            return Ok(false);
        };
        for (section, key) in [("messages", message), ("events", event)] {
            let value = parent
                .get::<Option<Table>>(section)
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if let Some(table) = value
                && table
                    .get::<Option<Function>>(key)
                    .map_err(|e| anyhow::anyhow!("{e}"))?
                    .is_some()
            {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Evaluate action text and invoke the event in the same enclosing transaction.
    pub fn action_message(
        &self,
        action: ObjectAction<'_>,
        message: &str,
        event: Option<&str>,
        default: Option<&str>,
        others: Option<&str>,
    ) -> Result<()> {
        self.action_message_content(
            action,
            message,
            event,
            ActionContent::None,
            (default, others),
        )
    }

    /// Evaluate providers before reading selected content, retaining their side effects.
    pub(crate) fn action_message_content(
        &self,
        action: ObjectAction<'_>,
        message: &str,
        event: Option<&str>,
        content: ActionContent,
        defaults: (Option<&str>, Option<&str>),
    ) -> Result<()> {
        let (default, others) = defaults;
        let ctx = self.action_context(action)?;
        ctx.set("message", message)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let mut enactor_text = default.map(str::to_string);
        let mut other_text = others.map(str::to_string);
        if let Some(parent) = self.action_parent(action.object)? {
            let messages = parent
                .get::<Option<Table>>("messages")
                .map_err(|e| anyhow::anyhow!("{e}"))?;
            if let Some(messages) = messages
                && let Some(f) = messages
                    .get::<Option<Function>>(message)
                    .map_err(|e| anyhow::anyhow!("{e}"))?
            {
                self.reset_callback_budget();
                let result: Table = self.call(&f, ctx.clone())?;
                for pair in result.pairs::<Value, Value>() {
                    let (key, value) = pair.map_err(|e| anyhow::anyhow!("{e}"))?;
                    let Value::String(key) = key else {
                        anyhow::bail!("Invalid action message field");
                    };
                    let key = key.to_str().map_err(|e| anyhow::anyhow!("{e}"))?;
                    ensure!(
                        matches!(key.as_ref(), "enactor_message" | "other_message"),
                        "Unknown action message field {key}"
                    );
                    ensure!(
                        key.as_ref() != "enactor_message"
                            || !matches!(
                                message,
                                "enter_source" | "leave_destination" | "teleport_source"
                            ),
                        "{message} only accepts other_message"
                    );
                    let Value::String(value) = value else {
                        anyhow::bail!("Action messages must be strings");
                    };
                    ensure!(
                        value.as_bytes().len() < 8192,
                        "Action message exceeds 8191 bytes"
                    );
                    let value = value
                        .to_str()
                        .map_err(|e| anyhow::anyhow!("{e}"))?
                        .to_string();
                    if key.as_ref() == "enactor_message" {
                        enactor_text = Some(value);
                    } else {
                        other_text = Some(value);
                    }
                }
            }
        }
        if !matches!(content, ActionContent::None) {
            let world = self.world.borrow();
            let object = world
                .objects
                .get(&action.object)
                .context("Action object missing")?;
            ensure!(
                object.kind != crate::world::Kind::Garbage,
                "Action object destroyed"
            );
            let stored = match content {
                ActionContent::Description => &object.description,
                ActionContent::InternalDescription => &object.internal_description,
                ActionContent::None => unreachable!(),
            };
            if let Some(text) = stored.as_ref().filter(|text| !text.is_empty()) {
                enactor_text = Some(text.clone());
            }
        }
        self.action_text(
            action.enactor,
            action.object,
            enactor_text.as_deref(),
            if action.silent {
                None
            } else {
                other_text.as_deref()
            },
        )?;
        if !action.silent
            && let Some(event) = event
        {
            self.object_event(action, event)?;
        }
        Ok(())
    }

    /// Route action text through the notification graph, including AUDIBLE containment/exits.
    pub fn action_text(
        &self,
        enactor: ObjectId,
        excluded: ObjectId,
        direct: Option<&str>,
        others: Option<&str>,
    ) -> Result<()> {
        if let Some(text) = direct.filter(|v| !v.is_empty()) {
            self.outbox.borrow_mut().push((enactor, text.into()));
            let has_exit = self.world.borrow().objects.values().any(|object| {
                object.kind == crate::world::Kind::Exit
                    && object.location == Some(enactor)
                    && object.flags.contains(crate::flags::Flag::Audible)
                    && object.destination.is_some_and(|to| to != enactor)
            });
            if has_exit {
                crate::notification::send(
                    &self.world.borrow(),
                    &self.outbox,
                    &super::configuration(&self.lua),
                    crate::notification::Request {
                        target: enactor,
                        sender: enactor,
                        document: text.into(),
                        policy: crate::notification::Policy::AUDIBLE_EXITS,
                        exclusions: None,
                    },
                )?;
            }
        }
        if let Some(text) = others.filter(|v| !v.is_empty()) {
            let world = self.world.borrow();
            let actor = world
                .objects
                .get(&enactor)
                .ok_or_else(|| anyhow::anyhow!("Action enactor missing"))?;
            if let Some(location) = actor.location {
                crate::notification::send(
                    &world,
                    &self.outbox,
                    &super::configuration(&self.lua),
                    crate::notification::Request {
                        target: location,
                        sender: enactor,
                        document: format!("{} {text}", actor.name).into(),
                        policy: crate::notification::Policy::ROOM,
                        exclusions: Some(vec![enactor, excluded]),
                    },
                )?;
            }
        }
        Ok(())
    }

    /// Invoke a single attached event with the operation's original identities.
    pub fn object_event(&self, action: ObjectAction<'_>, event: &str) -> Result<()> {
        let Some(parent) = self.action_parent(action.object)? else {
            return Ok(());
        };
        let ctx = self.action_context(action)?;
        ctx.set("event", event)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        self.call_event(parent, event, ctx)
    }

    /// Invoke a native lock-failure event without leaking the lock-only subject field.
    fn failure_event(&self, action: ObjectAction<'_>, event: &str) -> Result<()> {
        let Some(parent) = self.action_parent(action.object)? else {
            return Ok(());
        };
        let ctx = self.action_context(action)?;
        ctx.set("subject", Value::Nil)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        self.call_event(parent, event, ctx)
    }

    /// Emit denial text and only the failure event selected by this action.
    pub fn deny_action(
        &self,
        invocation: LockInvocation,
        outcome: &LockOutcome,
        default: &str,
        event: Option<&str>,
    ) -> Result<()> {
        if invocation.silent {
            return Ok(());
        }
        self.action_text(
            invocation.enactor,
            invocation.object,
            outcome.enactor_message.as_deref().or(Some(default)),
            outcome.other_message.as_deref(),
        )?;
        if let Some(event) = event {
            self.failure_event(
                ObjectAction {
                    object: invocation.object,
                    enactor: invocation.enactor,
                    cause: invocation.cause,
                    descriptor: invocation.descriptor,
                    source: None,
                    destination: None,
                    operation: invocation.kind.key(),
                    silent: false,
                },
                event,
            )?;
        }
        Ok(())
    }
}

impl Scripts {
    /// Apply departure/arrival actions with the captured cause and hearing state.
    pub fn transition_action_context(
        &self,
        movement: &crate::movement::Move,
        entering: bool,
        hush: bool,
        context: TransitionContext,
    ) -> Result<()> {
        use crate::flags::{self, Flag};
        let location = self
            .world
            .borrow()
            .objects
            .get(&movement.object)
            .ok_or_else(|| anyhow::anyhow!("Moved object missing"))?
            .location;
        if location
            == if entering {
                movement.source
            } else {
                Some(movement.destination)
            }
        {
            return Ok(());
        }
        let Some(location) = location else {
            return Ok(());
        };
        let (quiet, announce) = {
            let w = self.world.borrow();
            let thing = w
                .objects
                .get(&movement.object)
                .ok_or_else(|| anyhow::anyhow!("Moved object missing"))?;
            let loc = w
                .objects
                .get(&location)
                .ok_or_else(|| anyhow::anyhow!("Movement location missing"))?;
            let dark_wizard = flags::is_wizard(&w, thing.id) && thing.flags.contains(Flag::Dark);
            let hear = context.hear;
            let visible = !thing.flags.contains(Flag::Dark) && !loc.flags.contains(Flag::Dark);
            let quiet =
                hush || !(flags::is_wizard(&w, location) || visible || (hear && !dark_wizard));
            (
                quiet,
                !quiet
                    && if entering {
                        hear && !dark_wizard
                    } else {
                        visible || (hear && !dark_wizard)
                    },
            )
        };
        let action = ObjectAction {
            object: location,
            enactor: movement.object,
            cause: context.cause,
            descriptor: movement.session,
            source: if !entering {
                Some(location)
            } else {
                movement.source
            },
            destination: if entering {
                Some(location)
            } else {
                Some(movement.destination)
            },
            operation: "move",
            silent: quiet,
        };
        self.action_message(
            action,
            if entering { "enter" } else { "leave" },
            Some(if entering { "on_enter" } else { "on_leave" }),
            None,
            None,
        )?;
        if !quiet {
            let other = if entering {
                movement.source
            } else {
                Some(movement.destination)
            };
            if let Some(other) = other {
                self.action_message(
                    ObjectAction {
                        object: other,
                        ..action
                    },
                    if entering {
                        "leave_destination"
                    } else {
                        "enter_source"
                    },
                    None,
                    None,
                    None,
                )?;
            }
        }
        if announce {
            self.action_text(
                movement.object,
                context.excluded,
                None,
                Some(if entering {
                    "has arrived."
                } else {
                    "has left."
                }),
            )?;
        }
        Ok(())
    }
}

/// Values captured before any movement messages or callbacks execute.
#[derive(Clone, Copy)]
pub struct TransitionContext {
    pub cause: ObjectId,
    pub hear: bool,
    pub excluded: ObjectId,
}
