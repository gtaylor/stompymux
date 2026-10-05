//! C-compatible `btech.map` contracts and explicit names for earlier Rust conveniences.

use super::{contract, error};
use crate::btech::{
    MapEmitAudience, MapLos, MapSpatialPoint, emit_battle_map_trusted_action,
    load_battle_map_trusted_action, map_hex_los, map_hex_point, map_members, map_spatial_range,
    map_unit_by_label, map_unit_los, map_unit_map, map_unit_point, place_battle_map_unit,
    set_cargo_transfer_point, set_map_link, update_battle_map_links_trusted_action,
};
use crate::{CargoTransferPoint, HexCoordinate, MapEntrance, MapLink, ObjectId, SharedWorld};
use mlua::{Function, Lua, MultiValue, Table, Value};

const GROUP: &str = "map";
const MAX_INT: i64 = i32::MAX as i64;

fn arg(arguments: &MultiValue, index: usize) -> Value {
    arguments.get(index).cloned().unwrap_or(Value::Nil)
}

fn argument_failure(argument: usize, message: impl ToString) -> mlua::Error {
    error::failure_with_detail(
        "mux.arg.invalid",
        message,
        serde_json::json!({ "argument": argument }),
    )
}

fn optional_number_field(table: &Table, field: &str, argument: usize) -> mlua::Result<Option<f64>> {
    match contract::field(table, field)? {
        Value::Nil => Ok(None),
        Value::Integer(value) => Ok(Some(value as f64)),
        Value::Number(value) if value.is_finite() => Ok(Some(value)),
        _ => Err(argument_failure(
            argument,
            format!("{field} must be a finite number"),
        )),
    }
}

/// Build the Lua list of `points`, keeping only those whose type matches `kind` exactly when
/// one is given.
pub(super) fn push_points_of_interest(
    lua: &Lua,
    points: &[crate::MapPointOfInterest],
    kind: Option<&str>,
) -> mlua::Result<Table> {
    let output = lua.create_table()?;
    for point in points
        .iter()
        .filter(|point| kind.is_none_or(|kind| point.kind == kind))
    {
        let value = lua.create_table()?;
        value.raw_set("type", point.kind.as_str())?;
        value.raw_set("name", point.name.as_str())?;
        value.raw_set("x", point.x)?;
        value.raw_set("y", point.y)?;
        value.raw_set("elevation", point.elevation)?;
        output.raw_set(output.raw_len() + 1, value)?;
    }
    Ok(output)
}

fn table(value: Value, argument: usize, label: &str) -> mlua::Result<Table> {
    match value {
        Value::Table(table) => Ok(table),
        _ => Err(argument_failure(
            argument,
            format!("{label} must be a table"),
        )),
    }
}

fn map_id(
    lua: &Lua,
    shared: &SharedWorld,
    value: Value,
    argument: usize,
) -> mlua::Result<ObjectId> {
    contract::require_special(lua, &shared.borrow(), value, argument, "map", "map")
}

fn unit_id(
    lua: &Lua,
    shared: &SharedWorld,
    value: Value,
    argument: usize,
    label: &str,
) -> mlua::Result<ObjectId> {
    contract::require_special(lua, &shared.borrow(), value, argument, "unit", label)
}

fn coordinate(
    shared: &SharedWorld,
    map: ObjectId,
    value: Value,
    argument: usize,
    allow_z: bool,
) -> mlua::Result<(HexCoordinate, Option<f64>)> {
    let record = table(value, argument, "value")?;
    contract::check_options(
        &record,
        if allow_z {
            &["x", "y", "z"]
        } else {
            &["x", "y"]
        },
        argument,
    )?;
    let x = contract::integer_field(&record, "x", 0, MAX_INT, argument)?;
    let y = contract::integer_field(&record, "y", 0, MAX_INT, argument)?;
    let point = HexCoordinate {
        x: x as i32,
        y: y as i32,
    };
    shared
        .borrow()
        .btech
        .maps()
        .get(&map)
        .ok_or_else(|| error::failure("mux.object.invalid", "Map not found"))?
        .base_hex(x, y)
        .map_err(|_| argument_failure(argument, "hex is outside the map"))?;
    let z = if allow_z {
        optional_number_field(&record, "z", argument)?
    } else {
        None
    };
    Ok((point, z))
}

fn bind_alias(
    lua: &Lua,
    native: &Table,
    old: &'static str,
    key: &'static str,
    name: &'static str,
) -> mlua::Result<()> {
    let function: Function = native.raw_get(old)?;
    contract::bind(lua, native, key, GROUP, name, function)
}

