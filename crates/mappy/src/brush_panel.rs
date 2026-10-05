//! The brushes and their inspector controls.
//!
//! Each brush paints one thing: Elevation sets ground height, Terrain the ground or water,
//! Overlays woods, snow and ice, Structures buildings, walls and bridges, and Conditions fire
//! and smoke. The panel
//! keeps every brush's selection, so switching brushes and back keeps the choices made.
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Theme,
    widget::{button, column, row, slider, text},
};
use stompymux_map::{
    DecorationKind, Ground, Hex, MAX_DEPTH, MAX_HEIGHT, Structure, Terrain, Woods,
};

use crate::{
    document::{Brush, Cover, Paint, TerrainFeature},
    map_view::{contrast, terrain_color},
};

/// Largest brush radius, in hexes from the center.
pub const MAX_RADIUS: u8 = 5;

/// Which brush paints, and so which layer of a hex changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BrushMode {
    Elevation,
    Terrain,
    Overlays,
    Structures,
    Conditions,
}

impl BrushMode {
    /// Every brush, in toolbar order.
    pub const ALL: [Self; 5] = [
        Self::Elevation,
        Self::Terrain,
        Self::Overlays,
        Self::Structures,
        Self::Conditions,
    ];

    /// The brush's name in the toolbar and inspector.
    pub fn name(self) -> &'static str {
        match self {
            Self::Elevation => "Elevation",
            Self::Terrain => "Terrain",
            Self::Overlays => "Overlays",
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
                "Sets the ground or water. Woods and snow stay on clear ground and ice on water; \
                 other terrain clears them. Knocks down buildings and walls; bridges stay over \
                 water."
            }
            Self::Overlays => {
                "Woods and snow cover dry hexes without structures; ice freezes water. Remove \
                 clears woods and snow and thaws ice."
            }
            Self::Structures => {
                "Buildings and walls clear the terrain under them; bridges bring water. Removing \
                 a structure leaves the terrain."
            }
            Self::Conditions => "Starts or puts out fire and smoke over any hex.",
        }
    }
}

/// Terrain choices: one kind of ground, or water.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainKind {
    Ground(Ground),
    Water,
}

/// Structure choices; the brush's structure height is the building or wall height, or the
/// bridge deck's height above the water.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StructureKind {
    None,
    Building,
    Wall,
    Bridge,
}

