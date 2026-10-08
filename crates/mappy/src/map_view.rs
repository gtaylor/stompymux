//! The hex map view: camera geometry, the colors of every hex layer, and pan, zoom and paint
//! input. A left press reports [`Message::Press`] and dragging onto further hexes reports
//! [`Message::Paint`], so the selected tool decides what each does.
//!
//! Hexes are flat-topped in staggered columns, with even columns offset half a hex south, the
//! same layout as [`HexCoordinate::center`]. Map-space pixels put the top-left of the
//! map's bounding box at the origin; the [`Camera`] places that origin on screen. Drawing
//! happens on the GPU in [`crate::render`].
use iced::{
    Color, Event, Point, Rectangle, Size, Vector,
    keyboard::{self, Modifiers},
    mouse,
    widget::shader::{self, Action},
};
use stompymux_map::{
    Condition, ConstructionClass, DecorationKind, Foliage, Ground, HexCoordinate, MapAsset, Route,
    StructureKind,
};

use crate::{
    Message,
    document::Document,
    render::{MapPrimitive, Uniforms},
};

/// √3, the height of a flat-topped hex with a vertex radius of one.
const SQRT_3: f32 = 1.732_050_8;

/// Smallest and largest hex radius in screen pixels.
pub const MIN_RADIUS: f32 = 0.5;
pub const MAX_RADIUS: f32 = 64.0;

/// Below this radius no grid is drawn; below the next, hexes are too small for their three rows
/// of labels to be legible, so none are drawn.
const GRID_RADIUS: f32 = 7.0;
const LABEL_RADIUS: f32 = 20.0;

/// Width in pixels of the background gap left between neighboring hexes as grid lines.
const GRID_GAP: f32 = 1.0;

/// Where the map sits on screen and how large its hexes are.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    /// Screen position, relative to the map view, of the map-space origin.
    pub offset: Vector,
    /// Hex vertex radius in screen pixels.
    pub radius: f32,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            offset: Vector::new(16.0, 16.0),
            radius: 14.0,
        }
    }
}

impl Camera {
    /// A camera showing the whole of a `width` by `height` map centered in `viewport`.
    pub fn fit(width: u16, height: u16, viewport: Size) -> Self {
        let margin = 16.0;
        let (columns, rows) = (f32::from(width), f32::from(height));
        let span_x = 1.5 * columns + 0.5;
        let span_y = SQRT_3 * (rows + 0.5);
        let radius = ((viewport.width - 2.0 * margin) / span_x)
            .min((viewport.height - 2.0 * margin) / span_y)
            .clamp(MIN_RADIUS, MAX_RADIUS);
        Self {
            offset: Vector::new(
                (viewport.width - span_x * radius) / 2.0,
                (viewport.height - span_y * radius) / 2.0,
            ),
            radius,
        }
    }

    /// The camera a newly opened map starts with: the whole map when its hexes are large enough
    /// to carry labels, otherwise the middle of the map at the smallest zoom that labels them.
    pub fn opening(width: u16, height: u16, viewport: Size) -> Self {
        let fit = Self::fit(width, height, viewport);
        if fit.radius >= LABEL_RADIUS {
            return fit;
        }
        let center = Vector::new(viewport.width / 2.0, viewport.height / 2.0);
        Self {
            offset: center + (fit.offset - center) * (LABEL_RADIUS / fit.radius),
            radius: LABEL_RADIUS,
        }
    }

    /// The camera after scaling by `factor` while keeping the map point under `anchor` still.
    pub fn zoomed(self, factor: f32, anchor: Point) -> Self {
        let radius = (self.radius * factor).clamp(MIN_RADIUS, MAX_RADIUS);
        let scale = radius / self.radius;
        let anchor = Vector::new(anchor.x, anchor.y);
        Self {
            offset: anchor + (self.offset - anchor) * scale,
            radius,
        }
    }

