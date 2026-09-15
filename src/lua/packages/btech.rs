//! BattleTech map and inspection bindings with callback/checking guards and detached result tables.
use super::error;
use crate::{ObjectId, SharedWorld};
use mlua::{Lua, LuaSerdeExt, Table, Value};

/// Register closures before facade installation; no live data escapes checking mode.
pub(super) fn register(lua: &Lua, api: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let native = lua.create_table()?;
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
        |lua, (actor, object, name, brand, quantity): (i64, i64, String, u8, i32)| {
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
                    brand,
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
        |lua, (actor, object, part, brand, quantity): (i64, i64, i32, u8, i32)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::set_battle_inventory_quantity_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(object),
                    part,
                    brand,
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

    let shared = world.clone();
    let settings = lua.create_function(move |lua, name: String| {
        crate::lua::transactions::require(lua)?;
        let weapon =
            crate::BattleWeapon::parse_operator_name(&name).map_err(mlua::Error::external)?;
        detached(lua, &shared.borrow().btech.weapon_settings().get(weapon))
    })?;
    native.set(
        "weapon_settings",
        error::wrap(lua, settings, "btech.operation.failed")?,
    )?;
    for (name, recycle) in [
        ("weapon_set_recycle", true),
        ("weapon_set_battle_value", false),
    ] {
        let setting =
            lua.create_function(move |lua, (actor, name, value): (i64, String, i64)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    let values = crate::edit_battle_weapon_settings(
                        &scripts,
                        &crate::lua::configuration(lua),
                        ObjectId(actor),
                        &name,
                        value,
                        recycle,
                    )
                    .map_err(mlua::Error::external)?;
                    detached(lua, &values)
                })
            })?;
        native.set(name, error::wrap(lua, setting, "btech.operation.failed")?)?;
    }
    let bootlegger = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_bootlegger(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_bootlegger",
        error::wrap(lua, bootlegger, "btech.operation.failed")?,
    )?;
    let vector = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_vector_report(
                &scripts.world(),
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_vector",
        error::wrap(lua, vector, "btech.operation.failed")?,
    )?;
    let range = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_range_report(
                &scripts.world(),
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_range_report",
        error::wrap(lua, range, "btech.operation.failed")?,
    )?;
    let bearing = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_bearing(
                &scripts.world(),
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_bearing",
        error::wrap(lua, bearing, "btech.operation.failed")?,
    )?;
    let eta = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_eta_action(
                &scripts,
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set("unit_eta", error::wrap(lua, eta, "btech.operation.failed")?)?;
    let supercharger = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_supercharger(&scripts, ObjectId(unit), ObjectId(pilot))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_supercharger",
        error::wrap(lua, supercharger, "btech.operation.failed")?,
    )?;
    let c3_targets = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3_targets(&scripts.world(), ObjectId(unit), ObjectId(pilot))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_c3_targets",
        error::wrap(lua, c3_targets, "btech.operation.failed")?,
    )?;
    let c3_network = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3_status(&scripts.world(), ObjectId(unit), ObjectId(pilot))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_c3_network",
        error::wrap(lua, c3_network, "btech.operation.failed")?,
    )?;
    let c3i_targets = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3i_targets(&scripts.world(), ObjectId(unit), ObjectId(pilot))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_c3i_targets",
        error::wrap(lua, c3i_targets, "btech.operation.failed")?,
    )?;
    let c3i_network = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3i_status(&scripts.world(), ObjectId(unit), ObjectId(pilot))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_c3i_network",
        error::wrap(lua, c3i_network, "btech.operation.failed")?,
    )?;
    let c3_message = lua.create_function(|lua, (unit, pilot, message): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3_message(&scripts, ObjectId(unit), ObjectId(pilot), &message)
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_c3_message",
        error::wrap(lua, c3_message, "btech.operation.failed")?,
    )?;
    let c3i_message = lua.create_function(|lua, (unit, pilot, message): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3i_message(&scripts, ObjectId(unit), ObjectId(pilot), &message)
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_c3i_message",
        error::wrap(lua, c3i_message, "btech.operation.failed")?,
    )?;
    let c3 = lua.create_function(|lua, (unit, pilot, arguments): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3(&scripts, ObjectId(unit), ObjectId(pilot), &arguments)
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set("unit_c3", error::wrap(lua, c3, "btech.operation.failed")?)?;
    let c3i = lua.create_function(|lua, (unit, pilot, arguments): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3i(&scripts, ObjectId(unit), ObjectId(pilot), &arguments)
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set("unit_c3i", error::wrap(lua, c3i, "btech.operation.failed")?)?;
    let masc = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_masc(&scripts, ObjectId(unit), ObjectId(pilot))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_masc",
        error::wrap(lua, masc, "btech.operation.failed")?,
    )?;
    let dump = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_dump(
                &scripts,
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_dump",
        error::wrap(lua, dump, "btech.operation.failed")?,
    )?;
    let turnmode = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_turnmode(
                &scripts,
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_turnmode",
        error::wrap(lua, turnmode, "btech.operation.failed")?,
    )?;
    let lateral = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_lateral(
                &scripts,
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_lateral",
        error::wrap(lua, lateral, "btech.operation.failed")?,
    )?;
    let brief = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_brief(
                &scripts,
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_brief",
        error::wrap(lua, brief, "btech.operation.failed")?,
    )?;
    let building_contacts = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let contacts = crate::battle_building_contacts(&scripts, ObjectId(unit), ObjectId(pilot))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &contacts)
    })?;
    native.set(
        "unit_building_contacts",
        error::wrap(lua, building_contacts, "btech.operation.failed")?,
    )?;

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
                crate::edit_battle_skill_threshold(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    &name,
                    threshold,
                )
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
    let template = lua.create_function(|lua, name: String| {
        crate::lua::transactions::require(lua)?;
        let config = crate::lua::configuration(lua);
        let template =
            crate::btech::read_template(&config.path(&config.database.mech_database), &name)
                .map_err(|e| error::failure("btech.template.invalid", format!("{e:#}")))?;
        detached(lua, &template)
    })?;
    native.set(
        "template",
        error::wrap(lua, template, "btech.template.invalid")?,
    )?;
    let template_check = lua.create_function(|lua, name: String| {
        crate::lua::transactions::require(lua)?;
        let config = crate::lua::configuration(lua);
        let template =
            crate::btech::read_template(&config.path(&config.database.mech_database), &name)
                .map_err(|e| error::failure("btech.template.invalid", format!("{e:#}")))?;
        detached(lua, &crate::check_battle_template(&template))
    })?;
    native.set(
        "template_check",
        error::wrap(lua, template_check, "btech.template.invalid")?,
    )?;
    let loadout = lua.create_function(|lua, name: String| {
        crate::lua::transactions::require(lua)?;
        let config = crate::lua::configuration(lua);
        let result =
            crate::btech::read_template(&config.path(&config.database.mech_database), &name)
                .and_then(|template| crate::BattleLoadout::resolve(&template))
                .map_err(|e| error::failure("btech.template.invalid", format!("{e:#}")))?;
        detached(lua, &result)
    })?;
    native.set(
        "loadout",
        error::wrap(lua, loadout, "btech.template.invalid")?,
    )?;
    let map = lua.create_function(|lua, name: String| {
        crate::lua::transactions::require(lua)?;
        let config = crate::lua::configuration(lua);
        let map = crate::btech::read_map(&config.path(&config.database.map_database), &name)
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        // Inspection deliberately returns metadata; a million-hex Lua table is not needed.
        let result = lua.create_table()?;
        result.set("width", map.width)?;
        result.set("height", map.height)?;
        result.set("gravity", map.gravity)?;
        result.set("temperature", map.temperature)?;
        result.set("flags", map.flags)?;
        Ok(result)
    })?;
    native.set("mapfile", error::wrap(lua, map, "btech.operation.failed")?)?;
    let shared = world.clone();
    let unit = lua.create_function(move |lua, id: i64| {
        crate::lua::transactions::require(lua)?;
        let world = shared.borrow();
        let unit = world
            .btech
            .units()
            .get(&ObjectId(id))
            .ok_or_else(|| error::failure("btech.operation.failed", "saved unit not found"))?;
        detached(lua, unit)
    })?;
    native.set("unit", error::wrap(lua, unit, "btech.operation.failed")?)?;
    let shared = world.clone();
    let map = lua.create_function(move |lua, id: i64| {
        crate::lua::transactions::require(lua)?;
        let world = shared.borrow();
        let map = world
            .btech
            .maps()
            .get(&ObjectId(id))
            .ok_or_else(|| error::failure("btech.operation.failed", "saved map not found"))?;
        let result = lua.create_table()?;
        result.set("name", map.name.as_str())?;
        result.set("width", map.width)?;
        result.set("height", map.height)?;
        result.set("gravity", map.gravity)?;
        result.set("temperature", map.temperature)?;
        result.set("flags", map.flags)?;
        result.set("terrain_ready", map.terrain_ready())?;
        result.set(
            "cargo_transfer_point",
            detached(lua, &map.cargo_transfer_point())?,
        )?;
        result.set("wrapping", map.wrapping())?;
        result.set("linked_markers", detached(lua, map.linked_markers())?)?;
        result.set("building_exits", detached(lua, map.building_exits())?)?;
        result.set("light", map.light)?;
        result.set("visibility", map.visibility)?;
        result.set("maximum_visibility", map.maximum_visibility)?;
        result.set("sensor_flags", map.sensor_flags)?;
        Ok(result)
    })?;
    native.set("map", error::wrap(lua, map, "btech.operation.failed")?)?;
    let shared = world.clone();
    let hex = lua.create_function(move |lua, (id, x, y): (i64, i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let world = shared.borrow();
        let map = world
            .btech
            .maps()
            .get(&ObjectId(id))
            .ok_or_else(|| error::failure("btech.operation.failed", "saved map not found"))?;
        let hex = map
            .hex(x, y)
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &hex)
    })?;
    native.set("map_hex", error::wrap(lua, hex, "btech.operation.failed")?)?;
    for (name, change) in [
        ("map_add_ice", crate::BattleIceChange::Grow),
        ("map_remove_ice", crate::BattleIceChange::Melt),
    ] {
        let action =
            lua.create_function(move |lua, (actor, id, percentage): (i64, i64, i32)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    let report = crate::change_battle_map_ice_action(
                        &scripts,
                        &crate::lua::configuration(lua),
                        ObjectId(actor),
                        ObjectId(id),
                        percentage,
                        change,
                    )
                    .map_err(mlua::Error::external)?;
                    detached(lua, &report)
                })
            })?;
        native.set(name, error::wrap(lua, action, "btech.operation.failed")?)?;
    }
    let set_hex = lua.create_function(
        |lua, (actor, id, x, y, symbol, elevation): (i64, i64, i32, i32, String, i32)| {
            crate::lua::transactions::require(lua)?;
            let terrain = crate::btech::terrain_edit::terrain_argument(&symbol)
                .map_err(mlua::Error::external)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let report = crate::set_battle_map_hex_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(id),
                    crate::BattleHexCoordinate { x, y },
                    terrain,
                    elevation,
                )
                .map_err(mlua::Error::external)?;
                detached(lua, &report)
            })
        },
    )?;
    native.set(
        "map_set_hex",
        error::wrap(lua, set_hex, "btech.operation.failed")?,
    )?;
    let set_station_field = lua.create_function(
        move |lua, (actor, station, field, value): (i64, i64, String, String)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::set_gunner_field(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(station),
                    &field,
                    &value,
                )
                .map_err(mlua::Error::external)
            })
        },
    )?;
    native.set(
        "gunner_set_field",
        error::wrap(lua, set_station_field, "btech.operation.failed")?,
    )?;
    let view_station_fields = lua.create_function(
        move |lua, (actor, station, options): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::view_gunner_fields(
                    &scripts,
                    ObjectId(actor),
                    ObjectId(station),
                    options.as_deref().unwrap_or(""),
                )
                .map_err(mlua::Error::external)
            })
        },
    )?;
    native.set(
        "gunner_view_fields",
        error::wrap(lua, view_station_fields, "btech.operation.failed")?,
    )?;
    let set_team = lua.create_function(move |lua, (actor, unit, team): (i64, i64, i32)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::set_battle_team_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(actor),
                ObjectId(unit),
                team,
            )
            .map_err(mlua::Error::external)
        })
    })?;
    native.set(
        "unit_set_team",
        error::wrap(lua, set_team, "btech.operation.failed")?,
    )?;
    let losemit = lua.create_function(move |lua, (actor, unit, message): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::battle_losemit_action(&scripts, ObjectId(actor), ObjectId(unit), &message)
                .map_err(mlua::Error::external)
        })
    })?;
    native.set(
        "unit_losemit",
        error::wrap(lua, losemit, "btech.operation.failed")?,
    )?;
    let damage =
        lua.create_function(
            move |lua,
                  (actor, unit, damage, clusters, rear, critical): (
                i64,
                i64,
                i32,
                i32,
                bool,
                bool,
            )| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    let report = crate::battle_damage_action(
                        &scripts,
                        &crate::lua::configuration(lua),
                        ObjectId(actor),
                        ObjectId(unit),
                        crate::BattleScenarioSalvo {
                            damage,
                            clusters,
                            rear,
                            critical,
                        },
                    )
                    .map_err(mlua::Error::external)?;
                    detached(lua, &report)
                })
            },
        )?;
    native.set(
        "unit_damage",
        error::wrap(lua, damage, "btech.operation.failed")?,
    )?;
    let weight = lua.create_function(move |lua, (actor, unit): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::battle_weight_action(&scripts, ObjectId(actor), ObjectId(unit))
            .map_err(mlua::Error::external)
    })?;
    native.set(
        "unit_weight",
        error::wrap(lua, weight, "btech.operation.failed")?,
    )?;
    let markings = lua.create_function(|lua, unit: i64| {
        let scripts = crate::Scripts::services(lua)?;
        crate::battle_unit_markings(&scripts.world(), ObjectId(unit))
            .map(str::to_owned)
            .map_err(mlua::Error::external)
    })?;
    native.set(
        "unit_markings",
        error::wrap(lua, markings, "btech.operation.failed")?,
    )?;
    let markings_set = lua.create_function(|lua, (actor, unit, value): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::set_battle_unit_markings(
                &mut scripts.world_mut(),
                ObjectId(actor),
                ObjectId(unit),
                &value,
            )
            .map(|()| true)
            .map_err(mlua::Error::external)
        })
    })?;
    native.set(
        "unit_set_markings",
        error::wrap(lua, markings_set, "btech.operation.failed")?,
    )?;
    let markings_view =
        lua.create_function(|lua, (unit, actor, target): (i64, i64, Option<i64>)| {
            let scripts = crate::Scripts::services(lua)?;
            crate::view_battle_unit_markings(
                &scripts.world(),
                ObjectId(unit),
                ObjectId(actor),
                target.map(ObjectId),
            )
            .map_err(mlua::Error::external)
        })?;
    native.set(
        "unit_view_markings",
        error::wrap(lua, markings_view, "btech.operation.failed")?,
    )?;
    let display_name = lua.create_function(|lua, unit: i64| {
        let scripts = crate::Scripts::services(lua)?;
        crate::battle_display_name(&scripts.world(), ObjectId(unit))
            .map(str::to_owned)
            .map_err(mlua::Error::external)
    })?;
    native.set(
        "unit_display_name",
        error::wrap(lua, display_name, "btech.operation.failed")?,
    )?;
    let display_name_set =
        lua.create_function(|lua, (actor, unit, value): (i64, i64, String)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::set_battle_unit_field_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(actor),
                ObjectId(unit),
                "displayname",
                &value,
            )
            .map(|()| true)
            .map_err(mlua::Error::external)
        })?;
    native.set(
        "unit_set_display_name",
        error::wrap(lua, display_name_set, "btech.operation.failed")?,
    )?;
    let preferred_id = lua.create_function(
        move |lua, (actor, unit, value): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::set_battle_preferred_id_action(
                    &scripts,
                    ObjectId(actor),
                    ObjectId(unit),
                    value.as_deref(),
                )
                .map_err(mlua::Error::external)
            })
        },
    )?;
    native.set(
        "unit_set_preferred_id",
        error::wrap(lua, preferred_id, "btech.operation.failed")?,
    )?;
    let setmapindex = lua.create_function(
        move |lua, (actor, unit, map, preferred): (i64, i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let report = crate::set_battle_map_index_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(unit),
                    ObjectId(map),
                    preferred.as_deref(),
                )
                .map_err(mlua::Error::external)?;
                detached(lua, &report)
            })
        },
    )?;
    native.set(
        "unit_setmapindex",
        error::wrap(lua, setmapindex, "btech.operation.failed")?,
    )?;
    let setxy = lua.create_function(
        move |lua, (actor, unit, x, y, elevation): (i64, i64, i32, i32, Option<i32>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let report = crate::set_battle_coordinates_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(unit),
                    crate::BattleScenarioPosition {
                        coordinate: crate::BattleHexCoordinate { x, y },
                        elevation,
                    },
                )
                .map_err(mlua::Error::external)?;
                detached(lua, &report)
            })
        },
    )?;
    native.set(
        "unit_setxy",
        error::wrap(lua, setxy, "btech.operation.failed")?,
    )?;
    let ood = lua.create_function(
        move |lua, (actor, unit, x, y, elevation): (i64, i64, i32, i32, Option<i32>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let report = crate::initiate_battle_orbital_drop_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(unit),
                    crate::BattleScenarioPosition {
                        coordinate: crate::BattleHexCoordinate { x, y },
                        elevation,
                    },
                )
                .map_err(mlua::Error::external)?;
                detached(lua, &report)
            })
        },
    )?;
    native.set("unit_ood", error::wrap(lua, ood, "btech.operation.failed")?)?;
    let damage_section = lua.create_function(
        move |lua,
              (actor, unit, section, damage, rear, critical): (
            i64,
            i64,
            String,
            i32,
            bool,
            bool,
        )| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let report = crate::battle_damage_section_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(unit),
                    crate::BattleScenarioHit {
                        section: &section,
                        damage,
                        rear,
                        critical,
                    },
                )
                .map_err(mlua::Error::external)?;
                detached(lua, &report)
            })
        },
    )?;
    native.set(
        "unit_damage_section",
        error::wrap(lua, damage_section, "btech.operation.failed")?,
    )?;
    let stats_world = world.clone();
    let stats = lua.create_function(move |lua, actor: i64| {
        crate::lua::transactions::require(lua)?;
        let report = crate::battle_runtime_stats(
            &stats_world.borrow(),
            &crate::lua::configuration(lua),
            ObjectId(actor),
        )
        .map_err(mlua::Error::external)?;
        detached(lua, &report)
    })?;
    native.set(
        "runtime_stats",
        error::wrap(lua, stats, "btech.operation.failed")?,
    )?;
    let set_map_field = lua.create_function(
        move |lua, (actor, map, field, value): (i64, i64, String, String)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::set_battle_map_field_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(map),
                    &field,
                    &value,
                )
                .map_err(mlua::Error::external)
            })
        },
    )?;
    native.set(
        "map_set_field",
        error::wrap(lua, set_map_field, "btech.operation.failed")?,
    )?;
    let set_unit_field = lua.create_function(
        move |lua, (actor, unit, field, value): (i64, i64, String, String)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::set_battle_unit_field_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(unit),
                    &field,
                    &value,
                )
                .map_err(mlua::Error::external)
            })
        },
    )?;
    native.set(
        "unit_set_field",
        error::wrap(lua, set_unit_field, "btech.operation.failed")?,
    )?;
    let check_map = lua.create_function(move |lua, (actor, map): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let report = crate::check_battle_map_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(actor),
                ObjectId(map),
            )
            .map_err(mlua::Error::external)?;
            detached(lua, &report)
        })
    })?;
    native.set(
        "map_check",
        error::wrap(lua, check_map, "btech.operation.failed")?,
    )?;
    let view_fields = lua.create_function(
        move |lua, (actor, map, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let report = crate::view_battle_map_fields_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(map),
                    arguments.as_deref().unwrap_or(""),
                )
                .map_err(mlua::Error::external)?;
                detached(lua, &report)
            })
        },
    )?;
    native.set(
        "map_fields",
        error::wrap(lua, view_fields, "btech.operation.failed")?,
    )?;
    let view_unit_fields = lua.create_function(
        move |lua, (actor, unit, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let report = crate::view_battle_unit_fields_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(unit),
                    arguments.as_deref().unwrap_or(""),
                )
                .map_err(mlua::Error::external)?;
                detached(lua, &report)
            })
        },
    )?;
    native.set(
        "unit_fields",
        error::wrap(lua, view_unit_fields, "btech.operation.failed")?,
    )?;
    let xp_ranking = lua.create_function(move |lua, (actor, skill): (i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let report = crate::battle_xp_ranking_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(actor),
                &skill,
            )
            .map_err(mlua::Error::external)?;
            detached(lua, &report)
        })
    })?;
    native.set(
        "character_xptop",
        error::wrap(lua, xp_ranking, "btech.operation.failed")?,
    )?;
    let save_database = lua.create_function(move |lua, actor: i64| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::request_battle_database_save(&scripts, ObjectId(actor))
                .map_err(mlua::Error::external)?;
            Ok(true)
        })
    })?;
    native.set(
        "database_save",
        error::wrap(lua, save_database, "btech.operation.failed")?,
    )?;
    let forms = lua.create_function(move |lua, actor: i64| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let forms = crate::battle_part_forms(&scripts.world(), ObjectId(actor))
            .map_err(mlua::Error::external)?;
        detached(lua, &forms)
    })?;
    native.set(
        "inventory_forms",
        error::wrap(lua, forms, "btech.operation.failed")?,
    )?;
    let register_station = lua.create_function(
        |lua, (actor, station, parent, arcs): (i64, i64, i64, i32)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::register_gunner_station(
                    &mut scripts.world_mut(),
                    ObjectId(actor),
                    ObjectId(station),
                    ObjectId(parent),
                    arcs,
                )
                .map_err(mlua::Error::external)?;
                Ok(true)
            })
        },
    )?;
    native.set(
        "gunner_register",
        error::wrap(lua, register_station, "btech.operation.failed")?,
    )?;
    for (name, initialize) in [("gunner_initialize", true), ("gunner_deinitialize", false)] {
        let callback = lua.create_function(move |lua, (actor, station): (i64, i64)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::gunner_station_action(
                    &scripts,
                    ObjectId(station),
                    ObjectId(actor),
                    initialize,
                )
                .map_err(mlua::Error::external)?;
                Ok(true)
            })
        })?;
        native.set(name, error::wrap(lua, callback, "btech.operation.failed")?)?;
    }
    let station_lock =
        lua.create_function(|lua, (station, gunner, target): (i64, i64, Option<i64>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::gunner_context(&scripts.world(), ObjectId(station), ObjectId(gunner))
                    .map_err(mlua::Error::external)?;
                let notice = crate::select_battle_target(
                    &mut scripts.world_mut(),
                    ObjectId(station),
                    ObjectId(gunner),
                    target.map(ObjectId),
                )
                .map_err(mlua::Error::external)?;
                crate::btech::notify_unit(&scripts, notice).map_err(mlua::Error::external)?;
                Ok(true)
            })
        })?;
    native.set(
        "gunner_lock",
        error::wrap(lua, station_lock, "btech.operation.failed")?,
    )?;
    let station_state = lua.create_function(|lua, station: i64| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        detached(
            lua,
            &scripts
                .world()
                .btech
                .gunner_stations()
                .get(&ObjectId(station)),
        )
    })?;
    native.set(
        "gunner_state",
        error::wrap(lua, station_state, "btech.operation.failed")?,
    )?;
    let gunner_gunnery =
        lua.create_function(|lua, (station, gunner, weapon): (i64, i64, usize)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let world = scripts.world();
            let context = crate::gunner_context(&world, ObjectId(station), ObjectId(gunner))
                .map_err(mlua::Error::external)?;
            context
                .gunnery_target(
                    &world,
                    weapon,
                    crate::lua::configuration(lua).battletech.extended_gunnery != 0,
                )
                .map_err(mlua::Error::external)
        })?;
    native.set(
        "gunner_gunnery",
        error::wrap(lua, gunner_gunnery, "btech.operation.failed")?,
    )?;
    let gunner_aim = lua.create_function(
        |lua, (station, gunner, weapon, target): (i64, i64, usize, Value)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let world = scripts.world();
            let context = crate::gunner_context(&world, ObjectId(station), ObjectId(gunner))
                .map_err(mlua::Error::external)?;
            let config = crate::lua::configuration(lua);
            let report = context
                .aim(
                    &world,
                    weapon,
                    firing_target(lua, target)?,
                    config.battletech.extended_gunnery != 0,
                    crate::BattleAimRules::configured(&config.battletech),
                )
                .map_err(mlua::Error::external)?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "gunner_aim",
        error::wrap(lua, gunner_aim, "btech.operation.failed")?,
    )?;
    let gunner_artillery = lua.create_function(|lua, (station, gunner): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let world = scripts.world();
        let context = crate::gunner_context(&world, ObjectId(station), ObjectId(gunner))
            .map_err(mlua::Error::external)?;
        context
            .artillery_gunnery_target(&world)
            .map_err(mlua::Error::external)
    })?;
    native.set(
        "gunner_artillery_gunnery",
        error::wrap(lua, gunner_artillery, "btech.operation.failed")?,
    )?;
    let view_map = lua.create_function(move |lua, (actor, map, x, y): (i64, i64, i32, i32)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let report = crate::view_battle_map_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(actor),
                ObjectId(map),
                crate::BattleHexCoordinate { x, y },
            )
            .map_err(mlua::Error::external)?;
            detached(lua, &report)
        })
    })?;
    native.set(
        "map_view",
        error::wrap(lua, view_map, "btech.operation.failed")?,
    )?;
    let add_mine = lua.create_function(
        move |lua,
              (actor, map, x, y, kind, strength, extra): (
            i64,
            i64,
            i32,
            i32,
            String,
            i32,
            Option<i32>,
        )| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::add_battle_mine_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(map),
                    crate::BattleMinePlacement {
                        coordinate: crate::BattleHexCoordinate { x, y },
                        kind: crate::BattleMineKind::parse(&kind).map_err(mlua::Error::external)?,
                        strength,
                        extra: extra.unwrap_or(0),
                    },
                )
                .map_err(mlua::Error::external)
            })
        },
    )?;
    native.set(
        "map_add_mine",
        error::wrap(lua, add_mine, "btech.operation.failed")?,
    )?;
    for (name, kind) in [
        ("map_add_fire", crate::BattleDecorationKind::Fire),
        ("map_add_smoke", crate::BattleDecorationKind::Smoke),
    ] {
        let action = lua.create_function(
            move |lua, (actor, id, x, y, duration): (i64, i64, i32, i32, i32)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    crate::add_battle_map_decoration_action(
                        &scripts,
                        &crate::lua::configuration(lua),
                        ObjectId(actor),
                        ObjectId(id),
                        crate::BattleHexCoordinate { x, y },
                        kind,
                        duration,
                    )
                    .map_err(mlua::Error::external)?;
                    Ok(true)
                })
            },
        )?;
        native.set(name, error::wrap(lua, action, "btech.operation.failed")?)?;
    }
    let shared = world.clone();
    let link = lua.create_function(move |lua, child: i64| {
        crate::lua::transactions::require(lua)?;
        let world = shared.borrow();
        let map = world
            .btech
            .maps()
            .get(&ObjectId(child))
            .ok_or_else(|| mlua::Error::external("Map not found"))?;
        detached(lua, &map.authored_link())
    })?;
    native.set(
        "map_link",
        error::wrap(lua, link, "btech.operation.failed")?,
    )?;
    let set_link = lua.create_function(|lua, (child, value): (i64, Value)| {
        crate::lua::transactions::require(lua)?;
        let link: Option<crate::BattleMapLink> = lua.from_value(value)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::set_battle_map_link(&mut scripts.world_mut(), ObjectId(child), link)
                .map_err(mlua::Error::external)?;
            Ok(true)
        })
    })?;
    native.set(
        "map_set_link",
        error::wrap(lua, set_link, "btech.operation.failed")?,
    )?;
    let update_links = lua.create_function(|lua, (actor, map): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let report = crate::update_battle_map_links_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(actor),
                ObjectId(map),
            )
            .map_err(mlua::Error::external)?;
            detached(lua, &report)
        })
    })?;
    native.set(
        "map_update_links",
        error::wrap(lua, update_links, "btech.operation.failed")?,
    )?;
    let list_map = lua.create_function(|lua, (actor, id, target): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let objects =
            crate::btech::map_list::parse_target(&target).map_err(mlua::Error::external)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::list_battle_map_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(actor),
                ObjectId(id),
                objects,
            )
            .map_err(mlua::Error::external)?;
            Ok(true)
        })
    })?;
    native.set(
        "map_list",
        error::wrap(lua, list_map, "btech.operation.failed")?,
    )?;
    let delete_objects = lua.create_function(
        |lua, (actor, id, kind, x, y): (i64, i64, Option<String>, Option<i32>, Option<i32>)| {
            crate::lua::transactions::require(lua)?;
            let kind = kind
                .as_deref()
                .map(crate::BattleMapObjectKind::parse)
                .transpose()
                .map_err(mlua::Error::external)?;
            let coordinate = match (x, y) {
                (Some(x), Some(y)) => Some(crate::BattleHexCoordinate { x, y }),
                (None, None) => None,
                _ => return Err(mlua::Error::external("Both X and Y are required")),
            };
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::delete_battle_map_objects_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(id),
                    kind,
                    coordinate,
                )
                .map_err(mlua::Error::external)
            })
        },
    )?;
    native.set(
        "map_delete_objects",
        error::wrap(lua, delete_objects, "btech.operation.failed")?,
    )?;
    let add_block = lua.create_function(
        |lua, (actor, id, x, y, radius, team): (i64, i64, i32, i32, i32, Option<i32>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::add_battle_landing_exclusion_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(id),
                    crate::BattleHexCoordinate { x, y },
                    radius,
                    team.unwrap_or(0),
                )
                .map_err(mlua::Error::external)
            })
        },
    )?;
    native.set(
        "map_add_block",
        error::wrap(lua, add_block, "btech.operation.failed")?,
    )?;
    let load_map = lua.create_function(|lua, (actor, id, name): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        // A caught load rejection retains its diagnostic, while an aborted outer
        // callback rolls back the complete transaction, including channel output.
        let outcome = crate::lua::transactions::run(lua, &scripts.world, || {
            Ok(crate::load_battle_map_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(actor),
                ObjectId(id),
                &name,
            ))
        })?;
        outcome.map_err(mlua::Error::external)?;
        Ok(true)
    })?;
    native.set(
        "map_load",
        error::wrap(lua, load_map, "btech.operation.failed")?,
    )?;
    let save_map = lua.create_function(|lua, (actor, id, name): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::save_battle_map_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(actor),
                ObjectId(id),
                &name,
            )
            .map_err(mlua::Error::external)?;
            Ok(true)
        })
    })?;
    native.set(
        "map_save",
        error::wrap(lua, save_map, "btech.operation.failed")?,
    )?;
    let resize = lua.create_function(|lua, (actor, id, width, height): (i64, i64, i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::resize_battle_map_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(actor),
                ObjectId(id),
                width,
                height,
            )
            .map_err(mlua::Error::external)?;
            Ok(true)
        })
    })?;
    native.set(
        "map_resize",
        error::wrap(lua, resize, "btech.operation.failed")?,
    )?;
    let clear_units = lua.create_function(|lua, (actor, id): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let units = crate::clear_battle_map_units_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(actor),
                ObjectId(id),
            )
            .map_err(mlua::Error::external)?;
            detached(lua, &units)
        })
    })?;
    native.set(
        "map_clear_units",
        error::wrap(lua, clear_units, "btech.operation.failed")?,
    )?;
    let emit = lua.create_function(|lua, (actor, id, text): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let units =
                crate::emit_battle_map_action(&scripts, ObjectId(actor), ObjectId(id), &text)
                    .map_err(mlua::Error::external)?;
            detached(lua, &units)
        })
    })?;
    native.set(
        "map_emit",
        error::wrap(lua, emit, "btech.operation.failed")?,
    )?;
    let environment = lua.create_function(|lua, (actor, id, value): (i64, i64, Value)| {
        crate::lua::transactions::require(lua)?;
        let conditions: crate::BattleMapEnvironment = lua.from_value(value)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let actual = crate::set_battle_map_environment_action(
                &scripts,
                ObjectId(actor),
                ObjectId(id),
                conditions,
            )
            .map_err(mlua::Error::external)?;
            detached(lua, &actual)
        })
    })?;
    native.set(
        "map_environment",
        error::wrap(lua, environment, "btech.operation.failed")?,
    )?;
    let conditions = lua.create_function(|lua, (id, light, visibility): (i64, String, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notices = (|| -> anyhow::Result<_> {
                crate::set_battle_map_visibility(
                    &mut scripts.world_mut(),
                    ObjectId(id),
                    light.parse()?,
                    u8::try_from(visibility)?,
                )
            })()
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            for notice in notices {
                crate::btech::notify_unit(&scripts, notice).map_err(mlua::Error::external)?;
            }
            Ok(true)
        })
    })?;
    native.set(
        "map_conditions",
        error::wrap(lua, conditions, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let clouds = lua.create_function(move |lua, (actor, id, altitude): (i64, i64, i16)| {
        crate::lua::transactions::require(lua)?;
        crate::set_battle_map_cloud_base(
            &mut shared.borrow_mut(),
            ObjectId(actor),
            ObjectId(id),
            altitude,
        )
        .map_err(|e| error::failure("btech.operation.failed", e))?;
        Ok(altitude)
    })?;
    native.set(
        "map_cloud",
        error::wrap(lua, clouds, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let wrapping = lua.create_function(move |lua, (id, enabled): (i64, mlua::Value)| {
        crate::lua::transactions::require(lua)?;
        let mlua::Value::Boolean(enabled) = enabled else {
            return Err(error::failure(
                "btech.operation.failed",
                "Wrapping requires a boolean",
            ));
        };
        crate::set_battle_map_wrapping(&mut shared.borrow_mut(), ObjectId(id), enabled)
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        Ok(true)
    })?;
    native.set(
        "map_wrapping",
        error::wrap(lua, wrapping, "btech.operation.failed")?,
    )?;
    for (name, create) in [("map_create", true), ("map_reload", false)] {
        let operation = lua.create_function(move |lua, (id, name): (i64, String)| {
            crate::lua::transactions::require(lua)?;
            let config = crate::lua::configuration(lua);
            let scripts = crate::Scripts::services(lua)?;
            crate::btech::map_load::initialize_map_action(
                &scripts,
                &config,
                ObjectId(id),
                &name,
                create,
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            Ok(true)
        })?;
        native.set(name, error::wrap(lua, operation, "btech.operation.failed")?)?;
    }
    let shared = world.clone();
    let create = lua.create_function(move |lua, (id, name): (i64, String)| {
        crate::lua::transactions::require(lua)?;
        let config = crate::lua::configuration(lua);
        let definition =
            crate::btech::read_unit_template(&config.path(&config.database.mech_database), &name)
                .map_err(|e| error::failure("btech.template.invalid", format!("{e:#}")))?;
        definition
            .create(&mut shared.borrow_mut(), ObjectId(id))
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        Ok(true)
    })?;
    native.set(
        "unit_create",
        error::wrap(lua, create, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let visibility = lua.create_function(move |lua, (id, value): (i64, Option<mlua::Table>)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        if let Some(value) = value {
            let flags: crate::BattleVisibility = lua.from_value(mlua::Value::Table(value))?;
            crate::set_battle_visibility(&mut scripts.world_mut(), ObjectId(id), flags)
                .map_err(mlua::Error::external)?;
        }
        let flags = crate::battle_visibility(&scripts.world(), ObjectId(id))
            .map_err(mlua::Error::external)?;
        lua.to_value(&flags)
    })?;
    native.set(
        "unit_visibility",
        error::wrap(lua, visibility, "btech.operation.failed")?,
    )?;
    let safe = lua.create_function(move |lua, (id, enabled): (i64, Option<bool>)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        if let Some(enabled) = enabled {
            crate::set_battle_combat_safe(&mut scripts.world_mut(), ObjectId(id), enabled)
                .map_err(mlua::Error::external)?;
        }
        crate::battle_combat_safe(&scripts.world(), ObjectId(id)).map_err(mlua::Error::external)
    })?;
    native.set(
        "unit_combat_safe",
        error::wrap(lua, safe, "btech.operation.failed")?,
    )?;
    let hold = lua.create_function(move |lua, (id, enabled): (i64, Option<bool>)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        if let Some(enabled) = enabled {
            crate::set_battle_weapons_hold(&mut scripts.world_mut(), ObjectId(id), enabled)
                .map_err(mlua::Error::external)?;
        }
        crate::battle_weapons_hold(&scripts.world(), ObjectId(id)).map_err(mlua::Error::external)
    })?;
    native.set(
        "unit_weapons_hold",
        error::wrap(lua, hold, "btech.operation.failed")?,
    )?;
    let observer = lua.create_function(move |lua, (id, enabled): (i64, Option<bool>)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        if let Some(enabled) = enabled {
            crate::set_battle_observer(&mut scripts.world_mut(), ObjectId(id), enabled)
                .map_err(mlua::Error::external)?;
        }
        crate::battle_unit_observer(&scripts.world(), ObjectId(id)).map_err(mlua::Error::external)
    })?;
    native.set(
        "unit_observer",
        error::wrap(lua, observer, "btech.operation.failed")?,
    )?;
    let fortified = lua.create_function(move |lua, (id, enabled): (i64, Option<bool>)| {
        crate::lua::transactions::require(lua)?;
        if let Some(enabled) = enabled {
            crate::set_battle_fortified(
                &mut crate::Scripts::services(lua)?.world_mut(),
                ObjectId(id),
                enabled,
            )
            .map_err(mlua::Error::external)?;
        }
        crate::battle_unit_fortified(&crate::Scripts::services(lua)?.world(), ObjectId(id))
            .map_err(mlua::Error::external)
    })?;
    native.set(
        "unit_fortified",
        error::wrap(lua, fortified, "btech.operation.failed")?,
    )?;
    let towable = lua.create_function(move |lua, (id, enabled): (i64, Option<bool>)| {
        crate::lua::transactions::require(lua)?;
        if let Some(enabled) = enabled {
            crate::set_battle_towable(&mut shared.borrow_mut(), ObjectId(id), enabled)
                .map_err(mlua::Error::external)?;
        }
        crate::battle_unit_towable(&shared.borrow(), ObjectId(id)).map_err(mlua::Error::external)
    })?;
    native.set(
        "unit_towable",
        error::wrap(lua, towable, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let state = lua.create_function(move |lua, id: i64| {
        crate::lua::transactions::require(lua)?;
        let world = shared.borrow();
        // Shared relationship metadata is projected once for every supported chassis.
        let state = lua.create_table()?;
        state.set(
            "towable",
            crate::battle_unit_towable(&world, ObjectId(id)).map_err(mlua::Error::external)?,
        )?;
        state.set(
            "towing",
            world.btech.tows().get(&ObjectId(id)).map(|id| id.0),
        )?;
        state.set(
            "towed_by",
            world.btech.towed_by(ObjectId(id)).map(|id| id.0),
        )?;
        let fall = world
            .btech
            .vehicles()
            .get(&ObjectId(id))
            .and_then(|unit| unit.free_fall())
            .or_else(|| {
                world
                    .btech
                    .constructed_units()
                    .get(&ObjectId(id))
                    .and_then(|unit| unit.free_fall())
            });
        state.set(
            "fortified",
            crate::battle_unit_fortified(&world, ObjectId(id)).map_err(mlua::Error::external)?,
        )?;
        state.set(
            "altitude",
            crate::battle_unit_altitude(&world, ObjectId(id)).map_err(mlua::Error::external)?,
        )?;
        state.set("free_fall", detached(lua, &fall)?)?;
        let orbital_drop = world
            .btech
            .constructed_units()
            .get(&ObjectId(id))
            .and_then(|unit| unit.orbital_drop())
            .or_else(|| {
                world
                    .btech
                    .vehicles()
                    .get(&ObjectId(id))
                    .and_then(|unit| unit.orbital_drop())
            });
        state.set("orbital_drop", detached(lua, &orbital_drop)?)?;
        if let Some(unit) = world.btech.constructed_units().get(&ObjectId(id)) {
            state.set("hull_down", detached(lua, &unit.hull_down())?)?;
        }
        state.set(
            "visibility",
            detached(
                lua,
                &crate::battle_visibility(&world, ObjectId(id)).map_err(mlua::Error::external)?,
            )?,
        )?;
        state.set(
            "weapons_hold",
            crate::battle_weapons_hold(&world, ObjectId(id)).map_err(mlua::Error::external)?,
        )?;
        state.set(
            "combat_safe",
            crate::battle_combat_safe(&world, ObjectId(id)).map_err(mlua::Error::external)?,
        )?;
        state.set(
            "observer",
            crate::battle_unit_observer(&world, ObjectId(id)).map_err(mlua::Error::external)?,
        )?;
        if let Some(vehicle) = world.btech.vehicles().get(&ObjectId(id)) {
            state.set("kind", "vehicle")?;
            if vehicle.vtol_fuel().is_some() {
                let fuel = crate::battle_vtol_fuel_status(&world, ObjectId(id))
                    .map_err(mlua::Error::external)?;
                state.set("fuel", detached(lua, &fuel)?)?;
            }
            state.set("sensor_signal", vehicle.sensor_signal())?;
            state.set("fired_recently", vehicle.fired_recently())?;
            state.set("radio", detached(lua, &vehicle.radio_channels())?)?;
            state.set(
                "radio_capabilities",
                detached(lua, &vehicle.radio_capabilities())?,
            )?;
            state.set("radio_skill", vehicle.radio_skill())?;
            state.set(
                "radio_experience_remaining",
                vehicle.radio_experience_remaining(),
            )?;
            state.set("dig", detached(lua, &vehicle.dig_state())?)?;
            state.set(
                "mass",
                detached(lua, &vehicle.mass().map_err(mlua::Error::external)?)?,
            )?;
            state.set("under_bridge", vehicle.under_bridge())?;
            state.set("turret_heading", vehicle.turret_heading())?;
            state.set("turret_locked", vehicle.turret_locked())?;
            state.set("turret_jammed", vehicle.turret_jammed())?;
            state.set("automatic_turret", vehicle.automatic_turret())?;
            state.set("turret_repairs", detached(lua, vehicle.turret_repairs())?)?;
            state.set(
                "maximum_speed",
                crate::btech::motion_controls::throttle_configured(
                    &world,
                    ObjectId(id),
                    crate::btech::SpeedPolicy::configured(&crate::lua::configuration(lua)),
                )
                .map_err(mlua::Error::external)?,
            )?;
            state.set("motive_speed_loss", vehicle.motive_speed_loss())?;
            state.set("immobilized", vehicle.immobilized())?;
            state.set(
                "elevation",
                crate::battle_unit_elevation(&world, ObjectId(id))
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?,
            )?;
            state.set("motion", detached(lua, &vehicle.motion())?)?;
            state.set("pilot", detached(lua, &vehicle.pilot())?)?;
            state.set("power", detached(lua, &vehicle.power())?)?;
            state.set("simulation_supported", false)?;
            state.set("definition", detached(lua, vehicle.definition())?)?;
            state.set("sections", detached(lua, vehicle.sections())?)?;
            state.set("ammunition", detached(lua, vehicle.ammunition())?)?;
            state.set("fire_modes", detached(lua, vehicle.fire_modes())?)?;
            state.set(
                "sensor_signature",
                detached(lua, &vehicle.sensor_signature())?,
            )?;
            state.set("hide_elapsed", vehicle.hide_elapsed())?;
            state.set("scanner_perception", vehicle.scanner_perception())?;
            state.set("sensor_ranges", detached(lua, &vehicle.sensor_ranges())?)?;
            state.set("friendly_fire_safety", vehicle.friendly_fire_safety())?;
            state.set("auto_fall", vehicle.auto_fall())?;
            state.set(
                "character_pilot",
                detached(lua, &vehicle.character_pilot_status())?,
            )?;
            state.set("ams_enabled", vehicle.ams_enabled())?;
            state.set(
                "artemis",
                detached(
                    lua,
                    &vehicle
                        .artemis_controllers()
                        .map_err(|e| error::failure("btech.operation.failed", e))?,
                )?,
            )?;
            state.set("beacons", detached(lua, vehicle.beacons())?)?;
            state.set("pod_removal", detached(lua, &vehicle.pod_removal())?)?;
            state.set("target_lock", detached(lua, &vehicle.target_selection())?)?;
            state.set("artillery_adjustment", vehicle.artillery_adjustment())?;
            state.set("spotter", vehicle.spotter().map(|id| id.0))?;
            state.set("spotter_events", detached(lua, vehicle.spotter_events())?)?;
            state.set("electronics", detached(lua, &vehicle.electronics())?)?;
            state.set("flooded", vehicle.flooded())?;
            state.set("crew_killed", vehicle.crew_killed())?;
            state.set(
                "c3_hardware",
                detached(lua, &vehicle.c3_hardware().map_err(mlua::Error::external)?)?,
            )?;
            state.set(
                "c3i_members",
                detached(
                    lua,
                    &crate::battle_c3i_members(&world, ObjectId(id))
                        .map_err(mlua::Error::external)?,
                )?,
            )?;
            state.set(
                "c3_members",
                detached(
                    lua,
                    &crate::battle_c3_members(&world, ObjectId(id))
                        .map_err(mlua::Error::external)?,
                )?,
            )?;
            state.set("brief", detached(lua, &vehicle.brief_settings())?)?;
            state.set(
                "sensor_selection",
                detached(lua, &vehicle.sensor_selection())?,
            )?;
            state.set("weapon_recycle", detached(lua, vehicle.weapon_recycle())?)?;
            state.set(
                "component_failures",
                detached(lua, vehicle.component_failures())?,
            )?;
            state.set("weapon_failures", detached(lua, vehicle.weapon_failures())?)?;
            state.set("unjam", detached(lua, &vehicle.unjam())?)?;
            state.set("spent_launchers", detached(lua, vehicle.spent_launchers())?)?;
            state.set("lost_criticals", detached(lua, vehicle.lost_criticals())?)?;
            state.set("piloting_damage", vehicle.piloting_damage())?;
            state.set("pilot_injuries", vehicle.pilot_injuries())?;
            state.set("crew_recovery_remaining", vehicle.crew_recovery().remaining)?;
            state.set("blinded_remaining", vehicle.blinded_remaining())?;
            state.set("self_destruct", detached(lua, &vehicle.self_destruct())?)?;
            state.set("self_destruct_safe", vehicle.self_destruct_safe())?;
            state.set("weapon_heat", vehicle.weapon_heat())?;
            state.set("inferno_remaining", vehicle.inferno_remaining())?;
            state.set(
                "burning_sections",
                detached(lua, vehicle.burning_sections())?,
            )?;
            state.set("extinguishing", vehicle.extinguishing())?;
            state.set("crew_stun_remaining", vehicle.crew_stun_remaining())?;
            state.set("crew_stunned", vehicle.crew_stunned())?;
            state.set("gunnery_damage", vehicle.gunnery_damage())?;
            state.set(
                "lost_stabilizers",
                detached(lua, vehicle.lost_stabilizers())?,
            )?;
            state.set(
                "ammunition_modes",
                detached(lua, vehicle.ammunition_modes())?,
            )?;
            state.set(
                "preferred_id",
                crate::battle_preferred_id(&world, ObjectId(id)).map_err(mlua::Error::external)?,
            )?;
            state.set("searchlight", detached(lua, &vehicle.searchlight())?)?;
            state.set("tag", detached(lua, &vehicle.tag())?)?;
            state.set("mw_safety", vehicle.mw_safety())?;
            state.set("bth_debug", vehicle.bth_debug())?;
            state.set("tight_turn_mode", vehicle.tight_turn_mode())?;
            state.set("armor_warning", vehicle.armor_warning())?;
            state.set("ammunition_warning", vehicle.ammunition_warning())?;
            state.set("searchlight_warning", vehicle.searchlight_warning())?;
            state.set("autocon_shutdown", vehicle.autocon_shutdown())?;
            state.set("last_startup", vehicle.last_startup())?;
            state.set("position", detached(lua, &vehicle.position())?)?;
            state.set("map_slot", detached(lua, &vehicle.map_slot())?)?;
            state.set("destroyed", vehicle.is_destroyed())?;
            return Ok(state);
        }
        let unit = world
            .btech
            .constructed_units()
            .get(&ObjectId(id))
            .ok_or_else(|| {
                error::failure(
                    "btech.operation.failed",
                    "Unit construction state is unavailable",
                )
            })?;
        // Explicit public projection keeps private simulation state out of script results.
        state.set("kind", "mech")?;
        state.set(
            "elevation",
            crate::battle_unit_elevation(&world, ObjectId(id))
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?,
        )?;
        state.set("hex_sync_pending", unit.hex_sync_pending())?;
        state.set("beacons", detached(lua, unit.beacons())?)?;
        state.set("narc_sections", detached(lua, &unit.narc_sections())?)?;
        state.set("ams_enabled", unit.ams_enabled())?;
        state.set("null_signature", detached(lua, &unit.null_signature())?)?;
        state.set("fired_recently", unit.fired_recently())?;
        state.set("sensor_signal", unit.sensor_signal())?;
        state.set("stealth", detached(lua, &unit.stealth())?)?;
        state.set("electronics", detached(lua, &unit.electronics())?)?;
        state.set("auto_fall", unit.auto_fall())?;
        state.set("triple_myomer_active", unit.triple_myomer_active())?;
        state.set(
            "movement_maximum_speed",
            crate::btech::motion_controls::throttle_configured(
                &world,
                ObjectId(id),
                crate::btech::SpeedPolicy::configured(&crate::lua::configuration(lua)),
            )
            .map_err(mlua::Error::external)?,
        )?;
        state.set("limb_recycle", detached(lua, unit.limb_recycle())?)?;
        state.set("carried_club", detached(lua, &unit.carried_club())?)?;
        state.set("charge", detached(lua, &unit.charge())?)?;
        state.set("friendly_fire_safety", unit.friendly_fire_safety())?;
        state.set("mw_safety", unit.mw_safety())?;
        state.set("bth_debug", unit.bth_debug())?;
        state.set("armor_warning", unit.armor_warning())?;
        state.set("tight_turn_mode", unit.tight_turn_mode())?;
        state.set("lateral", detached(lua, &unit.lateral())?)?;
        state.set("autocon_shutdown", unit.autocon_shutdown())?;
        state.set("ammunition_warning", unit.ammunition_warning())?;
        state.set("searchlight_warning", unit.searchlight_warning())?;
        state.set(
            "mass",
            detached(lua, &unit.mass().map_err(mlua::Error::external)?)?,
        )?;
        state.set(
            "engine",
            detached(lua, &unit.engine().map_err(mlua::Error::external)?)?,
        )?;
        state.set("destroyed", unit.is_destroyed())?;
        state.set("facing", detached(lua, &unit.facing())?)?;
        state.set("stagger", detached(lua, unit.stagger())?)?;
        state.set("stand_timer", detached(lua, &unit.stand_timer())?)?;
        state.set("flooded_sections", detached(lua, unit.flooded_sections())?)?;
        state.set("posture", detached(lua, &unit.posture())?)?;
        state.set("map_slot", unit.map_slot())?;
        state.set("target_lock", detached(lua, &unit.target_selection())?)?;
        state.set("spotter", unit.spotter().map(|id| id.0))?;
        state.set("spotter_events", detached(lua, unit.spotter_events())?)?;
        state.set("artillery_adjustment", unit.artillery_adjustment())?;
        state.set("tag", detached(lua, &unit.tag())?)?;
        state.set("sensor_selection", detached(lua, &unit.sensor_selection())?)?;
        state.set(
            "radio_experience_remaining",
            unit.radio_experience_remaining(),
        )?;
        state.set("sensor_ranges", detached(lua, &unit.sensor_ranges())?)?;
        state.set(
            "preferred_id",
            crate::battle_preferred_id(&world, ObjectId(id)).map_err(mlua::Error::external)?,
        )?;
        state.set("last_startup", unit.last_startup())?;
        state.set("battlefield_id", unit.battlefield_id())?;
        state.set("radio_skill", unit.radio_skill())?;
        state.set("radio", detached(lua, &unit.radio_channels())?)?;
        state.set(
            "radio_capabilities",
            detached(lua, &unit.radio_capabilities())?,
        )?;
        state.set("searchlight", detached(lua, &unit.searchlight())?)?;
        state.set("sensor_signature", detached(lua, &unit.sensor_signature())?)?;
        state.set("scanner_perception", unit.scanner_perception())?;
        state.set("stun_remaining", unit.stun_remaining())?;
        state.set("pilot_injuries", unit.pilot_injuries())?;
        state.set("crew_recovery_remaining", unit.crew_recovery().remaining)?;
        state.set("blinded_remaining", unit.blinded_remaining())?;
        state.set(
            "reactor_instability_remaining",
            unit.reactor_instability_remaining(),
        )?;
        state.set("self_destruct", detached(lua, &unit.self_destruct())?)?;
        state.set("self_destruct_safe", unit.self_destruct_safe())?;
        state.set(
            "character_pilot",
            detached(lua, &unit.character_pilot_status())?,
        )?;
        state.set("heat", detached(lua, &unit.heat())?)?;
        state.set("heat_cutoff", detached(lua, &unit.heat_cutoff())?)?;
        state.set("hide_elapsed", unit.hide_elapsed())?;
        state.set("inferno_remaining", unit.inferno_remaining())?;
        state.set("overheat_clock", detached(lua, &unit.overheat_clock())?)?;
        state.set("weapon_recycle", detached(lua, unit.weapon_recycle())?)?;
        state.set(
            "component_failures",
            detached(lua, unit.component_failures())?,
        )?;
        state.set("weapon_failures", detached(lua, unit.weapon_failures())?)?;
        state.set("gyro", detached(lua, &unit.gyro())?)?;
        state.set("gyro_damage", unit.gyro_damage())?;
        state.set(
            "masc_installed",
            unit.masc_installed()
                .map_err(|e| error::failure("btech.operation.failed", e))?,
        )?;
        state.set(
            "masc_operational",
            unit.masc_operational()
                .map_err(|e| error::failure("btech.operation.failed", e))?,
        )?;
        state.set(
            "c3_hardware",
            detached(
                lua,
                &unit
                    .c3_hardware()
                    .map_err(|e| error::failure("btech.operation.failed", e))?,
            )?,
        )?;
        state.set(
            "c3i_members",
            detached(
                lua,
                &crate::battle_c3i_members(&world, ObjectId(id))
                    .map_err(|e| error::failure("btech.operation.failed", e))?,
            )?,
        )?;
        state.set(
            "c3_members",
            detached(
                lua,
                &crate::battle_c3_members(&world, ObjectId(id))
                    .map_err(|e| error::failure("btech.operation.failed", e))?,
            )?,
        )?;
        state.set(
            "c3_operational",
            unit.c3_operational()
                .map_err(|e| error::failure("btech.operation.failed", e))?,
        )?;
        state.set("masc", detached(lua, &unit.masc())?)?;
        state.set("supercharger", detached(lua, &unit.supercharger())?)?;
        state.set("supercharger_installed", unit.supercharger_installed())?;
        state.set("supercharger_operational", unit.supercharger_operational())?;
        state.set("unjam", detached(lua, &unit.unjam())?)?;
        state.set("dumping", detached(lua, &unit.dumping())?)?;
        state.set("weapon_damage", detached(lua, &unit.weapon_damage())?)?;
        state.set(
            "artemis",
            detached(
                lua,
                &unit
                    .artemis_controllers()
                    .map_err(|e| error::failure("btech.operation.failed", e))?,
            )?,
        )?;
        state.set("mobility", detached(lua, &unit.mobility())?)?;
        state.set("flight", detached(lua, &unit.flight())?)?;
        state.set(
            "airborne",
            detached(lua, &unit.flight().map(|flight| flight.sample()))?,
        )?;
        state.set("jump_stabilization", unit.jump_stabilization())?;
        let gravity = unit
            .position()
            .and_then(|position| world.btech.maps().get(&position.map))
            .map_or(100, |map| map.gravity);
        state.set(
            "jump_capacity",
            detached(
                lua,
                &unit
                    .jump_capacity(gravity)
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?,
            )?,
        )?;
        state.set("lost_criticals", detached(lua, unit.lost_criticals())?)?;
        state.set("definition", detached(lua, unit.definition())?)?;
        state.set("sections", detached(lua, unit.sections())?)?;
        state.set("ammunition", detached(lua, unit.ammunition())?)?;
        state.set("power", detached(lua, &unit.power())?)?;
        state.set("pilot", detached(lua, &unit.pilot())?)?;
        state.set("position", detached(lua, &unit.position())?)?;
        state.set("motion", detached(lua, &unit.motion())?)?;
        Ok(state)
    })?;
    native.set(
        "unit_state",
        error::wrap(lua, state, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let place = lua.create_function(move |lua, (id, map, x, y): (i64, i64, i64, i64)| {
        crate::lua::transactions::require(lua)?;
        crate::place_battle_unit(&mut shared.borrow_mut(), ObjectId(id), ObjectId(map), x, y)
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        Ok(true)
    })?;
    native.set(
        "unit_place",
        error::wrap(lua, place, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let remove = lua.create_function(move |lua, (id, destination): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        crate::remove_battle_unit(
            &mut shared.borrow_mut(),
            ObjectId(id),
            ObjectId(destination),
        )
        .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        Ok(true)
    })?;
    native.set(
        "unit_remove",
        error::wrap(lua, remove, "btech.operation.failed")?,
    )?;
    for (name, assign) in [("unit_pilot", true), ("unit_release", false)] {
        let shared = world.clone();
        let operation = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
            crate::lua::transactions::require(lua)?;
            let operation = if assign {
                crate::assign_battle_pilot
            } else {
                crate::release_battle_pilot
            };
            operation(&mut shared.borrow_mut(), ObjectId(unit), ObjectId(pilot))
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        })?;
        native.set(name, error::wrap(lua, operation, "btech.operation.failed")?)?;
    }
    for (name, start) in [("unit_start", true), ("unit_stop", false)] {
        let operation =
            lua.create_function(move |lua, (unit, pilot, fast): (i64, i64, Option<bool>)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    if !start {
                        crate::stop_battle_unit_action(
                            &scripts,
                            &crate::lua::configuration(lua),
                            ObjectId(unit),
                            ObjectId(pilot),
                        )
                        .map_err(mlua::Error::external)?;
                        return Ok(true);
                    }
                    let notices = crate::start_battle_unit(
                        &mut scripts.world.borrow_mut(),
                        ObjectId(unit),
                        ObjectId(pilot),
                        fast.unwrap_or(false),
                    )
                    .map(|notice| vec![notice])
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                    for notice in notices {
                        crate::btech::notify_unit(&scripts, notice)
                            .map_err(|e| error::failure("btech.operation.failed", e))?;
                    }
                    Ok(true)
                })
            })?;
        native.set(name, error::wrap(lua, operation, "btech.operation.failed")?)?;
    }
    let shared = world.clone();
    let range = lua.create_function(move |lua, (first, second): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let range = crate::battle_unit_range(&shared.borrow(), ObjectId(first), ObjectId(second))
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &range)
    })?;
    native.set(
        "unit_range",
        error::wrap(lua, range, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let status = lua.create_function(move |lua, (id, options): (i64, Option<String>)| {
        crate::btech::unit_status_configured(
            &shared.borrow(),
            ObjectId(id),
            options.as_deref().unwrap_or(""),
            crate::btech::status::StatusRules::configured(&crate::lua::configuration(lua)),
        )
        .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))
    })?;
    native.set(
        "unit_status",
        error::wrap(lua, status, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let station_status = lua.create_function(
        move |lua, (station, gunner, options): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let world = shared.borrow();
            crate::gunner_context(&world, ObjectId(station), ObjectId(gunner))
                .map_err(mlua::Error::external)?;
            crate::btech::status::for_operator(
                &world,
                ObjectId(station),
                ObjectId(gunner),
                options.as_deref().unwrap_or(""),
                crate::btech::status::StatusRules::configured(&crate::lua::configuration(lua)),
            )
            .map_err(mlua::Error::external)
        },
    )?;
    native.set(
        "gunner_status",
        error::wrap(lua, station_status, "btech.operation.failed")?,
    )?;
    let station_contacts = lua.create_function(
        move |lua, (station, gunner, options): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::gunner_context(&scripts.world(), ObjectId(station), ObjectId(gunner))
                .map_err(mlua::Error::external)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::btech::contact_report::report(
                    &scripts,
                    ObjectId(station),
                    ObjectId(gunner),
                    options.as_deref().unwrap_or(""),
                )
                .map_err(mlua::Error::external)
            })
        },
    )?;
    native.set(
        "gunner_contacts",
        error::wrap(lua, station_contacts, "btech.operation.failed")?,
    )?;
    let club = lua.create_function(|lua, (id, pilot, target): (i64, i64, Option<i64>)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let report = crate::btech::physical::configured_club(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(id),
                ObjectId(pilot),
                target.map(ObjectId),
            )
            .map_err(mlua::Error::external)?;
            detached(lua, &report)
        })
    })?;
    native.set(
        "unit_club",
        error::wrap(lua, club, "btech.operation.failed")?,
    )?;
    let grab = lua.create_function(|lua, (id, pilot, arm): (i64, i64, Option<String>)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notices = crate::grab_battle_club(
                &mut scripts.world.borrow_mut(),
                ObjectId(id),
                ObjectId(pilot),
                arm.as_deref(),
            )
            .map_err(mlua::Error::external)?;
            for notice in &notices {
                crate::btech::notify_unit(&scripts, notice.clone())
                    .map_err(mlua::Error::external)?;
            }
            detached(lua, &notices)
        })
    })?;
    native.set(
        "unit_grabclub",
        error::wrap(lua, grab, "btech.operation.failed")?,
    )?;
    let charge = lua.create_function(|lua, (id, pilot, target): (i64, i64, mlua::Value)| {
        crate::lua::transactions::require(lua)?;
        let selection = match target {
            mlua::Value::Nil => crate::BattleChargeSelection::Default,
            mlua::Value::Integer(id) => crate::BattleChargeSelection::Target(ObjectId(id)),
            mlua::Value::String(value) if value.to_str()? == "-" => {
                crate::BattleChargeSelection::Cancel
            }
            _ => return Err(mlua::Error::external("Expected a target dbref, '-' or nil")),
        };
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notices = crate::select_battle_charge(
                &mut scripts.world.borrow_mut(),
                ObjectId(id),
                ObjectId(pilot),
                selection,
            )
            .map_err(mlua::Error::external)?;
            for notice in &notices {
                crate::btech::notify_unit(&scripts, notice.clone())
                    .map_err(mlua::Error::external)?;
            }
            detached(lua, &notices)
        })
    })?;
    native.set(
        "unit_charge",
        error::wrap(lua, charge, "btech.operation.failed")?,
    )?;
    for (name, trip) in [("unit_kick", false), ("unit_trip", true)] {
        let callback = lua.create_function(
            move |lua, (id, pilot, leg, target): (i64, i64, Option<String>, Option<i64>)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    let config = crate::lua::configuration(lua);
                    let resolve = if trip {
                        crate::btech::physical::configured_trip
                    } else {
                        crate::btech::physical::configured_kick
                    };
                    let report = resolve(
                        &scripts,
                        &config,
                        ObjectId(id),
                        ObjectId(pilot),
                        leg.as_deref(),
                        target.map(ObjectId),
                    )
                    .map_err(mlua::Error::external)?;
                    detached(lua, &report)
                })
            },
        )?;
        native.set(name, error::wrap(lua, callback, "btech.operation.failed")?)?;
    }
    for (name, kind) in [
        ("unit_punch", crate::BattleArmAttack::Punch),
        ("unit_axe", crate::BattleArmAttack::Axe),
        ("unit_sword", crate::BattleArmAttack::Sword),
        ("unit_mace", crate::BattleArmAttack::Mace),
        ("unit_saw", crate::BattleArmAttack::Saw),
        ("unit_claw", crate::BattleArmAttack::Claw),
    ] {
        let callback = lua.create_function(
            move |lua, (id, pilot, leg, target): (i64, i64, Option<String>, Option<i64>)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    let config = crate::lua::configuration(lua);
                    let report = crate::btech::physical::configured_arm_attack(
                        &scripts,
                        &config,
                        ObjectId(id),
                        ObjectId(pilot),
                        leg.as_deref(),
                        target.map(ObjectId),
                        kind,
                    )
                    .map_err(mlua::Error::external)?;
                    detached(lua, &report)
                })
            },
        )?;
        native.set(name, error::wrap(lua, callback, "btech.operation.failed")?)?;
    }
    for (name, suite, mode) in [
        (
            "unit_ecm",
            crate::BattleElectronicSuite::Guardian,
            crate::BattleElectronicMode::Ecm,
        ),
        (
            "unit_eccm",
            crate::BattleElectronicSuite::Guardian,
            crate::BattleElectronicMode::Eccm,
        ),
        (
            "unit_angelecm",
            crate::BattleElectronicSuite::Angel,
            crate::BattleElectronicMode::Ecm,
        ),
        (
            "unit_angeleccm",
            crate::BattleElectronicSuite::Angel,
            crate::BattleElectronicMode::Eccm,
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
    let searchlight = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notice = crate::toggle_battle_searchlight(
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
        "unit_slite",
        error::wrap(lua, searchlight, "btech.operation.failed")?,
    )?;
    let arms = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notice = crate::flip_battle_arms(
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
        "unit_fliparms",
        error::wrap(lua, arms, "btech.operation.failed")?,
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
    let shared = world.clone();
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
                crate::BattleHexCoordinate { x, y },
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
    for (name, station_only) in [("unit_lock_hex", false), ("gunner_lock_hex", true)] {
        let lock_hex = lua.create_function(
            move |lua, (unit, pilot, x, y, mode): (i64, i64, i32, i32, Option<String>)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    if station_only {
                        crate::gunner_context(&scripts.world(), ObjectId(unit), ObjectId(pilot))
                            .map_err(mlua::Error::external)?;
                    }
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
                        crate::BattleHexCoordinate { x, y },
                        mode,
                    )
                    .map_err(mlua::Error::external)?;
                    crate::btech::notify_unit(&scripts, notice).map_err(mlua::Error::external)?;
                    Ok(true)
                })
            },
        )?;
        native.set(name, error::wrap(lua, lock_hex, "btech.operation.failed")?)?;
    }
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
    let flamerheat = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_flamer_heat(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_flamerheat",
        error::wrap(lua, flamerheat, "btech.operation.failed")?,
    )?;
    let lbx = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_lbx(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set("unit_lbx", error::wrap(lua, lbx, "btech.operation.failed")?)?;
    for (name, mode) in [
        ("unit_firesmoke", crate::BattleAmmunitionMode::Smoke),
        ("unit_firemine", crate::BattleAmmunitionMode::Mine),
    ] {
        let action = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let selected = crate::toggle_battle_missile_rounds(
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
                    &selected.special_round_message(index),
                )
                .map_err(|e| error::failure("btech.operation.failed", e))?;
                detached(lua, &selected)
            })
        })?;
        native.set(name, error::wrap(lua, action, "btech.operation.failed")?)?;
    }
    let cluster = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_cluster(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.cluster_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_cluster",
        error::wrap(lua, cluster, "btech.operation.failed")?,
    )?;
    let artemis = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_artemis(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.artemis_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_artemis",
        error::wrap(lua, artemis, "btech.operation.failed")?,
    )?;
    let hotload = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_hotload(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.hotload_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_hotload",
        error::wrap(lua, hotload, "btech.operation.failed")?,
    )?;
    let ultra = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_ultra(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.ultra_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_ultra",
        error::wrap(lua, ultra, "btech.operation.failed")?,
    )?;
    let rapid = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_rapid(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.rapid_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_rapid",
        error::wrap(lua, rapid, "btech.operation.failed")?,
    )?;
    let gatling = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_gatling(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.gatling_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_gatling",
        error::wrap(lua, gatling, "btech.operation.failed")?,
    )?;
    let armor_piercing =
        lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let mode = crate::toggle_battle_armor_piercing(
                    &mut scripts.world.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    index,
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                crate::btech::notify_unit_text(
                    &scripts,
                    ObjectId(unit),
                    &mode.armor_piercing_message(index),
                )
                .map_err(|e| error::failure("btech.operation.failed", e))?;
                detached(lua, &mode)
            })
        })?;
    native.set(
        "unit_armor_piercing",
        error::wrap(lua, armor_piercing, "btech.operation.failed")?,
    )?;
    let caseless = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let mode = crate::toggle_battle_caseless(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &mode.caseless_message(index))
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &mode)
        })
    })?;
    native.set(
        "unit_caseless",
        error::wrap(lua, caseless, "btech.operation.failed")?,
    )?;
    let findcenter = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report =
            crate::find_battle_hex_center(&scripts.world.borrow(), ObjectId(unit), ObjectId(pilot))
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_findcenter",
        error::wrap(lua, findcenter, "btech.operation.failed")?,
    )?;
    let navigate = lua.create_function(
        move |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_navigate(
                &scripts.world.borrow(),
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or(""),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_navigate",
        error::wrap(lua, navigate, "btech.operation.failed")?,
    )?;
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
    let tactical = lua.create_function(
        move |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_tactical_map(
                &scripts.world.borrow(),
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or(""),
                crate::battle_view_dimensions(&scripts.world.borrow(), ObjectId(pilot))
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_tactical",
        error::wrap(lua, tactical, "btech.operation.failed")?,
    )?;
    let lrsmap = lua.create_function(
        move |lua, (unit, pilot, mode, arguments): (i64, i64, String, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::btech::long_range_map::from_arguments(
                &scripts.world.borrow(),
                ObjectId(unit),
                ObjectId(pilot),
                &mode,
                arguments.as_deref().unwrap_or(""),
                crate::battle_view_dimensions(&scripts.world.borrow(), ObjectId(pilot))
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_lrsmap",
        error::wrap(lua, lrsmap, "btech.operation.failed")?,
    )?;
    let viewport = lua.create_function(
        move |lua,
              (unit, pilot, kind, arguments, dimensions): (
            i64,
            i64,
            String,
            Option<String>,
            Option<mlua::Table>,
        )| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let kind = match kind.as_str() {
                "tactical" => crate::BattleViewKind::Tactical,
                "long_range" => crate::BattleViewKind::LongRange,
                _ => {
                    return Err(error::failure(
                        "btech.operation.failed",
                        "Invalid view kind",
                    ));
                }
            };
            let defaults = crate::battle_view_dimensions(&scripts.world.borrow(), ObjectId(pilot))
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            let dimensions = match dimensions {
                Some(table) => crate::BattleViewDimensions {
                    tactical_width: table
                        .get::<Option<u16>>("tactical_width")?
                        .unwrap_or(defaults.tactical_width),
                    tactical_height: table
                        .get::<Option<u16>>("tactical_height")?
                        .unwrap_or(defaults.tactical_height),
                    long_range_height: table
                        .get::<Option<u16>>("long_range_height")?
                        .unwrap_or(defaults.long_range_height),
                },
                None => defaults,
            };
            let report = crate::resolve_battle_viewport(
                &scripts.world.borrow(),
                ObjectId(unit),
                ObjectId(pilot),
                kind,
                arguments.as_deref().unwrap_or(""),
                dimensions,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_viewport",
        error::wrap(lua, viewport, "btech.operation.failed")?,
    )?;
    let view_center = lua.create_function(
        move |lua, (unit, pilot, kind, arguments): (i64, i64, String, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let kind = match kind.as_str() {
                "tactical" => crate::BattleViewKind::Tactical,
                "long_range" => crate::BattleViewKind::LongRange,
                _ => {
                    return Err(error::failure(
                        "btech.operation.failed",
                        "Invalid view kind",
                    ));
                }
            };
            let report = crate::parse_battle_view_center(
                &scripts.world.borrow(),
                ObjectId(unit),
                ObjectId(pilot),
                kind,
                arguments.as_deref().unwrap_or(""),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_view_center",
        error::wrap(lua, view_center, "btech.operation.failed")?,
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
            let config = crate::lua::configuration(lua);
            crate::set_radio_frequency_action(
                &scripts,
                &config,
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
            let mode = crate::BattleRadioMode::parse(&mode, capabilities)
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
    let rac = lua.create_function(
        move |lua, (unit, pilot, index, rounds): (i64, i64, usize, Option<u8>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let rounds = rounds.unwrap_or(1);
            crate::lua::transactions::run(lua, &scripts.world, || {
                let changed = crate::set_battle_rotary(
                    &mut scripts.world.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    index,
                    rounds,
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                crate::btech::notify_unit_text(
                    &scripts,
                    ObjectId(unit),
                    &crate::btech::rotary::message(index, rounds, changed),
                )
                .map_err(|e| error::failure("btech.operation.failed", e))?;
                Ok(changed)
            })
        },
    )?;
    native.set("unit_rac", error::wrap(lua, rac, "btech.operation.failed")?)?;
    let unjam = lua.create_function(move |lua, (unit, pilot, index): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let message = crate::begin_battle_unjam(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                index,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit_text(&scripts, ObjectId(unit), &message)
                .map_err(|e| error::failure("btech.operation.failed", e))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_unjam",
        error::wrap(lua, unjam, "btech.operation.failed")?,
    )?;
    for (name, station) in [("unit_sight", false), ("gunner_sight", true)] {
        let sighting_world = world.clone();
        let sight = lua.create_function(
            move |lua, (unit, pilot, weapon, target): (i64, i64, usize, Value)| {
                crate::lua::transactions::require(lua)?;
                if station {
                    crate::gunner_context(
                        &sighting_world.borrow(),
                        ObjectId(unit),
                        ObjectId(pilot),
                    )
                    .map_err(mlua::Error::external)?;
                }
                crate::lua::transactions::run(lua, &sighting_world, || {
                    let config = crate::lua::configuration(lua);
                    let scripts = crate::Scripts::services(lua)?;
                    let report = crate::btech::sight::resolve_action(
                        &scripts,
                        &config,
                        ObjectId(unit),
                        ObjectId(pilot),
                        weapon,
                        firing_target(lua, target.clone())?,
                    )
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                    detached(lua, &report)
                })
            },
        )?;
        native.set(name, error::wrap(lua, sight, "btech.operation.failed")?)?;
    }
    for (name, station) in [("unit_fire", false), ("gunner_fire", true)] {
        let firing_world = world.clone();
        let fire = lua.create_function(
            move |lua, (unit, pilot, weapon, target): (i64, i64, usize, Value)| {
                crate::lua::transactions::require(lua)?;
                if station {
                    crate::gunner_context(&firing_world.borrow(), ObjectId(unit), ObjectId(pilot))
                        .map_err(mlua::Error::external)?;
                }
                crate::lua::transactions::run(lua, &firing_world, || {
                    let config = crate::lua::configuration(lua);
                    let scripts = crate::Scripts::services(lua)?;
                    let report = crate::btech::firing::resolve_action(
                        &scripts,
                        &config,
                        ObjectId(unit),
                        ObjectId(pilot),
                        weapon,
                        firing_target(lua, target.clone())?,
                    )
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                    detached(lua, &report)
                })
            },
        )?;
        native.set(name, error::wrap(lua, fire, "btech.operation.failed")?)?;
    }
    let explode_safe = lua.create_function(move |lua, (unit, safe): (i64, bool)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::set_battle_self_destruct_safe(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                safe,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_explode_safe",
        error::wrap(lua, explode_safe, "btech.operation.failed")?,
    )?;
    let explode = lua.create_function(move |lua, (unit, pilot, text): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let config = crate::lua::configuration(lua);
        crate::lua::transactions::run(lua, &scripts.world, || {
            crate::self_destruct_action(&scripts, &config, ObjectId(unit), ObjectId(pilot), &text)
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_explode",
        error::wrap(lua, explode, "btech.operation.failed")?,
    )?;
    let reactor_explode = lua.create_function(move |lua, unit: i64| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let config = crate::lua::configuration(lua);
        crate::lua::transactions::run(lua, &scripts.world, || {
            let report = crate::reactor_explosion_action(&scripts, &config, ObjectId(unit))
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        })
    })?;
    native.set(
        "unit_reactor_explode",
        error::wrap(lua, reactor_explode, "btech.operation.failed")?,
    )?;
    let autoturret = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notice = crate::toggle_battle_automatic_turret(
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
        "unit_autoturret",
        error::wrap(lua, autoturret, "btech.operation.failed")?,
    )?;
    let disable = lua.create_function(move |lua, (unit, pilot, weapon): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let notice = crate::disable_gauss_weapon(
                &mut scripts.world.borrow_mut(),
                ObjectId(unit),
                ObjectId(pilot),
                weapon,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            crate::btech::notify_unit(&scripts, notice)
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            Ok(true)
        })
    })?;
    native.set(
        "unit_disable",
        error::wrap(lua, disable, "btech.operation.failed")?,
    )?;
    let usebin = lua.create_function(
        move |lua, (unit, pilot, weapon, section): (i64, i64, usize, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let notice = crate::set_battle_ammunition_section(
                    &mut scripts.world.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    weapon,
                    section.as_deref().filter(|s| !s.starts_with('-')),
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                crate::btech::notify_unit(&scripts, notice)
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                Ok(true)
            })
        },
    )?;
    native.set(
        "unit_usebin",
        error::wrap(lua, usebin, "btech.operation.failed")?,
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
    let tic_fire = lua.create_function(
        move |lua, (unit, pilot, groups, target): (i64, i64, Vec<usize>, Value)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let config = crate::lua::configuration(lua);
                let report = crate::fire_battle_tics(
                    &scripts,
                    &config,
                    ObjectId(unit),
                    ObjectId(pilot),
                    groups,
                    firing_target(lua, target.clone())?,
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                detached(lua, &report)
            })
        },
    )?;
    native.set(
        "unit_tic_fire",
        error::wrap(lua, tic_fire, "btech.operation.failed")?,
    )?;
    let tic_world = world.clone();
    let tic = lua.create_function(move |lua, (unit, pilot, group): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let members =
            crate::battle_tic(&tic_world.borrow(), ObjectId(unit), ObjectId(pilot), group)
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &members)
    })?;
    native.set("unit_tic", error::wrap(lua, tic, "btech.operation.failed")?)?;
    let tic_world = world.clone();
    let tic_edit = lua.create_function(
        move |lua,
              (unit, pilot, group, operation, members): (
            i64,
            i64,
            usize,
            String,
            Option<Vec<usize>>,
        )| {
            crate::lua::transactions::require(lua)?;
            crate::lua::transactions::run(lua, &tic_world, || {
                let edit = match operation.as_str() {
                    "add" => crate::BattleTicEdit::Add(members.ok_or_else(|| {
                        error::failure("btech.operation.failed", "Supply weapon numbers")
                    })?),
                    "remove" => crate::BattleTicEdit::Remove(members.ok_or_else(|| {
                        error::failure("btech.operation.failed", "Supply weapon numbers")
                    })?),
                    "clear" if members.is_none() => crate::BattleTicEdit::Clear,
                    _ => {
                        return Err(error::failure(
                            "btech.operation.failed",
                            "Use add, remove or clear",
                        ));
                    }
                };
                crate::edit_battle_tic(
                    &mut tic_world.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    group,
                    edit,
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))
            })
        },
    )?;
    native.set(
        "unit_tic_edit",
        error::wrap(lua, tic_edit, "btech.operation.failed")?,
    )?;
    let weapons_world = world.clone();
    let weapons = lua.create_function(move |lua, unit: i64| {
        crate::lua::transactions::require(lua)?;
        if weapons_world
            .borrow()
            .btech
            .vehicles()
            .contains_key(&ObjectId(unit))
        {
            let weapons = crate::btech::firing::vehicle_weapon_states(
                &weapons_world.borrow(),
                ObjectId(unit),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            return detached(lua, &weapons);
        }
        let weapons = crate::btech::firing::weapon_states(&weapons_world.borrow(), ObjectId(unit))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &weapons)
    })?;
    native.set(
        "unit_weapons",
        error::wrap(lua, weapons, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let diagnostics = lua.create_function(move |lua, id: i64| {
        crate::lua::transactions::require(lua)?;
        let rows = crate::battle_weapon_diagnostics(&shared.borrow(), ObjectId(id))
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &rows)
    })?;
    native.set(
        "unit_weapon_diagnostics",
        error::wrap(lua, diagnostics, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let specifications = lua.create_function(move |lua, id: i64| {
        crate::lua::transactions::require(lua)?;
        let rows = crate::battle_weapon_specifications(
            &shared.borrow(),
            ObjectId(id),
            crate::lua::configuration(lua).battletech.erange != 0,
        )
        .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &rows)
    })?;
    native.set(
        "unit_weapon_specifications",
        error::wrap(lua, specifications, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let criticals = lua.create_function(move |lua, (id, section): (i64, String)| {
        crate::lua::transactions::require(lua)?;
        let report = crate::battle_critical_report(
            &shared.borrow(),
            ObjectId(id),
            &section,
            crate::lua::configuration(lua).battletech.parts != 0,
        )
        .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_criticals",
        error::wrap(lua, criticals, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let aimed = lua.create_function(move |lua, id: i64| {
        crate::lua::transactions::require(lua)?;
        let selected = crate::battle_aimed_section(&shared.borrow(), ObjectId(id))
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &selected)
    })?;
    native.set(
        "unit_aimed_section",
        error::wrap(lua, aimed, "btech.operation.failed")?,
    )?;
    let target = lua.create_function(|lua, (id, pilot, section): (i64, i64, Option<String>)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let selected = crate::lua::transactions::run(lua, &scripts.world, || {
            crate::btech::aimed_target::action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(id),
                ObjectId(pilot),
                section.as_deref().filter(|section| *section != "-"),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))
        })
        .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &selected)
    })?;
    native.set(
        "unit_target",
        error::wrap(lua, target, "btech.operation.failed")?,
    )?;
    // Station facade guards reuse installed display bindings and preserve their argument grammar.
    for method in [
        "bearing",
        "range_report",
        "vector",
        "eta",
        "findcenter",
        "tactical",
        "lrsmap",
        "navigate",
        "scan",
        "report",
        "scan_hex",
        "scan_building",
        "scan_terrain",
        "scan_selected",
    ] {
        let measure: mlua::Function = native.get(format!("unit_{method}"))?;
        let shared = world.clone();
        let station_measure = lua.create_function(move |lua, arguments: mlua::MultiValue| {
            crate::lua::transactions::require(lua)?;
            let station = <i64 as mlua::FromLua>::from_lua(
                arguments.front().cloned().unwrap_or(Value::Nil),
                lua,
            )?;
            let gunner = <i64 as mlua::FromLua>::from_lua(
                arguments.get(1).cloned().unwrap_or(Value::Nil),
                lua,
            )?;
            crate::gunner_context(&shared.borrow(), ObjectId(station), ObjectId(gunner))
                .map_err(mlua::Error::external)?;
            measure.call::<Value>(arguments)
        })?;
        native.set(
            format!("gunner_{method}"),
            error::wrap(lua, station_measure, "btech.operation.failed")?,
        )?;
    }
    api.set("btech", native)
}

/// Publish the canonical namespace with explicit asset/read-only identity operations.
pub(super) fn install(lua: &Lua, api: &Table, mux: &Table) -> mlua::Result<()> {
    let native: Table = api.get("btech")?;
    let package = lua.create_table()?;
    for (group, name, method) in [
        ("runtime", "runtime_stats", "stats"),
        ("unit", "unit_set_team", "set_team"),
        ("unit", "unit_losemit", "losemit"),
        ("unit", "unit_damage_section", "damage_section"),
        ("unit", "unit_damage", "damage"),
        ("unit", "unit_weight", "weight"),
        ("unit", "unit_setxy", "setxy"),
        ("unit", "unit_fields", "fields"),
        ("unit", "unit_set_field", "set_field"),
        ("unit", "unit_ood", "ood"),
        ("unit", "unit_setmapindex", "setmapindex"),
        ("unit", "unit_markings", "markings"),
        ("unit", "unit_set_markings", "set_markings"),
        ("unit", "unit_view_markings", "view"),
        ("unit", "unit_display_name", "display_name"),
        ("unit", "unit_set_display_name", "set_display_name"),
        ("unit", "unit_set_preferred_id", "set_preferred_id"),
        ("cargo", "cargo_manifest", "manifest"),
        ("cargo", "cargo_stores", "stores"),
        ("cargo", "cargo_load", "load"),
        ("cargo", "cargo_unload", "unload"),
        ("map", "map_cargo_point", "cargo_point"),
        ("map", "map_set_cargo_point", "set_cargo_point"),
        ("inventory", "inventory_part", "part"),
        ("inventory", "inventory_mass", "mass"),
        ("inventory", "inventory_set_named", "set_named"),
        ("inventory", "inventory_read", "read"),
        ("inventory", "inventory_add", "add"),
        ("inventory", "inventory_add_stores", "add_stores"),
        ("inventory", "inventory_remove", "remove"),
        ("inventory", "inventory_clear", "clear"),
        ("inventory", "inventory_fix", "fix"),
        ("inventory", "inventory_set", "set"),
        ("weapon", "weapon_settings", "settings"),
        ("weapon", "weapon_set_recycle", "set_recycle"),
        ("weapon", "weapon_set_battle_value", "set_battle_value"),
        ("unit", "unit_brief", "brief"),
        ("unit", "unit_lateral", "lateral"),
        ("unit", "unit_turnmode", "turnmode"),
        ("unit", "unit_dump", "dump"),
        ("unit", "unit_masc", "masc"),
        ("unit", "unit_c3i", "c3i"),
        ("unit", "unit_c3", "c3"),
        ("unit", "unit_c3i_message", "c3i_message"),
        ("unit", "unit_c3_message", "c3_message"),
        ("unit", "unit_c3_network", "c3_network"),
        ("unit", "unit_c3_targets", "c3_targets"),
        ("unit", "unit_c3i_network", "c3i_network"),
        ("unit", "unit_c3i_targets", "c3i_targets"),
        ("unit", "unit_supercharger", "supercharger"),
        ("unit", "unit_eta", "eta"),
        ("unit", "unit_bearing", "bearing"),
        ("unit", "unit_range_report", "range_report"),
        ("unit", "unit_vector", "vector"),
        ("unit", "unit_bootlegger", "bootlegger"),
        ("unit", "unit_building_contacts", "building_contacts"),
        ("player", "player_contact_options", "contact_options"),
        ("player", "player_view_dimensions", "view_dimensions"),
        (
            "player",
            "player_contact_preferences",
            "contact_preferences",
        ),
        ("gunner", "gunner_set_field", "set_field"),
        ("gunner", "gunner_view_fields", "view_fields"),
        ("gunner", "gunner_contacts", "contacts"),
        ("gunner", "gunner_status", "status"),
        ("gunner", "gunner_scan", "scan"),
        ("gunner", "gunner_report", "report"),
        ("gunner", "gunner_scan_hex", "scan_hex"),
        ("gunner", "gunner_scan_building", "scan_building"),
        ("gunner", "gunner_scan_terrain", "scan_terrain"),
        ("gunner", "gunner_scan_selected", "scan_selected"),
        ("gunner", "gunner_tactical", "tactical"),
        ("gunner", "gunner_lrsmap", "lrsmap"),
        ("gunner", "gunner_navigate", "navigate"),
        ("gunner", "gunner_bearing", "bearing"),
        ("gunner", "gunner_range_report", "range_report"),
        ("gunner", "gunner_vector", "vector"),
        ("gunner", "gunner_eta", "eta"),
        ("gunner", "gunner_findcenter", "findcenter"),
        ("gunner", "gunner_register", "register"),
        ("gunner", "gunner_initialize", "initialize"),
        ("gunner", "gunner_deinitialize", "deinitialize"),
        ("gunner", "gunner_state", "state"),
        ("gunner", "gunner_gunnery", "gunnery"),
        ("gunner", "gunner_aim", "aim"),
        ("gunner", "gunner_fire", "fire"),
        ("gunner", "gunner_sight", "sight"),
        ("gunner", "gunner_artillery_gunnery", "artillery_gunnery"),
        ("gunner", "gunner_lock", "lock"),
        ("gunner", "gunner_lock_hex", "lock_hex"),
        ("character", "character_state", "state"),
        ("character", "character_list", "list"),
        ("character", "character_skills", "skills"),
        ("character", "character_advantages", "advantages"),
        ("character", "character_threshold", "threshold"),
        ("character", "character_xptop", "xptop"),
        ("database", "database_save", "save"),
        ("inventory", "inventory_forms", "forms"),
        ("character", "character_progress", "progress"),
        ("unit", "unit_evacuate", "evacuate"),
        ("character", "character_set_threshold", "set_threshold"),
        ("template", "template", "inspect"),
        ("template", "template_check", "check"),
        ("template", "loadout", "loadout"),
        ("map", "mapfile", "inspect_file"),
        ("unit", "unit", "inspect"),
        ("unit", "unit_fuel", "fuel"),
        ("unit", "unit_set_fuel", "set_fuel"),
        ("unit", "unit_create", "create"),
        ("unit", "unit_state", "state"),
        ("unit", "unit_towable", "towable"),
        ("unit", "unit_fortified", "fortified"),
        ("unit", "unit_observer", "observer"),
        ("unit", "unit_weapons_hold", "weapons_hold"),
        ("unit", "unit_combat_safe", "combat_safe"),
        ("unit", "unit_visibility", "visibility"),
        ("unit", "unit_kick", "kick"),
        ("unit", "unit_trip", "trip"),
        ("unit", "unit_punch", "punch"),
        ("unit", "unit_axe", "axe"),
        ("unit", "unit_sword", "sword"),
        ("unit", "unit_mace", "mace"),
        ("unit", "unit_saw", "saw"),
        ("unit", "unit_claw", "claw"),
        ("unit", "unit_club", "club"),
        ("unit", "unit_grabclub", "grabclub"),
        ("unit", "unit_charge", "charge"),
        ("unit", "unit_status", "status"),
        ("unit", "unit_place", "place"),
        ("unit", "unit_remove", "remove"),
        ("unit", "unit_pilot", "pilot"),
        ("unit", "unit_release", "release"),
        ("unit", "unit_start", "start"),
        ("unit", "unit_stop", "stop"),
        ("unit", "unit_range", "range"),
        ("unit", "unit_heading", "heading"),
        ("unit", "unit_turret", "turret"),
        ("unit", "unit_fixturret", "fixturret"),
        ("unit", "unit_speed", "speed"),
        ("unit", "unit_rottorso", "rottorso"),
        ("unit", "unit_slite", "slite"),
        ("unit", "unit_stealth", "stealth"),
        ("unit", "unit_nss", "nss"),
        ("unit", "unit_fliparms", "fliparms"),
        ("unit", "unit_auto_fall", "auto_fall"),
        ("unit", "unit_ams", "ams"),
        ("unit", "unit_ecm", "ecm"),
        ("unit", "unit_eccm", "eccm"),
        ("unit", "unit_angelecm", "angelecm"),
        ("unit", "unit_angeleccm", "angeleccm"),
        ("unit", "unit_pods", "pods"),
        ("unit", "unit_removepod", "removepod"),
        ("unit", "unit_removepods", "removepods"),
        ("unit", "unit_extinguish", "extinguish"),
        ("unit", "unit_inarc", "inarc"),
        ("unit", "unit_narc", "narc"),
        ("unit", "unit_explosive", "explosive"),
        ("unit", "unit_friendly_fire_safety", "friendly_fire_safety"),
        ("unit", "unit_mw_safety", "mw_safety"),
        ("unit", "unit_snipe", "snipe"),
        ("unit", "unit_bth_debug", "bth_debug"),
        ("unit", "unit_autocon_shutdown", "autocon_shutdown"),
        ("unit", "unit_armor_warning", "armor_warning"),
        ("unit", "unit_ammunition_warning", "ammunition_warning"),
        ("unit", "unit_searchlight_warning", "searchlight_warning"),
        ("unit", "unit_jump", "jump"),
        ("unit", "unit_dfa", "dfa"),
        ("unit", "unit_land", "land"),
        ("unit", "unit_pickup", "pickup"),
        ("unit", "unit_dropoff", "dropoff"),
        ("unit", "unit_dig", "dig"),
        ("unit", "unit_hulldown", "hulldown"),
        ("unit", "unit_takeoff", "takeoff"),
        ("unit", "unit_enterbase", "enterbase"),
        ("unit", "unit_vertical", "vertical"),
        ("unit", "unit_stand", "stand"),
        ("unit", "unit_prone", "prone"),
        ("unit", "unit_sensors", "sensors"),
        ("unit", "unit_sensor_report", "sensor_report"),
        ("unit", "unit_tag", "tag"),
        ("unit", "unit_spot", "spot"),
        ("unit", "unit_contacts", "contacts"),
        ("unit", "unit_lock", "lock"),
        ("unit", "unit_lock_hex", "lock_hex"),
        ("unit", "unit_gunnery", "gunnery"),
        ("unit", "unit_aim_hex", "aim_hex"),
        ("unit", "unit_fire", "fire"),
        ("unit", "unit_sight", "sight"),
        ("unit", "unit_target", "target"),
        ("unit", "unit_aimed_section", "aimed_section"),
        ("unit", "unit_tic", "tic"),
        ("unit", "unit_tic_fire", "tic_fire"),
        ("unit", "unit_heatcutoff", "heatcutoff"),
        ("unit", "unit_hide", "hide"),
        ("unit", "unit_disable", "disable"),
        ("unit", "unit_usebin", "usebin"),
        ("unit", "unit_autoturret", "autoturret"),
        ("unit", "unit_reactor_explode", "reactor_explode"),
        ("unit", "unit_explode", "explode"),
        ("unit", "unit_explode_safe", "explode_safe"),
        ("unit", "unit_tic_edit", "tic_edit"),
        ("unit", "unit_flamerheat", "flamerheat"),
        ("unit", "unit_lbx", "lbx"),
        ("unit", "unit_cluster", "cluster"),
        ("unit", "unit_cluster", "firecluster"),
        ("unit", "unit_firesmoke", "firesmoke"),
        ("unit", "unit_firemine", "firemine"),
        ("unit", "unit_artemis", "artemis"),
        ("unit", "unit_hotload", "hotload"),
        ("unit", "unit_ultra", "ultra"),
        ("unit", "unit_rapid", "rapidfire"),
        ("unit", "unit_gatling", "gattling"),
        ("unit", "unit_rac", "rac"),
        ("unit", "unit_armor_piercing", "armorpiercing"),
        ("unit", "unit_caseless", "caseless"),
        ("unit", "unit_incendiary", "incendiary"),
        ("unit", "unit_scan", "scan"),
        ("unit", "unit_scan_hex", "scan_hex"),
        ("unit", "unit_report", "report"),
        ("unit", "unit_view_center", "view_center"),
        ("unit", "unit_viewport", "viewport"),
        ("unit", "unit_findcenter", "findcenter"),
        ("unit", "unit_navigate", "navigate"),
        ("unit", "unit_tactical", "tactical"),
        ("unit", "unit_lrsmap", "lrsmap"),
        ("unit", "unit_scan_selected", "scan_selected"),
        ("unit", "unit_scan_terrain", "scan_terrain"),
        ("unit", "unit_scan_building", "scan_building"),
        ("unit", "unit_radio_target", "radio_target"),
        ("unit", "unit_radio_send", "radio_send"),
        ("unit", "unit_radio_frequency", "radio_frequency"),
        ("unit", "unit_radio_title", "radio_title"),
        ("unit", "unit_radio_mode", "radio_mode"),
        ("unit", "unit_inferno", "inferno"),
        ("unit", "unit_precision", "precision"),
        ("unit", "unit_sguided", "sguided"),
        ("unit", "unit_fireswarm", "fireswarm"),
        ("unit", "unit_fireswarm1", "fireswarm1"),
        ("unit", "unit_mml", "mml"),
        ("unit", "unit_atmrange", "atmrange"),
        ("unit", "unit_atmexplosive", "atmexplosive"),
        ("unit", "unit_stinger", "stinger"),
        ("unit", "unit_flechette", "flechette"),
        ("unit", "unit_unjam", "unjam"),
        ("unit", "unit_weapons", "weapons"),
        ("unit", "unit_weapon_diagnostics", "weapon_diagnostics"),
        ("unit", "unit_criticals", "criticals"),
        (
            "unit",
            "unit_weapon_specifications",
            "weapon_specifications",
        ),
        ("map", "map", "inspect"),
        ("map", "map_hex", "hex"),
        ("map", "map_emit", "emit"),
        ("map", "map_clear_units", "clear_units"),
        ("map", "map_resize", "resize"),
        ("map", "map_save", "save"),
        ("map", "map_load", "load"),
        ("map", "map_view", "view"),
        ("map", "map_check", "check"),
        ("map", "map_fields", "fields"),
        ("map", "map_set_field", "set_field"),
        ("map", "map_add_mine", "add_mine"),
        ("map", "map_add_fire", "add_fire"),
        ("map", "map_add_smoke", "add_smoke"),
        ("map", "map_link", "link"),
        ("map", "map_set_link", "set_link"),
        ("map", "map_update_links", "update_links"),
        ("map", "map_list", "list"),
        ("map", "map_delete_objects", "delete_objects"),
        ("map", "map_add_block", "add_block"),
        ("map", "map_set_hex", "set_hex"),
        ("map", "map_create", "create"),
        ("map", "map_reload", "reload"),
        ("map", "map_conditions", "conditions"),
        ("map", "map_cloud", "cloud_base"),
        ("map", "map_environment", "environment"),
        ("map", "map_add_ice", "add_ice"),
        ("map", "map_remove_ice", "remove_ice"),
        ("map", "map_wrapping", "wrapping"),
    ] {
        let table = match package.get::<Value>(group)? {
            Value::Table(table) => table,
            _ => lua.create_table()?,
        };
        // Resolve through the private table so checking restrictions cannot leave a live closure behind.
        let dispatch: mlua::Function = lua
            .load("local native,key=...; return function(...) return native[key](...) end")
            .call((native.clone(), name))?;
        table.set(method, dispatch)?;
        package.set(group, table)?;
    }
    let errors: Table = mux.get("error")?;
    let codes: mlua::Function = errors.get("code_tree")?;
    package.set("errors", codes.call::<Value>("btech")?)?;
    lua.globals().set("btech", package.clone())?;
    lua.globals()
        .get::<Table>("package")?
        .get::<Table>("loaded")?
        .set("btech", package)?;
    Ok(())
}

/// Lua-facing optional fields use nil, so absent locks, positions and shot results are falsey.
fn detached<T: serde::Serialize + ?Sized>(lua: &Lua, value: &T) -> mlua::Result<Value> {
    lua.to_value_with(
        value,
        mlua::serde::SerializeOptions::new()
            .serialize_none_to_null(false)
            .serialize_unit_to_null(false),
    )
}

/// Unit firing and TICs accept a dbref, an X/Y table, or nil for the cockpit selection.
fn firing_target(lua: &Lua, value: Value) -> mlua::Result<crate::BattleFireTarget> {
    match value {
        Value::Nil => Ok(crate::BattleFireTarget::Selected),
        Value::Table(_) => Ok(crate::BattleFireTarget::Hex {
            coordinate: lua.from_value(value)?,
        }),
        _ => Ok(crate::BattleFireTarget::Unit {
            unit: ObjectId(lua.from_value(value)?),
        }),
    }
}
