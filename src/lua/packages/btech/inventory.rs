//! Inventory, cargo, and part native bindings.

use super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let add_stores = lua.create_function(
        |lua, (actor, object, pattern, quantity): (i64, i64, String, i32)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::add_battle_stores_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(object),
                    &pattern,
                    quantity,
                )
                .map_err(mlua::Error::external)
            })
        },
    )?;
    native.set(
        "inventory_add_stores",
        error::wrap(lua, add_stores, "btech.operation.failed")?,
    )?;
    for (name, add) in [("inventory_add", true), ("inventory_remove", false)] {
        let change = lua.create_function(
            move |lua, (actor, object, pattern, quantity): (i64, i64, String, i32)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    let change = if add {
                        crate::BattleInventoryChange::Add { pattern, quantity }
                    } else {
                        crate::BattleInventoryChange::Remove { pattern, quantity }
                    };
                    let rows = crate::change_battle_inventory_action(
                        &scripts,
                        &crate::lua::configuration(lua),
                        ObjectId(actor),
                        ObjectId(object),
                        change,
                    )
                    .map_err(mlua::Error::external)?;
                    detached(lua, &rows)
                })
            },
        )?;
        native.set(name, error::wrap(lua, change, "btech.operation.failed")?)?;
    }
    let fix = lua.create_function(|lua, (actor, object): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let report = crate::clean_battle_inventory_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(actor),
                ObjectId(object),
            )
            .map_err(mlua::Error::external)?;
            detached(lua, &report)
        })
    })?;
    native.set(
        "inventory_fix",
        error::wrap(lua, fix, "btech.operation.failed")?,
    )?;
    let clear = lua.create_function(|lua, (actor, object): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::change_battle_inventory_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(actor),
                ObjectId(object),
                crate::BattleInventoryChange::Clear,
            )
            .map_err(mlua::Error::external)?;
            Ok(())
        })
    })?;
    native.set(
        "inventory_clear",
        error::wrap(lua, clear, "btech.operation.failed")?,
    )?;
    let fuel = lua.create_function(|lua, id: i64| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let status = crate::battle_vtol_fuel_status(&scripts.world(), ObjectId(id))
            .map_err(mlua::Error::external)?;
        detached(lua, &status)
    })?;
    native.set(
        "unit_fuel",
        error::wrap(lua, fuel, "btech.operation.failed")?,
    )?;
    let set_fuel = lua.create_function(|lua, (actor, id, amount): (i64, i64, u32)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let status = crate::set_battle_vtol_fuel(
                &mut scripts.world_mut(),
                &crate::lua::configuration(lua),
                ObjectId(actor),
                ObjectId(id),
                amount,
            )
            .map_err(mlua::Error::external)?;
            detached(lua, &status)
        })
    })?;
    native.set(
        "unit_set_fuel",
        error::wrap(lua, set_fuel, "btech.operation.failed")?,
    )?;
    for (name, stores) in [("cargo_manifest", false), ("cargo_stores", true)] {
        let read = lua.create_function(move |lua, (actor, pattern): (i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let rows = crate::battle_cargo_manifest(
                &scripts.world(),
                &crate::lua::configuration(lua),
                ObjectId(actor),
                stores,
                pattern.as_deref().unwrap_or(""),
            )
            .map_err(mlua::Error::external)?;
            detached(lua, &rows)
        })?;
        native.set(name, error::wrap(lua, read, "btech.operation.failed")?)?;
    }
    for (name, load) in [("cargo_load", true), ("cargo_unload", false)] {
        let transfer =
            lua.create_function(move |lua, (actor, pattern, quantity): (i64, String, i32)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    let rows = crate::transfer_battle_cargo_action(
                        &scripts,
                        &crate::lua::configuration(lua),
                        ObjectId(actor),
                        load,
                        &pattern,
                        quantity,
                    )
                    .map_err(mlua::Error::external)?;
                    detached(lua, &rows)
                })
            })?;
        native.set(name, error::wrap(lua, transfer, "btech.operation.failed")?)?;
    }

    let shared = world.clone();
    let point = lua.create_function(move |lua, map: i64| {
        crate::lua::transactions::require(lua)?;
        let world = shared.borrow();
        let map = world
            .btech
            .maps()
            .get(&ObjectId(map))
            .ok_or_else(|| mlua::Error::external("Map not found"))?;
        detached(lua, &map.cargo_transfer_point())
    })?;
    native.set(
        "map_cargo_point",
        error::wrap(lua, point, "btech.operation.failed")?,
    )?;
    let set_point = lua.create_function(|lua, (actor, map, point): (i64, i64, Value)| {
        crate::lua::transactions::require(lua)?;
        let point: Option<crate::BattleCargoTransferPoint> = lua.from_value(point)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::set_battle_cargo_transfer_point(
                &mut scripts.world_mut(),
                ObjectId(actor),
                ObjectId(map),
                point,
            )
            .map_err(mlua::Error::external)
        })
    })?;
    native.set(
        "map_set_cargo_point",
        error::wrap(lua, set_point, "btech.operation.failed")?,
    )?;

    let part = lua.create_function(|lua, value: Value| {
        crate::lua::transactions::require(lua)?;
        let part = match value {
            Value::Integer(id) => i32::try_from(id)
                .ok()
                .and_then(crate::BattlePart::from_id)
                .ok_or_else(|| mlua::Error::external("Unknown inventory part"))?,
            Value::String(name) => {
                crate::BattlePart::parse(name.to_str()?.as_ref()).map_err(mlua::Error::external)?
            }
            _ => return Err(mlua::Error::external("Expected part identifier or name")),
        };
        detached(lua, &part)
    })?;
    native.set(
        "inventory_part",
        error::wrap(lua, part, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let mass = lua.create_function(move |lua, object: i64| {
        crate::lua::transactions::require(lua)?;
        crate::battle_inventory_mass(&shared.borrow(), ObjectId(object))
            .map_err(mlua::Error::external)
    })?;
    native.set(
        "inventory_mass",
        error::wrap(lua, mass, "btech.operation.failed")?,
    )?;
    let named = lua.create_function(
        |lua, (actor, object, name, quantity): (i64, i64, String, i32)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let part = crate::BattlePart::parse(&name).map_err(mlua::Error::external)?;
                crate::set_battle_inventory_quantity_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(object),
                    part.part_id,
                    quantity,
                )
                .map_err(mlua::Error::external)
            })
        },
    )?;
    native.set(
        "inventory_set_named",
        error::wrap(lua, named, "btech.operation.failed")?,
    )?;

    let shared = world.clone();
    let inventory = lua.create_function(move |lua, object: i64| {
        crate::lua::transactions::require(lua)?;
        let world = shared.borrow();
        let entries =
            crate::battle_inventory(&world, ObjectId(object)).map_err(mlua::Error::external)?;
        detached(lua, &entries)
    })?;
    native.set(
        "inventory_read",
        error::wrap(lua, inventory, "btech.operation.failed")?,
    )?;
    let inventory_set = lua.create_function(
        |lua, (actor, object, part, quantity): (i64, i64, i32, i32)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::set_battle_inventory_quantity_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(object),
                    part,
                    quantity,
                )
                .map_err(mlua::Error::external)
            })
        },
    )?;
    native.set(
        "inventory_set",
        error::wrap(lua, inventory_set, "btech.operation.failed")?,
    )?;

    Ok(())
}
