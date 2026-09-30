//! Live terrain edits preserve unit positions and share their mutation with seasonal ice growth.
use super::{BattleHex, BattleHexCoordinate, Terrain};
use crate::{Config, ObjectId, Scripts, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::sync::Arc;

/// A base-terrain edit; temporary overlays and map objects remain independently owned.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BattleMapHexChange {
    pub map: ObjectId,
    pub coordinate: BattleHexCoordinate,
    pub before: BattleHex,
    pub after: BattleHex,
}

/// Replace a tile while retaining physical altitude. Authority and atomic publication belong to the caller.
pub(super) fn replace_hex(
    world: &mut World,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    after: BattleHex,
) -> Result<BattleMapHexChange> {
    ensure!(after.elevation <= 9, "Elevation exceeds map limits");
    let before = world
        .btech
        .maps()
        .get(&map)
        .context("Map not found")?
        .base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let report = BattleMapHexChange {
        map,
        coordinate,
        before,
        after,
    };
    if world.btech.maps()[&map].stored_hex(i64::from(coordinate.x), i64::from(coordinate.y))?
        == after
    {
        return Ok(report);
    }
    let on_tile = |position: Option<super::BattlePosition>| {
        position.is_some_and(|position| {
            position.map == map
                && i32::from(position.x) == coordinate.x
                && i32::from(position.y) == coordinate.y
        })
    };
    for unit in world.btech.constructed.values_mut() {
        if on_tile(unit.position) && unit.retained_altitude().is_none() {
            unit.ground_elevation = Some(unit.altitude(before));
        }
    }
    for unit in world.btech.vehicles.values_mut() {
        if !on_tile(unit.position()) {
            continue;
        }
        if unit.ground_elevation.is_none()
            && unit.vtol_flight().is_none()
            && unit.free_fall().is_none()
            && unit.orbital_drop().is_none()
        {
            unit.ground_elevation = Some(unit.altitude(before));
        }
        if after.terrain != Terrain::Bridge || after.elevation < 2 {
            unit.under_bridge = false;
        }
    }
    let record = world.btech.maps.get_mut(&map).unwrap();
    let index = (i64::from(coordinate.y) * record.width + i64::from(coordinate.x)) as usize;
    Arc::make_mut(record.terrain.as_mut().unwrap())[index] = after;
    Ok(report)
}

/// Wizard terrain change with the reference's absolute, capped elevation magnitude.
/// Editing changes terrain facts without moving units or applying a combat fracture.
pub fn set_map_hex_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
    coordinate: BattleHexCoordinate,
    terrain: Terrain,
    elevation: i32,
) -> Result<BattleMapHexChange> {
    scripts.atomic(|before| {
        ensure!(
            crate::authority::is_wizard(before, actor),
            "Permission denied."
        );
        ensure!(
            before
                .objects
                .get(&map)
                .is_some_and(|object| object.kind != crate::Kind::Garbage
                    && !object.flags.contains(crate::Flag::Going)),
            "Map is unavailable"
        );
        let report = replace_hex(
            &mut scripts.world_mut(),
            map,
            coordinate,
            BattleHex {
                terrain,
                elevation: elevation.unsigned_abs().min(9) as u8,
            },
        )?;
        super::notify_message(
            scripts,
            super::BattleMessageTarget::Player(actor),
            "Hex set!",
        )?;
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(report)
    })
}

/// Native terrain arguments use the first symbol, including the operator's grassland shorthand.
pub(crate) fn terrain_argument(value: &str) -> Result<Terrain> {
    match value.chars().next().context("Expected a terrain symbol")? {
        '.' => Ok(Terrain::Grassland),
        symbol => Terrain::from_symbol(symbol),
    }
}

/// Parse one coordinate, symbol and signed magnitude before authorizing an edit.
fn parse(arguments: &str) -> Result<(BattleHexCoordinate, Terrain, i32)> {
    let args: Vec<_> = arguments.split_whitespace().take(5).collect();
    ensure!(args.len() == 4, "Expected x y terrain elevation");
    Ok((
        BattleHexCoordinate {
            x: args[0].parse().context("Invalid x coordinate")?,
            y: args[1].parse().context("Invalid y coordinate")?,
        },
        terrain_argument(args[2])?,
        args[3].parse().context("Invalid elevation")?,
    ))
}

/// Native operators edit the map containing their player object.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let (coordinate, terrain, elevation) = parse(&input.args)?;
        let map = super::special_dispatch::object(ctx)?;
        set_map_hex_action(
            ctx.scripts,
            ctx.config,
            ctx.player,
            map,
            coordinate,
            terrain,
            elevation,
        )?;
        Ok(())
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Canonical symbols, grassland shorthand and signed magnitudes share one bounded grammar.
    #[test]
    fn terrain_edit_arguments() {
        for terrain in [
            Terrain::Grassland,
            Terrain::Road,
            Terrain::LightForest,
            Terrain::HeavyForest,
            Terrain::Water,
            Terrain::Ice,
            Terrain::Bridge,
            Terrain::HighWater,
            Terrain::Rough,
            Terrain::Mountains,
            Terrain::Fire,
            Terrain::Smoke,
            Terrain::Snow,
            Terrain::Building,
            Terrain::Wall,
            Terrain::Sand,
        ] {
            let symbol = if terrain == Terrain::Grassland {
                '.'
            } else {
                terrain.symbol()
            };
            assert_eq!(
                parse(&format!("1 2 {symbol} -9")).unwrap(),
                (BattleHexCoordinate { x: 1, y: 2 }, terrain, -9)
            );
        }
        assert_eq!(parse("0 0 .ignored -2147483648").unwrap().2, i32::MIN);
        for args in [
            "",
            "0 0 .",
            "0 0 . 1 extra",
            "x 0 . 1",
            "0 y . 1",
            "0 0 X 1",
            "0 0 . 2147483648",
        ] {
            assert!(parse(args).is_err(), "{args}");
        }
    }
}
