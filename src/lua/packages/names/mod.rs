//! Native bindings for `mux.names`: seeded mercenary outfit, person and callsign names.
//!
//! The package is pure. It reads the word lists once at registration, from
//! `data/names` under the game directory when that directory exists and from the lists
//! embedded in `stompymux-namegen` otherwise, and it stays available during `@lua/check`.
use crate::config::Config;
use anyhow::{Context, Result};
use mlua::{Lua, MetaMethod, Table, UserData, UserDataMethods, Value};
use std::rc::Rc;
use stompymux_namegen::{Culture, Flavor, Lists, NameSpec, OutfitName, PersonName};

/// Seeds must round-trip through LuaJIT numbers, which hold 53 integer bits exactly.
const SEED_BITS: u32 = 53;

/// The largest roster or callsign batch one call returns.
const MAX_COUNT: i64 = 500;

/// Typed culture constant from `mux.names.cultures`.
#[derive(Clone, Copy, PartialEq, Eq)]
struct CultureValue(Culture);

/// Typed flavor constant from `mux.names.flavors`.
#[derive(Clone, Copy, PartialEq, Eq)]
struct FlavorValue(Flavor);

/// Immutable lookup namespace for culture constants, keyed by uppercase name.
struct CultureNamespace;

/// Immutable lookup namespace for flavor constants, keyed by uppercase name.
struct FlavorNamespace;

fn immutable(what: &str) -> mlua::Error {
    super::error::failure("mux.arg.invalid", format!("{what} values are immutable"))
}

impl UserData for CultureValue {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::ToString, |_, value, ()| Ok(value.0.name()));
        m.add_meta_function(MetaMethod::Eq, |_, (left, right): (Value, Value)| {
            super::error::typed_eq::<Self, _>("mux.names.culture", left, right, |a, b| a == b)
        });
        m.add_meta_method(MetaMethod::NewIndex, |_, _, _: (Value, Value)| {
            Err::<(), _>(immutable("culture"))
        });
    }
}

impl UserData for FlavorValue {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::ToString, |_, value, ()| Ok(value.0.name()));
        m.add_meta_function(MetaMethod::Eq, |_, (left, right): (Value, Value)| {
            super::error::typed_eq::<Self, _>("mux.names.flavor", left, right, |a, b| a == b)
        });
        m.add_meta_method(MetaMethod::NewIndex, |_, _, _: (Value, Value)| {
            Err::<(), _>(immutable("flavor"))
        });
    }
}

/// The constant key for a culture or flavor: its snake_case name in upper case.
fn constant_key(name: &str) -> String {
    name.to_ascii_uppercase()
}

/// Resolve an uppercase namespace key against a catalog, raising the C-style lookup error.
fn lookup<T: Copy>(
    lua: &Lua,
    key: Value,
    what: &str,
    catalog: &[T],
    name: impl Fn(T) -> &'static str,
) -> mlua::Result<T> {
    let Value::String(key) = key else {
        return Err(super::error::argument_failure(
            lua,
            2,
            "mux.arg.invalid",
            format!("{what} constant name must be a string"),
        ));
    };
    let key = key.to_str()?;
    catalog
        .iter()
        .copied()
        .find(|item| constant_key(name(*item)) == *key)
        .ok_or_else(|| {
            super::error::argument_failure(
                lua,
                2,
                "mux.arg.invalid",
                format!("unknown {what} constant '{}'", &*key),
            )
        })
}

impl UserData for CultureNamespace {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Index, |lua, _, key: Value| {
            let culture = lookup(lua, key, "culture", Culture::ALL, Culture::name)?;
            lua.create_userdata(CultureValue(culture))
                .map(Value::UserData)
        });
        m.add_meta_method(MetaMethod::NewIndex, |_, _, _: (Value, Value)| {
            Err::<(), _>(immutable("culture"))
        });
    }
}

impl UserData for FlavorNamespace {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Index, |lua, _, key: Value| {
            let flavor = lookup(lua, key, "flavor", Flavor::ALL, Flavor::name)?;
            lua.create_userdata(FlavorValue(flavor))
                .map(Value::UserData)
        });
        m.add_meta_method(MetaMethod::NewIndex, |_, _, _: (Value, Value)| {
            Err::<(), _>(immutable("flavor"))
        });
    }
}

/// Read the optional spec table (argument 1): `seed`, `culture` and `flavor`, each optional.
fn spec_arg(lua: &Lua, values: &mlua::MultiValue) -> mlua::Result<NameSpec> {
    let invalid =
        |message: String| super::error::argument_failure(lua, 1, "mux.arg.invalid", message);
    let table = match values.front() {
        None | Some(Value::Nil) => return Ok(NameSpec::default()),
        Some(Value::Table(table)) => table,
        Some(other) => {
            return Err(invalid(format!(
                "name spec must be a table or nil, got {}",
                super::error::lua_type_name(other)
            )));
        }
    };
    let mut spec = NameSpec::default();
    for pair in table.clone().pairs::<Value, Value>() {
        let (key, value) = pair?;
        let Value::String(key) = key else {
            return Err(invalid("name spec keys must be strings".to_string()));
        };
        match &*key.to_str()? {
            "seed" => spec.seed = Some(seed_field(lua, value).map_err(invalid)?),
            "culture" => {
                spec.culture = Some(
                    constant_field::<CultureValue>(&value, "culture", "mux.names.cultures")
                        .map_err(invalid)?
                        .0,
                );
            }
            "flavor" => {
                spec.flavor = Some(
                    constant_field::<FlavorValue>(&value, "flavor", "mux.names.flavors")
                        .map_err(invalid)?
                        .0,
                );
            }
            other => return Err(invalid(format!("unknown name spec field '{other}'"))),
        }
    }
    Ok(spec)
}

