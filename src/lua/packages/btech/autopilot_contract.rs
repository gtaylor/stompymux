//! Trusted Lua management of unit-attached ground autopilots.

use super::{constants, contract, detached, error};
use crate::btech::autopilot::{
    AutopilotConfig, AutopilotConfigPatch, AutopilotController, AutopilotFireMode, AutopilotOrder,
    AutopilotRangeBand, AutopilotSubmissionMode,
};
use crate::{ObjectId, SharedWorld, btech::BattlePosition};
use mlua::{Lua, LuaSerdeExt, MultiValue, Table, Value};

fn arg(args: &MultiValue, index: usize) -> Value {
    args.get(index).cloned().unwrap_or(Value::Nil)
}

fn bad(message: impl ToString) -> mlua::Error {
    error::failure("mux.arg.invalid", message)
}

fn table(value: Value, label: &str) -> mlua::Result<Table> {
    match value {
        Value::Table(table) => Ok(table),
        _ => Err(bad(format!("{label} must be a table"))),
    }
}

fn ground_unit(lua: &Lua, shared: &SharedWorld, value: Value) -> mlua::Result<ObjectId> {
    let world = shared.borrow();
    let id = contract::require_object(lua, &world, value, 1)?;
    if world.btech.constructed_units().contains_key(&id) {
        return Ok(id);
    }
    if world.btech.vehicles().get(&id).is_some_and(|vehicle| {
        matches!(
            vehicle.definition().movement,
            crate::btech::BattleVehicleMovement::Tracked
                | crate::btech::BattleVehicleMovement::Wheeled
                | crate::btech::BattleVehicleMovement::Hover
        )
    }) {
        return Ok(id);
    }
    Err(error::failure(
        "mux.object.invalid",
        "A constructed Mech or ground vehicle is required",
    ))
}

fn controller(world: &crate::World, id: ObjectId) -> mlua::Result<&AutopilotController> {
    world.btech.controllers().get(&id).ok_or_else(|| {
        error::failure(
            "mux.object.unavailable",
            "No autopilot is attached to this unit",
        )
    })
}

fn with_controller<T>(
    shared: &SharedWorld,
    id: ObjectId,
    change: impl FnOnce(&mut AutopilotController) -> mlua::Result<T>,
) -> mlua::Result<T> {
    let mut world = shared.borrow_mut();
    let time = world.btech.simulation_time();
    let controllers = &mut world.btech.controllers;
    let controller = controllers.get_mut(&id).ok_or_else(|| {
        error::failure(
            "mux.object.unavailable",
            "No autopilot is attached to this unit",
        )
    })?;
    let first = controller.next_feedback_sequence;
    let result = change(controller);
    controller.stamp_feedback(first, time);
    result
}

fn revision(value: Value) -> mlua::Result<Option<u64>> {
    match value {
        Value::Nil => Ok(None),
        Value::Integer(value) if value >= 0 => Ok(Some(value as u64)),
        _ => Err(bad("expected_revision must be a nonnegative integer")),
    }
}

fn position(lua: &Lua, world: &crate::World, value: Value) -> mlua::Result<BattlePosition> {
    let position: BattlePosition = lua.from_value(value).map_err(mlua::Error::external)?;
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .ok_or_else(|| bad("Map is unavailable"))?;
    map.hex(i64::from(position.x), i64::from(position.y))
        .map_err(mlua::Error::external)?;
    Ok(position)
}

fn range(lua: &Lua, value: Value) -> mlua::Result<Option<AutopilotRangeBand>> {
    if matches!(value, Value::Nil) {
        return Ok(None);
    }
    let range: AutopilotRangeBand = lua.from_value(value).map_err(mlua::Error::external)?;
    range.validate().map_err(mlua::Error::external)?;
    Ok(Some(range))
}

