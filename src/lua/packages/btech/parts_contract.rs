//! C-compatible loose-part catalogue, lookup, store, and cost bindings.

use super::{contract, error};
use crate::{BattlePart, BattlePartForm, SharedWorld, World};
use mlua::{Lua, LuaString, MultiValue, Table, Value};
use std::sync::Arc;

const GROUP: &str = "parts";
const ITEM_COUNT: i64 = 1024;
const BRAND_COUNT: i64 = 5;
const LUA_SAFE_INTEGER_MAX: i64 = 9_007_199_254_740_991;

/// Stable native part and manufacturer identity shared by all BattleTech projections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PartReference {
    pub id: i32,
    pub brand: u8,
}

fn argument_failure(argument: usize, code: &'static str, message: impl ToString) -> mlua::Error {
    error::failure_with_detail(code, message, serde_json::json!({ "argument": argument }))
}

fn value(arguments: &MultiValue, index: usize) -> Value {
    arguments.get(index).cloned().unwrap_or(Value::Nil)
}

fn form_for(catalogue: &[BattlePartForm], part: PartReference) -> Option<&BattlePartForm> {
    catalogue
        .iter()
        .find(|form| form.part_id == part.id && form.brand_id == part.brand)
}

fn bytes_equal_case_insensitive(left: &[u8], right: &str) -> bool {
    left.eq_ignore_ascii_case(right.as_bytes())
}

fn c_bytes(value: &LuaString) -> Vec<u8> {
    let bytes = value.as_bytes();
    let length = bytes
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(bytes.len());
    bytes[..length].to_vec()
}

/// Decode a packed ID, exact catalogue name, or record without rejecting projection fields.
pub(super) fn check_part(
    value: Value,
    argument: usize,
    catalogue: &[BattlePartForm],
) -> mlua::Result<Option<PartReference>> {
    let part = match value {
        Value::Integer(packed) => {
            if !(0..=i32::MAX as i64).contains(&packed) {
                return Err(argument_failure(
                    argument,
                    "mux.arg.invalid",
                    "packed part ID must be a nonnegative integer",
                ));
            }
            let brand = packed / ITEM_COUNT;
            if brand > BRAND_COUNT {
                return Ok(None);
            }
            PartReference {
                id: (packed % ITEM_COUNT) as i32,
                brand: brand as u8,
            }
        }
        Value::Number(packed) => {
            if !packed.is_finite()
                || packed.fract() != 0.0
                || !(0.0..=i32::MAX as f64).contains(&packed)
            {
                return Err(argument_failure(
                    argument,
                    "mux.arg.invalid",
                    "packed part ID must be a nonnegative integer",
                ));
            }
            let packed = packed as i64;
            let brand = packed / ITEM_COUNT;
            if brand > BRAND_COUNT {
                return Ok(None);
            }
            PartReference {
                id: (packed % ITEM_COUNT) as i32,
                brand: brand as u8,
            }
        }
        Value::String(name) => {
            let name = c_bytes(&name);
            let mut found = None;
            for form in catalogue.iter().filter(|form| {
                bytes_equal_case_insensitive(&name, &form.short_name)
                    || bytes_equal_case_insensitive(&name, &form.long_name)
                    || bytes_equal_case_insensitive(&name, &form.very_long_name)
            }) {
                let candidate = PartReference {
                    id: form.part_id,
                    brand: form.brand_id,
                };
                if found.is_some_and(|found| found != candidate) {
                    return Err(argument_failure(
                        argument,
                        "btech.part.ambiguous",
                        "part name is ambiguous",
                    ));
                }
                found = Some(candidate);
            }
            return Ok(found);
        }
        Value::Table(record) => PartReference {
            id: contract::integer_field(&record, "id", 0, ITEM_COUNT - 1, argument)? as i32,
            brand: contract::integer_field(&record, "brand", 0, BRAND_COUNT, argument)? as u8,
        },
        _ => {
            return Err(argument_failure(
                argument,
                "mux.arg.invalid",
                "part must be a packed ID, name, or part record",
            ));
        }
    };
    Ok(form_for(catalogue, part).map(|_| part))
}

fn require_part(
    value: Value,
    argument: usize,
    catalogue: &[BattlePartForm],
) -> mlua::Result<PartReference> {
    check_part(value, argument, catalogue)?
        .ok_or_else(|| argument_failure(argument, "btech.part.not_found", "part is not registered"))
}

pub(super) fn part_category(id: i32) -> &'static str {
    match id {
        1..=192 => "weapon",
        193..=384 => "ammunition",
        385..=393 => "bomb",
        394..=511 => "special",
        512..=1023 => "cargo",
        _ => "other",
    }
}

/// C catalogue slot count for weapon identities, including raw infantry weapons
/// that the combat enum does not yet simulate.
pub(super) fn weapon_critical_slots(id: i32) -> Option<u8> {
    weapon_contract(id).and_then(|weapon| u8::try_from(weapon.slots).ok())
}

/// Fully qualified spelling used by C template files for a registered form.
pub(super) fn part_equipment_name(part: PartReference) -> Option<String> {
    form_for(&registered_catalogue(), part).map(|form| form.very_long_name.clone())
}

