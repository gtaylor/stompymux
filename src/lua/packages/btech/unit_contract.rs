//! C-compatible read-only live-unit inspection bindings.

use super::{constants, contract, error, inspection_records, parts_contract};
use crate::{ObjectId, SharedWorld};
use mlua::{Lua, MultiValue, Table, Value};

const GROUP: &str = "unit";

fn value(args: &MultiValue, index: usize) -> Value {
    args.get(index).cloned().unwrap_or(Value::Nil)
}

fn require_mech(lua: &Lua, shared: &SharedWorld, value: Value) -> mlua::Result<ObjectId> {
    let world = shared.borrow();
    let id = contract::require_special(lua, &world, value, 1, "mech", "unit")?;
    if !world.btech.constructed_units().contains_key(&id)
        && !world.btech.vehicles().contains_key(&id)
        && crate::btech::registered_unit_default_template(&world, id).is_none()
    {
        return Err(error::failure_with_detail(
            "mux.object.unavailable",
            "unit runtime state is unavailable",
            serde_json::json!({"argument":1}),
        ));
    }
    Ok(id)
}

fn require_unit(lua: &Lua, shared: &SharedWorld, value: Value) -> mlua::Result<ObjectId> {
    contract::require_special(lua, &shared.borrow(), value, 1, "mech", "unit")
}

fn raw_live_section(
    value: Value,
    template: &crate::RawTemplate,
    required: bool,
    argument: usize,
) -> mlua::Result<Option<crate::RawSectionCode>> {
    if value.is_nil() {
        return if required {
            Err(error::failure_with_detail(
                "mux.arg.invalid",
                "section is required",
                serde_json::json!({"argument":argument}),
            ))
        } else {
            Ok(None)
        };
    }
    let code = constants::require(value, argument, "section", &constants::SECTIONS)?;
    crate::RawSectionCode::for_unit(template.class, template.movement)
        .iter()
        .copied()
        .find(|section| *section as i32 == code)
        .map(Some)
        .ok_or_else(|| {
            error::failure_with_detail(
                "mux.arg.invalid",
                "section is not valid for this unit",
                serde_json::json!({"argument":argument}),
            )
        })
}

fn section(
    value: Value,
    template: &crate::BattleTemplate,
    required: bool,
    argument: usize,
) -> mlua::Result<Option<crate::BattleSection>> {
    if value.is_nil() {
        return if required {
            Err(error::failure_with_detail(
                "mux.arg.invalid",
                "section is required",
                serde_json::json!({"argument":argument}),
            ))
        } else {
            Ok(None)
        };
    }
    let code = constants::require(value, argument, "section", &constants::SECTIONS)?;
    crate::btech::inspection_section(template, code)
        .map(Some)
        .map_err(|_| {
            error::failure_with_detail(
                "mux.arg.invalid",
                "section is not valid for this unit",
                serde_json::json!({"argument":argument}),
            )
        })
}

fn vehicle_section(
    world: &crate::World,
    id: ObjectId,
    unit: &crate::BattleVehicle,
    value: Value,
    required: bool,
    argument: usize,
) -> mlua::Result<Option<crate::BattleVehicleSection>> {
    if value.is_nil() {
        return if required {
            Err(error::failure_with_detail(
                "mux.arg.invalid",
                "section is required",
                serde_json::json!({"argument":argument}),
            ))
        } else {
            Ok(None)
        };
    }
    let code = constants::require(value, argument, "section", &constants::SECTIONS)?;
    if !crate::btech::administrative_section_valid(world, id, code) {
        return Err(error::failure_with_detail(
            "mux.arg.invalid",
            "section is not valid for this unit",
            serde_json::json!({"argument":argument}),
        ));
    }
    crate::btech::inspection_vehicle_section_for(unit.definition(), code)
        .map(Some)
        .map_err(|_| {
            error::failure_with_detail(
                "mux.arg.invalid",
                "section is not valid for this unit",
                serde_json::json!({"argument":argument}),
            )
        })
}

