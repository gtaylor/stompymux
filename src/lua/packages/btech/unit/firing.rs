//! Firing commands: sight and fire actions, plus TIC management.

use super::super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    {
        let sighting_world = world.clone();
        let sight = lua.create_function(
            move |lua, (unit, pilot, weapon, target): (i64, i64, usize, Value)| {
                crate::lua::transactions::require(lua)?;
                crate::lua::transactions::run(lua, &sighting_world, || {
                    let config = crate::lua::configuration(lua);
                    let scripts = crate::Scripts::services(lua)?;
                    let report = crate::btech::sight::resolve_action(
                        &scripts,
                        &config,
                        ObjectId(unit),
                        ObjectId(pilot),
                        weapon,
                        firing_target(lua, target.clone())?,
                    )
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                    detached(lua, &report)
                })
            },
        )?;
        native.set(
            "unit_sight",
            error::wrap(lua, sight, "btech.operation.failed")?,
        )?;
    }
    {
        let firing_world = world.clone();
        let fire = lua.create_function(
            move |lua, (unit, pilot, weapon, target): (i64, i64, usize, Value)| {
                crate::lua::transactions::require(lua)?;
                crate::lua::transactions::run(lua, &firing_world, || {
                    let config = crate::lua::configuration(lua);
                    let scripts = crate::Scripts::services(lua)?;
                    let report = crate::btech::firing::resolve_action(
                        &scripts,
                        &config,
                        ObjectId(unit),
                        ObjectId(pilot),
                        weapon,
                        firing_target(lua, target.clone())?,
                    )
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                    detached(lua, &report)
                })
            },
        )?;
        native.set(
            "unit_fire",
            error::wrap(lua, fire, "btech.operation.failed")?,
        )?;
    }
    let tic_fire = lua.create_function(
        move |lua, (unit, pilot, groups, target): (i64, i64, Vec<usize>, Value)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            crate::lua::transactions::run(lua, &scripts.world, || {
                let config = crate::lua::configuration(lua);
                let report = crate::fire_battle_tics(
                    &scripts,
                    &config,
                    ObjectId(unit),
                    ObjectId(pilot),
                    groups,
                    firing_target(lua, target.clone())?,
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
                detached(lua, &report)
            })
        },
    )?;
    native.set(
        "unit_tic_fire",
        error::wrap(lua, tic_fire, "btech.operation.failed")?,
    )?;
    let tic_world = world.clone();
    let tic = lua.create_function(move |lua, (unit, pilot, group): (i64, i64, usize)| {
        crate::lua::transactions::require(lua)?;
        let members = crate::tic(&tic_world.borrow(), ObjectId(unit), ObjectId(pilot), group)
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &members)
    })?;
    native.set("unit_tic", error::wrap(lua, tic, "btech.operation.failed")?)?;
    let tic_world = world.clone();
    let tic_edit = lua.create_function(
        move |lua,
              (unit, pilot, group, operation, members): (
            i64,
            i64,
            usize,
            String,
            Option<Vec<usize>>,
        )| {
            crate::lua::transactions::require(lua)?;
            crate::lua::transactions::run(lua, &tic_world, || {
                let edit = match operation.as_str() {
                    "add" => crate::TicEdit::Add(members.ok_or_else(|| {
                        error::failure("btech.operation.failed", "Supply weapon numbers")
                    })?),
                    "remove" => crate::TicEdit::Remove(members.ok_or_else(|| {
                        error::failure("btech.operation.failed", "Supply weapon numbers")
                    })?),
                    "clear" if members.is_none() => crate::TicEdit::Clear,
                    _ => {
                        return Err(error::failure(
                            "btech.operation.failed",
                            "Use add, remove or clear",
                        ));
                    }
                };
                crate::edit_battle_tic(
                    &mut tic_world.borrow_mut(),
                    ObjectId(unit),
                    ObjectId(pilot),
                    group,
                    edit,
                )
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))
            })
        },
    )?;
    native.set(
        "unit_tic_edit",
        error::wrap(lua, tic_edit, "btech.operation.failed")?,
    )?;
    Ok(())
}
