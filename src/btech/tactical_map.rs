//! Bounded tactical hex displays with live terrain and acquired contact labels.
use super::{BattleHexCoordinate, BattleViewDimensions, BattleViewKind, BattleViewport, Terrain};
use crate::{ObjectId, World};
use anyhow::{Result, bail};
use serde::Serialize;

/// A filtered tactical display shared by native and Lua clients.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleTacticalMap {
    /// Clipped geometry, including the requested display center.
    pub viewport: BattleViewport,
    /// Escaped, styled text; rendering has no game-state side effects.
    pub text: String,
}

/// One display character and trusted palette style.
#[derive(Clone, Copy)]
struct Pixel {
    glyph: char,
    style: &'static str,
}

impl Pixel {
    /// An unstyled structural character.
    fn plain(glyph: char) -> Self {
        Self { glyph, style: "" }
    }
}

/// Render standard terrain, L visibility, U underlying terrain, C/T cliffs, B landing zones or M mines.
/// Remaining arguments use the shared own-unit, acquired-contact or bearing/range grammar.
/// Two-character contact cells show the first two characters of the battlefield label.
pub fn tactical_map(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    arguments: &str,
    dimensions: BattleViewDimensions,
) -> Result<BattleTacticalMap> {
    display(world, observer, pilot, arguments, dimensions, false)
}

/// Local navigation view retains off-map cells and uses a radius-two hex outline without labels.
pub(super) fn navigation_hex_map(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    arguments: &str,
) -> Result<BattleTacticalMap> {
    display(
        world,
        observer,
        pilot,
        arguments,
        BattleViewDimensions::default(),
        true,
    )
}

/// Build filtered cells once for rectangular tactical views and local navigation views.
fn display(
    world: &World,
    observer: ObjectId,
    pilot: ObjectId,
    arguments: &str,
    dimensions: BattleViewDimensions,
    navigation: bool,
) -> Result<BattleTacticalMap> {
    let arguments = arguments.trim();
    let (first, rest) = arguments
        .split_once(char::is_whitespace)
        .unwrap_or((arguments, ""));
    let flag = !navigation && first.len() == 1 && first.bytes().all(|b| b.is_ascii_alphabetic());
    let landing = flag && first.eq_ignore_ascii_case("B");
    let mines = flag && first.eq_ignore_ascii_case("M");
    let (visible, underlying, cliff, center) = if flag {
        match first.to_ascii_uppercase().as_str() {
            "L" => (true, false, None, rest),
            "U" => (false, true, None, rest),
            "C" => (false, false, Some(3), rest),
            "T" => (false, false, Some(2), rest),
            "B" | "M" => (false, false, None, rest),
            _ => bail!("Invalid tactical map flag."),
        }
    } else {
        (false, false, None, arguments)
    };
    let viewport = if navigation {
        let position = super::view_center::navigation_center(world, observer, pilot, arguments)?;
        BattleViewport {
            map: position.map,
            requested_center: position.center,
            origin: BattleHexCoordinate {
                x: position.center.x - 2,
                y: position.center.y - 2,
            },
            width: 5,
            height: 5,
            maximum_range: position.maximum_range,
        }
    } else {
        super::resolve_viewport(
            world,
            observer,
            pilot,
            BattleViewKind::Tactical,
            center,
            dimensions,
        )?
    };
    let observer = super::combat_operator::for_owner(world, observer, pilot)?
        .source
        .unit;
    let map = &world.btech.maps()[&viewport.map];
    if (cliff.is_some() || landing) && map.has_flag(super::BattleMapFlag::Dark) {
        bail!("You can't see that much here!");
    }
    let ansi = world.objects[&pilot].flags.contains(crate::Flag::Ansi);
    let width = usize::from(viewport.width);
    let height = usize::from(viewport.height);
    let mut canvas = terrain_canvas(world, viewport, Some(observer), visible, underlying, ansi)?;
    if mines {
        draw_mines(world, observer, map, viewport, &mut canvas)?;
    }
    // Stable map order resolves stacked contacts. The scanner's own marker has final priority.
    let mut occupied = std::collections::BTreeSet::new();
    let mut reader = None;
    for id in super::map_slots::all_unit_order(world, viewport.map)?
        .into_iter()
        .filter(|id| *id != observer)
        .chain(std::iter::once(observer))
    {
        if (cliff.is_some() || landing) && id != observer {
            continue;
        }
        let unit = super::scanner::scanner_unit(world, id).expect("placed map unit");
        let Some(position) = unit.position else {
            continue;
        };
        let x = i32::from(position.x) - viewport.origin.x;
        let y = i32::from(position.y) - viewport.origin.y;
        if x < 0 || y < 0 || x >= i32::from(viewport.width) || y >= i32::from(viewport.height) {
            continue;
        }
        let (label, friendly) = if id == observer {
            ("**".to_owned(), true)
        } else {
            if reader.is_none() {
                reader = Some(super::contacts::ContactReader::new(world, observer)?);
            }
            let Some(view) = reader.as_ref().expect("reader built").view(id)? else {
                continue;
            };
            let mut label = unit.label().expect("placed contact");
            if view.friendly {
                label.make_ascii_lowercase();
            }
            (label, view.friendly)
        };
        if !occupied.insert((x, y)) && id != observer {
            continue;
        }
        let row = y as usize * 2 + usize::from(i32::from(position.x).rem_euclid(2) == 0);
        let column = x as usize * 3 + 1;
        for (offset, glyph) in label.chars().take(2).enumerate() {
            canvas[row][column + offset] = Pixel {
                glyph,
                style: if ansi {
                    super::map_style::contact(id == observer, friendly)
                } else {
                    ""
                },
            };
        }
    }
    if landing {
        let team = super::scanner::scanner_unit(world, observer)
            .expect("checked scanner")
            .signature
            .team;
        for y in 0..height {
            for x in 0..width {
                let coordinate = BattleHexCoordinate {
                    x: viewport.origin.x + x as i32,
                    y: viewport.origin.y + y as i32,
                };
                let ready = map.landing_suitability(coordinate, team)?
                    == super::BattleLandingSuitability::Ready;
                let row = y * 2 + usize::from(coordinate.x.rem_euclid(2) == 0) + 1;
                canvas[row][x * 3 + 1] = Pixel {
                    glyph: if ready { 'O' } else { 'X' },
                    style: if !ansi {
                        ""
                    } else if ready {
                        "[fg=green bold]"
                    } else {
                        "[fg=red bold]"
                    },
                };
            }
        }
    }
    if let Some(threshold) = cliff {
        draw_cliffs(map, viewport, &mut canvas, threshold, ansi)?;
    }
    Ok(BattleTacticalMap {
        viewport,
        text: if navigation {
            render_local(viewport, &canvas)
        } else {
            render(viewport, &canvas)
        },
    })
}

