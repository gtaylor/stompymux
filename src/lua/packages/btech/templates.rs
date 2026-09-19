//! Template, loadout, and map-file native bindings.

use super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, _world: &SharedWorld) -> mlua::Result<()> {
    let template = lua.create_function(|lua, name: String| {
        crate::lua::transactions::require(lua)?;
        let config = crate::lua::configuration(lua);
        let template =
            crate::btech::read_template(&config.path(&config.database.mech_database), &name)
                .map_err(|e| error::failure("btech.template.invalid", format!("{e:#}")))?;
        detached(lua, &template)
    })?;
    native.set(
        "template",
        error::wrap(lua, template, "btech.template.invalid")?,
    )?;
    let template_check = lua.create_function(|lua, name: String| {
        crate::lua::transactions::require(lua)?;
        let config = crate::lua::configuration(lua);
        let template =
            crate::btech::read_template(&config.path(&config.database.mech_database), &name)
                .map_err(|e| error::failure("btech.template.invalid", format!("{e:#}")))?;
        detached(lua, &crate::check_battle_template(&template))
    })?;
    native.set(
        "template_check",
        error::wrap(lua, template_check, "btech.template.invalid")?,
    )?;
    let loadout = lua.create_function(|lua, name: String| {
        crate::lua::transactions::require(lua)?;
        let config = crate::lua::configuration(lua);
        let result =
            crate::btech::read_template(&config.path(&config.database.mech_database), &name)
                .and_then(|template| crate::BattleLoadout::resolve(&template))
                .map_err(|e| error::failure("btech.template.invalid", format!("{e:#}")))?;
        detached(lua, &result)
    })?;
    native.set(
        "loadout",
        error::wrap(lua, loadout, "btech.template.invalid")?,
    )?;
    let map = lua.create_function(|lua, name: String| {
        crate::lua::transactions::require(lua)?;
        let config = crate::lua::configuration(lua);
        let map = crate::btech::read_map(&config.path(&config.database.map_database), &name)
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        // Inspection deliberately returns metadata; a million-hex Lua table is not needed.
        let result = lua.create_table()?;
        result.set("width", map.width)?;
        result.set("height", map.height)?;
        result.set("gravity", map.gravity)?;
        result.set("temperature", map.temperature)?;
        result.set("flags", map.flags)?;
        Ok(result)
    })?;
    native.set("mapfile", error::wrap(lua, map, "btech.operation.failed")?)?;
    Ok(())
}
