//! The Points tool: placing and editing a map's points of interest, its inspector controls,
//! and the markers that show every point over the map.
//!
//! Points of interest are map metadata for scripts, which read them with
//! `btech.map.points_of_interest`; units never see them. With the tool selected, pressing on
//! an empty hex adds a point there and pressing on a point selects it, pressing again cycling
//! through the points that share the hex. Dragging moves the selected point. The inspector
//! edits the selected point's type, name and elevation, lists every point, and deletes the
//! selected one.
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Point, Size, Theme,
    widget::{button, column, container, pin, row, scrollable, space, stack, text, text_input},
};
use stompymux_map::{HexCoordinate, MapAsset, MapPointOfInterest};

use crate::{document::Document, map_view::Camera};

/// Widget id of the selected point's name input, which a new point focuses.
pub const NAME_INPUT: &str = "point-name";

/// The type a new point takes when no point is selected to copy it from.
const DEFAULT_KIND: &str = "objective";

/// Below this hex radius in pixels, markers drop their name labels.
const LABEL_RADIUS: f32 = 10.0;

/// Smallest and largest marker diameter in pixels.
const MIN_MARKER: f32 = 8.0;
const MAX_MARKER: f32 = 22.0;

/// Fill of a marker, and of the selected one.
const MARKER_COLOR: Color = Color::from_rgb(0.98, 0.72, 0.10);
const SELECTED_COLOR: Color = Color::from_rgb(0.20, 0.85, 1.0);

/// A text field of a point, which typing into edits as one undoable step until another field
/// or point is edited.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Field {
    Kind,
    Name,
    Elevation,
}

/// A change to the points from the inspector or the keyboard.
#[derive(Debug, Clone)]
pub enum PointEdit {
    /// Select the point with this index.
    Select(usize),
    /// The selected point's type was typed into.
    Kind(String),
    /// The selected point's name was typed into.
    Name(String),
    /// The selected point's elevation was typed into; blank clears it.
    Elevation(String),
    /// Remove the selected point.
    Delete,
}

/// The selected point and the edit in progress.
#[derive(Debug, Clone, Default)]
pub struct PointsPanel {
    /// Index of the selected point, which may be stale after an undo; see
    /// [`PointsPanel::selected`].
    selected: Option<usize>,
    /// The field being typed into, which a change of field or point closes into an edit.
    typing: Option<(usize, Field)>,
    /// Elevation text that is not yet a valid elevation, such as a lone `-`.
    elevation_draft: Option<String>,
}

impl PointsPanel {
    /// The selected point's index, if it still names a point in `document`.
    pub fn selected(&self, document: &Document) -> Option<usize> {
        self.selected
            .filter(|&index| index < document.map.points_of_interest.len())
    }

    /// Forget the selection, as when another map replaces this one.
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Forget half-typed text, which the document may no longer match after an undo or redo.
    pub fn forget_typing(&mut self) {
        self.typing = None;
        self.elevation_draft = None;
    }

    /// The left button went down on `at`: select the point there, the next one if a point
    /// there is already selected, or else add a point there as part of a drag that
    /// [`Document::end_stroke`] closes. Returns whether a point was added.
    pub fn press(&mut self, document: &mut Document, at: HexCoordinate) -> bool {
        document.end_stroke();
        self.forget_typing();
        let here: Vec<usize> = document.points_at(at).map(|(index, _)| index).collect();
        if !here.is_empty() {
            let current = self
                .selected(document)
                .and_then(|selected| here.iter().position(|&index| index == selected));
            self.selected = Some(match current {
                Some(position) => here[(position + 1) % here.len()],
                None => here[0],
            });
            return false;
        }
        let (Ok(x), Ok(y)) = (u16::try_from(at.x), u16::try_from(at.y)) else {
            return false;
        };
        let mut points = document.map.points_of_interest.clone();
        let kind = self
            .selected(document)
            .map_or(DEFAULT_KIND.into(), |index| points[index].kind.clone());
        let name = unused_name(&points);
        points.push(MapPointOfInterest {
            kind,
            name,
            x,
            y,
            elevation: None,
        });
        self.selected = Some(points.len() - 1);
        document.drag_points(points);
        true
    }