/// Draw the same terrain and hex edges for cockpit and map-only views.
fn terrain_canvas(
    world: &World,
    viewport: BattleViewport,
    observer: Option<ObjectId>,
    visible: bool,
    underlying: bool,
    ansi: bool,
) -> Result<Vec<Vec<Pixel>>> {
    let map = &world.btech.maps()[&viewport.map];
    let width = usize::from(viewport.width);
    let height = usize::from(viewport.height);
    let columns = width * 3 + 1;
    let mut canvas = vec![vec![Pixel::plain(' '); columns]; height * 2 + 1];
    // Hex edges alternate by global column parity, including clipped views starting on odd columns.
    for (row, line) in canvas.iter_mut().enumerate() {
        let odd = (viewport.origin.x + row as i32).rem_euclid(2) == 1;
        let pattern = if odd {
            ['/', ']', '[', '\\', ']', '[']
        } else {
            ['\\', ']', '[', '/', ']', '[']
        };
        for (column, pixel) in line.iter_mut().enumerate() {
            *pixel = Pixel::plain(pattern[column % 6]);
        }
    }
    let viewer = observer.map(|observer| super::hex_visibility::HexViewer::new(world, observer));
    for y in 0..height {
        for x in 0..width {
            let coordinate = BattleHexCoordinate {
                x: viewport.origin.x + x as i32,
                y: viewport.origin.y + y as i32,
            };
            if coordinate.x < 0
                || coordinate.y < 0
                || i64::from(coordinate.x) >= map.width
                || i64::from(coordinate.y) >= map.height
            {
                continue;
            }
            let row = y * 2 + usize::from(coordinate.x.rem_euclid(2) == 0);
            let column = x * 3 + 1;
            let seen = match &viewer {
                Some(viewer) => {
                    !(visible || map.has_flag(super::BattleMapFlag::Dark))
                        || viewer.visible(coordinate)?
                }
                None => true,
            };
            let pixels = if !seen {
                [Pixel {
                    glyph: '?',
                    style: if ansi { "[fg=blue]" } else { "" },
                }; 4]
            } else {
                let hex = map.hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
                let base = map.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
                let (top, bottom) = match hex.terrain() {
                    Terrain::Grassland => (' ', '_'),
                    Terrain::Bridge => ('#', '+'),
                    Terrain::Fire | Terrain::Smoke if underlying => {
                        (hex.terrain().symbol(), base.terrain().symbol())
                    }
                    terrain => (terrain.symbol(), terrain.symbol()),
                };
                let style = if ansi {
                    super::map_style::terrain(hex.terrain(), hex.elevation())
                } else {
                    ""
                };
                let elevation = match hex.elevation() {
                    0 => bottom,
                    1..=9 => char::from(b'0' + hex.elevation()),
                    _ => '?',
                };
                let bottom_style = if ansi
                    && underlying
                    && matches!(hex.terrain(), Terrain::Fire | Terrain::Smoke)
                {
                    super::map_style::terrain(base.terrain(), base.elevation())
                } else {
                    style
                };
                [
                    Pixel { glyph: top, style },
                    Pixel { glyph: top, style },
                    Pixel {
                        glyph: bottom,
                        style: bottom_style,
                    },
                    Pixel {
                        glyph: elevation,
                        style: bottom_style,
                    },
                ]
            };
            canvas[row][column..column + 2].copy_from_slice(&pixels[..2]);
            canvas[row + 1][column..column + 2].copy_from_slice(&pixels[2..]);
        }
    }
    Ok(canvas)
}

