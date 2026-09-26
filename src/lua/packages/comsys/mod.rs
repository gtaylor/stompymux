//! Generation-sensitive channel handles and immutable comsys flag bindings.
use crate::{
    communication::{ChannelFlag, ChannelId, Service},
    config::Config,
    flags::Flag,
    runtime::{Effects, Outbox, SharedWorld},
    world::{Kind, ObjectId},
};
use anyhow::Result;
use mlua::{AnyUserData, Lua, MetaMethod, Table, UserData, UserDataMethods, Value};
use std::rc::Rc;

/// Shared world-thread dependencies retained by Lua handles.
struct Bindings {
    world: SharedWorld,
    outbox: Outbox,
    effects: Effects,
}

impl Bindings {
    fn service<'a>(&'a self, lua: &'a Lua) -> Service<'a> {
        Service {
            world: &self.world,
            outbox: &self.outbox,
            effects: &self.effects,
            config: crate::communication::ServiceConfig::Shared(crate::lua::configuration(lua)),
            host: lua,
        }
    }
}

/// Errors retain contextual domain messages without cross-thread Lua requirements.
fn error(e: impl std::fmt::Display) -> mlua::Error {
    super::error::failure("mux.channel.invalid", e)
}

/// Structured mux.arg.invalid rejection carrying the public argument number.
fn arg_failure(lua: &Lua, argument: usize, message: impl ToString) -> mlua::Error {
    super::error::argument_failure(lua, argument, "mux.arg.invalid", message)
}

fn require_runtime(lua: &Lua, function: &str) -> mlua::Result<()> {
    if lua
        .app_data_ref::<crate::lua::RuntimeMode>()
        .is_some_and(|mode| *mode == crate::lua::RuntimeMode::Checking)
    {
        return Err(super::error::failure(
            "mux.unavailable.checking",
            format!("mux.comsys.{function} is unavailable during @lua/check"),
        ));
    }
    Ok(())
}

/// Identity is independent of channel name, and survives no provisional recreation.
#[derive(Clone)]
struct Handle {
    id: ChannelId,
    name: String,
    bindings: Rc<Bindings>,
}

impl Handle {
    /// Re-resolve by native identity; stale handles raise the C stale-channel error.
    fn name(&self, lua: &Lua) -> mlua::Result<String> {
        require_runtime(lua, "Channel")?;
        self.bindings.service(lua).by_id(self.id).map_err(|_| {
            super::error::argument_failure(
                lua,
                1,
                "mux.channel.invalid",
                "channel no longer exists",
            )
        })
    }
}

/// Validate an object argument with the C lua_mux_require_object messages.
fn require_object(
    lua: &Lua,
    value: Value,
    b: &Bindings,
    argument: usize,
) -> mlua::Result<ObjectId> {
    let world = b.world.borrow();
    // Re-raise the shared converter's C message with live caller context so the
    // message keeps C's "bad argument #N to 'name'" framing inside methods.
    super::world::handles::identity_at(lua, value, &world, argument, "object").map_err(|e| {
        let message = e
            .downcast_ref::<super::error::Failure>()
            .map(|f| String::from_utf8_lossy(&f.message).into_owned())
            .unwrap_or_else(|| e.to_string());
        super::error::argument_failure(lua, argument, "mux.object.invalid", message)
    })
}

/// Construct the established Lua object wrapper from a validated identity.
fn wrapper(lua: &Lua, id: ObjectId) -> mlua::Result<Value> {
    let mux: Table = lua.globals().get("mux")?;
    let world: Table = mux.get("world")?;
    world.get::<mlua::Function>("object")?.call(id.0)
}

