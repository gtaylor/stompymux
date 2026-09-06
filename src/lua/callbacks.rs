//! Lifecycle, appearance and lock callbacks with existing rollback and budget boundaries.
use super::Scripts;
use crate::world::{Kind, ObjectId};
use anyhow::{Context, Result};
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
            ("cause", player.map(|id| id.0)),
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
                ("cause", Some(movement.actor.0)),
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
        if let Err(error) = self.call_event(t, name, ctx) {
            *self.world.borrow_mut() = before;
            self.outbox.borrow_mut().truncate(pending);
            eprintln!("Lua {name} callback failed: {error:#}");
        }
        Ok(())
    }

    /// Invoke an optional event handler with a renewed instruction budget.
    fn call_event(&self, t: &Table, name: &str, ctx: Table) -> Result<()> {
        if let Some(events) = t
            .get::<Option<Table>>("events")
            .map_err(|e| anyhow::anyhow!(e.to_string()))?
            && let Some(f) = events
                .get::<Option<Function>>(name)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?
        {
            self.budget.reset();
            f.call::<()>(ctx)
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
        self.sync_parents()?;
        let parent = self.world.borrow().objects[&location].lua_parent.clone();
        let t = self
            .parents
            .get(&parent)
            .context("appearance parent missing")?;
        self.budget.reset();
        let f = match t
            .get::<Option<Function>>("internal_appearance")
            .map_err(|e| anyhow::anyhow!(e.to_string()))?
        {
            Some(f) => f,
            None => self
                .lua
                .load("return require('object_appearances').render_internal_appearance")
                .eval::<Function>()
                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
        };
        f.call(self.context(Some(player), Some(location), session)?)
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    /// Evaluate teleport policy with explicit enactor, subject and cause identities.
    pub fn movement_lock(
        &self,
        location: ObjectId,
        lock: &str,
        subject: ObjectId,
        movement: &crate::movement::Move,
    ) -> Result<bool> {
        self.sync_parents()?;
        self.budget.reset();
        let f:Function=self.lua.load("return function(t) t.object=mux.world.object(t.object); return mux.world.lock_passes(t) end").eval().map_err(|e|anyhow::anyhow!(e.to_string()))?;
        let ctx = self.context(Some(movement.object), Some(location), movement.session)?;
        ctx.set("lock", lock)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for (key, value) in [
            ("subject", Some(subject.0)),
            ("cause", Some(movement.actor.0)),
            ("source", movement.source.map(|id| id.0)),
            ("destination", Some(movement.destination.0)),
        ] {
            ctx.set(key, value)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        f.call(ctx).map_err(|e| anyhow::anyhow!(e.to_string()))
    }

    /// Evaluate the existing traversal policy for the player and exit.
    pub fn lock(&self, player: ObjectId, exit: ObjectId) -> Result<bool> {
        self.sync_parents()?;
        let f:Function=self.lua.load("return function(o,p) return mux.world.lock_passes({object=mux.world.object(o),enactor=p,lock='traverse'}) end").eval().map_err(|e|anyhow::anyhow!(e.to_string()))?;
        self.budget.reset();
        f.call((exit.0, player.0))
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }
}