/// Render a labelled map-only view without unit overlays or scanner admission.
pub(super) fn map_view(
    world: &World,
    map: ObjectId,
    player: ObjectId,
    center: BattleHexCoordinate,
    dimensions: BattleViewDimensions,
) -> Result<BattleTacticalMap> {
    let record = world
        .btech
        .maps()
        .get(&map)
        .ok_or_else(|| anyhow::anyhow!("Map not found"))?;
    let viewport = super::viewport::map_viewport(map, record, center, dimensions)?;
    let ansi = world
        .objects
        .get(&player)
        .is_some_and(|player| player.flags.contains(crate::Flag::Ansi));
    let canvas = terrain_canvas(world, viewport, None, false, false, ansi)?;
    Ok(BattleTacticalMap {
        viewport,
        text: render(viewport, &canvas),
    })
}

/// Select the radius-two polygon from the staggered canvas, with explicit boundary roofs.
fn render_local(viewport: BattleViewport, canvas: &[Vec<Pixel>]) -> String {
    let mut output = vec![vec![Pixel::plain(' '); 16]; 12];
    for y in 0..5 {
        for x in 0..5 {
            let coordinate = BattleHexCoordinate {
                x: viewport.origin.x + x as i32,
                y: viewport.origin.y + y as i32,
            };
            if coordinate.distance(viewport.requested_center) > 2 {
                continue;
            }
            let row = y * 2 + usize::from(coordinate.x.rem_euclid(2) == 0);
            let column = x * 3;
            output[row + 1][column..column + 4].copy_from_slice(&canvas[row][column..column + 4]);
            output[row + 2][column..column + 4]
                .copy_from_slice(&canvas[row + 1][column..column + 4]);
            let north = BattleHexCoordinate {
                x: coordinate.x,
                y: coordinate.y - 1,
            };
            if north.distance(viewport.requested_center) > 2 {
                output[row][column + 1..column + 3].fill(Pixel::plain('_'));
            }
        }
    }
    let start = usize::from(output[0].iter().all(|p| p.glyph == ' '));
    let mut lines = Vec::new();
    for row in &output[start..start + 11] {
        let end = row
            .iter()
            .rposition(|p| p.glyph != ' ')
            .map_or(0, |i| i + 1);
        let mut line = String::new();
        append_pixels(&mut line, &row[..end]);
        lines.push(line);
    }
    lines.join("\n")
}

/// Show the first authored field per hex when non-trigger and currently visible through terrain.
/// This read-only display neither recognizes mines nor exposes their kind, owner or strength.
fn draw_mines(
    world: &World,
    observer: ObjectId,
    map: &super::StoredBattleMap,
    viewport: BattleViewport,
    canvas: &mut [Vec<Pixel>],
) -> Result<()> {
    let mut fields = std::collections::BTreeMap::new();
    for (_, mine) in map.ordered_minefields() {
        fields
            .entry((mine.coordinate.x, mine.coordinate.y))
            .or_insert(mine.kind);
    }
    let viewer = super::hex_visibility::HexViewer::new(world, observer);
    for y in 0..usize::from(viewport.height) {
        for x in 0..usize::from(viewport.width) {
            let coordinate = BattleHexCoordinate {
                x: viewport.origin.x + x as i32,
                y: viewport.origin.y + y as i32,
            };
            let row = y * 2 + usize::from(coordinate.x.rem_euclid(2) == 0);
            let column = x * 3 + 1;
            let elevation = canvas[row + 1][column + 1];
            if elevation.glyph.is_ascii_digit() {
                canvas[row][column + 1] = elevation;
            }
            canvas[row + 1][column..column + 2].fill(Pixel::plain(' '));
            let Some(kind) = fields.get(&(coordinate.x, coordinate.y)) else {
                continue;
            };
            if *kind == super::BattleMineKind::Trigger
                || !viewer.visible(coordinate)?
                || !super::visibility::hex_unblocked(world, observer, coordinate)?
            {
                continue;
            }
            canvas[row + 1][column] = Pixel::plain('<');
            canvas[row + 1][column + 1] = Pixel::plain('>');
        }
    }
    Ok(())
}

