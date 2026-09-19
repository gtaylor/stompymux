//! C-compatible `btech.character` bindings.

use super::*;

fn arg(arguments: &mlua::MultiValue, index: usize) -> Value {
    arguments.get(index).cloned().unwrap_or(Value::Nil)
}

fn definition(
    _lua: &Lua,
    value: Value,
    argument: usize,
) -> mlua::Result<crate::btech::CharacterValueDefinition> {
    let found = match value {
        Value::String(value) => std::str::from_utf8(
            value
                .as_bytes()
                .split(|byte| *byte == 0)
                .next()
                .unwrap_or_default(),
        )
        .ok()
        .and_then(crate::btech::character_value_definition),
        value => {
            let code = ranged_integer(value, "value", argument)?;
            usize::try_from(code)
                .ok()
                .and_then(crate::btech::character_value_definition_code)
        }
    };
    found.ok_or_else(|| {
        error::failure_with_detail(
            "mux.arg.invalid",
            "unknown character value",
            serde_json::json!({"argument":argument}),
        )
    })
}

fn ranged_integer(value: Value, label: &str, argument: usize) -> mlua::Result<i64> {
    let value = match value {
        Value::Integer(value) if (i32::MIN as i64..=i32::MAX as i64).contains(&value) => {
            Some(value)
        }
        Value::Number(value)
            if value.is_finite()
                && value.fract() == 0.0
                && value >= i32::MIN as f64
                && value <= i32::MAX as f64 =>
        {
            Some(value as i64)
        }
        Value::Integer(_) | Value::Number(_) => None,
        _ => {
            return Err(error::failure_with_detail(
                "mux.arg.invalid",
                format!("{label} must be an integer"),
                serde_json::json!({"argument":argument}),
            ));
        }
    };
    value.ok_or_else(|| {
        error::failure_with_detail(
            "mux.arg.invalid",
            format!("{label} must be a ranged integer"),
            serde_json::json!({"argument":argument}),
        )
    })
}

fn character(
    lua: &Lua,
    world: &crate::World,
    value: Value,
    argument: usize,
) -> mlua::Result<ObjectId> {
    let Value::UserData(ref userdata) = value else {
        return Err(error::failure_with_detail(
            "mux.object.invalid",
            "character must be an Object",
            serde_json::json!({"argument":argument}),
        ));
    };
    if !userdata.is::<crate::lua::packages::world::handles::Object>() {
        return Err(error::failure_with_detail(
            "mux.object.invalid",
            "character must be an Object",
            serde_json::json!({"argument":argument}),
        ));
    }
    if userdata
        .borrow::<crate::lua::packages::world::handles::Object>()?
        .is_stale_in(world)
    {
        return Err(error::failure_with_detail(
            "mux.object.unavailable",
            "object no longer exists",
            serde_json::json!({"argument":argument}),
        ));
    }
    let id = contract::require_object(lua, world, value, argument)?;
    if !world
        .objects
        .get(&id)
        .is_some_and(|object| object.kind == crate::Kind::Player)
    {
        return Err(error::failure_with_detail(
            "mux.object.invalid",
            "character must be a player",
            serde_json::json!({"argument":argument}),
        ));
    }
    Ok(id)
}

fn push_definition(
    lua: &Lua,
    definition: crate::btech::CharacterValueDefinition,
) -> mlua::Result<Table> {
    let result = lua.create_table()?;
    result.raw_set("code", definition.code)?;
    result.raw_set("name", definition.name)?;
    result.raw_set("kind", definition.kind)?;
    result.raw_set(
        "default_experience_threshold",
        definition.default_experience_threshold,
    )?;
    Ok(result)
}

pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "character_catalog_contract",
        "character",
        "catalog",
        lua.create_function(move |lua, arguments: mlua::MultiValue| {
            let Value::String(kind) = arg(&arguments, 0) else {
                return Err(error::failure_with_detail(
                    "mux.arg.invalid",
                    "kind must be a string",
                    serde_json::json!({"argument":1}),
                ));
            };
            let bytes = kind.as_bytes();
            let kind =
                std::str::from_utf8(bytes.split(|byte| *byte == 0).next().unwrap_or_default())
                    .unwrap_or("");
            let kind = kind.to_ascii_lowercase();
            let world = shared.borrow();
            let player = arguments
                .get(1)
                .filter(|value| !value.is_nil())
                .cloned()
                .map(|value| character(lua, &world, value, 2))
                .transpose()?;
            let result = lua.create_table()?;
            let mut output = 1;
            for entry in crate::btech::character_value_definitions() {
                if !entry.kind.eq_ignore_ascii_case(&kind) {
                    continue;
                }
                if let Some(player) = player
                    && entry.kind != "Char_attribute"
                {
                    let saved = crate::btech::character_saved_value(&world, player, entry);
                    if crate::btech::character_raw_value(&world, player, entry) == 0
                        && saved.experience == 0
                    {
                        continue;
                    }
                }
                result.raw_set(output, push_definition(lua, entry)?)?;
                output += 1;
            }
            Ok(result)
        })?,
    )?;

    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "character_value_contract",
        "character",
        "value",
        lua.create_function(move |lua, arguments: mlua::MultiValue| {
            let world = shared.borrow();
            let player = character(lua, &world, arg(&arguments, 0), 1)?;
            let definition = definition(lua, arg(&arguments, 1), 2)?;
            let result = lua.create_table()?;
            result.raw_set("definition", push_definition(lua, definition)?)?;
            result.raw_set(
                "amount",
                crate::btech::character_raw_value(&world, player, definition),
            )?;
            if definition.kind == "Char_skill" {
                let saved = crate::btech::character_saved_value(&world, player, definition);
                let progress = crate::battle_skill_progress(&world, player, definition.name)
                    .map_err(|e| error::failure("btech.operation.failed", e))?;
                result.raw_set("target", progress.target)?;
                result.raw_set("experience", saved.experience)?;
                result.raw_set(
                    "experience_to_next_level",
                    progress
                        .next_level_balance
                        .map(|n| n.saturating_add(1).min(i64::MAX as u64) as i64)
                        .unwrap_or(-1),
                )?;
            }
            Ok(result)
        })?,
    )?;

    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "character_threshold_contract",
        "character",
        "experience_threshold",
        lua.create_function(move |_, value: Value| {
            let Value::String(value) = value else {
                return Err(error::failure_with_detail(
                    "mux.arg.invalid",
                    "skill must be a string",
                    serde_json::json!({"argument":1}),
                ));
            };
            let bytes = value.as_bytes();
            let name =
                std::str::from_utf8(bytes.split(|byte| *byte == 0).next().unwrap_or_default())
                    .unwrap_or("");
            crate::battle_skill_threshold(&shared.borrow(), name)
                .map(i64::from)
                .map_err(|_| {
                    error::failure_with_detail(
                        "mux.arg.invalid",
                        "unknown skill",
                        serde_json::json!({"argument":1}),
                    )
                })
        })?,
    )?;

    for (key, name, mode) in [
        ("character_set_value_contract", "set_value", 0),
        ("character_set_target_contract", "set_skill_target", 1),
        (
            "character_set_experience_contract",
            "set_skill_experience",
            2,
        ),
        (
            "character_add_experience_contract",
            "add_skill_experience",
            3,
        ),
    ] {
        contract::bind(
            lua,
            native,
            key,
            "character",
            name,
            lua.create_function(move |lua, arguments: mlua::MultiValue| {
                let scripts = crate::Scripts::services(lua)?;
                let mut world = scripts.world_mut();
                let player = character(lua, &world, arg(&arguments, 0), 1)?;
                let definition = definition(lua, arg(&arguments, 1), 2)?;
                let amount = ranged_integer(
                    arg(&arguments, 2),
                    if mode == 1 {
                        "target"
                    } else if mode == 2 {
                        "experience"
                    } else {
                        "amount"
                    },
                    3,
                )? as i32;
                match mode {
                    0 => crate::btech::set_character_raw_value(
                        &mut world, player, definition, amount,
                    )
                    .map_err(mlua::Error::external)?,
                    1 => {
                        if definition.kind != "Char_skill" {
                            return Err(error::failure_with_detail(
                                "mux.arg.invalid",
                                "value is not a skill",
                                serde_json::json!({"argument":2}),
                            ));
                        }
                        crate::btech::set_character_skill_target(
                            &mut world, player, definition, amount,
                        )
                        .map_err(|_| {
                            error::failure_with_detail(
                                "mux.arg.invalid",
                                "target is unreachable for this skill",
                                serde_json::json!({"argument":3}),
                            )
                        })?
                    }
                    2 => {
                        if amount < 0 {
                            return Err(error::failure_with_detail(
                                "mux.arg.invalid",
                                "experience must be nonnegative",
                                serde_json::json!({"argument":3}),
                            ));
                        }
                        crate::btech::set_character_skill_experience(
                            &mut world,
                            player,
                            definition,
                            amount as u32,
                        )
                        .map_err(mlua::Error::external)?
                    }
                    _ => {
                        if definition.kind != "Char_skill" || amount < 0 {
                            return Err(contract::operation_failure(
                                "experience_adjustment_failed",
                                "experience adjustment failed",
                            ));
                        }
                        let award = crate::award_battle_skill_experience(
                            &mut world,
                            player,
                            definition.name,
                            amount as u32,
                            crate::clock::wall_time(),
                            true,
                        )
                        .map_err(|_| {
                            contract::operation_failure(
                                "experience_adjustment_failed",
                                "experience adjustment failed",
                            )
                        })?;
                        if !award.accepted {
                            return Err(contract::operation_failure(
                                "experience_adjustment_failed",
                                "experience adjustment failed",
                            ));
                        }
                    }
                }
                Ok(mlua::MultiValue::new())
            })?,
        )?;
    }
    Ok(())
}
