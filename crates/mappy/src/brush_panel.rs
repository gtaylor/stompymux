//! The brushes and their inspector controls.
//!
//! Each brush paints one layer of a hex: Elevation the ground height, Terrain the ground or
//! water, Foliage woods, jungle and fields, Routes roads and rail, Structures buildings, walls
//! and bridges, and Conditions either the weather (ice, snow and mud) or fire and smoke. The
//! panel keeps every brush's selection, so switching brushes and back keeps the choices made.
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Theme,
    widget::{button, column, row, slider, space, text},
};
use stompymux_map::{
    Condition, ConstructionClass, DecorationKind, Flow, Foliage, Ground, Hex, MAX_DEPTH,
    MAX_HEIGHT, Route, Structure, StructureKind, Water,
};

use crate::{
    document::{Brush, Paint, TerrainFeature},
    map_view::{
        condition_color, contrast, foliage_color, ground_color, overlay_color, route_color,
        structure_color, water_color,
    },
};

/// Largest brush radius, in hexes from the center.
pub const MAX_RADIUS: u8 = 5;

/// Choice buttons laid out per row.
const CHOICES_PER_ROW: usize = 3;

/// Which brush paints, and so which layer of a hex changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushMode {
    Elevation,
    Terrain,
    Foliage,
    Routes,
    Structures,
    Conditions,
}

impl BrushMode {
    /// Every brush, in toolbar order.
    pub const ALL: [Self; 6] = [
        Self::Elevation,
        Self::Terrain,
        Self::Foliage,
        Self::Routes,
        Self::Structures,
        Self::Conditions,
    ];

    /// The brush's name in the toolbar and inspector.
    pub fn name(self) -> &'static str {
        match self {
            Self::Elevation => "Elevation",
            Self::Terrain => "Terrain",
            Self::Foliage => "Foliage",
            Self::Routes => "Routes",
            Self::Structures => "Structures",
            Self::Conditions => "Conditions",
        }
    }

    /// What painting with the brush does, for the inspector.
    fn summary(self) -> &'static str {
        match self {
            Self::Elevation => {
                "Sets ground height and leaves everything on it. Keys 0-9 pick a level."
            }
            Self::Terrain => {
                "Sets the ground or water. Ground drains water, and a bridge goes with it; \
                 pavement and heavy industrial clear foliage, and magma also clears routes and \
                 weather. Water lies over clear ground, clearing foliage, routes, buildings and \
                 walls and washing away snow and mud; bridges and ice stay. Rapids and torrents \
                 need depth 1 or more."
            }
            Self::Foliage => {
                "Grows woods, jungle or fields on dry ground that supports them, where no \
                 building or wall stands. None clears foliage."
            }
            Self::Routes => {
                "Lays a road or rail line through dry hexes without a building or wall, and \
                 never over magma. None removes it."
            }
            Self::Structures => {
                "Buildings and walls drain water and clear foliage and routes; a bridge brings \
                 still water one level deep to a dry hex. New structures start at their \
                 class's full construction factor. None leaves the terrain."
            }
            Self::Conditions => {
                "Paints the weather or fire and smoke, whichever group was picked last. Snow \
                 and mud lie only on dry ground that is not magma; ice also freezes water."
            }
        }
    }
}

/// Terrain choices: one kind of ground, or water.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainKind {
    Ground(Ground),
    Water,
}

/// Which layer the Conditions brush paints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConditionLayer {
    /// Ice, snow or mud.
    Weather,
    /// Fire or smoke.
    Overlay,
}

/// A change to the brush from its controls or the keyboard.
#[derive(Debug, Clone)]
pub enum BrushEdit {
    Mode(BrushMode),
    /// Pick a level, switching to the Elevation brush.
    Level(u8),
    Terrain(TerrainKind),
    Depth(u8),
    Flow(Flow),
    Foliage(Option<Foliage>),
    Route(Option<Route>),
    Structure(Option<StructureKind>),
    Class(ConstructionClass),
    StructureHeight(u8),
    /// Pick a weather condition, making the Conditions brush paint weather.
    Weather(Option<Condition>),
    /// Pick fire or smoke, making the Conditions brush paint them.
    Overlay(Option<DecorationKind>),
    Radius(u8),
    /// Grow or shrink the brush by one step.
    RadiusStep(i8),
}

