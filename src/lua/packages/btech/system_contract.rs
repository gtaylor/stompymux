//! C-compatible server-wide BattleTech queries.

use super::*;

pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    contract::bind(
        lua,
        native,
        "system_event_lag_contract",
        "system",
        "event_lag",
        lua.create_function(move |lua, _: mlua::MultiValue| {
            let telemetry = lua
                .app_data_ref::<std::rc::Rc<std::cell::Cell<crate::EventTelemetry>>>()
                .ok_or_else(|| {
                    error::failure("mux.state.unavailable", "event timing is unavailable")
                })?
                .get();
            Ok(telemetry.lag(crate::clock::wall_time()))
        })?,
    )?;
    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "system_units_in_zone_contract",
        "system",
        "units_in_zone",
        lua.create_function(move |lua, args: mlua::MultiValue| {
            let world = shared.borrow();
            let zone = contract::require_object(
                lua,
                &world,
                args.front().cloned().unwrap_or(Value::Nil),
                1,
            )?;
            let ids: Vec<_> = world
                .objects
                .iter()
                .filter_map(|(&id, object)| {
                    (object.kind == crate::Kind::Thing
                        && !object.flags.contains(crate::Flag::Going)
                        && object.zone == Some(zone)
                        && world
                            .btech
                            .registrations()
                            .get(&id)
                            .is_some_and(|kind| kind == "UNIT"))
                    .then_some(id)
                })
                .collect();
            drop(world);
            let result = lua.create_table()?;
            for (index, id) in ids.into_iter().enumerate() {
                result.raw_set(index + 1, contract::push_object(lua, &shared, id)?)?;
            }
            Ok(result)
        })?,
    )?;
    Ok(())
}
