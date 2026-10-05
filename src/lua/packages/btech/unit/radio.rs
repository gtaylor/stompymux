//! Radio commands: targeted and broadcast sends, frequencies, channel titles, and modes.

use super::super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, _world: &SharedWorld) -> mlua::Result<()> {
    let radio_target = lua.create_function(
        move |lua, (unit, pilot, target, message): (i64, i64, i64, String)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::send_targeted_radio_action(
                &scripts,
                ObjectId(unit),
                ObjectId(pilot),
                ObjectId(target),
                &message,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_radio_target",
        error::wrap(lua, radio_target, "btech.operation.failed")?,
    )?;
    let radio_send = lua.create_function(
        move |lua, (unit, pilot, channel, message): (i64, i64, u8, String)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let config = crate::lua::configuration(lua);
            let report = crate::send_radio_action(
                &scripts,
                &config,
                ObjectId(unit),
                ObjectId(pilot),
                channel,
                &message,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_radio_send",
        error::wrap(lua, radio_send, "btech.operation.failed")?,
    )?;
    let radio_frequency = lua.create_function(
        move |lua, (unit, pilot, channel, frequency): (i64, i64, u8, u32)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::set_radio_frequency_action(
                &scripts,
                ObjectId(unit),
                ObjectId(pilot),
                channel,
                frequency,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        },
    )?;
    native.set(
        "unit_radio_frequency",
        error::wrap(lua, radio_frequency, "btech.operation.failed")?,
    )?;
    let radio_title = lua.create_function(
        move |lua, (unit, pilot, channel, title): (i64, i64, u8, String)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::set_radio_title(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                channel,
                &title,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        },
    )?;
    native.set(
        "unit_radio_title",
        error::wrap(lua, radio_title, "btech.operation.failed")?,
    )?;
    let radio_mode = lua.create_function(
        move |lua, (unit, pilot, channel, mode): (i64, i64, u8, String)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let mut world = scripts.world.borrow_mut();
            let capabilities = crate::unit_radio_capabilities(&world, ObjectId(unit))
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            let mode = crate::RadioMode::parse(&mode, capabilities)
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::set_radio_mode(&mut world, ObjectId(unit), ObjectId(pilot), channel, mode)
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        },
    )?;
    native.set(
        "unit_radio_mode",
        error::wrap(lua, radio_mode, "btech.operation.failed")?,
    )?;
    Ok(())
}