/// A change to the brush from its controls or the keyboard.
#[derive(Debug, Clone)]
pub enum BrushEdit {
    Mode(BrushMode),
    /// Pick a level, switching to the Elevation brush.
    Level(u8),
    Terrain(TerrainKind),
    Depth(u8),
    Cover(Option<Cover>),
    Structure(StructureKind),
    StructureHeight(u8),
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
    pub cover: Option<Cover>,
    pub structure: StructureKind,
    pub structure_height: u8,
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
            cover: Some(Cover::Woods(Woods::Light)),
            structure: StructureKind::Building,
            structure_height: 1,
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
            BrushEdit::Cover(cover) => self.cover = cover,
            BrushEdit::Structure(structure) => self.structure = structure,
            BrushEdit::StructureHeight(height) => self.structure_height = height.min(MAX_HEIGHT),
            BrushEdit::Overlay(overlay) => self.overlay = overlay,
            BrushEdit::Radius(radius) => self.radius = radius.min(MAX_RADIUS),
            BrushEdit::RadiusStep(step) => {
                self.radius = self.radius.saturating_add_signed(step).min(MAX_RADIUS);
            }
        }
    }

    /// Take every brush's selection from `hex`, for the eyedropper, keeping the selected brush.
    pub fn pick(&mut self, hex: Hex) {
        self.level = hex.level();
        self.terrain = match TerrainFeature::of(hex) {
            TerrainFeature::Ground(ground) => TerrainKind::Ground(ground),
            TerrainFeature::Water { depth } => {
                self.depth = depth;
                TerrainKind::Water
            }
        };
        self.cover = Cover::of(hex);
        (self.structure, self.structure_height) = match hex.structure() {
            None => (StructureKind::None, self.structure_height),
            Some(Structure::Building { height }) => (StructureKind::Building, height),
            Some(Structure::Wall { height }) => (StructureKind::Wall, height),
            Some(Structure::Bridge { deck }) => (StructureKind::Bridge, deck),
        };
        self.overlay = hex.overlay();
    }

    /// The selected brush with its selections.
    pub fn brush(&self) -> Brush {
        let paint = match self.mode {
            BrushMode::Elevation => Paint::Level(self.level),
            BrushMode::Terrain => Paint::Terrain(self.terrain_feature()),
            BrushMode::Overlays => Paint::Cover(self.cover),
            BrushMode::Structures => Paint::Structure(self.structure()),
            BrushMode::Conditions => Paint::Overlay(self.overlay),
        };
        Brush {
            paint,
            radius: self.radius,
        }
    }

    fn terrain_feature(&self) -> TerrainFeature {
        match self.terrain {
            TerrainKind::Ground(ground) => TerrainFeature::Ground(ground),
            TerrainKind::Water => TerrainFeature::Water { depth: self.depth },
        }
    }

    fn structure(&self) -> Option<Structure> {
        let height = self.structure_height;
        match self.structure {
            StructureKind::None => None,
            StructureKind::Building => Some(Structure::Building { height }),
            StructureKind::Wall => Some(Structure::Wall { height }),
            StructureKind::Bridge => Some(Structure::Bridge { deck: height }),
        }
    }

    /// The selected brush's controls, then the brush size.
    pub fn view(&self) -> Element<'_, BrushEdit> {
        let options = match self.mode {
            BrushMode::Elevation => amount("Level", self.level, MAX_HEIGHT, BrushEdit::Level),
            BrushMode::Terrain => self.terrain_options(),
            BrushMode::Overlays => self.cover_options(),
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
        let ground = |ground, terrain, name| (TerrainKind::Ground(ground), terrain, name);
        let rows = [
            [
                ground(Ground::Clear, Terrain::Grassland, "clear"),
                ground(Ground::Road, Terrain::Road, "road"),
                ground(Ground::Rough, Terrain::Rough, "rough"),
            ],
            [
                ground(Ground::Mountains, Terrain::Mountains, "mountains"),
                ground(Ground::Sand, Terrain::Sand, "sand"),
                (TerrainKind::Water, Terrain::Water, "water"),
            ],
        ];
        let rows = rows.into_iter().map(|choices| {
            row(choices.into_iter().map(|(kind, terrain, name)| {
                choice(
                    name,
                    Some(terrain_color(terrain)),
                    self.terrain == kind,
                    BrushEdit::Terrain(kind),
                )
            }))
            .spacing(4)
            .into()
        });
        let mut options = column(rows).spacing(4);
        if self.terrain == TerrainKind::Water {
            options = options.push(amount("Depth", self.depth, MAX_DEPTH, BrushEdit::Depth));
        }
        options.spacing(8).into()
    }

    fn cover_options(&self) -> Element<'_, BrushEdit> {
        let rows: [&[Option<Cover>]; 2] = [
            &[None, Some(Cover::Snow), Some(Cover::Ice)],
            &[
                Some(Cover::Woods(Woods::Light)),
                Some(Cover::Woods(Woods::Heavy)),
            ],
        ];
        let rows = rows.into_iter().map(|covers| {
            row(covers.iter().map(|&cover| {
                let (terrain, name) = match cover {
                    None => (None, "remove"),
                    Some(Cover::Snow) => (Some(Terrain::Snow), "snow"),
                    Some(Cover::Ice) => (Some(Terrain::Ice), "ice"),
                    Some(Cover::Woods(Woods::Light)) => (Some(Terrain::LightForest), "light woods"),
                    Some(Cover::Woods(Woods::Heavy)) => (Some(Terrain::HeavyForest), "heavy woods"),
                };
                choice(
                    name,
                    terrain.map(terrain_color),
                    self.cover == cover,
                    BrushEdit::Cover(cover),
                )
            }))
            .spacing(4)
            .into()
        });
        column(rows).spacing(4).into()
    }

    fn structure_options(&self) -> Element<'_, BrushEdit> {
        let structures = [
            (StructureKind::None, None, "remove"),
            (StructureKind::Building, Some(Terrain::Building), "building"),
            (StructureKind::Wall, Some(Terrain::Wall), "wall"),
            (StructureKind::Bridge, Some(Terrain::Bridge), "bridge"),
        ];
        let choices = row(structures.into_iter().map(|(kind, terrain, name)| {
            choice(
                name,
                terrain.map(terrain_color),
                self.structure == kind,
                BrushEdit::Structure(kind),
            )
        }))
        .spacing(4);
        let height_label = match self.structure {
            StructureKind::None => return choices.into(),
            StructureKind::Bridge => "Deck",
            StructureKind::Building | StructureKind::Wall => "Height",
        };
        column![
            choices,
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
        let overlays = [
            (None, None, "none"),
            (Some(DecorationKind::Fire), Some(Terrain::Fire), "fire"),
            (Some(DecorationKind::Smoke), Some(Terrain::Smoke), "smoke"),
        ];
        row(overlays.into_iter().map(|(overlay, terrain, name)| {
            choice(
                name,
                terrain.map(terrain_color),
                self.overlay == overlay,
                BrushEdit::Overlay(overlay),
            )
        }))
        .spacing(4)
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

/// A selectable button, filled with a map color when the choice has one.
fn choice<'a>(
    label: &'a str,
    fill: Option<Color>,
    selected: bool,
    edit: BrushEdit,
) -> Element<'a, BrushEdit> {
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
        assert_eq!(
            panel.brush().paint,
            Paint::Terrain(TerrainFeature::Water { depth: MAX_DEPTH })
        );
        panel.edit(BrushEdit::Mode(BrushMode::Overlays));
        panel.edit(BrushEdit::Cover(Some(Cover::Ice)));
        assert_eq!(panel.brush().paint, Paint::Cover(Some(Cover::Ice)));
        panel.edit(BrushEdit::Mode(BrushMode::Structures));
        panel.edit(BrushEdit::Structure(StructureKind::Bridge));
        panel.edit(BrushEdit::StructureHeight(3));
        assert_eq!(
            panel.brush().paint,
            Paint::Structure(Some(Structure::Bridge { deck: 3 }))
        );
        panel.edit(BrushEdit::Level(99));
        assert_eq!(panel.mode, BrushMode::Elevation);
        assert_eq!(panel.brush().paint, Paint::Level(MAX_HEIGHT));
    }

    /// The eyedropper fills every brush, so painting each in toolbar order onto a blank hex
    /// rebuilds the picked one.
    #[test]
    fn picking_a_hex_fills_every_brush() {
        let hexes = [
            Hex::new(Terrain::Ice, 6).with_level(12),
            Hex::new(Terrain::Bridge, 4).with_level(2),
            Hex::new(Terrain::HeavyForest, 3),
            Hex::new(Terrain::Snow, 5),
            Hex::new(Terrain::Mountains, 7),
            Hex::new(Terrain::Wall, 35),
            Hex::at_level(2).with_overlay(Some(DecorationKind::Smoke)),
        ];
        for hex in hexes {
            let mut panel = BrushPanel::default();
            panel.pick(hex);
            let mut document = crate::document::Document::new(1, 1).unwrap();
            let at = stompymux_map::HexCoordinate { x: 0, y: 0 };
            for mode in BrushMode::ALL {
                panel.edit(BrushEdit::Mode(mode));
                document.paint(at, panel.brush());
            }
            assert_eq!(document.hex(at), Some(hex), "{hex:?}");
        }
    }
}