/// Resolve every authored critical through the native very-long-name registry.
/// The pinned C registry installs manufacturer-qualified rows only
/// (create_brandname returns early for brand zero), so templates naming
/// legacy unbranded parts are malformed there; this mirrors that gate.
pub(super) fn normalize_raw_template_parts(template: &mut crate::RawTemplate) -> bool {
    let catalogue = registered_catalogue();
    for critical in template
        .sections
        .values_mut()
        .flat_map(|section| section.criticals.values_mut())
    {
        let Some(form) = catalogue.iter().find(|form| {
            form.very_long_name
                .eq_ignore_ascii_case(&critical.equipment)
        }) else {
            return false;
        };
        critical.equipment = crate::btech::BattlePart::from_id(form.part_id)
            .map_or_else(|| critical.equipment.clone(), |part| part.name);
        if critical.brand.unwrap_or_default() == 0 {
            critical.brand = Some(form.brand_id);
        }
    }
    true
}

/// The C loader resolves every authored critical through the very-long part
/// registry before construction (template_load part_match_next), rejecting the
/// whole template when any name is unregistered.
pub(super) fn unit_template_parts_registered(template: &crate::BattleUnitTemplate) -> bool {
    let catalogue = registered_catalogue();
    let registered = |critical: &crate::CriticalDefinition| {
        catalogue.iter().any(|form| {
            form.very_long_name
                .eq_ignore_ascii_case(&critical.equipment)
        })
    };
    match template {
        crate::BattleUnitTemplate::Mech(mech) => mech
            .sections
            .values()
            .flat_map(|layout| layout.criticals.values())
            .all(registered),
        crate::BattleUnitTemplate::Vehicle(vehicle) => vehicle
            .sections
            .values()
            .flat_map(|layout| layout.criticals.values())
            .all(registered),
    }
}

fn optional_category(value: Value, argument: usize) -> mlua::Result<Option<&'static str>> {
    if value.is_nil() {
        return Ok(None);
    }
    let Value::String(value) = value else {
        return Err(argument_failure(
            argument,
            "mux.arg.invalid",
            "category must be a string",
        ));
    };
    for candidate in ["weapon", "ammunition", "bomb", "special", "cargo", "other"] {
        if c_bytes(&value).eq_ignore_ascii_case(candidate.as_bytes()) {
            return Ok(Some(candidate));
        }
    }
    Err(argument_failure(
        argument,
        "mux.arg.invalid",
        "unknown part category",
    ))
}

