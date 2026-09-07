//! Captured Lua schedule registrations and runtime-only minute scheduling.
mod cron;
mod inspection;
use super::Scripts;
use crate::{
    flags::Flag,
    state::Generation,
    world::{Kind, ObjectId, World},
};
use anyhow::{Context, Result, ensure};
pub use cron::Cron;
use mlua::{Function, Table, Value};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Immutable declaration identity; functions are captured before any game callback runs.
#[derive(Clone)]
pub struct Definition {
    /// Root-qualified module path.
    pub module: String,
    /// One-based declaration order within the module.
    pub index: usize,
    /// Case-sensitive job identity within the module.
    pub name: String,
    /// Validated UTC expression and original spelling.
    pub cron: Cron,
    handler: Function,
}

/// Includes empty modules so inspection can distinguish unknown modules from no schedules.
#[derive(Default)]
pub struct Catalog {
    modules: BTreeMap<String, Vec<Definition>>,
}

impl Catalog {
    /// Validate dense arrays and declaration types with module/index diagnostics.
    pub(super) fn register(&mut self, module: &str, table: &Table) -> Result<()> {
        let result =
            declarations(module, table).with_context(|| format!("loading {module} schedules"))?;
        self.modules.insert(module.into(), result);
        Ok(())
    }

    /// Enumerate captured metadata in lexical module and declaration order.
    pub fn definitions(&self) -> impl Iterator<Item = &Definition> {
        self.modules.values().flatten()
    }
}

/// Validate the array separately from each declaration so errors identify the failing layer.
fn declarations(module: &str, table: &Table) -> Result<Vec<Definition>> {
    let schedules = table.raw_get::<Value>("schedules").map_err(lua_error)?;
    if schedules.is_nil() {
        return Ok(Vec::new());
    }
    let Value::Table(schedules) = schedules else {
        anyhow::bail!("schedules must be an array");
    };
    let count = schedules.raw_len();
    let mut slots = 0;
    for pair in schedules.clone().pairs::<Value, Value>() {
        let (key, _) = pair.map_err(lua_error)?;
        let valid = match key {
            Value::Integer(i) => i >= 1 && (i as usize) <= count,
            Value::Number(n) => n >= 1.0 && n.fract() == 0.0 && n <= count as f64,
            _ => false,
        };
        ensure!(valid, "schedules must be a dense array");
        slots += 1;
    }
    ensure!(slots == count, "schedules must be a dense array");
    let mut names = BTreeSet::new();
    (1..=count)
        .map(|index| {
            declaration(module, index, &schedules, &mut names)
                .with_context(|| format!("schedule declaration {index}"))
        })
        .collect()
}

/// Capture a handler only after metadata and within-module uniqueness are checked.
fn declaration(
    module: &str,
    index: usize,
    schedules: &Table,
    names: &mut BTreeSet<String>,
) -> Result<Definition> {
    let table: Table = schedules.raw_get(index).map_err(lua_error)?;
    let name = string(&table, "name")?;
    ensure!(!name.is_empty(), "schedule name must not be empty");
    ensure!(names.insert(name.clone()), "duplicate schedule {name:?}");
    let cron = Cron::parse(&string(&table, "cron")?)?;
    let Value::Function(handler) = table.raw_get::<Value>("handler").map_err(lua_error)? else {
        anyhow::bail!("handler must be a function");
    };
    Ok(Definition {
        module: module.into(),
        index,
        name,
        cron,
        handler,
    })
}

/// Avoid Lua's automatic numeric-to-string coercion for declaration metadata.
fn string(table: &Table, key: &str) -> Result<String> {
    let Value::String(value) = table.raw_get::<Value>(key).map_err(lua_error)? else {
        anyhow::bail!("{key} must be a string");
    };
    Ok(value.to_str().map_err(lua_error)?.to_string())
}

/// Preserve Lua error context without requiring its VM handles to be Send.
fn lua_error(error: mlua::Error) -> anyhow::Error {
    anyhow::anyhow!("{error}")
}

/// Pending jobs retain original module identity and provisional-object incarnation.
#[derive(Clone)]
pub struct Job {
    definition: Definition,
    object: Option<(ObjectId, Generation)>,
    /// Earliest execution time, as Unix seconds.
    pub due: i64,
    /// Exclusive execution deadline at the next minute boundary.
    pub expires: i64,
}

impl Job {
    /// Identify failures without evaluating or rendering client-controlled markup.
    pub fn description(&self) -> String {
        format!(
            "{} schedule {:?}{}",
            self.definition.module,
            self.definition.name,
            self.object
                .map_or(String::new(), |(id, _)| format!(" on #{}", id.0))
        )
    }
}

/// Explicit wrapping-u64 version of C lua_schedule_hash on the supported 64-bit server.
pub fn jitter(path: &str, name: &str, object: Option<ObjectId>, minute: i64) -> i64 {
    let mut hash = 2166136261u64;
    for byte in path.bytes().chain(name.bytes()) {
        hash = (hash ^ u64::from(byte)).wrapping_mul(16777619);
    }
    hash ^= object.map_or(-1, |id| id.0) as u64;
    hash ^= minute as u64;
    (hash % 55) as i64
}

/// Timing is deliberately separate from the durable world and callback rollback.
#[derive(Default)]
pub struct Queue {
    high_water: Option<i64>,
    jobs: VecDeque<Job>,
}

