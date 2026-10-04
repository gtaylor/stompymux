//! Battle map viewing, terrain, links, persistence, environment, and map flag native bindings.

use super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let view_map = lua.create_function(move |lua, (actor, map, x, y): (i64, i64, i32, i32)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            let report = crate::view_battle_map_action(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(actor),
                ObjectId(map),
                crate::HexCoordinate { x, y },
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
                        coordinate: crate::HexCoordinate { x, y },
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
        ("map_add_fire", crate::DecorationKind::Fire),
        ("map_add_smoke", crate::DecorationKind::Smoke),
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
                        crate::HexCoordinate { x, y },
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
                (Some(x), Some(y)) => Some(crate::HexCoordinate { x, y }),
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
                    crate::HexCoordinate { x, y },
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
    let conditions = lua.create_function(|lua, (id, light, visibility): (i64, Value, i64)| {
        crate::lua::transactions::require(lua)?;
        let light = constants::require(light, 2, "light", &constants::LIGHT_LEVELS)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            (|| -> anyhow::Result<_> {
                crate::set_battle_map_visibility(
                    &mut scripts.world_mut(),
                    ObjectId(id),
                    crate::BattleLight::from_stored(i64::from(light))?,
                    u8::try_from(visibility)?,
                )
            })()
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            Ok(true)
        })
    })?;
    native.set(
        "map_conditions",
        error::wrap(lua, conditions, "btech.operation.failed")?,
    )?;
    let shared = world.clone();
    let has_flag = lua.create_function(move |lua, (id, flag): (i64, Value)| {
        crate::lua::transactions::require(lua)?;
        let flag = constants::require_map_flag(flag, 2)?;
        let world = shared.borrow();
        let map = world
            .btech
            .maps()
            .get(&ObjectId(id))
            .ok_or_else(|| error::failure("btech.operation.failed", "Map not found"))?;
        Ok(map.has_flag(flag))
    })?;
    native.set(
        "map_has_flag",
        error::wrap(lua, has_flag, "btech.operation.failed")?,
    )?;
    let set_flag =
        lua.create_function(|lua, (actor, id, flag, enabled): (i64, i64, Value, bool)| {
            crate::lua::transactions::require(lua)?;
            let flag = constants::require_map_flag(flag, 3)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                crate::set_battle_map_flag_action(
                    &scripts,
                    &crate::lua::configuration(lua),
                    ObjectId(actor),
                    ObjectId(id),
                    flag,
                    enabled,
                )
                .map_err(mlua::Error::external)
            })
        })?;
    native.set(
        "map_set_flag",
        error::wrap(lua, set_flag, "btech.operation.failed")?,
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
    Ok(())
}
