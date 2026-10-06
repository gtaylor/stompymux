//! Local hex navigation with a continuous-position compass and live cockpit readouts.
use crate::{ObjectId, World};
use anyhow::{Context, Result};
use serde::Serialize;

/// Read-only navigation display and its requested local map center.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct NavigationReport {
    pub center: super::HexCoordinate,
    pub text: String,
}

/// Combine a radius-two map with the positions of acquired units inside the selected center hex.
pub fn navigate(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    arguments: &str,
) -> Result<NavigationReport> {
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
    let layers = layer_lines(tile);
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
            9..=12 => layers.get(row - 9).cloned().unwrap_or_default(),
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
    Ok(NavigationReport {
        center,
        text: lines.join("\n"),
    })
}

/// Convert normalized continuous coordinates into the bounded within-hex plot.
fn plot_cell(center: super::HexCoordinate, point: super::Point) -> Option<(usize, usize)> {
    let center = center.center();
    let width = 2.0 / 3.0_f64.sqrt();
    let column = ((point.x - center.x + width / 2.0) / width * 21.0 + 4.0).trunc();
    let row = ((point.y - center.y + 0.5) * 9.0 + 2.0).trunc();
    if !(0.0..13.0).contains(&row) || !(0.0..28.0).contains(&column) {
        return None;
    }
    Some((row as usize, column as usize))
}

/// The readout lines naming each layer of `hex` the terrain line can hide: its ground or
/// water, foliage, road or rail, structure and construction factor, and ice, snow or mud. A
/// hex holds at most four of them.
fn layer_lines(hex: super::Hex) -> Vec<String> {
    let mut layers: Vec<(String, String)> = Vec::new();
    match hex.water() {
        Some(water) if water.flow.is_still() => {
            layers.push(("Water:".into(), format!("Depth {}", water.depth)));
        }
        Some(water) => layers.push((
            "Water:".into(),
            format!("Depth {} {}", water.depth, water.flow.label()),
        )),
        None => layers.push(("Ground:".into(), hex.ground().label().into())),
    }
    if let Some(foliage) = hex.foliage() {
        layers.push(("Foliage:".into(), foliage.label().into()));
    }
    if let Some(route) = hex.route() {
        layers.push(("Route:".into(), route.label().into()));
    }
    if let Some(structure) = hex.structure() {
        layers.push((
            format!("{}:", structure.kind.label()),
            structure.class.label().into(),
        ));
        layers.push((
            "CF:".into(),
            format!("{}/{}", structure.cf, structure.class.construction_factor()),
        ));
    }
    if let Some(condition) = hex.condition() {
        layers.push(("Surface:".into(), condition.label().into()));
    }
    layers
        .into_iter()
        .map(|(label, value)| format!("{label:<8}{value:>18}"))
        .collect()
}

/// Display names for terrain features, and for the fire and smoke shown on the effect line.
fn terrain_name(terrain: super::Terrain) -> &'static str {
    terrain.label()
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
    use crate::btech::{
        Condition, ConstructionClass, Flow, Hex, Route, Structure, StructureKind, Terrain, Water,
    };

    /// Each layer the one-symbol terrain line can hide gets its own readout line, at most 26
    /// characters wide.
    #[test]
    fn layer_lines_name_every_layer() {
        let woods_road = Hex::new(Terrain::UltraHeavyJungle, 0)
            .with_route(Some(Route::DirtRoad))
            .with_condition(Some(Condition::DeepSnow));
        assert_eq!(
            layer_lines(woods_road),
            [
                "Ground:              Clear",
                "Foliage:Ultra-heavy jungle",
                "Route:           Dirt road",
                "Surface:         Deep snow",
            ]
        );
        let bridge = Hex::new(Terrain::Bridge, 2)
            .with_water(Some(Water {
                depth: 3,
                flow: Flow::Rapids,
            }))
            .with_structure(Some(Structure {
                cf: 35,
                ..Structure::new(StructureKind::Bridge, 2, ConstructionClass::Heavy)
            }));
        assert_eq!(
            layer_lines(bridge),
            [
                "Water:      Depth 3 Rapids",
                "Bridge:              Heavy",
                "CF:                  35/90",
            ]
        );
        for line in layer_lines(woods_road)
            .into_iter()
            .chain(layer_lines(bridge))
        {
            assert!(line.chars().count() <= 26, "{line}");
        }
    }

    #[test]
    fn within_hex_positions_are_bounded_and_preserve_continuous_offsets() {
        let hex = super::super::HexCoordinate { x: 2, y: 2 };
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