fn wildcard(pattern: &[u8], text: &[u8]) -> bool {
    let (mut p, mut t, mut star, mut retry) = (0, 0, None, 0);
    while t < text.len() {
        if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            p += 1;
            retry = t;
            continue;
        }
        let escaped = p < pattern.len() && pattern[p] == b'\\' && p + 1 < pattern.len();
        let literal = if escaped { p + 1 } else { p };
        if literal < pattern.len()
            && ((!escaped && pattern[literal] == b'?')
                || pattern[literal].eq_ignore_ascii_case(&text[t]))
        {
            p = literal + 1;
            t += 1;
            continue;
        }
        let Some(star) = star else { return false };
        retry += 1;
        t = retry;
        p = star + 1;
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

#[derive(Clone, Copy)]
struct WeaponContract {
    kind: &'static str,
    heat: i32,
    damage: i32,
    minimum: i32,
    short: i32,
    medium: i32,
    long: i32,
    slots: i32,
    ammunition: i32,
    recycle: i32,
    battle_value: i32,
    mass: u32,
}

macro_rules! weapon_contracts {
    ($(($kind:literal,$heat:literal,$damage:literal,$minimum:literal,$short:literal,$medium:literal,$long:literal,$slots:literal,$ammunition:literal,$recycle:literal,$battle_value:literal,$mass:literal)),+ $(,)?) => {
        &[ $(WeaponContract { kind: $kind, heat: $heat, damage: $damage, minimum: $minimum, short: $short, medium: $medium, long: $long, slots: $slots, ammunition: $ammunition, recycle: $recycle, battle_value: $battle_value, mass: $mass }),+ ]
    };
}

const WEAPONS: &[WeaponContract] = weapon_contracts! {
    ("energy",0,0,0,1,1,1,1,0,30,1,512),
    ("missile",2,2,0,4,8,12,1,50,15,40,1024),
    ("missile",3,2,0,4,8,12,1,25,15,79,2048),
    ("missile",4,2,0,4,8,12,2,15,15,119,3072),
    ("artillery",20,20,0,0,0,20,30,5,60,171,30720),
    ("ballistic",0,32,0,24,48,72,1,30,10,1,0),
    ("ballistic",0,9,0,9,18,27,1,20,10,1,0),
    ("ballistic",0,9,0,22,44,66,1,50,10,1,0),
    ("ballistic",0,7,0,4,9,13,1,50,7,1,0),
    ("ballistic",0,14,0,30,60,90,1,15,10,1,0),
    ("ballistic",0,27,0,35,70,105,1,10,10,1,0),
    ("ballistic",0,9,0,40,80,120,1,45,10,1,0),
    ("ballistic",0,17,0,24,48,72,1,30,10,1,0),
    ("ballistic",0,11,0,5,10,15,1,50,10,1,0),
    ("ballistic",0,13,0,22,44,66,1,30,10,1,0),
    ("ballistic",0,10,0,7,14,20,1,50,5,1,0),
    ("ballistic",0,12,0,6,12,18,1,10,15,1,0),
    ("ballistic",0,16,0,7,14,20,1,50,10,1,0),
    ("ballistic",0,18,0,8,16,24,1,50,7,1,0),
    ("ballistic",0,21,0,19,38,57,1,30,10,1,0),
    ("missile",3,1,10,12,24,36,1,18,30,1000,6144),
    ("missile",6,1,10,12,24,36,4,9,30,1000,8192),
    ("missile",8,1,10,12,24,36,6,6,30,1000,12288),
    ("missile",10,1,10,12,24,36,8,4,30,1000,18432),
    ("missile",2,2,4,6,12,18,1,24,15,1000,2048),
    ("missile",4,2,4,6,12,18,2,12,20,1000,5120),
    ("missile",5,2,4,6,12,18,3,8,25,1000,7168),
    ("missile",6,2,4,6,12,18,5,6,30,1000,10240),
    ("missile",2,3,0,2,4,6,1,50,15,1000,1024),
    ("missile",3,3,0,2,4,6,1,25,15,1000,2048),
    ("missile",4,3,0,2,4,6,2,15,15,1000,3072),
    ("ballistic",1,2,3,10,20,35,4,30,12,1000,8192),
    ("ballistic",3,5,0,8,16,28,5,15,20,1000,12288),
    ("ballistic",7,10,0,6,12,20,6,8,25,1000,14336),
    ("energy",12,10,0,8,15,25,1,0,20,249,4096),
    ("energy",5,7,0,5,10,15,1,0,15,108,1024),
    ("energy",2,5,0,2,4,6,1,0,10,31,512),
    ("energy",1,2,0,1,2,4,1,0,15,7,256),
    ("energy",15,15,0,7,14,23,2,0,25,412,6144),
    ("energy",3,2,0,1,2,3,1,0,10,6,512),
    ("energy",18,16,0,5,10,15,3,0,30,243,4096),
    ("energy",7,10,0,3,6,9,2,0,25,76,1024),
    ("energy",3,6,0,1,2,3,1,0,20,15,512),
    ("energy",10,10,0,6,14,20,2,0,23,265,6144),
    ("energy",4,7,0,4,8,12,1,0,18,111,2048),
    ("energy",2,3,0,2,4,6,1,0,13,24,1024),
    ("energy",1,3,0,1,2,3,1,0,15,12,512),
    ("energy",13,10,0,7,15,23,3,0,30,271,6144),
    ("energy",6,7,0,5,9,14,2,0,30,116,2048),
    ("energy",3,5,0,2,4,6,1,0,30,36,1536),
    ("missile",1,2,0,1,1,1,1,24,10,63,512),
    ("ballistic",1,15,2,7,15,22,6,8,30,321,12288),
    ("ballistic",1,2,4,10,20,30,3,45,15,47,5120),
    ("ballistic",1,5,3,8,15,24,4,20,20,93,7168),
    ("ballistic",2,10,0,6,12,18,5,10,25,148,10240),
    ("ballistic",6,20,0,4,8,12,9,5,30,237,12288),
    ("ballistic",0,2,0,1,2,3,1,200,7,5,256),
    ("ballistic",0,1,0,2,4,6,1,200,7,5,256),
    ("ballistic",0,3,0,1,2,3,1,100,7,6,512),
    ("ballistic",1,2,2,9,18,27,2,45,12,62,5120),
    ("ballistic",1,5,0,7,14,21,3,20,20,123,7168),
    ("ballistic",3,10,0,6,12,18,4,10,25,211,10240),
    ("ballistic",7,20,0,4,8,12,8,5,30,337,12288),
    ("artillery",10,20,0,0,0,6,12,5,60,171,12288),
    ("missile",2,2,4,5,10,15,2,20,15,53,1536),
    ("missile",4,2,4,5,10,15,3,10,20,105,3584),
    ("missile",6,2,4,5,10,15,4,7,25,147,5120),
    ("missile",8,2,4,5,10,15,5,5,30,212,7168),
    ("missile",2,1,0,7,14,21,1,24,15,55,1024),
    ("missile",4,1,0,7,14,21,1,12,20,109,2560),
    ("missile",5,1,0,7,14,21,2,8,25,164,3584),
    ("missile",6,1,0,7,14,21,4,6,20,220,5120),
    ("missile",1,4,0,4,8,12,1,6,30,30,2048),
    ("missile",2,2,0,3,6,9,1,50,15,21,512),
    ("missile",3,2,0,3,6,9,1,25,15,39,1024),
    ("missile",4,2,0,3,6,9,1,15,15,59,1536),
    ("energy",10,10,3,6,12,18,3,0,30,176,7168),
    ("energy",8,8,0,5,10,15,2,0,25,124,5120),
    ("energy",3,5,0,3,6,9,1,0,20,46,1024),
    ("energy",1,3,0,1,2,3,1,0,15,9,512),
    ("energy",3,2,0,1,2,3,1,0,10,6,1024),
    ("energy",15,10,0,7,14,23,3,0,30,229,7168),
    ("energy",12,8,0,7,14,19,2,0,25,163,5120),
    ("energy",5,5,0,4,8,12,1,0,20,62,1024),
    ("energy",2,3,0,2,4,5,1,0,15,17,512),
    ("energy",10,9,0,3,7,10,2,0,25,119,7168),
    ("energy",4,6,0,2,4,6,1,0,20,48,2048),
    ("energy",2,3,0,1,2,3,1,0,15,12,1024),
    ("energy",14,9,0,5,10,15,2,0,27,178,7168),
    ("energy",6,6,0,3,6,9,1,0,22,71,2048),
    ("energy",3,3,0,2,4,5,1,0,17,21,1024),
    ("energy",10,10,0,9,13,15,2,0,30,165,6144),
    ("energy",5,5,3,6,12,18,2,0,30,88,3072),
    ("energy",15,15,3,6,12,18,4,0,30,317,10240),
    ("ballistic",1,2,4,8,16,24,1,45,12,37,6144),
    ("ballistic",1,5,3,6,12,18,4,20,20,70,8192),
    ("ballistic",3,10,0,5,10,15,7,10,25,124,12288),
    ("ballistic",7,20,0,3,6,9,10,5,30,178,14336),
    ("ballistic",3,2,0,1,2,3,1,20,10,5,512),
    ("ballistic",0,2,0,1,2,3,1,200,7,5,512),
    ("missile",1,2,0,1,1,1,1,12,10,32,512),
    ("ballistic",1,15,2,7,15,22,7,8,30,321,15360),
    ("ballistic",1,8,3,8,17,25,5,16,20,159,12288),
    ("ballistic",2,25,4,6,13,20,11,4,30,346,18432),
    ("ballistic",1,2,4,9,18,27,4,45,15,42,6144),
    ("ballistic",1,5,3,7,14,21,5,20,20,83,8192),
    ("ballistic",2,10,0,6,12,18,6,10,25,148,11264),
    ("ballistic",6,20,0,4,8,12,11,5,30,237,14336),
    ("ballistic",1,2,0,6,12,18,3,45,15,118,8192),
    ("ballistic",1,5,0,5,10,15,6,20,22,247,10240),
    ("ballistic",1,2,4,8,17,25,3,45,12,56,7168),
    ("ballistic",1,5,2,6,13,20,5,20,20,113,9216),
    ("ballistic",4,10,0,6,12,18,7,10,25,253,13312),
    ("ballistic",8,20,0,3,7,10,10,5,30,282,15360),
    ("artillery",10,20,0,0,0,5,15,5,60,171,15360),
    ("artillery",10,10,0,0,0,12,20,10,60,86,20480),
    ("artillery",6,5,0,0,0,14,15,20,60,40,15360),
    ("ballistic",5,4,0,2,4,6,1,10,15,20,1024),
    ("ballistic",1,2,0,6,12,18,1,45,12,30,4096),
    ("ballistic",1,5,0,5,10,15,2,20,20,62,5120),
    ("artillery",20,20,4,6,13,20,15,5,30,348,20480),
    ("artillery",10,10,2,4,8,12,10,10,25,115,15360),
    ("artillery",6,5,3,4,9,14,7,20,25,58,10240),
    ("ballistic",0,2,0,2,4,6,1,100,7,6,1024),
    ("ballistic",10,10,0,5,10,15,2,10,30,210,6144),
    ("missile",2,1,0,3,6,9,2,33,30,29,1536),
    ("missile",3,1,0,3,6,9,3,20,30,45,3072),
    ("missile",4,1,0,3,6,9,4,14,30,67,4608),
    ("missile",5,1,0,3,6,9,5,11,30,86,6144),
    ("missile",2,1,6,7,14,21,1,24,15,45,2048),
    ("missile",4,1,6,7,14,21,2,12,20,90,5120),
    ("missile",5,1,6,7,14,21,3,8,25,136,7168),
    ("missile",6,1,6,7,14,21,5,6,30,181,10240),
    ("missile",2,2,0,3,6,9,1,50,15,21,1024),
    ("missile",3,2,0,3,6,9,1,25,15,39,2048),
    ("missile",4,2,0,3,6,9,2,15,15,59,3072),
    ("missile",4,1,0,3,8,15,2,24,20,56,3072),
    ("missile",6,1,0,3,8,15,3,12,30,112,7168),
    ("missile",10,1,0,3,8,15,5,8,30,168,10240),
    ("missile",12,1,0,3,8,15,7,6,30,224,12288),
    ("missile",1,4,0,3,6,9,2,6,30,30,3072),
    ("missile",1,6,0,4,9,15,3,4,30,75,5120),
    ("missile",2,2,0,3,6,9,1,50,15,30,1536),
    ("missile",3,2,0,3,6,9,1,25,15,59,3072),
    ("missile",4,2,0,3,6,9,2,15,15,89,4608),
    ("missile",3,1,0,5,11,18,1,0,30,18,512),
    ("missile",4,1,0,4,9,15,2,0,30,23,1024),
    ("missile",5,1,0,3,7,12,3,0,30,24,1536),
    ("missile",3,5,5,6,12,18,1,12,20,64,3072),
    ("missile",5,10,5,6,12,18,2,6,20,127,7168),
    ("missile",7,15,5,6,12,18,3,4,30,229,11264),
    ("missile",8,20,5,6,12,18,5,3,30,305,15360),
    ("melee",0,5,0,1,1,1,1,0,3,1,0),
    ("melee",0,7,0,1,1,1,1,0,3,1,0),
    ("missile",2,1,6,7,14,21,1,24,15,87,2048),
    ("missile",4,1,6,7,14,21,2,12,20,173,5120),
    ("missile",5,1,6,7,14,21,3,8,25,260,7168),
    ("missile",6,1,6,7,14,21,5,6,30,346,10240),
    ("energy",0,0,0,1,1,1,1,0,30,1,512),
    ("ballistic",0,2,0,3,6,9,2,50,12,15,512),
    ("ballistic",0,3,0,1,2,3,1,25,15,15,1024),
    ("ballistic",3,3,0,1,2,3,2,10,25,30,1536),
    ("ballistic",5,4,0,2,4,6,1,20,10,20,1024),
    ("ballistic",1,2,2,9,18,27,4,45,10,75,7168),
    ("ballistic",1,5,0,7,14,21,5,20,15,150,10240),
    ("ballistic",3,10,0,6,12,18,7,10,20,250,14336),
    ("ballistic",7,20,0,4,8,12,10,5,25,400,16384),
    ("energy",15,10,0,7,14,22,2,0,25,400,6144),
    ("energy",1,2,0,1,1,1,1,24,25,105,512),
    ("energy",12,2,0,1,1,1,1,24,25,105,512),
    ("ballistic",0,1,0,1,2,2,1,20,7,5,512),
    ("ballistic",0,1,0,1,2,3,1,10,10,7,768),
    ("ballistic",0,2,0,1,2,4,1,5,15,9,1024),
    ("ballistic",0,1,0,1,2,3,1,20,5,8,1024),
    ("energy",1,2,0,1,2,3,1,0,10,9,768),
    ("energy",1,1,0,1,2,3,1,0,10,5,768),
    ("missile",1,1,0,2,4,6,1,2,20,12,1024),
    ("missile",1,1,4,6,9,12,1,1,20,22,1024),
    ("missile",2,1,3,7,14,21,2,24,15,67,3072),
    ("missile",4,1,3,7,14,21,4,12,20,104,6144),
    ("missile",5,1,3,7,14,21,6,8,25,157,9216),
    ("missile",6,1,3,7,14,21,9,6,30,210,12288),
};

fn weapon_contract(id: i32) -> Option<&'static WeaponContract> {
    usize::try_from(id - 1)
        .ok()
        .and_then(|index| WEAPONS.get(index))
}