/// The selected brush and every brush's selections.
#[derive(Debug, Clone)]
pub struct BrushPanel {
    pub mode: BrushMode,
    pub level: u8,
    pub terrain: TerrainKind,
    pub depth: u8,
    pub flow: Flow,
    pub foliage: Option<Foliage>,
    pub route: Option<Route>,
    pub structure: Option<StructureKind>,
    pub class: ConstructionClass,
    pub structure_height: u8,
    pub condition_layer: ConditionLayer,
    pub condition: Option<Condition>,
    pub overlay: Option<DecorationKind>,
    pub radius: u8,
}

impl Default for BrushPanel {
    /// The Terrain brush painting clear ground.
    fn default() -> Self {
        Self {
            mode: BrushMode::Terrain,
            level: 0,
            terrain: TerrainKind::Ground(Ground::Clear),
            depth: 1,
            flow: Flow::Still,
            foliage: Some(Foliage::LightWoods),
            route: Some(Route::PavedRoad),
            structure: Some(StructureKind::Building),
            class: ConstructionClass::Medium,
            structure_height: 1,
            condition_layer: ConditionLayer::Weather,
            condition: Some(Condition::ThinSnow),
            overlay: Some(DecorationKind::Fire),
            radius: 0,
        }
    }
}

impl BrushPanel {
    /// Apply a change.
    pub fn edit(&mut self, edit: BrushEdit) {
        match edit {
            BrushEdit::Mode(mode) => self.mode = mode,
            BrushEdit::Level(level) => {
                self.mode = BrushMode::Elevation;
                self.level = level.min(MAX_HEIGHT);
            }
            BrushEdit::Terrain(terrain) => self.terrain = terrain,
            BrushEdit::Depth(depth) => self.depth = depth.min(MAX_DEPTH),
            BrushEdit::Flow(flow) => self.flow = flow,
            BrushEdit::Foliage(foliage) => self.foliage = foliage,
            BrushEdit::Route(route) => self.route = route,
            BrushEdit::Structure(structure) => self.structure = structure,
            BrushEdit::Class(class) => self.class = class,
            BrushEdit::StructureHeight(height) => self.structure_height = height.min(MAX_HEIGHT),
            BrushEdit::Weather(condition) => {
                self.condition_layer = ConditionLayer::Weather;
                self.condition = condition;
            }
            BrushEdit::Overlay(overlay) => {
                self.condition_layer = ConditionLayer::Overlay;
                self.overlay = overlay;
            }
            BrushEdit::Radius(radius) => self.radius = radius.min(MAX_RADIUS),
            BrushEdit::RadiusStep(step) => {
                self.radius = self.radius.saturating_add_signed(step).min(MAX_RADIUS);
            }
        }
    }

    /// Take every brush's selection from `hex`, for the eyedropper, keeping the selected brush
    /// and the Conditions brush's layer.
    pub fn pick(&mut self, hex: Hex) {
        self.level = hex.level();
        self.terrain = match TerrainFeature::of(hex) {
            TerrainFeature::Ground(ground) => TerrainKind::Ground(ground),
            TerrainFeature::Water(water) => {
                self.depth = water.depth;
                self.flow = water.flow;
                TerrainKind::Water
            }
        };
        self.foliage = hex.foliage();
        self.route = hex.route();
        self.structure = hex.structure().map(|structure| structure.kind);
        if let Some(structure) = hex.structure() {
            self.class = structure.class;
            self.structure_height = structure.height;
        }
        self.condition = hex.condition();
        self.overlay = hex.overlay();
    }

