//! Unit and map inspection, administrative editing, persistence, and gunner-station native bindings.

use super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
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
    Ok(())
}
