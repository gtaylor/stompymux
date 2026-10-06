//! Mappy: a desktop viewer and editor for BattleTech map assets.
//!
//! `mappy [MAP_DIR]` edits the `.toml` map files in `MAP_DIR` (default `game/maps`). The File
//! menu creates maps, opens them from that directory and saves them back into it. The toolbar
//! picks a brush, which paints one layer: elevation, terrain (ground or water), foliage,
//! routes, structures, or conditions (weather, or fire and smoke). The Options dialog sets the
//! map's rule flags, gravity, temperature, light, visibility and wind. The left mouse button
//! paints; Alt+click picks up a hex's layers into every brush. Scrolling, right or middle drag
//! and the arrow keys pan; Shift+scroll pans sideways and Ctrl+scroll zooms. Maps are read and written by the game's
//! own map file code in `stompymux-map`, so whatever Mappy saves loads the same in the server.
mod brush_panel;
mod document;
mod map_view;
mod render;

use std::path::PathBuf;

use iced::{
    Alignment, Color, Element, Fill, Point, Size, Subscription, Task, Theme, Vector, keyboard,
    widget::{
        button, center, checkbox, column, container, mouse_area, opaque, operation, radio, row,
        rule, scrollable, shader, slider, space, stack, text, text_input, tooltip,
    },
};
use stompymux_map::{Hex, HexCoordinate, Light, MAX_VISIBILITY, MapFlag, StructureKind, Wind};

use brush_panel::{BrushEdit, BrushMode, BrushPanel};
use document::{Document, MapSettings};
use map_view::{Camera, MapView};
use render::LABEL_LEGEND;

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
    Hovered(Option<HexCoordinate>),
    Paint(HexCoordinate),
    StrokeEnded,
    Pick(HexCoordinate),
    Panned(Vector),
    /// Scale hexes by `factor`, keeping the map point under `anchor` still.
    Zoomed {
        factor: f32,
        anchor: Point,
    },
    Fit,
    Brush(BrushEdit),
    Undo,
    Redo,
    /// Show or hide the File menu.
    ToggleFileMenu,
    CloseMenu,
    ShowDialog(Dialog),
    CloseDialog,
    /// Close the open dialog, or else the open menu.
    Escape,
    FilterChanged(String),
    RefreshList,
    Open(String),
    NewWidth(String),
    NewHeight(String),
    CreateNew,
    SaveNameChanged(String),
    /// Save to the map's file, or ask for a name if it has none.
    Save,
    /// Save under the name in the Save As dialog.
    SaveAs,
    /// Replace the map's settings as one undoable edit.
    SetSettings(MapSettings),
    /// Replace the map's settings as part of a slider drag, which `StrokeEnded` closes into
    /// one undoable edit.
    DragSettings(MapSettings),
}

/// A modal dialog opened from the File menu or the toolbar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    New,
    Open,
    SaveAs,
    /// The map's settings, which apply as they change.
    Options,
}

/// Height of the menu bar, which the File menu drops down below.
const MENU_BAR_HEIGHT: f32 = 30.0;

/// Widget ids of the inputs dialogs focus when they open.
const NEW_WIDTH_INPUT: &str = "new-width";
const OPEN_FILTER_INPUT: &str = "open-filter";
const SAVE_NAME_INPUT: &str = "save-name";

/// Application state.
struct Mappy {
    map_dir: PathBuf,
    maps: Vec<String>,
    filter: String,
    file_menu_open: bool,
    dialog: Option<Dialog>,
    /// Why the open dialog's last action failed.
    dialog_error: String,
    document: Document,
    camera: Camera,
    viewport: Option<Size>,
    /// Frame the map in the next reported viewport, for maps opened before the canvas has a
    /// size.
    frame_pending: bool,
    hover: Option<HexCoordinate>,
    brush: BrushPanel,
    new_width: String,
    new_height: String,
    save_name: String,
    /// Result of the last file operation.
    status: String,
}

