//! The Regions tool: outlining a map's regions, its inspector controls, and the corner markers
//! and name labels drawn over the map.
//!
//! A region is outlined by corner hexes in order and holds every hex on or inside the outline;
//! see [`MapRegion`]. The shader tints and outlines region hexes (see `RegionMask` in
//! `render.rs`); this module adds the numbered corners of the selected region and each region's
//! name.
//!
//! With the tool selected and no region selected, pressing on a region's hex selects that
//! region, and pressing anywhere else starts a new region with its first corner there. With a
//! region selected, pressing on one of its corners selects the corner, and pressing anywhere
//! else inserts a new corner after the selected one (or at the end). Dragging moves the
//! selected corner. Escape or Done lets go of the region, so the next press can select another.
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Size, Theme,
    widget::{button, column, container, pin, row, scrollable, space, stack, text, text_input},
};
use stompymux_map::{HexCoordinate, MapRegion};

use crate::{document::Document, map_view::Camera};

/// Widget id of the selected region's name input, which a new region focuses.
pub const NAME_INPUT: &str = "region-name";

/// The type a new region takes when no region was selected before to copy it from.
const DEFAULT_KIND: &str = "deployment";

/// Below this hex radius in pixels, regions drop their name labels.
const LABEL_RADIUS: f32 = 10.0;

/// Smallest and largest corner marker size in pixels.
const MIN_CORNER: f32 = 14.0;
const MAX_CORNER: f32 = 22.0;

/// Fill of a corner marker, and of the selected corner.
const CORNER_COLOR: Color = Color::WHITE;
const SELECTED_CORNER_COLOR: Color = Color::from_rgb(0.20, 0.85, 1.0);

/// Corner buttons per row in the inspector.
const CORNERS_PER_ROW: usize = 4;

/// A text field of a region, which typing into edits as one undoable step until another field
/// or region is edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Kind,
    Name,
}

/// A change to the regions from the inspector or the keyboard.
#[derive(Debug, Clone)]
pub enum RegionEdit {
    /// Select the region with this index.
    Select(usize),
    /// Select the selected region's corner with this index.
    SelectCorner(usize),
    /// The selected region's type was typed into.
    Kind(String),
    /// The selected region's name was typed into.
    Name(String),
    /// Remove the selected corner, and the region with it if it was the last.
    DeleteCorner,
    /// Remove the selected region.
    DeleteRegion,
    /// Remove the selected corner if there is one, or else the selected region.
    Delete,
    /// Let go of the selected region.
    Done,
}

/// The selected region and corner, and the edit in progress.
#[derive(Debug, Clone, Default)]
pub struct RegionsPanel {
    /// Index of the selected region, which may be stale after an undo; see
    /// [`RegionsPanel::selected`].
    selected: Option<usize>,
    /// Index of the selected corner of the selected region; see [`RegionsPanel::corner`].
    corner: Option<usize>,
    /// The field being typed into, which a change of field or region closes into an edit.
    typing: Option<(usize, Field)>,
    /// The type of the last region selected, which a new region copies.
    last_kind: Option<String>,
}

impl RegionsPanel {
    /// The selected region's index, if it still names a region in `document`.
    pub fn selected(&self, document: &Document) -> Option<usize> {
        self.selected
            .filter(|&index| index < document.map.regions.len())
    }

    /// The selected corner's index, if it still names a corner of the selected region.
    pub fn corner(&self, document: &Document) -> Option<usize> {
        let region = &document.map.regions[self.selected(document)?];
        self.corner.filter(|&index| index < region.corners.len())
    }

    /// Forget the selection, as when another map replaces this one.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Forget half-finished typing, which the document may no longer match after an undo or
    /// redo.
    pub fn forget_typing(&mut self) {
        self.typing = None;
    }

