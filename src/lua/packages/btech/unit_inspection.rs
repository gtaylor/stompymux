//! Unit weapon, diagnostic, critical, aiming, and gunner inspection native bindings.

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
        let report = crate::battle_critical_report(
            &shared.borrow(),
            ObjectId(id),
            &section,
            crate::lua::configuration(lua).battletech.parts != 0,
        )
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
    // Station facade guards reuse installed display bindings and preserve their argument grammar.
    for method in [
        "bearing",
        "range_report",
        "vector",
        "eta",
        "findcenter",
        "tactical",
        "lrsmap",
        "navigate",
        "scan",
        "report",
        "scan_hex",
        "scan_building",
        "scan_terrain",
        "scan_selected",
    ] {
        let measure: mlua::Function = native.get(format!("unit_{method}"))?;
        let shared = world.clone();
        let station_measure = lua.create_function(move |lua, arguments: mlua::MultiValue| {
            crate::lua::transactions::require(lua)?;
            let station = <i64 as mlua::FromLua>::from_lua(
                arguments.front().cloned().unwrap_or(Value::Nil),
                lua,
            )?;
            let gunner = <i64 as mlua::FromLua>::from_lua(
                arguments.get(1).cloned().unwrap_or(Value::Nil),
                lua,
            )?;
            crate::gunner_context(&shared.borrow(), ObjectId(station), ObjectId(gunner))
                .map_err(mlua::Error::external)?;
            measure.call::<Value>(arguments)
        })?;
        native.set(
            format!("gunner_{method}"),
            error::wrap(lua, station_measure, "btech.operation.failed")?,
        )?;
    }
    Ok(())
}
