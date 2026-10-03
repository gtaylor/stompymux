//! Mappy: a desktop viewer and editor for BattleTech map assets.
//!
//! `mappy [MAP_DIR]` lists the `.toml` map files in `MAP_DIR` (default `game/maps`) for
//! opening. The left mouse button paints the brush's switched-on layers; Alt+click picks up
//! every layer of a hex. Scrolling, right or middle drag and the arrow keys pan; Ctrl+scroll
//! zooms. Maps are read and written by the game's own map file code, so whatever Mappy saves
//! loads the same in the server.
mod brush_panel;
mod document;
mod map_view;
mod render;

use std::path::PathBuf;

use iced::{
    Alignment, Color, Element, Fill, Point, Size, Subscription, Task, Theme, Vector, keyboard,
    widget::{
        button, checkbox, column, container, row, rule, scrollable, shader, text, text_input,
    },
};
use stompymux_rs::{
    BattleDecorationKind, BattleGround, BattleHex, BattleHexCoordinate, BattleMapFlag,
    BattleStructure, BattleWoods,
};

use brush_panel::{BrushEdit, BrushPanel};
use document::{Document, MapSettings};
use map_view::{Camera, MapView};
use render::Label;

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
    Brush(BrushEdit),
    Label(Label),
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
    brush: BrushPanel,
    label: Label,
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
            brush: BrushPanel::default(),
            label: Label::default(),
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
                self.document.paint(coordinate, self.brush.brush());
            }
            Message::StrokeEnded => self.document.end_stroke(),
            Message::Pick(coordinate) => {
                if let Some(hex) = self.document.hex(coordinate) {
                    self.brush.pick(hex);
                }
            }
            Message::Panned(delta) => {
                self.camera.offset += delta;
            }
            Message::Zoomed { factor, anchor } => {
                self.camera = self.camera.zoomed(factor, anchor);
            }
            Message::Fit => self.fit(),
            Message::Brush(edit) => self.brush.edit(edit),
            Message::Label(label) => self.label = label,
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
                    self.status = format!("Opened {name}");
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
            .filter(|entry| {
                let path = entry.path();
                path.is_file()
                    && path
                        .extension()
                        .is_some_and(|extension| extension == "toml")
            })
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

    /// Save under the name in the save box, adding `.toml` if it is missing, and refusing to
    /// replace a different existing file.
    fn save_as(&mut self) {
        let name = self.save_name.trim();
        if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
            self.status = "Enter a plain file name to save as".into();
            return;
        }
        let name = if name.ends_with(".toml") {
            name.to_owned()
        } else {
            format!("{name}.toml")
        };
        let path = self.map_dir.join(&name);
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
            brush_radius: self.brush.radius,
            label: self.label,
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
            rule::vertical(1),
            text("Labels"),
            row(Label::ALL.into_iter().map(|label| {
                button(text(label.name()).size(13))
                    .style(if label == self.label {
                        button::primary
                    } else {
                        button::secondary
                    })
                    .on_press(Message::Label(label))
                    .into()
            }))
            .spacing(2),
            text(self.label.legend()).size(12),
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
        let mut panel = column![];
        let unsavable = self.document.unsavable().len();
        if unsavable > 0 {
            panel = panel.push(
                text(format!(
                    "{unsavable} hex(es), hatched red, can't be saved. A map file holds one of \
                     ground, woods, water, building or wall per hex, and bridges only over water."
                ))
                .size(12)
                .color(Color::from_rgb(1.0, 0.45, 0.4)),
            );
        }
        let panel = panel
            .push(self.brush.view().map(Message::Brush))
            .push(heading("Conditions"))
            .push(condition(
                "Gravity (%)",
                &self.gravity,
                Message::GravityChanged,
            ))
            .push(condition(
                "Temperature",
                &self.temperature,
                Message::TemperatureChanged,
            ))
            .push(heading("Flags"))
            .push(column(flags).spacing(4))
            .spacing(8);
        scrollable(panel.padding(12)).width(320).height(Fill).into()
    }

    fn status_bar(&self) -> Element<'_, Message> {
        let map = &self.document.map;
        let hover = self.hover.and_then(|coordinate| {
            let hex = self.document.hex(coordinate)?;
            let index = coordinate.y as usize * usize::from(map.width) + coordinate.x as usize;
            let unsavable = if self.document.unsavable().contains(&index) {
                " · can't be saved"
            } else {
                ""
            };
            Some(format!(
                "{},{}  {}{unsavable}",
                coordinate.x,
                coordinate.y,
                describe(hex)
            ))
        });
        container(
            row![
                text(format!("{}×{}", map.width, map.height)).width(90),
                text(hover.unwrap_or_default()).width(520),
                text(&self.status),
            ]
            .spacing(16),
        )
        .padding([4, 12])
        .into()
    }
}

/// A hex's layers in words, for the hover readout.
fn describe(hex: BattleHex) -> String {
    let ground = match hex.ground() {
        BattleGround::Clear => "clear",
        BattleGround::Road => "road",
        BattleGround::Rough => "rough",
        BattleGround::Mountains => "mountains",
        BattleGround::Snow => "snow",
        BattleGround::Sand => "sand",
    };
    let mut parts = vec![format!("level {} {ground}", hex.level())];
    match hex.woods() {
        Some(BattleWoods::Light) => parts.push("light woods".into()),
        Some(BattleWoods::Heavy) => parts.push("heavy woods".into()),
        None => {}
    }
    if let Some(water) = hex.water() {
        let kind = if water.frozen { "ice" } else { "water" };
        parts.push(format!("{kind} depth {}", water.depth));
    }
    match hex.structure() {
        Some(BattleStructure::Building { height }) => parts.push(format!("building {height}")),
        Some(BattleStructure::Wall { height }) => parts.push(format!("wall {height}")),
        Some(BattleStructure::Bridge { deck }) => parts.push(format!("bridge deck {deck}")),
        None => {}
    }
    match hex.overlay() {
        Some(BattleDecorationKind::Fire) => parts.push("fire".into()),
        Some(BattleDecorationKind::Smoke) => parts.push("smoke".into()),
        None => {}
    }
    parts.join(" · ")
}

/// A section heading in the inspector.
fn heading(label: &str) -> Element<'_, Message> {
    text(label).size(15).into()
}

/// Keyboard shortcuts: Ctrl+Z/Ctrl+Shift+Z/Ctrl+Y for history, Ctrl+S to save, digits for
/// the brush level, `[` and `]` for brush size, F to fit and the arrow keys (faster with Shift) to
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
        "[" => Some(Message::Brush(BrushEdit::RadiusStep(-1))),
        "]" => Some(Message::Brush(BrushEdit::RadiusStep(1))),
        digit if digit.len() == 1 && digit.as_bytes()[0].is_ascii_digit() => {
            Some(Message::Brush(BrushEdit::Level(digit.as_bytes()[0] - b'0')))
        }
        _ => None,
    }
}