/// Match the C closed option tables: reject unknown fields before wrong types.
fn check_options(
    lua: &Lua,
    value: Value,
    argument: usize,
    allowed: &[&str],
) -> mlua::Result<Option<Table>> {
    if value.is_nil() {
        return Ok(None);
    }
    let Value::Table(table) = value else {
        return Err(arg_failure(lua, argument, "options must be a table"));
    };
    for entry in table.clone().pairs::<Value, Value>() {
        let (key, _) = entry?;
        let key = match &key {
            Value::String(key) => key.to_str()?.to_owned(),
            _ => "<non-string>".to_owned(),
        };
        if !allowed.contains(&key.as_str()) {
            return Err(arg_failure(
                lua,
                argument,
                format!("unknown options field '{key}'"),
            ));
        }
    }
    Ok(Some(table))
}

/// Read one boolean option field after the field set has been validated.
fn option_boolean(lua: &Lua, table: &Table, argument: usize, field: &str) -> mlua::Result<bool> {
    match table.get::<Value>(field)? {
        Value::Nil => Ok(false),
        Value::Boolean(value) => Ok(value),
        _ => Err(arg_failure(
            lua,
            argument,
            format!("options.{field} must be a boolean"),
        )),
    }
}

impl UserData for Handle {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::ToString, |_, h, ()| {
            Ok(format!("channel({})", h.name))
        });
        m.add_method("name", |lua, h, ()| h.name(lua));
        m.add_method("object", |lua, h, ()| {
            let name = h.name(lua)?;
            let id = h.bindings.world.borrow().channels[&name].object;
            match id {
                None => Ok(Value::Nil),
                Some(id) => {
                    let live = h
                        .bindings
                        .world
                        .borrow()
                        .objects
                        .get(&id)
                        .map_or(false, |o| o.kind != Kind::Garbage);
                    if !live {
                        return Err(super::error::failure(
                            "mux.object.invalid",
                            "attached channel object no longer exists",
                        ));
                    }
                    wrapper(lua, id)
                }
            }
        });
        m.add_method("set_object", |lua, h, values: mlua::MultiValue| {
            let name = h.name(lua)?;
            let Some(value) = values.front().cloned() else {
                return Err(arg_failure(lua, 2, "expected an object or nil"));
            };
            let id = if value.is_nil() {
                None
            } else {
                let id = require_object(lua, value, &h.bindings, 2)?;
                if h.bindings.world.borrow().objects[&id]
                    .flags
                    .contains(Flag::Going)
                {
                    return Err(super::error::argument_failure(
                        lua,
                        2,
                        "mux.object.unavailable",
                        "object is being destroyed",
                    ));
                }
                Some(id)
            };
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
        m.add_method("emit", |lua, h, values: mlua::MultiValue| {
            let name = h.name(lua)?;
            // C's luaL_checklstring sees the message at stack index 2 (self at 1).
            let message = super::error::check_string_arg_at(lua, &values, 1, 2)?;
            if message.contains(&0) {
                return Err(arg_failure(lua, 2, "message contains an embedded NUL byte"));
            }
            let message = String::from_utf8(message)
                .map_err(|_| arg_failure(lua, 2, "message is not valid UTF-8"))?;
            let options = values.get(1).cloned().unwrap_or(Value::Nil);
            let no_header = match check_options(lua, options, 3, &["no_header"])? {
                Some(table) => option_boolean(lua, &table, 3, "no_header")?,
                None => false,
            };
            crate::lua::transactions::run(lua, &h.bindings.world, || {
                h.bindings
                    .service(lua)
                    .emit(&name, &message, no_header)
                    .map_err(error)
            })
        });
        m.add_method("who", |lua, h, options_value: Value| {
            let name = h.name(lua)?;
            let all = match check_options(lua, options_value, 2, &["all"])? {
                Some(table) => option_boolean(lua, &table, 2, "all")?,
                None => false,
            };
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
                let who = require_object(lua, value, &h.bindings, 2)?;
                if h.bindings.world.borrow().objects[&who].kind != Kind::Player {
                    return Err(super::error::argument_failure(
                        lua,
                        2,
                        "mux.object.invalid",
                        "object must be a player",
                    ));
                }
                if h.bindings.world.borrow().objects[&who]
                    .flags
                    .contains(Flag::Going)
                {
                    return Err(super::error::argument_failure(
                        lua,
                        2,
                        "mux.object.unavailable",
                        "player is being destroyed",
                    ));
                }
                let Value::String(alias) = alias else {
                    return Err(arg_failure(lua, 3, "alias must be a string"));
                };
                if alias.as_bytes().contains(&0) {
                    return Err(arg_failure(lua, 3, "alias contains an embedded NUL byte"));
                }
                let Value::Boolean(quiet) = quiet else {
                    return Err(arg_failure(lua, 4, "quiet must be a boolean"));
                };
                let alias = alias.to_str()?.to_owned();
                if alias.is_empty() {
                    return Err(arg_failure(lua, 3, "alias is required"));
                }
                if alias.len() > 5 || !alias.bytes().all(|b| (33..=126).contains(&b)) {
                    return Err(arg_failure(
                        lua,
                        3,
                        "alias must be 1-5 printable ASCII characters without spaces",
                    ));
                }
                let in_use = h
                    .bindings
                    .world
                    .borrow()
                    .channel_aliases
                    .get(&who)
                    .is_some_and(|aliases| {
                        aliases.iter().any(|a| a.alias.eq_ignore_ascii_case(&alias))
                    });
                if in_use {
                    return Err(arg_failure(lua, 3, "alias is already in use"));
                }
                crate::lua::transactions::run(lua, &h.bindings.world, || {
                    h.bindings
                        .service(lua)
                        .add(who, &name, &alias, quiet, true)
                        .map_err(|e| super::error::failure("mux.internal", format!("{e:#}")))
                })
            },
        );
        m.add_method("boot_player", |lua, h, value: Value| {
            let name = h.name(lua)?;
            let who = require_object(lua, value, &h.bindings, 2)?;
            let member = h.bindings.world.borrow().channels[&name]
                .users
                .iter()
                .any(|u| u.who == who);
            if !member {
                return Err(super::error::argument_failure(
                    lua,
                    2,
                    "mux.channel.invalid",
                    "object is not a member of this channel",
                ));
            }
            crate::lua::transactions::run(lua, &h.bindings.world, || {
                h.bindings
                    .service(lua)
                    .boot(ObjectId(1), who, &name)
                    .map_err(error)
            })
        });
        m.add_meta_function(MetaMethod::Eq, |_, (left, right): (Value, Value)| {
            // C's channel __eq luaL_checkudatas both operands
            // (mux_comsys_bindings.c lua_mux_channel_equal).
            super::error::typed_eq::<Handle, _>("btmux.channel", left, right, |a, b| {
                a.id == b.id && Rc::ptr_eq(&a.bindings, &b.bindings)
            })
        });
        m.add_meta_method(MetaMethod::NewIndex, |_, _, _: (Value, Value)| {
            Err::<(), _>(super::error::failure(
                "mux.arg.invalid",
                "channel values are immutable",
            ))
        });
    }
}