/// Register strict canonical contracts after retaining distinguishable older extensions.
pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    for (old, key, name) in [
        ("map_link", "map_authored_link", "authored_link"),
        ("map_set_link", "map_set_authored_link", "set_authored_link"),
        ("map_load", "map_load_as", "load_as"),
        ("map_emit", "map_emit_as", "emit_as"),
        ("map_update_links", "map_update_links_as", "update_links_as"),
    ] {
        bind_alias(lua, native, old, key, name)?;
    }

    let shared = world.clone();
    let elevation = lua.create_function(move |lua, arguments: MultiValue| {
        let map = map_id(lua, &shared, arg(&arguments, 0), 1)?;
        let (point, _) = coordinate(&shared, map, arg(&arguments, 1), 2, false)?;
        Ok(shared.borrow().btech.maps()[&map]
            .hex(i64::from(point.x), i64::from(point.y))
            .map_err(mlua::Error::external)?
            .level())
    })?;
    contract::bind(
        lua,
        native,
        "map_contract_elevation",
        GROUP,
        "elevation",
        elevation,
    )?;

    let shared = world.clone();
    let terrain = lua.create_function(move |lua, arguments: MultiValue| {
        let map = map_id(lua, &shared, arg(&arguments, 0), 1)?;
        let (point, _) = coordinate(&shared, map, arg(&arguments, 1), 2, false)?;
        super::detached(
            lua,
            &shared.borrow().btech.maps()[&map]
                .hex(i64::from(point.x), i64::from(point.y))
                .map_err(mlua::Error::external)?
                .terrain(),
        )
    })?;
    contract::bind(
        lua,
        native,
        "map_contract_terrain",
        GROUP,
        "terrain",
        terrain,
    )?;

    let shared = world.clone();
    let unit_by_id = lua.create_function(move |lua, arguments: MultiValue| {
        let origin = contract::require_object(lua, &shared.borrow(), arg(&arguments, 0), 1)?;
        let label = match arg(&arguments, 1) {
            Value::String(value) => {
                if value.as_bytes().len() != 2 || !value.as_bytes().iter().all(u8::is_ascii) {
                    return Err(argument_failure(
                        2,
                        "id must contain exactly two ASCII characters",
                    ));
                }
                value.to_str()?.to_owned()
            }
            _ => return Err(argument_failure(2, "id must be a string")),
        };
        let registration = shared.borrow().btech.registrations().get(&origin).cloned();
        if registration.as_deref().is_none_or(|kind| {
            !kind.eq_ignore_ascii_case("map") && !kind.eq_ignore_ascii_case("unit")
        }) {
            return Err(error::failure_with_detail(
                "mux.object.invalid",
                "object must be a BTech unit or map",
                serde_json::json!({ "argument": 1 }),
            ));
        }
        let found =
            map_unit_by_label(&shared.borrow(), origin, &label).map_err(mlua::Error::external)?;
        contract::push_optional_object(lua, &shared, found)
    })?;
    contract::bind(
        lua,
        native,
        "map_contract_unit_by_id",
        GROUP,
        "unit_by_id",
        unit_by_id,
    )?;

    let shared = world.clone();
    let units = lua.create_function(move |lua, arguments: MultiValue| {
        let map = map_id(lua, &shared, arg(&arguments, 0), 1)?;
        let filter = arguments.get(1).cloned().unwrap_or(Value::Nil);
        let filter = if filter == Value::Nil {
            None
        } else {
            let filter = table(filter, 2, "filter")?;
            contract::check_options(&filter, &["origin", "range"], 2)?;
            let origin_value = contract::field(&filter, "origin")?;
            let (origin, _) = coordinate(&shared, map, origin_value, 2, false)?;
            let range = optional_number_field(&filter, "range", 2)?
                .filter(|range| *range >= 0.0)
                .ok_or_else(|| argument_failure(2, "filter.range must be non-negative"))?;
            Some((origin, range))
        };
        let output = lua.create_table()?;
        let world = shared.borrow();
        for member in map_members(&world, map).map_err(mlua::Error::external)? {
            if filter.is_some_and(|(origin, range)| {
                map_spatial_range(
                    map_hex_point(origin, 0.0),
                    MapSpatialPoint {
                        x: (member.point.x * 322.5) as f32,
                        y: (member.point.y * 322.5) as f32,
                        z: 0.0,
                    },
                )
                .is_ok_and(|actual| actual > range)
            }) {
                continue;
            }
            let index = output.raw_len() + 1;
            output.raw_set(index, contract::push_object(lua, &shared, member.id)?)?;
        }
        Ok(output)
    })?;
    contract::bind(lua, native, "map_contract_units", GROUP, "units", units)?;

    let shared = world.clone();
    let blast_zones = lua.create_function(move |lua, arguments: MultiValue| {
        let map = map_id(lua, &shared, arg(&arguments, 0), 1)?;
        let output = lua.create_table()?;
        for (_, zone) in shared.borrow().btech.maps()[&map].ordered_landing_exclusions() {
            let value = lua.create_table()?;
            value.raw_set("x", zone.coordinate.x)?;
            value.raw_set("y", zone.coordinate.y)?;
            value.raw_set("radius", zone.radius)?;
            output.raw_set(output.raw_len() + 1, value)?;
        }
        Ok(output)
    })?;
    contract::bind(
        lua,
        native,
        "map_contract_blast_zones",
        GROUP,
        "blast_zones",
        blast_zones,
    )?;

    let shared = world.clone();
    let points_of_interest = lua.create_function(move |lua, arguments: MultiValue| {
        let map = map_id(lua, &shared, arg(&arguments, 0), 1)?;
        let kind = match arg(&arguments, 1) {
            Value::Nil => None,
            Value::String(kind) => Some(kind.to_str()?.to_owned()),
            _ => return Err(argument_failure(2, "type must be a string")),
        };
        let points = shared.borrow().btech.maps()[&map]
            .points_of_interest
            .clone();
        push_points_of_interest(lua, &points, kind.as_deref())
    })?;
    contract::bind(
        lua,
        native,
        "map_contract_points_of_interest",
        GROUP,
        "points_of_interest",
        points_of_interest,
    )?;

    let shared = world.clone();
    let in_blast_zone = lua.create_function(move |lua, arguments: MultiValue| {
        let map = map_id(lua, &shared, arg(&arguments, 0), 1)?;
        let (point, _) = coordinate(&shared, map, arg(&arguments, 1), 2, false)?;
        Ok(shared.borrow().btech.maps()[&map]
            .ordered_landing_exclusions()
            .any(|(_, zone)| {
                zone.radius >= 0
                    && map_spatial_range(
                        map_hex_point(point, 0.0),
                        map_hex_point(zone.coordinate, 0.0),
                    )
                    .is_ok_and(|distance| distance <= zone.radius as f64)
            }))
    })?;
    contract::bind(
        lua,
        native,
        "map_contract_in_blast_zone",
        GROUP,
        "in_blast_zone",
        in_blast_zone,
    )?;

    let shared = world.clone();
    let range = lua.create_function(move |lua, arguments: MultiValue| {
        let map = map_id(lua, &shared, arg(&arguments, 0), 1)?;
        let endpoint = |value: Value, argument| -> mlua::Result<MapSpatialPoint> {
            if matches!(value, Value::Table(_)) {
                let (hex, z) = coordinate(&shared, map, value, argument, true)?;
                let terrain = shared.borrow().btech.maps()[&map]
                    .base_hex(i64::from(hex.x), i64::from(hex.y))
                    .map_err(mlua::Error::external)?;
                Ok(map_hex_point(
                    hex,
                    z.unwrap_or(f64::from(terrain.standing_height())),
                ))
            } else {
                let id = unit_id(lua, &shared, value, argument, "range endpoint")?;
                map_unit_point(&shared.borrow(), map, id)
                    .map_err(|error| argument_failure(argument, error))
            }
        };
        map_spatial_range(
            endpoint(arg(&arguments, 1), 2)?,
            endpoint(arg(&arguments, 2), 3)?,
        )
        .map_err(mlua::Error::external)
    })?;
    contract::bind(lua, native, "map_contract_range", GROUP, "range", range)?;

    let shared = world.clone();
    let place = lua.create_function(move |lua, arguments: MultiValue| {
        let unit = unit_id(lua, &shared, arg(&arguments, 0), 1, "unit")?;
        let map = map_id(lua, &shared, arg(&arguments, 1), 2)?;
        let (point, z) = coordinate(&shared, map, arg(&arguments, 2), 3, true)?;
        let z = z
            .map(|z| {
                if z.fract() != 0.0 || !(0.0..=10_000.0).contains(&z) {
                    return Err(argument_failure(
                        3,
                        "z must be an integer from 0 through 10000",
                    ));
                }
                Ok(z as i32)
            })
            .transpose()?;
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            place_battle_map_unit(&mut scripts.world_mut(), unit, map, point, z)
                .map_err(|error| contract::operation_failure("map_placement_failed", error))
        })
    })?;
    contract::bind(
        lua,
        native,
        "map_contract_place_unit",
        GROUP,
        "place_unit",
        place,
    )?;

    let load = lua.create_function(move |lua, arguments: MultiValue| {
        let scripts = crate::Scripts::services(lua)?;
        let map = map_id(lua, &scripts.world, arg(&arguments, 0), 1)?;
        let name = contract::string(arg(&arguments, 1), "name", 8191, 2)?;
        contract::validate_resource_name(&name, "name", 2)?;
        crate::lua::transactions::require(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            load_battle_map_trusted_action(&scripts, &crate::lua::configuration(lua), map, &name)
                .map_err(|error| contract::operation_failure("map_file_invalid", error))
        })
    })?;
    contract::bind(lua, native, "map_contract_load", GROUP, "load", load)?;

    let update_links = lua.create_function(move |lua, arguments: MultiValue| {
        let scripts = crate::Scripts::services(lua)?;
        let map = map_id(lua, &scripts.world, arg(&arguments, 0), 1)?;
        crate::lua::transactions::require(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            update_battle_map_links_trusted_action(&scripts, &crate::lua::configuration(lua), map)
                .map_err(|error| contract::operation_failure("map_links_update_failed", error))
        })
    })?;
    contract::bind(
        lua,
        native,
        "map_contract_update_links",
        GROUP,
        "update_links",
        update_links,
    )?;

    register_emit(lua, native)?;
    register_cargo(lua, native, world)?;
    register_links(lua, native, world)?;
    register_los(lua, native, world)
}

