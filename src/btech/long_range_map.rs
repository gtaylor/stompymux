//! Long-range terrain, elevation and contact maps with bounded staggered-row text rendering.
use super::{BattleViewDimensions, BattleViewKind, BattleViewport};
use crate::{ObjectId, World};
use anyhow::{Result, bail};
use serde::Serialize;
use std::collections::BTreeMap;

/// Long-range content and visibility modes; fine terrain/elevation masks remain a separate policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum BattleLongRangeMode {
    Terrain,
    Elevation,
    ColoredElevation,
    Units,
    VisibleTerrain,
    VisibleElevation,
    VisibleUnits,
    /// Terrain with any fire or smoke over it left out.
    UnderlyingTerrain,
}

impl std::str::FromStr for BattleLongRangeMode {
    type Err = anyhow::Error;
    fn from_str(value: &str) -> Result<Self> {
        let lower = value.to_ascii_lowercase();
        // Descriptive API modes share the same selector as cockpit mode initials.
        let selector = match lower.as_str() {
            "units" => Some(b'm'),
            "visible_terrain" => Some(b'l'),
            "visible_elevation" => Some(b'h'),
            "visible_units" => Some(b's'),
            "underlying_terrain" => Some(b'u'),
            _ => lower.as_bytes().first().copied(),
        };
        match selector {
            Some(b't') => Ok(Self::Terrain),
            Some(b'e') => Ok(Self::Elevation),
            Some(b'c') => Ok(Self::ColoredElevation),
            Some(b'm') => Ok(Self::Units),
            Some(b'l') => Ok(Self::VisibleTerrain),
            Some(b'h') => Ok(Self::VisibleElevation),
            Some(b's') => Ok(Self::VisibleUnits),
            Some(b'u') => Ok(Self::UnderlyingTerrain),
            _ => bail!(
                "Supported LRS sensor types: T (terrain), E (elevation), C (colored elevation), M (units), L/H/S (visible terrain/elevation/units), U (terrain under fire and smoke)"
            ),
        }
    }
}

/// A bounded, already-filtered long-range display for native and Lua clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleLongRangeMap {
    /// Clipped display geometry.
    pub viewport: BattleViewport,
    /// Styled text with filtered contact markers and terrain.
    pub text: String,
}

/// Render the requested long-range mode without changing contacts, targets, dice or equipment.
/// Ordinary maps show terrain independent of sensor acquisition; dark maps mask unseen hexes.
/// Unit markers require current acquired visibility; a selected scanner marker is '*'.
pub fn long_range_map(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    mode: BattleLongRangeMode,
    arguments: &str,
    dimensions: BattleViewDimensions,
) -> Result<BattleLongRangeMap> {
    let viewport = super::resolve_viewport(
        world,
        observer,
        pilot,
        BattleViewKind::LongRange,
        arguments,
        dimensions,
    )?;
    render_viewport(world, observer, pilot, mode, viewport)
}

/// Native and Lua requests share cockpit, centering and mode-error ordering.
pub(crate) fn from_arguments(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    mode: &str,
    arguments: &str,
    dimensions: BattleViewDimensions,
) -> Result<BattleLongRangeMap> {
    let viewport = super::resolve_viewport(
        world,
        observer,
        pilot,
        BattleViewKind::LongRange,
        arguments,
        dimensions,
    )?;
    render_viewport(world, observer, pilot, mode.parse()?, viewport)
}

