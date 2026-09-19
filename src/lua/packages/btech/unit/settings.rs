//! Unit and pilot preference commands: behavior and warning toggles, heat cutoff, and contact or view settings.

use super::super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let shared = world.clone();
    let auto_fall = lua.create_function(move |lua, (unit, pilot, enabled): (i64, i64, bool)| {
        crate::lua::transactions::require(lua)?;
        crate::set_battle_auto_fall(
            &mut shared.borrow_mut(),
            ObjectId(unit),
            ObjectId(pilot),
            enabled,
        )
        .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        Ok(true)
    })?;
    native.set(
        "unit_auto_fall",
        error::wrap(lua, auto_fall, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let searchlight_warning =
        lua.create_function(move |lua, (unit, pilot, enabled): (i64, i64, bool)| {
            crate::lua::transactions::require(lua)?;
            crate::set_battle_searchlight_warning(
                &mut shared.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                enabled,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        })?;
    native.set(
        "unit_searchlight_warning",
        error::wrap(lua, searchlight_warning, "btech.operation.failed")?,
    )?;
    for (name, setter) in [
        (
            "unit_mw_safety",
            crate::set_battle_mw_safety
                as fn(&mut crate::World, ObjectId, ObjectId, bool) -> anyhow::Result<()>,
        ),
        (
            "unit_bth_debug",
            crate::set_battle_bth_debug
                as fn(&mut crate::World, ObjectId, ObjectId, bool) -> anyhow::Result<()>,
        ),
        (
            "unit_autocon_shutdown",
            crate::set_battle_autocon_shutdown
                as fn(&mut crate::World, ObjectId, ObjectId, bool) -> anyhow::Result<()>,
        ),
        (
            "unit_friendly_fire_safety",
            crate::set_battle_friendly_fire_safety
                as fn(&mut crate::World, ObjectId, ObjectId, bool) -> anyhow::Result<()>,
        ),
        (
            "unit_armor_warning",
            crate::set_battle_armor_warning
                as fn(&mut crate::World, ObjectId, ObjectId, bool) -> anyhow::Result<()>,
        ),
        (
            "unit_ammunition_warning",
            crate::set_battle_ammunition_warning
                as fn(&mut crate::World, ObjectId, ObjectId, bool) -> anyhow::Result<()>,
        ),
    ] {
        let shared = world.clone();
        let operation =
            lua.create_function(move |lua, (unit, pilot, enabled): (i64, i64, bool)| {
                crate::lua::transactions::require(lua)?;
                setter(
                    &mut shared.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    enabled,
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                Ok(true)
            })?;
        native.set(name, error::wrap(lua, operation, "btech.operation.failed")?)?;
    }
    let shared = world.clone();
    let contact_preferences =
        lua.create_function(move |lua, (player, value): (i64, Option<Table>)| {
            crate::lua::transactions::require(lua)?;
            if let Some(table) = value {
                let preferences: crate::BattleContactPreferences =
                    lua.from_value(Value::Table(table))?;
                crate::lua::transactions::run(lua, &shared, || {
                    crate::set_battle_contact_preferences(
                        &mut shared.borrow_mut(),
                        ObjectId(player),
                        preferences,
                    )
                    .map_err(|e| error::failure("btech.operation.failed", e))
                })?;
            }
            let preferences = crate::battle_contact_preferences(&shared.borrow(), ObjectId(player))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &preferences)
        })?;
    native.set(
        "player_contact_preferences",
        error::wrap(lua, contact_preferences, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let view_dimensions =
        lua.create_function(move |lua, (player, value): (i64, Option<Table>)| {
            crate::lua::transactions::require(lua)?;
            if let Some(table) = value {
                let dimensions: crate::BattleViewDimensions =
                    lua.from_value(Value::Table(table))?;
                crate::lua::transactions::run(lua, &shared, || {
                    crate::set_battle_view_dimensions(
                        &mut shared.borrow_mut(),
                        ObjectId(player),
                        dimensions,
                    )
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))
                })?;
            }
            let dimensions = crate::battle_view_dimensions(&shared.borrow(), ObjectId(player))
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &dimensions)
        })?;
    native.set(
        "player_view_dimensions",
        error::wrap(lua, view_dimensions, "btech.operation.failed")?,
    )?;
    let heat_cutoff = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let config = crate::lua::configuration(lua);
            let notice = crate::toggle_battle_heat_cutoff(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                config.battletech.heatcutoff > 0,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit(&scripts, notice)
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_heatcutoff",
        error::wrap(lua, heat_cutoff, "btech.operation.failed")?,
    )?;
    Ok(())
}
