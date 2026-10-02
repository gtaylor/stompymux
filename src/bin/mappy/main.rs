//! Mappy: a desktop viewer and editor for BattleTech map assets.
//!
//! `mappy [MAP_DIR]` lists the map files in `MAP_DIR` (default `game/maps`) for opening.
//! Terrain is painted with the left mouse button; Alt+click picks up a hex's terrain and
//! elevation. Scrolling, right or middle drag and the arrow keys pan; Ctrl+scroll zooms.
//! Maps are decoded, checked and encoded by the game's own map codec, so whatever Mappy
//! saves loads the same in the server.
mod document;
mod map_view;
mod render;

use std::path::PathBuf;

use iced::{
    Alignment, Background, Border, Color, Element, Fill, Point, Size, Subscription, Task, Theme,
    Vector, keyboard,
    widget::{
        button, checkbox, column, container, row, rule, scrollable, shader, slider, text,
        text_input,
    },
};
use stompymux_rs::{BattleHexCoordinate, BattleMapFlag, Terrain};

use document::{Brush, Document, MapSettings};
use map_view::{Camera, MapView, contrast, terrain_color};

fn main() -> iced::Result {
    let map_dir = std::env::args()
        .nth(1)
        .map_or_else(|| PathBuf::from("game/maps"), PathBuf::from);
    iced::application(
        move || Mappy::new(map_dir.clone()),
        Mappy::update,
        Mappy::view,
    )
    .title(Mappy::title)
    .subscription(Mappy::subscription)
    .theme(|_: &Mappy| Theme::Dark)
    .window_size(Size::new(1440.0, 900.0))
    .run()
}

/// Everything the user can do, from widgets, the canvas and key bindings.
#[derive(Debug, Clone)]
pub enum Message {
    /// The canvas has this size, used to fit maps to the view.
    Viewport(Size),
    Hovered(Option<BattleHexCoordinate>),
    Paint(BattleHexCoordinate),
    StrokeEnded,
    Pick(BattleHexCoordinate),
    Panned(Vector),
    /// Scale hexes by `factor`, keeping the map point under `anchor` still.
    Zoomed {
        factor: f32,
        anchor: Point,
    },
    Fit,
    SelectTerrain(Terrain),
    SelectElevation(u8),
    PaintTerrain(bool),
    PaintElevation(bool),
    BrushRadius(u8),
    /// Grow or shrink the brush by one step.
    BrushStep(i8),
    Undo,
    Redo,
    FilterChanged(String),
    RefreshList,
    Open(String),
    NewWidth(String),
    NewHeight(String),
    CreateNew,
    SaveNameChanged(String),
    Save,
    SaveAs,
    GravityChanged(String),
    TemperatureChanged(String),
    ApplyConditions,
    ToggleFlag(BattleMapFlag, bool),
}

/// Application state.
struct Mappy {
    map_dir: PathBuf,
    maps: Vec<String>,
    filter: String,
    document: Document,
    camera: Camera,
    viewport: Option<Size>,
    /// Fit the next reported viewport, for maps opened before the canvas has a size.
    fit_pending: bool,
    hover: Option<BattleHexCoordinate>,
    terrain: Terrain,
    elevation: u8,
    paint_terrain: bool,
    paint_elevation: bool,
    brush_radius: u8,
    new_width: String,
    new_height: String,
    save_name: String,
    gravity: String,
    temperature: String,
    /// Result of the last file operation.
    status: String,
}

impl Mappy {
    fn new(map_dir: PathBuf) -> Self {
        let mut mappy = Self {
            map_dir,
            maps: Vec::new(),
            filter: String::new(),
            document: Document::new(30, 30).expect("default map size is valid"),
            camera: Camera::default(),
            viewport: None,
            fit_pending: true,
            hover: None,
            terrain: Terrain::Grassland,
            elevation: 0,
            paint_terrain: true,
            paint_elevation: true,
            brush_radius: 0,
            new_width: "30".into(),
            new_height: "30".into(),
            save_name: String::new(),
            gravity: String::new(),
            temperature: String::new(),
            status: String::new(),
        };
        mappy.refresh_list();
        mappy.sync_conditions();
        mappy
    }

