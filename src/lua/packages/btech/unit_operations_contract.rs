//! C-compatible live-unit and trusted construction operations.

use super::{constants, contract, error, parts_contract};
use crate::{
    BattleAmmunitionMode, BattleFireMode, BattleSection, BattleWeapon, ObjectId, SharedWorld,
};
use mlua::{Lua, MultiValue, Table, Value};
use std::sync::Arc;

const GROUP: &str = "unit";
fn arg(values: &MultiValue, index: usize) -> Value {
    values.get(index).cloned().unwrap_or(Value::Nil)
}
fn failure(argument: usize, code: &'static str, message: impl ToString) -> mlua::Error {
    error::failure_with_detail(code, message, serde_json::json!({"argument": argument}))
}
fn table(value: Value, argument: usize) -> mlua::Result<Table> {
    if let Value::Table(value) = value {
        Ok(value)
    } else {
        Err(failure(
            argument,
            "mux.arg.invalid",
            "value must be a table",
        ))
    }
}
fn unit(lua: &Lua, shared: &SharedWorld, value: Value) -> mlua::Result<ObjectId> {
    contract::require_special(lua, &shared.borrow(), value, 1, "mech", "unit")
}
fn materialize(shared: &SharedWorld, id: ObjectId) -> mlua::Result<()> {
    crate::btech::ensure_registered_unit_runtime(&mut shared.borrow_mut(), id)
        .map_err(mlua::Error::external)
}
#[derive(Clone, Copy)]
enum Section {
    Mech(BattleSection),
    Vehicle(crate::BattleVehicleSection),
}
fn section(
    value: Value,
    argument: usize,
    shared: &SharedWorld,
    id: ObjectId,
) -> mlua::Result<Section> {
    let code = constants::require(value, argument, "section", &constants::SECTIONS)?;
    let world = shared.borrow();
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        return crate::btech::inspection_section(unit.definition(), code)
            .map(Section::Mech)
            .map_err(|_| {
                failure(
                    argument,
                    "mux.arg.invalid",
                    "section is not valid for this unit",
                )
            });
    }
    if world.btech.vehicles().contains_key(&id) {
        return crate::btech::inspection_vehicle_section(code)
            .map(Section::Vehicle)
            .map_err(|_| {
                failure(
                    argument,
                    "mux.arg.invalid",
                    "section is not valid for this unit",
                )
            });
    }
    if world.btech.registrations().get(&id).map(String::as_str) == Some("MECH") {
        return crate::btech::registered_unit_default_template(&world, id)
            .and_then(|definition| crate::btech::inspection_section(&definition, code).ok())
            .map(Section::Mech)
            .ok_or_else(|| {
                failure(
                    argument,
                    "mux.arg.invalid",
                    "section is not valid for this unit",
                )
            });
    }
    Err(failure(
        1,
        "mux.object.unavailable",
        "unit runtime state is unavailable",
    ))
}
fn slots(shared: &SharedWorld, id: ObjectId, section: Section) -> i64 {
    match section {
        Section::Mech(section) => shared
            .borrow()
            .btech
            .constructed_units()
            .get(&id)
            .map_or(12, |unit| unit.chassis().critical_slots(section) as i64),
        Section::Vehicle(_) => 12,
    }
}
fn optional_bool(record: &Table, name: &str, argument: usize) -> mlua::Result<bool> {
    match contract::field(record, name)? {
        Value::Nil => Ok(false),
        Value::Boolean(v) => Ok(v),
        _ => Err(failure(
            argument,
            "mux.arg.invalid",
            format!("{name} must be a boolean"),
        )),
    }
}
fn optional_string(record: &Table, name: &str, argument: usize) -> mlua::Result<Option<String>> {
    match contract::field(record, name)? {
        Value::Nil => Ok(None),
        Value::String(v) => {
            let bytes = v.as_bytes();
            let end = bytes
                .iter()
                .position(|byte| *byte == 0)
                .unwrap_or(bytes.len());
            Ok(Some(String::from_utf8_lossy(&bytes[..end]).into_owned()))
        }
        _ => Err(failure(
            argument,
            "mux.arg.invalid",
            format!("{name} must be a string"),
        )),
    }
}
fn optional_integer(record: &Table, name: &str, argument: usize) -> mlua::Result<i32> {
    match contract::field(record, name)? {
        Value::Nil => Ok(0),
        value => direct_integer(value, name, i32::MIN as i64, i32::MAX as i64, argument)
            .map(|v| v as i32),
    }
}
fn direct_integer(
    value: Value,
    name: &str,
    minimum: i64,
    maximum: i64,
    argument: usize,
) -> mlua::Result<i64> {
    let number = match value {
        Value::Integer(value) => value as f64,
        Value::Number(value) => value,
        _ => {
            return Err(failure(
                argument,
                "mux.arg.invalid",
                format!("{name} must be an integer"),
            ));
        }
    };
    if !number.is_finite()
        || number.fract() != 0.0
        || number < minimum as f64
        || number > maximum as f64
    {
        return Err(failure(
            argument,
            "mux.arg.invalid",
            format!("{name} is outside its valid range"),
        ));
    }
    Ok(number as i64)
}
fn named_bits(
    record: &Table,
    name: &str,
    argument: usize,
    catalog: &'static constants::Catalog,
) -> mlua::Result<Vec<i32>> {
    let value = contract::field(record, name)?;
    if value == Value::Nil {
        return Ok(Vec::new());
    }
    let Value::Table(values) = value else {
        return Err(failure(
            argument,
            "mux.arg.invalid",
            format!("{name} must be an array"),
        ));
    };
    (1..=values.raw_len())
        .map(|index| constants::require(values.raw_get(index)?, argument, name, catalog))
        .collect()
}