impl Mappy {
    fn new(map_dir: PathBuf) -> Self {
        let mut mappy = Self {
            map_dir,
            maps: Vec::new(),
            filter: String::new(),
            file_menu_open: false,
            dialog: None,
            dialog_error: String::new(),
            document: Document::new(30, 30).expect("default map size is valid"),
            camera: Camera::default(),
            viewport: None,
            frame_pending: true,
            hover: None,
            brush: BrushPanel::default(),
            new_width: "30".into(),
            new_height: "30".into(),
            save_name: String::new(),
            status: String::new(),
        };
        mappy.refresh_list();
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
                if self.frame_pending {
                    self.frame_new_map();
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
            Message::Undo => self.document.undo(),
            Message::Redo => self.document.redo(),
            Message::ToggleFileMenu => self.file_menu_open = !self.file_menu_open,
            Message::CloseMenu => self.file_menu_open = false,
            Message::ShowDialog(dialog) => return self.show_dialog(dialog),
            Message::CloseDialog => self.dialog = None,
            Message::Escape => {
                if self.dialog.is_some() {
                    self.dialog = None;
                } else {
                    self.file_menu_open = false;
                }
            }
            Message::FilterChanged(filter) => self.filter = filter,
            Message::RefreshList => self.refresh_list(),
            Message::Open(name) => {
                let result = self.open(name);
                self.finish_dialog(result);
            }
            Message::NewWidth(value) => self.new_width = value,
            Message::NewHeight(value) => self.new_height = value,
            Message::CreateNew => {
                let result = self.create_new();
                self.finish_dialog(result);
            }
            Message::SaveNameChanged(name) => self.save_name = name,
            Message::Save => {
                self.file_menu_open = false;
                let Some(path) = self.document.path.clone() else {
                    return self.show_dialog(Dialog::SaveAs);
                };
                if let Err(error) = self.save(path) {
                    self.status = format!("{error:#}");
                }
            }
            Message::SaveAs => {
                let result = self.save_as();
                self.finish_dialog(result);
            }
            Message::SetSettings(settings) => self.document.set_settings(settings),
            Message::DragSettings(settings) => self.document.drag_settings(settings),
        }
        Task::none()
    }

    /// Close the menu and open `dialog`, focusing its first input if it has one.
    fn show_dialog(&mut self, dialog: Dialog) -> Task<Message> {
        self.file_menu_open = false;
        self.dialog = Some(dialog);
        self.dialog_error.clear();
        let input = match dialog {
            Dialog::New => NEW_WIDTH_INPUT,
            Dialog::Open => {
                self.refresh_list();
                OPEN_FILTER_INPUT
            }
            Dialog::SaveAs => SAVE_NAME_INPUT,
            Dialog::Options => return Task::none(),
        };
        operation::focus(input)
    }

    /// Close the dialog after its action succeeded, or show why it failed.
    fn finish_dialog(&mut self, result: anyhow::Result<()>) {
        match result {
            Ok(()) => self.dialog = None,
            Err(error) => self.dialog_error = format!("{error:#}"),
        }
    }

    /// Open the map file `name` from the map directory.
    fn open(&mut self, name: String) -> anyhow::Result<()> {
        let document = Document::open(&self.map_dir.join(&name))?;
        self.status = format!("Opened {name}");
        self.save_name = name;
        self.replace_document(document);
        Ok(())
    }

    /// Replace the map with a blank one of the size in the New dialog.
    fn create_new(&mut self) -> anyhow::Result<()> {
        let (Ok(width), Ok(height)) = (
            self.new_width.trim().parse(),
            self.new_height.trim().parse(),
        ) else {
            anyhow::bail!("width and height must be numbers");
        };
        let document = Document::new(width, height)?;
        self.status = "New map".into();
        self.save_name.clear();
        self.replace_document(document);
        Ok(())
    }

    /// Show the whole map.
    fn fit(&mut self) {
        let Some(viewport) = self.viewport else {
            return;
        };
        self.camera = Camera::fit(self.document.map.width, self.document.map.height, viewport);
    }

    /// Frame a newly opened map close enough to read its labels, now or once the canvas
    /// reports its size.
    fn frame_new_map(&mut self) {
        let Some(viewport) = self.viewport else {
            self.frame_pending = true;
            return;
        };
        self.frame_pending = false;
        let map = &self.document.map;
        self.camera = Camera::opening(map.width, map.height, viewport);
    }