    /// Screen center of a hex.
    pub fn center(self, coordinate: HexCoordinate) -> Point {
        let height = SQRT_3 * self.radius;
        let stagger = if coordinate.x.rem_euclid(2) == 0 {
            1.0
        } else {
            0.5
        };
        Point::new(
            self.offset.x + self.radius * (1.0 + 1.5 * coordinate.x as f32),
            self.offset.y + height * (coordinate.y as f32 + stagger),
        )
    }

    /// The hex containing a screen point, whether or not it is on the map. The shader's
    /// `hex_at` makes the same choice for every pixel it draws.
    fn hex_at(self, point: Point) -> HexCoordinate {
        let height = SQRT_3 * self.radius;
        let column = ((point.x - self.offset.x - self.radius) / (1.5 * self.radius)).round() as i32;
        let mut best = (f32::INFINITY, HexCoordinate { x: 0, y: 0 });
        // The containing hex is the one with the nearest center, which is always within one
        // column and one row of the rough estimate.
        for x in column - 1..=column + 1 {
            let stagger = if x.rem_euclid(2) == 0 { 1.0 } else { 0.5 };
            let row = ((point.y - self.offset.y) / height - stagger).round() as i32;
            for y in row - 1..=row + 1 {
                let coordinate = HexCoordinate { x, y };
                let distance = point.distance(self.center(coordinate));
                if distance < best.0 {
                    best = (distance, coordinate);
                }
            }
        }
        best.1
    }
}

/// A color from its red, green and blue channels.
const fn rgb((r, g, b): (f32, f32, f32)) -> Color {
    Color { r, g, b, a: 1.0 }
}

/// The color of bare ground, before elevation shading.
pub fn ground_color(ground: Ground) -> Color {
    rgb(match ground {
        Ground::Clear => (0.74, 0.71, 0.64),
        Ground::Pavement => (0.66, 0.66, 0.68),
        Ground::Rough => (0.62, 0.53, 0.37),
        Ground::UltraRough => (0.46, 0.37, 0.25),
        Ground::Rubble => (0.60, 0.50, 0.46),
        Ground::UltraRubble => (0.43, 0.34, 0.32),
        Ground::Sand => (0.88, 0.80, 0.55),
        Ground::Tundra => (0.62, 0.68, 0.58),
        Ground::Swamp => (0.36, 0.44, 0.28),
        Ground::MagmaCrust => (0.30, 0.20, 0.19),
        Ground::Magma => (0.82, 0.14, 0.04),
        Ground::HeavyIndustrial => (0.42, 0.37, 0.50),
    })
}

/// The color of foliage: woods in greens, jungle in teal greens, planted fields in yellow,
/// denser stands darker.
pub fn foliage_color(foliage: Foliage) -> Color {
    rgb(match foliage {
        Foliage::LightWoods => (0.30, 0.52, 0.20),
        Foliage::HeavyWoods => (0.16, 0.38, 0.13),
        Foliage::UltraHeavyWoods => (0.07, 0.24, 0.07),
        Foliage::LightJungle => (0.18, 0.58, 0.46),
        Foliage::HeavyJungle => (0.08, 0.43, 0.36),
        Foliage::UltraHeavyJungle => (0.02, 0.29, 0.25),
        Foliage::PlantedFields => (0.83, 0.76, 0.30),
    })
}

/// The color of a road's surface or of rail track.
pub fn route_color(route: Route) -> Color {
    rgb(match route {
        Route::PavedRoad => (0.27, 0.27, 0.30),
        Route::GravelRoad => (0.55, 0.54, 0.51),
        Route::DirtRoad => (0.55, 0.41, 0.27),
        Route::Rail => (0.13, 0.10, 0.10),
    })
}

/// The color of shallow still water, which the map darkens with depth.
pub fn water_color() -> Color {
    rgb((0.22, 0.46, 0.80))
}

/// The tint of ice, snow or mud.
pub fn condition_color(condition: Condition) -> Color {
    rgb(match condition {
        Condition::Ice => (0.70, 0.92, 0.98),
        Condition::ThinSnow => (0.90, 0.93, 0.97),
        Condition::DeepSnow => (1.0, 1.0, 1.0),
        Condition::Mud => (0.40, 0.28, 0.16),
    })
}

