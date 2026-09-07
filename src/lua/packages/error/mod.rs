//! Structured Lua errors and immutable codes used by editable test helpers.
use mlua::{AnyUserData, Lua, MetaMethod, Table, UserData, UserDataMethods};

#[derive(Clone)]
struct Code(String);
impl UserData for Code {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Eq, |_, code, other: AnyUserData| {
            Ok(other.borrow::<Code>().is_ok_and(|other| code.0 == other.0))
        });
        m.add_meta_method(MetaMethod::ToString, |_, c, ()| Ok(c.0.clone()));
        m.add_meta_method(MetaMethod::Index, |_, c, key: String| {
            if key == "code" {
                Ok(c.0.clone())
            } else {
                Err(mlua::Error::runtime("unknown error code field"))
            }
        });
    }
}
struct Codes;
impl UserData for Codes {
    fn add_methods<M: UserDataMethods<Self>>(m: &mut M) {
        m.add_meta_method(MetaMethod::Index, |lua, _, key: String| {
            if !["assertion", "runtime"].contains(&key.as_str()) {
                return Err(mlua::Error::runtime("unknown testing error code"));
            }
            lua.create_userdata(Code(format!("testing.{key}")))
        });
    }
}

pub(super) fn install(lua: &Lua, mux: &Table) -> mlua::Result<()> {
    let api: Table = lua.load(include_str!("api.lua")).eval()?;
    api.set(
        "code_tree",
        lua.create_function(|lua, root: String| {
            if root != "testing" {
                return Err(mlua::Error::runtime("unknown error code root"));
            }
            lua.create_userdata(Codes)
        })?,
    )?;
    mux.set("error", api)
}