/// Immutable namespace rejects spelling mistakes and noncanonical constants.
struct Constants;
impl UserData for Constants {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Index, |lua, _, key: Value| {
            let name = lua.coerce_string(key)?.ok_or_else(|| {
                super::error::argument_failure(
                    lua,
                    2,
                    "mux.channel_flag.invalid",
                    "channel flag name must be a string",
                )
            })?;
            let bytes = name.as_bytes();
            let bytes = bytes.split(|byte| *byte == 0).next().unwrap_or_default();
            let text = String::from_utf8_lossy(bytes);
            let flag = ChannelFlag::parse(&text)
                .ok()
                .filter(|flag| flag.name() == text)
                .ok_or_else(|| {
                    super::error::argument_failure(
                        lua,
                        2,
                        "mux.channel_flag.invalid",
                        format!("unknown channel flag constant '{text}'"),
                    )
                })?;
            flag_value(lua, flag).map(Value::UserData)
        });
        m.add_meta_method(MetaMethod::NewIndex, |_, _, _: (Value, Value)| {
            Err::<(), _>(immutable())
        });
    }
}

/// Channel flag values and namespaces reject assignment with the C error.
fn immutable() -> mlua::Error {
    super::error::failure(
        "mux.channel_flag.invalid",
        "channel flag values are immutable",
    )
}

