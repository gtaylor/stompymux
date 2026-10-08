//! Mappy: a desktop viewer and editor for BattleTech map assets.
//!
//! `mappy [MAP_DIR]` edits the `.toml` map files in `MAP_DIR` (default `game/maps`). The File
//! menu creates maps, opens them from that directory and saves them back into it, offering to
//! save unsaved changes first, as closing the window does. The Map menu's Options dialog sets
//! the map's rule flags, gravity, temperature, light, visibility and wind, and its Resize
//! dialog changes the map's size at the chosen edges. Its Map generator panel describes a
//! battlefield for `stompymux-mapgen` and previews it live in place of the map; applying it
//! replaces the map as one undoable edit. The toolbar picks a brush, which paints
//! one layer: elevation, terrain (ground or water), foliage, routes, structures, or conditions
//! (weather, or fire and smoke). The left mouse button
//! paints; Alt+click picks up a hex's layers into every brush. Beside the brushes, the Points
//! tool adds, selects, moves and edits the map's scripted points of interest, and the Regions
//! tool outlines its scripted regions corner by corner. Points show as markers and regions as
//! tinted, outlined areas over the map whichever tool is selected. Scrolling, right or middle drag
//! and the arrow keys pan; Shift+scroll pans sideways and Ctrl+scroll zooms. Maps are read and written by the game's
//! own map file code in `stompymux-map`, so whatever Mappy saves loads the same in the server.
mod brush_panel;
mod document;
mod generator_panel;
mod map_view;
mod points_panel;
mod regions_panel;
mod render;

use std::{path::PathBuf, sync::Arc};

use iced::{
    Alignment, Color, Element, Fill, Point, Size, Subscription, Task, Theme, Vector, keyboard,
    widget::{
        button, center, checkbox, column, container, mouse_area, opaque, operation, radio, row,
        rule, scrollable, shader, slider, space, stack, text, text_input, tooltip,
    },
    window,
};
use stompymux_map::{Hex, HexCoordinate, Light, MAX_VISIBILITY, MapFlag, StructureKind, Wind};

use brush_panel::{BrushEdit, BrushMode, BrushPanel};
use document::{Document, MapSettings, ResizeEdge};
use generator_panel::{GenerationResult, GeneratorEdit, GeneratorPanel};
use map_view::{Camera, MapView};
use points_panel::{PointEdit, PointsPanel};
use regions_panel::{RegionEdit, RegionsPanel};
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
    .exit_on_close_request(false)
    .run()
}

/// Everything the user can do, from widgets, the canvas and key bindings.
#[derive(Debug, Clone)]
pub enum Message {
    /// The canvas has this size, used to fit maps to the view.
    Viewport(Size),
    Hovered(Option<HexCoordinate>),
    /// The left button went down on a hex.
    Press(HexCoordinate),
    /// The left button was dragged onto another hex.
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
    /// Select the Points tool.
    PointsTool,
    Points(PointEdit),
    /// Select the Regions tool.
    RegionsTool,
    Regions(RegionEdit),
    /// Delete what the selected tool has selected.
    Delete,
    Undo,
    Redo,
    /// Show or hide a menu from the menu bar.
    ToggleMenu(Menu),
    CloseMenu,
    /// Open a dialog; New and Open first offer to save unsaved changes.
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
    ResizeWidth(String),
    ResizeHeight(String),
    /// Whether the Resize dialog spreads the change over every edge.
    ResizeEvenly(bool),
    ResizeColumns(ResizeEdge),
    ResizeRows(ResizeEdge),
    /// Resize the map as the Resize dialog says.
    Resize,
    /// The window's close button was pressed.
    CloseRequested,
    /// Save the map, then carry on with the action that was waiting on that answer.
    SaveThen(Pending),
    /// Carry on with the waiting action, dropping unsaved changes.
    Discard(Pending),
    /// Open the map generator panel, previewing a generated map in place of this one.
    ShowGenerator,
    Generator(GeneratorEdit),
    /// Generation number `.0` finished.
    Generated(u64, GenerationResult),
    /// Replace the map with the generator's preview.
    ApplyGenerator,
    /// Close the generator, showing the map as it was.
    CancelGenerator,
}

/// What the left mouse button does on the map.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    /// Paint with the selected brush.
    Brush,
    /// Add, select and move points of interest.
    Points,
    /// Outline regions and move their corners.
    Regions,
}

/// A menu in the menu bar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Menu {
    File,
    Map,
}