fn push_weapon(lua: &Lua, id: i32) -> mlua::Result<Table> {
    let weapon = weapon_contract(id)
        .ok_or_else(|| error::failure("mux.internal", "weapon catalogue mismatch"))?;
    let result = lua.create_table()?;
    result.raw_set("kind", weapon.kind)?;
    result.raw_set("heat", weapon.heat)?;
    result.raw_set("damage", weapon.damage)?;
    result.raw_set("minimum_range", weapon.minimum)?;
    result.raw_set("short_range", weapon.short)?;
    result.raw_set("medium_range", weapon.medium)?;
    result.raw_set("long_range", weapon.long)?;
    result.raw_set("critical_slots", weapon.slots)?;
    result.raw_set("ammunition_per_ton", weapon.ammunition)?;
    result.raw_set("recycle_time", weapon.recycle)?;
    result.raw_set("battle_value", weapon.battle_value)?;
    Ok(result)
}

/// Build the detached C record for one registered part identity.
pub(super) fn push_part(
    lua: &Lua,
    world: &World,
    catalogue: &[BattlePartForm],
    part: PartReference,
) -> mlua::Result<Table> {
    let form = (part.brand != 0)
        .then(|| form_for(catalogue, part))
        .flatten();
    let native = BattlePart::from_id(part.id)
        .ok_or_else(|| error::failure("mux.internal", "part catalogue mismatch"))?;
    let cost = crate::btech::part_cost(world, part.id)
        .map_err(|failure| error::failure("mux.internal", failure))?;
    if cost > LUA_SAFE_INTEGER_MAX as u64 {
        return Err(error::failure(
            "mux.internal",
            "part cost is not representable in Lua",
        ));
    }
    let result = lua.create_table()?;
    result.raw_set("id", part.id)?;
    result.raw_set("brand", part.brand)?;
    result.raw_set(
        "packed_id",
        i64::from(part.brand) * ITEM_COUNT + i64::from(part.id),
    )?;
    if let Some(form) = form {
        result.raw_set("short_name", form.short_name.as_str())?;
        result.raw_set("long_name", form.long_name.as_str())?;
        result.raw_set("very_long_name", form.very_long_name.as_str())?;
    }
    result.raw_set("category", part_category(part.id))?;
    let mass = weapon_contract(part.id).map_or(native.mass, |weapon| weapon.mass);
    result.raw_set("weight_tons", f64::from(mass) / 1024.0)?;
    result.raw_set("cost", cost as i64)?;
    if part_category(part.id) == "weapon" {
        result.raw_set("weapon", push_weapon(lua, part.id)?)?;
    }
    Ok(result)
}

