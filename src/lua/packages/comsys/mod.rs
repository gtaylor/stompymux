//! Generation-sensitive channel handles and immutable comsys flag bindings.
use crate::{
    communication::{ChannelFlag, ChannelId, Service},
    config::Config,
    flags::Flag,
    lua::{Outbox, SharedWorld},
    world::{Kind, ObjectId},
};
use anyhow::Result;
use mlua::{AnyUserData, Lua, MetaMethod, Table, UserData, UserDataMethods, Value};
use std::rc::Rc;

/// Shared world-thread dependencies retained by Lua handles.
struct Bindings {
    world: SharedWorld,
    outbox: Outbox,
}

impl Bindings {
    fn service<'a>(&'a self, lua: &'a Lua) -> Service<'a> {
        Service {
            world: &self.world,
            outbox: &self.outbox,
            config: crate::lua::configuration(lua),
            lua,
        }
    }
}

/// Errors retain contextual domain messages without cross-thread Lua requirements.
fn error(e: impl std::fmt::Display) -> mlua::Error {
    super::error::failure("mux.channel.invalid", e)
}

/// Identity is independent of channel name, and survives no provisional recreation.
#[derive(Clone)]
struct Handle {
    id: ChannelId,
    bindings: Rc<Bindings>,
}

impl Handle {
    fn name(&self, lua: &Lua) -> mlua::Result<String> {
        self.bindings.service(lua).by_id(self.id).map_err(error)
    }
}

/// Resolve the existing object facade and reject dead or foreign values.
fn object(value: Value, b: &Bindings) -> mlua::Result<ObjectId> {
    let id = super::world::handles::identity(value)?;
    if !b
        .world
        .borrow()
        .objects
        .get(&id)
        .is_some_and(|o| o.kind != Kind::Garbage)
    {
        return Err(super::error::failure(
            "mux.object.invalid",
            "invalid object",
        ));
    }
    Ok(id)
}

/// Construct the established Lua object wrapper from a validated identity.
fn wrapper(lua: &Lua, id: ObjectId) -> mlua::Result<Value> {
    let mux: Table = lua.globals().get("mux")?;
    let world: Table = mux.get("world")?;
    world.get::<mlua::Function>("object")?.call(id.0)
}