/// Render an admitted viewport with the same terrain and contact rules for every caller.
fn render_viewport(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    mode: BattleLongRangeMode,
    viewport: BattleViewport,
) -> Result<BattleLongRangeMap> {
    let observer = super::combat_operator::for_owner(world, observer, pilot)?
        .source
        .unit;
    let map = &world.btech.maps()[&viewport.map];
    let ansi = world.objects[&pilot].flags.contains(crate::Flag::Ansi);
    let mut occupants = Vec::new();
    if matches!(
        mode,
        BattleLongRangeMode::Units | BattleLongRangeMode::VisibleUnits
    ) {
        let mut reader = None;
        for id in super::map_slots::all_unit_order(world, viewport.map)? {
            let unit = super::scanner::scanner_unit(world, id).expect("placed map unit");
            let Some(position) = unit.position else {
                continue;
            };
            let glyph = if id == observer {
                '*'
            } else {
                // The reference includes the lower boundary row in sorting, even though
                // that row is not drawn. Off-viewport contacts must not reorder a stack.
                if i32::from(position.x) < viewport.origin.x
                    || i32::from(position.x) >= viewport.origin.x + i32::from(viewport.width)
                    || i32::from(position.y) < viewport.origin.y
                    || i32::from(position.y) > viewport.origin.y + i32::from(viewport.height)
                {
                    continue;
                }
                if reader.is_none() {
                    reader = Some(super::contacts::ContactReader::new(world, observer)?);
                }
                let Some(view) = reader.as_ref().expect("reader built").view(id)? else {
                    continue;
                };
                let glyph = unit_glyph(world, id);
                if view.friendly {
                    glyph
                } else {
                    glyph.to_ascii_uppercase()
                }
            };
            occupants.push((
                (i32::from(position.y), i32::from(position.x)),
                Cell {
                    glyph,
                    style: if ansi {
                        super::map_style::contact(id == observer, glyph.is_ascii_lowercase())
                    } else {
                        ""
                    },
                },
            ));
        }
    }
    let units = stack_markers(occupants);
    let viewer = super::hex_visibility::HexViewer::new(world, observer);
    let mut cells = Vec::with_capacity(usize::from(viewport.width) * usize::from(viewport.height));
    for y in viewport.origin.y..viewport.origin.y + i32::from(viewport.height) {
        for x in viewport.origin.x..viewport.origin.x + i32::from(viewport.width) {
            if let Some(&cell) = units.get(&(x, y)) {
                cells.push(cell);
                continue;
            }
            if (map.has_flag(super::BattleMapFlag::Dark)
                || matches!(
                    mode,
                    BattleLongRangeMode::VisibleTerrain
                        | BattleLongRangeMode::VisibleElevation
                        | BattleLongRangeMode::VisibleUnits
                ))
                && !viewer.visible(super::BattleHexCoordinate { x, y })?
            {
                cells.push(Cell {
                    glyph: '?',
                    style: "[fg=blue]",
                });
                continue;
            }
            let hex = if mode == BattleLongRangeMode::UnderlyingTerrain {
                map.base_hex(i64::from(x), i64::from(y))?
            } else {
                map.hex(i64::from(x), i64::from(y))?
            };
            let glyph = match mode {
                BattleLongRangeMode::Elevation
                | BattleLongRangeMode::ColoredElevation
                | BattleLongRangeMode::VisibleElevation => {
                    match super::map_style::shown_height(hex) {
                        0 if matches!(
                            mode,
                            BattleLongRangeMode::Elevation | BattleLongRangeMode::VisibleElevation
                        ) =>
                        {
                            ' '
                        }
                        elevation => stompymux_map::height_glyph(elevation),
                    }
                }
                _ => hex.terrain().symbol(),
            };
            let colored = mode == BattleLongRangeMode::ColoredElevation
                || (ansi
                    && !matches!(
                        mode,
                        BattleLongRangeMode::Elevation | BattleLongRangeMode::VisibleElevation
                    ));
            cells.push(Cell {
                glyph,
                style: if colored {
                    super::map_style::terrain(hex)
                } else {
                    ""
                },
            });
        }
    }
    Ok(BattleLongRangeMap {
        text: render(viewport, &cells),
        viewport,
    })
}

/// One filtered display glyph and its trusted style, independent of text escaping.
#[derive(Clone, Copy)]
struct Cell {
    glyph: char,
    style: &'static str,
}

/// Select one marker per hex after the cockpit's positional exchange ordering.
/// Equal positions are never exchanged directly, but moving earlier positions
/// through the list can reorder a stack. A stable sort would change its winner.
fn stack_markers(mut occupants: Vec<((i32, i32), Cell)>) -> BTreeMap<(i32, i32), Cell> {
    for first in 0..occupants.len() {
        for next in first + 1..occupants.len() {
            if occupants[first].0 > occupants[next].0 {
                occupants.swap(first, next);
            }
        }
    }
    let mut markers = BTreeMap::new();
    for ((y, x), cell) in occupants {
        markers.entry((x, y)).or_insert(cell);
    }
    markers
}

/// Coalesce adjacent styles while escaping every data glyph; row labels remain outside styling.
fn append_cell(text: &mut String, active: &mut &'static str, cell: Cell) {
    if *active != cell.style {
        text.push_str("[reset]");
        text.push_str(cell.style);
        *active = cell.style;
    }
    text.push_str(&crate::text::escape(&cell.glyph.to_string()));
}

/// Render staggered hex rows beneath the cockpit's fixed three coordinate-label rows.
fn render(viewport: BattleViewport, cells: &[Cell]) -> String {
    let width = usize::from(viewport.width);
    let last_x = viewport.origin.x + i32::from(viewport.width) - 1;
    let labels: Vec<_> = (viewport.origin.x..=last_x)
        .map(|x| format!("{x:3}").into_bytes())
        .collect();
    let mut lines = Vec::new();
    // Coordinates above 999 retain their first three digits in the reference display.
    for row in 0..3 {
        let mut line = "    ".to_owned();
        for label in &labels {
            line.push(char::from(label[row]));
        }
        lines.push(line);
    }
    for (row, cells) in cells.chunks_exact(width).enumerate() {
        let y = viewport.origin.y + row as i32;
        let mut top = format!("{y:3} ");
        let mut bottom = "    ".to_owned();
        let mut top_style = "";
        let mut bottom_style = "";
        for (column, &cell) in cells.iter().enumerate() {
            let odd = (viewport.origin.x + column as i32).rem_euclid(2) == 1;
            if odd {
                append_cell(&mut top, &mut top_style, cell);
                bottom.push(' ');
            } else {
                top.push(' ');
                append_cell(&mut bottom, &mut bottom_style, cell);
            }
        }
        if !top_style.is_empty() {
            top.push_str("[reset]");
        }
        if !bottom_style.is_empty() {
            bottom.push_str("[reset]");
        }
        bottom.push_str(&format!(" {y:<3}"));
        lines.push(top);
        lines.push(bottom);
    }
    lines.join("\n")
}

