//! Trusted macro-set handles, permission constants, and player attachment bindings.
use super::error::failure;
use crate::{
    lua::transactions,
    macros::{MacroSetId, service},
    runtime::SharedWorld,
    world::ObjectId,
};
use mlua::{Lua, MetaMethod, Table, UserData, UserDataMethods, Value};

/// Translate a domain error without losing its structured code.
fn domain(e: service::Error) -> mlua::Error {
    failure(e.0, e.1)
}

/// Runtime identity and the transaction-owned world it belongs to.
#[derive(Clone)]
struct Set {
    id: MacroSetId,
    world: SharedWorld,
}

impl Set {
    /// Check callback availability and resolve the current catalog position.
    fn index(&self, lua: &Lua) -> mlua::Result<usize> {
        transactions::require(lua)?;
        service::index(&self.world.borrow().macros, self.id).map_err(domain)
    }
}

/// Reuse object conversion and additionally validate the required object kind.
fn object(
    lua: &Lua,
    world: &SharedWorld,
    value: Value,
    argument: usize,
    player: bool,
) -> mlua::Result<ObjectId> {
    let w = world.borrow();
    let id = super::world::handles::identity_at(lua, value, &w, argument, "object")?;
    service::object(&w, id, player).map_err(domain)?;
    Ok(id)
}

/// Require UTF-8 Lua strings without coercion or trimming.
fn string(value: Value) -> mlua::Result<String> {
    match value {
        Value::String(s) => s
            .to_str()
            .map(|s| s.to_owned())
            .map_err(|_| failure("mux.arg.invalid", "expected UTF-8 text")),
        _ => Err(failure("mux.arg.invalid", "expected a string")),
    }
}

/// Accept only integral, nonnegative catalog and slot numbers.
fn number(value: Value) -> mlua::Result<usize> {
    match value {
        Value::Integer(n) if n >= 0 => {
            usize::try_from(n).map_err(|_| failure("mux.arg.invalid", "number is out of range"))
        }
        Value::Number(n)
            if n.is_finite() && n >= 0.0 && n.fract() == 0.0 && n < usize::MAX as f64 =>
        {
            Ok(n as usize)
        }
        _ => Err(failure("mux.arg.invalid", "expected a nonnegative integer")),
    }
}

/// Accept only genuine set handles from this world.
fn handle(value: Value, world: &SharedWorld) -> mlua::Result<Set> {
    if let Value::UserData(u) = value
        && let Ok(s) = u.borrow::<Set>()
        && std::rc::Rc::ptr_eq(&s.world, world)
    {
        return Ok(s.clone());
    }
    Err(failure("mux.arg.invalid", "expected a macro set handle"))
}

/// Reject mutation of userdata fields and constant namespaces.
fn immutable() -> mlua::Error {
    failure(
        "mux.arg.invalid",
        "macro handles and constants are immutable",
    )
}

impl UserData for Set {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_function(MetaMethod::Eq, |_, (a, b): (Value, Value)| {
            super::error::typed_eq::<Self, _>("mux.macro.Set", a, b, |a, b| {
                a.id == b.id && std::rc::Rc::ptr_eq(&a.world, &b.world)
            })
        });
        m.add_meta_method(MetaMethod::NewIndex, |_, _, _: (Value, Value)| {
            Err::<(), _>(immutable())
        });
        m.add_method("number", |lua, s, ()| s.index(lua));
        m.add_method("description", |lua, s, ()| {
            let i = s.index(lua)?;
            Ok(s.world.borrow().macros.sets[i].description.clone())
        });
        m.add_method("owner", |lua, s, ()| {
            let i = s.index(lua)?;
            let owner = s.world.borrow().macros.sets[i].owner;
            let mux: Table = lua.globals().get("mux")?;
            mux.get::<Table>("world")?
                .get::<mlua::Function>("object")?
                .call::<Value>(owner.0)
        });
        m.add_method("set_owner", |lua, s, value: Value| {
            s.index(lua)?;
            let owner = object(lua, &s.world, value, 2, false)?;
            service::set_owner(&mut s.world.borrow_mut(), s.id, owner).map_err(domain)
        });
        m.add_method("list_macros", |lua, s, ()| {
            let i = s.index(lua)?;
            let w = s.world.borrow();
            let mut entries: Vec<_> = w.macros.sets[i].entries.iter().collect();
            entries.sort_by_key(|e| e.alias.to_ascii_lowercase());
            let out = lua.create_table()?;
            for e in entries {
                let row = lua.create_table()?;
                row.set("alias", e.alias.clone())?;
                row.set("expansion", e.expansion.clone())?;
                out.push(row)?;
            }
            Ok(out)
        });
        for (method, update) in [("add_macro", false), ("update_macro", true)] {
            m.add_method(method, move |lua, s, (alias, expansion): (Value, Value)| {
                s.index(lua)?;
                service::define(
                    &mut s.world.borrow_mut().macros,
                    s.id,
                    string(alias)?,
                    string(expansion)?,
                    update,
                )
                .map_err(domain)
            });
        }
        m.add_method("delete_macro", |lua, s, alias: Value| {
            s.index(lua)?;
            service::delete(&mut s.world.borrow_mut().macros, s.id, &string(alias)?).map_err(domain)
        });
        m.add_method("flags", |lua, s, ()| {
            s.index(lua)?;
            Ok(Flags(s.clone()))
        });
    }
}