/// Native suspension factor by locomotion and tonnage (C mech_consistency.c susp_factor).
fn suspension_factor(movement: crate::RawMovement, tons: i32) -> i32 {
    match movement {
        crate::RawMovement::Tracked => 0,
        crate::RawMovement::Wheeled => 20,
        crate::RawMovement::Foil => match tons {
            ..=10 => 60,
            ..=20 => 105,
            ..=30 => 150,
            ..=40 => 195,
            ..=50 => 255,
            ..=60 => 300,
            ..=70 => 345,
            ..=80 => 390,
            ..=90 => 435,
            _ => 480,
        },
        crate::RawMovement::Hover => match tons {
            ..=10 => 40,
            ..=20 => 85,
            ..=30 => 130,
            ..=40 => 175,
            _ => 235,
        },
        crate::RawMovement::Hull | crate::RawMovement::Submarine => 30,
        crate::RawMovement::Vtol => match tons {
            ..=10 => 50,
            ..=20 => 95,
            _ => 140,
        },
        _ => 0,
    }
}

fn live_mech_section(
    world: &crate::World,
    id: ObjectId,
    value: Value,
    template: &crate::BattleTemplate,
    required: bool,
    argument: usize,
) -> mlua::Result<Option<crate::BattleSection>> {
    let selected = section(value, template, required, argument)?;
    if let Some(selected) = selected {
        let code = crate::btech::inspection_section_code(template, selected)
            .map_err(mlua::Error::external)?;
        let has_runtime = world.btech.constructed_units().contains_key(&id)
            || world.btech.vehicles().contains_key(&id);
        if has_runtime && !crate::btech::administrative_section_valid(world, id, code) {
            return Err(error::failure_with_detail(
                "mux.arg.invalid",
                "section is not valid for this unit",
                serde_json::json!({"argument":argument}),
            ));
        }
    }
    Ok(selected)
}

fn bind(
    lua: &Lua,
    native: &Table,
    key: &'static str,
    name: &'static str,
    function: mlua::Function,
) -> mlua::Result<()> {
    contract::bind(lua, native, key, GROUP, name, function)
}

