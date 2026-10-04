//! Target selection and gunnery query commands: sniping, TAG, spotting, locks, and aimed-shot modifiers.

use super::super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let snipe = lua.create_function(
        move |lua, (unit, pilot, target, selection): (i64, i64, i64, String)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::battle_snipe_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(unit),
                    ObjectId(pilot),
                    ObjectId(target),
                    &selection,
                )
                .map_err(mlua::Error::external)
            })?;
            Ok(true)
        },
    )?;
    native.set(
        "unit_snipe",
        error::wrap(lua, snipe, "btech.operation.failed")?,
    )?;
    let tag = lua.create_function(move |lua, (unit, pilot, target): (i64, i64, Option<i64>)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notices = crate::select_battle_tag(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                target.map(ObjectId),
            )
            .map_err(mlua::Error::external)?;
            for notice in notices {
                crate::btech::notify_unit(&scripts, notice).map_err(mlua::Error::external)?;
            }
            Ok(true)
        })
    })?;
    native.set("unit_tag", error::wrap(lua, tag, "btech.operation.failed")?)?;
    let spot =
        lua.create_function(move |lua, (unit, pilot, target): (i64, i64, Option<i64>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let notices = crate::select_battle_spotter(
                    &mut scripts.world.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    target.map(ObjectId),
                )
                .map_err(mlua::Error::external)?;
                for notice in notices {
                    crate::btech::notify_unit(&scripts, notice).map_err(mlua::Error::external)?;
                }
                Ok(true)
            })
        })?;
    native.set(
        "unit_spot",
        error::wrap(lua, spot, "btech.operation.failed")?,
    )?;
    let gunnery_world = world.clone();
    let gunnery = lua.create_function(move |lua, (unit, weapon): (i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let config = crate::lua::configuration(lua);
        crate::battle_unit_gunnery_target(
            &gunnery_world.borrow(),
            ObjectId(unit),
            weapon,
            config.battletech.extended_gunnery != 0,
        )
        .map_err(|e| error::failure("btech.operation.failed", e))
    })?;
    native.set(
        "unit_gunnery",
        error::wrap(lua, gunnery, "btech.operation.failed")?,
    )?;
    let hex_aim_world = world.clone();
    let aim_hex =
        lua.create_function(move |lua, (unit, weapon, x, y): (i64, usize, i32, i32)| {
            crate::lua::transactions::require(lua)?;
            let config = crate::lua::configuration(lua);
            let report = crate::battle_pilot_hex_aim_modifiers(
                &hex_aim_world.borrow(),
                ObjectId(unit),
                crate::HexCoordinate { x, y },
                weapon,
                config.battletech.extended_gunnery != 0,
                crate::BattleAimRules::configured(&config.battletech),
            )
            .map_err(mlua::Error::external)?;
            let result = detached(lua, &report)?;
            if let Value::Table(table) = &result {
                table.set("subtotal", report.subtotal())?;
            }
            Ok(result)
        })?;
    native.set(
        "unit_aim_hex",
        error::wrap(lua, aim_hex, "btech.operation.failed")?,
    )?;
    let lock_world = world.clone();
    let lock =
        lua.create_function(move |lua, (unit, pilot, target): (i64, i64, Option<i64>)| {
            crate::lua::transactions::require(lua)?;
            crate::select_battle_target(
                &mut lock_world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                target.map(ObjectId),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            Ok(true)
        })?;
    native.set(
        "unit_lock",
        error::wrap(lua, lock, "btech.operation.failed")?,
    )?;
    {
        let lock_hex = lua.create_function(
            move |lua, (unit, pilot, x, y, mode): (i64, i64, i32, i32, Option<String>)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    let mode = mode
                        .as_deref()
                        .map(str::parse)
                        .transpose()
                        .map_err(mlua::Error::external)?
                        .unwrap_or(crate::BattleHexTargetMode::UnitAtHex);
                    let notice = crate::select_battle_hex_target(
                        &mut scripts.world.borrow_mut(),
                        ObjectId(unit),
                        ObjectId(pilot),
                        crate::HexCoordinate { x, y },
                        mode,
                    )
                    .map_err(mlua::Error::external)?;
                    crate::btech::notify_unit(&scripts, notice).map_err(mlua::Error::external)?;
                    Ok(true)
                })
            },
        )?;
        native.set(
            "unit_lock_hex",
            error::wrap(lua, lock_hex, "btech.operation.failed")?,
        )?;
    }
    Ok(())
}
