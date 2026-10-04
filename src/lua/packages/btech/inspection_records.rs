//! Lua table projection for detached BattleTech inspection records.

use super::{constants, parts_contract};
use crate::{PartForm, World};
use mlua::{Lua, Table, Value};

fn pair(lua: &Lua, values: (u32, u32)) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.raw_set("current", values.0)?;
    table.raw_set("original", values.1)?;
    Ok(table)
}

pub(super) fn armor(lua: &Lua, row: crate::btech::InspectionArmor) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    if let Some(section) = row.section {
        table.raw_set(
            "section",
            constants::push(lua, &constants::SECTIONS, section)?,
        )?;
    }
    table.raw_set("armor", pair(lua, row.armor)?)?;
    table.raw_set("internal", pair(lua, row.internal)?)?;
    table.raw_set("rear_armor", pair(lua, row.rear_armor)?)?;
    Ok(table)
}

pub(super) fn weapon(
    lua: &Lua,
    world: &World,
    catalogue: &[PartForm],
    row: &crate::btech::InspectionWeapon,
) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    table.raw_set("number", row.number)?;
    table.raw_set(
        "section",
        constants::push(lua, &constants::SECTIONS, row.section)?,
    )?;
    table.raw_set("first_slot", row.first_slot)?;
    table.raw_set(
        "part",
        parts_contract::push_part(
            lua,
            world,
            catalogue,
            parts_contract::PartReference { id: row.part.id },
        )?,
    )?;
    table.raw_set("slot_count", row.slot_count)?;
    table.raw_set("recycle", row.recycle)?;
    table.raw_set("recycle_time", row.recycle_time)?;
    table.raw_set("operational", row.operational)?;
    Ok(table)
}

pub(super) fn weapons(
    lua: &Lua,
    world: &World,
    catalogue: &[PartForm],
    rows: impl IntoIterator<Item = crate::btech::InspectionWeapon>,
) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    for (index, row) in rows.into_iter().enumerate() {
        table.raw_set(index + 1, weapon(lua, world, catalogue, &row)?)?;
    }
    Ok(table)
}

pub(super) fn inventory(
    lua: &Lua,
    world: &World,
    catalogue: &[PartForm],
    rows: impl IntoIterator<Item = (crate::btech::InspectionPart, u32)>,
) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    let mut rows: Vec<_> = rows
        .into_iter()
        .filter(|(part, _)| catalogue.iter().any(|form| form.part_id == part.id))
        .collect();
    rows.sort_by_key(|(part, _)| {
        catalogue
            .iter()
            .position(|form| form.part_id == part.id)
            .unwrap_or(usize::MAX)
    });
    for (index, (part, quantity)) in rows.into_iter().enumerate() {
        let row = lua.create_table()?;
        row.raw_set(
            "part",
            parts_contract::push_part(
                lua,
                world,
                catalogue,
                parts_contract::PartReference { id: part.id },
            )?,
        )?;
        row.raw_set("quantity", quantity)?;
        table.raw_set(index + 1, row)?;
    }
    Ok(table)
}

pub(super) fn criticals(
    lua: &Lua,
    world: &World,
    catalogue: &[PartForm],
    rows: impl IntoIterator<Item = crate::btech::InspectionCritical>,
) -> mlua::Result<Table> {
    let table = lua.create_table()?;
    for (index, item) in rows.into_iter().enumerate() {
        let row = lua.create_table()?;
        row.raw_set(
            "section",
            constants::push(lua, &constants::SECTIONS, item.section)?,
        )?;
        row.raw_set("slot", item.slot)?;
        row.raw_set("kind", item.kind)?;
        if let Some(part) = item.part {
            row.raw_set(
                "part",
                parts_contract::push_part(
                    lua,
                    world,
                    catalogue,
                    parts_contract::PartReference { id: part.id },
                )?,
            )?;
        }
        row.raw_set("operational", item.operational)?;
        row.raw_set("temporary_failure", item.temporary_failure)?;
        row.raw_set("auxiliary_data", item.auxiliary_data)?;
        if let Some((rounds, capacity)) = item.ammunition {
            let ammunition = lua.create_table()?;
            ammunition.raw_set("rounds", rounds)?;
            ammunition.raw_set("capacity", capacity)?;
            row.raw_set("ammunition", ammunition)?;
        }
        let fire = lua.create_table()?;
        for (i, mode) in item.fire_modes.into_iter().enumerate() {
            fire.raw_set(i + 1, constants::push(lua, &constants::FIRE_MODES, mode)?)?;
        }
        row.raw_set("fire_modes", fire)?;
        let ammo = lua.create_table()?;
        for (i, mode) in item.ammunition_modes.into_iter().enumerate() {
            ammo.raw_set(
                i + 1,
                constants::push(lua, &constants::AMMUNITION_MODES, mode)?,
            )?;
        }
        row.raw_set("ammunition_modes", ammo)?;
        table.raw_set(index + 1, row)?;
    }
    Ok(table)
}

pub(super) fn optional_string(lua: &Lua, value: Option<&str>) -> mlua::Result<Value> {
    value
        .map(|value| lua.create_string(value).map(Value::String))
        .unwrap_or(Ok(Value::Nil))
}