/// A seed is a non-negative integer that a Lua number can hold exactly.
fn seed_field(lua: &Lua, value: Value) -> Result<u64, String> {
    let number = match value {
        Value::Integer(seed) => seed as f64,
        Value::Number(seed) => seed,
        Value::String(_) => lua
            .coerce_number(value)
            .ok()
            .flatten()
            .ok_or_else(|| "seed must be a number".to_string())?,
        _ => return Err("seed must be a number".to_string()),
    };
    if !number.is_finite()
        || number.fract() != 0.0
        || number < 0.0
        || number >= (1u64 << SEED_BITS) as f64
    {
        return Err(format!(
            "seed must be an integer from 0 to 2^{SEED_BITS} - 1"
        ));
    }
    Ok(number as u64)
}

/// A culture or flavor field must be one of the package's own constants.
fn constant_field<T: UserData + Copy + 'static>(
    value: &Value,
    what: &str,
    namespace: &str,
) -> Result<T, String> {
    let Value::UserData(data) = value else {
        return Err(format!("{what} must be a constant from {namespace}"));
    };
    data.borrow::<T>()
        .map(|constant| *constant)
        .map_err(|_| format!("{what} must be a constant from {namespace}"))
}

/// Read the count argument (argument 2) for batch calls.
fn count_arg(lua: &Lua, values: &mlua::MultiValue) -> mlua::Result<usize> {
    let count = super::error::check_integer_arg(lua, values, 2)?;
    if !(1..=MAX_COUNT).contains(&count) {
        return Err(super::error::argument_failure(
            lua,
            2,
            "mux.arg.invalid",
            format!("count must be from 1 to {MAX_COUNT}"),
        ));
    }
    Ok(count as usize)
}

/// Seeds left out of a spec come from the clock, trimmed so Lua numbers hold them exactly.
fn with_seed(spec: NameSpec) -> NameSpec {
    NameSpec {
        seed: Some(
            spec.seed
                .unwrap_or_else(|| stompymux_seeded_rng::random_seed() >> (64 - SEED_BITS)),
        ),
        ..spec
    }
}

fn outfit_record(lua: &Lua, outfit: &OutfitName) -> mlua::Result<Table> {
    let record = lua.create_table()?;
    record.set("text", outfit.text.as_str())?;
    record.set("pattern", outfit.pattern.as_str())?;
    record.set("seed", outfit.spec.seed as f64)?;
    record.set(
        "culture",
        lua.create_userdata(CultureValue(outfit.spec.culture))?,
    )?;
    record.set(
        "flavor",
        lua.create_userdata(FlavorValue(outfit.spec.flavor))?,
    )?;
    Ok(record)
}

fn person_record(lua: &Lua, person: &PersonName) -> mlua::Result<Table> {
    let record = lua.create_table()?;
    record.set("given", person.given.as_str())?;
    record.set("family", person.family.as_str())?;
    record.set("full", person.full())?;
    record.set(
        "culture",
        lua.create_userdata(CultureValue(person.culture))?,
    )?;
    Ok(record)
}

/// Load the word lists, preferring `data/names` under the game directory when present.
fn load_lists(config: &Config) -> Result<Lists> {
    let overrides = config.path("data/names");
    if overrides.is_dir() {
        return Lists::from_dir(&overrides)
            .with_context(|| format!("Loading name lists from {}", overrides.display()));
    }
    Lists::embedded().context("Loading embedded name lists")
}

/// Register the `names` table on the private native API.
pub(super) fn register(lua: &Lua, api: &Table, config: &Config) -> Result<()> {
    let lists = Rc::new(load_lists(config)?);
    let install = || -> mlua::Result<()> {
        let table = lua.create_table()?;
        table.set("cultures", lua.create_userdata(CultureNamespace)?)?;
        table.set("flavors", lua.create_userdata(FlavorNamespace)?)?;

        let l = lists.clone();
        table.set(
            "outfit",
            lua.create_function(move |lua, values: mlua::MultiValue| {
                let spec = with_seed(spec_arg(lua, &values)?);
                outfit_record(lua, &l.outfit_name(&spec))
            })?,
        )?;
        let l = lists.clone();
        table.set(
            "person",
            lua.create_function(move |lua, values: mlua::MultiValue| {
                let spec = with_seed(spec_arg(lua, &values)?);
                person_record(lua, &l.person_name(&spec))
            })?,
        )?;
        let l = lists.clone();
        table.set(
            "roster",
            lua.create_function(move |lua, values: mlua::MultiValue| {
                let spec = with_seed(spec_arg(lua, &values)?);
                let count = count_arg(lua, &values)?;
                let roster = lua.create_table()?;
                for person in l.roster(&spec, count) {
                    roster.push(person_record(lua, &person)?)?;
                }
                Ok(roster)
            })?,
        )?;
        let l = lists.clone();
        table.set(
            "callsign",
            lua.create_function(move |lua, values: mlua::MultiValue| {
                let spec = with_seed(spec_arg(lua, &values)?);
                lua.create_string(l.callsign(&spec))
            })?,
        )?;
        let l = lists.clone();
        table.set(
            "callsigns",
            lua.create_function(move |lua, values: mlua::MultiValue| {
                let spec = with_seed(spec_arg(lua, &values)?);
                let count = count_arg(lua, &values)?;
                lua.create_sequence_from(l.callsigns(&spec, count))
            })?,
        )?;
        api.set("names", table)
    };
    install().map_err(|e| anyhow::anyhow!(e.to_string()))
}

/// Install `mux.names` from the native table.
pub(super) fn install(lua: &Lua, api: &Table, mux: &Table) -> mlua::Result<()> {
    lua.load(include_str!("api.lua"))
        .set_name("@builtin/names")
        .call((api, mux))
}
