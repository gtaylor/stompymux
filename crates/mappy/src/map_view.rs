//! The hex map view: camera geometry, terrain colors, and pan, zoom and paint input.
//!
//! Hexes are flat-topped in staggered columns, with even columns offset half a hex south, the
//! same layout as [`BattleHexCoordinate::center`]. Map-space pixels put the top-left of the
//! map's bounding box at the origin; the [`Camera`] places that origin on screen. Drawing
//! happens on the GPU in [`crate::render`].
use iced::{
    Color, Event, Point, Rectangle, Size, Vector,
    keyboard::{self, Modifiers},
    mouse,
    widget::shader::{self, Action},
};
use stompymux_map::{BattleHexCoordinate, BattleMapAsset, Terrain};

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
    fn center(self, coordinate: BattleHexCoordinate) -> Point {
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
    fn hex_at(self, point: Point) -> BattleHexCoordinate {
        let height = SQRT_3 * self.radius;
        let column = ((point.x - self.offset.x - self.radius) / (1.5 * self.radius)).round() as i32;
        let mut best = (f32::INFINITY, BattleHexCoordinate { x: 0, y: 0 });
        // The containing hex is the one with the nearest center, which is always within one
        // column and one row of the rough estimate.
        for x in column - 1..=column + 1 {
            let stagger = if x.rem_euclid(2) == 0 { 1.0 } else { 0.5 };
            let row = ((point.y - self.offset.y) / height - stagger).round() as i32;
            for y in row - 1..=row + 1 {
                let coordinate = BattleHexCoordinate { x, y };
                let distance = point.distance(self.center(coordinate));
                if distance < best.0 {
                    best = (distance, coordinate);
                }
            }
        }
        best.1
    }
}

/// The base color for a terrain, before elevation shading.
pub fn terrain_color(terrain: Terrain) -> Color {
    let (red, green, blue) = match terrain {
        Terrain::Grassland => (0.47, 0.64, 0.31),
        Terrain::Road => (0.64, 0.60, 0.52),
        Terrain::LightForest => (0.30, 0.52, 0.22),
        Terrain::HeavyForest => (0.12, 0.35, 0.14),
        Terrain::Water => (0.22, 0.46, 0.80),
        Terrain::Ice => (0.74, 0.88, 0.96),
        Terrain::Bridge => (0.56, 0.38, 0.22),
        Terrain::Rough => (0.58, 0.51, 0.37),
        Terrain::Mountains => (0.47, 0.41, 0.39),
        Terrain::Fire => (0.93, 0.38, 0.10),
        Terrain::Smoke => (0.56, 0.56, 0.60),
        Terrain::Snow => (0.94, 0.95, 0.98),
        Terrain::Building => (0.42, 0.42, 0.48),
        Terrain::Wall => (0.20, 0.20, 0.24),
        Terrain::Sand => (0.88, 0.80, 0.55),
    };
    Color::from_rgb(red, green, blue)
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
        last: BattleHexCoordinate,
    },
    Panning {
        last: Point,
    },
}

/// Whether a coordinate is on the map.
fn contains(map: &BattleMapAsset, coordinate: BattleHexCoordinate) -> bool {
    (0..i32::from(map.width)).contains(&coordinate.x)
        && (0..i32::from(map.height)).contains(&coordinate.y)
}

/// The map view: turns input into messages and hands the GPU a [`MapPrimitive`] to draw.
pub struct MapView<'a> {
    pub document: &'a Document,
    pub camera: Camera,
    pub hover: Option<BattleHexCoordinate>,
    pub brush_radius: u8,
}

impl MapView<'_> {
    /// The on-map hex under a screen position relative to the view.
    fn on_map(&self, position: Point) -> Option<BattleHexCoordinate> {
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
                Some(Action::publish(Message::Paint(coordinate)).and_capture())
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
                // Scrolling pans in both directions; Ctrl+scroll zooms around the cursor.
                // Line deltas from mouse wheels count as this many pixels.
                const LINE: f32 = 40.0;
                let scroll = match delta {
                    mouse::ScrollDelta::Lines { x, y } => Vector::new(x * LINE, y * LINE),
                    mouse::ScrollDelta::Pixels { x, y } => Vector::new(*x, *y),
                };
                let message = if state.modifiers.control() {
                    Message::Zoomed {
                        factor: 1.15_f32.powf(scroll.y / LINE),
                        anchor: position,
                    }
                } else {
                    Message::Panned(scroll)
                };
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
                    let coordinate = BattleHexCoordinate { x, y };
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
            let coordinate = BattleHexCoordinate { x, y };
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