/// The tint of fire or smoke.
pub fn overlay_color(overlay: DecorationKind) -> Color {
    rgb(match overlay {
        DecorationKind::Fire => (0.96, 0.52, 0.10),
        DecorationKind::Smoke => (0.6, 0.59, 0.56),
    })
}

/// How much each construction class darkens a structure's color, per class above light.
pub const CLASS_SHADE: f32 = 0.12;

/// The color of a structure: a building filling its hex, or a wall or bridge deck running toward
/// its neighbors, darker for stronger construction classes.
pub fn structure_color(kind: StructureKind, class: ConstructionClass) -> Color {
    let base = structure_base_color(kind);
    let step = ConstructionClass::ALL
        .iter()
        .position(|candidate| *candidate == class)
        .unwrap_or(0) as f32;
    let scale = 1.0 - CLASS_SHADE * step;
    Color::from_rgb(base.r * scale, base.g * scale, base.b * scale)
}

/// The color of a light structure of `kind`.
pub fn structure_base_color(kind: StructureKind) -> Color {
    rgb(match kind {
        StructureKind::Building => (0.58, 0.58, 0.66),
        StructureKind::Wall => (0.52, 0.5, 0.56),
        StructureKind::Bridge => (0.72, 0.72, 0.75),
    })
}

/// Black or white, whichever reads better on `background`.
pub fn contrast(background: Color) -> Color {
    let luminance = 0.299 * background.r + 0.587 * background.g + 0.114 * background.b;
    if luminance > 0.55 {
        Color::BLACK
    } else {
        Color::WHITE
    }
}

/// Map-view interaction state kept by the widget between events.
#[derive(Debug, Default)]
pub struct Interaction {
    drag: Drag,
    modifiers: Modifiers,
    /// Last view size reported to the app, so it can fit maps to the view.
    viewport: Option<Size>,
}

/// What a held mouse button is doing.
#[derive(Debug, Default)]
enum Drag {
    #[default]
    None,
    Painting {
        last: HexCoordinate,
    },
    Panning {
        last: Point,
    },
}

/// Whether a coordinate is on the map.
fn contains(map: &MapAsset, coordinate: HexCoordinate) -> bool {
    (0..i32::from(map.width)).contains(&coordinate.x)
        && (0..i32::from(map.height)).contains(&coordinate.y)
}

/// The map view: turns input into messages and hands the GPU a [`MapPrimitive`] to draw.
pub struct MapView<'a> {
    pub document: &'a Document,
    pub camera: Camera,
    pub hover: Option<HexCoordinate>,
    pub brush_radius: u8,
    /// Index of the region to mark as selected.
    pub selected_region: Option<usize>,
}

impl MapView<'_> {
    /// The on-map hex under a screen position relative to the view.
    fn on_map(&self, position: Point) -> Option<HexCoordinate> {
        Some(self.camera.hex_at(position))
            .filter(|coordinate| contains(&self.document.map, *coordinate))
    }
}