fn register_emit(lua: &Lua, native: &Table) -> mlua::Result<()> {
    let emit = lua.create_function(move |lua, arguments: MultiValue| {
        let scripts = crate::Scripts::services(lua)?;
        let map = map_id(lua, &scripts.world, arg(&arguments, 0), 1)?;
        let message = contract::string(arg(&arguments, 1), "message", 8191, 2)?;
        let options = arguments.get(2).cloned().unwrap_or(Value::Nil);
        let audience = if options == Value::Nil {
            MapEmitAudience::All
        } else {
            let options = table(options, 3, "options")?;
            contract::check_options(&options, &["audience", "origin", "range"], 3)?;
            let audience = match contract::field(&options, "audience")? {
                Value::Nil => "all".to_owned(),
                value => contract::string(value, "audience", 64, 3)?,
            };
            let origin = contract::field(&options, "origin")?;
            let range = contract::field(&options, "range")?;
            match audience.as_str() {
                "all" => {
                    if origin != Value::Nil || range != Value::Nil {
                        return Err(argument_failure(3, "all audience forbids origin and range"));
                    }
                    MapEmitAudience::All
                }
                "range" => {
                    if origin == Value::Nil {
                        return Err(argument_failure(3, "audience requires origin"));
                    }
                    let (hex, z) = coordinate(&scripts.world, map, origin, 3, true)?;
                    if range == Value::Nil {
                        return Err(argument_failure(
                            3,
                            "range audience requires a non-negative range",
                        ));
                    }
                    let range = match range {
                        Value::Integer(value) if value >= 0 => value as f64,
                        Value::Number(value) if value.is_finite() && value >= 0.0 => value,
                        Value::Integer(_) | Value::Number(_) => {
                            return Err(argument_failure(
                                3,
                                "range audience requires a non-negative range",
                            ));
                        }
                        _ => return Err(argument_failure(3, "range must be a finite number")),
                    };
                    MapEmitAudience::Range {
                        origin: hex.center(),
                        z,
                        range,
                    }
                }
                "line_of_sight" => {
                    if origin == Value::Nil {
                        return Err(argument_failure(3, "audience requires origin"));
                    }
                    let (origin, z) = coordinate(&scripts.world, map, origin, 3, true)?;
                    if z.is_some() || range != Value::Nil {
                        return Err(argument_failure(3, "line_of_sight forbids z and range"));
                    }
                    MapEmitAudience::LineOfSight { origin }
                }
                _ => return Err(argument_failure(3, "unknown audience")),
            }
        };
        crate::lua::transactions::require(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            emit_battle_map_trusted_action(&scripts, map, &message, audience)
                .map_err(|error| contract::operation_failure("map_emit_failed", error))
        })
    })?;
    contract::bind(lua, native, "map_contract_emit", GROUP, "emit", emit)
}

