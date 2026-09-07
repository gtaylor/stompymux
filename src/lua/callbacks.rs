//! Lifecycle, appearance and lock callbacks with existing rollback and budget boundaries.
use super::Scripts;
use crate::world::{Kind, ObjectId};
use anyhow::Result;
use mlua::{Function, Table};

impl Scripts {
    /// Build the existing callback identity and scope fields.
    pub fn context(
        &self,
        player: Option<ObjectId>,
        object: Option<ObjectId>,
        session: Option<u64>,
    ) -> Result<Table> {
        let t = self
            .lua
            .create_table()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for (key, val) in [
            ("enactor", player.map(|id| id.0)),
            (
                "cause",
                super::transactions::cause(&self.lua)
                    .or(player)
                    .map(|id| id.0),
            ),
            ("object", object.map(|id| id.0)),
            ("subject", player.map(|id| id.0)),
            ("descriptor", session.map(|id| id as i64)),
        ] {
            t.set(key, val)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        t.set("scope", if player.is_some() { "object" } else { "global" })
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        Ok(t)
    }

    /// Run a lifecycle event with the default disconnect context.
    pub fn event(&self, name: &str, player: Option<ObjectId>, session: Option<u64>) -> Result<()> {
        self.lifecycle(name, player, session, false, "disconnect")
    }

    /// Invoke global and applicable object lifecycle hooks in their existing order.
    pub fn lifecycle(
        &self,
        name: &str,
        player: Option<ObjectId>,
        session: Option<u64>,
        reconnect: bool,
        reason: &str,
    ) -> Result<()> {
        self.sync_parents()?;
        let enactor = player.or(Some(ObjectId(1)));
        let ctx = self.context(enactor, None, session)?;
        ctx.set("scope", "global")
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        ctx.set("reconnect", reconnect)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        ctx.set("reason", reason)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for t in &self.globals {
            self.lifecycle_callback(t, name, ctx.clone())?;
        }
        let ids: Vec<ObjectId> = if let Some(player) = player {
            let w = self.world.borrow();
            let mut ids = vec![player];
            if let Some(zone) = w.objects[&player].zone {
                ids.push(zone);
                if w.objects[&zone].kind == Kind::Room {
                    ids.extend(
                        w.objects
                            .values()
                            .filter(|o| o.location == Some(zone))
                            .map(|o| o.id),
                    );
                }
            }
            ids.sort();
            ids.dedup();
            ids
        } else {
            self.world.borrow().objects.keys().copied().collect()
        };
        for id in ids {
            let parent = self.world.borrow().objects[&id].lua_parent.clone();
            if let Some(t) = self.parents.get(&parent) {
                let object_ctx = self.context(enactor, Some(id), session)?;
                object_ctx
                    .set("reconnect", reconnect)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                object_ctx
                    .set("reason", reason)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                self.lifecycle_callback(t, name, object_ctx)?;
            }
        }
        Ok(())
    }

    /// Run a location hook with the moved object and initiating actor kept distinct.
    pub fn movement_event(
        &self,
        name: &str,
        location: ObjectId,
        movement: &crate::movement::Move,
    ) -> Result<()> {
        self.sync_parents()?;
        let parent = self.world.borrow().objects[&location].lua_parent.clone();
        if let Some(t) = self.parents.get(&parent) {
            let ctx = self.context(Some(movement.object), Some(location), movement.session)?;
            for (key, value) in [
                (
                    "cause",
                    Some(
                        super::transactions::cause(&self.lua)
                            .unwrap_or(movement.actor)
                            .0,
                    ),
                ),
                ("source", movement.source.map(|id| id.0)),
                ("destination", Some(movement.destination.0)),
            ] {
                ctx.set(key, value)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            }
            self.call_event(t, name, ctx)?;
        }
        Ok(())
    }

    /// Isolate non-server callback failures and discard their staged mutations/output.
    fn lifecycle_callback(&self, t: &Table, name: &str, ctx: Table) -> Result<()> {
        if name.starts_with("on_server_") {
            return self.call_event(t, name, ctx);
        }
        let before = self.world.borrow().clone();
        let pending = self.outbox.borrow().len();
        let flow_effects = crate::lua::flows::snapshot(&self.lua);
        if let Err(error) = self.call_event(t, name, ctx) {
            *self.world.borrow_mut() = before;
            self.outbox.borrow_mut().truncate(pending);
            crate::lua::flows::restore(&self.lua, flow_effects);
            eprintln!("Lua {name} callback failed: {error:#}");
        }
        Ok(())
    }

    /// Invoke an optional event handler with a renewed instruction budget.
    pub(super) fn call_event(&self, t: &Table, name: &str, ctx: Table) -> Result<()> {
        ctx.set("event", name).map_err(|e| anyhow::anyhow!("{e}"))?;
        if ctx
            .get::<mlua::Value>("args")
            .map_err(|e| anyhow::anyhow!("{e}"))?
            == mlua::Value::Nil
        {
            ctx.set(
                "args",
                self.lua
                    .create_table()
                    .map_err(|e| anyhow::anyhow!("{e}"))?,
            )
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        if let Some(events) = t
            .get::<Option<Table>>("events")
            .map_err(|e| anyhow::anyhow!(e.to_string()))?
            && let Some(f) = events
                .get::<Option<Function>>(name)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?
        {
            self.budget.reset();
            self.call::<()>(&f, ctx)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        Ok(())
    }

    /// Render the current location for an initiating session.
    pub fn appearance(&self, player: ObjectId, location: ObjectId, session: u64) -> Result<String> {
        self.appearance_for(player, location, Some(session))
    }

    /// Container modules may omit a renderer; use the copied generic appearance package.
    pub fn appearance_for(
        &self,
        player: ObjectId,
        location: ObjectId,
        session: Option<u64>,
    ) -> Result<String> {
        if let Some(text) =
            self.render_appearance(player, location, session, super::AppearanceMode::Internal)?
        {
            return Ok(text);
        }
        self.budget.reset();
        let f = self
            .lua
            .load("return require('object_appearances').render_internal_appearance")
            .eval::<Function>()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        self.call(&f, self.context(Some(player), Some(location), session)?)
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    /// Evaluate teleport policy with explicit enactor, subject and cause identities.
    pub fn movement_lock(
        &self,
        location: ObjectId,
        lock: LockType,
        subject: ObjectId,
        movement: &crate::movement::Move,
    ) -> Result<bool> {
        Ok(self
            .movement_lock_outcome(location, lock, subject, movement)?
            .passes)
    }

    /// Preserve policy messages for teleport and teleport-out denial.
    pub fn movement_lock_outcome(
        &self,
        location: ObjectId,
        lock: LockType,
        subject: ObjectId,
        movement: &crate::movement::Move,
    ) -> Result<LockOutcome> {
        let ctx = self.context(Some(movement.object), Some(location), movement.session)?;
        ctx.set("lock", lock.key())
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        for (key, value) in [
            ("subject", Some(subject.0)),
            (
                "cause",
                Some(
                    super::transactions::cause(&self.lua)
                        .unwrap_or(movement.actor)
                        .0,
                ),
            ),
            ("source", movement.source.map(|id| id.0)),
            ("destination", Some(movement.destination.0)),
        ] {
            ctx.set(key, value).map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        self.lock_outcome(ctx)
    }

    /// Evaluate the existing traversal policy for the player and exit.
    pub fn lock(&self, player: ObjectId, exit: ObjectId) -> Result<bool> {
        self.sync_parents()?;
        let f:Function=self.lua.load("return function(o,p) return mux.world._lock_result({object=mux.world.object(o),enactor=p,lock='traverse'}).passes end").eval().map_err(|e|anyhow::anyhow!(e.to_string()))?;
        self.budget.reset();
        self.call(&f, (exit.0, player.0))
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }
}

use crate::{LockInvocation, LockOutcome, LockType};
impl Scripts {
    /// Evaluate a full policy result; malformed results cannot retain callback mutations.
    pub fn lock_outcome(&self, context: Table) -> Result<LockOutcome> {
        self.sync_parents()?;
        self.budget.reset();
        let f: Function = self
            .lua
            .load("return mux.world._lock_result")
            .eval()
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let t: Table = self.call(&f, context)?;
        Ok(LockOutcome {
            passes: t.get("passes").map_err(|e| anyhow::anyhow!("{e}"))?,
            enactor_message: t
                .get("enactor_message")
                .map_err(|e| anyhow::anyhow!("{e}"))?,
            other_message: t.get("other_message").map_err(|e| anyhow::anyhow!("{e}"))?,
        })
    }

    /// Denial notifications honor silent Wizards and optional empty messages, then fire on_fail.
    pub fn lock_denied(&self, ctx: Table, result: &LockOutcome, default: &str) -> Result<()> {
        let player = ObjectId(
            ctx.get::<i64>("enactor")
                .map_err(|e| anyhow::anyhow!("{e}"))?,
        );
        let object = ObjectId(
            ctx.get::<i64>("object")
                .map_err(|e| anyhow::anyhow!("{e}"))?,
        );
        if ctx
            .get::<Option<bool>>("silent")
            .map_err(|e| anyhow::anyhow!("{e}"))?
            .unwrap_or(false)
        {
            return Ok(());
        }
        if let Some(message) = result
            .enactor_message
            .as_deref()
            .or(Some(default))
            .filter(|s| !s.is_empty())
        {
            self.outbox.borrow_mut().push((player, message.into()));
        }
        if let Some(message) = result.other_message.as_deref().filter(|s| !s.is_empty()) {
            let w = self.world.borrow();
            let actor = &w.objects[&player];
            if let Some(location) = actor.location {
                for other in w.objects.values().filter(|o| {
                    o.kind == Kind::Player
                        && o.location == Some(location)
                        && o.id != player
                        && o.id != object
                }) {
                    self.outbox
                        .borrow_mut()
                        .push((other.id, format!("{} {message}", actor.name).into()));
                }
            }
        }
        let parent = self
            .world
            .borrow()
            .objects
            .get(&object)
            .map(|o| o.lua_parent.clone());
        if let Some(t) = parent.and_then(|p| self.parents.get(&p)) {
            let event = match ctx
                .get::<Option<String>>("lock")
                .map_err(|e| anyhow::anyhow!("{e}"))?
                .as_deref()
            {
                Some("teleport") => "on_teleport_destination_fail",
                Some("teleport_out") => "on_teleport_out_fail",
                Some("traverse") | Some("take") => "on_fail",
                _ => "",
            };
            if !event.is_empty() {
                self.call_event(t, event, ctx)?;
            }
        }
        Ok(())
    }

    /// Build traversal context with the same silent convention as the C server.
    pub fn traversal(
        &self,
        player: ObjectId,
        exit: ObjectId,
        session: impl Into<Option<u64>>,
    ) -> Result<bool> {
        let ctx = self.context(Some(player), Some(exit), session.into())?;
        ctx.set("lock", "traverse")
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let silent = {
            let w = self.world.borrow();
            crate::flags::is_wizard(&w, player)
                && w.objects[&player].flags.contains(crate::flags::Flag::Dark)
        };
        ctx.set("silent", silent)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        let result = match self.lock_outcome(ctx.clone()) {
            Ok(result) => result,
            Err(error) => {
                eprintln!("Traversal lock on #{} failed: {error:#}", exit.0);
                LockOutcome {
                    passes: false,
                    enactor_message: None,
                    other_message: None,
                }
            }
        };
        if !result.passes {
            self.lock_denied(ctx, &result, "You cannot go that way.")?;
        }
        Ok(result.passes)
    }
}

impl Scripts {
    /// Evaluate a typed native policy with consistent context and fresh callback limits.
    pub fn evaluate_lock(&self, invocation: LockInvocation) -> Result<LockOutcome> {
        let ctx = self.context(
            Some(invocation.enactor),
            Some(invocation.object),
            invocation.descriptor,
        )?;
        for (key, value) in [
            ("cause", invocation.cause.0),
            ("subject", invocation.subject.0),
        ] {
            ctx.set(key, value).map_err(|e| anyhow::anyhow!("{e}"))?;
        }
        ctx.set("lock", invocation.kind.key())
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        ctx.set("silent", invocation.silent)
            .map_err(|e| anyhow::anyhow!("{e}"))?;
        super::transactions::with_descriptor(&self.lua, invocation.descriptor, || {
            self.lock_outcome(ctx)
        })
    }

    /// Run key-aware preferences silently; a failed callback cannot promote a candidate.
    pub fn prefer_matches(
        &self,
        player: ObjectId,
        candidates: Vec<ObjectId>,
        session: Option<u64>,
    ) -> Result<Vec<ObjectId>> {
        let mut passing = Vec::new();
        for id in &candidates {
            let result = self.evaluate_lock(LockInvocation {
                kind: LockType::Match,
                object: *id,
                enactor: player,
                subject: player,
                cause: super::transactions::cause(&self.lua).unwrap_or(player),
                descriptor: session,
                silent: true,
            });
            match result {
                Ok(result) if result.passes => passing.push(*id),
                Err(e) => eprintln!("MATCH lock on #{} failed: {e:#}", id.0),
                _ => {}
            }
        }
        Ok(if passing.is_empty() {
            candidates
        } else {
            passing
        })
    }
}