/// Typed immutable permission bit; unrelated flag families are rejected.
#[derive(Clone, Copy, PartialEq)]
struct Flag(i64);
impl UserData for Flag {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_function(MetaMethod::Eq, |_, (a, b): (Value, Value)| {
            super::error::typed_eq::<Self, _>("mux.macro.Flag", a, b, |a, b| a == b)
        });
        m.add_meta_method(MetaMethod::NewIndex, |_, _, _: (Value, Value)| {
            Err::<(), _>(immutable())
        });
        m.add_meta_method(MetaMethod::ToString, |_, f, ()| {
            Ok(match f.0 {
                1 => "LOCKED",
                2 => "READ",
                _ => "WRITE",
            })
        });
    }
}

/// Require a constant from the macro permission family.
fn flag(value: Value) -> mlua::Result<Flag> {
    if let Value::UserData(u) = value
        && let Ok(f) = u.borrow::<Flag>()
    {
        return Ok(*f);
    }
    Err(failure(
        "mux.arg.invalid",
        "expected a mux.macro.flags constant",
    ))
}

/// Namespace whose values cannot be overwritten or forged from integers.
struct Constants;
impl UserData for Constants {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Index, |_, _, value: Value| {
            Ok(Flag(match string(value)?.as_str() {
                "LOCKED" => 1,
                "READ" => 2,
                "WRITE" => 4,
                _ => return Err(failure("mux.arg.invalid", "unknown macro flag")),
            }))
        });
        m.add_meta_method(MetaMethod::NewIndex, |_, _, _: (Value, Value)| {
            Err::<(), _>(immutable())
        });
    }
}

/// Live flags share the owning set's identity and transaction guards.
struct Flags(Set);
impl UserData for Flags {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_method("list", |lua, f, ()| {
            let i = f.0.index(lua)?;
            let bits = f.0.world.borrow().macros.sets[i].modes;
            lua.create_sequence_from([1, 2, 4].into_iter().filter(|b| bits.has(*b)).map(Flag))
        });
        m.add_method("has", |lua, f, value: Value| {
            let i = f.0.index(lua)?;
            Ok(f.0.world.borrow().macros.sets[i].modes.has(flag(value)?.0))
        });
        for (method, add) in [("add", true), ("remove", false)] {
            m.add_method(method, move |lua, f, value: Value| {
                let i = f.0.index(lua)?;
                let bit = flag(value)?.0;
                let mut w = f.0.world.borrow_mut();
                let modes = &mut w.macros.sets[i].modes;
                if add {
                    modes.0 |= bit;
                } else {
                    modes.0 &= !bit;
                }
                Ok(())
            });
        }
        m.add_meta_method(MetaMethod::NewIndex, |_, _, _: (Value, Value)| {
            Err::<(), _>(immutable())
        });
    }
}

/// Register against the private native table before installing the public namespace.
pub(super) fn register(lua: &Lua, api: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let table = lua.create_table()?;
    table.set("flags", Constants)?;
    let w = world.clone();
    table.set(
        "list_sets",
        lua.create_function(move |lua, ()| {
            transactions::require(lua)?;
            lua.create_sequence_from(w.borrow().macros.sets.iter().map(|s| Set {
                id: s.id,
                world: w.clone(),
            }))
        })?,
    )?;
    let w = world.clone();
    table.set(
        "set",
        lua.create_function(move |lua, value: Value| {
            transactions::require(lua)?;
            Ok(w.borrow().macros.sets.get(number(value)?).map(|s| Set {
                id: s.id,
                world: w.clone(),
            }))
        })?,
    )?;
    let w = world.clone();
    table.set(
        "create_set",
        lua.create_function(move |lua, (owner, description): (Value, Value)| {
            transactions::require(lua)?;
            let owner = object(lua, &w, owner, 1, false)?;
            let id = service::create(&mut w.borrow_mut(), owner, string(description)?)
                .map_err(domain)?;
            Ok(Set {
                id,
                world: w.clone(),
            })
        })?,
    )?;
    let w = world.clone();
    table.set(
        "destroy_set",
        lua.create_function(move |lua, value: Value| {
            transactions::require(lua)?;
            let s = handle(value, &w)?;
            let i = s.index(lua)?;
            w.borrow_mut().macros.remove(i);
            Ok(())
        })?,
    )?;
    let w = world.clone();
    table.set(
        "attach",
        lua.create_function(move |lua, (player, set): (Value, Value)| {
            transactions::require(lua)?;
            let player = object(lua, &w, player, 1, true)?;
            let s = handle(set, &w)?;
            service::attach(&mut w.borrow_mut(), player, s.id).map_err(domain)
        })?,
    )?;
    let w = world.clone();
    table.set(
        "detach",
        lua.create_function(move |lua, (player, slot): (Value, Value)| {
            transactions::require(lua)?;
            let player = object(lua, &w, player, 1, true)?;
            service::detach(&mut w.borrow_mut(), player, number(slot)?).map_err(domain)
        })?,
    )?;
    let w = world.clone();
    table.set(
        "list_player_sets",
        lua.create_function(move |lua, player: Value| {
            transactions::require(lua)?;
            let player = object(lua, &w, player, 1, true)?;
            let world = w.borrow();
            let out = lua.create_table()?;
            if let Some(slots) = world.macros.players.get(&player) {
                for (slot, index) in slots.slots.iter().enumerate() {
                    if let Some(i) = index {
                        let row = lua.create_table()?;
                        row.set("slot", slot)?;
                        row.set("selected", slots.current == Some(slot))?;
                        row.set(
                            "set",
                            Set {
                                id: world.macros.sets[*i].id,
                                world: w.clone(),
                            },
                        )?;
                        out.push(row)?;
                    }
                }
            }
            Ok(out)
        })?,
    )?;
    api.set("macro", table)
}