    /// Select region `index`, or none, with no corner selected.
    fn select(&mut self, document: &Document, index: Option<usize>) {
        self.selected = index;
        self.corner = None;
        self.typing = None;
        if let Some(region) = index.and_then(|index| document.map.regions.get(index)) {
            self.last_kind = Some(region.kind.clone());
        }
    }

    /// The left button went down on `at`; see the module documentation for what that does.
    /// Changes are part of a drag that [`Document::end_stroke`] closes. Returns whether a new
    /// region was started.
    pub fn press(&mut self, document: &mut Document, at: HexCoordinate) -> bool {
        document.end_stroke();
        self.typing = None;
        let (Ok(x), Ok(y)) = (u16::try_from(at.x), u16::try_from(at.y)) else {
            return false;
        };
        let mut regions = document.map.regions.clone();
        if let Some(index) = self.selected(document) {
            let corners = &mut regions[index].corners;
            let current = self.corner(document);
            // Prefer the selected corner when several share the hex.
            let existing = current
                .filter(|&corner| corners[corner] == [x, y])
                .or_else(|| corners.iter().position(|&corner| corner == [x, y]));
            if let Some(corner) = existing {
                self.corner = Some(corner);
                return false;
            }
            let insert = current.map_or(corners.len(), |corner| corner + 1);
            corners.insert(insert, [x, y]);
            self.corner = Some(insert);
            document.drag_regions(regions);
            return false;
        }
        if let Some((index, _)) = document.regions_at(at).next() {
            self.select(document, Some(index));
            return false;
        }
        regions.push(MapRegion {
            kind: self.last_kind.clone().unwrap_or(DEFAULT_KIND.into()),
            name: unused_name(&regions),
            corners: vec![[x, y]],
        });
        self.selected = Some(regions.len() - 1);
        self.corner = Some(0);
        document.drag_regions(regions);
        true
    }

    /// The pointer was dragged onto `at` with the button held: move the selected corner there.
    pub fn drag(&mut self, document: &mut Document, at: HexCoordinate) {
        let (Some(index), Some(corner)) = (self.selected(document), self.corner(document)) else {
            return;
        };
        let (Ok(x), Ok(y)) = (u16::try_from(at.x), u16::try_from(at.y)) else {
            return;
        };
        let mut regions = document.map.regions.clone();
        if regions[index].corners[corner] == [x, y] {
            return;
        }
        regions[index].corners[corner] = [x, y];
        document.drag_regions(regions);
    }

    /// Apply a change from the inspector or the keyboard.
    pub fn edit(&mut self, document: &mut Document, edit: RegionEdit) {
        match edit {
            RegionEdit::Select(index) => {
                document.end_stroke();
                self.select(document, Some(index));
                return;
            }
            RegionEdit::Done => {
                document.end_stroke();
                self.select(document, None);
                return;
            }
            RegionEdit::SelectCorner(corner) => {
                document.end_stroke();
                self.typing = None;
                self.corner = Some(corner);
                return;
            }
            _ => {}
        }
        let Some(index) = self.selected(document) else {
            return;
        };
        let mut regions = document.map.regions.clone();
        let field = match edit {
            RegionEdit::Delete if self.corner(document).is_some() => {
                return self.edit(document, RegionEdit::DeleteCorner);
            }
            RegionEdit::Delete | RegionEdit::DeleteRegion => {
                regions.remove(index);
                self.select(document, None);
                document.set_regions(regions);
                return;
            }
            RegionEdit::DeleteCorner => {
                let Some(corner) = self.corner(document) else {
                    return;
                };
                let corners = &mut regions[index].corners;
                corners.remove(corner);
                if corners.is_empty() {
                    regions.remove(index);
                    self.select(document, None);
                } else {
                    // The corner before keeps the place, so the next press adds back here.
                    self.corner = Some(corner.saturating_sub(1));
                    self.typing = None;
                }
                document.set_regions(regions);
                return;
            }
            RegionEdit::Kind(kind) => {
                self.last_kind = Some(kind.clone());
                regions[index].kind = kind;
                Field::Kind
            }
            RegionEdit::Name(name) => {
                regions[index].name = name;
                Field::Name
            }
            RegionEdit::Select(_) | RegionEdit::SelectCorner(_) | RegionEdit::Done => return,
        };
        if self.typing != Some((index, field)) {
            document.end_stroke();
            self.typing = Some((index, field));
        }
        document.drag_regions(regions);
    }

