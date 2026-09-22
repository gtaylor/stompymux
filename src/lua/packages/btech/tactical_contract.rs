//! Trusted Lua tactical snapshots and atomic multi-controller intentions.

use super::{autopilot_contract, constants, contract, detached, error};
use crate::{ObjectId, SharedWorld, TacticalIntention};
use mlua::{Lua, MultiValue, Table, Value};
use std::collections::BTreeMap;

fn invalid(message: &str) -> mlua::Error {
    error::failure("mux.arg.invalid", message)
}

/// Read a dense bounded array without silently dropping holes or unknown keys.
fn array(value: Value, maximum: usize) -> mlua::Result<Vec<Value>> {
    let Value::Table(table) = value else {
        return Err(invalid("Expected an array"));
    };
    let count = table.clone().pairs::<Value, Value>().count();
    if count > maximum {
        return Err(invalid("Array exceeds request limit"));
    }
    for pair in table.clone().pairs::<Value, Value>() {
        let (key, _) = pair?;
        contract::integer(key, "array index", 1, count as i64, 1)?;
    }
    (1..=count)
        .map(|index| {
            let value = table.raw_get(index)?;
            if matches!(value, Value::Nil) {
                return Err(invalid("Expected a dense array"));
            }
            Ok(value)
        })
        .collect()
}

/// Install detached observations and revision-guarded intentions on the facade.
pub(super) fn register(lua: &Lua, native: &Table, shared: &SharedWorld) -> mlua::Result<()> {
    let world = shared.clone();
    contract::bind(
        lua,
        native,
        "tactical_observe",
        "tactical",
        "observe",
        lua.create_function(move |lua, args: MultiValue| {
            crate::lua::transactions::require(lua)?;
            let borrowed = world.borrow();
            let units = array(
                args.front().cloned().unwrap_or(Value::Nil),
                crate::MAX_TACTICAL_UNITS,
            )?
            .into_iter()
            .map(|v| contract::require_object(lua, &borrowed, v, 1))
            .collect::<mlua::Result<Vec<_>>>()?;
            let mut cursors = BTreeMap::new();
            match args.get(1).cloned().unwrap_or(Value::Nil) {
                Value::Nil => {}
                Value::Table(table) => {
                    if table.clone().pairs::<Value, Value>().count() > crate::MAX_TACTICAL_UNITS {
                        return Err(invalid("Too many feedback cursors"));
                    }
                    for pair in table.pairs::<Value, Value>() {
                        let (id, sequence) = pair?;
                        let id = ObjectId(contract::integer(id, "unit", 0, i64::MAX, 2)?);
                        let sequence =
                            contract::integer(sequence, "feedback sequence", 0, i64::MAX, 2)?
                                as u64;
                        cursors.insert(id, sequence);
                    }
                }
                _ => return Err(invalid("feedback_cursors must be a table")),
            }
            let snapshot = crate::observe_tactical(&borrowed, &units, &cursors)
                .map_err(mlua::Error::external)?;
            detached(lua, &snapshot)
        })?,
    )?;

    let world = shared.clone();
    contract::bind(
        lua,
        native,
        "tactical_submit",
        "tactical",
        "submit",
        lua.create_function(move |lua, value: Value| {
            crate::lua::transactions::require(lua)?;
            let specs = array(value, crate::MAX_TACTICAL_UNITS)?;
            let mut intentions = Vec::with_capacity(specs.len());
            {
                let borrowed = world.borrow();
                for value in specs {
                    let Value::Table(spec) = value else {
                        return Err(invalid("Intention must be a table"));
                    };
                    contract::check_options(
                        &spec,
                        &["unit", "expected_revision", "mode", "orders"],
                        1,
                    )?;
                    let unit = contract::require_object(lua, &borrowed, spec.raw_get("unit")?, 1)?;
                    let expected_revision = contract::integer(
                        spec.raw_get("expected_revision")?,
                        "expected_revision",
                        0,
                        i64::MAX,
                        1,
                    )? as u64;
                    let mode = match constants::require(
                        spec.raw_get("mode")?,
                        1,
                        "submission mode",
                        &constants::AUTOPILOT_SUBMISSION_MODES,
                    )? {
                        0 => crate::btech::AutopilotSubmissionMode::Append,
                        _ => crate::btech::AutopilotSubmissionMode::Replace,
                    };
                    let orders = array(spec.raw_get("orders")?, 64)?
                        .into_iter()
                        .map(|v| autopilot_contract::decode_order(lua, &borrowed, v))
                        .collect::<mlua::Result<Vec<_>>>()?;
                    intentions.push(TacticalIntention {
                        unit,
                        expected_revision,
                        mode,
                        orders,
                    });
                }
            }
            let result = crate::submit_tactical(&mut world.borrow_mut(), &intentions)
                .map_err(mlua::Error::external)?;
            detached(lua, &result)
        })?,
    )?;
    Ok(())
}
