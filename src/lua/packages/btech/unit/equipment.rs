//! Onboard special equipment commands: ECM suites, anti-pod gear, beacons, extinguishers, and concealment toggles.

use super::super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, _world: &SharedWorld) -> mlua::Result<()> {
    for (name, suite, mode) in [
        (
            "unit_ecm",
            crate::ElectronicSuite::Guardian,
            crate::ElectronicMode::Ecm,
        ),
        (
            "unit_eccm",
            crate::ElectronicSuite::Guardian,
            crate::ElectronicMode::Eccm,
        ),
        (
            "unit_angelecm",
            crate::ElectronicSuite::Angel,
            crate::ElectronicMode::Ecm,
        ),
        (
            "unit_angeleccm",
            crate::ElectronicSuite::Angel,
            crate::ElectronicMode::Eccm,
        ),
    ] {
        let callback = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let selected = crate::btech::electronics::configure(
                    &scripts,
                    ObjectId(unit),
                    ObjectId(pilot),
                    suite,
                    mode,
                )
                .map_err(mlua::Error::external)?;
                detached(lua, &selected)
            })
        })?;
        native.set(name, error::wrap(lua, callback, "btech.operation.failed")?)?;
    }
    let pods = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let rows =
            crate::inspect_battle_pods(&scripts.world.borrow(), ObjectId(unit), ObjectId(pilot))
                .map_err(mlua::Error::external)?;
        detached(lua, &rows)
    })?;
    native.set(
        "unit_pods",
        error::wrap(lua, pods, "btech.operation.failed")?,
    )?;
    let extinguish = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::begin_battle_vehicle_extinguishing_action(
                &scripts,
                ObjectId(unit),
                ObjectId(pilot),
            )
            .map_err(mlua::Error::external)?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_extinguish",
        error::wrap(lua, extinguish, "btech.operation.failed")?,
    )?;
    let remove_all = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::begin_battle_pod_removal_action(&scripts, ObjectId(unit), ObjectId(pilot))
                .map_err(mlua::Error::external)?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_removepods",
        error::wrap(lua, remove_all, "btech.operation.failed")?,
    )?;
    let removal = lua.create_function(
        |lua, (unit, pilot, section, kind): (i64, i64, String, String)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let config = crate::lua::configuration(lua);
                let location =
                    crate::btech::pods::section(&scripts.world.borrow(), ObjectId(unit), &section)
                        .map_err(mlua::Error::external)?;
                let report = crate::remove_battle_pod_action(
                    &scripts,
                    &config,
                    ObjectId(unit),
                    ObjectId(pilot),
                    location,
                    crate::btech::pods::kind(&kind),
                    crate::btech::physical::configured_rules(&config).fall,
                )
                .map_err(mlua::Error::external)?;
                detached(lua, &report)
            })
        },
    )?;
    native.set(
        "unit_removepod",
        error::wrap(lua, removal, "btech.operation.failed")?,
    )?;
    let inarc = lua.create_function(
        |lua, (unit, pilot, index, selector): (i64, i64, usize, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let mode = crate::btech::inarc::selector(selector.as_deref());
                let text = crate::btech::inarc::select(
                    &mut scripts.world.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    index,
                    mode,
                )
                .map_err(mlua::Error::external)?;
                crate::btech::notify_unit_text(&scripts, ObjectId(unit), &text)
                    .map_err(mlua::Error::external)?;
                detached(lua, &mode)
            })
        },
    )?;
    native.set(
        "unit_inarc",
        error::wrap(lua, inarc, "btech.operation.failed")?,
    )?;
    for (name, explosive) in [("unit_narc", false), ("unit_explosive", true)] {
        let callback =
            lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    let mode = if explosive {
                        crate::toggle_battle_explosive(
                            &mut scripts.world.borrow_mut(),
                            ObjectId(unit),
                            ObjectId(pilot),
                            index,
                        )
                    } else {
                        crate::toggle_battle_narc(
                            &mut scripts.world.borrow_mut(),
                            ObjectId(unit),
                            ObjectId(pilot),
                            index,
                        )
                    }
                    .map_err(mlua::Error::external)?;
                    crate::btech::notify_unit_text(
                        &scripts,
                        ObjectId(unit),
                        &crate::btech::narc::message(mode, index, explosive),
                    )
                    .map_err(mlua::Error::external)?;
                    detached(lua, &mode)
                })
            })?;
        native.set(name, error::wrap(lua, callback, "btech.operation.failed")?)?;
    }
    let ams = lua.create_function(
        move |lua, (unit, pilot, enabled): (i64, i64, Option<bool>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::btech::ams::configure(&scripts, ObjectId(unit), ObjectId(pilot), enabled)
                    .map_err(mlua::Error::external)
            })
        },
    )?;
    native.set("unit_ams", error::wrap(lua, ams, "btech.operation.failed")?)?;
    let null_signature = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notice = crate::toggle_battle_null_signature(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit(&scripts, notice)
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_nss",
        error::wrap(lua, null_signature, "btech.operation.failed")?,
    )?;
    let stealth = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notice = crate::toggle_battle_stealth(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit(&scripts, notice)
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_stealth",
        error::wrap(lua, stealth, "btech.operation.failed")?,
    )?;
    let searchlight = lua.create_function(move |lua, (unit, pilot, mode): (i64, i64, Value)| {
        crate::lua::transactions::require(lua)?;
        let mode = match mode {
            Value::Nil => None,
            mode => Some(
                crate::SearchlightMode::from_stored(i64::from(constants::require(
                    mode,
                    3,
                    "mode",
                    &constants::SEARCHLIGHT_MODES,
                )?))
                .map_err(mlua::Error::external)?,
            ),
        };
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mut world = scripts.world.borrow_mut();
            let notice = match mode {
                None => {
                    crate::toggle_battle_searchlight(&mut world, ObjectId(unit), ObjectId(pilot))
                }
                Some(mode) => crate::set_battle_searchlight_mode(
                    &mut world,
                    ObjectId(unit),
                    ObjectId(pilot),
                    mode,
                ),
            }
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            drop(world);
            crate::btech::notify_unit(&scripts, notice)
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_slite",
        error::wrap(lua, searchlight, "btech.operation.failed")?,
    )?;
    let hide = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notice = crate::begin_battle_hiding(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit(&scripts, notice)
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_hide",
        error::wrap(lua, hide, "btech.operation.failed")?,
    )?;
    Ok(())
}