fn register_cargo(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let shared = world.clone();
    let get = lua.create_function(move |lua, arguments: MultiValue| {
        contract::check_arity(&arguments, 1)?;
        let map = map_id(lua, &shared, arg(&arguments, 0), 1)?;
        match shared.borrow().btech.maps()[&map].cargo_transfer_point() {
            Some(point) => super::detached(lua, &point),
            None => Ok(Value::Nil),
        }
    })?;
    contract::bind(
        lua,
        native,
        "map_contract_cargo_transfer_point",
        GROUP,
        "cargo_transfer_point",
        get,
    )?;

    let shared = world.clone();
    let set = lua.create_function(move |lua, arguments: MultiValue| {
        contract::check_arity(&arguments, 2)?;
        let map = map_id(lua, &shared, arg(&arguments, 0), 1)?;
        let point = match arg(&arguments, 1) {
            Value::Nil => None,
            value => {
                let value = table(value, 2, "point")?;
                contract::check_options(&value, &["x", "y", "reveal_hint"], 2)?;
                let x = contract::integer_field(&value, "x", 0, MAX_INT, 2)? as i32;
                let y = contract::integer_field(&value, "y", 0, MAX_INT, 2)? as i32;
                let reveal_hint = contract::boolean_field(&value, "reveal_hint", 2)?;
                shared.borrow().btech.maps()[&map]
                    .base_hex(i64::from(x), i64::from(y))
                    .map_err(|_| {
                        argument_failure(2, "cargo-transfer coordinates are outside the map")
                    })?;
                Some(CargoTransferPoint { x, y, reveal_hint })
            }
        };
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            set_cargo_transfer_point(&mut scripts.world_mut(), ObjectId(1), map, point).map_err(
                |error| {
                    contract::operation_failure(
                        if point.is_some() {
                            "cargo_transfer_store_failed"
                        } else {
                            "cargo_transfer_clear_failed"
                        },
                        error,
                    )
                },
            )
        })
    })?;
    contract::bind(
        lua,
        native,
        "map_contract_set_cargo_transfer_point",
        GROUP,
        "set_cargo_transfer_point",
        set,
    )
}