    fn title(&self) -> String {
        let name = self
            .document
            .path
            .as_deref()
            .and_then(|path| path.file_name())
            .map_or("untitled".into(), |name| {
                name.to_string_lossy().into_owned()
            });
        let dirty = if self.document.dirty { "*" } else { "" };
        format!("Mappy — {name}{dirty}")
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::Viewport(size) => {
                self.viewport = Some(size);
                if self.fit_pending {
                    self.fit();
                }
            }
            Message::Hovered(hover) => self.hover = hover,
            Message::Paint(coordinate) => {
                self.hover = Some(coordinate);
                self.document.paint(coordinate, self.brush());
            }
            Message::StrokeEnded => self.document.end_stroke(),
            Message::Pick(coordinate) => {
                if let Some(hex) = self.document.hex(coordinate) {
                    self.terrain = hex.terrain();
                    self.elevation = hex.elevation();
                }
            }
            Message::Panned(delta) => {
                self.camera.offset += delta;
            }
            Message::Zoomed { factor, anchor } => {
                self.camera = self.camera.zoomed(factor, anchor);
            }
            Message::Fit => self.fit(),
            Message::SelectTerrain(terrain) => {
                self.terrain = terrain;
                self.paint_terrain = true;
            }
            Message::SelectElevation(elevation) => {
                self.elevation = elevation;
                self.paint_elevation = true;
            }
            Message::PaintTerrain(enabled) => self.paint_terrain = enabled,
            Message::PaintElevation(enabled) => self.paint_elevation = enabled,
            Message::BrushRadius(radius) => self.brush_radius = radius.min(5),
            Message::BrushStep(step) => {
                self.brush_radius = self.brush_radius.saturating_add_signed(step).min(5);
            }
            Message::Undo => {
                self.document.undo();
                self.sync_conditions();
            }
            Message::Redo => {
                self.document.redo();
                self.sync_conditions();
            }
            Message::FilterChanged(filter) => self.filter = filter,
            Message::RefreshList => self.refresh_list(),
            Message::Open(name) => match Document::open(&self.map_dir.join(&name)) {
                Ok(document) => {
                    self.status = match document.load_issues.len() {
                        0 => format!("Opened {name}"),
                        count => format!("Opened {name} with {count} file problem(s)"),
                    };
                    self.save_name = name;
                    self.replace_document(document);
                }
                Err(error) => self.status = format!("{error:#}"),
            },
            Message::NewWidth(value) => self.new_width = value,
            Message::NewHeight(value) => self.new_height = value,
            Message::CreateNew => {
                let created = self
                    .new_width
                    .trim()
                    .parse()
                    .ok()
                    .zip(self.new_height.trim().parse().ok())
                    .ok_or_else(|| anyhow::anyhow!("width and height must be numbers"))
                    .and_then(|(width, height)| Document::new(width, height));
                match created {
                    Ok(document) => {
                        self.status = "New map".into();
                        self.save_name.clear();
                        self.replace_document(document);
                    }
                    Err(error) => self.status = format!("{error:#}"),
                }
            }
            Message::SaveNameChanged(name) => self.save_name = name,
            Message::Save => match self.document.path.clone() {
                Some(path) => self.save(path),
                None => self.save_as(),
            },
            Message::SaveAs => self.save_as(),
            Message::GravityChanged(value) => self.gravity = value,
            Message::TemperatureChanged(value) => self.temperature = value,
            Message::ApplyConditions => {
                let (Ok(gravity), Ok(temperature)) = (
                    self.gravity.trim().parse::<u8>(),
                    self.temperature.trim().parse::<i8>(),
                ) else {
                    self.status = "Gravity must be 0-255 and temperature -128 to 127".into();
                    self.sync_conditions();
                    return Task::none();
                };
                self.document.set_settings(MapSettings {
                    gravity,
                    temperature,
                    ..self.document.settings()
                });
            }
            Message::ToggleFlag(flag, enabled) => {
                let settings = self.document.settings();
                let flags = flag.apply(i64::from(settings.flags), enabled) as i32;
                self.document
                    .set_settings(MapSettings { flags, ..settings });
            }
        }
        Task::none()
    }

    /// The brush built from the palette selections.
    fn brush(&self) -> Brush {
        Brush {
            terrain: self.paint_terrain.then_some(self.terrain),
            elevation: self.paint_elevation.then_some(self.elevation),
            radius: self.brush_radius,
        }
    }

    /// Fit the map to the canvas, or once the canvas reports its size.
    fn fit(&mut self) {
        let Some(viewport) = self.viewport else {
            self.fit_pending = true;
            return;
        };
        self.fit_pending = false;
        self.camera = Camera::fit(self.document.map.width, self.document.map.height, viewport);
    }

    fn replace_document(&mut self, document: Document) {
        self.document = document;
        self.hover = None;
        self.sync_conditions();
        self.fit();
    }

    /// Reset the gravity and temperature inputs to the map's values.
    fn sync_conditions(&mut self) {
        let settings = self.document.settings();
        self.gravity = settings.gravity.to_string();
        self.temperature = settings.temperature.to_string();
    }

    /// Reread the map directory's file names.
    fn refresh_list(&mut self) {
        let entries = match std::fs::read_dir(&self.map_dir) {
            Ok(entries) => entries,
            Err(error) => {
                self.status = format!("reading {}: {error}", self.map_dir.display());
                return;
            }
        };
        self.maps = entries
            .filter_map(Result::ok)
            .filter(|entry| entry.path().is_file())
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect();
        self.maps.sort_by_key(|name| name.to_lowercase());
    }

    fn save(&mut self, path: PathBuf) {
        self.status = match self.document.save_to(&path) {
            Ok(()) => format!("Saved {}", path.display()),
            Err(error) => format!("{error:#}"),
        };
    }

    /// Save under the name in the save box, refusing to replace a different existing file.
    fn save_as(&mut self) {
        let name = self.save_name.trim();
        if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
            self.status = "Enter a plain file name to save as".into();
            return;
        }
        let path = self.map_dir.join(name);
        if path.exists() && self.document.path.as_deref() != Some(path.as_path()) {
            self.status = format!("{name} already exists; open it to overwrite it");
            return;
        }
        self.save(path);
        self.refresh_list();
    }

    fn subscription(&self) -> Subscription<Message> {
        keyboard::listen().filter_map(key_binding)
    }

    fn view(&self) -> Element<'_, Message> {
        let map = MapView {
            document: &self.document,
            camera: self.camera,
            hover: self.hover,
            brush_radius: self.brush_radius,
        };
        let body = row![
            self.map_list(),
            rule::vertical(1),
            shader(map).width(Fill).height(Fill),
            rule::vertical(1),
            self.inspector(),
        ];
        column![
            self.toolbar(),
            rule::horizontal(1),
            body,
            rule::horizontal(1),
            self.status_bar()
        ]
        .into()
    }

    fn toolbar(&self) -> Element<'_, Message> {
        let size_input = |value: &str, on_input: fn(String) -> Message| {
            text_input("", value)
                .on_input(on_input)
                .on_submit(Message::CreateNew)
                .width(56)
        };
        row![
            text("New"),
            size_input(&self.new_width, Message::NewWidth),
            text("×"),
            size_input(&self.new_height, Message::NewHeight),
            button("Create").on_press(Message::CreateNew),
            rule::vertical(1),
            button("Save").on_press(Message::Save),
            text_input("file name", &self.save_name)
                .on_input(Message::SaveNameChanged)
                .on_submit(Message::SaveAs)
                .width(180),
            button("Save as").on_press(Message::SaveAs),
            rule::vertical(1),
            button("Undo").on_press_maybe(self.document.can_undo().then_some(Message::Undo)),
            button("Redo").on_press_maybe(self.document.can_redo().then_some(Message::Redo)),
            button("Fit").on_press(Message::Fit),
        ]
        .spacing(8)
        .padding(8)
        .height(52)
        .align_y(Alignment::Center)
        .into()
    }

    fn map_list(&self) -> Element<'_, Message> {
        let filter = self.filter.to_lowercase();
        let open = self
            .document
            .path
            .as_deref()
            .and_then(|path| path.file_name());
        let entries = self
            .maps
            .iter()
            .filter(|name| name.to_lowercase().contains(&filter))
            .map(|name| {
                let selected = open.is_some_and(|open| open.to_string_lossy() == *name);
                button(text(name).size(13))
                    .width(Fill)
                    .style(if selected {
                        button::primary
                    } else {
                        button::text
                    })
                    .on_press(Message::Open(name.clone()))
                    .into()
            });
        column![
            row![
                text_input("filter maps", &self.filter).on_input(Message::FilterChanged),
                button("↻").on_press(Message::RefreshList),
            ]
            .spacing(4),
            scrollable(column(entries)).height(Fill),
        ]
        .spacing(8)
        .padding(8)
        .width(220)
        .into()
    }

    fn inspector(&self) -> Element<'_, Message> {
        let swatches = Terrain::ALL.chunks(3).map(|terrains| {
            row(terrains.iter().map(|&terrain| self.swatch(terrain)))
                .spacing(4)
                .into()
        });
        let elevations = (0..=9u8).map(|elevation| {
            let selected = elevation == self.elevation;
            button(text(elevation.to_string()).center())
                .width(Fill)
                .padding([4, 0])
                .style(if selected {
                    button::primary
                } else {
                    button::secondary
                })
                .on_press(Message::SelectElevation(elevation))
                .into()
        });
        let settings = self.document.settings();
        let flags = BattleMapFlag::ALL.into_iter().map(|flag| {
            checkbox(flag.is_set(i64::from(settings.flags)))
                .label(flag.name())
                .text_size(13)
                .on_toggle(move |enabled| Message::ToggleFlag(flag, enabled))
                .into()
        });
        let condition = |label, value: &str, on_input: fn(String) -> Message| {
            row![
                text(label).width(100),
                text_input("", value)
                    .on_input(on_input)
                    .on_submit(Message::ApplyConditions)
                    .width(Fill),
            ]
            .align_y(Alignment::Center)
        };
        let issues: Vec<Element<'_, Message>> = self
            .document
            .load_issues
            .iter()
            .map(|issue| text(issue.to_string()).size(12).into())
            .collect();
        let mut panel = column![
            heading("Terrain"),
            checkbox(self.paint_terrain)
                .label("Paint terrain")
                .on_toggle(Message::PaintTerrain),
            column(swatches).spacing(4),
            heading("Elevation"),
            checkbox(self.paint_elevation)
                .label("Paint elevation")
                .on_toggle(Message::PaintElevation),
            row(elevations).spacing(2),
            heading("Brush"),
            row![
                text(format!("Radius {}", self.brush_radius)).width(80),
                slider(0..=5u8, self.brush_radius, Message::BrushRadius),
            ]
            .align_y(Alignment::Center),
            heading("Conditions"),
            condition("Gravity (%)", &self.gravity, Message::GravityChanged),
            condition(
                "Temperature",
                &self.temperature,
                Message::TemperatureChanged
            ),
            heading("Flags"),
            column(flags).spacing(4),
        ]
        .spacing(8);
        if !issues.is_empty() {
            panel = panel.push(heading("File problems (fixed on save)"));
            panel = panel.push(column(issues).spacing(4));
        }
        scrollable(panel.padding(12)).width(300).height(Fill).into()
    }

    /// A palette button filled with a terrain's map color.
    fn swatch(&self, terrain: Terrain) -> Element<'_, Message> {
        let selected = terrain == self.terrain;
        let fill = terrain_color(terrain);
        button(text(terrain.name().replace('_', " ")).size(12).center())
            .width(Fill)
            .padding([6, 2])
            .style(move |_theme: &Theme, status| button::Style {
                background: Some(Background::Color(match status {
                    button::Status::Hovered => Color { a: 0.85, ..fill },
                    _ => fill,
                })),
                text_color: contrast(fill),
                border: Border {
                    color: if selected {
                        Color::WHITE
                    } else {
                        Color::TRANSPARENT
                    },
                    width: 2.0,
                    radius: 4.0.into(),
                },
                ..button::Style::default()
            })
            .on_press(Message::SelectTerrain(terrain))
            .into()
    }

    fn status_bar(&self) -> Element<'_, Message> {
        let map = &self.document.map;
        let hover = self.hover.and_then(|coordinate| {
            let hex = self.document.hex(coordinate)?;
            Some(format!(
                "{},{}  {} {}",
                coordinate.x,
                coordinate.y,
                hex.terrain().name(),
                hex.elevation()
            ))
        });
        container(
            row![
                text(format!("{}×{}", map.width, map.height)).width(90),
                text(hover.unwrap_or_default()).width(260),
                text(&self.status),
            ]
            .spacing(16),
        )
        .padding([4, 12])
        .into()
    }
}

