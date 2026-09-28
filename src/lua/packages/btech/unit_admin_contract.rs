//! C-compatible scalar and identity administration for supported live chassis.
use super::*;

fn unit(lua: &Lua, world: &crate::World, value: Value) -> mlua::Result<ObjectId> {
    contract::require_special(lua, world, value, 1, "MECH", "unit")
}
fn admit(lua: &Lua, args: &mlua::MultiValue) -> mlua::Result<()> {
    let scripts = crate::Scripts::services(lua)?;
    unit(
        lua,
        &scripts.world(),
        args.front().cloned().unwrap_or(Value::Nil),
    )?;
    Ok(())
}
fn admit_registered(lua: &Lua, args: &mlua::MultiValue) -> mlua::Result<()> {
    let scripts = crate::Scripts::services(lua)?;
    unit(
        lua,
        &scripts.world(),
        args.front().cloned().unwrap_or(Value::Nil),
    )?;
    Ok(())
}
fn zero(result: anyhow::Result<()>) -> mlua::Result<mlua::MultiValue> {
    result.map_err(|e| contract::operation_failure("unit_admin_update_failed", e))?;
    Ok(mlua::MultiValue::new())
}
fn number(value: Value, label: &str, min: f64, max: f64, argument: usize) -> mlua::Result<f64> {
    let number = match value {
        Value::Integer(value) => value as f64,
        Value::Number(value) => value,
        _ => {
            return Err(error::failure_with_detail(
                "mux.arg.invalid",
                format!("{label} must be a number"),
                serde_json::json!({"argument":argument}),
            ));
        }
    };
    if !number.is_finite() || !(min..=max).contains(&number) {
        return Err(error::failure_with_detail(
            "mux.arg.invalid",
            format!("{label} is outside its valid range"),
            serde_json::json!({"argument":argument}),
        ));
    }
    Ok(number)
}
fn integer(value: Value, label: &str, min: i64, max: i64, argument: usize) -> mlua::Result<i64> {
    let number = match value {
        Value::Integer(value) => value as f64,
        Value::Number(value) => value,
        _ => {
            return Err(error::failure_with_detail(
                "mux.arg.invalid",
                format!("{label} must be an integer"),
                serde_json::json!({"argument":argument}),
            ));
        }
    };
    if !number.is_finite() || number.fract() != 0.0 || number < min as f64 || number > max as f64 {
        return Err(error::failure_with_detail(
            "mux.arg.invalid",
            format!("{label} is outside its valid range"),
            serde_json::json!({"argument":argument}),
        ));
    }
    Ok(number as i64)
}
/// Mirror of the native optional_string acceptance rule: nil clears, anything
/// else must be a string of 1 to `maximum` bytes.
fn optional_string(
    value: Value,
    label: &str,
    maximum: usize,
    argument: usize,
) -> mlua::Result<Option<String>> {
    if value.is_nil() {
        return Ok(None);
    }
    let Value::String(value) = value else {
        return Err(error::failure_with_detail(
            "mux.arg.invalid",
            format!("{label} must be a string or nil"),
            serde_json::json!({"argument":argument}),
        ));
    };
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(error::failure_with_detail(
            "mux.arg.invalid",
            format!("{label} must contain 1 to {maximum} bytes"),
            serde_json::json!({"argument":argument}),
        ));
    }
    value
        .to_str()
        .map(|value| Some(value.to_owned()))
        .map_err(|_| {
            error::failure_with_detail(
                "mux.arg.invalid",
                format!("{label} must be valid UTF-8"),
                serde_json::json!({"argument":argument}),
            )
        })
}
fn patch_integer(table: &Table, name: &str) -> mlua::Result<Option<u16>> {
    let value: Value = table.get(name)?;
    if value.is_nil() {
        return Ok(None);
    }
    match value {
        Value::Integer(value) if (0..=255).contains(&value) => Ok(Some(value as u16)),
        Value::Number(value)
            if value.is_finite() && value.fract() == 0.0 && (0.0..=255.0).contains(&value) =>
        {
            Ok(Some(value as u16))
        }
        Value::Integer(_) | Value::Number(_) => Err(error::failure_with_detail(
            "mux.arg.invalid",
            format!("{name} must be an integer from 0 through 255"),
            serde_json::json!({"argument":3}),
        )),
        _ => Err(error::failure_with_detail(
            "mux.arg.invalid",
            format!("{name} must be an integer"),
            serde_json::json!({"argument":3}),
        )),
    }
}