    /// The tool's summary, the selected region's fields and corners, and the list of every
    /// region.
    pub fn view<'a>(&'a self, document: &'a Document) -> Element<'a, RegionEdit> {
        let map = &document.map;
        let selected = self.selected(document);
        let summary = if selected.is_some() {
            "Click a corner to select it and drag to move it. Click anywhere else to add a \
             corner after the selected one. The region holds every hex on or inside its \
             outline. Delete removes the selected corner; Escape or Done lets go of the region."
        } else {
            "Named areas scripts can test units against; units never see them. Click a \
             region's hex to select it, or click anywhere else to start a new region there."
        };
        let mut panel = column![text("Regions").size(15), text(summary).size(12)].spacing(8);
        if let Some(index) = selected {
            panel = panel.push(self.region_form(document, index));
        }
        let entries = map.regions.iter().enumerate().map(|(index, region)| {
            button(
                text(format!(
                    "{} · {} · {} hexes",
                    display(&region.name),
                    display(&region.kind),
                    document.region_hexes(index).len()
                ))
                .size(12),
            )
            .width(Fill)
            .style(if selected == Some(index) {
                button::primary
            } else {
                button::text
            })
            .on_press(RegionEdit::Select(index))
            .into()
        });
        let count = map.regions.len();
        let list: Element<'_, RegionEdit> = if count == 0 {
            text("This map has no regions.").size(12).into()
        } else {
            scrollable(column(entries).spacing(2)).height(200).into()
        };
        panel
            .push(text(format!("All regions ({count})")).size(15))
            .push(list)
            .into()
    }

    /// The selected region's editable fields, its corners, any reason it cannot be saved, and
    /// its buttons.
    fn region_form<'a>(&'a self, document: &'a Document, index: usize) -> Element<'a, RegionEdit> {
        let map = &document.map;
        let region = &map.regions[index];
        let corner = self.corner(document);
        let field = |label, input: Element<'a, RegionEdit>| {
            row![text(label).size(13).width(FIELD_LABEL_WIDTH), input].align_y(Alignment::Center)
        };
        let mut form = column![
            text(format!(
                "Selected region · {} corners · {} hexes",
                region.corners.len(),
                document.region_hexes(index).len()
            ))
            .size(15),
            field(
                "Name",
                text_input("name", &region.name)
                    .id(NAME_INPUT)
                    .on_input(RegionEdit::Name)
                    .size(13)
                    .into()
            ),
            field(
                "Type",
                text_input(DEFAULT_KIND, &region.kind)
                    .on_input(RegionEdit::Kind)
                    .size(13)
                    .into()
            ),
            text("Corners, in outline order").size(13),
            corner_grid(&region.corners, corner),
        ]
        .spacing(6);
        if let Err(error) = region.validate(i64::from(map.width), i64::from(map.height)) {
            form = form.push(
                text(format!("Cannot save: {error}."))
                    .size(12)
                    .color(PROBLEM_COLOR),
            );
        }
        form.push(
            row![
                button(text("Done").size(13))
                    .style(button::secondary)
                    .on_press(RegionEdit::Done),
                space::horizontal(),
                button(text("Delete corner").size(13))
                    .style(button::danger)
                    .on_press_maybe(corner.map(|_| RegionEdit::DeleteCorner)),
                button(text("Delete region").size(13))
                    .style(button::danger)
                    .on_press(RegionEdit::DeleteRegion),
            ]
            .spacing(6),
        )
        .into()
    }
}