    /// The pointer was dragged onto `at` with the button held: move the selected point there.
    pub fn drag(&mut self, document: &mut Document, at: HexCoordinate) {
        let Some(index) = self.selected(document) else {
            return;
        };
        let (Ok(x), Ok(y)) = (u16::try_from(at.x), u16::try_from(at.y)) else {
            return;
        };
        let mut points = document.map.points_of_interest.clone();
        if (points[index].x, points[index].y) == (x, y) {
            return;
        }
        points[index].x = x;
        points[index].y = y;
        document.drag_points(points);
    }

    /// Apply a change from the inspector or the keyboard.
    pub fn edit(&mut self, document: &mut Document, edit: PointEdit) {
        if let PointEdit::Select(index) = edit {
            document.end_stroke();
            self.forget_typing();
            self.selected = Some(index);
            return;
        }
        let Some(index) = self.selected(document) else {
            return;
        };
        let mut points = document.map.points_of_interest.clone();
        let field = match edit {
            PointEdit::Select(_) => return,
            PointEdit::Delete => {
                points.remove(index);
                self.forget_typing();
                self.selected = None;
                document.set_points(points);
                return;
            }
            PointEdit::Kind(kind) => {
                points[index].kind = kind;
                Field::Kind
            }
            PointEdit::Name(name) => {
                points[index].name = name;
                Field::Name
            }
            PointEdit::Elevation(value) => {
                let Some(elevation) = parse_elevation(&value) else {
                    self.elevation_draft = Some(value);
                    return;
                };
                self.elevation_draft = None;
                points[index].elevation = elevation;
                Field::Elevation
            }
        };
        if self.typing != Some((index, field)) {
            document.end_stroke();
            self.typing = Some((index, field));
        }
        document.drag_points(points);
    }

    /// The tool's summary, the selected point's fields, and the list of every point.
    pub fn view<'a>(&'a self, document: &'a Document) -> Element<'a, PointEdit> {
        let map = &document.map;
        let selected = self.selected(document);
        let mut panel = column![
            text("Points of interest").size(15),
            text(
                "Markers scripts read with btech.map.points_of_interest; units never see \
                 them. Click an empty hex to add a point, click a point to select it (again \
                 to cycle through a hex's points), and drag to move it. Delete removes the \
                 selected point."
            )
            .size(12),
        ]
        .spacing(8);
        if let Some(index) = selected {
            panel = panel.push(self.point_form(map, index));
        }
        let entries = map
            .points_of_interest
            .iter()
            .enumerate()
            .map(|(index, point)| {
                button(
                    text(format!(
                        "{} · {} at {},{}",
                        display(&point.name),
                        display(&point.kind),
                        point.x,
                        point.y
                    ))
                    .size(12),
                )
                .width(Fill)
                .style(if selected == Some(index) {
                    button::primary
                } else {
                    button::text
                })
                .on_press(PointEdit::Select(index))
                .into()
            });
        let count = map.points_of_interest.len();
        let list: Element<'_, PointEdit> = if count == 0 {
            text("This map has no points of interest.").size(12).into()
        } else {
            scrollable(column(entries).spacing(2)).height(240).into()
        };
        panel
            .push(text(format!("All points ({count})")).size(15))
            .push(list)
            .into()
    }

    /// The selected point's editable fields, where it stands, any reason it cannot be saved,
    /// and its Delete button.
    fn point_form<'a>(&'a self, map: &'a MapAsset, index: usize) -> Element<'a, PointEdit> {
        let point = &map.points_of_interest[index];
        let elevation = self.elevation_draft.clone().unwrap_or_else(|| {
            point
                .elevation
                .map_or_else(String::new, |elevation| elevation.to_string())
        });
        let field = |label, input: Element<'a, PointEdit>| {
            row![text(label).size(13).width(FIELD_LABEL_WIDTH), input].align_y(Alignment::Center)
        };
        let mut form = column![
            text(format!("Selected point at {},{}", point.x, point.y)).size(15),
            field(
                "Type",
                text_input("objective", &point.kind)
                    .on_input(PointEdit::Kind)
                    .size(13)
                    .into()
            ),
            field(
                "Name",
                text_input("name", &point.name)
                    .id(NAME_INPUT)
                    .on_input(PointEdit::Name)
                    .size(13)
                    .into()
            ),
            field(
                "Elevation",
                text_input("ground", &elevation)
                    .on_input(PointEdit::Elevation)
                    .size(13)
                    .into()
            ),
            text("Elevation is in levels above the hex's ground, negative for below; leave it blank when the point has no particular height.").size(12),
        ]
        .spacing(6);
        let problem = match (
            &self.elevation_draft,
            point.validate(i64::from(map.width), i64::from(map.height)),
        ) {
            (Some(_), _) => Some(format!(
                "Elevation must be a whole number from {} to {}.",
                i8::MIN,
                i8::MAX
            )),
            (None, Err(error)) => Some(format!("Cannot save: {error}.")),
            (None, Ok(())) => None,
        };
        if let Some(problem) = problem {
            form = form.push(text(problem).size(12).color(PROBLEM_COLOR));
        }
        form.push(row![
            space::horizontal(),
            button(text("Delete point").size(13))
                .style(button::danger)
                .on_press(PointEdit::Delete),
        ])
        .into()
    }
}

