//! Typed access constants for Lua command declarations.
use crate::world::{ObjectId, World};
use mlua::{AnyUserData, Lua, MetaMethod, Table, UserData, UserDataMethods, Value};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum CommandAccess {
    #[default]
    Public,
    Wizard,
    God,
}

impl CommandAccess {
    fn name(self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::Wizard => "WIZARD",
            Self::God => "GOD",
        }
    }

    pub(crate) fn allows(self, world: &World, player: ObjectId) -> bool {
        match self {
            Self::Public => true,
            Self::Wizard => crate::authority::is_wizard(world, player),
            Self::God => player == ObjectId(1),
        }
    }
}

impl UserData for CommandAccess {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::ToString, |_, access, ()| Ok(access.name()));
        methods.add_meta_function(MetaMethod::Eq, |_, (left, right): (Value, Value)| {
            // C's access __eq luaL_checkudatas both operands (command_access.c
            // lua_command_access_equal).
            super::packages::error::typed_eq::<Self, _>(
                "btmux.command_access",
                left,
                right,
                |a, b| a == b,
            )
        });
        methods.add_meta_method(
            MetaMethod::NewIndex,
            |_, _, _: (Value, Value)| -> mlua::Result<()> {
                Err(super::packages::error::failure(
                    "mux.access.invalid",
                    "command access constants are immutable",
                ))
            },
        );
    }
}

struct AccessNamespace;

impl UserData for AccessNamespace {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_meta_method(MetaMethod::Index, |lua, _, key: Value| {
            let Value::String(key) = key else {
                return Err(super::packages::error::argument_failure(
                    lua,
                    2,
                    "mux.access.invalid",
                    "constant name must be a string",
                ));
            };
            let bytes = key.as_bytes();
            let bytes = bytes
                .split(|byte| *byte == 0)
                .next()
                .expect("split always yields one item");
            let access = match bytes {
                b"PUBLIC" => CommandAccess::Public,
                b"WIZARD" => CommandAccess::Wizard,
                b"GOD" => CommandAccess::God,
                _ => {
                    let key = String::from_utf8_lossy(bytes);
                    return Err(super::packages::error::argument_failure(
                        lua,
                        2,
                        "mux.access.invalid",
                        format!("unknown command access constant '{key}'"),
                    ));
                }
            };
            push(lua, access)
        });
        methods.add_meta_method(
            MetaMethod::NewIndex,
            |_, _, _: (Value, Value)| -> mlua::Result<()> {
                Err(super::packages::error::failure(
                    "mux.access.invalid",
                    "mux.world.access constants are immutable",
                ))
            },
        );
    }
}

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

fn push(lua: &Lua, access: CommandAccess) -> mlua::Result<Value> {
    let value = lua.create_userdata(access)?;
    let immutable = lua.create_function(|_, _: (Value, Value, Value)| -> mlua::Result<()> {
        Err(super::packages::error::failure(
            "mux.access.invalid",
            "command access constants are immutable",
        ))
    })?;
    let immutable = super::packages::error::wrap(lua, immutable, "mux.access.invalid")?;
    set_metamethod(lua, &value, "__newindex", &immutable)?;
    protect(lua, &value, "protected command access constant metatable")?;
    Ok(Value::UserData(value))
}

pub(crate) fn install(lua: &Lua, world: &Table) -> mlua::Result<()> {
    let namespace = lua.create_userdata(AccessNamespace)?;
    let index = lua.create_function(|lua, (_namespace, key): (Value, Value)| {
        let Value::String(key) = key else {
            return Err(super::packages::error::argument_failure(
                lua,
                2,
                "mux.access.invalid",
                "constant name must be a string",
            ));
        };
        let bytes = key.as_bytes();
        let bytes = bytes
            .split(|byte| *byte == 0)
            .next()
            .expect("split always yields one item");
        let access = match bytes {
            b"PUBLIC" => CommandAccess::Public,
            b"WIZARD" => CommandAccess::Wizard,
            b"GOD" => CommandAccess::God,
            _ => {
                let key = String::from_utf8_lossy(bytes);
                return Err(super::packages::error::argument_failure(
                    lua,
                    2,
                    "mux.access.invalid",
                    format!("unknown command access constant '{key}'"),
                ));
            }
        };
        push(lua, access)
    })?;
    let immutable = lua.create_function(|_, _: (Value, Value, Value)| -> mlua::Result<()> {
        Err(super::packages::error::failure(
            "mux.access.invalid",
            "mux.world.access constants are immutable",
        ))
    })?;
    let immutable = super::packages::error::wrap(lua, immutable, "mux.access.invalid")?;
    set_metamethod(lua, &namespace, "__index", &index)?;
    set_metamethod(lua, &namespace, "__newindex", &immutable)?;
    protect(
        lua,
        &namespace,
        "protected command access namespace metatable",
    )?;
    world.set("access", namespace)
}

fn set_metamethod(
    lua: &Lua,
    value: &AnyUserData,
    name: &'static str,
    function: &mlua::Function,
) -> mlua::Result<()> {
    unsafe {
        lua.exec_raw::<()>((value.clone(), name, function.clone()), |state| {
            if mlua::ffi::lua_getmetatable(state, 1) != 0 {
                mlua::ffi::lua_pushvalue(state, 3);
                mlua::ffi::lua_setfield(
                    state,
                    -2,
                    mlua::ffi::lua_tolstring(state, 2, std::ptr::null_mut()),
                );
            }
        })
    }
}

pub(crate) fn read(entry: &Table) -> mlua::Result<CommandAccess> {
    match entry.get::<Value>("access")? {
        Value::Nil => Ok(CommandAccess::Public),
        Value::UserData(value) => Ok(value
            .borrow::<CommandAccess>()
            .map(|value| *value)
            .map_err(|_| mlua::Error::RuntimeError("invalid command access".into()))?),
        _ => Err(mlua::Error::RuntimeError("invalid command access".into())),
    }
}
