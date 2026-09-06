//! Checked lock results retain denial metadata and roll back malformed callback results.
use crate::lua::{Outbox, SharedWorld, err, transactions};
use mlua::{Function, Lua, Table, Value};

/// C LBUF_SIZE bounds optional lock messages, excluding their terminator.
const MESSAGE_LIMIT: usize = 8192;

/// Install a private lock invoker; public Lua callers still receive a boolean.
pub(super) fn register(
    lua: &Lua,
    api: &Table,
    world: &SharedWorld,
    outbox: &Outbox,
) -> mlua::Result<()> {
    let w = world.clone();
    let out = outbox.clone();
    api.set(
        "evaluate_lock",
        lua.create_function(move |lua, (f, ctx): (Function, Table)| {
            transactions::run(lua, &w, &out, || {
                let value = f.call::<Value>(ctx)?;
                let result = lua.create_table()?;
                match value {
                    Value::Boolean(v) => result.set("passes", v)?,
                    Value::Table(t) => {
                        for pair in t.pairs::<Value, Value>() {
                            let (key, _) = pair?;
                            match key {
                                Value::String(k)
                                    if matches!(
                                        k.to_str()?.as_ref(),
                                        "passes" | "enactor_message" | "other_message"
                                    ) => {}
                                _ => return Err(err("invalid lock result field")),
                            }
                        }
                        let Value::Boolean(passes) = t.get::<Value>("passes")? else {
                            return Err(err("lock result passes must be boolean"));
                        };
                        result.set("passes", passes)?;
                        for key in ["enactor_message", "other_message"] {
                            match t.get::<Value>(key)? {
                                Value::Nil => {}
                                Value::String(v) if v.as_bytes().len() < MESSAGE_LIMIT => {
                                    v.to_str()?;
                                    result.set(key, v)?;
                                }
                                _ => {
                                    return Err(err(
                                        "lock message must be a string shorter than 8192 bytes",
                                    ));
                                }
                            }
                        }
                    }
                    _ => return Err(err("lock must return a boolean or result table")),
                }
                Ok(result)
            })
        })?,
    )
}
