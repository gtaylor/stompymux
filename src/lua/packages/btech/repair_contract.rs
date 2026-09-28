//! Repair queries whose behavior does not require the unsupplied repair scheduler.

use super::*;

pub(super) fn register(lua: &Lua, native: &Table, world: &SharedWorld) -> mlua::Result<()> {
    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "repair_apply_contract",
        "repair",
        "apply",
        lua.create_function(move |lua, args: mlua::MultiValue| {
            // C resolves the unit and its runtime before inspecting argument two.
            let id = {
                let world = shared.borrow();
                contract::require_special(
                    lua,
                    &world,
                    args.front().cloned().unwrap_or(Value::Nil),
                    1,
                    "MECH",
                    "unit",
                )?
            };
            let request = match args.get(1) {
                None => {
                    return Err(error::failure_with_detail(
                        "mux.arg.invalid",
                        "repair request is required",
                        serde_json::json!({"argument":2}),
                    ));
                }
                Some(Value::Table(request)) => request.clone(),
                Some(_) => {
                    return Err(error::failure_with_detail(
                        "mux.arg.invalid",
                        "repair request must be a table",
                        serde_json::json!({"argument":2}),
                    ));
                }
            };
            let operation = constants::require(
                request.raw_get::<Value>("operation")?,
                2,
                "operation",
                &constants::REPAIR_OPERATIONS,
            )?;
            let (kind, allowed) = match operation {
                0 => (
                    crate::btech::AdministrativeRepairKind::Reattach,
                    &["operation", "section"][..],
                ),
                1 => (
                    crate::btech::AdministrativeRepairKind::Part,
                    &["operation", "section", "slot"][..],
                ),
                12 => (
                    crate::btech::AdministrativeRepairKind::Armor,
                    &["operation", "section", "value"][..],
                ),
                13 => (
                    crate::btech::AdministrativeRepairKind::RearArmor,
                    &["operation", "section", "value"][..],
                ),
                14 => (
                    crate::btech::AdministrativeRepairKind::Internal,
                    &["operation", "section", "value"][..],
                ),
                _ => {
                    return Err(error::failure_with_detail(
                        "mux.arg.invalid",
                        "operation is not an immediate repair operation",
                        serde_json::json!({"argument":2}),
                    ));
                }
            };
            contract::check_options(&request, allowed, 2)?;
            let section_code = constants::require(
                request.raw_get::<Value>("section")?,
                2,
                "section",
                &constants::SECTIONS,
            )?;
            if !crate::btech::administrative_section_valid(&shared.borrow(), id, section_code) {
                return Err(error::failure_with_detail(
                    "mux.arg.invalid",
                    "section is not valid for this unit",
                    serde_json::json!({"argument":2}),
                ));
            }
            let (rear, slots) =
                crate::btech::administrative_section_info(&shared.borrow(), id, section_code)
                    .ok_or_else(|| {
                        error::failure_with_detail(
                            "mux.arg.invalid",
                            "section is not valid for this unit",
                            serde_json::json!({"argument":2}),
                        )
                    })?;
            if kind == crate::btech::AdministrativeRepairKind::RearArmor && !rear {
                return Err(error::failure_with_detail(
                    "mux.arg.invalid",
                    "rear armor is only valid on Mech torso sections",
                    serde_json::json!({"argument":2}),
                ));
            }
            let value = match kind {
                crate::btech::AdministrativeRepairKind::Armor
                | crate::btech::AdministrativeRepairKind::RearArmor
                | crate::btech::AdministrativeRepairKind::Internal => {
                    contract::integer(request.raw_get::<Value>("value")?, "value", 0, 255, 2)?
                        as u16
                }
                crate::btech::AdministrativeRepairKind::Part => {
                    (contract::integer(
                        request.raw_get::<Value>("slot")?,
                        "slot",
                        1,
                        slots as i64,
                        2,
                    )? - 1) as u16
                }
                crate::btech::AdministrativeRepairKind::Reattach => 0,
            };
            crate::btech::ensure_registered_unit_runtime(&mut shared.borrow_mut(), id).map_err(
                |_| {
                    error::failure_with_detail(
                        "mux.object.unavailable",
                        "unit runtime state is unavailable",
                        serde_json::json!({"argument":1}),
                    )
                },
            )?;
            crate::btech::apply_administrative_repair(
                &mut shared.borrow_mut(),
                id,
                section_code,
                kind,
                value,
            )
            .map_err(|e| contract::operation_failure("repair_apply_failed", e))?;
            Ok(mlua::MultiValue::new())
        })?,
    )?;
    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "repair_is_fixable_contract",
        "repair",
        "is_fixable",
        lua.create_function(move |lua, args: mlua::MultiValue| {
            let world = shared.borrow();
            let unit = contract::require_special(
                lua,
                &world,
                args.front().cloned().unwrap_or(Value::Nil),
                1,
                "MECH",
                "unit",
            )?;
            crate::btech::administrative_is_fixable(&world, unit).ok_or_else(|| {
                error::failure_with_detail(
                    "mux.object.unavailable",
                    "unit runtime state is unavailable",
                    serde_json::json!({"argument":1}),
                )
            })
        })?,
    )?;
    let shared = world.clone();
    contract::bind(
        lua,
        native,
        "repair_technician_available_contract",
        "repair",
        "technician_available_in",
        lua.create_function(move |lua, args: mlua::MultiValue| {
            let world = shared.borrow();
            let player = contract::require_object(
                lua,
                &world,
                args.front().cloned().unwrap_or(Value::Nil),
                1,
            )?;
            if !world
                .objects
                .get(&player)
                .is_some_and(|object| object.kind == crate::Kind::Player)
            {
                return Err(error::failure_with_detail(
                    "mux.object.invalid",
                    "object is not a player",
                    serde_json::json!({"argument":1}),
                ));
            }
            let available = world
                .btech
                .player_configuration
                .get(&player)
                .map_or(0, |configuration| configuration.technician_available_at);
            Ok(available.saturating_sub(crate::clock::wall_time()).max(0))
        })?,
    )?;
    Ok(())
}