impl UserData for Handle {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_method("name", |lua, h, ()| h.name(lua));
        m.add_method("object", |lua, h, ()| {
            let name = h.name(lua)?;
            let id = h.bindings.world.borrow().channels[&name].object;
            id.map_or(Ok(Value::Nil), |id| wrapper(lua, id))
        });
        m.add_method("set_object", |lua, h, values: mlua::MultiValue| {
            let name = h.name(lua)?;
            if values.len() != 1 {
                return Err(error("set_object requires an object or nil"));
            }
            let value = values.into_iter().next().unwrap();
            let id = if matches!(value, Value::Nil) {
                None
            } else {
                Some(object(value, &h.bindings)?)
            };
            if id.is_some_and(|id| {
                h.bindings.world.borrow().objects[&id]
                    .flags
                    .contains(Flag::Going)
            }) {
                return Err(super::error::failure(
                    "mux.object.unavailable",
                    "object is being destroyed",
                ));
            }
            h.bindings
                .world
                .borrow_mut()
                .channels
                .get_mut(&name)
                .unwrap()
                .object = id;
            Ok(())
        });
        m.add_method("flags", |lua, h, ()| {
            h.name(lua)?;
            lua.create_userdata(Flags(h.clone()))
        });
        m.add_method("user_count", |lua, h, ()| {
            let name = h.name(lua)?;
            Ok(h.bindings.world.borrow().channels[&name].users.len())
        });
        m.add_method("max_user_count", |lua, h, ()| {
            let name = h.name(lua)?;
            Ok(h.bindings.world.borrow().channels[&name].max_users)
        });
        m.add_method("message_count", |lua, h, ()| {
            let name = h.name(lua)?;
            Ok(h.bindings.world.borrow().channels[&name].messages)
        });
        m.add_method(
            "emit",
            |lua, h, (message, options): (String, Option<Table>)| {
                let name = h.name(lua)?;
                let no_header = option(options, "no_header")?;
                h.bindings
                    .service(lua)
                    .emit(&name, &message, no_header)
                    .map_err(error)
            },
        );
        m.add_method("who", |lua, h, options: Option<Table>| {
            let name = h.name(lua)?;
            let all = option(options, "all")?;
            let users = h.bindings.service(lua).members(&name, all).map_err(error)?;
            let result = lua.create_table()?;
            for (i, u) in users.iter().enumerate() {
                let entry = lua.create_table()?;
                entry.set("object", wrapper(lua, u.who)?)?;
                entry.set("listening", u.listening)?;
                result.set(i + 1, entry)?;
            }
            Ok(result)
        });
        m.add_method(
            "add_player",
            |lua, h, (value, alias, quiet): (Value, Value, Value)| {
                let name = h.name(lua)?;
                let who = object(value, &h.bindings)?;
                let Value::String(alias) = alias else {
                    return Err(error("alias must be a string"));
                };
                let alias = alias.to_str()?.to_string();
                let Value::Boolean(quiet) = quiet else {
                    return Err(error("quiet must be a boolean"));
                };
                if h.bindings.world.borrow().objects[&who]
                    .flags
                    .contains(Flag::Going)
                {
                    return Err(error("player is being destroyed"));
                }
                if h.bindings.world.borrow().objects[&who].kind != Kind::Player {
                    return Err(error("expected a player"));
                }
                h.bindings
                    .service(lua)
                    .add(who, &name, &alias, quiet, true)
                    .map_err(error)
            },
        );
        m.add_method("boot_player", |lua, h, value: Value| {
            let name = h.name(lua)?;
            let who = object(value, &h.bindings)?;
            h.bindings
                .service(lua)
                .boot(ObjectId(1), who, &name)
                .map_err(error)
        });
        m.add_meta_method(MetaMethod::Eq, |_, h, other: AnyUserData| {
            Ok(other
                .borrow::<Handle>()
                .is_ok_and(|o| o.id == h.id && Rc::ptr_eq(&o.bindings, &h.bindings)))
        });
    }
}

/// Immutable namespace rejects spelling mistakes and noncanonical constants.
struct Constants;
impl UserData for Constants {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Index, |lua, _, name: String| {
            let flag = ChannelFlag::parse(&name)
                .map_err(|e| super::error::failure("mux.channel_flag.invalid", e))?;
            if name != flag.name() {
                return Err(super::error::failure(
                    "mux.channel_flag.invalid",
                    "channel constants require uppercase names",
                ));
            }
            lua.create_userdata(flag)
        });
    }
}

impl UserData for ChannelFlag {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::ToString, |_, f, ()| Ok(f.name()));
        m.add_meta_method(MetaMethod::Eq, |_, f, other: AnyUserData| {
            Ok(other.borrow::<ChannelFlag>().is_ok_and(|o| *o == *f))
        });
    }
}

/// Flag access shares the channel generation and cannot accept world flag userdata.
struct Flags(Handle);
impl UserData for Flags {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_method("list", |lua, f, ()| {
            let name = f.0.name(lua)?;
            let flags = f.0.bindings.world.borrow().channels[&name].flags;
            let result = lua.create_table()?;
            for flag in [
                ChannelFlag::Public,
                ChannelFlag::Loud,
                ChannelFlag::Transparent,
            ] {
                if flags.has(flag) {
                    result.push(lua.create_userdata(flag)?)?;
                }
            }
            Ok(result)
        });
        m.add_method("has", |lua, f, value: AnyUserData| {
            let flag = *value.borrow::<ChannelFlag>().map_err(|_| {
                super::error::failure("mux.channel_flag.invalid", "expected a typed ChannelFlag")
            })?;
            let name = f.0.name(lua)?;
            Ok(f.0.bindings.world.borrow().channels[&name].flags.has(flag))
        });
        for (name, enabled) in [("add", true), ("remove", false)] {
            m.add_method(name, move |lua, f, value: AnyUserData| {
                let flag = *value.borrow::<ChannelFlag>().map_err(|_| {
                    super::error::failure(
                        "mux.channel_flag.invalid",
                        "expected a typed ChannelFlag",
                    )
                })?;
                let name = f.0.name(lua)?;
                Ok(f.0
                    .bindings
                    .world
                    .borrow_mut()
                    .channels
                    .get_mut(&name)
                    .unwrap()
                    .flags
                    .set(flag, enabled))
            });
        }
    }
}

