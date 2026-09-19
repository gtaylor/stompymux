//! Map display and navigation commands: tactical and long-range maps, viewports, and navigation reports.

use super::super::*;

/// Register this package slice on the private native table.
pub(super) fn register(lua: &Lua, native: &Table, _world: &SharedWorld) -> mlua::Result<()> {
    let findcenter = lua.create_function(move |lua, (unit, pilot): (i64, i64)| {
        crate::lua::transactions::require(lua)?;
        let scripts = crate::Scripts::services(lua)?;
        let report =
            crate::find_battle_hex_center(&scripts.world.borrow(), ObjectId(unit), ObjectId(pilot))
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
        detached(lua, &report)
    })?;
    native.set(
        "unit_findcenter",
        error::wrap(lua, findcenter, "btech.operation.failed")?,
    )?;
    let navigate = lua.create_function(
        move |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_navigate(
                &scripts.world.borrow(),
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or(""),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_navigate",
        error::wrap(lua, navigate, "btech.operation.failed")?,
    )?;
    let tactical = lua.create_function(
        move |lua, (unit, pilot, arguments): (i64, i64, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::battle_tactical_map(
                &scripts.world.borrow(),
                ObjectId(unit),
                ObjectId(pilot),
                arguments.as_deref().unwrap_or(""),
                crate::battle_view_dimensions(&scripts.world.borrow(), ObjectId(pilot))
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_tactical",
        error::wrap(lua, tactical, "btech.operation.failed")?,
    )?;
    let lrsmap = lua.create_function(
        move |lua, (unit, pilot, mode, arguments): (i64, i64, String, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let report = crate::btech::long_range_map::from_arguments(
                &scripts.world.borrow(),
                ObjectId(unit),
                ObjectId(pilot),
                &mode,
                arguments.as_deref().unwrap_or(""),
                crate::battle_view_dimensions(&scripts.world.borrow(), ObjectId(pilot))
                    .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_lrsmap",
        error::wrap(lua, lrsmap, "btech.operation.failed")?,
    )?;
    let viewport = lua.create_function(
        move |lua,
              (unit, pilot, kind, arguments, dimensions): (
            i64,
            i64,
            String,
            Option<String>,
            Option<mlua::Table>,
        )| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let kind = match kind.as_str() {
                "tactical" => crate::BattleViewKind::Tactical,
                "long_range" => crate::BattleViewKind::LongRange,
                _ => {
                    return Err(error::failure(
                        "btech.operation.failed",
                        "Invalid view kind",
                    ));
                }
            };
            let defaults = crate::battle_view_dimensions(&scripts.world.borrow(), ObjectId(pilot))
                .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            let dimensions = match dimensions {
                Some(table) => crate::BattleViewDimensions {
                    tactical_width: table
                        .get::<Option<u16>>("tactical_width")?
                        .unwrap_or(defaults.tactical_width),
                    tactical_height: table
                        .get::<Option<u16>>("tactical_height")?
                        .unwrap_or(defaults.tactical_height),
                    long_range_height: table
                        .get::<Option<u16>>("long_range_height")?
                        .unwrap_or(defaults.long_range_height),
                },
                None => defaults,
            };
            let report = crate::resolve_battle_viewport(
                &scripts.world.borrow(),
                ObjectId(unit),
                ObjectId(pilot),
                kind,
                arguments.as_deref().unwrap_or(""),
                dimensions,
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_viewport",
        error::wrap(lua, viewport, "btech.operation.failed")?,
    )?;
    let view_center = lua.create_function(
        move |lua, (unit, pilot, kind, arguments): (i64, i64, String, Option<String>)| {
            crate::lua::transactions::require(lua)?;
            let scripts = crate::Scripts::services(lua)?;
            let kind = match kind.as_str() {
                "tactical" => crate::BattleViewKind::Tactical,
                "long_range" => crate::BattleViewKind::LongRange,
                _ => {
                    return Err(error::failure(
                        "btech.operation.failed",
                        "Invalid view kind",
                    ));
                }
            };
            let report = crate::parse_battle_view_center(
                &scripts.world.borrow(),
                ObjectId(unit),
                ObjectId(pilot),
                kind,
                arguments.as_deref().unwrap_or(""),
            )
            .map_err(|e| error::failure("btech.operation.failed", format!("{e:#}")))?;
            detached(lua, &report)
        },
    )?;
    native.set(
        "unit_view_center",
        error::wrap(lua, view_center, "btech.operation.failed")?,
    )?;
    Ok(())
}
