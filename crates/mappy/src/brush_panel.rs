//! The layer brush: which of a hex's layers to paint, what to paint into each, and the
//! inspector controls for choosing them.
//!
//! A hex is ground at a level, optionally covered by woods or water, optionally carrying a
//! building, wall or bridge, and optionally burning or smoking. Each of those layers can be
//! switched on in the brush independently; painting changes only the switched-on layers.
use iced::{
    Alignment, Background, Border, Color, Element, Fill, Theme,
    widget::{button, checkbox, column, row, slider, text},
};
use stompymux_map::{
    BattleDecorationKind, BattleHex, Ground, MAX_DEPTH, MAX_HEIGHT, Structure, Terrain, Water,
    Woods,
};

use crate::{
    document::Brush,
    map_view::{contrast, terrain_color},
};

/// Largest brush radius, in hexes from the center.
pub const MAX_RADIUS: u8 = 5;

/// One layer of a hex the brush can paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layer {
    Level,
    Ground,
    Woods,
    Water,
    Structure,
    Overlay,
}

/// Water choices: none, open water or ice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaterKind {
    None,
    Open,
    Frozen,
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
    Enable(Layer, bool),
    Level(u8),
    Ground(Ground),
    Woods(Option<Woods>),
    Water(WaterKind),
    Depth(u8),
    Structure(StructureKind),
    StructureHeight(u8),
    Overlay(Option<BattleDecorationKind>),
    Radius(u8),
    /// Grow or shrink the brush by one step.
    RadiusStep(i8),
}

/// The brush's selections, kept for every layer whether or not it is switched on.
#[derive(Debug, Clone)]
pub struct BrushPanel {
    pub level: u8,
    pub ground: Ground,
    pub woods: Option<Woods>,
    pub water: WaterKind,
    pub depth: u8,
    pub structure: StructureKind,
    pub structure_height: u8,
    pub overlay: Option<BattleDecorationKind>,
    pub radius: u8,
    /// Layers switched on, in [`Layer`] order.
    enabled: [bool; 6],
}

impl Default for BrushPanel {
    /// Paints clear ground, leaving everything else alone.
    fn default() -> Self {
        Self {
            level: 0,
            ground: Ground::Clear,
            woods: None,
            water: WaterKind::None,
            depth: 1,
            structure: StructureKind::None,
            structure_height: 1,
            overlay: None,
            radius: 0,
            enabled: [false, true, false, false, false, false],
        }
    }
}

impl BrushPanel {
    /// Whether painting changes `layer`.
    pub fn enabled(&self, layer: Layer) -> bool {
        self.enabled[layer as usize]
    }

    /// Apply a change; choosing a value for a layer also switches that layer on.
    pub fn edit(&mut self, edit: BrushEdit) {
        let layer = match edit {
            BrushEdit::Enable(layer, enabled) => {
                self.enabled[layer as usize] = enabled;
                return;
            }
            BrushEdit::Radius(radius) => {
                self.radius = radius.min(MAX_RADIUS);
                return;
            }
            BrushEdit::RadiusStep(step) => {
                self.radius = self.radius.saturating_add_signed(step).min(MAX_RADIUS);
                return;
            }
            BrushEdit::Level(level) => {
                self.level = level.min(MAX_HEIGHT);
                Layer::Level
            }
            BrushEdit::Ground(ground) => {
                self.ground = ground;
                Layer::Ground
            }
            BrushEdit::Woods(woods) => {
                self.woods = woods;
                Layer::Woods
            }
            BrushEdit::Water(water) => {
                self.water = water;
                Layer::Water
            }
            BrushEdit::Depth(depth) => {
                self.depth = depth.min(MAX_DEPTH);
                Layer::Water
            }
            BrushEdit::Structure(structure) => {
                self.structure = structure;
                Layer::Structure
            }
            BrushEdit::StructureHeight(height) => {
                self.structure_height = height.min(MAX_HEIGHT);
                Layer::Structure
            }
            BrushEdit::Overlay(overlay) => {
                self.overlay = overlay;
                Layer::Overlay
            }
        };
        self.enabled[layer as usize] = true;
    }