/// Register functions once; user code never supplies handle identities as numbers.
pub(super) fn register(
    lua: &Lua,
    api: &Table,
    _config: &Config,
    world: &SharedWorld,
    outbox: &Outbox,
) -> Result<()> {
    let b = Rc::new(Bindings {
        world: world.clone(),
        outbox: outbox.clone(),
    });
    let install = || -> mlua::Result<()> {
        let table = lua.create_table()?;
        table.set("flags", lua.create_userdata(Constants)?)?;
        let bindings = b.clone();
        table.set(
            "create_channel",
            lua.create_function(move |lua, name: String| {
                let id = bindings.service(lua).create(&name).map_err(error)?;
                lua.create_userdata(Handle {
                    id,
                    bindings: bindings.clone(),
                })
            })?,
        )?;
        let bindings = b.clone();
        table.set(
            "channel",
            lua.create_function(move |lua, name: String| {
                let service = bindings.service(lua);
                if name.contains('\0') {
                    return Err(error("channel name contains an embedded NUL byte"));
                }
                let name = service.name(&name).map_err(error)?;
                let id = bindings.world.borrow().channels[&name].id;
                Ok(Value::UserData(lua.create_userdata(Handle {
                    id,
                    bindings: bindings.clone(),
                })?))
            })?,
        )?;
        let bindings = b.clone();
        table.set(
            "destroy_channel",
            lua.create_function(move |lua, value: AnyUserData| {
                let h = value.borrow::<Handle>()?;
                if !Rc::ptr_eq(&h.bindings, &bindings) {
                    return Err(error("foreign channel handle"));
                }
                let name = h.name(lua)?;
                bindings.service(lua).destroy(&name).map_err(error)
            })?,
        )?;
        let bindings = b.clone();
        table.set(
            "list_channels",
            lua.create_function(move |lua, ()| {
                let mut channels = bindings
                    .world
                    .borrow()
                    .channels
                    .values()
                    .map(|c| (c.name.to_ascii_lowercase(), c.id))
                    .collect::<Vec<_>>();
                channels.sort_by(|a, b| a.0.cmp(&b.0));
                let result = lua.create_table()?;
                for (i, (_, id)) in channels.into_iter().enumerate() {
                    result.set(
                        i + 1,
                        lua.create_userdata(Handle {
                            id,
                            bindings: bindings.clone(),
                        })?,
                    )?;
                }
                Ok(result)
            })?,
        )?;
        api.set("comsys", table)
    };
    install().map_err(|e| anyhow::anyhow!(e.to_string()))
}

/// Keep server-owned namespace installation embedded in the executable.
pub(super) fn install(
    lua: &Lua,
    api: &Table,
    mux: &Table,
    id: &mlua::Function,
) -> mlua::Result<()> {
    lua.load(include_str!("api.lua"))
        .set_name("@builtin/comsys")
        .call((api, mux, id))
}

/// Match the C API's closed option tables instead of silently accepting typos.
fn option(options: Option<Table>, key: &str) -> mlua::Result<bool> {
    let Some(options) = options else {
        return Ok(false);
    };
    for entry in options.clone().pairs::<Value, Value>() {
        let (name, value) = entry?;
        if !matches!(name, Value::String(ref name) if name.to_str()?.as_ref() == key) {
            return Err(error("unknown channel option"));
        }
        if !matches!(value, Value::Boolean(_)) {
            return Err(error("channel option must be boolean"));
        }
    }
    Ok(options.get::<Option<bool>>(key)?.unwrap_or(false))
}