fn push_link(lua: &Lua, shared: &SharedWorld, link: MapLink) -> mlua::Result<Value> {
    let output = lua.create_table()?;
    output.raw_set("parent", contract::push_object(lua, shared, link.parent)?)?;
    output.raw_set("x", link.coordinate.x)?;
    output.raw_set("y", link.coordinate.y)?;
    let entrances = lua.create_table()?;
    for (index, name) in ["north", "east", "south", "west"].into_iter().enumerate() {
        let value = match link.entrances[index] {
            MapEntrance::None => continue,
            MapEntrance::Offset { distance } => {
                let value = lua.create_table()?;
                value.raw_set("mode", "offset")?;
                value.raw_set("offset", distance)?;
                value
            }
            MapEntrance::Exact { coordinate } => {
                let value = lua.create_table()?;
                value.raw_set("mode", "exact")?;
                value.raw_set("x", coordinate.x)?;
                value.raw_set("y", coordinate.y)?;
                value
            }
        };
        entrances.raw_set(name, value)?;
    }
    output.raw_set("entrances", entrances)?;
    Ok(Value::Table(output))
}

fn parse_entrance(
    shared: &SharedWorld,
    child: ObjectId,
    value: Value,
    direction: &str,
) -> mlua::Result<MapEntrance> {
    if value == Value::Nil {
        return Ok(MapEntrance::None);
    }
    let value = table(value, 2, direction)?;
    let mode = contract::string(contract::field(&value, "mode")?, "mode", 16, 2)
        .map_err(|_| argument_failure(2, "mode must be 'offset' or 'exact'"))?;
    match mode.as_str() {
        "offset" => {
            contract::check_options(&value, &["mode", "offset"], 2)?;
            Ok(MapEntrance::Offset {
                distance: contract::integer_field(&value, "offset", 0, MAX_INT, 2)? as i32,
            })
        }
        "exact" => {
            contract::check_options(&value, &["mode", "x", "y"], 2)?;
            let x = contract::integer_field(&value, "x", 0, MAX_INT, 2)?;
            let y = contract::integer_field(&value, "y", 0, MAX_INT, 2)?;
            shared.borrow().btech.maps()[&child]
                .base_hex(x, y)
                .map_err(|_| {
                    argument_failure(2, "exact entrance coordinates are outside the child map")
                })?;
            Ok(MapEntrance::Exact {
                coordinate: HexCoordinate {
                    x: x as i32,
                    y: y as i32,
                },
            })
        }
        _ => Err(argument_failure(2, "mode must be 'offset' or 'exact'")),
    }
}