/// Width of the labels beside the selected point's fields.
const FIELD_LABEL_WIDTH: f32 = 70.0;

/// The color of a reason the selected point cannot be saved.
const PROBLEM_COLOR: Color = Color::from_rgb(1.0, 0.45, 0.4);

/// An elevation typed into the inspector: `Some(None)` for blank, `Some(Some(level))` for a
/// whole number in range, and `None` for anything else.
fn parse_elevation(value: &str) -> Option<Option<i8>> {
    let value = value.trim();
    if value.is_empty() {
        return Some(None);
    }
    value.parse().ok().map(Some)
}

/// `Point N` for the smallest `N` no point is named yet.
fn unused_name(points: &[MapPointOfInterest]) -> String {
    (1..)
        .map(|number| format!("Point {number}"))
        .find(|name| points.iter().all(|point| point.name != *name))
        .expect("some number is unused")
}

/// A name or type for display, marking an empty one.
fn display(value: &str) -> &str {
    if value.is_empty() { "(blank)" } else { value }
}

/// A marker for each point of interest within `viewport`, named when hexes are large enough,
/// with the `selected` point's marker highlighted. Markers never take the mouse, so the map
/// under them still receives every click and drag.
pub fn markers<'a, Message: 'a>(
    map: &'a MapAsset,
    camera: Camera,
    viewport: Size,
    selected: Option<usize>,
) -> Element<'a, Message> {
    let diameter = (camera.radius * 0.8).clamp(MIN_MARKER, MAX_MARKER);
    let labelled = camera.radius >= LABEL_RADIUS;
    let mut layers = stack![].width(Fill).height(Fill).clip(true);
    // The selected marker goes last so it draws over any that share its hex.
    let mut order: Vec<usize> = (0..map.points_of_interest.len())
        .filter(|&index| Some(index) != selected)
        .collect();
    order.extend(selected);
    for index in order {
        let point = &map.points_of_interest[index];
        let center = camera.center(HexCoordinate {
            x: i32::from(point.x),
            y: i32::from(point.y),
        });
        if !near_view(center, viewport, diameter) {
            continue;
        }
        let fill = if Some(index) == selected {
            SELECTED_COLOR
        } else {
            MARKER_COLOR
        };
        let dot = container(space())
            .width(diameter)
            .height(diameter)
            .style(move |_: &Theme| container::Style {
                background: Some(Background::Color(fill)),
                border: Border {
                    color: Color::BLACK,
                    width: 2.0,
                    radius: (diameter / 2.0).into(),
                },
                ..container::Style::default()
            });
        layers = layers.push(
            pin(dot)
                .x(center.x - diameter / 2.0)
                .y(center.y - diameter / 2.0),
        );
        if !labelled {
            continue;
        }
        let label = container(text(display(&point.name)).size(12).color(Color::WHITE))
            .padding([1, 4])
            .style(|_: &Theme| container::Style {
                background: Some(Background::Color(Color {
                    a: 0.75,
                    ..Color::BLACK
                })),
                border: Border::default().rounded(3),
                ..container::Style::default()
            });
        layers = layers.push(
            pin(label)
                .x(center.x + diameter / 2.0 + 3.0)
                .y(center.y - 9.0),
        );
    }
    layers.into()
}