fn c_string(value: &mlua::LuaString) -> String {
    let bytes = value.as_bytes();
    let end = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}
fn fire_mode(bits: &[i32]) -> BattleFireMode {
    if bits.contains(&65536) {
        BattleFireMode::Heat
    } else if bits.contains(&32768) {
        BattleFireMode::Rotary6
    } else if bits.contains(&16384) {
        BattleFireMode::Rotary4
    } else if bits.contains(&8192) {
        BattleFireMode::Rotary2
    } else if bits.contains(&4096) {
        BattleFireMode::Gatling
    } else if bits.contains(&2048) {
        BattleFireMode::Rapid
    } else if bits.contains(&1024) {
        BattleFireMode::Ultra
    } else if bits.contains(&64) {
        BattleFireMode::Hotload
    } else {
        BattleFireMode::Normal
    }
}
/// Project the first round bit; an MML family bit selects the matching long-range supply.
fn ammo_mode(bits: &[i32]) -> BattleAmmunitionMode {
    let long_range = bits.contains(&4194304);
    let round = munition_mode(
        bits.iter()
            .copied()
            .find(|&bit| bit != 4194304)
            .unwrap_or(0),
    );
    if !long_range {
        return round;
    }
    round
        .with_mml_family(true)
        .unwrap_or(BattleAmmunitionMode::MmlLrm)
}