impl shader::Program<Message> for MapView<'_> {
    type State = Interaction;
    type Primitive = MapPrimitive;

    fn update(
        &self,
        state: &mut Interaction,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<Action<Message>> {
        if state.viewport != Some(bounds.size()) {
            state.viewport = Some(bounds.size());
            return Some(Action::publish(Message::Viewport(bounds.size())));
        }
        if let Event::Keyboard(keyboard::Event::ModifiersChanged(modifiers)) = event {
            state.modifiers = *modifiers;
            return None;
        }
        let Event::Mouse(event) = event else {
            return None;
        };
        let position = cursor.position_in(bounds);
        match (event, position) {
            (mouse::Event::ButtonPressed(mouse::Button::Left), Some(position)) => {
                let coordinate = self.on_map(position)?;
                if state.modifiers.alt() {
                    return Some(Action::publish(Message::Pick(coordinate)).and_capture());
                }
                state.drag = Drag::Painting { last: coordinate };
                Some(Action::publish(Message::Press(coordinate)).and_capture())
            }
            (
                mouse::Event::ButtonPressed(mouse::Button::Right | mouse::Button::Middle),
                Some(position),
            ) => {
                state.drag = Drag::Panning { last: position };
                Some(Action::capture())
            }
            (mouse::Event::ButtonReleased(_), _) => match std::mem::take(&mut state.drag) {
                Drag::Painting { .. } => Some(Action::publish(Message::StrokeEnded)),
                Drag::Panning { .. } | Drag::None => None,
            },
            (mouse::Event::CursorMoved { .. }, Some(position)) => {
                let hover = self.on_map(position);
                match &mut state.drag {
                    Drag::Panning { last } => {
                        let delta = position - *last;
                        *last = position;
                        Some(Action::publish(Message::Panned(delta)))
                    }
                    Drag::Painting { last } => {
                        let coordinate = hover?;
                        if coordinate == *last {
                            return None;
                        }
                        *last = coordinate;
                        Some(Action::publish(Message::Paint(coordinate)))
                    }
                    Drag::None if hover != self.hover => {
                        Some(Action::publish(Message::Hovered(hover)))
                    }
                    Drag::None => None,
                }
            }
            (mouse::Event::CursorLeft, _) | (mouse::Event::CursorMoved { .. }, None) => {
                self.hover.map(|_| Action::publish(Message::Hovered(None)))
            }
            (mouse::Event::WheelScrolled { delta }, Some(position)) => {
                let message = wheel_message(*delta, state.modifiers, position);
                Some(Action::publish(message).and_capture())
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Interaction,
        _cursor: mouse::Cursor,
        bounds: Rectangle,
    ) -> MapPrimitive {
        let map = &self.document.map;
        let camera = self.camera;
        let hover = self
            .hover
            .map_or([0.0, 0.0], |hover| [hover.x as f32, hover.y as f32]);
        MapPrimitive {
            feed: self.document.hex_feed(),
            width: map.width,
            height: map.height,
            regions: self.document.region_feed(),
            selected_region: self.selected_region,
            uniforms: Uniforms {
                size: [bounds.width, bounds.height],
                offset: [camera.offset.x, camera.offset.y],
                map_size: [f32::from(map.width), f32::from(map.height)],
                hover,
                radius: camera.radius,
                grid_gap: if camera.radius >= GRID_RADIUS {
                    GRID_GAP
                } else {
                    0.0
                },
                brush: self.hover.map_or(-1.0, |_| f32::from(self.brush_radius)),
                labels: if camera.radius >= LABEL_RADIUS {
                    1.0
                } else {
                    0.0
                },
            },
        }
    }

    fn mouse_interaction(
        &self,
        state: &Interaction,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> mouse::Interaction {
        match state.drag {
            Drag::Panning { .. } => mouse::Interaction::Grabbing,
            Drag::Painting { .. } => mouse::Interaction::Crosshair,
            Drag::None if cursor.is_over(bounds) => mouse::Interaction::Crosshair,
            Drag::None => mouse::Interaction::default(),
        }
    }
}

/// What a wheel or touchpad scroll does over the map: it pans in both directions, Shift turns a
/// plain wheel's vertical scroll sideways, and Ctrl zooms around the cursor at `position`.
fn wheel_message(delta: mouse::ScrollDelta, modifiers: Modifiers, position: Point) -> Message {
    // Line deltas from mouse wheels count as this many pixels.
    const LINE: f32 = 40.0;
    let scroll = match delta {
        mouse::ScrollDelta::Lines { x, y } => Vector::new(x * LINE, y * LINE),
        mouse::ScrollDelta::Pixels { x, y } => Vector::new(x, y),
    };
    if modifiers.control() {
        return Message::Zoomed {
            factor: 1.15_f32.powf(scroll.y / LINE),
            anchor: position,
        };
    }
    // Some systems already turn Shift+wheel sideways; only swap a purely vertical scroll.
    if modifiers.shift() && scroll.x == 0.0 {
        return Message::Panned(Vector::new(scroll.y, 0.0));
    }
    Message::Panned(scroll)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every hex center maps back to its own coordinate at several zoom levels.
    #[test]
    fn hex_centers_round_trip_through_hit_testing() {
        for radius in [MIN_RADIUS, 9.5, MAX_RADIUS] {
            let camera = Camera {
                offset: Vector::new(-37.0, 12.5),
                radius,
            };
            for x in -2..12 {
                for y in -2..12 {
                    let coordinate = HexCoordinate { x, y };
                    assert_eq!(camera.hex_at(camera.center(coordinate)), coordinate);
                }
            }
        }
    }

    /// Screen layout matches the game's own geometry, up to scale and offset.
    #[test]
    fn layout_matches_game_geometry() {
        let camera = Camera {
            offset: Vector::new(0.0, 0.0),
            radius: 1.0,
        };
        let height = SQRT_3;
        for (x, y) in [(0, 0), (1, 0), (4, 7), (5, 3)] {
            let coordinate = HexCoordinate { x, y };
            let game = coordinate.center();
            let screen = camera.center(coordinate);
            // Game units are hex heights; the stagger and column spacing must agree.
            assert!(
                (screen.y / height - 0.5 - game.y as f32).abs() < 1e-4,
                "{x},{y}"
            );
            assert!((screen.x / height - game.x as f32).abs() < 1e-4, "{x},{y}");
        }
    }

    /// Large maps open zoomed in far enough for labels, centered as the fitted view was; small
    /// maps that already show labels open fitted.
    #[test]
    fn maps_open_close_enough_for_labels() {
        let viewport = Size::new(900.0, 700.0);
        let small = Camera::opening(8, 6, viewport);
        assert_eq!(small, Camera::fit(8, 6, viewport));
        assert!(small.radius >= LABEL_RADIUS);
        let fit = Camera::fit(120, 90, viewport);
        let large = Camera::opening(120, 90, viewport);
        assert!(fit.radius < LABEL_RADIUS);
        assert_eq!(large.radius, LABEL_RADIUS);
        // The same map point, in hex radii from the map origin, sits at the view's middle.
        let middle = Vector::new(viewport.width / 2.0, viewport.height / 2.0);
        let under_middle = |camera: Camera| (middle - camera.offset) * (1.0 / camera.radius);
        let shift = under_middle(large) - under_middle(fit);
        assert!(shift.x.abs() < 1e-3 && shift.y.abs() < 1e-3, "{shift:?}");
    }

    /// Shift turns a vertical wheel scroll into a sideways pan, and leaves sideways scrolls be.
    #[test]
    fn shift_scrolls_sideways() {
        let at = Point::new(10.0, 10.0);
        let pan = |delta, modifiers| match wheel_message(delta, modifiers, at) {
            Message::Panned(pan) => pan,
            message => panic!("{message:?}"),
        };
        let wheel = mouse::ScrollDelta::Lines { x: 0.0, y: -1.0 };
        assert_eq!(pan(wheel, Modifiers::empty()), Vector::new(0.0, -40.0));
        assert_eq!(pan(wheel, Modifiers::SHIFT), Vector::new(-40.0, 0.0));
        let sideways = mouse::ScrollDelta::Pixels { x: 12.0, y: 0.0 };
        assert_eq!(pan(sideways, Modifiers::SHIFT), Vector::new(12.0, 0.0));
    }

    /// Zooming keeps the map point under the cursor fixed.
    #[test]
    fn zoom_keeps_the_anchor_fixed() {
        let camera = Camera::default();
        let anchor = Point::new(300.0, 200.0);
        let before = camera.hex_at(anchor);
        let zoomed = camera.zoomed(2.0, anchor);
        assert_eq!(zoomed.hex_at(anchor), before);
    }
}
