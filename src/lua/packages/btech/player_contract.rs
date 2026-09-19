//! C-compatible player configuration bindings.

use super::*;
use std::sync::Arc;

fn argument_failure(argument: usize, code: &'static str, message: &str) -> mlua::Error {
    error::failure_with_detail(code, message, serde_json::json!({"argument":argument}))
}

fn first(arguments: &mlua::MultiValue) -> Value {
    arguments.get(0).cloned().unwrap_or(Value::Nil)
}

fn second(arguments: &mlua::MultiValue) -> Value {
    arguments.get(1).cloned().unwrap_or(Value::Nil)
}

/// C `optional_string`: nil clears, strings are bounded, other values are rejected.
fn optional_reference(value: Value) -> mlua::Result<Option<Vec<u8>>> {
    if value.is_nil() {
        return Ok(None);
    }
    let Value::String(value) = value else {
        return Err(argument_failure(
            2,
            "mux.arg.invalid",
            "reference must be a string or nil",
        ));
    };
    let raw = value.as_bytes();
    if raw.is_empty() || raw.len() > 24 {
        return Err(argument_failure(
            2,
            "mux.arg.invalid",
            "reference must contain 1 to 24 bytes",
        ));
    }
    let end = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
    Ok(Some(raw[..end].to_vec()))
}

/// C `set_mechwarrior_template` validation: resolve the reference, then load it.
fn validate_template_reference(lua: &Lua, reference: &[u8]) -> mlua::Result<()> {
    if reference.windows(2).any(|bytes| bytes == b"..")
        || reference.contains(&b'/')
        || reference.contains(&b'\\')
    {
        return Err(argument_failure(
            2,
            "mux.arg.invalid",
            "reference must not contain path components",
        ));
    }
    let config = crate::lua::configuration(lua);
    let root = config.path(&config.database.mech_database);
    let shared = crate::Scripts::services(lua)?.world;
    let path = {
        let mut world = shared.borrow_mut();
        crate::btech::resolve_template_path_bytes_cached(
            &mut world.btech.template_registry,
            &root,
            reference,
        )
        .map_err(mlua::Error::external)?
    };
    let Some(path) = path else {
        return Err(argument_failure(
            2,
            "btech.template.not_found",
            "template was not found",
        ));
    };
    let mut template = crate::btech::read_resolved_raw_template(&root, &path)
        .map_err(|_| argument_failure(2, "btech.template.invalid", "template is malformed"))?;
    if !parts_contract::normalize_raw_template_parts(&mut template) {
        return Err(argument_failure(
            2,
            "btech.template.invalid",
            "template is malformed",
        ));
    }
    Ok(())
}

fn equipment(
    table: &Table,
    field: &str,
    catalogue: &[crate::BattlePartForm],
) -> mlua::Result<Option<crate::BattlePersonalEquipment>> {
    let value = contract::field(table, field)?;
    if value.is_nil() {
        return Ok(None);
    }
    let Value::Table(value) = value else {
        return Err(error::failure_with_detail(
            "mux.arg.invalid",
            format!("{field} must be a table or nil"),
            serde_json::json!({"argument":2}),
        ));
    };
    contract::check_options(&value, &["weapon", "ammunition"], 2)?;
    let part = parts_contract::check_part(contract::field(&value, "weapon")?, 2, catalogue)?
        .ok_or_else(|| {
            error::failure_with_detail(
                "btech.part.not_found",
                format!("{field}.weapon was not found"),
                serde_json::json!({"argument":2}),
            )
        })?;
    if !(1..=192).contains(&part.id) {
        return Err(error::failure_with_detail(
            "btech.part.wrong_kind",
            format!("{field}.weapon is not a weapon"),
            serde_json::json!({"argument":2}),
        ));
    }
    if !matches!(part.id, 6..=20 | 153 | 154) {
        return Err(error::failure_with_detail(
            "btech.part.wrong_kind",
            format!("{field}.weapon is not a personal-combat weapon"),
            serde_json::json!({"argument":2}),
        ));
    }
    let ammunition = match contract::field(&value, "ammunition")? {
        Value::Nil => None,
        Value::Integer(ammunition) if (0..=255).contains(&ammunition) => Some(ammunition as u8),
        Value::Number(ammunition)
            if {
                ammunition.is_finite()
                    && ammunition.fract() == 0.0
                    && (0.0..=255.0).contains(&ammunition)
            } =>
        {
            Some(ammunition as u8)
        }
        _ => {
            return Err(argument_failure(
                2,
                "mux.arg.invalid",
                "ammunition must be an integer from 0 to 255",
            ));
        }
    };
    if ammunition.is_some() && matches!(part.id, 153 | 154) {
        return Err(error::failure_with_detail(
            "mux.arg.invalid",
            format!("{field}.ammunition is invalid for this weapon"),
            serde_json::json!({"argument":2}),
        ));
    }
    let weapon = catalogue
        .iter()
        .find(|form| form.part_id == part.id && form.brand_id == 0)
        .ok_or_else(|| error::failure("mux.internal", "part catalogue mismatch"))?
        .very_long_name
        .clone();
    Ok(Some(crate::BattlePersonalEquipment { weapon, ammunition }))
}