fn patch(lua: &Lua, value: Value) -> mlua::Result<AutopilotConfigPatch> {
    let options = table(value, "options")?;
    contract::check_options(
        &options,
        &[
            "speed_percent",
            "fire_mode",
            "heat_ceiling",
            "preferred_range",
        ],
        2,
    )?;
    let fire_mode = match options.raw_get::<Value>("fire_mode")? {
        Value::Nil => None,
        value => Some(
            match constants::require(value, 2, "fire mode", &constants::AUTOPILOT_FIRE_MODES)? {
                0 => AutopilotFireMode::Hold,
                1 => AutopilotFireMode::AssignedTarget,
                _ => AutopilotFireMode::Opportunistic,
            },
        ),
    };
    let preferred_range = match options.raw_get::<Value>("preferred_range")? {
        Value::Nil => None,
        Value::Boolean(false) => Some(None),
        value => Some(range(lua, value)?),
    };
    Ok(AutopilotConfigPatch {
        speed_percent: options.raw_get("speed_percent")?,
        fire_mode,
        heat_ceiling: options.raw_get("heat_ceiling")?,
        preferred_range,
    })
}

pub(super) fn decode_order(
    lua: &Lua,
    world: &crate::World,
    value: Value,
) -> mlua::Result<AutopilotOrder> {
    let spec = table(value, "order")?;
    let kind = constants::require(
        spec.raw_get("kind")?,
        2,
        "order kind",
        &constants::AUTOPILOT_ORDERS,
    )?;
    let order = match kind {
        0 | 5 => {
            contract::check_options(&spec, &["kind", "destination", "arrival_radius"], 2)?;
            let destination = position(lua, world, spec.raw_get("destination")?)?;
            let arrival_radius = spec.raw_get::<Option<u16>>("arrival_radius")?.unwrap_or(0);
            if kind == 0 {
                AutopilotOrder::Move {
                    destination,
                    arrival_radius,
                }
            } else {
                AutopilotOrder::AttackMove {
                    destination,
                    arrival_radius,
                }
            }
        }
        1 => {
            contract::check_options(&spec, &["kind"], 2)?;
            AutopilotOrder::Hold
        }
        2 => {
            contract::check_options(&spec, &["kind", "target", "separation"], 2)?;
            AutopilotOrder::Follow {
                target: ObjectId(spec.raw_get("target")?),
                separation: spec.raw_get::<Option<u16>>("separation")?.unwrap_or(2),
            }
        }
        3 => {
            contract::check_options(&spec, &["kind", "waypoints"], 2)?;
            let waypoints = spec
                .raw_get::<Table>("waypoints")?
                .sequence_values::<Value>()
                .map(|value| position(lua, world, value?))
                .collect::<mlua::Result<Vec<_>>>()?;
            AutopilotOrder::Patrol { waypoints }
        }
        4 => {
            contract::check_options(&spec, &["kind", "target", "range"], 2)?;
            AutopilotOrder::Attack {
                target: ObjectId(spec.raw_get("target")?),
                range: range(lua, spec.raw_get("range")?)?,
            }
        }
        _ => return Err(bad("Unsupported autopilot order kind")),
    };
    order.validate().map_err(mlua::Error::external)?;
    Ok(order)
}

fn validate_resume(world: &crate::World, unit: ObjectId) -> mlua::Result<()> {
    let controller = controller(world, unit)?;
    let (position, destroyed) = if let Some(mech) = world.btech.constructed_units().get(&unit) {
        (mech.position(), mech.is_destroyed())
    } else if let Some(vehicle) = world.btech.vehicles().get(&unit) {
        (vehicle.position(), vehicle.is_destroyed())
    } else {
        return Err(bad("Unit is unavailable"));
    };
    if destroyed || position.is_none() {
        return Err(bad("Autopilot unit must be intact and placed"));
    }
    let map = position.expect("position checked above").map;
    let orders = controller
        .active_order()
        .into_iter()
        .chain(controller.queued_orders());
    for record in orders {
        let order_map = match &record.order {
            AutopilotOrder::Move { destination, .. }
            | AutopilotOrder::AttackMove { destination, .. } => Some(destination.map),
            AutopilotOrder::Patrol { waypoints } => waypoints.first().map(|waypoint| waypoint.map),
            AutopilotOrder::Hold
            | AutopilotOrder::Follow { .. }
            | AutopilotOrder::Attack { .. } => None,
        };
        if order_map.is_some_and(|order_map| order_map != map) {
            return Err(bad("Autopilot order is on another map"));
        }
    }
    Ok(())
}

