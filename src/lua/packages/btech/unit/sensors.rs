//! Sensor and contact commands: sensor selection, sensor reports, contact lists, and scan actions.

use super::super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let shared = world.clone();
    let sensors = lua.create_function(
        move |lua, (unit, pilot, primary, secondary): (i64, i64, String, String)| {
            crate::lua::transactions::require(lua)?;
            let result = (|| -> anyhow::Result<()> {
                crate::select_battle_optical_sensors(
                    &mut shared.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    crate::BattleSensorPair {
                        primary: primary.parse()?,
                        secondary: secondary.parse()?,
                    },
                )
            })();
            result.map_err(|e| error::failure("btech.operation.failed", e))?;
            Ok(true)
        },
    )?;
    native.set(
        "unit_sensors",
        error::wrap(lua, sensors, "btech.operation.failed")?,
    )?;
    let report_world = world.clone();
    let sensor_report = lua.create_function(move |_, (unit, verbose): (i64, Option<bool>)| {
        crate::battle_sensor_report(
            &report_world.borrow(),
            ObjectId(unit),
            verbose.unwrap_or(false),
        )
        .map_err(mlua::Error::external)
    })?;
    native.set(
        "unit_sensor_report",
        error::wrap(lua, sensor_report, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let contacts = lua.create_function(move |lua, (unit, preferences): (i64, Option<Table>)| {
        crate::lua::transactions::require(lua)?;
        let contacts = match preferences {
            Some(table) => crate::filtered_battle_contacts(
                &shared.borrow(),
                ObjectId(unit),
                lua.from_value(Value::Table(table))?,
            ),
            None => crate::visible_battle_contacts(&shared.borrow(), ObjectId(unit)),
        }
        .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &contacts)
    })?;
    native.set(
        "unit_contacts",
        error::wrap(lua, contacts, "btech.operation.failed")?,
    )?;
    let report_unit = lua.create_function(move |lua, (unit, pilot, target): (i64, i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::report_battle_unit(
            &scripts.world.borrow(),
            ObjectId(unit),
            ObjectId(pilot),
            ObjectId(target),
        )
        .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))
    })?;
    native.set(
        "unit_report",
        error::wrap(lua, report_unit, "btech.operation.failed")?,
    )?;
    let scan_selected = lua.create_function(
        move |lua, (unit, pilot, options): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let config = crate::lua::configuration(lua);
            let report = crate::scan_battle_selected_action(
                &scripts,
                &config,
                ObjectId(unit),
                ObjectId(pilot),
                options.as_deref().unwrap_or(""),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_scan_selected",
        error::wrap(lua, scan_selected, "btech.operation.failed")?,
    )?;
    let scan_terrain =
        lua.create_function(move |lua, (unit, pilot, x, y): (i64, i64, i32, i32)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let config = crate::lua::configuration(lua);
            let report = crate::scan_battle_hex_action(
                &scripts,
                &config,
                ObjectId(unit),
                ObjectId(pilot),
                crate::BattleHexCoordinate { x, y },
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        })?;
    native.set(
        "unit_scan_terrain",
        error::wrap(lua, scan_terrain, "btech.operation.failed")?,
    )?;
    let scan_building =
        lua.create_function(move |lua, (unit, pilot, x, y): (i64, i64, i32, i32)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let config = crate::lua::configuration(lua);
            let report = crate::scan_battle_building_action(
                &scripts,
                &config,
                ObjectId(unit),
                ObjectId(pilot),
                crate::BattleHexCoordinate { x, y },
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        })?;
    native.set(
        "unit_scan_building",
        error::wrap(lua, scan_building, "btech.operation.failed")?,
    )?;
    let scan_hex = lua.create_function(
        move |lua, (unit, pilot, x, y, options): (i64, i64, i32, i32, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::scan_battle_hex_unit_action(
                &scripts,
                ObjectId(unit),
                ObjectId(pilot),
                crate::BattleHexCoordinate { x, y },
                options.as_deref().unwrap_or(""),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))
        },
    )?;
    native.set(
        "unit_scan_hex",
        error::wrap(lua, scan_hex, "btech.operation.failed")?,
    )?;
    let scan = lua.create_function(
        move |lua, (unit, pilot, target, options): (i64, i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::scan_battle_unit_action(
                &scripts,
                ObjectId(unit),
                ObjectId(pilot),
                ObjectId(target),
                options.as_deref().unwrap_or(""),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))
        },
    )?;
    native.set(
        "unit_scan",
        error::wrap(lua, scan, "btech.operation.failed")?,
    )?;
    Ok(())
}