/// Hide the metatable of a comsys userdata behind the C protection label.
fn protect(lua: &Lua, value: &AnyUserData, label: &'static str) -> mlua::Result<()> {
    unsafe {
        lua.exec_raw::<()>((value.clone(), label), |state| {
            if mlua::ffi::lua_getmetatable(state, 1) != 0 {
                mlua::ffi::lua_pushvalue(state, 2);
                mlua::ffi::lua_setfield(state, -2, c"__metatable".as_ptr());
            }
        })
    }
}

/// Create one protected typed channel-flag constant userdata.
fn flag_value(lua: &Lua, flag: ChannelFlag) -> mlua::Result<AnyUserData> {
    let value = lua.create_userdata(flag)?;
    protect(lua, &value, "protected channel flag constant metatable")?;
    Ok(value)
}

impl UserData for ChannelFlag {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::ToString, |_, f, ()| Ok(f.name()));
        m.add_meta_function(MetaMethod::Eq, |_, (left, right): (Value, Value)| {
            // C's channel flag __eq luaL_checkudatas both operands
            // (mux_comsys_channel_flag_bindings.c lua_mux_channel_flag_equal).
            super::error::typed_eq::<Self, _>("btmux.channel_flag", left, right, |a, b| a == b)
        });
        m.add_meta_method(MetaMethod::NewIndex, |_, _, _: (Value, Value)| {
            Err::<(), _>(immutable())
        });
    }
}

/// Flag methods accept only typed constants from this runtime, as in C.
fn check_flag(lua: &Lua, value: Value) -> mlua::Result<ChannelFlag> {
    let invalid = || {
        super::error::argument_failure(
            lua,
            2,
            "mux.channel_flag.invalid",
            "expected a mux.comsys.flags constant",
        )
    };
    let Value::UserData(userdata) = value else {
        return Err(invalid());
    };
    Ok(*userdata.borrow::<ChannelFlag>().map_err(|_| invalid())?)
}

