//! Self-destruct commands: safety flag, initiation, and reactor explosion.

use super::super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, _world: &SharedWorld) -> mlua::Result<()> {
    let explode_safe = lua.create_function(move |lua, (unit, safe): (i64, bool)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::set_battle_self_destruct_safe(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                safe,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_explode_safe",
        error::wrap(lua, explode_safe, "btech.operation.failed")?,
    )?;
    let explode = lua.create_function(move |lua, (unit, pilot, text): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let config = crate::lua::configuration(lua);
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::self_destruct_action(&scripts, &config, ObjectId(unit), ObjectId(pilot), &text)
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_explode",
        error::wrap(lua, explode, "btech.operation.failed")?,
    )?;
    let reactor_explode = lua.create_function(move |lua, unit: i64| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let config = crate::lua::configuration(lua);
        crate::lua::transactions::run(lua, &scripts.world, || {
            let report = crate::reactor_explosion_action(&scripts, &config, ObjectId(unit))
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        })
    })?;
    native.set(
        "unit_reactor_explode",
        error::wrap(lua, reactor_explode, "btech.operation.failed")?,
    )?;
    Ok(())
}