pub(super) fn register(lua: &Lua, native: &Table, _world: &SharedWorld) -> mlua::Result<()> {
    // The canonical C contracts below replace three older Rust extensions. Keep the
    // actor-aware forms under descriptive names so existing scripts retain their
    // transaction, notification, and return-value behavior.
    for (native_key, public_name, old_key) in [
        (
            "unit_set_markings_as_extension",
            "set_markings_as",
            "unit_set_markings",
        ),
        (
            "unit_set_display_name_as_extension",
            "set_display_name_as",
            "unit_set_display_name",
        ),
        (
            "unit_set_preferred_id_as_extension",
            "set_preferred_id_as",
            "unit_set_preferred_id",
        ),
    ] {
        let old: mlua::Function = native.raw_get(old_key)?;
        contract::bind(
            lua,
            native,
            native_key,
            "unit",
            public_name,
            lua.create_function(move |_, args: mlua::MultiValue| {
                old.call::<mlua::MultiValue>(args)
            })?,
        )?;
    }
    for (key, public, label, min, max) in [
        (
            "unit_set_max_speed_contract",
            "set_max_speed",
            "maxspeed",
            0.0,
            10000.0,
        ),
        (
            "unit_set_jump_speed_contract",
            "set_jump_speed",
            "maxjumpspeed",
            0.0,
            10000.0,
        ),
    ] {
        contract::bind(
            lua,
            native,
            key,
            "unit",
            public,
            lua.create_function(move |lua, args: mlua::MultiValue| {
                admit(lua, &args)?;
                let value = number(
                    args.get(1).cloned().unwrap_or(Value::Nil),
                    "speed",
                    min,
                    max,
                    2,
                )?;
                let scripts = crate::Scripts::services(lua)?;
                let id = unit(lua, &scripts.world(), args[0].clone())?;
                zero(crate::btech::set_administrative_scalar(
                    &mut scripts.world_mut(),
                    id,
                    label,
                    value,
                ))
            })?,
        )?;
    }
    for (key, public, label, value_label, min, max) in [
        (
            "unit_set_lrs_contract",
            "set_long_range_sensor_range",
            "lrsrange",
            "range",
            0,
            127,
        ),
        (
            "unit_set_tactical_contract",
            "set_tactical_range",
            "tacrange",
            "range",
            0,
            127,
        ),
        (
            "unit_set_scan_contract",
            "set_scan_range",
            "scanrange",
            "range",
            0,
            127,
        ),
        (
            "unit_set_radio_range_contract",
            "set_radio_range",
            "radiorange",
            "range",
            0,
            32767,
        ),
    ] {
        contract::bind(
            lua,
            native,
            key,
            "unit",
            public,
            lua.create_function(move |lua, args: mlua::MultiValue| {
                admit(lua, &args)?;
                let value = integer(
                    args.get(1).cloned().unwrap_or(Value::Nil),
                    value_label,
                    min,
                    max,
                    2,
                )?;
                let scripts = crate::Scripts::services(lua)?;
                let id = unit(lua, &scripts.world(), args[0].clone())?;
                zero(crate::btech::set_administrative_scalar(
                    &mut scripts.world_mut(),
                    id,
                    label,
                    value as f64,
                ))
            })?,
        )?;
    }
    contract::bind(
        lua,
        native,
        "unit_set_tonnage_contract",
        "unit",
        "set_tonnage",
        lua.create_function(|lua, args: mlua::MultiValue| {
            admit(lua, &args)?;
            let tons = number(
                args.get(1).cloned().unwrap_or(Value::Nil),
                "tons",
                1.0,
                (i32::MAX / 1024) as f64,
                2,
            )?;
            if tons.fract() != 0.0 {
                return Err(error::failure_with_detail(
                    "mux.arg.invalid",
                    "tons must be an integer",
                    serde_json::json!({"argument":2}),
                ));
            }
            let scripts = crate::Scripts::services(lua)?;
            let id = unit(lua, &scripts.world(), args[0].clone())?;
            zero(crate::btech::set_administrative_scalar(
                &mut scripts.world_mut(),
                id,
                "tons",
                tons,
            ))
        })?,
    )?;
    contract::bind(
        lua,
        native,
        "unit_set_radio_quality_contract",
        "unit",
        "set_radio_quality",
        lua.create_function(|lua, args: mlua::MultiValue| {
            admit(lua, &args)?;
            let quality = integer(
                args.get(1).cloned().unwrap_or(Value::Nil),
                "quality",
                1,
                5,
                2,
            )? as u8;
            let scripts = crate::Scripts::services(lua)?;
            let id = unit(lua, &scripts.world(), args[0].clone())?;
            zero(crate::btech::set_administrative_radio_quality(
                &mut scripts.world_mut(),
                id,
                quality,
            ))
        })?,
    )?;
    contract::bind(
        lua,
        native,
        "unit_set_heat_sinks_contract",
        "unit",
        "set_heat_sinks",
        lua.create_function(|lua, args: mlua::MultiValue| {
            admit(lua, &args)?;
            let count = integer(
                args.get(1).cloned().unwrap_or(Value::Nil),
                "count",
                0,
                127,
                2,
            )? as u16;
            let scripts = crate::Scripts::services(lua)?;
            let id = unit(
                lua,
                &scripts.world(),
                args.front().cloned().unwrap_or(Value::Nil),
            )?;
            zero(crate::btech::set_administrative_heat_sinks(
                &mut scripts.world_mut(),
                id,
                count,
            ))
        })?,
    )?;
    for (key, public, enabled) in [
        ("unit_add_technology_contract", "add_technology", true),
        (
            "unit_remove_technology_contract",
            "remove_technology",
            false,
        ),
    ] {
        contract::bind(
            lua,
            native,
            key,
            "unit",
            public,
            lua.create_function(move |lua, args: mlua::MultiValue| {
                admit(lua, &args)?;
                let code = constants::require(
                    args.get(1).cloned().unwrap_or(Value::Nil),
                    2,
                    "technology",
                    &constants::TECHNOLOGY,
                )?;
                let scripts = crate::Scripts::services(lua)?;
                let id = unit(
                    lua,
                    &scripts.world(),
                    args.front().cloned().unwrap_or(Value::Nil),
                )?;
                if code >= 57
                    && crate::btech::administrative_unit_class(&scripts.world(), id).as_deref()
                        != Some("Battlesuit")
                {
                    return Err(error::failure_with_detail(
                        "mux.object.invalid",
                        "infantry technology requires a battlesuit",
                        serde_json::json!({"argument":1}),
                    ));
                }
                zero(crate::btech::set_administrative_technology(
                    &mut scripts.world_mut(),
                    id,
                    code,
                    enabled,
                ))
            })?,
        )?;
    }
    contract::bind(
        lua,
        native,
        "unit_clear_technologies_contract",
        "unit",
        "clear_technologies",
        lua.create_function(|lua, args: mlua::MultiValue| {
            admit(lua, &args)?;
            let group = constants::require(
                args.get(1).cloned().unwrap_or(Value::Nil),
                2,
                "group",
                &constants::TECHNOLOGY_GROUPS,
            )?;
            let scripts = crate::Scripts::services(lua)?;
            let id = unit(
                lua,
                &scripts.world(),
                args.front().cloned().unwrap_or(Value::Nil),
            )?;
            zero(crate::btech::clear_administrative_technologies(
                &mut scripts.world_mut(),
                id,
                group,
            ))
        })?,
    )?;
    contract::bind(
        lua,
        native,
        "unit_set_type_contract",
        "unit",
        "set_unit_type",
        lua.create_function(|lua, args: mlua::MultiValue| {
            admit(lua, &args)?;
            let kind = constants::require(
                args.get(1).cloned().unwrap_or(Value::Nil),
                2,
                "unit_type",
                &constants::UNIT_TYPES,
            )?;
            let scripts = crate::Scripts::services(lua)?;
            let id = unit(
                lua,
                &scripts.world(),
                args.front().cloned().unwrap_or(Value::Nil),
            )?;
            zero(crate::btech::set_administrative_unit_type(
                &mut scripts.world_mut(),
                id,
                kind,
            ))
        })?,
    )?;
    contract::bind(
        lua,
        native,
        "unit_set_armor_contract",
        "unit",
        "set_armor",
        lua.create_function(|lua, args: mlua::MultiValue| {
            admit(lua, &args)?;
            let scripts = crate::Scripts::services(lua)?;
            let id = unit(
                lua,
                &scripts.world(),
                args.front().cloned().unwrap_or(Value::Nil),
            )?;
            let section_value = args.get(1).cloned().unwrap_or(Value::Nil);
            if section_value.is_nil() {
                return Err(error::failure_with_detail(
                    "mux.arg.invalid",
                    "section is required and must exist on the unit",
                    serde_json::json!({"argument":2}),
                ));
            }
            let section = constants::require(section_value, 2, "section", &constants::SECTIONS)?;
            if !crate::btech::administrative_section_valid(&scripts.world(), id, section) {
                return Err(error::failure_with_detail(
                    "mux.arg.invalid",
                    "section is not valid for this unit",
                    serde_json::json!({"argument":2}),
                ));
            }
            let Value::Table(patch) = args.get(2).cloned().unwrap_or(Value::Nil) else {
                return Err(error::failure_with_detail(
                    "mux.arg.invalid",
                    "value must be a table",
                    serde_json::json!({"argument":3}),
                ));
            };
            contract::check_options(&patch, &["armor", "internal", "rear_armor"], 3)?;
            let armor = patch_integer(&patch, "armor")?;
            let internal = patch_integer(&patch, "internal")?;
            let rear = patch_integer(&patch, "rear_armor")?;
            if armor.is_none() && internal.is_none() && rear.is_none() {
                return Err(error::failure_with_detail(
                    "mux.arg.invalid",
                    "patch must contain at least one field",
                    serde_json::json!({"argument":3}),
                ));
            }
            zero(crate::btech::set_administrative_armor(
                &mut scripts.world_mut(),
                id,
                section,
                armor,
                internal,
                rear,
            ))
        })?,
    )?;
    contract::bind(
        lua,
        native,
        "unit_set_cargo_contract",
        "unit",
        "set_cargo_capacity",
        lua.create_function(|lua, args: mlua::MultiValue| {
            admit(lua, &args)?;
            let space = integer(
                args.get(1).cloned().unwrap_or(Value::Nil),
                "space",
                0,
                5000,
                2,
            )?;
            let maximum = integer(
                args.get(2).cloned().unwrap_or(Value::Nil),
                "maximum_tons",
                1,
                100,
                3,
            )?;
            let scripts = crate::Scripts::services(lua)?;
            let id = unit(lua, &scripts.world(), args[0].clone())?;
            zero(crate::btech::set_administrative_cargo(
                &mut scripts.world_mut(),
                id,
                space as u32,
                maximum as u8,
            ))
        })?,
    )?;
    contract::bind(
        lua,
        native,
        "unit_set_movement_contract",
        "unit",
        "set_movement_type",
        lua.create_function(|lua, args: mlua::MultiValue| {
            admit(lua, &args)?;
            let code = constants::require(
                args.get(1).cloned().unwrap_or(Value::Nil),
                2,
                "movement_type",
                &constants::MOVEMENT_TYPES,
            )?;
            let scripts = crate::Scripts::services(lua)?;
            let id = unit(lua, &scripts.world(), args[0].clone())?;
            zero(crate::btech::set_administrative_movement_type(
                &mut scripts.world_mut(),
                id,
                code,
            ))
        })?,
    )?;
    for (key, public, label, field_name, maxlen) in [
        (
            "unit_set_display_contract",
            "set_display_name",
            "name",
            "display_name",
            16383usize,
        ),
        (
            "unit_set_markings_contract",
            "set_markings",
            "markings",
            "markings",
            16383usize,
        ),
    ] {
        contract::bind(
            lua,
            native,
            key,
            "unit",
            public,
            lua.create_function(move |lua, args: mlua::MultiValue| {
                contract::check_arity(&args, 2)?;
                admit_registered(lua, &args)?;
                let value = optional_string(args[1].clone(), label, maxlen, 2)?;
                if field_name == "display_name"
                    && value.as_ref().is_some_and(|value| value.len() > 120)
                {
                    return Err(contract::operation_failure(
                        "display_name_store_failed",
                        "unable to store unit display name",
                    ));
                }
                let scripts = crate::Scripts::services(lua)?;
                let id = unit(lua, &scripts.world(), args[0].clone())?;
                crate::btech::set_unit_identity_configuration(
                    &mut scripts.world_mut(),
                    id,
                    field_name,
                    value,
                );
                Ok(mlua::MultiValue::new())
            })?,
        )?;
    }
    contract::bind(
        lua,
        native,
        "unit_set_preferred_contract",
        "unit",
        "set_preferred_id",
        lua.create_function(|lua, args: mlua::MultiValue| {
            contract::check_arity(&args, 2)?;
            admit_registered(lua, &args)?;
            let value = optional_string(args[1].clone(), "preferred_id", 2, 2)?;
            if value.as_ref().is_some_and(|value| {
                value.len() != 2 || !value.bytes().all(|byte| byte.is_ascii_alphabetic())
            }) {
                return Err(error::failure_with_detail(
                    "mux.arg.invalid",
                    "preferred_id must contain exactly two ASCII letters",
                    serde_json::json!({"argument":2}),
                ));
            }
            let scripts = crate::Scripts::services(lua)?;
            let id = unit(lua, &scripts.world(), args[0].clone())?;
            crate::btech::set_unit_identity_configuration(
                &mut scripts.world_mut(),
                id,
                "preferred_id",
                value,
            );
            Ok(mlua::MultiValue::new())
        })?,
    )?;
    contract::bind(
        lua,
        native,
        "unit_set_pilot_contract",
        "unit",
        "set_assigned_pilot",
        lua.create_function(|lua, args: mlua::MultiValue| {
            contract::check_arity(&args, 2)?;
            admit_registered(lua, &args)?;
            let scripts = crate::Scripts::services(lua)?;
            let id = unit(lua, &scripts.world(), args[0].clone())?;
            let pilot = if args[1].is_nil() {
                None
            } else {
                let world = scripts.world();
                let p = contract::require_object(lua, &world, args[1].clone(), 2)?;
                if world.objects[&p].kind != crate::Kind::Player {
                    return Err(error::failure_with_detail(
                        "mux.object.invalid",
                        "assigned pilot must be a player",
                        serde_json::json!({"argument":2}),
                    ));
                }
                Some(p)
            };
            zero(crate::btech::set_administrative_assigned_pilot(
                &mut scripts.world_mut(),
                id,
                pilot,
            ))
        })?,
    )?;
    contract::bind(
        lua,
        native,
        "unit_unregister_contract",
        "unit",
        "unregister",
        lua.create_function(|lua, args: mlua::MultiValue| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let id = contract::require_object(
                lua,
                &scripts.world(),
                args.front().cloned().unwrap_or(Value::Nil),
                1,
            )?;
            if scripts.world().objects[&id].kind != crate::Kind::Thing {
                return Err(error::failure_with_detail(
                    "mux.object.invalid",
                    "object must be a live thing",
                    serde_json::json!({"argument":1}),
                ));
            }
            // Rust extension without a C Lua counterpart: the reference exposes
            // teardown only through the native @btech/unregister command, so this
            // trusted host API reuses the command's shared teardown helper under
            // host authority (#1) instead of restating the wizard gate.
            let config = crate::lua::configuration(lua);
            crate::btech::unregister_special(&scripts, &config, crate::ObjectId(1), id)
                .map_err(|e| contract::operation_failure("unit_unregister_failed", e))?;
            Ok(true)
        })?,
    )?;
    Ok(())
}
