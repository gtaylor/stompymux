//! Special munition commands: incendiary, inferno, precision, swarm, semi-guided, MML, ATM, and other rounds.

use super::super::*;
use crate::btech::AmmunitionFeedback;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, _world: &SharedWorld) -> mlua::Result<()> {
    let incendiary = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_incendiary(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(
                &scripts,
                ObjectId(unit),
                &mode.incendiary_message(index),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_incendiary",
        error::wrap(lua, incendiary, "btech.operation.failed")?,
    )?;
    let inferno = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_inferno(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.inferno_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_inferno",
        error::wrap(lua, inferno, "btech.operation.failed")?,
    )?;
    let precision = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_precision(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(
                &scripts,
                ObjectId(unit),
                &mode.precision_message(index),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_precision",
        error::wrap(lua, precision, "btech.operation.failed")?,
    )?;
    for (name, friend_or_foe) in [("unit_fireswarm", false), ("unit_fireswarm1", true)] {
        let action = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let mode = crate::toggle_battle_swarm(
                    &mut scripts.world.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    index,
                    friend_or_foe,
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                crate::btech::notify_unit_text(
                    &scripts,
                    ObjectId(unit),
                    &mode.swarm_message(index),
                )
                .map_err(|e| error::failure("btech.operation.failed", e))?;
                detached(lua, &mode)
            })
        })?;
        native.set(name, error::wrap(lua, action, "btech.operation.failed")?)?;
    }
    let semiguided = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_semiguided(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(
                &scripts,
                ObjectId(unit),
                &mode.semiguided_message(index),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_sguided",
        error::wrap(lua, semiguided, "btech.operation.failed")?,
    )?;
    let mml_ammunition =
        lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let mode = crate::toggle_mml_ammunition(
                    &mut scripts.world.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    index,
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.mml_message(index))
                    .map_err(|e| error::failure("btech.operation.failed", e))?;
                detached(lua, &mode)
            })
        })?;
    native.set(
        "unit_mml",
        error::wrap(lua, mml_ammunition, "btech.operation.failed")?,
    )?;
    for (name, mode) in [
        ("unit_atmrange", crate::BattleAmmunitionMode::ExtendedRange),
        (
            "unit_atmexplosive",
            crate::BattleAmmunitionMode::HighExplosive,
        ),
    ] {
        let callback =
            lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    let selected = crate::toggle_atm_ammunition(
                        &mut scripts.world.borrow_mut(),
                        ObjectId(unit),
                        ObjectId(pilot),
                        index,
                        mode,
                    )
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                    crate::btech::notify_unit_text(
                        &scripts,
                        ObjectId(unit),
                        &selected.atm_message(index),
                    )
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                    detached(lua, &selected)
                })
            })?;
        native.set(name, error::wrap(lua, callback, "btech.operation.failed")?)?;
    }
    let stinger = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_stinger(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.stinger_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_stinger",
        error::wrap(lua, stinger, "btech.operation.failed")?,
    )?;
    let flechette = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_flechette(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(
                &scripts,
                ObjectId(unit),
                &mode.flechette_message(index),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_flechette",
        error::wrap(lua, flechette, "btech.operation.failed")?,
    )?;
    Ok(())
}