    fn replace_document(&mut self, document: Document) {
        self.document = document;
        self.hover = None;
        self.frame_new_map();
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

    fn save(&mut self, path: PathBuf) -> anyhow::Result<()> {
        self.document.save_to(&path)?;
        self.status = format!("Saved {}", path.display());
        Ok(())
    }

    /// Save under the name in the Save As dialog, adding `.toml` if it is missing, and refusing
    /// to replace a different existing file.
    fn save_as(&mut self) -> anyhow::Result<()> {
        let name = self.save_name.trim();
        if name.is_empty() || name.contains(['/', '\\']) || name.starts_with('.') {
            anyhow::bail!("Enter a plain file name to save as");
        }
        let name = if name.ends_with(".toml") {
            name.to_owned()
        } else {
            format!("{name}.toml")
        };
        let path = self.map_dir.join(&name);
        if path.exists() && self.document.path.as_deref() != Some(path.as_path()) {
            anyhow::bail!("{name} already exists; open it to overwrite it");
        }
        self.save(path)?;
        self.refresh_list();
        Ok(())
    }

    /// Key bindings, with only Escape while a dialog is open so typing there leaves the map
    /// alone.
    fn subscription(&self) -> Subscription<Message> {
        if self.dialog.is_some() {
            return keyboard::listen().filter_map(dialog_key_binding);
        }
        keyboard::listen().filter_map(key_binding)
    }

    fn view(&self) -> Element<'_, Message> {
        let map = MapView {
            document: &self.document,
            camera: self.camera,
            hover: self.hover,
            brush_radius: self.brush.radius,
        };
        let body = row![
            shader(map).width(Fill).height(Fill),
            rule::vertical(1),
            self.inspector(),
        ];
        let editor = column![
            self.menu_bar(),
            rule::horizontal(1),
            self.toolbar(),
            rule::horizontal(1),
            body,
            rule::horizontal(1),
            self.status_bar()
        ];
        // The editor stays the stack's first layer so opening an overlay keeps its widget state.
        let layers = stack![editor];
        if let Some(dialog) = self.dialog {
            return layers.push(modal(self.dialog_view(dialog))).into();
        }
        if self.file_menu_open {
            return layers.push(self.file_menu()).into();
        }
        layers.into()
    }