/// Width of the labels beside the selected region's fields.
const FIELD_LABEL_WIDTH: f32 = 70.0;

/// The color of a reason the selected region cannot be saved.
const PROBLEM_COLOR: Color = Color::from_rgb(1.0, 0.45, 0.4);

/// A button per corner, numbered from 1 in outline order, with the `selected` one highlighted.
fn corner_grid<'a>(corners: &[[u16; 2]], selected: Option<usize>) -> Element<'a, RegionEdit> {
    let mut rows = column![].spacing(4);
    for (line, chunk) in corners.chunks(CORNERS_PER_ROW).enumerate() {
        let mut cells = row![].spacing(4);
        for (offset, &[x, y]) in chunk.iter().enumerate() {
            let index = line * CORNERS_PER_ROW + offset;
            cells = cells.push(
                button(text(format!("{} · {x},{y}", index + 1)).size(12).center())
                    .width(Fill)
                    .padding([4, 2])
                    .style(if selected == Some(index) {
                        button::primary
                    } else {
                        button::secondary
                    })
                    .on_press(RegionEdit::SelectCorner(index)),
            );
        }
        for _ in chunk.len()..CORNERS_PER_ROW {
            cells = cells.push(space().width(Fill));
        }
        rows = rows.push(cells);
    }
    rows.into()
}

/// `Region N` for the smallest `N` no region is named yet.
fn unused_name(regions: &[MapRegion]) -> String {
    (1..)
        .map(|number| format!("Region {number}"))
        .find(|name| regions.iter().all(|region| region.name != *name))
        .expect("some number is unused")
}

/// A name or type for display, marking an empty one.
fn display(value: &str) -> &str {
    if value.is_empty() { "(blank)" } else { value }
}

/// Each region's name over the middle of its corners when hexes are large enough, and the
/// `selected` region's numbered corners with the `corner` one highlighted. A region holding a
/// point of interest of the same name, such as a generated settlement's, leaves the naming to
/// the point's own label. Nothing here takes the mouse, so the map under it still receives
/// every click and drag.
pub fn overlay<'a, Message: 'a>(
    document: &'a Document,
    camera: Camera,
    viewport: Size,
    selected: Option<usize>,
    corner: Option<usize>,
) -> Element<'a, Message> {
    let map = &document.map;
    let mut layers = stack![].width(Fill).height(Fill).clip(true);
    let visible = |point: iced::Point, margin: f32| {
        point.x > -margin
            && point.y > -margin
            && point.x < viewport.width + margin
            && point.y < viewport.height + margin
    };
    if camera.radius >= LABEL_RADIUS {
        for (index, region) in map.regions.iter().enumerate() {
            if named_by_point(document, index) {
                continue;
            }
            let corners: Vec<_> = region
                .corner_hexes()
                .map(|hex| camera.center(hex))
                .collect();
            let count = corners.len().max(1) as f32;
            let middle = iced::Point::new(
                corners.iter().map(|point| point.x).sum::<f32>() / count,
                corners.iter().map(|point| point.y).sum::<f32>() / count,
            );
            if !visible(middle, 240.0) {
                continue;
            }
            let label = container(text(display(&region.name)).size(13).color(Color::WHITE))
                .padding([1, 6])
                .style(|_: &Theme| container::Style {
                    background: Some(Background::Color(Color {
                        a: 0.6,
                        ..Color::BLACK
                    })),
                    border: Border::default().rounded(3),
                    ..container::Style::default()
                });
            // Roughly centered: the label's width is unknown here.
            let shift = 3.5 * display(&region.name).chars().count() as f32 + 6.0;
            layers = layers.push(pin(label).x(middle.x - shift).y(middle.y - 10.0));
        }
    }
    let Some(region) = selected.and_then(|index| map.regions.get(index)) else {
        return layers.into();
    };
    let size = (camera.radius * 0.7).clamp(MIN_CORNER, MAX_CORNER);
    for (index, hex) in region.corner_hexes().enumerate() {
        let center = camera.center(hex);
        if !visible(center, size) {
            continue;
        }
        let fill = if Some(index) == corner {
            SELECTED_CORNER_COLOR
        } else {
            CORNER_COLOR
        };
        let marker = container(
            text((index + 1).to_string())
                .size(11)
                .color(Color::BLACK)
                .center(),
        )
        .center(size)
        .style(move |_: &Theme| container::Style {
            background: Some(Background::Color(fill)),
            border: Border {
                color: Color::BLACK,
                width: 1.5,
                radius: 3.0.into(),
            },
            ..container::Style::default()
        });
        layers = layers.push(
            pin(marker)
                .x(center.x - size / 2.0)
                .y(center.y - size / 2.0),
        );
    }
    layers.into()
}