    /// The selected brush with its selections.
    pub fn brush(&self) -> Brush {
        let paint = match self.mode {
            BrushMode::Elevation => Paint::Level(self.level),
            BrushMode::Terrain => Paint::Terrain(self.terrain_feature()),
            BrushMode::Foliage => Paint::Foliage(self.foliage),
            BrushMode::Routes => Paint::Route(self.route),
            BrushMode::Structures => Paint::Structure(self.structure_paint()),
            BrushMode::Conditions => match self.condition_layer {
                ConditionLayer::Weather => Paint::Condition(self.condition),
                ConditionLayer::Overlay => Paint::Overlay(self.overlay),
            },
        };
        Brush {
            paint,
            radius: self.radius,
        }
    }

    fn terrain_feature(&self) -> TerrainFeature {
        match self.terrain {
            TerrainKind::Ground(ground) => TerrainFeature::Ground(ground),
            TerrainKind::Water => TerrainFeature::Water(Water {
                depth: self.depth,
                flow: self.flow,
            }),
        }
    }

    fn structure_paint(&self) -> Option<Structure> {
        let kind = self.structure?;
        Some(Structure::new(kind, self.structure_height, self.class))
    }

    /// The selected brush's controls, then the brush size.
    pub fn view(&self) -> Element<'_, BrushEdit> {
        let options = match self.mode {
            BrushMode::Elevation => amount("Level", self.level, MAX_HEIGHT, BrushEdit::Level),
            BrushMode::Terrain => self.terrain_options(),
            BrushMode::Foliage => self.foliage_options(),
            BrushMode::Routes => self.route_options(),
            BrushMode::Structures => self.structure_options(),
            BrushMode::Conditions => self.condition_options(),
        };
        column![
            text(format!("{} brush", self.mode.name())).size(15),
            text(self.mode.summary()).size(12),
            options,
            text("Size").size(15),
            amount("Radius", self.radius, MAX_RADIUS, BrushEdit::Radius),
        ]
        .spacing(8)
        .into()
    }

    fn terrain_options(&self) -> Element<'_, BrushEdit> {
        let grounds = Ground::ALL.into_iter().map(|ground| {
            let kind = TerrainKind::Ground(ground);
            Choice {
                label: ground.label(),
                fill: Some(ground_color(ground)),
                selected: self.terrain == kind,
                edit: BrushEdit::Terrain(kind),
            }
        });
        let water = Choice {
            label: "Water",
            fill: Some(water_color()),
            selected: self.terrain == TerrainKind::Water,
            edit: BrushEdit::Terrain(TerrainKind::Water),
        };
        let options = column![choices(grounds.chain([water]))].spacing(8);
        if self.terrain != TerrainKind::Water {
            return options.into();
        }
        let flows = Flow::ALL.into_iter().map(|flow| Choice {
            label: flow.label(),
            fill: None,
            selected: self.flow == flow,
            edit: BrushEdit::Flow(flow),
        });
        options
            .push(amount("Depth", self.depth, MAX_DEPTH, BrushEdit::Depth))
            .push(choices(flows))
            .into()
    }

    fn foliage_options(&self) -> Element<'_, BrushEdit> {
        let foliage = Foliage::ALL.into_iter().map(Some).chain([None]);
        choices(foliage.map(|foliage| Choice {
            label: foliage.map_or("None", Foliage::label),
            fill: foliage.map(foliage_color),
            selected: self.foliage == foliage,
            edit: BrushEdit::Foliage(foliage),
        }))
    }

    fn route_options(&self) -> Element<'_, BrushEdit> {
        let routes = Route::ALL.into_iter().map(Some).chain([None]);
        choices(routes.map(|route| Choice {
            label: route.map_or("None", Route::label),
            fill: route.map(route_color),
            selected: self.route == route,
            edit: BrushEdit::Route(route),
        }))
    }

    fn structure_options(&self) -> Element<'_, BrushEdit> {
        let kinds = StructureKind::ALL.into_iter().map(Some).chain([None]);
        let kinds = choices(kinds.map(|kind| Choice {
            label: kind.map_or("None", StructureKind::label),
            fill: kind.map(|kind| structure_color(kind, ConstructionClass::Light)),
            selected: self.structure == kind,
            edit: BrushEdit::Structure(kind),
        }));
        let height_label = match self.structure {
            None => return kinds,
            Some(StructureKind::Bridge) => "Deck",
            Some(StructureKind::Building | StructureKind::Wall) => "Height",
        };
        let classes = ConstructionClass::ALL.into_iter().map(|class| Choice {
            label: class.label(),
            fill: None,
            selected: self.class == class,
            edit: BrushEdit::Class(class),
        });
        column![
            kinds,
            text(format!(
                "Construction class (CF {})",
                self.class.construction_factor()
            ))
            .size(13),
            choices(classes),
            amount(
                height_label,
                self.structure_height,
                MAX_HEIGHT,
                BrushEdit::StructureHeight
            ),
        ]
        .spacing(8)
        .into()
    }

    fn condition_options(&self) -> Element<'_, BrushEdit> {
        let weather = self.condition_layer == ConditionLayer::Weather;
        let conditions = Condition::ALL.into_iter().map(Some).chain([None]);
        let conditions = choices(conditions.map(|condition| Choice {
            label: condition.map_or("None", Condition::label),
            fill: condition.map(condition_color),
            selected: weather && self.condition == condition,
            edit: BrushEdit::Weather(condition),
        }));
        let overlays = [
            Some(DecorationKind::Fire),
            Some(DecorationKind::Smoke),
            None,
        ];
        let overlays = choices(overlays.into_iter().map(|overlay| Choice {
            label: overlay.map_or("None", |overlay| overlay.terrain().label()),
            fill: overlay.map(overlay_color),
            selected: !weather && self.overlay == overlay,
            edit: BrushEdit::Overlay(overlay),
        }));
        column![
            text("Weather").size(13),
            conditions,
            text("Fire and smoke").size(13),
            overlays,
        ]
        .spacing(8)
        .into()
    }
}