    fn menu_bar(&self) -> Element<'_, Message> {
        let file = button(text("File").size(14))
            .padding([4, 10])
            .style(if self.file_menu_open {
                button::primary
            } else {
                button::text
            })
            .on_press(Message::ToggleFileMenu);
        row![file]
            .padding([0, 4])
            .height(MENU_BAR_HEIGHT)
            .align_y(Alignment::Center)
            .into()
    }

    /// The File menu, dropped down below the menu bar over a backdrop that closes it when
    /// clicked.
    fn file_menu(&self) -> Element<'_, Message> {
        let item = |label, shortcut, message| {
            button(
                row![
                    text(label).size(14),
                    space::horizontal(),
                    text(shortcut).size(12)
                ]
                .align_y(Alignment::Center),
            )
            .width(Fill)
            .style(button::text)
            .on_press(message)
        };
        let menu = container(
            column![
                item("New…", "Ctrl+N", Message::ShowDialog(Dialog::New)),
                item("Open…", "Ctrl+O", Message::ShowDialog(Dialog::Open)),
                rule::horizontal(1),
                item("Save", "Ctrl+S", Message::Save),
                item(
                    "Save As…",
                    "Ctrl+Shift+S",
                    Message::ShowDialog(Dialog::SaveAs)
                ),
            ]
            .spacing(2),
        )
        .padding(4)
        .width(240)
        .style(container::bordered_box);
        let placed = column![space().height(MENU_BAR_HEIGHT), menu]
            .padding([0, 4])
            .width(Fill)
            .height(Fill);
        opaque(mouse_area(placed).on_press(Message::CloseMenu))
    }

    fn dialog_view(&self, dialog: Dialog) -> Element<'_, Message> {
        let (title, body, confirm) = match dialog {
            Dialog::New => (
                "New map",
                self.new_dialog(),
                Some(("Create", Message::CreateNew)),
            ),
            Dialog::Open => ("Open map", self.open_dialog(), None),
            Dialog::SaveAs => (
                "Save map as",
                self.save_as_dialog(),
                Some(("Save", Message::SaveAs)),
            ),
            Dialog::Options => ("Map options", self.options_dialog(), None),
        };
        // Options apply as they change, so that dialog is closed rather than cancelled.
        let dismiss = if dialog == Dialog::Options {
            "Close"
        } else {
            "Cancel"
        };
        let mut buttons = row![
            space::horizontal(),
            button(dismiss)
                .style(button::secondary)
                .on_press(Message::CloseDialog)
        ]
        .spacing(8);
        if let Some((label, message)) = confirm {
            buttons = buttons.push(button(label).on_press(message));
        }
        let mut content = column![text(title).size(18), body].spacing(12);
        if !self.dialog_error.is_empty() {
            content = content.push(
                text(&self.dialog_error)
                    .size(13)
                    .color(Color::from_rgb(1.0, 0.45, 0.4)),
            );
        }
        // The Options dialog's labelled sliders and light choices need more room.
        let width = if dialog == Dialog::Options { 600 } else { 440 };
        container(content.push(buttons))
            .padding(16)
            .width(width)
            .style(container::bordered_box)
            .into()
    }

    fn new_dialog(&self) -> Element<'_, Message> {
        let size_input = |value: &str, on_input: fn(String) -> Message| {
            text_input("", value)
                .on_input(on_input)
                .on_submit(Message::CreateNew)
                .width(64)
        };
        row![
            text("Width"),
            size_input(&self.new_width, Message::NewWidth).id(NEW_WIDTH_INPUT),
            text("Height"),
            size_input(&self.new_height, Message::NewHeight),
        ]
        .spacing(8)
        .align_y(Alignment::Center)
        .into()
    }

    fn open_dialog(&self) -> Element<'_, Message> {
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
            text(self.map_dir.display().to_string()).size(12),
            row![
                text_input("filter maps", &self.filter)
                    .id(OPEN_FILTER_INPUT)
                    .on_input(Message::FilterChanged),
                button("↻").on_press(Message::RefreshList),
            ]
            .spacing(4),
            scrollable(column(entries)).height(360),
        ]
        .spacing(8)
        .into()
    }

    fn save_as_dialog(&self) -> Element<'_, Message> {
        column![
            text(format!("Saved in {}", self.map_dir.display())).size(12),
            text_input("file name", &self.save_name)
                .id(SAVE_NAME_INPUT)
                .on_input(Message::SaveNameChanged)
                .on_submit(Message::SaveAs),
        ]
        .spacing(8)
        .into()
    }

    /// The map's settings, each change applied as one undoable edit; a slider drag undoes as
    /// one edit when it is released.
    fn options_dialog(&self) -> Element<'_, Message> {
        let settings = self.document.settings();
        let flags = i64::from(settings.flags);
        let flags = MapFlag::ALL.into_iter().map(|flag| {
            let toggle = checkbox(flag.is_set(flags))
                .label(flag.name())
                .text_size(13)
                .on_toggle(move |enabled| {
                    let flags = flag.apply(flags, enabled) as i32;
                    Message::SetSettings(MapSettings { flags, ..settings })
                });
            tooltip(
                toggle,
                container(text(flag.description()).size(13))
                    .padding(6)
                    .style(container::rounded_box),
                tooltip::Position::Right,
            )
            .gap(8)
            .into()
        });
        let drag = move |change: MapSettings| Message::DragSettings(change);
        let gravity = slider(0..=u8::MAX, settings.gravity, move |gravity| {
            drag(MapSettings {
                gravity,
                ..settings
            })
        })
        .on_release(Message::StrokeEnded);
        let temperature = slider(
            i16::from(i8::MIN)..=i16::from(i8::MAX),
            i16::from(settings.temperature),
            move |temperature| {
                let temperature = i8::try_from(temperature).unwrap_or(settings.temperature);
                drag(MapSettings {
                    temperature,
                    ..settings
                })
            },
        )
        .on_release(Message::StrokeEnded);
        // Daylight levels on one row and the night levels below them.
        let light_choice = |light: Light| -> Element<'_, Message> {
            radio(light.label(), light, Some(settings.light), move |light| {
                Message::SetSettings(MapSettings { light, ..settings })
            })
            .size(14)
            .text_size(13)
            .width(LIGHT_CHOICE_WIDTH)
            .into()
        };
        let (daylight, night) = Light::ALL.split_at(3);
        let lights = column![
            row(daylight.iter().copied().map(light_choice)),
            row(night.iter().copied().map(light_choice)),
        ]
        .spacing(6);
        let visibility = slider(0..=MAX_VISIBILITY, settings.visibility, move |visibility| {
            drag(MapSettings {
                visibility,
                ..settings
            })
        })
        .on_release(Message::StrokeEnded);
        let wind = settings.wind;
        let direction = slider(0..=359_u16, wind.direction, move |direction| {
            drag(MapSettings {
                wind: Wind { direction, ..wind },
                ..settings
            })
        })
        .on_release(Message::StrokeEnded);
        let speed = slider(0..=MAX_WIND_SPEED, wind.speed, move |speed| {
            drag(MapSettings {
                wind: Wind { speed, ..wind },
                ..settings
            })
        })
        .on_release(Message::StrokeEnded);
        let environment = column![
            setting(format!("Gravity {}%", settings.gravity), gravity),
            setting(
                format!("Temperature {} °C", settings.temperature),
                temperature
            ),
            row![text("Light").size(13).width(SETTING_LABEL_WIDTH), lights],
            setting(
                format!("Visibility {} hexes", settings.visibility),
                visibility
            ),
            setting(format!("Wind from {}°", wind.direction), direction),
            setting(format!("Wind speed {}", wind.speed), speed),
        ]
        .spacing(8);
        column![
            heading("Environment"),
            environment,
            heading("Flags"),
            column(flags).spacing(6)
        ]
        .spacing(8)
        .into()
    }

    fn toolbar(&self) -> Element<'_, Message> {
        let brushes = BrushMode::ALL.into_iter().map(|mode| {
            button(mode.name())
                .style(if self.brush.mode == mode {
                    button::primary
                } else {
                    button::secondary
                })
                .on_press(Message::Brush(BrushEdit::Mode(mode)))
                .into()
        });
        row![
            text("Brush"),
            row(brushes).spacing(4),
            rule::vertical(1),
            button("Undo").on_press_maybe(self.document.can_undo().then_some(Message::Undo)),
            button("Redo").on_press_maybe(self.document.can_redo().then_some(Message::Redo)),
            button("Fit").on_press(Message::Fit),
            button("Options").on_press(Message::ShowDialog(Dialog::Options)),
            rule::vertical(1),
            text(format!("Labels — {LABEL_LEGEND}")).size(12),
        ]
        .spacing(8)
        .padding(8)
        .height(52)
        .align_y(Alignment::Center)
        .into()
    }

    fn inspector(&self) -> Element<'_, Message> {
        let hovered = self.hover.and_then(|coordinate| {
            let hex = self.document.hex(coordinate)?;
            Some((coordinate, hex))
        });
        let hex_info: Element<'_, Message> = match hovered {
            None => text("Hover over a hex to see its layers.").size(12).into(),
            Some((coordinate, hex)) => column(
                std::iter::once(format!("At {},{}", coordinate.x, coordinate.y))
                    .chain(hex_layers(hex))
                    .map(|line| text(line).size(12).into()),
            )
            .spacing(2)
            .into(),
        };
        let panel = column![
            self.brush.view().map(Message::Brush),
            heading("Hex"),
            hex_info
        ]
        .spacing(8);
        scrollable(panel.padding(12)).width(320).height(Fill).into()
    }

    fn status_bar(&self) -> Element<'_, Message> {
        let map = &self.document.map;
        let hover = self.hover.and_then(|coordinate| {
            let hex = self.document.hex(coordinate)?;
            Some(format!(
                "{},{}  {}",
                coordinate.x,
                coordinate.y,
                hex_layers(hex).join(" · ")
            ))
        });
        container(
            row![
                text(format!("{}×{}", map.width, map.height)).width(90),
                text(hover.unwrap_or_default()).width(620),
                text(&self.status),
            ]
            .spacing(16),
        )
        .padding([4, 12])
        .into()
    }
}