impl Menu {
    const ALL: [Self; 2] = [Self::File, Self::Map];

    fn label(self) -> &'static str {
        match self {
            Self::File => "File",
            Self::Map => "Map",
        }
    }
}

/// An action that would drop unsaved changes, held while the user says whether to save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    /// Show the New dialog.
    New,
    /// Show the Open dialog.
    Open,
    /// Quit Mappy.
    Exit,
}

/// A modal dialog opened from a menu.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Dialog {
    New,
    Open,
    SaveAs,
    /// The map's settings, which apply as they change.
    Options,
    Resize,
    /// Whether to save unsaved changes before the waiting action.
    Unsaved(Pending),
}

/// Height of the menu bar, which menus drop down below.
const MENU_BAR_HEIGHT: f32 = 30.0;

/// Width of each menu bar button, which places each menu under its button.
const MENU_BUTTON_WIDTH: f32 = 56.0;

/// Widget ids of the inputs dialogs focus when they open.
const NEW_WIDTH_INPUT: &str = "new-width";
const OPEN_FILTER_INPUT: &str = "open-filter";
const SAVE_NAME_INPUT: &str = "save-name";
const RESIZE_WIDTH_INPUT: &str = "resize-width";

/// Application state.
struct Mappy {
    map_dir: PathBuf,
    maps: Vec<String>,
    filter: String,
    menu: Option<Menu>,
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
    tool: Tool,
    brush: BrushPanel,
    points: PointsPanel,
    regions: RegionsPanel,
    new_width: String,
    new_height: String,
    save_name: String,
    /// Action to carry on with once the Save As dialog saves.
    after_save: Option<Pending>,
    resize_width: String,
    resize_height: String,
    /// Whether a resize spreads over every edge rather than `resize_columns` and `resize_rows`.
    resize_evenly: bool,
    resize_columns: ResizeEdge,
    resize_rows: ResizeEdge,
    /// Result of the last file operation.
    status: String,
    /// The map generator, while it is open. Its preview stands in for the map, which
    /// cannot be edited or saved until the preview is applied or cancelled.
    generator: Option<GeneratorPanel>,
    /// Number of the last generation started, which tells late results from current ones.
    generation: u64,
}

