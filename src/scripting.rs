//! LuaJIT compatibility bindings and bounded world callbacks.
use crate::{config::Config, text, world::*};
use anyhow::{Context, Result, ensure};
use mlua::{Function, HookTriggers, Lua, LuaSerdeExt, Table, Value, VmState};
use std::{
    cell::{Cell, RefCell},
    collections::BTreeMap,
    path::{Path, PathBuf},
    rc::Rc,
};
pub type SharedWorld = Rc<RefCell<World>>;
pub type Outbox = Rc<RefCell<Vec<(ObjectId, String)>>>;
pub struct Scripts {
    pub lua: Lua,
    pub world: SharedWorld,
    pub outbox: Outbox,
    globals: Vec<Table>,
    parents: BTreeMap<String, Table>,
    budget: Rc<Cell<usize>>,
    instruction_limit: usize,
    pub warnings: Vec<String>,
}
fn err(e: impl std::fmt::Display) -> mlua::Error {
    mlua::Error::RuntimeError(e.to_string())
}
fn files(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for e in std::fs::read_dir(dir)? {
        let p = e?.path();
        if p.is_dir() {
            paths.extend(files(&p)?)
        } else if p.extension().is_some_and(|s| s == "lua") {
            paths.push(p);
        }
    }
    paths.sort();
    Ok(paths)
}
impl Scripts {
    pub fn new(config: &Config, world: SharedWorld) -> Result<Self> {
        let lua = Lua::new();
        lua.set_memory_limit(config.lua.memory_limit)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        // JIT traces can bypass instruction hooks. Keep the LuaJIT runtime, with tracing off,
        // so an operator-supplied script cannot monopolize the world owner indefinitely.
        lua.load("if jit then jit.off() end")
            .exec()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let budget = Rc::new(Cell::new(config.lua.instruction_limit));
        let b = budget.clone();
        // Sampling granularity is an implementation detail; the budget is configured.
        const HOOK_QUANTUM: usize = 1000;
        let quantum = config.lua.instruction_limit.min(HOOK_QUANTUM);
        lua.set_global_hook(
            HookTriggers::new().every_nth_instruction(quantum as u32),
            move |_, _| {
                let n = b.get();
                if n <= quantum {
                    return Err(err("Lua instruction budget exceeded"));
                }
                b.set(n - quantum);
                Ok(VmState::Continue)
            },
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let outbox: Outbox = Default::default();
        let api = lua
            .create_table()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        macro_rules! bind {
            ($name:literal,$f:expr) => {
                api.set(
                    $name,
                    lua.create_function($f)
                        .map_err(|e| anyhow::anyhow!(e.to_string()))?,
                )
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            };
        }
        let w = world.clone();
        bind!("get", move |lua, (id, key): (i64, String)| {
            let w = w.borrow();
            let o = w
                .objects
                .get(&ObjectId(id))
                .ok_or_else(|| err("object does not exist"))?;
            match key.as_str() {
                "name" => lua.to_value(&o.name),
                "description" => lua.to_value_with(
                    &o.description,
                    mlua::SerializeOptions::new().serialize_none_to_null(false),
                ),
                "type" => Ok(Value::Integer(o.kind.code())),
                "location" => lua.to_value_with(
                    &o.location,
                    mlua::SerializeOptions::new().serialize_none_to_null(false),
                ),
                "home" => lua.to_value_with(
                    &o.home,
                    mlua::SerializeOptions::new().serialize_none_to_null(false),
                ),
                "affiliation" => lua.to_value_with(
                    &o.affiliation,
                    mlua::SerializeOptions::new().serialize_none_to_null(false),
                ),
                "flags" => lua.to_value(&o.flags),
                _ => Err(err("unsupported object field")),
            }
        });
        let w = world.clone();
        bind!("set", move |lua, (id, key, v): (i64, String, Value)| {
            let mut w = w.borrow_mut();
            let o = w
                .objects
                .get_mut(&ObjectId(id))
                .ok_or_else(|| err("object does not exist"))?;
            match key.as_str() {
                "name" => o.name = lua.from_value(v)?,
                "description" => o.description = lua.from_value(v)?,
                "home" => o.home = lua.from_value(v)?,
                "location" => o.location = lua.from_value(v)?,
                _ => return Err(err("unsupported object mutation")),
            }
            Ok(())
        });
        let w = world.clone();
        let c = config.clone();
        bind!("create", move |_, t: Table| {
            let mut w = w.borrow_mut();
            let kind = Kind::from_code(t.get("type")?).map_err(err)?;
            if kind == Kind::Player || kind == Kind::Garbage {
                return Err(err("Use account registration to create players"));
            }
            let id = w.create(&c, t.get("name")?, kind);
            let o = w.objects.get_mut(&id).unwrap();
            for key in ["location", "zone", "destination"] {
                let value: Option<i64> = t.get(key)?;
                match key {
                    "location" => o.location = value.map(ObjectId),
                    "zone" => o.zone = value.map(ObjectId),
                    _ => o.destination = value.map(ObjectId),
                }
            }
            Ok(id.0)
        });
        let w = world.clone();
        bind!("contents", move |lua,
                                (id, types, viewer): (
            i64,
            Vec<i64>,
            Option<i64>
        )| {
            let w = w.borrow();
            lua.to_value(
                &w.objects
                    .values()
                    .filter(|o| {
                        o.location == Some(ObjectId(id))
                            && (types.is_empty() || types.contains(&o.kind.code()))
                            && viewer.is_none_or(|v| w.visible(o, ObjectId(v)))
                    })
                    .map(|o| o.id.0)
                    .collect::<Vec<_>>(),
            )
        });
        api.set(
            "flags",
            lua.create_userdata(crate::flags::LuaFlags)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let w = world.clone();
        bind!(
            "has_flag",
            move |_, (id, flag): (i64, mlua::AnyUserData)| {
                let flag = *flag.borrow::<crate::flags::Flag>()?;
                let world = w.borrow();
                let o = world
                    .objects
                    .get(&ObjectId(id))
                    .filter(|o| o.kind != Kind::Garbage)
                    .ok_or_else(|| err("object does not exist"))?;
                Ok(o.flags.contains(flag))
            }
        );
        let w = world.clone();
        bind!("flag", move |_,
                            (id, flag, add): (
            i64,
            mlua::AnyUserData,
            bool
        )| {
            let flag = *flag.borrow::<crate::flags::Flag>()?;
            crate::flags::change(&mut w.borrow_mut(), ObjectId(1), ObjectId(id), flag, add)
                .map_err(err)
        });
        api.set(
            "powers",
            lua.create_userdata(crate::powers::LuaPowers)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?,
        )
        .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let w = world.clone();
        bind!(
            "has_power",
            move |_, (id, power): (i64, mlua::AnyUserData)| {
                let power = *power.borrow::<crate::powers::Power>()?;
                let world = w.borrow();
                let o = world
                    .objects
                    .get(&ObjectId(id))
                    .filter(|o| o.kind != Kind::Garbage)
                    .ok_or_else(|| err("object does not exist"))?;
                Ok(o.powers.contains(power))
            }
        );
        let w = world.clone();
        bind!("power", move |_,
                             (id, power, value): (
            i64,
            mlua::AnyUserData,
            bool
        )| {
            let power = *power.borrow::<crate::powers::Power>()?;
            crate::powers::change(&mut w.borrow_mut(), ObjectId(1), ObjectId(id), power, value)
                .map_err(err)
        });
        let w = world.clone();
        bind!("entries", move |lua, (id, ns): (i64, String)| {
            let w = w.borrow();
            let o = w
                .objects
                .get(&ObjectId(id))
                .ok_or_else(|| err("object missing"))?;
            let result = lua.create_table()?;
            if let Some(entries) = o.state.get(&ns) {
                for (i, (key, value)) in entries.iter().enumerate() {
                    let t = lua.create_table()?;
                    t.set("key", key.clone())?;
                    t.set("value", lua.to_value(value)?)?;
                    result.set(i + 1, t)?;
                }
            }
            Ok(result)
        });
        let w = world.clone();
        let c = config.clone();
        bind!("state_set", move |lua,
                                 (id, ns, key, v): (
            i64,
            String,
            String,
            Value
        )| {
            if ns.is_empty() || key.is_empty() || ns.len() > 255 || key.len() > 255 {
                return Err(err("invalid state namespace/key"));
            }
            let value: Option<Scalar> = if v.is_nil() {
                None
            } else {
                Some(lua.from_value(v)?)
            };
            if let Some(Scalar::String(s)) = &value
                && s.len() > c.lua.state_value_limit
            {
                return Err(err("Lua state string limit exceeded"));
            }
            let mut w = w.borrow_mut();
            let o = w
                .objects
                .get_mut(&ObjectId(id))
                .ok_or_else(|| err("object missing"))?;
            let mut state = o.state.clone();
            let entries = state.entry(ns).or_default();
            if let Some(v) = value {
                entries.insert(key, v);
            } else {
                entries.remove(&key);
            }
            if state.values().map(|v| v.len()).sum::<usize>() > c.lua.state_entry_limit
                || serde_json::to_vec(&state).map_err(err)?.len() > c.lua.state_object_limit
            {
                return Err(err("Lua object state limit exceeded"));
            }
            o.state = state;
            Ok(())
        });
        let o = outbox.clone();
        let output_settings = config.lua.clone();
        let message_limit = config.runtime.output_message_limit;
        bind!("pemit", move |_, (id, s): (i64, String)| {
            let mut out = o.borrow_mut();
            if s.len() > message_limit
                || out.len() >= output_settings.output_entry_limit
                || out.iter().map(|(_, s)| s.len()).sum::<usize>() + s.len()
                    > output_settings.output_byte_limit
            {
                return Err(err("Lua output limit exceeded"));
            }
            out.push((ObjectId(id), s));
            Ok(())
        });
        let c = config.clone();
        bind!("config", move |lua, key: String| {
            if let Some(v) = c.effective_value(&key) {
                lua.to_value(v)
            } else {
                Ok(Value::Nil)
            }
        });
        let w = world.clone();
        bind!("channel", move |_,
                               (name, object, flag): (
            String,
            Option<i64>,
            Option<i64>
        )| {
            let mut w = w.borrow_mut();
            let channel = w.channels.entry(name.clone()).or_insert(Channel {
                name,
                object: None,
                flags: 0,
                messages: 0,
            });
            if let Some(id) = object {
                channel.object = Some(ObjectId(id));
            }
            if let Some(f) = flag {
                channel.flags |= f;
            }
            Ok(())
        });
        bind!("markup", |_, s: String| Ok(text::markup(&s)));
        bind!("width", |_, s: String| Ok(text::width(&s)));
        bind!("truncate", |_, (s, n): (String, usize)| Ok(text::truncate(
            &s, n
        )));
        bind!("strip", |_, s: String| Ok(text::plain(&s)));
        bind!("style", |_, (s, t): (String, Table)| Ok(text::style(
            &s,
            &t.get::<Option<String>>("foreground")?.unwrap_or_default()
        )));
        lua.globals()
            .set("_native", api)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let packages = config.lua_dir().join("packages").canonicalize()?;
        let package: Table = lua
            .globals()
            .get("package")
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        package
            .set("path", format!("{}/?.lua", packages.display()))
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        package
            .set("cpath", "")
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        lua.load(include_str!("scripting_api.lua"))
            .exec()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        // Scripts are game extensions, not operating-system extensions.
        for name in ["io", "os", "debug", "ffi", "jit", "dofile", "loadfile"] {
            lua.globals()
                .set(name, Value::Nil)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        let mut s = Self {
            lua,
            world,
            outbox,
            globals: Vec::new(),
            parents: BTreeMap::new(),
            budget,
            instruction_limit: config.lua.instruction_limit,
            warnings: Vec::new(),
        };
        let dir = config.lua_dir();
        for p in files(&dir.join("object_logic"))? {
            let name = p
                .strip_prefix(dir.join("object_logic"))?
                .to_string_lossy()
                .replace('\\', "/");
            let t = s.load_module(&p)?;
            s.parents.insert(name, t);
        }
        let table = s
            .lua
            .create_table()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for (name, t) in &s.parents {
            table
                .set(name.as_str(), t.clone())
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        s.lua
            .globals()
            .set("_parents", table)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        let parent_map = s
            .lua
            .create_table()
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for o in s.world.borrow().objects.values() {
            if !o.lua_parent.is_empty() {
                ensure!(
                    s.parents.contains_key(&o.lua_parent),
                    "missing Lua parent {}",
                    o.lua_parent
                );
                parent_map
                    .set(o.id.0, o.lua_parent.as_str())
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            }
        }
        s.lua
            .globals()
            .set("_object_parents", parent_map)
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for p in files(&dir.join("global_logic"))? {
            let t = s.load_module(&p)?;
            if t.contains_key("schedules")
                .map_err(|e| anyhow::anyhow!(e.to_string()))?
            {
                s.warnings
                    .push(format!("{}: schedules deferred", p.display()));
            }
            s.globals.push(t);
        }
        Ok(s)
    }
    fn load_module(&self, p: &Path) -> Result<Table> {
        self.budget.set(self.instruction_limit);
        self.lua
            .load(std::fs::read_to_string(p)?)
            .set_name(p.to_string_lossy())
            .eval()
            .map_err(|e| anyhow::anyhow!(e.to_string()))
            .with_context(|| format!("loading {}", p.display()))
    }
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
    pub fn sync_parents(&self) -> Result<()> {
        let t: Table = self
            .lua
            .globals()
            .get("_object_parents")
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        for o in self.world.borrow().objects.values() {
            t.set(o.id.0, o.lua_parent.as_str())
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
        }
        Ok(())
    }
    pub fn event(&self, name: &str, player: Option<ObjectId>, session: Option<u64>) -> Result<()> {
        self.lifecycle(name, player, session, false, "disconnect")
    }
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
    fn call_event(&self, t: &Table, name: &str, ctx: Table) -> Result<()> {
        if let Some(events) = t
            .get::<Option<Table>>("events")
            .map_err(|e| anyhow::anyhow!(e.to_string()))?
            && let Some(f) = events
                .get::<Option<Function>>(name)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?
        {
            self.budget.set(self.instruction_limit);
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
        self.budget.set(self.instruction_limit);
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
        self.budget.set(self.instruction_limit);
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
    pub fn lock(&self, player: ObjectId, exit: ObjectId) -> Result<bool> {
        self.sync_parents()?;
        let f:Function=self.lua.load("return function(o,p) return mux.world.lock_passes({object=mux.world.object(o),enactor=p,lock='traverse'}) end").eval().map_err(|e|anyhow::anyhow!(e.to_string()))?;
        self.budget.set(self.instruction_limit);
        f.call((exit.0, player.0))
            .map_err(|e| anyhow::anyhow!(e.to_string()))
    }
    pub fn dispatch(&self, player: ObjectId, session: u64, line: &str) -> Result<bool> {
        let dispatch: Function = self
            .lua
            .globals()
            .get("_dispatch")
            .map_err(|e| anyhow::anyhow!(e.to_string()))?;
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
            if let Some(t) = self.parents.get(&parent) {
                let ctx = self.context(Some(player), Some(id), Some(session))?;
                ctx.set("scope", Value::Nil)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                ctx.set("command", line)
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?;
                self.budget.set(self.instruction_limit);
                if dispatch
                    .call::<bool>((t.clone(), ctx, line))
                    .map_err(|e| anyhow::anyhow!(e.to_string()))?
                {
                    return Ok(true);
                }
            }
        }
        for t in &self.globals {
            let ctx = self.context(Some(player), None, Some(session))?;
            ctx.set("scope", "global")
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            ctx.set("command", line)
                .map_err(|e| anyhow::anyhow!(e.to_string()))?;
            self.budget.set(self.instruction_limit);
            if dispatch
                .call::<bool>((t.clone(), ctx, line))
                .map_err(|e| anyhow::anyhow!(e.to_string()))?
            {
                return Ok(true);
            }
        }
        Ok(false)
    }
}