/// Flag access shares the channel generation and cannot accept world flag userdata.
struct Flags(Handle);
impl UserData for Flags {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::ToString, |lua, f, ()| {
            Ok(format!("channel_flags({})", f.0.name(lua)?))
        });
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
                    result.push(flag_value(lua, flag)?)?;
                }
            }
            Ok(result)
        });
        m.add_method("has", |lua, f, value: Value| {
            // C validates the live channel identity before the flag argument.
            let name = f.0.name(lua)?;
            let flag = check_flag(lua, value)?;
            Ok(f.0.bindings.world.borrow().channels[&name].flags.has(flag))
        });
        for (name, enabled) in [("add", true), ("remove", false)] {
            m.add_method(name, move |lua, f, value: Value| {
                let name = f.0.name(lua)?;
                let flag = check_flag(lua, value)?;
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
    effects: &Effects,
) -> Result<()> {
    let b = Rc::new(Bindings {
        world: world.clone(),
        outbox: outbox.clone(),
        effects: effects.clone(),
    });
    let install = || -> mlua::Result<()> {
        let table = lua.create_table()?;
        let namespace = lua.create_userdata(Constants)?;
        protect(
            lua,
            &namespace,
            "protected channel flag namespace metatable",
        )?;
        table.set("flags", namespace)?;
        let bindings = b.clone();
        table.set(
            "create_channel",
            lua.create_function(move |lua, values: mlua::MultiValue| {
                let name = super::error::check_string_arg(lua, &values, 1)?;
                require_runtime(lua, "create_channel")?;
                if name.contains(&0) {
                    return Err(arg_failure(
                        lua,
                        1,
                        "channel name contains an embedded NUL byte",
                    ));
                }
                let name = String::from_utf8_lossy(&name).into_owned();
                let exists = bindings
                    .world
                    .borrow()
                    .channels
                    .keys()
                    .any(|candidate| candidate.eq_ignore_ascii_case(&name));
                if exists {
                    return Err(super::error::argument_failure(
                        lua,
                        1,
                        "mux.channel.invalid",
                        format!("channel '{name}' already exists"),
                    ));
                }
                if name.is_empty()
                    || name.len() >= 50
                    || !name.bytes().all(|byte| (33..=126).contains(&byte))
                {
                    return Err(arg_failure(
                        lua,
                        1,
                        "channel name must be printable ASCII without spaces and shorter than 50 bytes",
                    ));
                }
                let id = bindings.service(lua).create(&name).map_err(error)?;
                lua.create_userdata(Handle {
                    id,
                    name,
                    bindings: bindings.clone(),
                })
            })?,
        )?;
        let bindings = b.clone();
        table.set(
            "channel",
            lua.create_function(move |lua, values: mlua::MultiValue| {
                let name = super::error::check_string_arg(lua, &values, 1)?;
                require_runtime(lua, "channel")?;
                if name.contains(&0) {
                    return Err(arg_failure(
                        lua,
                        1,
                        "channel name contains an embedded NUL byte",
                    ));
                }
                let name = String::from_utf8_lossy(&name).into_owned();
                let canonical = bindings
                    .world
                    .borrow()
                    .channels
                    .keys()
                    .find_map(|candidate| {
                        candidate
                            .eq_ignore_ascii_case(&name)
                            .then(|| candidate.clone())
                    });
                let Some(name) = canonical else {
                    return Err(super::error::argument_failure(
                        lua,
                        1,
                        "mux.channel.invalid",
                        format!("unknown channel '{name}'"),
                    ));
                };
                let id = bindings.world.borrow().channels[&name].id;
                Ok(Value::UserData(lua.create_userdata(Handle {
                    id,
                    name,
                    bindings: bindings.clone(),
                })?))
            })?,
        )?;
        let bindings = b.clone();
        table.set(
            "destroy_channel",
            lua.create_function(move |lua, value: Value| {
                // C luaL_checkudata produces a plain '?'-named type error.
                let check = || {
                    mlua::Error::external(super::error::PlainError(
                        format!(
                            "bad argument #1 to '?' (btmux.channel expected, got {})",
                            super::error::lua_type_name(&value)
                        )
                        .into_bytes(),
                    ))
                };
                let Value::UserData(userdata) = &value else {
                    return Err(check());
                };
                let h = userdata.borrow::<Handle>().map_err(|_| check())?;
                if !Rc::ptr_eq(&h.bindings, &bindings) {
                    return Err(error("channel no longer exists"));
                }
                let name = h.name(lua)?;
                bindings.service(lua).destroy(&name).map_err(error)
            })?,
        )?;
        let bindings = b.clone();
        table.set(
            "list_channels",
            lua.create_function(move |lua, ()| {
                require_runtime(lua, "list_channels")?;
                let mut channels = bindings
                    .world
                    .borrow()
                    .channels
                    .values()
                    .map(|c| (c.name.clone(), c.id))
                    .collect::<Vec<_>>();
                // C sorts by strcasecmp with the original spelling as tie-breaker.
                channels.sort_by(|left, right| {
                    left.0
                        .to_ascii_lowercase()
                        .cmp(&right.0.to_ascii_lowercase())
                        .then_with(|| left.0.cmp(&right.0))
                });
                let result = lua.create_table()?;
                for (i, (name, id)) in channels.into_iter().enumerate() {
                    result.set(
                        i + 1,
                        lua.create_userdata(Handle {
                            id,
                            name,
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