impl Mappy {
    fn new(map_dir: PathBuf) -> Self {
        let mut mappy = Self {
            map_dir,
            maps: Vec::new(),
            filter: String::new(),
            menu: None,
            dialog: None,
            dialog_error: String::new(),
            document: Document::new(30, 30).expect("default map size is valid"),
            camera: Camera::default(),
            viewport: None,
            frame_pending: true,
            hover: None,
            tool: Tool::Brush,
            brush: BrushPanel::default(),
            points: PointsPanel::default(),
            regions: RegionsPanel::default(),
            new_width: "30".into(),
            new_height: "30".into(),
            save_name: String::new(),
            after_save: None,
            resize_width: String::new(),
            resize_height: String::new(),
            resize_evenly: true,
            resize_columns: ResizeEdge::End,
            resize_rows: ResizeEdge::End,
            status: String::new(),
            generator: None,
            generation: 0,
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
        if self.generator.is_some() && edits_the_map(&message) {
            return Task::none();
        }
        match message {
            Message::Viewport(size) => {
                self.viewport = Some(size);
                if self.frame_pending {
                    self.frame_new_map();
                }
            }
            Message::Hovered(hover) => self.hover = hover,
            Message::Press(coordinate) => {
                self.hover = Some(coordinate);
                let name_input = match self.tool {
                    Tool::Brush => {
                        self.document.paint(coordinate, self.brush.brush());
                        None
                    }
                    Tool::Points => self
                        .points
                        .press(&mut self.document, coordinate)
                        .then_some(points_panel::NAME_INPUT),
                    Tool::Regions => self
                        .regions
                        .press(&mut self.document, coordinate)
                        .then_some(regions_panel::NAME_INPUT),
                };
                if let Some(input) = name_input {
                    return operation::focus(input).chain(operation::select_all(input));
                }
            }
            Message::Paint(coordinate) => {
                self.hover = Some(coordinate);
                match self.tool {
                    Tool::Brush => self.document.paint(coordinate, self.brush.brush()),
                    Tool::Points => self.points.drag(&mut self.document, coordinate),
                    Tool::Regions => self.regions.drag(&mut self.document, coordinate),
                }
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
            Message::Brush(edit) => {
                if matches!(edit, BrushEdit::Mode(_) | BrushEdit::Level(_)) {
                    self.tool = Tool::Brush;
                }
                self.brush.edit(edit);
            }
            Message::PointsTool => self.tool = Tool::Points,
            Message::Points(edit) => self.points.edit(&mut self.document, edit),
            Message::RegionsTool => self.tool = Tool::Regions,
            Message::Regions(edit) => self.regions.edit(&mut self.document, edit),
            Message::Delete => match self.tool {
                Tool::Brush => {}
                Tool::Points => self.points.edit(&mut self.document, PointEdit::Delete),
                Tool::Regions => self.regions.edit(&mut self.document, RegionEdit::Delete),
            },
            Message::Undo => {
                let shown = self.shown_size();
                self.document.undo();
                self.points.forget_typing();
                self.regions.forget_typing();
                self.refit_if_resized(shown);
            }
            Message::Redo => {
                let shown = self.shown_size();
                self.document.redo();
                self.points.forget_typing();
                self.regions.forget_typing();
                self.refit_if_resized(shown);
            }
            Message::ToggleMenu(menu) => {
                self.menu = (self.menu != Some(menu)).then_some(menu);
            }
            Message::CloseMenu => self.menu = None,
            Message::ShowDialog(dialog) => {
                let pending = match dialog {
                    Dialog::New => Some(Pending::New),
                    Dialog::Open => Some(Pending::Open),
                    _ => None,
                };
                return match pending {
                    Some(pending) => self.request(pending),
                    None => self.show_dialog(dialog),
                };
            }
            Message::CloseDialog => self.close_dialog(),
            Message::Escape => {
                if self.dialog.is_some() {
                    self.close_dialog();
                } else if self.menu.is_some() {
                    self.menu = None;
                } else if self.tool == Tool::Regions
                    && self.regions.selected(&self.document).is_some()
                {
                    self.regions.edit(&mut self.document, RegionEdit::Done);
                } else if self.generator.is_some() {
                    self.close_generator();
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
                self.menu = None;
                let Some(path) = self.document.path.clone() else {
                    return self.show_dialog(Dialog::SaveAs);
                };
                if let Err(error) = self.save(path) {
                    self.status = format!("{error:#}");
                }
            }
            Message::SaveAs => {
                let result = self.save_as();
                let saved = result.is_ok();
                self.finish_dialog(result);
                if let Some(pending) = self.after_save.take_if(|_| saved) {
                    return self.proceed(pending);
                }
            }
            Message::SetSettings(settings) => self.document.set_settings(settings),
            Message::DragSettings(settings) => self.document.drag_settings(settings),
            Message::ResizeWidth(value) => self.resize_width = value,
            Message::ResizeHeight(value) => self.resize_height = value,
            Message::ResizeEvenly(evenly) => self.resize_evenly = evenly,
            Message::ResizeColumns(edge) => {
                self.resize_columns = edge;
                self.resize_evenly = false;
            }
            Message::ResizeRows(edge) => {
                self.resize_rows = edge;
                self.resize_evenly = false;
            }
            Message::Resize => {
                let result = self.resize();
                self.finish_dialog(result);
            }
            Message::CloseRequested => return self.request(Pending::Exit),
            Message::SaveThen(pending) => {
                let Some(path) = self.document.path.clone() else {
                    self.after_save = Some(pending);
                    return self.show_dialog(Dialog::SaveAs);
                };
                if let Err(error) = self.save(path) {
                    self.dialog_error = format!("{error:#}");
                    return Task::none();
                }
                return self.proceed(pending);
            }
            Message::Discard(pending) => return self.proceed(pending),
            Message::ShowGenerator => {
                self.menu = None;
                let map = &self.document.map;
                self.generator = Some(GeneratorPanel::new(
                    self.document.spec.as_ref(),
                    map.width,
                    map.height,
                ));
                self.fit();
                return self.generate_preview();
            }
            Message::Generator(edit) => {
                let Some(generator) = &mut self.generator else {
                    return Task::none();
                };
                if generator.edit(edit) {
                    return self.generate_preview();
                }
            }
            Message::Generated(id, result) => {
                let shown = self.shown_size();
                let Some(generator) = &mut self.generator else {
                    return Task::none();
                };
                let again = generator.finish(id, result);
                self.refit_if_resized(shown);
                if again {
                    return self.generate_preview();
                }
            }
            Message::ApplyGenerator => self.apply_generator(),
            Message::CancelGenerator => self.close_generator(),
        }
        Task::none()
    }

    /// Generate the generator's current choices in the background, unless a generation is
    /// running already, in which case it follows that one.
    fn generate_preview(&mut self) -> Task<Message> {
        let Some(generator) = &mut self.generator else {
            return Task::none();
        };
        self.generation += 1;
        let id = self.generation;
        let Some(spec) = generator.request(id) else {
            return Task::none();
        };
        Task::perform(
            async move {
                stompymux_mapgen::generate(&spec)
                    .map(Arc::new)
                    .map_err(|error| format!("{error:#}"))
            },
            move |result| Message::Generated(id, result),
        )
    }

    /// Replace the map with the generator's preview and close the generator, if the preview
    /// matches its current choices.
    fn apply_generator(&mut self) {
        let Some(preview) = self.generator.as_ref().and_then(GeneratorPanel::ready) else {
            return;
        };
        let generated = &preview.generated;
        let report = &generated.report;
        self.status = format!(
            "Generated map: {}×{} {}",
            report.width,
            report.height,
            report.biome.label().to_lowercase()
        );
        self.document
            .generate(preview.document.map.clone(), generated.spec.clone());
        self.points.clear();
        self.regions.clear();
        self.generator = None;
    }

    /// Close the generator, dropping its preview, and frame the map again if the preview was
    /// a different size.
    fn close_generator(&mut self) {
        let shown = self.shown_size();
        self.generator = None;
        self.refit_if_resized(shown);
    }

    /// Show the whole map if it is no longer the `shown` size, as after undoing a resize or
    /// swapping in a generated preview.
    fn refit_if_resized(&mut self, shown: (u16, u16)) {
        if self.shown_size() == shown {
            return;
        }
        self.hover = None;
        self.fit();
    }

    /// The document on screen: the generator's preview while there is one, else the map.
    fn shown(&self) -> &Document {
        self.generator
            .as_ref()
            .and_then(|generator| generator.preview.as_ref())
            .map_or(&self.document, |preview| &preview.document)
    }

    /// Width and height of the map on screen.
    fn shown_size(&self) -> (u16, u16) {
        let map = &self.shown().map;
        (map.width, map.height)
    }

    /// Carry on with `pending` now if the map has no unsaved changes, or else ask whether to
    /// save them first.
    fn request(&mut self, pending: Pending) -> Task<Message> {
        if self.document.dirty {
            return self.show_dialog(Dialog::Unsaved(pending));
        }
        self.proceed(pending)
    }

    /// Do the action that was waiting on unsaved changes.
    fn proceed(&mut self, pending: Pending) -> Task<Message> {
        match pending {
            Pending::New => self.show_dialog(Dialog::New),
            Pending::Open => self.show_dialog(Dialog::Open),
            Pending::Exit => iced::exit(),
        }
    }

    /// Close the menu and open `dialog`, focusing its first input if it has one.
    fn show_dialog(&mut self, dialog: Dialog) -> Task<Message> {
        self.menu = None;
        self.dialog = Some(dialog);
        self.dialog_error.clear();
        let input = match dialog {
            Dialog::New => NEW_WIDTH_INPUT,
            Dialog::Open => {
                self.refresh_list();
                OPEN_FILTER_INPUT
            }
            Dialog::SaveAs => {
                if let Some(name) = self
                    .document
                    .path
                    .as_deref()
                    .and_then(|path| path.file_name())
                {
                    self.save_name = name.to_string_lossy().into_owned();
                }
                SAVE_NAME_INPUT
            }
            Dialog::Resize => {
                self.resize_width = self.document.map.width.to_string();
                self.resize_height = self.document.map.height.to_string();
                RESIZE_WIDTH_INPUT
            }
            Dialog::Options | Dialog::Unsaved(_) => return Task::none(),
        };
        operation::focus(input)
    }

    /// Close the dialog, dropping any action that was waiting on it.
    fn close_dialog(&mut self) {
        self.dialog = None;
        self.after_save = None;
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

    /// Resize the map to the size and edges in the Resize dialog, then frame it again.
    fn resize(&mut self) -> anyhow::Result<()> {
        let (Ok(width), Ok(height)) = (
            self.resize_width.trim().parse(),
            self.resize_height.trim().parse(),
        ) else {
            anyhow::bail!("width and height must be numbers");
        };
        let (columns, rows) = if self.resize_evenly {
            (ResizeEdge::Both, ResizeEdge::Both)
        } else {
            (self.resize_columns, self.resize_rows)
        };
        let map = &self.document.map;
        if (width, height) == (map.width, map.height) {
            return Ok(());
        }
        self.document.resize(width, height, columns, rows)?;
        self.hover = None;
        self.frame_new_map();
        self.status = format!("Resized to {width}×{height}");
        Ok(())
    }

    /// Show the whole map.
    fn fit(&mut self) {
        let Some(viewport) = self.viewport else {
            return;
        };
        let (width, height) = self.shown_size();
        self.camera = Camera::fit(width, height, viewport);
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
        self.points.clear();
        self.regions.clear();
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

    /// Window close requests and key bindings, with only Escape while a dialog is open so
    /// typing there leaves the map alone.
    fn subscription(&self) -> Subscription<Message> {
        let keys = if self.dialog.is_some() {
            keyboard::listen().filter_map(dialog_key_binding)
        } else {
            keyboard::listen().filter_map(key_binding)
        };
        Subscription::batch([
            keys,
            window::close_requests().map(|_| Message::CloseRequested),
        ])
    }

    fn view(&self) -> Element<'_, Message> {
        let editing = self.generator.is_none();
        let tool = |tool| editing && self.tool == tool;
        let selected_region = tool(Tool::Regions)
            .then(|| self.regions.selected(&self.document))
            .flatten();
        let map = MapView {
            document: self.shown(),
            camera: self.camera,
            hover: self.hover,
            // The Points and Regions tools work on single hexes, so they outline only the
            // hovered one.
            brush_radius: if tool(Tool::Brush) || !editing {
                self.brush.radius
            } else {
                0
            },
            selected_region,
        };
        let viewport = self.viewport.unwrap_or(Size::INFINITE);
        let regions = regions_panel::overlay(
            self.shown(),
            self.camera,
            viewport,
            selected_region,
            selected_region.and(self.regions.corner(&self.document)),
        );
        let selected_point = tool(Tool::Points)
            .then(|| self.points.selected(&self.document))
            .flatten();
        let markers =
            points_panel::markers(&self.shown().map, self.camera, viewport, selected_point);
        let side = match &self.generator {
            Some(generator) => self.generator_view(generator),
            None => self.inspector(),
        };
        let body = row![
            stack![shader(map).width(Fill).height(Fill), regions, markers].clip(true),
            rule::vertical(1),
            side,
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
        if let Some(menu) = self.menu {
            return layers.push(self.menu_view(menu)).into();
        }
        layers.into()
    }

    /// The menu bar, whose menus wait while the generator is open.
    fn menu_bar(&self) -> Element<'_, Message> {
        let buttons = Menu::ALL.into_iter().map(|menu| {
            button(text(menu.label()).size(14).center().width(Fill))
                .padding([4, 10])
                .width(MENU_BUTTON_WIDTH)
                .style(if self.menu == Some(menu) {
                    button::primary
                } else {
                    button::text
                })
                .on_press_maybe(
                    self.generator
                        .is_none()
                        .then_some(Message::ToggleMenu(menu)),
                )
                .into()
        });
        row(buttons)
            .padding([0, 4])
            .height(MENU_BAR_HEIGHT)
            .align_y(Alignment::Center)
            .into()
    }

    /// An open menu, dropped down below its menu bar button over a backdrop that closes it
    /// when clicked. The menu bar stays uncovered, so another menu's button switches to it.
    fn menu_view(&self, menu: Menu) -> Element<'_, Message> {
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
        let items = match menu {
            Menu::File => column![
                item("New…", "Ctrl+N", Message::ShowDialog(Dialog::New)),
                item("Open…", "Ctrl+O", Message::ShowDialog(Dialog::Open)),
                rule::horizontal(1),
                item("Save", "Ctrl+S", Message::Save),
                item(
                    "Save As…",
                    "Ctrl+Shift+S",
                    Message::ShowDialog(Dialog::SaveAs)
                ),
            ],
            Menu::Map => column![
                item("Options…", "", Message::ShowDialog(Dialog::Options)),
                item("Resize…", "", Message::ShowDialog(Dialog::Resize)),
                rule::horizontal(1),
                item("Map generator…", "Ctrl+G", Message::ShowGenerator),
            ],
        };
        let index = Menu::ALL.iter().position(|&each| each == menu).unwrap_or(0);
        let dropdown = container(items.spacing(2))
            .padding(4)
            .width(240)
            .style(container::bordered_box);
        let placed = row![space().width(MENU_BUTTON_WIDTH * index as f32), dropdown]
            .padding([0, 4])
            .width(Fill)
            .height(Fill);
        column![
            space().height(MENU_BAR_HEIGHT),
            opaque(mouse_area(placed).on_press(Message::CloseMenu))
        ]
        .into()
    }

    fn dialog_view(&self, dialog: Dialog) -> Element<'_, Message> {
        let (title, body, actions): (&str, _, Vec<(&str, Message)>) = match dialog {
            Dialog::New => (
                "New map",
                self.new_dialog(),
                vec![("Create", Message::CreateNew)],
            ),
            Dialog::Open => ("Open map", self.open_dialog(), Vec::new()),
            Dialog::SaveAs => (
                "Save map as",
                self.save_as_dialog(),
                vec![("Save", Message::SaveAs)],
            ),
            Dialog::Options => ("Map options", self.options_dialog(), Vec::new()),
            Dialog::Resize => (
                "Resize map",
                self.resize_dialog(),
                vec![("Resize", Message::Resize)],
            ),
            Dialog::Unsaved(pending) => (
                "Unsaved changes",
                self.unsaved_dialog(pending),
                vec![
                    ("Don't save", Message::Discard(pending)),
                    ("Save", Message::SaveThen(pending)),
                ],
            ),
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
        // The last action is the dialog's main one.
        let last = actions.len().saturating_sub(1);
        for (index, (label, message)) in actions.into_iter().enumerate() {
            let style = if index == last {
                button::primary
            } else {
                button::secondary
            };
            buttons = buttons.push(button(label).style(style).on_press(message));
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

    /// The new size, and whether it spreads over every edge or which edges it changes.
    fn resize_dialog(&self) -> Element<'_, Message> {
        let map = &self.document.map;
        let size_input = |value: &str, on_input: fn(String) -> Message| {
            text_input("", value)
                .on_input(on_input)
                .on_submit(Message::Resize)
                .width(64)
        };
        let size = row![
            text("Width"),
            size_input(&self.resize_width, Message::ResizeWidth).id(RESIZE_WIDTH_INPUT),
            text("Height"),
            size_input(&self.resize_height, Message::ResizeHeight),
            text(format!("now {}×{}", map.width, map.height)).size(12),
        ]
        .spacing(8)
        .align_y(Alignment::Center);
        let mode = |label, evenly| {
            radio(
                label,
                evenly,
                Some(self.resize_evenly),
                Message::ResizeEvenly,
            )
            .size(14)
            .text_size(13)
        };
        let mut content = column![
            size,
            mode("Equally from every edge", true),
            mode("From the chosen edges", false),
        ]
        .spacing(8);
        // The edge choices always show, so the dialog keeps its layout as the mode changes;
        // picking one switches to the chosen edges.
        let evenly = self.resize_evenly;
        let edges = |label: &'static str,
                     names: [&'static str; 3],
                     chosen: ResizeEdge,
                     on_choose: fn(ResizeEdge) -> Message| {
            let choices = [ResizeEdge::Start, ResizeEdge::End, ResizeEdge::Both]
                .into_iter()
                .zip(names)
                .map(move |(edge, name)| {
                    radio(name, edge, (!evenly).then_some(chosen), on_choose)
                        .size(14)
                        .text_size(13)
                        .width(110)
                        .into()
                });
            row![text(label).size(13).width(90), row(choices)].align_y(Alignment::Center)
        };
        content = content
            .push(edges(
                "Columns at",
                ["Left", "Right", "Both"],
                self.resize_columns,
                Message::ResizeColumns,
            ))
            .push(edges(
                "Rows at",
                ["Top", "Bottom", "Both"],
                self.resize_rows,
                Message::ResizeRows,
            ));
        content.into()
    }

    /// What will happen to unsaved changes before `pending`.
    fn unsaved_dialog(&self, pending: Pending) -> Element<'_, Message> {
        let name = self
            .document
            .path
            .as_deref()
            .and_then(|path| path.file_name())
            .map_or("the untitled map".into(), |name| {
                name.to_string_lossy().into_owned()
            });
        let before = match pending {
            Pending::New => "creating a new map",
            Pending::Open => "opening another map",
            Pending::Exit => "quitting",
        };
        text(format!("Save your changes to {name} before {before}?"))
            .size(14)
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
            let choice = radio(light.label(), light, Some(settings.light), move |light| {
                Message::SetSettings(MapSettings { light, ..settings })
            })
            .size(14)
            .text_size(13)
            .width(LIGHT_CHOICE_WIDTH);
            tooltip(
                choice,
                container(text(light_effects(light)).size(13))
                    .padding(6)
                    .max_width(320)
                    .style(container::rounded_box),
                tooltip::Position::Bottom,
            )
            .gap(6)
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

    /// The generator form above its Cancel and Apply buttons, in place of the inspector.
    fn generator_view<'a>(&'a self, generator: &'a GeneratorPanel) -> Element<'a, Message> {
        let form = generator.view().map(Message::Generator);
        let apply = generator.ready().map(|_| Message::ApplyGenerator);
        let buttons = row![
            space::horizontal(),
            button("Cancel")
                .style(button::secondary)
                .on_press(Message::CancelGenerator),
            button("Apply").on_press_maybe(apply),
        ]
        .spacing(8)
        .padding(12);
        column![
            scrollable(container(form).padding(12)).height(Fill),
            rule::horizontal(1),
            buttons,
        ]
        .width(GENERATOR_WIDTH)
        .height(Fill)
        .into()
    }

    /// The brush picker, the Points tool, history buttons and label legend. While the
    /// generator is open no tool is highlighted and none can be picked, since the canvas shows
    /// its preview; the tool in use comes back when the generator closes.
    fn toolbar(&self) -> Element<'_, Message> {
        let editing = self.generator.is_none();
        let tool_button = |label, chosen: bool, message| {
            button(label)
                .style(if editing && chosen {
                    button::primary
                } else {
                    button::secondary
                })
                .on_press_maybe(editing.then_some(message))
        };
        let brushes = BrushMode::ALL.into_iter().map(|mode| {
            tool_button(
                mode.name(),
                self.tool == Tool::Brush && self.brush.mode == mode,
                Message::Brush(BrushEdit::Mode(mode)),
            )
            .into()
        });
        row![
            text("Brush"),
            row(brushes).spacing(4),
            rule::vertical(1),
            tool_button("Points", self.tool == Tool::Points, Message::PointsTool),
            tool_button("Regions", self.tool == Tool::Regions, Message::RegionsTool),
            rule::vertical(1),
            button("Undo").on_press_maybe(
                (self.generator.is_none() && self.document.can_undo()).then_some(Message::Undo)
            ),
            button("Redo").on_press_maybe(
                (self.generator.is_none() && self.document.can_redo()).then_some(Message::Redo)
            ),
            button("Fit").on_press(Message::Fit),
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
                    .chain(annotation_lines(&self.document, coordinate))
                    .map(|line| text(line).size(12).into()),
            )
            .spacing(2)
            .into(),
        };
        let tool = match self.tool {
            Tool::Brush => self.brush.view().map(Message::Brush),
            Tool::Points => self.points.view(&self.document).map(Message::Points),
            Tool::Regions => self.regions.view(&self.document).map(Message::Regions),
        };
        let panel = column![tool, heading("Hex"), hex_info].spacing(8);
        scrollable(panel.padding(12)).width(320).height(Fill).into()
    }

    fn status_bar(&self) -> Element<'_, Message> {
        let shown = self.shown();
        let map = &shown.map;
        let hover = self.hover.and_then(|coordinate| {
            let hex = shown.hex(coordinate)?;
            let mut layers = hex_layers(hex);
            layers.extend(annotation_lines(shown, coordinate));
            Some(format!(
                "{},{}  {}",
                coordinate.x,
                coordinate.y,
                layers.join(" · ")
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

/// The points of interest on a hex and the regions it belongs to in words, one per entry, for
/// the hover readouts.
fn annotation_lines(document: &Document, coordinate: HexCoordinate) -> Vec<String> {
    // Blank fields are named as blank rather than shown as empty text.
    let name = |name: &str| match name {
        "" => "(blank name)".to_owned(),
        name => format!("\"{name}\""),
    };
    let kind = |kind: &str| match kind {
        "" => "blank type".to_owned(),
        kind => kind.to_owned(),
    };
    let points = document.points_at(coordinate).map(|(_, point)| {
        let height = point.elevation.map_or_else(String::new, |elevation| {
            format!(", elevation {elevation:+}")
        });
        format!(
            "Point {} ({}{height})",
            name(&point.name),
            kind(&point.kind)
        )
    });
    let regions = document
        .regions_at(coordinate)
        .map(|(_, region)| format!("Region {} ({})", name(&region.name), kind(&region.kind)));
    points.chain(regions).collect()
}

/// What a light level does to attacks and sight, for its choice's tooltip in the Options
/// dialog.
fn light_effects(light: Light) -> String {
    let Some(heat_step) = light.heat_step() else {
        return "No to-hit modifiers. Units see as far as the map's visibility.".into();
    };
    let weapon = light.aim_modifier(false, false, None);
    let lit = light.aim_modifier(false, true, None);
    let physical = light.aim_modifier(true, false, None);
    if !light.is_night() {
        return format!(
            "Weapon attacks +{weapon} to hit, -1 for every {heat_step} heat the target has. \
             Physical attacks +{physical}.\nUnits see as far as the map's visibility; \
             searchlights do not help."
        );
    }
    let physical = if physical > 0 {
        format!("+{physical} (+0 against lit targets)")
    } else {
        "+0".into()
    };
    format!(
        "Weapon attacks +{weapon} to hit (+{lit} against lit targets or ones with their \
         searchlight on), -1 for every {heat_step} heat the target has. Physical attacks \
         {physical}.\nUnits see as far as the map's visibility, and lit targets up to three \
         times as far."
    )
}

/// Width of each light choice in the Options dialog, so the two rows line up.
const LIGHT_CHOICE_WIDTH: f32 = 140.0;

/// Width of the labels beside the Options dialog's controls.
const SETTING_LABEL_WIDTH: f32 = 150.0;

/// Width of the generator panel, which needs more room than the inspector.
const GENERATOR_WIDTH: f32 = 380.0;

/// Whether `message` changes or replaces the map, or opens a menu or dialog that would, which
/// waits while the generator's preview is on screen.
fn edits_the_map(message: &Message) -> bool {
    matches!(
        message,
        Message::Press(_)
            | Message::Paint(_)
            | Message::Pick(_)
            | Message::Brush(_)
            | Message::PointsTool
            | Message::Points(_)
            | Message::RegionsTool
            | Message::Regions(_)
            | Message::Delete
            | Message::Undo
            | Message::Redo
            | Message::ToggleMenu(_)
            | Message::ShowDialog(_)
            | Message::Save
            | Message::ShowGenerator
    )
}

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
/// Ctrl+Shift+S for the File menu, Ctrl+G for the map generator, Escape to close the menu,
/// let go of the selected region or cancel the generator, E, T, F, R, S and C for the
/// Elevation, Terrain, Foliage, Routes, Structures and Conditions brushes, P for the Points
/// tool, G for the Regions tool, Delete to remove the selected point or region corner, digits
/// for the elevation level, `[` and `]` for brush size, Home to fit and the arrow keys (faster
/// with Shift) to pan.
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
                Named::Delete => return Some(Message::Delete),
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
            ("g", _) => Some(Message::ShowGenerator),
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
        "p" => Some(Message::PointsTool),
        "g" => Some(Message::RegionsTool),
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Hover readouts quote point and region names and mark a blank name or type.
    #[test]
    fn annotation_lines_mark_blank_fields() {
        let mut document = Document::new(3, 3).unwrap();
        let point = |kind: &str, name: &str| stompymux_map::MapPointOfInterest {
            kind: kind.into(),
            name: name.into(),
            x: 1,
            y: 1,
            elevation: None,
        };
        document.set_points(vec![point("objective", "Tower"), point("", "")]);
        document.set_regions(vec![stompymux_map::MapRegion {
            kind: "deployment".into(),
            name: "North".into(),
            corners: vec![[0, 1], [2, 1]],
        }]);
        assert_eq!(
            annotation_lines(&document, HexCoordinate { x: 1, y: 1 }),
            [
                "Point \"Tower\" (objective)",
                "Point (blank name) (blank type)",
                "Region \"North\" (deployment)"
            ]
        );
        assert!(annotation_lines(&document, HexCoordinate { x: 1, y: 0 }).is_empty());
    }

    /// Each light tooltip states the modifiers the server applies at that level.
    #[test]
    fn light_effects_describe_the_modifiers() {
        assert!(light_effects(Light::Day).starts_with("No to-hit modifiers"));
        let dawn = light_effects(Light::Dawn);
        assert!(dawn.contains("+1 to hit, -1 for every 25 heat"), "{dawn}");
        assert!(dawn.contains("searchlights do not help"), "{dawn}");
        let moonless = light_effects(Light::MoonlessNight);
        assert!(
            moonless.contains("+3 to hit (+0 against lit targets"),
            "{moonless}"
        );
        assert!(moonless.contains("Physical attacks +1 (+0"), "{moonless}");
        let pitch = light_effects(Light::PitchBlack);
        assert!(pitch.contains("+4 to hit (+1 against lit"), "{pitch}");
    }
}