fn parse_loadout(
    table: Table,
    catalogue: &[crate::BattlePartForm],
) -> mlua::Result<crate::BattlePersonalLoadout> {
    contract::check_options(&table, &["armor", "right", "left"], 2)?;
    let Value::Table(armor) = contract::field(&table, "armor")? else {
        return Err(error::failure_with_detail(
            "mux.arg.invalid",
            "armor must be a table",
            serde_json::json!({"argument":2}),
        ));
    };
    contract::check_options(&armor, &["head", "torso", "hands", "feet"], 2)?;
    Ok(crate::BattlePersonalLoadout {
        armor_head: contract::integer_field(&armor, "head", 0, 2, 2)? as u8,
        armor_torso: contract::integer_field(&armor, "torso", 0, 8, 2)? as u8,
        armor_hands: contract::integer_field(&armor, "hands", 0, 2, 2)? as u8,
        armor_feet: contract::integer_field(&armor, "feet", 0, 2, 2)? as u8,
        right: equipment(&table, "right", catalogue)?,
        left: equipment(&table, "left", catalogue)?,
    })
}

fn push_loadout(
    lua: &Lua,
    world: &crate::World,
    catalogue: &[crate::BattlePartForm],
    loadout: &crate::BattlePersonalLoadout,
) -> mlua::Result<Table> {
    let result = lua.create_table()?;
    let armor = lua.create_table()?;
    armor.raw_set("head", loadout.armor_head)?;
    armor.raw_set("torso", loadout.armor_torso)?;
    armor.raw_set("hands", loadout.armor_hands)?;
    armor.raw_set("feet", loadout.armor_feet)?;
    result.raw_set("armor", armor)?;
    for (field, equipment) in [("right", &loadout.right), ("left", &loadout.left)] {
        let Some(equipment) = equipment else { continue };
        let form = catalogue
            .iter()
            .find(|form| form.very_long_name.eq_ignore_ascii_case(&equipment.weapon))
            .ok_or_else(|| {
                error::failure(
                    "mux.internal",
                    "configured loadout contains an unknown weapon",
                )
            })?;
        let value = lua.create_table()?;
        value.raw_set(
            "weapon",
            parts_contract::push_part(
                lua,
                world,
                catalogue,
                parts_contract::PartReference {
                    id: form.part_id,
                    brand: 0,
                },
            )?,
        )?;
        if let Some(ammunition) = equipment.ammunition {
            value.raw_set("ammunition", ammunition)?
        }
        result.raw_set(field, value)?;
    }
    Ok(result)
}

fn player(
    lua: &Lua,
    world: &crate::World,
    value: Value,
    argument: usize,
) -> mlua::Result<ObjectId> {
    let id = contract::require_object(lua, world, value, argument)?;
    if !world
        .objects
        .get(&id)
        .is_some_and(|object| object.kind == crate::Kind::Player)
    {
        return Err(error::failure_with_detail(
            "mux.object.invalid",
            "object is not a live player",
            serde_json::json!({"argument":argument}),
        ));
    }
    Ok(id)
}