/// A labelled slider for a height, depth or radius.
fn amount<'a>(
    label: &'a str,
    value: u8,
    max: u8,
    on_change: fn(u8) -> BrushEdit,
) -> Element<'a, BrushEdit> {
    row![
        text(format!("{label} {value}")).width(90),
        slider(0..=max, value, on_change),
    ]
    .align_y(Alignment::Center)
    .into()
}

/// One selectable button: its text, its map color if it has one, and what it selects.
struct Choice {
    label: &'static str,
    fill: Option<Color>,
    selected: bool,
    edit: BrushEdit,
}

/// Choice buttons in rows of [`CHOICES_PER_ROW`], the last row padded so every button is the
/// same width.
fn choices<'a>(choices: impl Iterator<Item = Choice>) -> Element<'a, BrushEdit> {
    let mut rows = column![].spacing(4);
    let mut remaining = choices.peekable();
    while remaining.peek().is_some() {
        let mut line = row![].spacing(4);
        for _ in 0..CHOICES_PER_ROW {
            line = match remaining.next() {
                Some(next) => line.push(choice(next)),
                None => line.push(space().width(Fill)),
            };
        }
        rows = rows.push(line);
    }
    rows.into()
}

/// A selectable button, filled with a map color when the choice has one.
fn choice<'a>(choice: Choice) -> Element<'a, BrushEdit> {
    let Choice {
        label,
        fill,
        selected,
        edit,
    } = choice;
    let content = text(label).size(12).center();
    let Some(fill) = fill else {
        return button(content)
            .width(Fill)
            .padding([6, 2])
            .style(if selected {
                button::primary
            } else {
                button::secondary
            })
            .on_press(edit)
            .into();
    };
    button(content)
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
        .on_press(edit)
        .into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use stompymux_map::Terrain;

    /// Each brush paints only its own selection, and picking a level switches to Elevation.
    #[test]
    fn brushes_paint_their_own_selection() {
        let mut panel = BrushPanel::default();
        assert_eq!(
            panel.brush().paint,
            Paint::Terrain(TerrainFeature::Ground(Ground::Clear))
        );
        panel.edit(BrushEdit::Terrain(TerrainKind::Water));
        panel.edit(BrushEdit::Depth(99));
        panel.edit(BrushEdit::Flow(Flow::Torrent));
        assert_eq!(
            panel.brush().paint,
            Paint::Terrain(TerrainFeature::Water(Water {
                depth: MAX_DEPTH,
                flow: Flow::Torrent
            }))
        );
        panel.edit(BrushEdit::Mode(BrushMode::Foliage));
        panel.edit(BrushEdit::Foliage(Some(Foliage::PlantedFields)));
        assert_eq!(
            panel.brush().paint,
            Paint::Foliage(Some(Foliage::PlantedFields))
        );
        panel.edit(BrushEdit::Mode(BrushMode::Routes));
        panel.edit(BrushEdit::Route(Some(Route::Rail)));
        assert_eq!(panel.brush().paint, Paint::Route(Some(Route::Rail)));
        panel.edit(BrushEdit::Mode(BrushMode::Structures));
        panel.edit(BrushEdit::Structure(Some(StructureKind::Bridge)));
        panel.edit(BrushEdit::Class(ConstructionClass::Hardened));
        panel.edit(BrushEdit::StructureHeight(3));
        assert_eq!(
            panel.brush().paint,
            Paint::Structure(Some(Structure::new(
                StructureKind::Bridge,
                3,
                ConstructionClass::Hardened
            )))
        );
        panel.edit(BrushEdit::Level(99));
        assert_eq!(panel.mode, BrushMode::Elevation);
        assert_eq!(panel.brush().paint, Paint::Level(MAX_HEIGHT));
    }

    /// The Conditions brush paints whichever group was picked from last.
    #[test]
    fn conditions_paint_the_last_picked_group() {
        let mut panel = BrushPanel::default();
        panel.edit(BrushEdit::Mode(BrushMode::Conditions));
        panel.edit(BrushEdit::Weather(Some(Condition::Ice)));
        assert_eq!(panel.brush().paint, Paint::Condition(Some(Condition::Ice)));
        panel.edit(BrushEdit::Overlay(None));
        assert_eq!(panel.brush().paint, Paint::Overlay(None));
        panel.edit(BrushEdit::Weather(None));
        assert_eq!(panel.brush().paint, Paint::Condition(None));
    }

    /// The eyedropper fills every brush, so painting each in toolbar order onto a blank hex,
    /// the Conditions brush in both of its layers, rebuilds the picked one.
    #[test]
    fn picking_a_hex_fills_every_brush() {
        let torrent = Water {
            depth: 4,
            flow: Flow::Torrent,
        };
        let hexes = [
            Hex::new(Terrain::Ice, 6).with_level(12),
            Hex::new(Terrain::Bridge, 4)
                .with_level(2)
                .with_water(Some(torrent)),
            Hex::new(Terrain::HeavyJungle, 3)
                .with_ground(Ground::Rough)
                .with_route(Some(Route::DirtRoad)),
            Hex::new(Terrain::DeepSnow, 5).with_ground(Ground::Tundra),
            Hex::new(Terrain::Rail, 7).with_condition(Some(Condition::Mud)),
            Hex::at_level(1)
                .with_ground(Ground::Magma)
                .with_structure(Some(Structure::new(
                    StructureKind::Wall,
                    35,
                    ConstructionClass::Hardened,
                ))),
            Hex::at_level(2).with_overlay(Some(DecorationKind::Smoke)),
        ];
        for hex in hexes {
            hex.validate().unwrap();
            let mut panel = BrushPanel::default();
            panel.pick(hex);
            let mut document = crate::document::Document::new(1, 1).unwrap();
            let at = stompymux_map::HexCoordinate { x: 0, y: 0 };
            for mode in BrushMode::ALL {
                panel.edit(BrushEdit::Mode(mode));
                document.paint(at, panel.brush());
            }
            panel.condition_layer = ConditionLayer::Overlay;
            document.paint(at, panel.brush());
            assert_eq!(document.hex(at), Some(hex), "{hex:?}");
        }
    }
}