/// Whether a marker centered at `center` could show in a view of `size`, allowing for its
/// label running off to the right.
fn near_view(center: Point, size: Size, diameter: f32) -> bool {
    const LABEL_ROOM: f32 = 240.0;
    center.x > -diameter - LABEL_ROOM
        && center.y > -diameter
        && center.x < size.width + diameter
        && center.y < size.height + diameter
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: HexCoordinate = HexCoordinate { x: 2, y: 1 };
    const ELSEWHERE: HexCoordinate = HexCoordinate { x: 4, y: 3 };

    /// Pressing an empty hex adds a point there, named and typed, that undoes in one step.
    #[test]
    fn pressing_an_empty_hex_adds_a_point() {
        let mut document = Document::new(6, 5).unwrap();
        let mut panel = PointsPanel::default();
        assert!(panel.press(&mut document, AT));
        document.end_stroke();
        assert_eq!(
            document.map.points_of_interest,
            [MapPointOfInterest {
                kind: DEFAULT_KIND.into(),
                name: "Point 1".into(),
                x: 2,
                y: 1,
                elevation: None,
            }]
        );
        assert_eq!(panel.selected(&document), Some(0));
        document.undo();
        assert!(document.map.points_of_interest.is_empty());
        assert_eq!(panel.selected(&document), None);
    }

    /// A new point copies the selected point's type and takes the first unused name.
    #[test]
    fn new_points_copy_the_selected_type() {
        let mut document = Document::new(6, 5).unwrap();
        let mut panel = PointsPanel::default();
        panel.press(&mut document, AT);
        panel.edit(&mut document, PointEdit::Kind("spawn".into()));
        panel.press(&mut document, ELSEWHERE);
        document.end_stroke();
        let second = &document.map.points_of_interest[1];
        assert_eq!(
            (second.kind.as_str(), second.name.as_str()),
            ("spawn", "Point 2")
        );
    }

    /// Pressing a hex with several points cycles through them instead of adding another.
    #[test]
    fn pressing_a_point_selects_and_cycles() {
        let mut document = Document::new(6, 5).unwrap();
        let point = |name: &str| MapPointOfInterest {
            kind: "objective".into(),
            name: name.into(),
            x: 2,
            y: 1,
            elevation: None,
        };
        document.set_points(vec![point("A"), point("B")]);
        let mut panel = PointsPanel::default();
        assert!(!panel.press(&mut document, AT));
        assert_eq!(panel.selected(&document), Some(0));
        assert!(!panel.press(&mut document, AT));
        assert_eq!(panel.selected(&document), Some(1));
        assert!(!panel.press(&mut document, AT));
        assert_eq!(panel.selected(&document), Some(0));
        assert_eq!(document.map.points_of_interest.len(), 2);
    }

    /// Dragging moves the selected point, and the whole drag undoes in one step.
    #[test]
    fn dragging_moves_the_selected_point() {
        let mut document = Document::new(6, 5).unwrap();
        let mut panel = PointsPanel::default();
        panel.press(&mut document, AT);
        document.end_stroke();
        let placed = document.map.clone();
        panel.press(&mut document, AT);
        panel.drag(&mut document, HexCoordinate { x: 3, y: 1 });
        panel.drag(&mut document, ELSEWHERE);
        document.end_stroke();
        let point = &document.map.points_of_interest[0];
        assert_eq!((point.x, point.y), (4, 3));
        document.undo();
        assert_eq!(document.map, placed);
    }

    /// Typing into one field undoes in one step; switching fields starts another.
    #[test]
    fn typing_undoes_one_field_at_a_time() {
        let mut document = Document::new(6, 5).unwrap();
        let mut panel = PointsPanel::default();
        panel.press(&mut document, AT);
        document.end_stroke();
        for name in ["T", "To", "Tower"] {
            panel.edit(&mut document, PointEdit::Name(name.into()));
        }
        for elevation in ["-", "-2"] {
            panel.edit(&mut document, PointEdit::Elevation(elevation.into()));
        }
        let point = &document.map.points_of_interest[0];
        assert_eq!((point.name.as_str(), point.elevation), ("Tower", Some(-2)));
        document.undo();
        let point = &document.map.points_of_interest[0];
        assert_eq!((point.name.as_str(), point.elevation), ("Tower", None));
        document.undo();
        assert_eq!(document.map.points_of_interest[0].name, "Point 1");
    }

    /// Elevation text that is not a level is held as a draft and leaves the point alone, and
    /// blank clears the elevation.
    #[test]
    fn elevation_accepts_levels_and_blank() {
        assert_eq!(parse_elevation(""), Some(None));
        assert_eq!(parse_elevation(" 5 "), Some(Some(5)));
        assert_eq!(parse_elevation("-128"), Some(Some(-128)));
        assert_eq!(parse_elevation("-"), None);
        assert_eq!(parse_elevation("200"), None);
        let mut document = Document::new(6, 5).unwrap();
        let mut panel = PointsPanel::default();
        panel.press(&mut document, AT);
        panel.edit(&mut document, PointEdit::Elevation("3".into()));
        panel.edit(&mut document, PointEdit::Elevation("3x".into()));
        assert_eq!(document.map.points_of_interest[0].elevation, Some(3));
        assert_eq!(panel.elevation_draft.as_deref(), Some("3x"));
        panel.edit(&mut document, PointEdit::Elevation(String::new()));
        assert_eq!(document.map.points_of_interest[0].elevation, None);
        assert_eq!(panel.elevation_draft, None);
    }

    /// Deleting removes the selected point as one undoable edit and clears the selection.
    #[test]
    fn deleting_removes_the_selected_point() {
        let mut document = Document::new(6, 5).unwrap();
        let mut panel = PointsPanel::default();
        panel.press(&mut document, AT);
        panel.press(&mut document, ELSEWHERE);
        document.end_stroke();
        panel.edit(&mut document, PointEdit::Select(0));
        panel.edit(&mut document, PointEdit::Delete);
        assert_eq!(panel.selected(&document), None);
        assert_eq!(document.map.points_of_interest.len(), 1);
        assert_eq!(document.map.points_of_interest[0].name, "Point 2");
        document.undo();
        assert_eq!(document.map.points_of_interest.len(), 2);
    }

    /// Points made with the tool save and load back unchanged.
    #[test]
    fn edited_points_survive_saving() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("points.toml");
        let mut document = Document::new(6, 5).unwrap();
        let mut panel = PointsPanel::default();
        panel.press(&mut document, AT);
        panel.edit(&mut document, PointEdit::Name("Comms \"Tower\"".into()));
        panel.edit(&mut document, PointEdit::Elevation("-1".into()));
        document.save_to(&path).unwrap();
        let reopened = Document::open(&path).unwrap();
        assert_eq!(
            reopened.map.points_of_interest,
            document.map.points_of_interest
        );
    }
}