/// Whether region `index` holds a point of interest with the same name.
fn named_by_point(document: &Document, index: usize) -> bool {
    let name = &document.map.regions[index].name;
    let hexes = document.region_hexes(index);
    document.map.points_of_interest.iter().any(|point| {
        point.name == *name
            && hexes
                .binary_search_by_key(&(i32::from(point.y), i32::from(point.x)), |hex| {
                    (hex.y, hex.x)
                })
                .is_ok()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: HexCoordinate = HexCoordinate { x: 2, y: 2 };

    /// Press each hex in turn as separate clicks.
    fn click(panel: &mut RegionsPanel, document: &mut Document, hexes: &[(i32, i32)]) {
        for &(x, y) in hexes {
            panel.press(document, HexCoordinate { x, y });
            document.end_stroke();
        }
    }

    /// Pressing an empty hex starts a region there; further presses add corners in order, and
    /// each corner undoes on its own.
    #[test]
    fn presses_start_regions_and_add_corners() {
        let mut document = Document::new(10, 8).unwrap();
        let mut panel = RegionsPanel::default();
        assert!(panel.press(&mut document, AT));
        document.end_stroke();
        click(&mut panel, &mut document, &[(7, 2), (7, 6)]);
        let region = &document.map.regions[0];
        assert_eq!(
            (region.kind.as_str(), region.name.as_str()),
            (DEFAULT_KIND, "Region 1")
        );
        assert_eq!(region.corners, [[2, 2], [7, 2], [7, 6]]);
        assert!(document.region_hexes(0).len() > 3);
        assert_eq!(panel.corner(&document), Some(2));
        document.undo();
        assert_eq!(document.map.regions[0].corners, [[2, 2], [7, 2]]);
    }

    /// A new corner goes after the selected one, and pressing a corner selects it.
    #[test]
    fn corners_insert_after_the_selected_corner() {
        let mut document = Document::new(10, 8).unwrap();
        let mut panel = RegionsPanel::default();
        click(&mut panel, &mut document, &[(1, 1), (8, 1), (8, 6)]);
        click(&mut panel, &mut document, &[(1, 1)]);
        assert_eq!(panel.corner(&document), Some(0));
        click(&mut panel, &mut document, &[(4, 0)]);
        assert_eq!(
            document.map.regions[0].corners,
            [[1, 1], [4, 0], [8, 1], [8, 6]]
        );
        assert_eq!(panel.corner(&document), Some(1));
    }

    /// Dragging moves the selected corner, and the whole drag undoes in one step.
    #[test]
    fn dragging_moves_the_selected_corner() {
        let mut document = Document::new(10, 8).unwrap();
        let mut panel = RegionsPanel::default();
        click(&mut panel, &mut document, &[(1, 1), (8, 1), (8, 6)]);
        let placed = document.map.clone();
        panel.press(&mut document, HexCoordinate { x: 8, y: 1 });
        panel.drag(&mut document, HexCoordinate { x: 9, y: 1 });
        panel.drag(&mut document, HexCoordinate { x: 9, y: 0 });
        document.end_stroke();
        assert_eq!(document.map.regions[0].corners, [[1, 1], [9, 0], [8, 6]]);
        document.undo();
        assert_eq!(document.map, placed);
    }

    /// Once let go of, pressing inside a region selects it instead of starting another, and a
    /// new region copies the type of the last one selected.
    #[test]
    fn presses_inside_regions_select_them() {
        let mut document = Document::new(10, 8).unwrap();
        let mut panel = RegionsPanel::default();
        click(&mut panel, &mut document, &[(1, 1), (8, 1), (8, 6), (1, 6)]);
        panel.edit(&mut document, RegionEdit::Kind("spawn".into()));
        panel.edit(&mut document, RegionEdit::Done);
        assert_eq!(panel.selected(&document), None);
        assert!(!panel.press(&mut document, HexCoordinate { x: 4, y: 3 }));
        assert_eq!(panel.selected(&document), Some(0));
        panel.edit(&mut document, RegionEdit::Done);
        assert!(panel.press(&mut document, HexCoordinate { x: 9, y: 7 }));
        document.end_stroke();
        let second = &document.map.regions[1];
        assert_eq!(
            (second.kind.as_str(), second.name.as_str()),
            ("spawn", "Region 2")
        );
    }

    /// Delete removes the selected corner, then the region once its last corner goes; typing a
    /// name undoes in one step.
    #[test]
    fn deleting_corners_and_regions() {
        let mut document = Document::new(10, 8).unwrap();
        let mut panel = RegionsPanel::default();
        click(&mut panel, &mut document, &[(1, 1), (8, 1)]);
        for name in ["N", "North"] {
            panel.edit(&mut document, RegionEdit::Name(name.into()));
        }
        panel.edit(&mut document, RegionEdit::Delete);
        assert_eq!(document.map.regions[0].corners, [[1, 1]]);
        assert_eq!(panel.corner(&document), Some(0));
        panel.edit(&mut document, RegionEdit::Delete);
        assert!(document.map.regions.is_empty());
        assert_eq!(panel.selected(&document), None);
        document.undo();
        document.undo();
        assert_eq!(document.map.regions[0].name, "North");
        document.undo();
        assert_eq!(document.map.regions[0].name, "Region 1");
        panel.edit(&mut document, RegionEdit::Select(0));
        panel.edit(&mut document, RegionEdit::DeleteRegion);
        assert!(document.map.regions.is_empty());
    }

    /// A region is named by a point of interest of the same name only when the point lies
    /// inside it.
    #[test]
    fn points_inside_regions_can_name_them() {
        let mut document = Document::new(10, 8).unwrap();
        let mut panel = RegionsPanel::default();
        click(&mut panel, &mut document, &[(1, 1), (8, 1), (8, 6), (1, 6)]);
        let point = |name: &str, x, y| stompymux_map::MapPointOfInterest {
            kind: "settlement".into(),
            name: name.into(),
            x,
            y,
            elevation: None,
        };
        document.set_points(vec![point("Region 1", 9, 7), point("Elsewhere", 4, 3)]);
        assert!(!named_by_point(&document, 0));
        document.set_points(vec![point("Region 1", 4, 3)]);
        assert!(named_by_point(&document, 0));
    }

    /// Regions made with the tool save and load back unchanged.
    #[test]
    fn edited_regions_survive_saving() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("regions.toml");
        let mut document = Document::new(10, 8).unwrap();
        let mut panel = RegionsPanel::default();
        click(&mut panel, &mut document, &[(1, 1), (8, 1), (8, 6)]);
        panel.edit(&mut document, RegionEdit::Name("North \"LZ\"".into()));
        document.save_to(&path).unwrap();
        let reopened = Document::open(&path).unwrap();
        assert_eq!(reopened.map.regions, document.map.regions);
        assert_eq!(reopened.region_feed().hexes, document.region_feed().hexes);
    }
}
