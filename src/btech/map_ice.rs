//! Operator ice growth and melting share map randomness and ordinary surface-break consequences.
use super::{BattleFallRules, BattleHexCoordinate, BattleSurfaceBreak, StoredBattleMap, Terrain};
use crate::{Config, ObjectId, Scripts};
use anyhow::{Context, Result, ensure};
use serde::Serialize;

/// The requested seasonal terrain transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BattleIceChange {
    Grow,
    Melt,
}

/// Changed coordinates and any occupant consequences, in map traversal order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleMapIceReport {
    pub map: ObjectId,
    pub changed: Vec<BattleHexCoordinate>,
    pub fractures: Vec<BattleSurfaceBreak>,
}

/// Apply the neighborhood probability after the percentage draw has succeeded.
fn eligible(
    map: &StoredBattleMap,
    coordinate: BattleHexCoordinate,
    change: BattleIceChange,
    dice: &mut super::BattleDice,
) -> Result<bool> {
    let count = coordinate
        .neighbors()?
        .into_iter()
        .filter_map(|neighbor| {
            map.base_hex(i64::from(neighbor.x), i64::from(neighbor.y))
                .ok()
        })
        .filter(|tile| match change {
            BattleIceChange::Grow => tile.is_open_water() || tile.has_bridge(),
            BattleIceChange::Melt => tile.is_ice(),
        })
        .count() as u8;
    Ok(match change {
        BattleIceChange::Grow => count <= 4 && (count < 2 || dice.d6() > count),
        BattleIceChange::Melt => count <= 4 || dice.die(3)? == 1,
    })
}

/// Grow or melt ice atomically, including map dice, occupied-hex falls, casualties and output.
/// Growth uses the original shoreline for the entire pass; melting exposes each new edge immediately.
/// Percentages outside 0–100 retain the reference's always/never threshold behavior.
pub fn change_map_ice_action(
    scripts: &Scripts,
    config: &Config,
    actor: ObjectId,
    map: ObjectId,
    percentage: i32,
    change: BattleIceChange,
) -> Result<BattleMapIceReport> {
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
        let original = before.btech.maps().get(&map).context("Map not found")?;
        original.validate()?;
        ensure!(original.terrain_ready(), "Map terrain is unavailable");
        ensure!(
            original.fire_dice.is_some(),
            "Map random stream is unavailable"
        );
        let mut report = BattleMapIceReport {
            map,
            changed: Vec::new(),
            fractures: Vec::new(),
        };
        for x in 0..original.width {
            for y in 0..original.height {
                let coordinate = BattleHexCoordinate {
                    x: x as i32,
                    y: y as i32,
                };
                let qualifies = {
                    let mut world = scripts.world_mut();
                    let live = world.btech.maps.get_mut(&map).unwrap();
                    let record = match change {
                        BattleIceChange::Grow => original,
                        BattleIceChange::Melt => &*live,
                    };
                    let tile = record.base_hex(x, y)?;
                    let eligible_tile = match change {
                        BattleIceChange::Grow => tile.is_open_water(),
                        BattleIceChange::Melt => tile.is_ice(),
                    };
                    if !eligible_tile {
                        continue;
                    }
                    let mut dice = live
                        .fire_dice
                        .clone()
                        .context("Map random stream is unavailable")?;
                    let selected = i32::from(dice.die(100)?) <= percentage
                        && eligible(record, coordinate, change, &mut dice)?;
                    // Commit each decision before an occupant fall can draw map-owned blast dice.
                    live.fire_dice = Some(dice);
                    selected
                };
                if !qualifies {
                    continue;
                }
                match change {
                    BattleIceChange::Grow => {
                        super::terrain_edit::replace_hex(
                            &mut scripts.world_mut(),
                            map,
                            coordinate,
                            original.base_hex(x, y)?.frozen(),
                        )?;
                    }
                    BattleIceChange::Melt => {
                        report
                            .fractures
                            .push(super::evacuation::break_surface_action(
                                scripts,
                                config,
                                map,
                                coordinate,
                                Terrain::Ice,
                                BattleFallRules::configured(config),
                            )?)
                    }
                }
                report.changed.push(coordinate);
            }
        }
        let verb = match change {
            BattleIceChange::Grow => "'iced'",
            BattleIceChange::Melt => "melted",
        };
        let count = if report.changed.is_empty() {
            "No".into()
        } else {
            report.changed.len().to_string()
        };
        super::notify_message(
            scripts,
            super::BattleMessageTarget::Player(actor),
            &format!("{count} hexes {verb}."),
        )?;
        scripts.world().validate(config)?;
        scripts.effects.validate()?;
        Ok(report)
    })
}

/// Native map commands select the actor's current location and require one signed percentage.
fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
    change: BattleIceChange,
) -> Result<crate::CommandAction> {
    let result = (|| -> Result<()> {
        let percentage = input
            .args
            .trim()
            .parse::<i32>()
            .context("Expected one integer percentage")?;
        let map = super::special_dispatch::object(ctx)?;
        change_map_ice_action(ctx.scripts, ctx.config, ctx.player, map, percentage, change)?;
        Ok(())
    })();
    Ok(match result {
        Ok(()) => crate::CommandAction::Continue,
        Err(error) => {
            crate::CommandAction::Report(crate::CommandReport::Reply(format!("{error:#}")))
        }
    })
}

/// Freeze eligible shore-connected water.
pub(crate) fn add_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command(ctx, input, BattleIceChange::Grow)
}

/// Melt eligible ice and resolve its occupants through the shared fracture action.
pub(crate) fn remove_command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    command(ctx, input, BattleIceChange::Melt)
}