    /// Take every layer's selection from `hex` and switch them all on, for the eyedropper.
    pub fn pick(&mut self, hex: BattleHex) {
        self.level = hex.level();
        self.ground = hex.ground();
        self.woods = hex.woods();
        (self.water, self.depth) = match hex.water() {
            None => (WaterKind::None, self.depth),
            Some(water) if water.frozen => (WaterKind::Frozen, water.depth),
            Some(water) => (WaterKind::Open, water.depth),
        };
        (self.structure, self.structure_height) = match hex.structure() {
            None => (StructureKind::None, self.structure_height),
            Some(Structure::Building { height }) => (StructureKind::Building, height),
            Some(Structure::Wall { height }) => (StructureKind::Wall, height),
            Some(Structure::Bridge { deck }) => (StructureKind::Bridge, deck),
        };
        self.overlay = hex.overlay();
        self.enabled = [true; 6];
    }

    /// The brush to paint with: the selections of the switched-on layers.
    pub fn brush(&self) -> Brush {
        let on = |layer| self.enabled(layer);
        let water = match self.water {
            WaterKind::None => None,
            WaterKind::Open | WaterKind::Frozen => Some(Water {
                depth: self.depth,
                frozen: self.water == WaterKind::Frozen,
            }),
        };
        let height = self.structure_height;
        let structure = match self.structure {
            StructureKind::None => None,
            StructureKind::Building => Some(Structure::Building { height }),
            StructureKind::Wall => Some(Structure::Wall { height }),
            StructureKind::Bridge => Some(Structure::Bridge { deck: height }),
        };
        Brush {
            level: on(Layer::Level).then_some(self.level),
            ground: on(Layer::Ground).then_some(self.ground),
            woods: on(Layer::Woods).then_some(self.woods),
            water: on(Layer::Water).then_some(water),
            structure: on(Layer::Structure).then_some(structure),
            overlay: on(Layer::Overlay).then_some(self.overlay),
            radius: self.radius,
        }
    }