/// The round selected by one reference ammunition bit, excluding the MML family bit.
fn munition_mode(bit: i32) -> BattleAmmunitionMode {
    match bit {
        1 | 8 => BattleAmmunitionMode::Cluster,
        // Shared reference bits resolve as template flags do: Artemis/Mine and Narc/Smoke.
        2 => BattleAmmunitionMode::Artemis,
        4 => BattleAmmunitionMode::Narc,
        16 => BattleAmmunitionMode::Mine,
        32 => BattleAmmunitionMode::Smoke,
        64 => BattleAmmunitionMode::Inferno,
        128 => BattleAmmunitionMode::Swarm,
        256 => BattleAmmunitionMode::Swarm1,
        512 => BattleAmmunitionMode::INarcExplosive,
        1024 => BattleAmmunitionMode::INarcHaywire,
        2048 => BattleAmmunitionMode::INarcEcm,
        4096 => BattleAmmunitionMode::INarcNemesis,
        8192 => BattleAmmunitionMode::ArmorPiercing,
        16384 => BattleAmmunitionMode::Flechette,
        32768 => BattleAmmunitionMode::Incendiary,
        65536 => BattleAmmunitionMode::Precision,
        131072 => BattleAmmunitionMode::Stinger,
        262144 => BattleAmmunitionMode::Caseless,
        524288 => BattleAmmunitionMode::SemiGuided,
        1048576 => BattleAmmunitionMode::ExtendedRange,
        2097152 => BattleAmmunitionMode::HighExplosive,
        _ => BattleAmmunitionMode::Normal,
    }
}
fn mode_names(bits: &[i32]) -> Vec<String> {
    bits.iter()
        .filter_map(|bit| {
            match bit {
                1 => Some("LBX/Cluster"),
                2 => Some("Artemis/Mine"),
                4 => Some("Narc/Smoke"),
                8 => Some("Cluster"),
                16 => Some("Mine"),
                32 => Some("Smoke"),
                64 => Some("Inferno"),
                128 => Some("Swarm"),
                256 => Some("Swarm1"),
                512 => Some("iNarc_Explosive"),
                1024 => Some("iNarc_Haywire"),
                2048 => Some("iNarc_ECM"),
                4096 => Some("iNarc_Nemesis"),
                8192 => Some("AP"),
                16384 => Some("Flechette"),
                32768 => Some("Incendiary"),
                65536 => Some("Precision"),
                131072 => Some("Stinger"),
                262144 => Some("Caseless"),
                524288 => Some("Sguided"),
                1048576 => Some("ExtendedRange"),
                2097152 => Some("HighExplosive"),
                4194304 => Some("MML_LRM"),
                _ => None,
            }
            .map(str::to_owned)
        })
        .collect()
}
fn fire_mode_names(bits: &[i32]) -> Vec<String> {
    bits.iter()
        .filter_map(|bit| {
            match bit {
                1 => Some("Destroyed"),
                2 => Some("Disabled"),
                4 => Some("Broken"),
                8 => Some("Damaged"),
                16 => Some("OnTC"),
                32 => Some("RearMount"),
                64 => Some("Hotload"),
                128 => Some("Halfton"),
                256 => Some("OneShot"),
                512 => Some("OneShot_Used"),
                1024 => Some("UltraMode"),
                2048 => Some("RapidFire"),
                4096 => Some("Gattling"),
                8192 => Some("Rotary_TwoShot"),
                16384 => Some("Rotary_FourShot"),
                32768 => Some("Rotary_SixShot"),
                65536 => Some("Heat"),
                131072 => Some("BackPack"),
                262144 => Some("Jettisoned"),
                524288 => Some("OmniBase"),
                1048576 => Some("RocketFired"),
                _ => None,
            }
            .map(str::to_owned)
        })
        .collect()
}

pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let catalogue = Arc::new(parts_contract::registered_catalogue());
    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "unit_contract_apply_damage",
        GROUP,
        "apply_damage",
        lua.create_function(move |lua, args: MultiValue| {
            let id = unit(lua, &shared, arg(&args, 0))?;
            let request = table(arg(&args, 1), 2)?;
            contract::check_options(
                &request,
                &[
                    "amount",
                    "cluster_size",
                    "direction_code",
                    "force_critical",
                    "unit_message",
                    "map_message",
                ],
                2,
            )?;
            let amount = contract::integer_field(&request, "amount", 1, 1000, 2)? as i32;
            let cluster = contract::integer_field(&request, "cluster_size", 1, 1000, 2)? as i32;
            let direction = contract::integer_field(&request, "direction_code", 0, 21, 2)? as usize;
            let critical = optional_bool(&request, "force_critical", 2)?;
            let unit_message = optional_string(&request, "unit_message", 2)?;
            let map_message = optional_string(&request, "map_message", 2)?;
            materialize(&shared, id)?;
            let scripts = crate::Scripts::services(lua)?;
            let config = crate::lua::configuration(lua);
            crate::btech::apply_unit_damage_action(
                &scripts,
                &config,
                id,
                crate::btech::UnitDamageRequest {
                    amount: amount as u16,
                    cluster_size: cluster as u16,
                    direction: direction as u8,
                    critical,
                    unit_message: unit_message.as_deref(),
                    map_message: map_message.as_deref(),
                },
            )
            .map_err(|_| contract::operation_failure("damage_failed", "damage operation failed"))?;
            Ok(MultiValue::new())
        })?,
    )?;

    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "unit_contract_piloting_check",
        GROUP,
        "piloting_check",
        lua.create_function(move |lua, args: MultiValue| {
            let id = unit(lua, &shared, arg(&args, 0))?;
            let options = table(arg(&args, 1), 2)?;
            contract::check_options(&options, &["roll_modifier", "damage_modifier"], 2)?;
            let roll = contract::integer_field(
                &options,
                "roll_modifier",
                i32::MIN as i64,
                i32::MAX as i64,
                2,
            )?;
            let damage = contract::integer_field(
                &options,
                "damage_modifier",
                i32::MIN as i64,
                i32::MAX as i64,
                2,
            )?;
            materialize(&shared, id)?;
            let scripts = crate::Scripts::services(lua)?;
            let config = crate::lua::configuration(lua);
            crate::btech::unit_piloting_check_action(
                &scripts,
                &config,
                id,
                roll as i32,
                damage as i32,
            )
            .map_err(mlua::Error::external)
        })?,
    )?;

    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "unit_contract_load_template",
        GROUP,
        "load_template",
        lua.create_function(move |lua, args: MultiValue| {
            let id =
                contract::require_special(lua, &shared.borrow(), arg(&args, 0), 1, "mech", "unit")?;
            let reference = match arg(&args, 1) {
                Value::String(v) if !v.as_bytes().is_empty() => c_string(&v),
                _ => {
                    return Err(failure(
                        2,
                        "mux.arg.invalid",
                        "reference must be a non-empty string",
                    ));
                }
            };
            contract::validate_resource_name(&reference, "reference", 2)?;
            let config = crate::lua::configuration(lua);
            let root = config.path(&config.database.mech_database);
            let path = crate::btech::resolve_template_path_cached(
                &mut shared.borrow_mut().btech.template_registry,
                &root,
                &reference,
            )
            .map_err(|_| failure(2, "btech.template.not_found", "template was not found"))?
            .ok_or_else(|| failure(2, "btech.template.not_found", "template was not found"))?;
            let template = crate::btech::read_resolved_template(&root, &path)
                .map_err(|_| failure(2, "btech.template.invalid", "template is malformed"))?;
            if !parts_contract::unit_template_parts_registered(&template) {
                return Err(failure(
                    2,
                    "btech.template.invalid",
                    "template is malformed",
                ));
            }
            crate::btech::load_unit_template(&mut shared.borrow_mut(), id, &reference, template)
                .map_err(|_| failure(2, "btech.template.invalid", "template is malformed"))?;
            Ok(MultiValue::new())
        })?,
    )?;

    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "unit_contract_restore",
        GROUP,
        "restore",
        lua.create_function(move |lua, args: MultiValue| {
            let id = unit(lua, &shared, arg(&args, 0))?;
            materialize(&shared, id)?;
            let reference = {
                let world = shared.borrow();
                if let Some(unit) = world.btech.constructed_units().get(&id) {
                    unit.definition().reference.clone()
                } else {
                    world.btech.vehicles()[&id].definition().reference.clone()
                }
            };
            let config = crate::lua::configuration(lua);
            let root = config.path(&config.database.mech_database);
            let path = crate::btech::resolve_template_path_cached(
                &mut shared.borrow_mut().btech.template_registry,
                &root,
                &reference,
            )
            .ok()
            .flatten()
            .ok_or_else(|| {
                contract::operation_failure(
                    "template_restore_failed",
                    "unable to restore unit template",
                )
            })?;
            let template = crate::btech::read_resolved_template(&root, &path).map_err(|_| {
                contract::operation_failure(
                    "template_restore_failed",
                    "unable to restore unit template",
                )
            })?;
            if !parts_contract::unit_template_parts_registered(&template) {
                return Err(contract::operation_failure(
                    "template_restore_failed",
                    "unable to restore unit template",
                ));
            }
            crate::btech::load_unit_template(&mut shared.borrow_mut(), id, &reference, template)
                .map_err(|_| {
                    contract::operation_failure(
                        "template_restore_failed",
                        "unable to restore unit template",
                    )
                })?;
            Ok(MultiValue::new())
        })?,
    )?;

    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "unit_contract_save_template",
        GROUP,
        "save_template",
        lua.create_function(move |lua, args: MultiValue| {
            let id = unit(lua, &shared, arg(&args, 0))?;
            let reference = match arg(&args, 1) {
                Value::String(v) if !v.as_bytes().is_empty() && v.as_bytes().len() <= 255 => {
                    c_string(&v)
                }
                _ => {
                    return Err(failure(
                        2,
                        "mux.arg.invalid",
                        "reference must contain 1 to 255 bytes",
                    ));
                }
            };
            contract::validate_resource_name(&reference, "reference", 2)?;
            materialize(&shared, id)?;
            let config = crate::lua::configuration(lua);
            let root = config.path(&config.database.mech_database);
            let source = {
                let world = shared.borrow();
                if let Some(unit) = world.btech.constructed_units().get(&id) {
                    crate::btech::unit_template_source(unit.definition(), &reference)
                } else {
                    crate::btech::vehicle_template_source(
                        world.btech.vehicles()[&id].definition(),
                        &reference,
                    )
                }
            };
            crate::btech::write_template(
                &mut shared.borrow_mut().btech.template_registry,
                &root,
                &reference,
                &source,
            )
            .map_err(|_| {
                contract::operation_failure("template_save_failed", "unable to save unit template")
            })?;
            Ok(MultiValue::new())
        })?,
    )?;

    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "unit_contract_reset_critical_slots",
        GROUP,
        "reset_critical_slots",
        lua.create_function(move |lua, args: MultiValue| {
            let id = unit(lua, &shared, arg(&args, 0))?;
            materialize(&shared, id)?;
            crate::btech::reset_unit_criticals(&mut shared.borrow_mut(), id)
                .map_err(mlua::Error::external)?;
            Ok(MultiValue::new())
        })?,
    )?;

    let shared = world.clone();
    let records = catalogue.clone();
    contract::bind(
        lua,
        native,
        "unit_contract_install_weapon",
        GROUP,
        "install_weapon",
        lua.create_function(move |lua, args: MultiValue| {
            let id = unit(lua, &shared, arg(&args, 0))?;
            let request = table(arg(&args, 1), 2)?;
            contract::check_options(
                &request,
                &[
                    "part",
                    "section",
                    "slots",
                    "rear_facing",
                    "targeting_computer",
                    "one_shot",
                ],
                2,
            )?;
            let part = parts_contract::check_part(contract::field(&request, "part")?, 2, &records)?
                .ok_or_else(|| failure(2, "btech.part.not_found", "part was not found"))?;
            if parts_contract::part_category(part.id) != "weapon" {
                return Err(failure(2, "btech.part.wrong_kind", "part must be a weapon"));
            }
            // Native administration validates the selected manufacturer form, then stores
            // only the part identity.  The eight infantry weapons are intentionally absent
            // from the combat enum, so derive their unbranded C template spelling from the
            // registered fully-qualified name instead of rejecting their raw critical.
            let equipment = crate::BattlePart::from_id(part.id)
                .map(|part| part.name)
                .or_else(|| {
                    parts_contract::part_equipment_name(part).and_then(|name| {
                        name.split_once('.')
                            .map(|(_, equipment)| equipment.to_owned())
                    })
                })
                .ok_or_else(|| failure(2, "btech.part.not_found", "part was not found"))?;
            let section = section(contract::field(&request, "section")?, 2, &shared, id)?;
            let Value::Table(raw) = contract::field(&request, "slots")? else {
                return Err(failure(2, "mux.arg.invalid", "slots must be an array"));
            };
            let required =
                if matches!(section, Section::Vehicle(_)) {
                    1
                } else {
                    usize::from(parts_contract::weapon_critical_slots(part.id).ok_or_else(
                        || failure(2, "btech.part.wrong_kind", "part must be a weapon"),
                    )?)
                };
            if raw.raw_len() == 0
                || raw.raw_len() > 12
                || (raw.raw_len() != required && required < 9)
                || raw.raw_len() > required
            {
                return Err(failure(
                    2,
                    "mux.arg.invalid",
                    "slots must contain the required critical slots",
                ));
            }
            let max = slots(&shared, id, section);
            let mut slots = Vec::new();
            for index in 1..=raw.raw_len() {
                let slot = direct_integer(raw.raw_get(index)?, "slot", 1, max, 2)? as u8 - 1;
                if slots.contains(&slot) {
                    return Err(failure(2, "mux.arg.invalid", "slots must be unique"));
                }
                slots.push(slot);
            }
            let mut modes = Vec::new();
            if optional_bool(&request, "rear_facing", 2)? {
                modes.push("RearMount".into())
            }
            if optional_bool(&request, "targeting_computer", 2)? {
                modes.push("OnTC".into())
            }
            if optional_bool(&request, "one_shot", 2)? {
                modes.push("OneShot".into())
            }
            materialize(&shared, id)?;
            match section {
                Section::Mech(section) => crate::btech::install_unit_weapon_named(
                    &mut shared.borrow_mut(),
                    id,
                    &equipment,
                    0,
                    section,
                    &slots,
                    modes,
                ),
                Section::Vehicle(section) => crate::btech::install_vehicle_weapon_named(
                    &mut shared.borrow_mut(),
                    id,
                    &equipment,
                    0,
                    section,
                    slots[0],
                    modes,
                ),
            }
            .map_err(mlua::Error::external)?;
            Ok(MultiValue::new())
        })?,
    )?;

    let shared = world.clone();
    let records = catalogue.clone();
    contract::bind(
        lua,
        native,
        "unit_contract_configure_ammunition",
        GROUP,
        "configure_ammunition",
        lua.create_function(move |lua, args: MultiValue| {
            let id = unit(lua, &shared, arg(&args, 0))?;
            let request = table(arg(&args, 1), 2)?;
            contract::check_options(
                &request,
                &["weapon", "section", "slot", "half_ton", "ammunition_modes"],
                2,
            )?;
            let part =
                parts_contract::check_part(contract::field(&request, "weapon")?, 2, &records)?
                    .ok_or_else(|| failure(2, "btech.part.not_found", "weapon was not found"))?;
            let weapon = BattleWeapon::from_part_id(part.id).ok_or_else(|| {
                failure(2, "btech.part.wrong_kind", "weapon must identify a weapon")
            })?;
            if weapon.profile().ammunition_per_ton == 0 {
                return Err(failure(
                    2,
                    "mux.arg.invalid",
                    "weapon does not use ammunition",
                ));
            }
            let section = section(contract::field(&request, "section")?, 2, &shared, id)?;
            let max = slots(&shared, id, section);
            let slot = contract::integer_field(&request, "slot", 1, max, 2)? as u8 - 1;
            let bits = named_bits(
                &request,
                "ammunition_modes",
                2,
                &constants::AMMUNITION_MODES,
            )?;
            let half = optional_bool(&request, "half_ton", 2)?;
            let modes = mode_names(&bits);
            materialize(&shared, id)?;
            match section {
                Section::Mech(section) => crate::btech::configure_unit_ammunition(
                    &mut shared.borrow_mut(),
                    id,
                    weapon,
                    0,
                    section,
                    slot,
                    half,
                    modes,
                ),
                Section::Vehicle(section) => crate::btech::configure_vehicle_ammunition(
                    &mut shared.borrow_mut(),
                    id,
                    weapon,
                    0,
                    section,
                    slot,
                    half,
                    modes,
                ),
            }
            .map_err(mlua::Error::external)?;
            Ok(MultiValue::new())
        })?,
    )?;

    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "unit_contract_restock_ammunition",
        GROUP,
        "restock_ammunition",
        lua.create_function(move |lua, args: MultiValue| {
            let id = unit(lua, &shared, arg(&args, 0))?;
            let section = section(arg(&args, 1), 2, &shared, id)?;
            let max = slots(&shared, id, section);
            let slot = direct_integer(arg(&args, 2), "slot", 1, max, 3)? as u8 - 1;
            materialize(&shared, id)?;
            match section {
                Section::Mech(section) => crate::btech::restock_unit_ammunition(
                    &mut shared.borrow_mut(),
                    id,
                    section,
                    slot,
                ),
                Section::Vehicle(section) => crate::btech::restock_vehicle_ammunition(
                    &mut shared.borrow_mut(),
                    id,
                    section,
                    slot,
                ),
            }
            .map_err(|e| {
                if e.to_string().contains("destroyed") {
                    contract::operation_failure(
                        "ammunition_destroyed",
                        "destroyed ammunition cannot be restocked",
                    )
                } else {
                    failure(
                        3,
                        "btech.part.wrong_kind",
                        "critical slot is not ammunition",
                    )
                }
            })?;
            Ok(MultiValue::new())
        })?,
    )?;

    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "unit_contract_set_weapon_modes",
        GROUP,
        "set_weapon_modes",
        lua.create_function(move |lua, args: MultiValue| {
            let id = unit(lua, &shared, arg(&args, 0))?;
            let number =
                direct_integer(arg(&args, 1), "weapon_number", 0, i32::MAX as i64, 2)? as usize;
            let request = table(arg(&args, 2), 3)?;
            contract::check_options(&request, &["fire_modes", "ammunition_modes"], 3)?;
            let fire = named_bits(&request, "fire_modes", 3, &constants::FIRE_MODES)?;
            let ammo = named_bits(
                &request,
                "ammunition_modes",
                3,
                &constants::AMMUNITION_MODES,
            )?;
            materialize(&shared, id)?;
            crate::btech::set_unit_weapon_modes(
                &mut shared.borrow_mut(),
                id,
                number,
                fire_mode(&fire),
                ammo_mode(&ammo),
                fire_mode_names(&fire),
                mode_names(&ammo),
            )
            .map_err(|_| failure(2, "mux.arg.invalid", "weapon number is not mounted"))?;
            Ok(MultiValue::new())
        })?,
    )?;

    let shared = world.clone();
    let records = catalogue;
    contract::bind(
        lua,
        native,
        "unit_contract_install_special",
        GROUP,
        "install_special",
        lua.create_function(move |lua, args: MultiValue| {
            let id = unit(lua, &shared, arg(&args, 0))?;
            let request = table(arg(&args, 1), 2)?;
            contract::check_options(&request, &["part", "section", "slot", "auxiliary_data"], 2)?;
            let raw = contract::field(&request, "part")?;
            let part = if raw == Value::Nil {
                None
            } else {
                Some(
                    parts_contract::check_part(raw, 2, &records)?
                        .ok_or_else(|| failure(2, "btech.part.not_found", "part was not found"))?,
                )
            };
            if part.is_some_and(|p| !(394..=511).contains(&p.id)) {
                return Err(failure(
                    2,
                    "btech.part.wrong_kind",
                    "part must be special equipment",
                ));
            }
            let sec = section(contract::field(&request, "section")?, 2, &shared, id)?;
            let max = slots(&shared, id, sec);
            let slot = contract::integer_field(&request, "slot", 1, max, 2)? as u8 - 1;
            let data = optional_integer(&request, "auxiliary_data", 2)?;
            let equipment = part
                .map(|p| {
                    crate::BattlePart::from_id(p.id)
                        .map(|part| part.name)
                        .or_else(|| {
                            parts_contract::part_equipment_name(p).and_then(|name| {
                                name.split_once('.')
                                    .map(|(_, equipment)| equipment.to_owned())
                            })
                        })
                        .ok_or_else(|| failure(2, "btech.part.not_found", "part was not found"))
                })
                .transpose()?;
            materialize(&shared, id)?;
            match sec {
                Section::Mech(section) => crate::btech::install_unit_special(
                    &mut shared.borrow_mut(),
                    id,
                    equipment,
                    0,
                    section,
                    slot,
                    data,
                ),
                Section::Vehicle(section) => crate::btech::install_vehicle_special(
                    &mut shared.borrow_mut(),
                    id,
                    equipment,
                    0,
                    section,
                    slot,
                    data,
                ),
            }
            .map_err(mlua::Error::external)?;
            Ok(MultiValue::new())
        })?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each round bit combined with the MML family bit selects the matching long-range supply.
    #[test]
    fn mml_family_bit_combines_with_every_long_range_round() {
        for (bit, mode) in [
            (0, BattleAmmunitionMode::MmlLrm),
            (2, BattleAmmunitionMode::MmlLrmArtemis),
            (4, BattleAmmunitionMode::MmlLrmNarc),
            (128, BattleAmmunitionMode::MmlLrmSwarm),
            (256, BattleAmmunitionMode::MmlLrmSwarm1),
            (131072, BattleAmmunitionMode::MmlLrmStinger),
            (524288, BattleAmmunitionMode::MmlLrmSemiGuided),
        ] {
            let bits: Vec<i32> = [bit, 4194304].into_iter().filter(|&bit| bit != 0).collect();
            assert_eq!(ammo_mode(&bits), mode, "{bits:?}");
        }
        assert_eq!(ammo_mode(&[2]), BattleAmmunitionMode::Artemis);
        assert_eq!(ammo_mode(&[4]), BattleAmmunitionMode::Narc);
        // SRM-only rounds fall back to the plain long-range supply.
        assert_eq!(ammo_mode(&[64, 4194304]), BattleAmmunitionMode::MmlLrm);
    }
}