fn categories(lua: &Lua) -> mlua::Result<Table> {
    let result = lua.create_table()?;
    for (index, (code, name)) in [
        ("weapon", "Weapons"),
        ("ammunition", "Ammunition"),
        ("bomb", "Bombs"),
        ("special", "Special Equipment"),
        ("cargo", "Cargo"),
        ("other", "Other"),
    ]
    .into_iter()
    .enumerate()
    {
        let row = lua.create_table()?;
        row.raw_set("code", code)?;
        row.raw_set("name", name)?;
        result.raw_set(index + 1, row)?;
    }
    Ok(result)
}

fn c_infantry_forms() -> Vec<BattlePartForm> {
    const ROWS: &[(i32, u8, &str, &str, &str)] = &[
        (
            175,
            5,
            "Agra.IL",
            "Agra.InfantryLaser",
            "Agra.IS.InfantryLaser",
        ),
        (
            173,
            5,
            "Ar.HIR",
            "Armstrong.HeavyInfantryRifle",
            "Armstrong.IS.HeavyInfantryRifle",
        ),
        (
            174,
            5,
            "Ar.IMG",
            "Armstrong.InfantryMachineGun",
            "Armstrong.IS.InfantryMachineGun",
        ),
        (
            172,
            5,
            "Ar.IR",
            "Armstrong.InfantryRifle",
            "Armstrong.IS.InfantryRifle",
        ),
        (
            171,
            5,
            "Ar.LIR",
            "Armstrong.LightInfantryRifle",
            "Armstrong.IS.LightInfantryRifle",
        ),
        (
            178,
            3,
            "Bi.ILRM",
            "Bical.InfantryLRM",
            "Bical.IS.InfantryLRM",
        ),
        (
            177,
            3,
            "Bi.ISRM",
            "Bical.InfantrySRM",
            "Bical.IS.InfantrySRM",
        ),
        (
            178,
            1,
            "Co.ILRM",
            "Coventry.InfantryLRM",
            "Coventry.IS.InfantryLRM",
        ),
        (
            177,
            1,
            "Co.ISRM",
            "Coventry.InfantrySRM",
            "Coventry.IS.InfantrySRM",
        ),
        (
            173,
            4,
            "De.HIR",
            "Deprus.HeavyInfantryRifle",
            "Deprus.IS.HeavyInfantryRifle",
        ),
        (
            174,
            4,
            "De.IMG",
            "Deprus.InfantryMachineGun",
            "Deprus.IS.InfantryMachineGun",
        ),
        (
            172,
            4,
            "De.IR",
            "Deprus.InfantryRifle",
            "Deprus.IS.InfantryRifle",
        ),
        (
            171,
            4,
            "De.LIR",
            "Deprus.LightInfantryRifle",
            "Deprus.IS.LightInfantryRifle",
        ),
        (
            176,
            3,
            "Fi.IF",
            "Firestorm.InfantryFlamer",
            "Firestorm.IS.InfantryFlamer",
        ),
        (
            175,
            2,
            "He.IL",
            "Hesperus.InfantryLaser",
            "Hesperus.IS.InfantryLaser",
        ),
        (
            176,
            2,
            "Ho.IF",
            "Hotshot.InfantryFlamer",
            "Hotshot.IS.InfantryFlamer",
        ),
        (
            178,
            4,
            "Ho.ILRM",
            "Holly.InfantryLRM",
            "Holly.IS.InfantryLRM",
        ),
        (
            177,
            4,
            "Ho.ISRM",
            "Holly.InfantrySRM",
            "Holly.IS.InfantrySRM",
        ),
        (
            175,
            1,
            "Lo.IL",
            "Lords.InfantryLaser",
            "Lords.IS.InfantryLaser",
        ),
        (
            173,
            1,
            "Lu.HIR",
            "Luxor.HeavyInfantryRifle",
            "Luxor.IS.HeavyInfantryRifle",
        ),
        (
            174,
            1,
            "Lu.IMG",
            "Luxor.InfantryMachineGun",
            "Luxor.IS.InfantryMachineGun",
        ),
        (
            172,
            1,
            "Lu.IR",
            "Luxor.InfantryRifle",
            "Luxor.IS.InfantryRifle",
        ),
        (
            171,
            1,
            "Lu.LIR",
            "Luxor.LightInfantryRifle",
            "Luxor.IS.LightInfantryRifle",
        ),
        (
            175,
            3,
            "Ma.IL",
            "Martell.InfantryLaser",
            "Martell.IS.InfantryLaser",
        ),
        (
            175,
            4,
            "Ma.IL",
            "Magna.InfantryLaser",
            "Magna.IS.InfantryLaser",
        ),
        (
            173,
            3,
            "Or.HIR",
            "Oriente.HeavyInfantryRifle",
            "Oriente.IS.HeavyInfantryRifle",
        ),
        (
            174,
            3,
            "Or.IMG",
            "Oriente.InfantryMachineGun",
            "Oriente.IS.InfantryMachineGun",
        ),
        (
            172,
            3,
            "Or.IR",
            "Oriente.InfantryRifle",
            "Oriente.IS.InfantryRifle",
        ),
        (
            171,
            3,
            "Or.LIR",
            "Oriente.LightInfantryRifle",
            "Oriente.IS.LightInfantryRifle",
        ),
        (
            176,
            4,
            "Pu.IF",
            "Purity.InfantryFlamer",
            "Purity.IS.InfantryFlamer",
        ),
        (
            176,
            1,
            "Py.IF",
            "Pynes.InfantryFlamer",
            "Pynes.IS.InfantryFlamer",
        ),
        (
            173,
            2,
            "SB.HIR",
            "SperryBrowning.HeavyInfantryRifle",
            "SperryBrowning.IS.HeavyInfantryRifle",
        ),
        (
            174,
            2,
            "SB.IMG",
            "SperryBrowning.InfantryMachineGun",
            "SperryBrowning.IS.InfantryMachineGun",
        ),
        (
            172,
            2,
            "SB.IR",
            "SperryBrowning.InfantryRifle",
            "SperryBrowning.IS.InfantryRifle",
        ),
        (
            171,
            2,
            "SB.LIR",
            "SperryBrowning.LightInfantryRifle",
            "SperryBrowning.IS.LightInfantryRifle",
        ),
        (
            178,
            2,
            "Sh.ILRM",
            "Shannon.InfantryLRM",
            "Shannon.IS.InfantryLRM",
        ),
        (
            177,
            2,
            "Sh.ISRM",
            "Shannon.InfantrySRM",
            "Shannon.IS.InfantrySRM",
        ),
        (
            178,
            5,
            "Te.ILRM",
            "Telos.InfantryLRM",
            "Telos.IS.InfantryLRM",
        ),
        (
            177,
            5,
            "Te.ISRM",
            "Telos.InfantrySRM",
            "Telos.IS.InfantrySRM",
        ),
        (
            176,
            5,
            "Ve.IF",
            "Ventra.InfantryFlamer",
            "Ventra.IS.InfantryFlamer",
        ),
    ];
    ROWS.iter()
        .map(
            |&(part_id, brand_id, short, long, very_long)| BattlePartForm {
                part_id,
                brand_id,
                short_name: short.into(),
                long_name: long.into(),
                very_long_name: very_long.into(),
            },
        )
        .collect()
}

