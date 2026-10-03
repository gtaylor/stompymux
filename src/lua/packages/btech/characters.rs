//! Player character, skill, advancement, contact-option, and evacuation native bindings.

use super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let contact_options =
        lua.create_function(|lua, (options, brief_buildings): (String, Option<bool>)| {
            let options = crate::parse_battle_contact_options_for_display(
                &options,
                brief_buildings.unwrap_or(false),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &options)
        })?;
    native.set(
        "player_contact_options",
        error::wrap(lua, contact_options, "btech.operation.failed")?,
    )?;

    let shared = world.clone();
    let character = lua.create_function(move |lua, player: i64| {
        crate::lua::transactions::require(lua)?;
        let world = shared.borrow();
        let profile = world
            .btech
            .characters()
            .get(&ObjectId(player))
            .ok_or_else(|| {
                error::failure("btech.operation.failed", "Character state is unavailable")
            })?;
        let result = detached(lua, profile)?;
        if let Value::Table(table) = &result {
            table.set(
                "perception_target",
                crate::battle_perception_target(&world, ObjectId(player))
                    .map_err(|e| error::failure("btech.operation.failed", e))?,
            )?;
            let values = match world.btech.character_values().get(&ObjectId(player)) {
                Some(values) => detached(lua, values)?,
                None => Value::Table(lua.create_table()?),
            };
            table.set("values", values)?;

            table.set(
                "unconscious_remaining",
                world
                    .btech
                    .recoveries()
                    .get(&ObjectId(player))
                    .map_or(0, |recovery| recovery.remaining),
            )?;
        }
        Ok(result)
    })?;
    native.set(
        "character_state",
        error::wrap(lua, character, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let character_list = lua.create_function(move |lua, arguments: mlua::MultiValue| {
        crate::lua::transactions::require(lua)?;
        if !(1..=2).contains(&arguments.len()) {
            return Err(mlua::Error::external(
                "Expected a category and optional character",
            ));
        }
        let kind = <String as mlua::FromLua>::from_lua(arguments[0].clone(), lua)?;
        let player = arguments
            .get(1)
            .cloned()
            .map(|value| match value {
                Value::String(name) => shared
                    .borrow()
                    .find_player(name.to_str()?.as_ref())
                    .ok_or_else(|| mlua::Error::external("Invalid character target")),
                value => <i64 as mlua::FromLua>::from_lua(value, lua).map(ObjectId),
            })
            .transpose()?;
        let values = crate::btech::character_list::list(&shared.borrow(), &kind, player)
            .map_err(mlua::Error::external)?;
        detached(lua, &values)
    })?;
    native.set(
        "character_list",
        error::wrap(lua, character_list, "btech.operation.failed")?,
    )?;
    let advantages = lua.create_function(|lua, ()| detached(lua, crate::BATTLE_ADVANTAGES))?;
    native.set(
        "character_advantages",
        error::wrap(lua, advantages, "btech.operation.failed")?,
    )?;
    let skills = lua.create_function(|lua, ()| detached(lua, crate::BATTLE_SKILLS))?;
    native.set(
        "character_skills",
        error::wrap(lua, skills, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let threshold = lua.create_function(move |lua, name: String| {
        crate::lua::transactions::require(lua)?;
        crate::battle_skill_threshold(&shared.borrow(), &name).map_err(mlua::Error::external)
    })?;
    native.set(
        "character_threshold",
        error::wrap(lua, threshold, "btech.operation.failed")?,
    )?;
    let threshold_set =
        lua.create_function(|lua, (actor, name, threshold): (i64, String, i64)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::edit_battle_skill_threshold(&scripts, ObjectId(actor), &name, threshold)
                    .map_err(mlua::Error::external)?;
                Ok(true)
            })
        })?;
    native.set(
        "character_set_threshold",
        error::wrap(lua, threshold_set, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let progress = lua.create_function(move |lua, (player, name): (i64, String)| {
        crate::lua::transactions::require(lua)?;
        let progress = crate::battle_skill_progress(&shared.borrow(), ObjectId(player), &name)
            .map_err(mlua::Error::external)?;
        detached(lua, &progress)
    })?;
    native.set(
        "character_progress",
        error::wrap(lua, progress, "btech.operation.failed")?,
    )?;
    let evacuate = lua.create_function(|lua, (unit, actor): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        if !crate::authority::is_wizard(&scripts.world.borrow(), ObjectId(actor)) {
            return Err(error::failure(
                "btech.operation.failed",
                "Permission denied.",
            ));
        }
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::btech::evacuate(&scripts, &crate::lua::configuration(lua), ObjectId(unit))
                .map_err(mlua::Error::external)
        })
    })?;
    native.set(
        "unit_evacuate",
        error::wrap(lua, evacuate, "btech.operation.failed")?,
    )?;
    Ok(())
}