/// A section heading in the inspector.
fn heading(label: &str) -> Element<'_, Message> {
    text(label).size(15).into()
}

/// Keyboard shortcuts: Ctrl+Z/Ctrl+Shift+Z/Ctrl+Y for history, Ctrl+S to save, digits for
/// elevation, `[` and `]` for brush size, F to fit and the arrow keys (faster with Shift) to
/// pan. Keys typed into text inputs are not seen.
fn key_binding(event: keyboard::Event) -> Option<Message> {
    let keyboard::Event::KeyPressed { key, modifiers, .. } = event else {
        return None;
    };
    let character = match key {
        keyboard::Key::Character(character) => character,
        keyboard::Key::Named(named) => {
            use keyboard::key::Named;
            let step = if modifiers.shift() { 240.0 } else { 60.0 };
            // The map moves opposite to the arrow, so the view travels in its direction.
            let pan = match named {
                Named::ArrowLeft => Vector::new(step, 0.0),
                Named::ArrowRight => Vector::new(-step, 0.0),
                Named::ArrowUp => Vector::new(0.0, step),
                Named::ArrowDown => Vector::new(0.0, -step),
                _ => return None,
            };
            return Some(Message::Panned(pan));
        }
        keyboard::Key::Unidentified => return None,
    };
    let character = character.to_lowercase();
    if modifiers.command() {
        return match (character.as_str(), modifiers.shift()) {
            ("z", false) => Some(Message::Undo),
            ("z", true) | ("y", _) => Some(Message::Redo),
            ("s", _) => Some(Message::Save),
            _ => None,
        };
    }
    match character.as_str() {
        "f" => Some(Message::Fit),
        "[" => Some(Message::BrushStep(-1)),
        "]" => Some(Message::BrushStep(1)),
        digit if digit.len() == 1 && digit.as_bytes()[0].is_ascii_digit() => {
            Some(Message::SelectElevation(digit.as_bytes()[0] - b'0'))
        }
        _ => None,
    }
}
