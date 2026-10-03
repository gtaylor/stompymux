//! Unit weapon, diagnostic, critical, and aiming inspection native bindings.

use super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let weapons_world = world.clone();
    let weapons = lua.create_function(move |lua, unit: i64| {
        crate::lua::transactions::require(lua)?;
        if weapons_world
            .borrow()
            .btech
            .vehicles()
            .contains_key(&ObjectId(unit))
        {
            let weapons = crate::btech::firing::vehicle_weapon_states(
                &weapons_world.borrow(),
                ObjectId(unit),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            return detached(lua, &weapons);
        }
        let weapons = crate::btech::firing::weapon_states(&weapons_world.borrow(), ObjectId(unit))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &weapons)
    })?;
    native.set(
        "unit_weapons",
        error::wrap(lua, weapons, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let diagnostics = lua.create_function(move |lua, id: i64| {
        crate::lua::transactions::require(lua)?;
        let rows = crate::battle_weapon_diagnostics(&shared.borrow(), ObjectId(id))
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &rows)
    })?;
    native.set(
        "unit_weapon_diagnostics",
        error::wrap(lua, diagnostics, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let specifications = lua.create_function(move |lua, id: i64| {
        crate::lua::transactions::require(lua)?;
        let rows = crate::battle_weapon_specifications(
            &shared.borrow(),
            ObjectId(id),
            crate::lua::configuration(lua).battletech.erange != 0,
        )
        .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &rows)
    })?;
    native.set(
        "unit_weapon_specifications",
        error::wrap(lua, specifications, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let criticals = lua.create_function(move |lua, (id, section): (i64, String)| {
        crate::lua::transactions::require(lua)?;
        let report = crate::battle_critical_report(&shared.borrow(), ObjectId(id), &section)
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_criticals",
        error::wrap(lua, criticals, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let aimed = lua.create_function(move |lua, id: i64| {
        crate::lua::transactions::require(lua)?;
        let selected = crate::battle_aimed_section(&shared.borrow(), ObjectId(id))
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &selected)
    })?;
    native.set(
        "unit_aimed_section",
        error::wrap(lua, aimed, "btech.operation.failed")?,
    )?;
    let target = lua.create_function(|lua, (id, pilot, section): (i64, i64, Option<String>)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let selected = crate::lua::transactions::run(lua, &scripts.world, || {
            crate::btech::aimed_target::action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(id),
                ObjectId(pilot),
                section.as_deref().filter(|section| *section != "-"),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))
        })
        .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &selected)
    })?;
    native.set(
        "unit_target",
        error::wrap(lua, target, "btech.operation.failed")?,
    )?;
    Ok(())
}
