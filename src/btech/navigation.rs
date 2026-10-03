//! Local hex navigation with a continuous-position compass and live cockpit readouts.
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::Serialize;

/// Read-only navigation display and its requested local map center.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct BattleNavigationReport {
    pub center: super::BattleHexCoordinate,
    pub text: String,
}

/// Combine a radius-two map with the positions of acquired units inside the selected center hex.
pub fn navigate(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    arguments: &str,
) -> Result<BattleNavigationReport> {
    let local = super::tactical_map::navigation_hex_map(world, observer, pilot, arguments)?;
    let observer = super::combat_operator::for_owner(world, observer, pilot)?
        .source
        .unit;
    let center = local.viewport.requested_center;
    let unit = super::scanner::scanner_unit(world, observer).context("Scanner is unavailable")?;
    let position = unit.position.context("Unit is not placed")?;
    let heading = unit.heading.context("Unit has no motion state")?;
    let vertical_speed = world
        .btech
        .vehicles()
        .get(&observer)
        .and_then(|unit| unit.vtol_flight())
        .map_or(0.0, |flight| flight.vertical_speed);
    let tile =
        world.btech.maps()[&position.map].hex(i64::from(position.x), i64::from(position.y))?;
    let mut compass: Vec<Vec<char>> = [
        "              0",
        "         ___________",
        "        /           \\",
        "  300  /             \\  60",
        "      /               \\",
        "     /                 \\",
        "270 (                   )  90",
        "     \\                 /",
        "      \\               /",
        "  240  \\             /  120",
        "        \\___________/",
        "",
        "             180",
    ]
    .into_iter()
    .map(|line| {
        let mut chars: Vec<_> = line.chars().collect();
        chars.resize(29, ' ');
        chars
    })
    .collect();
    for id in super::map_slots::all_unit_order(world, position.map)?
        .into_iter()
        .filter(|id| *id != observer)
        .chain(std::iter::once(observer))
    {
        let target = super::scanner::scanner_unit(world, id).expect("placed map unit");
        let Some(target_position) = target.position else {
            continue;
        };
        let marker = if id == observer {
            '*'
        } else {
            if i32::from(target_position.x) != center.x || i32::from(target_position.y) != center.y
            {
                continue;
            }
            let Some(contact) = super::visible_contact(world, observer, id)? else {
                continue;
            };
            if contact.friendly { 'x' } else { 'X' }
        };
        let Some(point) = target.point else {
            continue;
        };
        if let Some((row, column)) = plot_cell(center, point) {
            compass[row][column] = marker;
        }
    }
    let map_lines: Vec<_> = local.text.lines().collect();
    let mut lines = Vec::new();
    for (row, compass) in compass.into_iter().enumerate() {
        let readout = match row {
            2 => format!(
                "Location:{:4},{:4}, {:3}",
                position.x,
                position.y,
                super::unit_elevation(world, observer)?.unwrap_or(0)
            ),
            // The terrain line names the ground feature; fire or smoke over it gets its own line.
            3 => format!(
                "Terrain: {:>14}",
                terrain_name(tile.with_overlay(None).terrain())
            ),
            4 => tile.overlay().map_or_else(String::new, |overlay| {
                format!("Effect:  {:>14}", terrain_name(overlay.terrain()))
            }),
            6 => format!("Speed:           {:6.1}", unit.speed),
            7 => format!("Vertical Speed:  {:6.1}", vertical_speed),
            8 => format!("Heading:           {:4.0}", heading),
            _ => String::new(),
        };
        let compass: String = compass.into_iter().collect();
        lines.push(
            format!(
                "{}  {:26}{}",
                crate::text::escape(&compass),
                crate::text::escape(&readout),
                map_lines.get(row).unwrap_or(&"")
            )
            .trim_end()
            .to_owned(),
        );
    }
    Ok(BattleNavigationReport {
        center,
        text: lines.join("\n"),
    })
}

/// Convert normalized continuous coordinates into the bounded within-hex plot.
fn plot_cell(
    center: super::BattleHexCoordinate,
    point: super::BattlePoint,
) -> Option<(usize, usize)> {
    let center = center.center();
    let width = 2.0 / 3.0_f64.sqrt();
    let column = ((point.x - center.x + width / 2.0) / width * 21.0 + 4.0).trunc();
    let row = ((point.y - center.y + 0.5) * 9.0 + 2.0).trunc();
    if !(0.0..13.0).contains(&row) || !(0.0..28.0).contains(&column) {
        return None;
    }
    Some((row as usize, column as usize))
}

/// Display names for terrain features, and for the fire and smoke shown on the effect line.
fn terrain_name(terrain: super::Terrain) -> &'static str {
    use super::Terrain::*;
    match terrain {
        Grassland => "Grassland",
        Road => "Road",
        LightForest => "Light Forest",
        HeavyForest => "Heavy Forest",
        Water => "Water",
        Ice => "Ice",
        Bridge => "Bridge",
        Rough => "Rough",
        Mountains => "Mountains",
        Fire => "Fire",
        Smoke => "Smoke",
        Snow => "Snow",
        Building => "Building",
        Wall => "Wall",
        Sand => "Sand",
    }
}

/// Native cockpit entry point for navigation centering and rendering.
pub(crate) fn command(
    ctx: &crate::CommandContext<'_>,
    input: &crate::CommandInput,
) -> Result<crate::CommandAction> {
    let result = (|| {
        let world = ctx.scripts.world.borrow();
        let unit = world
            .objects
            .get(&ctx.player)
            .and_then(|p| p.location)
            .context("Enter a unit first")?;
        navigate(&world, unit, ctx.player, &input.args)
    })();
    Ok(crate::CommandAction::Report(match result {
        Ok(report) => crate::CommandReport::Styled(report.text),
        Err(error) => crate::CommandReport::Reply(format!("{error:#}")),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn within_hex_positions_are_bounded_and_preserve_continuous_offsets() {
        let hex = super::super::BattleHexCoordinate { x: 2, y: 2 };
        assert_eq!(plot_cell(hex, hex.center()), Some((6, 14)));
        assert_eq!(
            plot_cell(hex, hex.center().project(0.0, 0.2).unwrap()),
            Some((4, 14))
        );
        assert_eq!(
            plot_cell(hex, hex.center().project(180.0, 100.0).unwrap()),
            None
        );
    }
}