fn register_links(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let shared = world.clone();
    let get = lua.create_function(move |lua, arguments: MultiValue| {
        contract::check_arity(&arguments, 1)?;
        let child = map_id(lua, &shared, arg(&arguments, 0), 1)?;
        let Some(link) = shared.borrow().btech.maps()[&child].authored_link() else {
            return Ok(Value::Nil);
        };
        if shared
            .borrow()
            .objects
            .get(&link.parent)
            .is_some_and(|object| object.flags.contains(crate::Flag::Going))
        {
            return Ok(Value::Nil);
        }
        push_link(lua, &shared, link)
    })?;
    contract::bind(lua, native, "map_contract_link", GROUP, "link", get)?;

    let shared = world.clone();
    let set = lua.create_function(move |lua, arguments: MultiValue| {
        contract::check_arity(&arguments, 2)?;
        let child = map_id(lua, &shared, arg(&arguments, 0), 1)?;
        let link = match arg(&arguments, 1) {
            Value::Nil => None,
            value => {
                let value = table(value, 2, "link")?;
                contract::check_options(&value, &["parent", "x", "y", "entrances"], 2)?;
                let parent =
                    contract::require_object_field(lua, &shared.borrow(), &value, "parent", 2)?;
                if !shared.borrow().btech.maps().contains_key(&parent) {
                    return Err(argument_failure(2, "parent is not a registered BTech map"));
                }
                if child == parent {
                    return Err(argument_failure(2, "a map cannot be linked to itself"));
                }
                let x = contract::integer_field(&value, "x", 0, MAX_INT, 2)?;
                let y = contract::integer_field(&value, "y", 0, MAX_INT, 2)?;
                shared.borrow().btech.maps()[&parent]
                    .base_hex(x, y)
                    .map_err(|_| {
                        argument_failure(2, "parent coordinates are outside the parent map")
                    })?;
                let entrances_value = contract::field(&value, "entrances")?;
                let mut entrances = [MapEntrance::None; 4];
                if entrances_value != Value::Nil {
                    let source = table(entrances_value, 2, "entrances")?;
                    contract::check_options(&source, &["north", "east", "south", "west"], 2)?;
                    for (index, direction) in
                        ["north", "east", "south", "west"].into_iter().enumerate()
                    {
                        entrances[index] = parse_entrance(
                            &shared,
                            child,
                            contract::field(&source, direction)?,
                            direction,
                        )?;
                    }
                }
                Some(MapLink {
                    parent,
                    coordinate: HexCoordinate {
                        x: x as i32,
                        y: y as i32,
                    },
                    entrances,
                })
            }
        };
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        crate::lua::transactions::run(lua, &scripts.world, || {
            set_map_link(&mut scripts.world_mut(), child, link)
                .map_err(|error| contract::operation_failure("map_link_store_failed", error))
        })
    })?;
    contract::bind(lua, native, "map_contract_set_link", GROUP, "set_link", set)
}

fn register_los(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let shared = world.clone();
    let los = lua.create_function(move |lua, arguments: MultiValue| {
        let observer = unit_id(lua, &shared, arg(&arguments, 0), 1, "observer")?;
        let target_value = arg(&arguments, 1);
        let result = if matches!(&target_value, Value::Table(_)) {
            let observer_map = map_unit_map(&shared.borrow(), observer)
                .map_err(|error| contract::operation_failure("observer_not_on_map", error))?;
            let (target, _) = coordinate(&shared, observer_map, target_value, 2, false)?;
            map_hex_los(&shared.borrow(), observer, target).map_err(mlua::Error::external)?
        } else {
            let target = unit_id(lua, &shared, target_value, 2, "target")?;
            map_unit_los(&shared.borrow(), observer, target).map_err(mlua::Error::external)?
        };
        Ok(match result {
            MapLos::Clear => "clear",
            MapLos::Blocked => "blocked",
            MapLos::None => "none",
        })
    })?;
    contract::bind(
        lua,
        native,
        "map_contract_line_of_sight",
        GROUP,
        "line_of_sight",
        los,
    )
}