impl Queue {
    /// Observe current UTC time without backfilling startup or missed minutes.
    pub fn observe(&mut self, catalog: &Catalog, world: &World, now: i64) {
        self.jobs.retain(|job| now < job.expires);
        let minute = now.div_euclid(60);
        let Some(previous) = self.high_water else {
            self.high_water = Some(minute);
            return;
        };
        if minute <= previous {
            return;
        }
        self.high_water = Some(minute);
        let Some(start) = minute.checked_mul(60) else {
            return;
        };
        let Some(expires) = start.checked_add(60) else {
            return;
        };
        let mut add = |definition: &Definition, object: Option<(ObjectId, Generation)>| {
            if definition.cron.matches(start) {
                let path = definition
                    .module
                    .split_once('/')
                    .expect("registered root")
                    .1;
                self.jobs.push_back(Job {
                    definition: definition.clone(),
                    object,
                    due: start + jitter(path, &definition.name, object.map(|(id, _)| id), minute),
                    expires,
                });
            }
        };
        for definition in catalog
            .definitions()
            .filter(|d| d.module.starts_with("global_logic/"))
        {
            add(definition, None);
        }
        for object in world
            .objects
            .values()
            .filter(|o| o.kind != Kind::Garbage && !o.flags.contains(Flag::Going))
        {
            if let Some(definitions) = catalog
                .modules
                .get(&format!("object_logic/{}", object.lua_parent))
            {
                for definition in definitions {
                    add(definition, Some((object.id, object.generation)));
                }
            }
        }
        // Stable sorting retains collection order for equal due times.
        self.jobs.make_contiguous().sort_by_key(|job| job.due);
    }

    /// Expired jobs are also ready for removal, without waiting for another minute tick.
    pub fn ready(&self, now: i64) -> bool {
        self.jobs.front().is_some_and(|job| now >= job.due)
    }

    /// Consume before invocation; failure never schedules an automatic retry.
    pub fn take_due(&mut self, now: i64) -> Option<Job> {
        while self.jobs.front().is_some_and(|job| now >= job.expires) {
            self.jobs.pop_front();
        }
        if self.ready(now) {
            self.jobs.pop_front()
        } else {
            None
        }
    }

    /// Shutdown and successful reload discard jobs while retaining the minute high-water mark.
    pub fn clear(&mut self) {
        self.jobs.clear();
    }
}

impl Scripts {
    /// Execute one captured callback; the world owner commits it before releasing output.
    pub fn run_schedule(&self, job: &Job) -> Result<bool> {
        if let Some((id, generation)) = job.object
            && !self.world.borrow().objects.get(&id).is_some_and(|o| {
                o.kind != Kind::Garbage
                    && !o.flags.contains(Flag::Going)
                    && o.generation == generation
            })
        {
            return Ok(false);
        }
        self.sync_parents()?;
        self.reset_callback_budget();
        let ctx = self.lua.create_table().map_err(lua_error)?;
        ctx.set(
            "scope",
            if job.object.is_some() {
                "object"
            } else {
                "global"
            },
        )
        .map_err(lua_error)?;
        if let Some((id, _)) = job.object {
            ctx.set("object", id.0).map_err(lua_error)?;
            ctx.set("enactor", 1).map_err(lua_error)?;
            ctx.set("cause", 1).map_err(lua_error)?;
        }
        ctx.set("event", "schedule").map_err(lua_error)?;
        ctx.set("schedule", job.definition.name.as_str())
            .map_err(lua_error)?;
        ctx.set("cron", job.definition.cron.source())
            .map_err(lua_error)?;
        ctx.set("args", self.lua.create_table().map_err(lua_error)?)
            .map_err(lua_error)?;
        self.call::<()>(&job.definition.handler, ctx)
            .with_context(|| job.description())?;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    /// Equal due times retain lexical module order and source declaration order.
    #[test]
    fn equal_due_times_keep_collection_order() {
        let lua = mlua::Lua::new();
        let handler = lua.create_function(|_, (): ()| Ok(())).unwrap();
        let mut catalog = Catalog::default();
        for module in ["global_logic/b.lua", "global_logic/a.lua"] {
            let path = module.split_once('/').unwrap().1;
            let names = (0..1000)
                .map(|n| format!("tick{n}"))
                .filter(|name| jitter(path, name, None, 3) == 8)
                .take(2);
            let definitions = names
                .enumerate()
                .map(|(index, name)| Definition {
                    module: module.into(),
                    index: index + 1,
                    name,
                    cron: Cron::parse("* * * * *").unwrap(),
                    handler: handler.clone(),
                })
                .collect();
            catalog.modules.insert(module.into(), definitions);
        }
        let mut queue = Queue::default();
        queue.observe(&catalog, &World::default(), 120);
        queue.observe(&catalog, &World::default(), 180);
        let mut identities = Vec::new();
        while let Some(job) = queue.take_due(188) {
            assert_eq!(job.due, 188);
            identities.push((job.definition.module, job.definition.index));
        }
        assert_eq!(
            identities,
            [
                ("global_logic/a.lua".into(), 1),
                ("global_logic/a.lua".into(), 2),
                ("global_logic/b.lua".into(), 1),
                ("global_logic/b.lua".into(), 2)
            ]
        );
    }

    #[test]
    fn jitter_matches_c_unsigned_long_64_bit_fixtures() {
        // lua_schedule.c: initial 2166136261, multiply 16777619, xor object/minute, modulo 55.
        assert_eq!(
            jitter("example.lua", "hourly_maintenance", None, 29809440),
            46
        );
        assert_eq!(jitter("z/test.lua", "tick", Some(ObjectId(2)), 3), 36);
        assert_eq!(jitter("a.lua", "global", None, 3), 8);
        assert_eq!(jitter("x.lua", "x", Some(ObjectId(0)), 100), 11);
    }
}