/// Exact C part-name registry, excluding unregistered brand-zero identities.
pub(super) fn registered_catalogue() -> Vec<BattlePartForm> {
    let mut catalogue = crate::btech::part_catalogue()
        .into_iter()
        .filter(|form| form.brand_id != 0)
        .collect::<Vec<_>>();
    catalogue.extend(c_infantry_forms());
    catalogue.sort_by(|left, right| {
        (&left.short_name, left.brand_id, left.part_id).cmp(&(
            &right.short_name,
            right.brand_id,
            right.part_id,
        ))
    });
    catalogue
}

/// Register all eight C-native `btech.parts` operations.
pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    // The native registry currently contains only manufacturer-qualified rows:
    // create_brandname rejects brand zero before formatting any component or
    // commodity name. Raw installed-part projection remains separate and may
    // still describe unregistered brand-zero identities.
    let catalogue = Arc::new(registered_catalogue());
    contract::bind(
        lua,
        native,
        "parts_contract_categories",
        GROUP,
        "categories",
        lua.create_function(|lua, _: MultiValue| categories(lua))?,
    )?;

    let shared = world.clone();
    let records = catalogue.clone();
    contract::bind(
        lua,
        native,
        "parts_contract_list",
        GROUP,
        "list",
        lua.create_function(move |lua, arguments: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let selected = optional_category(value(&arguments, 0), 1)?;
            let world = shared.borrow();
            let result = lua.create_table()?;
            for (output, form) in (1..).zip(records.iter().filter(|form| {
                selected.is_none_or(|selected| selected == part_category(form.part_id))
            })) {
                result.raw_set(
                    output,
                    push_part(
                        lua,
                        &world,
                        &records,
                        PartReference {
                            id: form.part_id,
                            brand: form.brand_id,
                        },
                    )?,
                )?;
            }
            Ok(result)
        })?,
    )?;

    let shared = world.clone();
    let records = catalogue.clone();
    contract::bind(
        lua,
        native,
        "parts_contract_search",
        GROUP,
        "search",
        lua.create_function(move |lua, arguments: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let Value::String(query) = value(&arguments, 0) else {
                return Err(argument_failure(
                    1,
                    "mux.arg.invalid",
                    "query must be a non-empty string",
                ));
            };
            let query = c_bytes(&query);
            if query.is_empty() {
                return Err(argument_failure(
                    1,
                    "mux.arg.invalid",
                    "query must be a non-empty string",
                ));
            }
            let world = shared.borrow();
            let result = lua.create_table()?;
            for (output, form) in (1..).zip(records.iter().filter(|form| {
                wildcard(&query, form.short_name.as_bytes())
                    || wildcard(&query, form.long_name.as_bytes())
                    || wildcard(&query, form.very_long_name.as_bytes())
            })) {
                result.raw_set(
                    output,
                    push_part(
                        lua,
                        &world,
                        &records,
                        PartReference {
                            id: form.part_id,
                            brand: form.brand_id,
                        },
                    )?,
                )?;
            }
            Ok(result)
        })?,
    )?;

    let shared = world.clone();
    let records = catalogue.clone();
    contract::bind(
        lua,
        native,
        "parts_contract_resolve",
        GROUP,
        "resolve",
        lua.create_function(move |lua, arguments: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let world = shared.borrow();
            check_part(value(&arguments, 0), 1, &records)?.map_or(Ok(Value::Nil), |part| {
                push_part(lua, &world, &records, part).map(Value::Table)
            })
        })?,
    )?;

    let shared = world.clone();
    let records = catalogue.clone();
    contract::bind(
        lua,
        native,
        "parts_contract_stores",
        GROUP,
        "stores",
        lua.create_function(move |lua, arguments: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let world = shared.borrow();
            let object = contract::require_object(lua, &world, value(&arguments, 0), 1)?;
            let result = lua.create_table()?;
            let mut output = 1;
            for entry in crate::btech::inventory(&world, object)
                .map_err(|failure| error::failure("btech.operation.failed", failure))?
            {
                let part = PartReference {
                    id: entry.part_id,
                    brand: entry.brand_id,
                };
                if entry.quantity <= 0 || form_for(&records, part).is_none() {
                    continue;
                }
                let row = lua.create_table()?;
                row.raw_set("part", push_part(lua, &world, &records, part)?)?;
                row.raw_set("quantity", entry.quantity)?;
                result.raw_set(output, row)?;
                output += 1;
            }
            Ok(result)
        })?,
    )?;

    let shared = world.clone();
    let records = catalogue.clone();
    contract::bind(
        lua,
        native,
        "parts_contract_store_quantity",
        GROUP,
        "store_quantity",
        lua.create_function(move |lua, arguments: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let world = shared.borrow();
            let object = contract::require_object(lua, &world, value(&arguments, 0), 1)?;
            let part = require_part(value(&arguments, 1), 2, &records)?;
            Ok(crate::btech::inventory(&world, object)
                .map_err(|failure| error::failure("btech.operation.failed", failure))?
                .iter()
                .find(|entry| entry.part_id == part.id && entry.brand_id == part.brand)
                .map_or(0, |entry| entry.quantity))
        })?,
    )?;

    let records = catalogue.clone();
    contract::bind(
        lua,
        native,
        "parts_contract_adjust_stores",
        GROUP,
        "adjust_stores",
        lua.create_function(move |lua, arguments: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let mut world = scripts.world_mut();
            let object = contract::require_object(lua, &world, value(&arguments, 0), 1)?;
            let part = require_part(value(&arguments, 1), 2, &records)?;
            let raw_delta = value(&arguments, 2);
            if !matches!(raw_delta, Value::Integer(_) | Value::Number(_)) {
                return Err(argument_failure(
                    3,
                    "mux.arg.invalid",
                    "delta must be a nonzero integer",
                ));
            }
            let delta = match raw_delta {
                Value::Integer(delta)
                    if delta != 0 && (i32::MIN as i64..=i32::MAX as i64).contains(&delta) =>
                {
                    delta
                }
                Value::Number(delta)
                    if delta.is_finite()
                        && delta.fract() == 0.0
                        && delta != 0.0
                        && (i32::MIN as f64..=i32::MAX as f64).contains(&delta) =>
                {
                    delta as i64
                }
                _ => {
                    return Err(argument_failure(
                        3,
                        "mux.arg.invalid",
                        "delta must be a nonzero ranged integer",
                    ));
                }
            };
            let current = crate::btech::inventory(&world, object)
                .map_err(|failure| contract::operation_failure("store_commit_failed", failure))?
                .iter()
                .find(|entry| entry.part_id == part.id && entry.brand_id == part.brand)
                .map_or(0, |entry| entry.quantity);
            let Some(updated) = i64::from(current)
                .checked_add(delta)
                .filter(|updated| (0..=i32::MAX as i64).contains(updated))
            else {
                return Err(contract::operation_failure(
                    "store_capacity_exceeded",
                    "store adjustment would exceed capacity",
                ));
            };
            crate::btech::set_part_store_quantity(
                &mut world,
                object,
                part.id,
                part.brand,
                updated as i32,
            )
            .map_err(|_| {
                contract::operation_failure(
                    "store_commit_failed",
                    "store adjustment could not be committed",
                )
            })?;
            Ok(MultiValue::new())
        })?,
    )?;

    let records = catalogue;
    contract::bind(
        lua,
        native,
        "parts_contract_set_cost",
        GROUP,
        "set_cost",
        lua.create_function(move |lua, arguments: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let mut world = scripts.world_mut();
            let part = require_part(value(&arguments, 0), 1, &records)?;
            let raw_cost = value(&arguments, 1);
            if !matches!(raw_cost, Value::Integer(_) | Value::Number(_)) {
                return Err(argument_failure(
                    2,
                    "mux.arg.invalid",
                    "cost must be a safe nonnegative integer",
                ));
            }
            let cost = match raw_cost {
                Value::Integer(cost) if (0..=LUA_SAFE_INTEGER_MAX).contains(&cost) => cost as u64,
                Value::Number(cost)
                    if cost.is_finite()
                        && cost.fract() == 0.0
                        && (0.0..=LUA_SAFE_INTEGER_MAX as f64).contains(&cost) =>
                {
                    cost as u64
                }
                _ => {
                    return Err(argument_failure(
                        2,
                        "mux.arg.invalid",
                        "cost must be an integer from 0 to 2^53-1",
                    ));
                }
            };
            crate::btech::set_part_cost(&mut world, part.id, cost)
                .map_err(|failure| error::failure("btech.operation.failed", failure))?;
            Ok(MultiValue::new())
        })?,
    )?;
    Ok(())
}
