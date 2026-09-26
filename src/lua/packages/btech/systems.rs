//! Weapon settings, calculations, networks, and shared unit-system native bindings.

use super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let shared = world.clone();
    let settings = lua.create_function(move |lua, name: String| {
        crate::lua::transactions::require(lua)?;
        let weapon =
            crate::BattleWeapon::parse_operator_name(&name).map_err(mlua::Error::external)?;
        detached(lua, &shared.borrow().btech.weapon_settings().get(weapon))
    })?;
    native.set(
        "weapon_settings",
        error::wrap(lua, settings, "btech.operation.failed")?,
    )?;
    for (name, recycle) in [
        ("weapon_set_recycle", true),
        ("weapon_set_battle_value", false),
    ] {
        let setting =
            lua.create_function(move |lua, (actor, name, value): (i64, String, i64)| {
                crate::lua::transactions::require(lua)?;
                let scripts = crate::Scripts::services(lua)?;
                crate::lua::transactions::run(lua, &scripts.world, || {
                    let values = crate::edit_battle_weapon_settings(
                        &scripts,
                        &crate::lua::configuration(lua),
                        ObjectId(actor),
                        &name,
                        value,
                        recycle,
                    )
                    .map_err(mlua::Error::external)?;
                    detached(lua, &values)
                })
            })?;
        native.set(name, error::wrap(lua, setting, "btech.operation.failed")?)?;
    }
    let bootlegger = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_bootlegger(
                &scripts,
                &crate::lua::configuration(lua),
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_bootlegger",
        error::wrap(lua, bootlegger, "btech.operation.failed")?,
    )?;
    let vector = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_vector_report(
                &scripts.world(),
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_vector",
        error::wrap(lua, vector, "btech.operation.failed")?,
    )?;
    let range = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_range_report(
                &scripts.world(),
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_range_report",
        error::wrap(lua, range, "btech.operation.failed")?,
    )?;
    let bearing = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_bearing(
                &scripts.world(),
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_bearing",
        error::wrap(lua, bearing, "btech.operation.failed")?,
    )?;
    let eta = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_eta_action(
                &scripts,
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set("unit_eta", error::wrap(lua, eta, "btech.operation.failed")?)?;
    let supercharger = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_supercharger(&scripts, ObjectId(unit), ObjectId(pilot))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_supercharger",
        error::wrap(lua, supercharger, "btech.operation.failed")?,
    )?;
    let c3_targets = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3_targets(&scripts.world(), ObjectId(unit), ObjectId(pilot))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_c3_targets",
        error::wrap(lua, c3_targets, "btech.operation.failed")?,
    )?;
    let c3_network = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3_status(&scripts.world(), ObjectId(unit), ObjectId(pilot))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_c3_network",
        error::wrap(lua, c3_network, "btech.operation.failed")?,
    )?;
    let c3i_targets = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3i_targets(&scripts.world(), ObjectId(unit), ObjectId(pilot))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_c3i_targets",
        error::wrap(lua, c3i_targets, "btech.operation.failed")?,
    )?;
    let c3i_network = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3i_status(&scripts.world(), ObjectId(unit), ObjectId(pilot))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_c3i_network",
        error::wrap(lua, c3i_network, "btech.operation.failed")?,
    )?;
    let c3_message = lua.create_function(|lua, (unit, pilot, message): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3_message(&scripts, ObjectId(unit), ObjectId(pilot), &message)
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_c3_message",
        error::wrap(lua, c3_message, "btech.operation.failed")?,
    )?;
    let c3i_message = lua.create_function(|lua, (unit, pilot, message): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3i_message(&scripts, ObjectId(unit), ObjectId(pilot), &message)
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_c3i_message",
        error::wrap(lua, c3i_message, "btech.operation.failed")?,
    )?;
    let c3 = lua.create_function(|lua, (unit, pilot, arguments): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3(&scripts, ObjectId(unit), ObjectId(pilot), &arguments)
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set("unit_c3", error::wrap(lua, c3, "btech.operation.failed")?)?;
    let c3i = lua.create_function(|lua, (unit, pilot, arguments): (i64, i64, String)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_c3i(&scripts, ObjectId(unit), ObjectId(pilot), &arguments)
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set("unit_c3i", error::wrap(lua, c3i, "btech.operation.failed")?)?;
    let masc = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report = crate::battle_masc(&scripts, ObjectId(unit), ObjectId(pilot))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_masc",
        error::wrap(lua, masc, "btech.operation.failed")?,
    )?;
    let dump = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_dump(
                &scripts,
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_dump",
        error::wrap(lua, dump, "btech.operation.failed")?,
    )?;
    let lateral = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_lateral(
                &scripts,
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_lateral",
        error::wrap(lua, lateral, "btech.operation.failed")?,
    )?;
    let brief = lua.create_function(
        |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_brief(
                &scripts,
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or_default(),
            )
            .map_err(|e| error::failure("btech.operation.failed", e))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_brief",
        error::wrap(lua, brief, "btech.operation.failed")?,
    )?;
    let building_contacts = lua.create_function(|lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let contacts = crate::battle_building_contacts(&scripts, ObjectId(unit), ObjectId(pilot))
            .map_err(|e| error::failure("btech.operation.failed", e))?;
        detached(lua, &contacts)
    })?;
    native.set(
        "unit_building_contacts",
        error::wrap(lua, building_contacts, "btech.operation.failed")?,
    )?;

    Ok(())
}