/// A hex's layers in words, one per entry, for the hover readouts.
fn hex_layers(hex: Hex) -> Vec<String> {
    let mut layers = vec![format!("Level {}", hex.level())];
    match hex.water() {
        Some(water) => layers.push(format!(
            "Water {} deep, {}",
            water.depth,
            water.flow.label().to_lowercase()
        )),
        None => layers.push(hex.ground().label().to_owned()),
    }
    layers.extend(hex.foliage().map(|foliage| foliage.label().to_owned()));
    layers.extend(hex.route().map(|route| route.label().to_owned()));
    if let Some(structure) = hex.structure() {
        let height = match structure.kind {
            StructureKind::Bridge => "deck",
            StructureKind::Building | StructureKind::Wall => "height",
        };
        layers.push(format!(
            "{} {}, {height} {}, CF {}",
            structure.class.label(),
            structure.kind.label().to_lowercase(),
            structure.height,
            structure.cf
        ));
    }
    layers.extend(
        hex.condition()
            .map(|condition| condition.label().to_owned()),
    );
    layers.extend(
        hex.overlay()
            .map(|overlay| overlay.terrain().label().to_owned()),
    );
    layers
}

/// Width of each light choice in the Options dialog, so the two rows line up.
const LIGHT_CHOICE_WIDTH: f32 = 140.0;