fn preferences_table(
    lua: &Lua,
    preferences: crate::BattlePlayerPreferences,
    configured: bool,
) -> mlua::Result<Table> {
    let result = lua.create_table()?;
    let d = preferences.dimensions;
    let c = preferences.contacts;
    result.raw_set("tactical_height", d.tactical_height)?;
    result.raw_set("tactical_width", d.tactical_width)?;
    result.raw_set("lrs_height", d.long_range_height)?;
    result.raw_set("include_dead", c.include_dead)?;
    result.raw_set("include_shutdown", c.include_shutdown)?;
    result.raw_set("include_enemies", c.include_enemies)?;
    result.raw_set("include_allies", c.include_allies)?;
    result.raw_set("include_target", c.include_target)?;
    result.raw_set(
        "buildings",
        match c.buildings {
            crate::BattleBuildingContactMode::FollowBrief => "follow_brief",
            crate::BattleBuildingContactMode::Include => "include",
            crate::BattleBuildingContactMode::Exclude => "exclude",
        },
    )?;
    result.raw_set("configured", configured)?;
    Ok(result)
}

pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    // C resolves loadout weapons through the manufacturer-qualified part
    // registry, where unbranded personal-combat names do not resolve.
    let catalogue = Arc::new(parts_contract::registered_catalogue());
    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "player_ui_preferences_contract",
        "player",
        "ui_preferences",
        lua.create_function(move |lua, args: mlua::MultiValue| {
            contract::check_arity(&args, 1)?;
            let world = shared.borrow();
            let player = player(lua, &world, first(&args), 1)?;
            let configured = world.btech.player_preferences.contains_key(&player);
            let prefs = world
                .btech
                .player_preferences
                .get(&player)
                .copied()
                .unwrap_or_default();
            preferences_table(lua, prefs, configured)
        })?,
    )?;
    contract::bind(
        lua,
        native,
        "player_set_ui_preferences_contract",
        "player",
        "set_ui_preferences",
        lua.create_function(|lua, args: mlua::MultiValue| {
            contract::check_arity(&args, 2)?;
            let scripts = crate::Scripts::services(lua)?;
            let mut world = scripts.world_mut();
            let player = player(lua, &world, first(&args), 1)?;
            if second(&args).is_nil() {
                Arc::make_mut(&mut world.btech.player_preferences).remove(&player);
                return Ok(mlua::MultiValue::new());
            }
            let Value::Table(table) = second(&args) else {
                return Err(argument_failure(
                    2,
                    "mux.arg.invalid",
                    "value must be a table",
                ));
            };
            contract::check_options(
                &table,
                &[
                    "tactical_height",
                    "tactical_width",
                    "lrs_height",
                    "include_dead",
                    "include_shutdown",
                    "include_enemies",
                    "include_allies",
                    "include_target",
                    "buildings",
                    "configured",
                ],
                2,
            )?;
            let dimensions = crate::BattleViewDimensions {
                tactical_height: contract::integer_field(&table, "tactical_height", 5, 24, 2)?
                    as u16,
                tactical_width: contract::integer_field(&table, "tactical_width", 5, 40, 2)? as u16,
                long_range_height: contract::integer_field(&table, "lrs_height", 10, 40, 2)? as u16,
            };
            let booleans = crate::BattleContactPreferences {
                include_dead: contract::boolean_field(&table, "include_dead", 2)?,
                include_shutdown: contract::boolean_field(&table, "include_shutdown", 2)?,
                include_enemies: contract::boolean_field(&table, "include_enemies", 2)?,
                include_allies: contract::boolean_field(&table, "include_allies", 2)?,
                include_target: contract::boolean_field(&table, "include_target", 2)?,
                buildings: crate::BattleBuildingContactMode::FollowBrief,
            };
            let mode = contract::string_field(&table, "buildings", 12, 2)?;
            let buildings = match mode.as_str() {
                "follow_brief" => crate::BattleBuildingContactMode::FollowBrief,
                "include" => crate::BattleBuildingContactMode::Include,
                "exclude" => crate::BattleBuildingContactMode::Exclude,
                _ => {
                    return Err(argument_failure(
                        2,
                        "mux.arg.invalid",
                        "buildings has an invalid mode",
                    ));
                }
            };
            let contacts = crate::BattleContactPreferences {
                buildings,
                ..booleans
            };
            Arc::make_mut(&mut world.btech.player_preferences).insert(
                player,
                crate::BattlePlayerPreferences {
                    dimensions,
                    contacts,
                },
            );
            Ok(mlua::MultiValue::new())
        })?,
    )?;
    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "player_template_contract",
        "player",
        "mechwarrior_template",
        lua.create_function(move |lua, args: mlua::MultiValue| {
            contract::check_arity(&args, 1)?;
            let world = shared.borrow();
            let player = player(lua, &world, first(&args), 1)?;
            Ok(crate::btech::player_configuration(&world, player)
                .map_err(mlua::Error::external)?
                .mechwarrior_template)
        })?,
    )?;
    contract::bind(
        lua,
        native,
        "player_set_template_contract",
        "player",
        "set_mechwarrior_template",
        lua.create_function(|lua, args: mlua::MultiValue| {
            contract::check_arity(&args, 2)?;
            let scripts = crate::Scripts::services(lua)?;
            let player = {
                let world = scripts.world.borrow();
                player(lua, &world, first(&args), 1)?
            };
            let reference = optional_reference(second(&args))?;
            if let Some(reference) = &reference {
                validate_template_reference(lua, reference)?;
            }
            let mut world = scripts.world_mut();
            let mut configuration = crate::btech::player_configuration(&world, player)
                .map_err(mlua::Error::external)?;
            configuration.mechwarrior_template =
                reference.map(|reference| String::from_utf8_lossy(&reference).into_owned());
            crate::btech::set_player_configuration(&mut world, player, configuration)
                .map_err(|e| contract::operation_failure("template_preference_store_failed", e))?;
            Ok(mlua::MultiValue::new())
        })?,
    )?;
    let shared = world.clone();
    let parts = catalogue.clone();
    contract::bind(
        lua,
        native,
        "player_loadout_contract",
        "player",
        "loadout",
        lua.create_function(move |lua, args: mlua::MultiValue| {
            contract::check_arity(&args, 1)?;
            let world = shared.borrow();
            let player = player(lua, &world, first(&args), 1)?;
            let configuration = crate::btech::player_configuration(&world, player)
                .map_err(mlua::Error::external)?;
            configuration
                .loadout
                .as_ref()
                .map(|loadout| push_loadout(lua, &world, &parts, loadout))
                .transpose()
        })?,
    )?;
    let parts = catalogue;
    contract::bind(
        lua,
        native,
        "player_set_loadout_contract",
        "player",
        "set_loadout",
        lua.create_function(move |lua, args: mlua::MultiValue| {
            contract::check_arity(&args, 2)?;
            let scripts = crate::Scripts::services(lua)?;
            let mut world = scripts.world_mut();
            let player = player(lua, &world, first(&args), 1)?;
            let loadout = if second(&args).is_nil() {
                None
            } else {
                let Value::Table(table) = second(&args) else {
                    return Err(argument_failure(
                        2,
                        "mux.arg.invalid",
                        "value must be a table",
                    ));
                };
                Some(parse_loadout(table, &parts)?)
            };
            let mut configuration = crate::btech::player_configuration(&world, player)
                .map_err(mlua::Error::external)?;
            configuration.loadout = loadout;
            crate::btech::set_player_configuration(&mut world, player, configuration)
                .map_err(|e| contract::operation_failure("loadout_store_failed", e))?;
            Ok(mlua::MultiValue::new())
        })?,
    )?;
    Ok(())
}