    /// The inspector controls, one section per layer, each with its on switch.
    pub fn view(&self) -> Element<'_, BrushEdit> {
        let grounds = [
            (Ground::Clear, Terrain::Grassland, "clear"),
            (Ground::Road, Terrain::Road, "road"),
            (Ground::Rough, Terrain::Rough, "rough"),
            (Ground::Mountains, Terrain::Mountains, "mountains"),
            (Ground::Snow, Terrain::Snow, "snow"),
            (Ground::Sand, Terrain::Sand, "sand"),
        ];
        let ground_rows = grounds.chunks(3).map(|chunk| {
            row(chunk.iter().map(|&(ground, terrain, name)| {
                choice(
                    name,
                    Some(terrain_color(terrain)),
                    self.ground == ground,
                    BrushEdit::Ground(ground),
                )
            }))
            .spacing(4)
            .into()
        });
        let woods = row![
            choice("none", None, self.woods.is_none(), BrushEdit::Woods(None)),
            choice(
                "light",
                Some(terrain_color(Terrain::LightForest)),
                self.woods == Some(Woods::Light),
                BrushEdit::Woods(Some(Woods::Light)),
            ),
            choice(
                "heavy",
                Some(terrain_color(Terrain::HeavyForest)),
                self.woods == Some(Woods::Heavy),
                BrushEdit::Woods(Some(Woods::Heavy)),
            ),
        ]
        .spacing(4);
        let water = row![
            choice(
                "none",
                None,
                self.water == WaterKind::None,
                BrushEdit::Water(WaterKind::None)
            ),
            choice(
                "water",
                Some(terrain_color(Terrain::Water)),
                self.water == WaterKind::Open,
                BrushEdit::Water(WaterKind::Open),
            ),
            choice(
                "ice",
                Some(terrain_color(Terrain::Ice)),
                self.water == WaterKind::Frozen,
                BrushEdit::Water(WaterKind::Frozen),
            ),
        ]
        .spacing(4);
        let structures = [
            (StructureKind::None, None, "none"),
            (StructureKind::Building, Some(Terrain::Building), "building"),
            (StructureKind::Wall, Some(Terrain::Wall), "wall"),
            (StructureKind::Bridge, Some(Terrain::Bridge), "bridge"),
        ];
        let structure = row(structures.into_iter().map(|(kind, terrain, name)| {
            choice(
                name,
                terrain.map(terrain_color),
                self.structure == kind,
                BrushEdit::Structure(kind),
            )
        }))
        .spacing(4);
        let height_label = if self.structure == StructureKind::Bridge {
            "Deck"
        } else {
            "Height"
        };
        let overlay = row![
            choice(
                "none",
                None,
                self.overlay.is_none(),
                BrushEdit::Overlay(None)
            ),
            choice(
                "fire",
                Some(terrain_color(Terrain::Fire)),
                self.overlay == Some(BattleDecorationKind::Fire),
                BrushEdit::Overlay(Some(BattleDecorationKind::Fire)),
            ),
            choice(
                "smoke",
                Some(terrain_color(Terrain::Smoke)),
                self.overlay == Some(BattleDecorationKind::Smoke),
                BrushEdit::Overlay(Some(BattleDecorationKind::Smoke)),
            ),
        ]
        .spacing(4);
        column![
            section(self, Layer::Level, "Level"),
            amount("Level", self.level, MAX_HEIGHT, BrushEdit::Level),
            section(self, Layer::Ground, "Ground"),
            column(ground_rows).spacing(4),
            section(self, Layer::Woods, "Woods"),
            woods,
            section(self, Layer::Water, "Water"),
            water,
            amount("Depth", self.depth, MAX_DEPTH, BrushEdit::Depth),
            section(self, Layer::Structure, "Structure"),
            structure,
            amount(
                height_label,
                self.structure_height,
                MAX_HEIGHT,
                BrushEdit::StructureHeight
            ),
            section(self, Layer::Overlay, "Fire and smoke"),
            overlay,
            text("Brush").size(15),
            amount("Radius", self.radius, MAX_RADIUS, BrushEdit::Radius),
        ]
        .spacing(8)
        .into()
    }
}

/// A layer's heading with the switch that turns painting it on or off.
fn section<'a>(panel: &BrushPanel, layer: Layer, label: &'a str) -> Element<'a, BrushEdit> {
    checkbox(panel.enabled(layer))
        .label(label)
        .text_size(15)
        .on_toggle(move |enabled| BrushEdit::Enable(layer, enabled))
        .into()
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

    /// Only switched-on layers reach the brush, and choosing a value switches its layer on.
    #[test]
    fn brushes_carry_only_switched_on_layers() {
        let mut panel = BrushPanel::default();
        assert_eq!(
            panel.brush(),
            Brush {
                ground: Some(Ground::Clear),
                ..Brush::default()
            }
        );
        panel.edit(BrushEdit::Enable(Layer::Ground, false));
        panel.edit(BrushEdit::StructureHeight(30));
        panel.edit(BrushEdit::Structure(StructureKind::Bridge));
        panel.edit(BrushEdit::Level(99));
        assert_eq!(
            panel.brush(),
            Brush {
                level: Some(MAX_HEIGHT),
                structure: Some(Some(Structure::Bridge { deck: 30 })),
                ..Brush::default()
            }
        );
    }

    /// The eyedropper copies every layer, so painting reproduces the picked hex exactly.
    #[test]
    fn picking_a_hex_reproduces_it() {
        let hexes = [
            BattleHex::new(Terrain::Ice, 6).with_level(12),
            BattleHex::new(Terrain::Bridge, 4),
            BattleHex::new(Terrain::Rough, 3).with_woods(Some(Woods::Heavy)),
            BattleHex::new(Terrain::Wall, 35),
            BattleHex::at_level(2).with_overlay(Some(BattleDecorationKind::Smoke)),
        ];
        for hex in hexes {
            let mut panel = BrushPanel::default();
            panel.pick(hex);
            assert_eq!(panel.brush(), Brush::matching(hex, 0), "{hex:?}");
        }
    }
}