/// Register the unit-attached controller API on the private native BattleTech table.
pub(super) fn register(lua: &Lua, native: &Table, shared: &SharedWorld) -> mlua::Result<()> {
    let world = shared.clone();
    contract::bind(
        lua,
        native,
        "autopilot_attach",
        "autopilot",
        "attach",
        lua.create_function(move |lua, args: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let id = ground_unit(lua, &world, arg(&args, 0))?;
            let config = if matches!(arg(&args, 1), Value::Nil) {
                AutopilotConfig::default()
            } else {
                let mut controller = AutopilotController::new();
                controller
                    .configure(patch(lua, arg(&args, 1))?, None)
                    .map_err(mlua::Error::external)?;
                controller.config().clone()
            };
            let mut world = world.borrow_mut();
            let controllers = &mut world.btech.controllers;
            if controllers.contains_key(&id) {
                return Err(bad("Autopilot already attached"));
            }
            controllers.insert(
                id,
                AutopilotController::with_config(config).map_err(mlua::Error::external)?,
            );
            Ok(())
        })?,
    )?;

    let world = shared.clone();
    contract::bind(
        lua,
        native,
        "autopilot_detach",
        "autopilot",
        "detach",
        lua.create_function(move |lua, value: Value| {
            crate::lua::transactions::require(lua)?;
            let id = ground_unit(lua, &world, value)?;
            let mut world = world.borrow_mut();
            if !world.btech.controllers().contains_key(&id) {
                return Err(bad("No autopilot is attached"));
            }
            let _ = crate::btech::set_speed_autopilot(&mut world, id, 0.0);
            world
                .btech
                .controllers
                .remove(&id)
                .expect("controller checked above");
            world.btech.autopilot_plans.remove(&id);
            Ok(())
        })?,
    )?;

    let world = shared.clone();
    contract::bind(
        lua,
        native,
        "autopilot_configure",
        "autopilot",
        "configure",
        lua.create_function(move |lua, args: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let id = ground_unit(lua, &world, arg(&args, 0))?;
            let patch = patch(lua, arg(&args, 1))?;
            with_controller(&world, id, |controller| {
                controller
                    .configure(patch, revision(arg(&args, 2))?)
                    .map_err(mlua::Error::external)?;
                Ok(controller.revision())
            })
        })?,
    )?;

    let world = shared.clone();
    contract::bind(
        lua,
        native,
        "autopilot_submit",
        "autopilot",
        "submit",
        lua.create_function(move |lua, args: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let id = ground_unit(lua, &world, arg(&args, 0))?;
            let specs = table(arg(&args, 1), "orders")?;
            let orders = {
                let borrowed = world.borrow();
                specs
                    .sequence_values::<Value>()
                    .map(|value| {
                        let order = decode_order(lua, &borrowed, value?)?;
                        crate::btech::autopilot::orders::validate_for_unit(&borrowed, id, &order)
                            .map_err(mlua::Error::external)?;
                        Ok(order)
                    })
                    .collect::<mlua::Result<Vec<_>>>()?
            };
            let mode = match constants::require(
                arg(&args, 2),
                3,
                "submission mode",
                &constants::AUTOPILOT_SUBMISSION_MODES,
            )? {
                0 => AutopilotSubmissionMode::Append,
                _ => AutopilotSubmissionMode::Replace,
            };
            let result = with_controller(&world, id, |controller| {
                let ids = controller
                    .submit(orders, mode, revision(arg(&args, 3))?)
                    .map_err(mlua::Error::external)?;
                detached(
                    lua,
                    &serde_json::json!({"ids": ids, "revision": controller.revision()}),
                )
            })?;
            world.borrow_mut().btech.autopilot_plans.remove(&id);
            let empty = world.borrow().btech.controllers()[&id].order_count() == 0;
            if empty {
                let _ = crate::btech::set_speed_autopilot(&mut world.borrow_mut(), id, 0.0);
            }
            Ok(result)
        })?,
    )?;

    let world = shared.clone();
    contract::bind(
        lua,
        native,
        "autopilot_cancel",
        "autopilot",
        "cancel",
        lua.create_function(move |lua, args: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let id = ground_unit(lua, &world, arg(&args, 0))?;
            let order_id = match arg(&args, 1) {
                Value::Integer(id) if id > 0 => id as u64,
                _ => return Err(bad("order_id must be positive")),
            };
            let canceled = with_controller(&world, id, |controller| {
                controller
                    .cancel(order_id, revision(arg(&args, 2))?)
                    .map_err(mlua::Error::external)
            })?;
            if canceled {
                world.borrow_mut().btech.autopilot_plans.remove(&id);
                let empty = world.borrow().btech.controllers()[&id].order_count() == 0;
                if empty {
                    let _ = crate::btech::set_speed_autopilot(&mut world.borrow_mut(), id, 0.0);
                }
            }
            Ok(canceled)
        })?,
    )?;

    for (key, name, resume) in [
        ("autopilot_pause", "pause", false),
        ("autopilot_resume", "resume", true),
    ] {
        let world = shared.clone();
        contract::bind(
            lua,
            native,
            key,
            "autopilot",
            name,
            lua.create_function(move |lua, value: Value| {
                crate::lua::transactions::require(lua)?;
                let id = ground_unit(lua, &world, value)?;
                if resume {
                    validate_resume(&world.borrow(), id)?;
                }
                with_controller(&world, id, |controller| {
                    if resume {
                        controller.resume(None)
                    } else {
                        controller.pause(None)
                    }
                    .map_err(mlua::Error::external)
                })?;
                world.borrow_mut().btech.autopilot_plans.remove(&id);
                if !resume {
                    let _ = crate::btech::set_speed_autopilot(&mut world.borrow_mut(), id, 0.0);
                }
                Ok(())
            })?,
        )?;
    }

    let world = shared.clone();
    contract::bind(
        lua,
        native,
        "autopilot_status",
        "autopilot",
        "status",
        lua.create_function(move |lua, value: Value| {
            crate::lua::transactions::require(lua)?;
            let id = ground_unit(lua, &world, value)?;
            let borrowed = world.borrow();
            detached(lua, controller(&borrowed, id)?)
        })?,
    )?;

    let world = shared.clone();
    contract::bind(
        lua,
        native,
        "autopilot_observe",
        "autopilot",
        "observe",
        lua.create_function(move |lua, value: Value| {
            crate::lua::transactions::require(lua)?;
            let id = ground_unit(lua, &world, value)?;
            let borrowed = world.borrow();
            let controller = controller(&borrowed, id)?;
            let observed = crate::btech::autopilot::observations::observe_with_memory(
                &borrowed,
                id,
                borrowed.btech.simulation_time(),
                controller.sightings(),
            )
            .map_err(mlua::Error::external)?;
            detached(lua, &observed)
        })?,
    )?;

    let world = shared.clone();
    contract::bind(
        lua,
        native,
        "autopilot_feedback",
        "autopilot",
        "feedback",
        lua.create_function(move |lua, args: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let id = ground_unit(lua, &world, arg(&args, 0))?;
            let after = revision(arg(&args, 1))?;
            let borrowed = world.borrow();
            detached(lua, &controller(&borrowed, id)?.feedback_since(after))
        })?,
    )?;
    Ok(())
}
