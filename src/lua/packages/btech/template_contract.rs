//! C-compatible immutable BattleTech template queries and displays.

use super::{constants, contract, error, inspection_records, parts_contract};
use crate::{Kind, ObjectId, SharedWorld};
use mlua::{Lua, MultiValue, Table, Value};
use std::path::PathBuf;

const GROUP: &str = "template";
fn value(args: &MultiValue, index: usize) -> Value {
    args.get(index).cloned().unwrap_or(Value::Nil)
}
fn argument(argument: usize, code: &'static str, message: &str) -> mlua::Error {
    error::failure_with_detail(code, message, serde_json::json!({"argument":argument}))
}

fn reference(value: Value, argument_number: usize) -> mlua::Result<Vec<u8>> {
    let Value::String(value) = value else {
        return Err(argument(
            argument_number,
            "mux.arg.invalid",
            "reference must be a string",
        ));
    };
    let raw = value.as_bytes();
    if raw.is_empty() {
        return Err(argument(
            argument_number,
            "mux.arg.invalid",
            "reference must not be empty",
        ));
    }
    let end = raw.iter().position(|byte| *byte == 0).unwrap_or(raw.len());
    let reference = raw[..end].to_vec();
    if reference.windows(2).any(|bytes| bytes == b"..")
        || reference.contains(&b'/')
        || reference.contains(&b'\\')
    {
        return Err(argument(
            argument_number,
            "mux.arg.invalid",
            "reference must not contain path components",
        ));
    }
    Ok(reference)
}