/// Width of the labels beside the Options dialog's controls.
const SETTING_LABEL_WIDTH: f32 = 150.0;

/// The fastest wind the Options dialog's slider sets.
const MAX_WIND_SPEED: u16 = 100;

/// A labelled control in the Options dialog.
fn setting<'a>(label: String, control: impl Into<Element<'a, Message>>) -> Element<'a, Message> {
    row![
        text(label).size(13).width(SETTING_LABEL_WIDTH),
        control.into()
    ]
    .align_y(Alignment::Center)
    .into()
}

/// A dimmed backdrop that blocks the editor and centres `content`, closing the dialog when it
/// is clicked.
fn modal(content: Element<'_, Message>) -> Element<'_, Message> {
    let backdrop = center(opaque(content)).style(|_| container::Style {
        background: Some(
            Color {
                a: 0.7,
                ..Color::BLACK
            }
            .into(),
        ),
        ..container::Style::default()
    });
    opaque(mouse_area(backdrop).on_press(Message::CloseDialog))
}

/// A section heading in the inspector.
fn heading(label: &str) -> Element<'_, Message> {
    text(label).size(15).into()
}

/// Keyboard shortcuts: Ctrl+Z/Ctrl+Shift+Z/Ctrl+Y for history, Ctrl+N, Ctrl+O, Ctrl+S and
/// Ctrl+Shift+S for the File menu, Escape to close the menu, E, T, F, R, S and C for the
/// Elevation, Terrain, Foliage, Routes, Structures and Conditions brushes, digits for the
/// elevation level, `[` and `]` for brush size, Home to fit and the arrow keys (faster with
/// Shift) to pan.
/// Keys typed into text inputs are not seen.
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
                Named::Escape => return Some(Message::Escape),
                Named::Home => return Some(Message::Fit),
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
            ("n", _) => Some(Message::ShowDialog(Dialog::New)),
            ("o", _) => Some(Message::ShowDialog(Dialog::Open)),
            ("s", false) => Some(Message::Save),
            ("s", true) => Some(Message::ShowDialog(Dialog::SaveAs)),
            _ => None,
        };
    }
    match character.as_str() {
        "e" => Some(Message::Brush(BrushEdit::Mode(BrushMode::Elevation))),
        "t" => Some(Message::Brush(BrushEdit::Mode(BrushMode::Terrain))),
        "f" => Some(Message::Brush(BrushEdit::Mode(BrushMode::Foliage))),
        "r" => Some(Message::Brush(BrushEdit::Mode(BrushMode::Routes))),
        "s" => Some(Message::Brush(BrushEdit::Mode(BrushMode::Structures))),
        "c" => Some(Message::Brush(BrushEdit::Mode(BrushMode::Conditions))),
        "[" => Some(Message::Brush(BrushEdit::RadiusStep(-1))),
        "]" => Some(Message::Brush(BrushEdit::RadiusStep(1))),
        digit if digit.len() == 1 && digit.as_bytes()[0].is_ascii_digit() => {
            Some(Message::Brush(BrushEdit::Level(digit.as_bytes()[0] - b'0')))
        }
        _ => None,
    }
}

/// The one key binding while a dialog is open: Escape closes it. An input that has focus takes
/// the first Escape to drop its focus.
fn dialog_key_binding(event: keyboard::Event) -> Option<Message> {
    match event {
        keyboard::Event::KeyPressed {
            key: keyboard::Key::Named(keyboard::key::Named::Escape),
            ..
        } => Some(Message::Escape),
        _ => None,
    }
}