/// Install canonical getters after the older Rust extensions.
pub(super) fn register(lua: &Lua, native: &Table, shared: &SharedWorld) -> mlua::Result<()> {
    let old: mlua::Function = native.raw_get("unit_weapons")?;
    bind(lua, native, "unit_weapon_states", "weapon_states", old)?;

    let world = shared.clone();
    bind(
        lua,
        native,
        "contract_unit_armor",
        "armor",
        lua.create_function(move |lua, args: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let id = require_mech(lua, &world, value(&args, 0))?;
            let borrowed = world.borrow();
            if let Some(unit) = borrowed.btech.vehicles().get(&id) {
                let raw = crate::btech::compose_vehicle_raw_inspection(unit);
                let selected = raw_live_section(value(&args, 1), &raw, false, 2)?;
                return inspection_records::armor(
                    lua,
                    crate::btech::inspect_composed_vehicle_armor(unit, selected)
                        .map_err(mlua::Error::external)?,
                );
            }
            let default = crate::btech::registered_unit_default_template(&borrowed, id);
            let unit = borrowed.btech.constructed_units().get(&id);
            let template = unit
                .map(crate::BattleUnit::definition)
                .or(default.as_ref())
                .unwrap();
            let raw = unit.map_or_else(
                || crate::RawTemplate::from(template),
                crate::btech::compose_unit_raw_inspection,
            );
            let selected = raw_live_section(value(&args, 1), &raw, false, 2)?;
            inspection_records::armor(
                lua,
                match unit {
                    Some(unit) => crate::btech::inspect_composed_unit_armor(unit, selected),
                    None => crate::btech::inspect_raw_template_armor(&raw, selected),
                }
                .map_err(mlua::Error::external)?,
            )
        })?,
    )?;

    let world = shared.clone();
    bind(
        lua,
        native,
        "contract_unit_criticals",
        "critical_slots",
        lua.create_function(move |lua, args: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let id = require_mech(lua, &world, value(&args, 0))?;
            let borrowed = world.borrow();
            let catalogue = parts_contract::registered_catalogue();
            if let Some(unit) = borrowed.btech.vehicles().get(&id) {
                let selected =
                    vehicle_section(&borrowed, id, unit, value(&args, 1), true, 2)?.unwrap();
                return inspection_records::criticals(
                    lua,
                    &borrowed,
                    &catalogue,
                    crate::btech::inspect_vehicle_criticals(unit, selected)
                        .map_err(mlua::Error::external)?,
                );
            }
            let default = crate::btech::registered_unit_default_template(&borrowed, id);
            let unit = borrowed.btech.constructed_units().get(&id);
            let template = unit
                .map(crate::BattleUnit::definition)
                .or(default.as_ref())
                .unwrap();
            let selected =
                live_mech_section(&borrowed, id, value(&args, 1), template, true, 2)?.unwrap();
            inspection_records::criticals(
                lua,
                &borrowed,
                &catalogue,
                match unit {
                    Some(unit) => crate::btech::inspect_unit_criticals(unit, selected),
                    None => crate::btech::inspect_template_criticals(template, selected),
                }
                .map_err(mlua::Error::external)?,
            )
        })?,
    )?;

    let world = shared.clone();
    bind(
        lua,
        native,
        "contract_unit_weapons",
        "weapons",
        lua.create_function(move |lua, args: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let id = require_mech(lua, &world, value(&args, 0))?;
            let borrowed = world.borrow();
            if let Some(unit) = borrowed.btech.vehicles().get(&id) {
                let selected = vehicle_section(&borrowed, id, unit, value(&args, 1), false, 2)?;
                let code = selected.map(crate::btech::inspection_vehicle_section_code);
                let rows = crate::btech::inspect_vehicle_weapons(unit)
                    .map_err(mlua::Error::external)?
                    .into_iter()
                    .filter(|row| code.is_none_or(|code| code == row.section));
                return inspection_records::weapons(
                    lua,
                    &borrowed,
                    &parts_contract::registered_catalogue(),
                    rows,
                );
            }
            let default = crate::btech::registered_unit_default_template(&borrowed, id);
            let unit = borrowed.btech.constructed_units().get(&id);
            let template = unit
                .map(crate::BattleUnit::definition)
                .or(default.as_ref())
                .unwrap();
            let selected = live_mech_section(&borrowed, id, value(&args, 1), template, false, 2)?;
            let rows = match unit {
                Some(unit) => crate::btech::inspect_unit_weapons(unit),
                None => crate::btech::inspect_template_weapons(template),
            }
            .map_err(mlua::Error::external)?
            .into_iter()
            .filter(|row| {
                selected.is_none_or(|s| {
                    crate::btech::inspection_section_code(template, s).ok() == Some(row.section)
                })
            });
            inspection_records::weapons(
                lua,
                &borrowed,
                &parts_contract::registered_catalogue(),
                rows,
            )
        })?,
    )?;

    let world = shared.clone();
    bind(
        lua,
        native,
        "contract_unit_radio",
        "radio_channels",
        lua.create_function(move |lua, arg: Value| {
            crate::lua::transactions::require(lua)?;
            let id = require_mech(lua, &world, arg)?;
            let borrowed = world.borrow();
            let result = lua.create_table()?;
            let channels = borrowed
                .btech
                .vehicles()
                .get(&id)
                .map(|unit| unit.radio_channels())
                .or_else(|| {
                    borrowed
                        .btech
                        .constructed_units()
                        .get(&id)
                        .map(|unit| unit.radio_channels())
                });
            // C projects no channels while the radio configuration is zeroed
            // (btech_unit_bindings.c:300 with mech_radio_state.c:46-48).
            const NO_CHANNELS: [crate::btech::BattleRadioChannel; 0] = [];
            let channels = channels.unwrap_or(&NO_CHANNELS);
            for (i, channel) in channels.iter().enumerate() {
                let row = lua.create_table()?;
                row.raw_set("channel", i + 1)?;
                row.raw_set("frequency", channel.frequency)?;
                row.raw_set("title", channel.title.as_str())?;
                let modes = lua.create_table()?;
                let mut n = 1;
                for (enabled, name) in [
                    (channel.mode.digital, "digital"),
                    (channel.mode.muted, "mute"),
                    (channel.mode.relay, "relay"),
                    (channel.mode.info, "information"),
                    (channel.mode.scan, "scan"),
                ] {
                    if enabled {
                        modes.raw_set(n, name)?;
                        n += 1;
                    }
                }
                row.raw_set("modes", modes)?;
                result.raw_set(i + 1, row)?;
            }
            Ok(result)
        })?,
    )?;

    let world = shared.clone();
    bind(
        lua,
        native,
        "contract_unit_engine",
        "engine",
        lua.create_function(move |lua, arg: Value| {
            crate::lua::transactions::require(lua)?;
            let id = require_mech(lua, &world, arg)?;
            let borrowed = world.borrow();
            let result = lua.create_table()?;
            if let Some(unit) = borrowed.btech.vehicles().get(&id) {
                let tons = crate::btech::administrative_unit_tonnage(&borrowed, id)
                    .ok_or_else(|| mlua::Error::external("unit tonnage is unavailable"))?;
                let movement = crate::btech::administrative_unit_movement(&borrowed, id)
                    .and_then(|value| crate::BattleVehicleMovement::parse(&value).ok())
                    .unwrap_or(unit.definition().movement);
                let (rating, suspension) = crate::btech::inspection_vehicle_engine_values(
                    tons,
                    movement,
                    unit.definition().max_speed,
                )
                .map_err(mlua::Error::external)?;
                result.raw_set("rating", rating)?;
                result.raw_set("suspension_factor", suspension)?;
                return Ok(result);
            }
            let default = crate::btech::registered_unit_default_template(&borrowed, id);
            let unit = borrowed.btech.constructed_units().get(&id);
            let template = unit
                .map(crate::BattleUnit::definition)
                .or(default.as_ref())
                .unwrap();
            let raw = unit.map_or_else(
                || crate::RawTemplate::from(template),
                crate::btech::compose_unit_raw_inspection,
            );
            result.raw_set(
                "rating",
                crate::btech::inspection_engine_rating(template).map_err(mlua::Error::external)?,
            )?;
            result.raw_set(
                "suspension_factor",
                suspension_factor(raw.movement, raw.tons),
            )?;
            Ok(result)
        })?,
    )?;

    let world = shared.clone();
    bind(
        lua,
        native,
        "contract_unit_bv",
        "battle_value",
        lua.create_function(move |lua, args: MultiValue| {
            if args.len() != 1 {
                return Err(error::failure(
                    "mux.arg.invalid",
                    "expected exactly 1 argument",
                ));
            }
            crate::lua::transactions::require(lua)?;
            let id = require_mech(lua, &world, value(&args, 0))?;
            let borrowed = world.borrow();
            let config = crate::lua::configuration(lua);
            let bv = if borrowed.btech.constructed_units().contains_key(&id)
                || borrowed.btech.vehicles().contains_key(&id)
            {
                crate::btech::inspection_battle_value(&borrowed, id, &config)
            } else {
                crate::btech::inspection_template_battle_value(
                    &crate::btech::registered_unit_default_template(&borrowed, id).unwrap(),
                )
            }
            .map_err(mlua::Error::external)?;
            let result = lua.create_table()?;
            result.raw_set("total", bv.total)?;
            result.raw_set("offensive", bv.offensive)?;
            result.raw_set("defensive", bv.defensive)?;
            Ok(result)
        })?,
    )?;

    for (name, payload) in [("installed_parts", false), ("payload", true)] {
        let world = shared.clone();
        let key = if payload {
            "contract_unit_payload"
        } else {
            "contract_unit_installed"
        };
        bind(
            lua,
            native,
            key,
            name,
            lua.create_function(move |lua, arg: Value| {
                crate::lua::transactions::require(lua)?;
                let id = require_mech(lua, &world, arg)?;
                let borrowed = world.borrow();
                if let Some(unit) = borrowed.btech.vehicles().get(&id) {
                    return inspection_records::inventory(
                        lua,
                        &borrowed,
                        &parts_contract::registered_catalogue(),
                        crate::btech::inspect_vehicle_inventory(unit, payload)
                            .map_err(mlua::Error::external)?,
                    );
                }
                let default = crate::btech::registered_unit_default_template(&borrowed, id);
                let unit = borrowed.btech.constructed_units().get(&id);
                let template = unit
                    .map(|unit| unit.definition())
                    .or(default.as_ref())
                    .unwrap();
                inspection_records::inventory(
                    lua,
                    &borrowed,
                    &parts_contract::registered_catalogue(),
                    match unit {
                        Some(unit) => crate::btech::inspect_unit_inventory(unit, payload),
                        None => crate::btech::inspect_template_inventory(template, payload),
                    }
                    .map_err(mlua::Error::external)?,
                )
            })?,
        )?;
    }

    let world = shared.clone();
    bind(
        lua,
        native,
        "contract_unit_preferred",
        "preferred_id",
        lua.create_function(move |lua, args: MultiValue| {
            contract::check_arity(&args, 1)?;
            crate::lua::transactions::require(lua)?;
            let id = require_unit(lua, &world, value(&args, 0))?;
            let borrowed = world.borrow();
            let configuration = crate::btech::unit_configuration(&borrowed, id);
            inspection_records::optional_string(lua, configuration.preferred_id.as_deref())
        })?,
    )?;
    let world = shared.clone();
    bind(
        lua,
        native,
        "contract_unit_markings",
        "markings",
        lua.create_function(move |lua, args: MultiValue| {
            contract::check_arity(&args, 1)?;
            crate::lua::transactions::require(lua)?;
            let id = require_unit(lua, &world, value(&args, 0))?;
            let borrowed = world.borrow();
            let configuration = crate::btech::unit_configuration(&borrowed, id);
            inspection_records::optional_string(lua, configuration.markings.as_deref())
        })?,
    )?;
    let world = shared.clone();
    bind(
        lua,
        native,
        "contract_unit_display",
        "display_name",
        lua.create_function(move |lua, arg: Value| {
            crate::lua::transactions::require(lua)?;
            let id = require_unit(lua, &world, arg)?;
            let borrowed = world.borrow();
            let configuration = crate::btech::unit_configuration(&borrowed, id);
            inspection_records::optional_string(lua, configuration.display_name.as_deref())
        })?,
    )?;
    let world = shared.clone();
    bind(
        lua,
        native,
        "contract_unit_pilot",
        "assigned_pilot",
        lua.create_function(move |lua, args: MultiValue| {
            contract::check_arity(&args, 1)?;
            crate::lua::transactions::require(lua)?;
            let id = require_unit(lua, &world, value(&args, 0))?;
            let borrowed = world.borrow();
            let pilot = crate::btech::unit_configuration(&borrowed, id).assigned_pilot;
            drop(borrowed);
            contract::push_optional_object(lua, &world, pilot)
        })?,
    )?;

    for (name, kph) in [
        ("effective_max_speed", false),
        ("effective_max_speed_kph", true),
    ] {
        let world = shared.clone();
        let key = if kph {
            "contract_unit_speed_kph"
        } else {
            "contract_unit_speed"
        };
        bind(
            lua,
            native,
            key,
            name,
            lua.create_function(move |lua, arg: Value| {
                crate::lua::transactions::require(lua)?;
                let id = require_mech(lua, &world, arg)?;
                let config = crate::lua::configuration(lua);
                let borrowed = world.borrow();
                let speed = if borrowed.btech.constructed_units().contains_key(&id)
                    || borrowed.btech.vehicles().contains_key(&id)
                {
                    crate::btech::inspection_effective_maximum_speed(&borrowed, id, &config)
                        .map_err(mlua::Error::external)?
                } else {
                    0.0
                };
                Ok(if kph { speed } else { speed / 10.75 })
            })?,
        )?;
    }

    let world = shared.clone();
    bind(
        lua,
        native,
        "contract_unit_condition",
        "section_condition",
        lua.create_function(move |lua, args: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let id = require_mech(lua, &world, value(&args, 0))?;
            let borrowed = world.borrow();
            if let Some(unit) = borrowed.btech.vehicles().get(&id) {
                let selected =
                    vehicle_section(&borrowed, id, unit, value(&args, 1), true, 2)?.unwrap();
                return Ok(crate::btech::inspect_vehicle_section_condition(
                    unit, selected,
                ));
            }
            let default = crate::btech::registered_unit_default_template(&borrowed, id);
            let Some(unit) = borrowed.btech.constructed_units().get(&id) else {
                let template = default.as_ref().unwrap();
                let selected =
                    live_mech_section(&borrowed, id, value(&args, 1), template, true, 2)?.unwrap();
                return Ok(if template.sections[&selected].internal == 0 {
                    "destroyed"
                } else {
                    "operational"
                });
            };
            let selected =
                live_mech_section(&borrowed, id, value(&args, 1), unit.definition(), true, 2)?
                    .unwrap();
            Ok(crate::btech::inspect_section_condition(unit, selected))
        })?,
    )?;

    let world = shared.clone();
    bind(
        lua,
        native,
        "contract_unit_tic_weapons",
        "tic_weapons",
        lua.create_function(move |lua, args: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let id = require_mech(lua, &world, value(&args, 0))?;
            let tic = match value(&args, 1) {
                Value::Integer(value) => value as f64,
                Value::Number(value) => value,
                _ => {
                    return Err(error::failure_with_detail(
                        "mux.arg.invalid",
                        "tic must be a number",
                        serde_json::json!({"argument":2}),
                    ));
                }
            };
            if !tic.is_finite() || !(0.0..=3.0).contains(&tic) {
                return Err(error::failure_with_detail(
                    "mux.arg.invalid",
                    "tic is outside its valid range",
                    serde_json::json!({"argument":2}),
                ));
            }
            if tic.fract() != 0.0 {
                return Err(error::failure_with_detail(
                    "mux.arg.invalid",
                    "tic must be an integer",
                    serde_json::json!({"argument":2}),
                ));
            }
            let borrowed = world.borrow();
            if let Some(unit) = borrowed.btech.vehicles().get(&id) {
                let selected = crate::btech::inspect_vehicle_tic(unit, tic as usize)
                    .map_err(mlua::Error::external)?;
                let rows = crate::btech::inspect_vehicle_weapons(unit)
                    .map_err(mlua::Error::external)?
                    .into_iter()
                    .filter(|row| selected.contains(&row.number));
                return inspection_records::weapons(
                    lua,
                    &borrowed,
                    &parts_contract::registered_catalogue(),
                    rows,
                );
            }
            let Some(unit) = borrowed.btech.constructed_units().get(&id) else {
                return Ok(lua.create_table()?);
            };
            let selected = crate::btech::inspect_unit_tic(unit, tic as usize)
                .map_err(mlua::Error::external)?;
            let rows = crate::btech::inspect_unit_weapons(unit)
                .map_err(mlua::Error::external)?
                .into_iter()
                .filter(|row| selected.contains(&row.number));
            inspection_records::weapons(
                lua,
                &borrowed,
                &parts_contract::registered_catalogue(),
                rows,
            )
        })?,
    )?;

    let world = shared.clone();
    bind(
        lua,
        native,
        "contract_unit_technologies",
        "technologies",
        lua.create_function(move |lua, arg: Value| {
            crate::lua::transactions::require(lua)?;
            let id = require_mech(lua, &world, arg)?;
            let borrowed = world.borrow();
            let result = lua.create_table()?;
            let default = crate::btech::registered_unit_default_template(&borrowed, id);
            let items = if let Some(unit) = borrowed.btech.vehicles().get(&id) {
                crate::btech::inspect_raw_template_technologies(
                    &crate::btech::compose_vehicle_raw_inspection(unit),
                )
            } else if let Some(unit) = borrowed.btech.constructed_units().get(&id) {
                crate::btech::inspect_raw_template_technologies(
                    &crate::btech::compose_unit_raw_inspection(unit),
                )
            } else {
                crate::btech::inspect_technologies(default.as_ref().unwrap())
            };
            for (index, item) in items
                .map_err(mlua::Error::external)?
                .into_iter()
                .enumerate()
            {
                let row = lua.create_table()?;
                row.raw_set(
                    "code",
                    constants::push(lua, &constants::TECHNOLOGY, item.code)?,
                )?;
                row.raw_set("name", item.name)?;
                row.raw_set("group", item.group)?;
                row.raw_set("source", item.source)?;
                result.raw_set(index + 1, row)?;
            }
            Ok(result)
        })?,
    )?;
    Ok(())
}