/// Native long-range maps require a content mode, followed by the shared centering grammar.
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
            .ok_or_else(|| anyhow::anyhow!("Enter a unit first"))?;
        let args = input.args.trim();
        let (mode, center) = args.split_once(char::is_whitespace).unwrap_or((args, ""));
        from_arguments(
            &world,
            unit,
            ctx.player,
            mode,
            center,
            super::view_dimensions(&world, ctx.player)?,
        )
    })();
    Ok(crate::CommandAction::Report(match result {
        Ok(report) => crate::CommandReport::Styled(report.text),
        Err(error) => crate::CommandReport::Reply(format!("{error:#}")),
    }))
}

/// Chassis markers retain the reference movement alphabet; affiliation sets case separately.
fn unit_glyph(world: &World, id: ObjectId) -> char {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return match unit.definition().movement {
            super::BattleVehicleMovement::Tracked => 't',
            super::BattleVehicleMovement::Wheeled => 'w',
            super::BattleVehicleMovement::Hover => 'h',
            super::BattleVehicleMovement::Vtol => 'v',
            super::BattleVehicleMovement::Stationary => 'u',
        };
    }
    match world.btech.constructed_units()[&id].chassis() {
        super::BattleMechChassis::Biped => 'b',
        super::BattleMechChassis::Quad => 'q',
    }
}

#[cfg(test)]
mod tests {
    use super::BattleLongRangeMode as Mode;

    /// Equal-position winners depend on intervening positional exchanges, not self priority.
    #[test]
    fn stacked_markers_follow_positional_exchange_order() {
        use super::{Cell, stack_markers};
        for (positions, expected) in [
            (vec![((2, 1), '*'), ((2, 1), 'v')], '*'),
            (vec![((2, 1), 'v'), ((2, 1), '*')], 'v'),
            (vec![((2, 1), '*'), ((2, 1), 'v'), ((1, 2), 'q')], 'v'),
            (vec![((2, 1), '*'), ((2, 1), 'v'), ((2, 0), 'q')], 'v'),
            (vec![((2, 1), '*'), ((2, 1), 'v'), ((3, 0), 'q')], '*'),
        ] {
            let markers = stack_markers(
                positions
                    .into_iter()
                    .map(|(position, glyph)| {
                        (
                            position,
                            Cell {
                                glyph,
                                style: "[fg=red]",
                            },
                        )
                    })
                    .collect(),
            );
            assert_eq!(markers[&(1, 2)].glyph, expected);
            assert_eq!(markers[&(1, 2)].style, "[fg=red]");
        }
        assert!(stack_markers(Vec::new()).is_empty());
    }

    /// Large coordinates keep three heading rows without shifting neighboring labels or hexes.
    #[test]
    fn coordinate_labels_keep_reference_three_row_layout() {
        use super::{BattleViewport, Cell, render};
        use crate::{ObjectId, btech::BattleHexCoordinate};

        for (x, labels) in [
            (8, ["    ", "  11", "8901"]),
            (98, ["  11", "9900", "8901"]),
            (998, ["9911", "9900", "8900"]),
            (9998, ["9911", "9900", "9900"]),
        ] {
            let origin = BattleHexCoordinate { x, y: 7 };
            let viewport = BattleViewport {
                map: ObjectId(1),
                requested_center: origin,
                origin,
                width: 4,
                height: 1,
                maximum_range: 8,
            };
            let cells = ['a', 'b', 'c', 'd'].map(|glyph| Cell { glyph, style: "" });
            let output = render(viewport, &cells);
            let rows: Vec<_> = output.lines().collect();
            assert_eq!(rows.len(), 5, "x={x}");
            for row in 0..3 {
                assert_eq!(rows[row], format!("    {}", labels[row]), "x={x}");
            }
            assert_eq!(rows[3], "  7  b d");
            assert_eq!(rows[4], "    a c  7  ");
        }
    }

    /// Cockpit mode initials and descriptive API names resolve without indexing empty input.
    #[test]
    fn mode_initials_and_descriptive_names() {
        for (mode, inputs) in [
            (Mode::Terrain, ["T", "terrain", "Terrain-map"]),
            (Mode::Elevation, ["E", "elevation", "Elevations"]),
            (
                Mode::ColoredElevation,
                ["C", "colored_elevation", "Combined"],
            ),
            (Mode::Units, ["M", "units", "Mechs"]),
            (Mode::VisibleTerrain, ["L", "visible_terrain", "LOS"]),
            (Mode::VisibleElevation, ["H", "visible_elevation", "Height"]),
            (Mode::VisibleUnits, ["S", "visible_units", "Sensors"]),
            (
                Mode::UnderlyingTerrain,
                ["U", "underlying_terrain", "Under"],
            ),
        ] {
            for input in inputs {
                assert_eq!(input.parse::<Mode>().unwrap(), mode);
            }
        }
        for input in ["", "?", "view", "🛰"] {
            assert!(input.parse::<Mode>().is_err());
        }
    }
}