fn root(lua: &Lua) -> PathBuf {
    let config = crate::lua::configuration(lua);
    config.path(&config.database.unit_database)
}
fn read_unit(lua: &Lua, value: Value, argument_number: usize) -> mlua::Result<crate::UnitTemplate> {
    // Every critical must name a known part; validate through the raw path
    // before parsing so display and value projections reject the same
    // templates.
    read_raw_unit(lua, value.clone(), argument_number)?;
    let reference = reference(value, argument_number)?;
    let root = root(lua);
    let shared = crate::Scripts::services(lua)?.world;
    let path = {
        let mut world = shared.borrow_mut();
        crate::btech::resolve_template_path_bytes_cached(
            &mut world.btech.template_registry,
            &root,
            &reference,
        )
        .map_err(mlua::Error::external)?
    };
    let Some(path) = path else {
        return Err(argument(
            argument_number,
            "btech.template.not_found",
            "template was not found",
        ));
    };
    crate::btech::read_resolved_template(&root, &path).map_err(|_| {
        argument(
            argument_number,
            "btech.template.invalid",
            "template is malformed",
        )
    })
}
fn read_raw_unit(
    lua: &Lua,
    value: Value,
    argument_number: usize,
) -> mlua::Result<crate::RawTemplate> {
    let reference = reference(value, argument_number)?;
    let root = root(lua);
    let shared = crate::Scripts::services(lua)?.world;
    let path = {
        let mut world = shared.borrow_mut();
        crate::btech::resolve_template_path_bytes_cached(
            &mut world.btech.template_registry,
            &root,
            &reference,
        )
        .map_err(mlua::Error::external)?
    };
    let Some(path) = path else {
        return Err(argument(
            argument_number,
            "btech.template.not_found",
            "template was not found",
        ));
    };
    let mut template = crate::btech::read_resolved_raw_template(&root, &path).map_err(|_| {
        argument(
            argument_number,
            "btech.template.invalid",
            "template is malformed",
        )
    })?;
    if !parts_contract::normalize_raw_template_parts(&mut template) {
        return Err(argument(
            argument_number,
            "btech.template.invalid",
            "template is malformed",
        ));
    }
    Ok(template)
}
fn section(
    value: Value,
    template: &crate::MechTemplate,
    required: bool,
    argument_number: usize,
) -> mlua::Result<Option<crate::MechSection>> {
    if value.is_nil() {
        return if required {
            Err(argument(
                argument_number,
                "mux.arg.invalid",
                "section is required",
            ))
        } else {
            Ok(None)
        };
    }
    let code = constants::require(value, argument_number, "section", &constants::SECTIONS)?;
    crate::btech::inspection_section(template, code)
        .map(Some)
        .map_err(|_| {
            argument(
                argument_number,
                "mux.arg.invalid",
                "section is not valid for this unit",
            )
        })
}
fn raw_section(
    value: Value,
    template: &crate::RawTemplate,
    required: bool,
    argument_number: usize,
) -> mlua::Result<Option<crate::RawSectionCode>> {
    if value.is_nil() {
        return if required {
            Err(argument(
                argument_number,
                "mux.arg.invalid",
                "section is required",
            ))
        } else {
            Ok(None)
        };
    }
    let code = constants::require(value, argument_number, "section", &constants::SECTIONS)?;
    crate::RawSectionCode::for_unit(template.class, template.movement)
        .iter()
        .copied()
        .find(|section| *section as i32 == code)
        .map(Some)
        .ok_or_else(|| {
            argument(
                argument_number,
                "mux.arg.invalid",
                "section is not valid for this unit",
            )
        })
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

fn require_player(
    lua: &Lua,
    shared: &SharedWorld,
    value: Value,
    argument_number: usize,
) -> mlua::Result<ObjectId> {
    let borrowed = shared.borrow();
    let id = contract::require_object(lua, &borrowed, value, argument_number)?;
    if borrowed.objects[&id].kind != Kind::Player {
        return Err(argument(
            argument_number,
            "mux.object.invalid",
            "display recipient must be a player",
        ));
    }
    Ok(id)
}

pub(super) fn register(lua: &Lua, native: &Table, shared: &SharedWorld) -> mlua::Result<()> {
    bind(
        lua,
        native,
        "contract_template_exists",
        "exists",
        lua.create_function(|lua, arg: Value| {
            crate::lua::transactions::require(lua)?;
            let reference = reference(arg, 1)?;
            let root = root(lua);
            let shared = crate::Scripts::services(lua)?.world;
            let path = {
                let mut borrowed = shared.borrow_mut();
                crate::btech::resolve_template_path_bytes_cached(
                    &mut borrowed.btech.template_registry,
                    &root,
                    &reference,
                )
                .map_err(mlua::Error::external)?
            };
            let Some(path) = path else {
                return Ok(false);
            };
            let mut template =
                crate::btech::read_resolved_raw_template(&root, &path).map_err(|_| {
                    argument(
                        1,
                        "btech.template.invalid",
                        "existing template is malformed",
                    )
                })?;
            if !parts_contract::normalize_raw_template_parts(&mut template) {
                return Err(argument(
                    1,
                    "btech.template.invalid",
                    "existing template is malformed",
                ));
            }
            Ok(true)
        })?,
    )?;
    bind(
        lua,
        native,
        "contract_template_engine",
        "engine",
        lua.create_function(|lua, arg: Value| {
            crate::lua::transactions::require(lua)?;
            let template = read_raw_unit(lua, arg, 1)?;
            let row = lua.create_table()?;
            let (rating, suspension) = crate::btech::inspect_raw_template_engine(&template);
            row.raw_set("rating", rating)?;
            row.raw_set("suspension_factor", suspension)?;
            Ok(row)
        })?,
    )?;
    bind(
        lua,
        native,
        "contract_template_bv",
        "battle_value",
        lua.create_function(|lua, args: MultiValue| {
            if args.len() != 1 {
                return Err(error::failure(
                    "mux.arg.invalid",
                    "expected exactly 1 argument",
                ));
            }
            crate::lua::transactions::require(lua)?;
            let template = read_raw_unit(lua, value(&args, 0), 1)?;
            let bv = crate::btech::inspect_raw_template_battle_value(&template)
                .map_err(mlua::Error::external)?;
            let row = lua.create_table()?;
            row.raw_set("total", bv.total)?;
            row.raw_set("offensive", bv.offensive)?;
            row.raw_set("defensive", bv.defensive)?;
            Ok(row)
        })?,
    )?;
    bind(
        lua,
        native,
        "contract_template_base_cost",
        "base_cost",
        lua.create_function(|lua, arg: Value| -> mlua::Result<u64> {
            crate::lua::transactions::require(lua)?;
            let template = read_raw_unit(lua, arg, 1)?;
            let scripts = crate::Scripts::services(lua)?;
            let cost = crate::btech::raw_template_base_cost(
                &scripts.world.borrow().btech.part_costs,
                &template,
            )
            .map_err(mlua::Error::external)?;
            if cost > 9_007_199_254_740_991 {
                return Err(error::failure(
                    "mux.internal",
                    "template base cost is not representable in Lua",
                ));
            }
            Ok(cost)
        })?,
    )?;
    bind(
        lua,
        native,
        "contract_template_armor",
        "armor",
        lua.create_function(|lua, args: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let template = read_raw_unit(lua, value(&args, 0), 1)?;
            let selected = raw_section(value(&args, 1), &template, false, 2)?;
            inspection_records::armor(
                lua,
                crate::btech::inspect_raw_template_armor(&template, selected)
                    .map_err(mlua::Error::external)?,
            )
        })?,
    )?;
    bind(
        lua,
        native,
        "contract_template_criticals",
        "critical_slots",
        lua.create_function(|lua, args: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let template = read_raw_unit(lua, value(&args, 0), 1)?;
            let scripts = crate::Scripts::services(lua)?;
            let world = scripts.world.borrow();
            let selected = raw_section(value(&args, 1), &template, true, 2)?.unwrap();
            inspection_records::criticals(
                lua,
                &world,
                parts_contract::registered_catalogue(),
                crate::btech::inspect_raw_template_criticals(&template, selected)
                    .map_err(mlua::Error::external)?,
            )
        })?,
    )?;
    bind(
        lua,
        native,
        "contract_template_weapons",
        "weapons",
        lua.create_function(|lua, args: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let template = read_raw_unit(lua, value(&args, 0), 1)?;
            let scripts = crate::Scripts::services(lua)?;
            let world = scripts.world.borrow();
            let selected = raw_section(value(&args, 1), &template, false, 2)?;
            let rows = crate::btech::inspect_raw_template_weapons(&template)
                .map_err(mlua::Error::external)?
                .into_iter()
                .filter(|row| selected.is_none_or(|section| row.section == section as i32));
            inspection_records::weapons(lua, &world, parts_contract::registered_catalogue(), rows)
        })?,
    )?;
    for (name, payload) in [("installed_parts", false), ("payload", true)] {
        let key = if payload {
            "contract_template_payload"
        } else {
            "contract_template_installed"
        };
        bind(
            lua,
            native,
            key,
            name,
            lua.create_function(move |lua, arg: Value| {
                crate::lua::transactions::require(lua)?;
                let template = read_raw_unit(lua, arg, 1)?;
                let scripts = crate::Scripts::services(lua)?;
                let world = scripts.world.borrow();
                inspection_records::inventory(
                    lua,
                    &world,
                    parts_contract::registered_catalogue(),
                    crate::btech::inspect_raw_template_inventory(&template, payload)
                        .map_err(mlua::Error::external)?,
                )
            })?,
        )?;
    }
    bind(
        lua,
        native,
        "contract_template_technologies",
        "technologies",
        lua.create_function(|lua, arg: Value| {
            crate::lua::transactions::require(lua)?;
            let template = read_raw_unit(lua, arg, 1)?;
            let result = lua.create_table()?;
            let items = crate::btech::inspect_raw_template_technologies(&template);
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
    for (name, kind) in [
        ("show_status", 0),
        ("show_weapon_specs", 1),
        ("show_critical_status", 2),
    ] {
        let world = shared.clone();
        let key = match kind {
            0 => "contract_template_show_status",
            1 => "contract_template_show_specs",
            _ => "contract_template_show_criticals",
        };
        bind(
            lua,
            native,
            key,
            name,
            lua.create_function(move |lua, args: MultiValue| {
                crate::lua::transactions::require(lua)?;
                let template = read_unit(lua, value(&args, 0), 1)?;
                let player = require_player(lua, &world, value(&args, 1), 2)?;
                let config = crate::lua::configuration(lua);
                let text = {
                    let borrowed = world.borrow();
                    match template {
                        crate::UnitTemplate::Mech(template) => match kind {
                            0 => crate::btech::inspect_template_status_text(&borrowed, &template),
                            1 => crate::btech::inspect_template_weapon_text(
                                &borrowed,
                                &template,
                                config.battletech.erange != 0,
                            ),
                            _ => {
                                let selected =
                                    section(value(&args, 2), &template, true, 3)?.unwrap();
                                crate::btech::inspect_template_critical_text(
                                    &borrowed, &template, selected,
                                )
                            }
                        },
                        crate::UnitTemplate::Vehicle(template) => match kind {
                            0 => crate::btech::inspect_vehicle_template_status_text(
                                &borrowed, &template,
                            ),
                            1 => crate::btech::inspect_vehicle_template_weapon_text(
                                &borrowed,
                                &template,
                                config.battletech.erange != 0,
                            ),
                            _ => {
                                if value(&args, 2).is_nil() {
                                    return Err(argument(
                                        3,
                                        "mux.arg.invalid",
                                        "section is required",
                                    ));
                                }
                                let selected = crate::btech::inspection_vehicle_section_for(
                                    &template,
                                    constants::require(
                                        value(&args, 2),
                                        3,
                                        "section",
                                        &constants::SECTIONS,
                                    )?,
                                )
                                .map_err(|_| {
                                    argument(
                                        3,
                                        "mux.arg.invalid",
                                        "section is not valid for this unit",
                                    )
                                })?;
                                crate::btech::inspect_vehicle_template_critical_text(
                                    &borrowed, &template, selected,
                                )
                            }
                        },
                    }
                }
                .map_err(mlua::Error::external)?;
                let scripts = crate::Scripts::services(lua)?;
                let document =
                    crate::text::Document::native_styled(text, config.lua.output_byte_limit)
                        .map_err(mlua::Error::external)?;
                crate::notification::direct(&scripts.outbox, &config, player, document)
                    .map_err(mlua::Error::external)?;
                Ok(())
            })?,
        )?;
    }
    Ok(())
}
