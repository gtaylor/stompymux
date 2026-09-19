//! Unit movement and posture commands: heading, speed, torso and turret rotation, stance, entry, takeoff, and jumps.

use super::super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, _world: &SharedWorld) -> mlua::Result<()> {
    for (name, heading) in [("unit_heading", true), ("unit_speed", false)] {
        let operation = lua.create_function(
            move |lua, (unit, pilot, value): (i64, i64, Option<Value>)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                let Some(value) = value.filter(|value| !matches!(value, Value::Nil)) else {
                    let motion = crate::battle_motion_readout(
                        &scripts.world(),
                        ObjectId(unit),
                        ObjectId(pilot),
                    )
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                    return Ok(Value::Number(if heading {
                        motion.heading
                    } else {
                        motion.speed
                    }));
                };
                let argument = match value {
                    Value::Integer(value) => value.to_string(),
                    Value::Number(value) => value.to_string(),
                    Value::String(value) => value.to_str()?.to_owned(),
                    _ => {
                        return Err(error::failure(
                            "btech.operation.failed",
                            "Expected a number or speed name",
                        ));
                    }
                };
                let value = if heading {
                    argument
                        .parse::<f64>()
                        .map_err(|_| error::failure("btech.operation.failed", "Invalid heading"))?
                } else {
                    crate::btech::motion_controls::speed_request(
                        &scripts.world(),
                        ObjectId(unit),
                        &argument,
                        crate::btech::SpeedPolicy::configured(&crate::lua::configuration(lua)),
                    )
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?
                };
                crate::lua::transactions::run(lua, &scripts.world, || {
                    let notice = if heading {
                        crate::set_battle_heading(
                            &mut scripts.world.borrow_mut(),
                            ObjectId(unit),
                            ObjectId(pilot),
                            value,
                        )
                    } else {
                        crate::btech::set_speed_configured(
                            &mut scripts.world.borrow_mut(),
                            ObjectId(unit),
                            ObjectId(pilot),
                            value,
                            crate::btech::SpeedPolicy::configured(&crate::lua::configuration(lua)),
                            crate::lua::configuration(lua).battletech.nofusionvtolfuel != 0,
                        )
                    }
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                    crate::btech::notify_unit(&scripts, notice)
                        .map_err(|e| error::failure("btech.operation.failed", e))?;
                    Ok(Value::Boolean(true))
                })
            },
        )?;
        native.set(name, error::wrap(lua, operation, "btech.operation.failed")?)?;
    }
    let fixturret = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notice = crate::begin_battle_turret_repair(
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
        "unit_fixturret",
        error::wrap(lua, fixturret, "btech.operation.failed")?,
    )?;
    let turret = lua.create_function(
        move |lua, (unit, pilot, heading): (i64, i64, Option<f64>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let Some(heading) = heading else {
                return crate::battle_turret_readout(
                    &scripts.world(),
                    ObjectId(unit),
                    ObjectId(pilot),
                )
                .map(Value::Number)
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")));
            };
            crate::lua::transactions::run(lua, &scripts.world, || {
                let notice = crate::set_battle_turret(
                    &mut scripts.world.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    heading,
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                crate::btech::notify_unit(&scripts, notice)
                    .map_err(|e| error::failure("btech.operation.failed", e))?;
                Ok(Value::Boolean(true))
            })
        },
    )?;
    native.set(
        "unit_turret",
        error::wrap(lua, turret, "btech.operation.failed")?,
    )?;
    let torso = lua.create_function(move |lua, (unit, pilot, direction): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let direction = match direction.trim().to_ascii_lowercase().as_str() {
            "l" | "left" => crate::BattleTorso::Left,
            "r" | "right" => crate::BattleTorso::Right,
            "c" | "center" => crate::BattleTorso::Center,
            _ => {
                return Err(error::failure(
                    "btech.operation.failed",
                    "Expected left, right or center",
                ));
            }
        };
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notice = crate::rotate_battle_torso(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                direction,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit(&scripts, notice)
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_rottorso",
        error::wrap(lua, torso, "btech.operation.failed")?,
    )?;
    let prone = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let report = crate::prone_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(unit),
                ObjectId(pilot),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        })
    })?;
    native.set(
        "unit_prone",
        error::wrap(lua, prone, "btech.operation.failed")?,
    )?;
    let stand = lua.create_function(
        move |lua, (unit, pilot, mode): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let mode = match mode.as_deref().unwrap_or("normal") {
                "normal" => crate::BattleStandMode::Normal,
                "anyway" => crate::BattleStandMode::Anyway,
                "careful" => crate::BattleStandMode::Careful,
                _ => {
                    return Err(error::failure(
                        "btech.operation.failed",
                        "Expected normal, anyway or careful stand mode",
                    ));
                }
            };
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let config = crate::lua::configuration(lua);
                let attempt = crate::btech::stand::configured_stand(
                    &scripts,
                    &config,
                    ObjectId(unit),
                    ObjectId(pilot),
                    mode,
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                detached(lua, &attempt)
            })
        },
    )?;
    native.set(
        "unit_stand",
        error::wrap(lua, stand, "btech.operation.failed")?,
    )?;
    let enterbase = lua.create_function(
        |lua, (unit, pilot, direction): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::btech::building_actions::enterbase(
                    &scripts,
                    ObjectId(unit),
                    ObjectId(pilot),
                    direction.as_deref().unwrap_or_default(),
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))
            })
        },
    )?;
    native.set(
        "unit_enterbase",
        error::wrap(lua, enterbase, "btech.operation.failed")?,
    )?;
    let takeoff = lua.create_function(|lua, (unit, pilot, delay): (i64, i64, Option<u16>)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::btech::vtol_controls::configured_takeoff(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(unit),
                ObjectId(pilot),
                delay.unwrap_or(0),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_takeoff",
        error::wrap(lua, takeoff, "btech.operation.failed")?,
    )?;
    let vertical = lua.create_function(|lua, (unit, pilot, speed): (i64, i64, Option<f64>)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let config = crate::lua::configuration(lua);
        let Some(speed) = speed else {
            return crate::battle_vtol_vertical_readout(
                &scripts.world(),
                ObjectId(unit),
                ObjectId(pilot),
                config.battletech.nofusionvtolfuel != 0,
            )
            .map(Value::Number)
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")));
        };
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::btech::vtol_controls::configured_vertical(
                &scripts,
                &config,
                ObjectId(unit),
                ObjectId(pilot),
                speed,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(Value::Boolean(true))
        })
    })?;
    native.set(
        "unit_vertical",
        error::wrap(lua, vertical, "btech.operation.failed")?,
    )?;
    for (name, pickup) in [("unit_pickup", true), ("unit_dropoff", false)] {
        let action =
            lua.create_function(move |lua, (unit, pilot, target): (i64, i64, Option<i64>)| {
                crate::lua::transactions::require(lua)?;
                if pickup != target.is_some() {
                    return Err(mlua::Error::external(
                        "Pickup requires a target; dropoff takes none",
                    ));
                }
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    crate::battle_tow_action(
                        &scripts,
                        &crate::lua::configuration(lua),
                        ObjectId(unit),
                        ObjectId(pilot),
                        target.map(ObjectId),
                    )
                    .map_err(mlua::Error::external)?;
                    Ok(true)
                })
            })?;
        native.set(name, error::wrap(lua, action, "btech.operation.failed")?)?;
    }
    let hull_down = lua.create_function(
        move |lua, (unit, pilot, argument): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::battle_hull_down_action(
                    &scripts,
                    ObjectId(unit),
                    ObjectId(pilot),
                    argument.as_deref().unwrap_or(""),
                )
                .map_err(mlua::Error::external)?;
                Ok(true)
            })
        },
    )?;
    native.set(
        "unit_hulldown",
        error::wrap(lua, hull_down, "btech.operation.failed")?,
    )?;
    let dig = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::battle_dig_action(&scripts, ObjectId(unit), ObjectId(pilot))
                .map_err(mlua::Error::external)?;
            Ok(true)
        })
    })?;
    native.set("unit_dig", error::wrap(lua, dig, "btech.operation.failed")?)?;
    let land = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let config = crate::lua::configuration(lua);
            crate::btech::landing::configured_land(
                &scripts,
                &config,
                ObjectId(unit),
                ObjectId(pilot),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_land",
        error::wrap(lua, land, "btech.operation.failed")?,
    )?;
    let dfa = lua.create_function(|lua, (unit, pilot, target): (i64, i64, Option<i64>)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::btech::launch_dfa_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(unit),
                ObjectId(pilot),
                target.map(ObjectId),
            )
            .map_err(mlua::Error::external)?;
            Ok(true)
        })
    })?;
    native.set("unit_dfa", error::wrap(lua, dfa, "btech.operation.failed")?)?;
    let jump = lua.create_function(
        move |lua, (unit, pilot, bearing, range): (i64, i64, i32, f64)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::btech::launch_jump_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(unit),
                    ObjectId(pilot),
                    bearing,
                    range,
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                Ok(true)
            })
        },
    )?;
    native.set(
        "unit_jump",
        error::wrap(lua, jump, "btech.operation.failed")?,
    )?;
    Ok(())
}