/// Draw each shared southern edge once; only adjacent tiles inside the viewport participate.
/// Water and ice depths are negative, independent of temporary surface decorations.
fn draw_cliffs(
    map: &super::StoredBattleMap,
    viewport: BattleViewport,
    canvas: &mut [Vec<Pixel>],
    threshold: i16,
    ansi: bool,
) -> Result<()> {
    let width = usize::from(viewport.width);
    let height = usize::from(viewport.height);
    let mut elevations = vec![vec![0i16; width]; height];
    for (y, line) in elevations.iter_mut().enumerate() {
        for (x, elevation) in line.iter_mut().enumerate() {
            let tile = map.base_hex(
                i64::from(viewport.origin.x) + x as i64,
                i64::from(viewport.origin.y) + y as i64,
            )?;
            *elevation = i16::from(tile.elevation())
                * if matches!(tile.terrain(), Terrain::Water | Terrain::Ice) {
                    -1
                } else {
                    1
                };
        }
    }
    let edge = |glyph, marker| Pixel {
        glyph: if ansi { glyph } else { marker },
        style: if ansi { "[fg=red bold]" } else { "" },
    };
    for y in 0..height {
        for x in 0..width {
            let even = (viewport.origin.x + x as i32).rem_euclid(2) == 0;
            let row = y * 2 + usize::from(even);
            let column = x * 3 + 1;
            // Move the displayed elevation out of the bottom edge, preserving the own-unit marker.
            if canvas[row][column].glyph != '*'
                && canvas[row + 1][column + 1].glyph.is_ascii_digit()
            {
                canvas[row][column + 1] = canvas[row + 1][column + 1];
            }
            let south =
                y + 1 < height && (elevations[y + 1][x] - elevations[y][x]).abs() >= threshold;
            let bottom = if south {
                edge('_', ',')
            } else {
                Pixel::plain('_')
            };
            canvas[row + 1][column..column + 2].fill(bottom);
            let diagonal_y = y + usize::from(even);
            if diagonal_y >= height {
                continue;
            }
            if x > 0 && (elevations[diagonal_y][x - 1] - elevations[y][x]).abs() >= threshold {
                canvas[row + 1][column - 1] = edge('\\', '|');
            }
            if x + 1 < width
                && (elevations[diagonal_y][x + 1] - elevations[y][x]).abs() >= threshold
            {
                canvas[row + 1][column + 2] = edge('/', '!');
            }
        }
    }
    Ok(())
}

/// Add full coordinate labels outside the escaped and independently styled canvas rows.
fn render(viewport: BattleViewport, canvas: &[Vec<Pixel>]) -> String {
    let last_x = viewport.origin.x + i32::from(viewport.width) - 1;
    let digits = last_x.to_string().len().max(3);
    let labels: Vec<_> = (viewport.origin.x..=last_x)
        .map(|x| format!("{x:digits$}").into_bytes())
        .collect();
    let mut lines = Vec::new();
    for row in 0..digits {
        let mut line = "     ".to_owned();
        for label in &labels {
            line.push(char::from(label[row]));
            line.push_str("  ");
        }
        lines.push(line);
    }
    for (row, pixels) in canvas.iter().enumerate() {
        let label = (row % 2 == 1).then(|| viewport.origin.y + (row / 2) as i32);
        let mut line = label.map_or_else(|| "    ".to_owned(), |y| format!("{y:3} "));
        append_pixels(&mut line, pixels);
        if let Some(y) = label {
            line.push_str(&format!(" {y:3}"));
        }
        lines.push(line);
    }
    lines.join("\n")
}

/// Native cockpit entry point for the shared tactical renderer.
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
        tactical_map(
            &world,
            unit,
            ctx.player,
            &input.args,
            super::view_dimensions(&world, ctx.player)?,
        )
    })();
    Ok(crate::CommandAction::Report(match result {
        Ok(report) => crate::CommandReport::Styled(report.text),
        Err(error) => crate::CommandReport::Reply(format!("{error:#}")),
    }))
}

/// Escape glyphs and reset styles before adjoining UI text.
fn append_pixels(line: &mut String, pixels: &[Pixel]) {
    let mut active = "";
    for pixel in pixels {
        if active != pixel.style {
            if !active.is_empty() {
                line.push_str("[reset]");
            }
            line.push_str(pixel.style);
            active = pixel.style;
        }
        line.push_str(&crate::text::escape(&pixel.glyph.to_string()));
    }
    if !active.is_empty() {
        line.push_str("[reset]");
    }
}
