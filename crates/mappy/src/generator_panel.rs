//! The map generator panel: a form over a [`MapSpec`] whose every change regenerates a
//! preview of the map in the background.
//!
//! Fields the form leaves unset take the chosen biome's defaults, which the form names, such
//! as "Default (Rolling)". The panel tracks the one generation it has running: a change made
//! while one runs waits for it to finish and then starts another from the latest choices, so
//! quick changes never pile up work. A seed left blank is picked by the generator, then filled
//! into the form so later changes keep the same landscape until New seed is pressed.
use std::{fmt, sync::Arc};

use iced::{
    Alignment, Color, Element, Fill,
    widget::{button, checkbox, column, container, pick_list, row, rule, space, text, text_input},
};
use stompymux_mapgen::{
    Amount, Biome, GeneratedMap, MAX_DIMENSION, MIN_DIMENSION, MapSize, MapSpec, Position, Relief,
    RoadSpec, SettlementKind, SettlementLayout, SettlementSize, SettlementSpec,
};

use crate::document::Document;

/// Biome buttons laid out per row.
const BIOMES_PER_ROW: usize = 3;

/// Width of the labels beside the form's controls.
const LABEL_WIDTH: f32 = 110.0;

/// Most rivers or through roads the generator accepts.
const MAX_CROSSINGS: u8 = 8;

/// Most identical settlements one entry places.
const MAX_SETTLEMENT_COUNT: u8 = 32;

/// Color of the form's error text.
const ERROR_COLOR: Color = Color::from_rgb(1.0, 0.45, 0.4);

/// Color of generation warnings.
const WARNING_COLOR: Color = Color::from_rgb(0.95, 0.75, 0.35);

/// Color of explanatory text.
const NOTE_COLOR: Color = Color::from_rgb(0.65, 0.67, 0.7);

/// A change to the generator form.
#[derive(Debug, Clone)]
pub enum GeneratorEdit {
    Biome(Biome),
    /// Set the width and height to a preset's.
    Size(MapSize),
    Width(String),
    Height(String),
    Seed(String),
    /// Clear the seed so the next preview picks a new one.
    NewSeed,
    Relief(Option<Relief>),
    Water(Option<Amount>),
    Woods(Option<Amount>),
    Rough(Option<Amount>),
    Fire(Option<Amount>),
    Rivers(Option<u8>),
    Frozen(Option<bool>),
    AddSettlement,
    RemoveSettlement(usize),
    Settlement(usize, SettlementEdit),
    ConnectSettlements(bool),
    ThroughRoads(Option<u8>),
    RoadWidth(Option<u8>),
}

/// A change to one settlement in the form.
#[derive(Debug, Clone)]
pub enum SettlementEdit {
    Size(SettlementSize),
    Kind(Option<SettlementKind>),
    Layout(Option<SettlementLayout>),
    Position(Option<Position>),
    Walled(Option<bool>),
    Count(String),
}

/// What a finished generation produced: the map to show and how it came about.
pub type GenerationResult = Result<Arc<GeneratedMap>, String>;

/// The latest generated map, shown in place of the open map until it is applied.
pub struct Preview {
    pub document: Document,
    pub generated: Arc<GeneratedMap>,
}

/// The generator form, its preview and the generation it has running.
pub struct GeneratorPanel {
    /// The choices made. Width and height are always set; other fields left `None` take the
    /// biome's defaults.
    pub spec: MapSpec,
    width: String,
    height: String,
    seed: String,
    /// Each settlement's count as typed, in the same order as `spec.settlements`.
    counts: Vec<String>,
    pub preview: Option<Preview>,
    /// Why the form's choices or the last generation failed.
    error: String,
    /// The generation running and the seed it was asked for.
    running: Option<(u64, Option<u64>)>,
    /// Whether the choices changed while a generation was running.
    stale: bool,
}

impl GeneratorPanel {
    /// A form starting from `spec`, the spec the open map was generated from, or else from
    /// the biome defaults at `width` by `height` hexes.
    pub fn new(spec: Option<&MapSpec>, width: u16, height: u16) -> Self {
        let clamp = |value: u16| value.clamp(MIN_DIMENSION, MAX_DIMENSION);
        let (mut spec, (width, height)) = match spec {
            Some(spec) => (spec.clone(), spec.dimensions()),
            None => (MapSpec::default(), (clamp(width), clamp(height))),
        };
        (spec.width, spec.height, spec.size) = (Some(width), Some(height), None);
        let counts = spec
            .settlements
            .iter()
            .map(|settlement| settlement.count.unwrap_or(1).to_string())
            .collect();
        Self {
            width: spec.width.unwrap_or_default().to_string(),
            height: spec.height.unwrap_or_default().to_string(),
            seed: spec.seed.map(|seed| seed.to_string()).unwrap_or_default(),
            counts,
            spec,
            preview: None,
            error: String::new(),
            running: None,
            stale: false,
        }
    }

    /// The preview to apply: one generated from the form's current choices.
    pub fn ready(&self) -> Option<&Preview> {
        if self.running.is_some() || self.stale || !self.error.is_empty() {
            return None;
        }
        self.preview.as_ref()
    }

    /// Apply `edit` to the form. Returns whether the choices changed, so the preview should
    /// be generated again.
    pub fn edit(&mut self, edit: GeneratorEdit) -> bool {
        let spec = &mut self.spec;
        match edit {
            GeneratorEdit::Biome(biome) => spec.biome = Some(biome),
            GeneratorEdit::Size(size) => {
                let (width, height) = size.dimensions();
                self.width = width.to_string();
                self.height = height.to_string();
            }
            GeneratorEdit::Width(width) => self.width = width,
            GeneratorEdit::Height(height) => self.height = height,
            GeneratorEdit::Seed(seed) => self.seed = seed,
            GeneratorEdit::NewSeed => self.seed.clear(),
            GeneratorEdit::Relief(relief) => spec.relief = relief,
            GeneratorEdit::Water(water) => spec.water = water,
            GeneratorEdit::Woods(woods) => spec.woods = woods,
            GeneratorEdit::Rough(rough) => spec.rough = rough,
            GeneratorEdit::Fire(fire) => spec.fire = fire,
            GeneratorEdit::Rivers(rivers) => spec.rivers = rivers,
            GeneratorEdit::Frozen(frozen) => spec.frozen = frozen,
            GeneratorEdit::AddSettlement => {
                spec.settlements
                    .push(SettlementSpec::new(SettlementSize::Village));
                self.counts.push("1".into());
            }
            GeneratorEdit::RemoveSettlement(index) => {
                if index < spec.settlements.len() {
                    spec.settlements.remove(index);
                    self.counts.remove(index);
                }
            }
            GeneratorEdit::Settlement(index, edit) => {
                let Some(settlement) = spec.settlements.get_mut(index) else {
                    return false;
                };
                match edit {
                    SettlementEdit::Size(size) => settlement.size = size,
                    SettlementEdit::Kind(kind) => settlement.kind = kind,
                    SettlementEdit::Layout(layout) => settlement.layout = layout,
                    SettlementEdit::Position(position) => settlement.position = position,
                    SettlementEdit::Walled(walled) => settlement.walled = walled,
                    SettlementEdit::Count(count) => self.counts[index] = count,
                }
            }
            GeneratorEdit::ConnectSettlements(connect) => {
                roads(spec).connect_settlements = Some(connect);
            }
            GeneratorEdit::ThroughRoads(through) => roads(spec).through_roads = through,
            GeneratorEdit::RoadWidth(width) => roads(spec).width = width,
        }
        match self.read_text_fields() {
            Ok(()) => {
                self.error.clear();
                true
            }
            Err(error) => {
                self.error = error;
                false
            }
        }
    }

    /// Copy the typed width, height, seed and settlement counts into the spec, or say which
    /// one is not a valid number.
    fn read_text_fields(&mut self) -> Result<(), String> {
        let dimension = |label: &str, value: &str| {
            value
                .trim()
                .parse::<u16>()
                .ok()
                .filter(|value| (MIN_DIMENSION..=MAX_DIMENSION).contains(value))
                .ok_or_else(|| {
                    format!("{label} must be a number from {MIN_DIMENSION} to {MAX_DIMENSION}")
                })
        };
        self.spec.width = Some(dimension("Width", &self.width)?);
        self.spec.height = Some(dimension("Height", &self.height)?);
        let seed = self.seed.trim();
        self.spec.seed = match seed {
            "" => None,
            _ => Some(
                seed.parse()
                    .map_err(|_| "Seed must be a whole number, or blank for a new one")?,
            ),
        };
        for (settlement, count) in self.spec.settlements.iter_mut().zip(&self.counts) {
            let count = count
                .trim()
                .parse::<u8>()
                .ok()
                .filter(|count| (1..=MAX_SETTLEMENT_COUNT).contains(count))
                .ok_or_else(|| {
                    format!("Settlement count must be a number from 1 to {MAX_SETTLEMENT_COUNT}")
                })?;
            settlement.count = (count > 1).then_some(count);
        }
        Ok(())
    }

    /// Start a generation numbered `id` from the current choices, returning the spec to
    /// generate, unless one is running already; that one is followed by another when it
    /// finishes.
    pub fn request(&mut self, id: u64) -> Option<MapSpec> {
        if self.running.is_some() {
            self.stale = true;
            return None;
        }
        self.running = Some((id, self.spec.seed));
        self.stale = false;
        Some(self.spec.clone())
    }

    /// Take the result of generation `id`. Returns whether the choices changed while it ran,
    /// so another should start. Results of generations this panel did not start are ignored.
    pub fn finish(&mut self, id: u64, result: GenerationResult) -> bool {
        let Some((_, requested_seed)) = self.running.take_if(|(running, _)| *running == id) else {
            return false;
        };
        let generated = match result {
            Ok(generated) => generated,
            Err(error) => {
                self.error = error;
                return self.stale;
            }
        };
        let map = match generated.map.to_asset() {
            Ok(map) => map,
            Err(error) => {
                self.error = format!("{error:#}");
                return self.stale;
            }
        };
        // Keep the seed the generator picked, unless the form asked for another meanwhile.
        if requested_seed.is_none() && self.spec.seed.is_none() {
            let seed = generated.spec.seed;
            self.spec.seed = seed;
            self.seed = seed.map(|seed| seed.to_string()).unwrap_or_default();
        }
        if !self.stale {
            self.error.clear();
        }
        self.preview = Some(Preview {
            document: Document::preview(map),
            generated,
        });
        self.stale
    }

    pub fn view(&self) -> Element<'_, GeneratorEdit> {
        // Fill in the biome defaults the form names for the fields left unset.
        let defaults = self.spec.resolve().ok();
        let default = |pick: fn(&MapSpec) -> Option<u8>| defaults.as_ref().and_then(pick);
        let biome = self.spec.biome.unwrap_or(Biome::Temperate);
        let size_preset = MapSize::ALL
            .into_iter()
            .find(|size| Some(size.dimensions()) == self.spec.width.zip(self.spec.height));
        let size = row![
            pick_list(
                MapSize::ALL.map(SizeChoice),
                size_preset.map(SizeChoice),
                |size| { GeneratorEdit::Size(size.0) }
            )
            .placeholder("Custom")
            .text_size(13)
            .width(Fill),
            text_input("W", &self.width)
                .on_input(GeneratorEdit::Width)
                .size(13)
                .width(52),
            text("×").size(13),
            text_input("H", &self.height)
                .on_input(GeneratorEdit::Height)
                .size(13)
                .width(52),
        ]
        .spacing(4)
        .align_y(Alignment::Center);
        let seed = row![
            text_input("Random", &self.seed)
                .on_input(GeneratorEdit::Seed)
                .size(13)
                .width(Fill),
            button(text("New seed").size(13)).on_press(GeneratorEdit::NewSeed),
        ]
        .spacing(4)
        .align_y(Alignment::Center);
        let resolved = |pick: fn(&MapSpec) -> Option<Amount>| defaults.as_ref().and_then(pick);
        let terrain = column![
            field(
                "Relief",
                choose(
                    self.spec.relief,
                    defaults.as_ref().and_then(|spec| spec.relief),
                    Relief::ALL,
                    GeneratorEdit::Relief
                )
            ),
            field(
                "Water",
                choose(
                    self.spec.water,
                    resolved(|spec| spec.water),
                    Amount::ALL,
                    GeneratorEdit::Water
                )
            ),
            field(
                "Woods",
                choose(
                    self.spec.woods,
                    resolved(|spec| spec.woods),
                    Amount::ALL,
                    GeneratorEdit::Woods
                )
            ),
            field(
                "Rough ground",
                choose(
                    self.spec.rough,
                    resolved(|spec| spec.rough),
                    Amount::ALL,
                    GeneratorEdit::Rough
                )
            ),
            field(
                "Fire",
                choose(
                    self.spec.fire,
                    resolved(|spec| spec.fire),
                    Amount::ALL,
                    GeneratorEdit::Fire
                )
            ),
            field(
                "Rivers",
                choose(
                    self.spec.rivers,
                    default(|spec| spec.rivers),
                    0..=MAX_CROSSINGS,
                    GeneratorEdit::Rivers
                )
            ),
            field(
                "Frozen",
                choose(
                    self.spec.frozen,
                    defaults.as_ref().and_then(|spec| spec.frozen),
                    [false, true],
                    GeneratorEdit::Frozen
                )
            ),
        ]
        .spacing(6);
        let roads = self.spec.roads.clone().unwrap_or_default();
        let resolved_roads = defaults.as_ref().and_then(|spec| spec.roads.clone());
        let road_default =
            |pick: fn(&RoadSpec) -> Option<u8>| resolved_roads.as_ref().and_then(pick);
        let road_options = column![
            checkbox(roads.connect_settlements.unwrap_or(true))
                .label("Connect settlements")
                .text_size(13)
                .on_toggle(GeneratorEdit::ConnectSettlements),
            field(
                "Through roads",
                choose(
                    roads.through_roads,
                    road_default(|roads| roads.through_roads),
                    0..=MAX_CROSSINGS,
                    GeneratorEdit::ThroughRoads
                )
            ),
            field(
                "Road width",
                choose(
                    roads.width,
                    road_default(|roads| roads.width),
                    1..=3,
                    GeneratorEdit::RoadWidth
                )
            ),
        ]
        .spacing(6);
        let settlements = column(
            self.spec
                .settlements
                .iter()
                .enumerate()
                .map(|(index, settlement)| self.settlement_view(index, settlement)),
        )
        .spacing(6);
        column![
            text("Map generator").size(15),
            note(
                "Every change regenerates the preview. Apply replaces the map as one step \
                 you can undo."
            ),
            biome_choices(biome),
            note(biome.description()),
            field("Size", size),
            field("Seed", seed),
            rule::horizontal(1),
            text("Terrain").size(14),
            terrain,
            rule::horizontal(1),
            row![
                text("Settlements").size(14),
                space::horizontal(),
                button(text("Add").size(13)).on_press(GeneratorEdit::AddSettlement),
            ]
            .align_y(Alignment::Center),
            settlements,
            rule::horizontal(1),
            text("Roads").size(14),
            road_options,
            rule::horizontal(1),
            self.status_view(),
            note(
                "Gravity, temperature and map flags follow the biome. Change them after \
                 applying in Map ▸ Options."
            ),
        ]
        .spacing(8)
        .into()
    }

    /// One settlement's choices in a box with a button that removes it.
    fn settlement_view<'a>(
        &'a self,
        index: usize,
        settlement: &'a SettlementSpec,
    ) -> Element<'a, GeneratorEdit> {
        let edit = move |edit: SettlementEdit| GeneratorEdit::Settlement(index, edit);
        let size = pick_list(
            SettlementSize::ALL.map(Labelled),
            Some(Labelled(settlement.size)),
            move |size| edit(SettlementEdit::Size(size.0)),
        )
        .text_size(13)
        .width(Fill);
        let position = pick_list(
            Position::ALL.map(Labelled),
            Some(Labelled(settlement.position.unwrap_or(Position::Random))),
            move |position| {
                let position = (position.0 != Position::Random).then_some(position.0);
                edit(SettlementEdit::Position(position))
            },
        )
        .text_size(13)
        .width(Fill);
        let count = text_input("1", &self.counts[index])
            .on_input(move |count| edit(SettlementEdit::Count(count)))
            .size(13)
            .width(44);
        let choices = column![
            row![
                size,
                text("×").size(13),
                count,
                button(text("Remove").size(12))
                    .style(button::secondary)
                    .on_press(GeneratorEdit::RemoveSettlement(index)),
            ]
            .spacing(4)
            .align_y(Alignment::Center),
            field("Where", position),
            field(
                "Kind",
                choose(settlement.kind, None, SettlementKind::ALL, move |kind| {
                    edit(SettlementEdit::Kind(kind))
                })
            ),
            field(
                "Layout",
                choose(
                    settlement.layout,
                    None,
                    SettlementLayout::ALL,
                    move |layout| edit(SettlementEdit::Layout(layout))
                )
            ),
            field(
                "Walled",
                choose(settlement.walled, None, [false, true], move |walled| {
                    edit(SettlementEdit::Walled(walled))
                })
            ),
        ]
        .spacing(4);
        container(choices)
            .padding(8)
            .width(Fill)
            .style(container::bordered_box)
            .into()
    }

    /// What the last generation built, or why it failed, and whether another is running.
    fn status_view(&self) -> Element<'_, GeneratorEdit> {
        let mut lines = column![].spacing(4);
        if self.running.is_some() {
            lines = lines.push(text("Generating…").size(13));
        }
        if !self.error.is_empty() {
            lines = lines.push(text(&self.error).size(13).color(ERROR_COLOR));
        }
        let Some(preview) = &self.preview else {
            return lines.into();
        };
        let report = &preview.generated.report;
        let buildings: usize = report
            .settlements
            .iter()
            .map(|settlement| settlement.buildings)
            .sum();
        lines = lines.push(
            text(format!(
                "{} map, {}×{}, seed {}",
                report.biome.label(),
                report.width,
                report.height,
                report.seed
            ))
            .size(13),
        );
        lines = lines.push(
            text(format!(
                "{} · {} · {} · {}",
                plural(report.settlements.len(), "settlement"),
                plural(buildings, "building"),
                plural(report.roads.len(), "road"),
                plural(report.rivers, "river"),
            ))
            .size(12)
            .color(NOTE_COLOR),
        );
        for warning in &report.warnings {
            lines = lines.push(text(warning).size(12).color(WARNING_COLOR));
        }
        lines.into()
    }
}

/// The spec's road settings, created empty if it has none.
fn roads(spec: &mut MapSpec) -> &mut RoadSpec {
    spec.roads.get_or_insert_with(RoadSpec::default)
}

/// `count` followed by `noun`, made plural unless there is one.
fn plural(count: usize, noun: &str) -> String {
    match count {
        1 => format!("1 {noun}"),
        _ => format!("{count} {noun}s"),
    }
}

/// A row of biome buttons, the chosen one highlighted.
fn biome_choices<'a>(chosen: Biome) -> Element<'a, GeneratorEdit> {
    let rows = Biome::ALL.chunks(BIOMES_PER_ROW).map(|biomes| {
        let mut line = row![].spacing(4);
        for index in 0..BIOMES_PER_ROW {
            let Some(&biome) = biomes.get(index) else {
                line = line.push(space().width(Fill));
                continue;
            };
            line = line.push(
                button(text(biome.label()).size(12).center())
                    .width(Fill)
                    .padding([6, 2])
                    .style(if biome == chosen {
                        button::primary
                    } else {
                        button::secondary
                    })
                    .on_press(GeneratorEdit::Biome(biome)),
            );
        }
        line.into()
    });
    column(rows).spacing(4).into()
}

/// A labelled control.
fn field<'a>(
    label: &'a str,
    control: impl Into<Element<'a, GeneratorEdit>>,
) -> Element<'a, GeneratorEdit> {
    row![text(label).size(13).width(LABEL_WIDTH), control.into()]
        .align_y(Alignment::Center)
        .into()
}

/// Explanatory text in the panel.
fn note(content: &str) -> Element<'_, GeneratorEdit> {
    text(content).size(12).color(NOTE_COLOR).into()
}

/// A drop-down choosing one of `values`, or the default: `default` when the generator's
/// choice is known, or "Auto" when it depends on where the generator puts things.
fn choose<'a, T>(
    value: Option<T>,
    default: Option<T>,
    values: impl IntoIterator<Item = T>,
    on_choose: impl Fn(Option<T>) -> GeneratorEdit + 'a,
) -> Element<'a, GeneratorEdit>
where
    T: Label + Copy + PartialEq + 'a,
{
    let options: Vec<_> = std::iter::once(Pick::Default(default))
        .chain(values.into_iter().map(Pick::Set))
        .collect();
    let selected = value.map_or(Pick::Default(default), Pick::Set);
    pick_list(options, Some(selected), move |pick| {
        on_choose(match pick {
            Pick::Default(_) => None,
            Pick::Set(value) => Some(value),
        })
    })
    .text_size(13)
    .width(Fill)
    .into()
}

/// A value with a name to show in the form.
trait Label {
    fn label(&self) -> String;
}

impl Label for Relief {
    fn label(&self) -> String {
        Relief::label(*self).into()
    }
}

impl Label for Amount {
    fn label(&self) -> String {
        Amount::label(*self).into()
    }
}

impl Label for SettlementSize {
    fn label(&self) -> String {
        SettlementSize::label(*self).into()
    }
}

impl Label for SettlementKind {
    fn label(&self) -> String {
        SettlementKind::label(*self).into()
    }
}

impl Label for SettlementLayout {
    fn label(&self) -> String {
        SettlementLayout::label(*self).into()
    }
}

impl Label for Position {
    fn label(&self) -> String {
        Position::label(*self).into()
    }
}

impl Label for u8 {
    fn label(&self) -> String {
        self.to_string()
    }
}

impl Label for bool {
    fn label(&self) -> String {
        if *self { "Yes" } else { "No" }.into()
    }
}

/// A drop-down entry: the default, or a chosen value.
#[derive(Debug, Clone, Copy, PartialEq)]
enum Pick<T> {
    /// Leave the field to the generator, which picks this value when it is known.
    Default(Option<T>),
    Set(T),
}

impl<T: Label> fmt::Display for Pick<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Default(Some(value)) => write!(formatter, "Default ({})", value.label()),
            Self::Default(None) => formatter.write_str("Auto"),
            Self::Set(value) => formatter.write_str(&value.label()),
        }
    }
}

/// A drop-down entry for a value with a label and no default.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Labelled<T>(T);

impl<T: Label> fmt::Display for Labelled<T> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0.label())
    }
}

/// A map size preset in the size drop-down, with its dimensions.
#[derive(Debug, Clone, Copy, PartialEq)]
struct SizeChoice(MapSize);

impl fmt::Display for SizeChoice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (width, height) = self.0.dimensions();
        write!(formatter, "{} {width}×{height}", self.0.label())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A generation of `spec` as the panel would receive it.
    fn generated(spec: &MapSpec) -> GenerationResult {
        stompymux_mapgen::generate(spec)
            .map(Arc::new)
            .map_err(|error| format!("{error:#}"))
    }

    #[test]
    fn new_forms_start_at_the_map_size_or_the_generated_spec() {
        let panel = GeneratorPanel::new(None, 40, 4);
        assert_eq!((panel.spec.width, panel.spec.height), (Some(40), Some(8)));
        assert_eq!(panel.spec.seed, None);
        let spec = MapSpec {
            seed: Some(9),
            biome: Some(Biome::Desert),
            size: Some(MapSize::Small),
            settlements: vec![SettlementSpec {
                count: Some(3),
                ..SettlementSpec::new(SettlementSize::Town)
            }],
            ..MapSpec::default()
        };
        let panel = GeneratorPanel::new(Some(&spec), 40, 40);
        assert_eq!((panel.spec.width, panel.spec.height), (Some(30), Some(30)));
        assert_eq!(panel.seed, "9");
        assert_eq!(panel.counts, ["3"]);
    }

    #[test]
    fn text_fields_must_hold_valid_numbers() {
        let mut panel = GeneratorPanel::new(None, 30, 30);
        assert!(!panel.edit(GeneratorEdit::Width("4".into())));
        assert!(panel.error.contains("Width"), "{}", panel.error);
        assert!(panel.edit(GeneratorEdit::Width("60".into())));
        assert_eq!(panel.spec.width, Some(60));
        assert!(panel.error.is_empty());
        assert!(!panel.edit(GeneratorEdit::Seed("abc".into())));
        assert!(panel.edit(GeneratorEdit::Seed(" 42 ".into())));
        assert_eq!(panel.spec.seed, Some(42));
        assert!(panel.edit(GeneratorEdit::AddSettlement));
        assert!(!panel.edit(GeneratorEdit::Settlement(
            0,
            SettlementEdit::Count("0".into())
        )));
        assert!(panel.edit(GeneratorEdit::Settlement(
            0,
            SettlementEdit::Count("4".into())
        )));
        assert_eq!(panel.spec.settlements[0].count, Some(4));
        assert!(panel.edit(GeneratorEdit::RemoveSettlement(0)));
        assert!(panel.spec.settlements.is_empty() && panel.counts.is_empty());
    }

    #[test]
    fn size_presets_set_both_dimensions() {
        let mut panel = GeneratorPanel::new(None, 30, 30);
        assert!(panel.edit(GeneratorEdit::Size(MapSize::Large)));
        assert_eq!((panel.spec.width, panel.spec.height), (Some(80), Some(80)));
        assert_eq!((panel.width.as_str(), panel.height.as_str()), ("80", "80"));
    }

    /// A change made while a generation runs waits for it, then asks for one more.
    #[test]
    fn changes_during_a_generation_queue_one_more() {
        let mut panel = GeneratorPanel::new(None, 16, 16);
        let spec = panel.request(1).unwrap();
        assert!(panel.running.is_some());
        panel.edit(GeneratorEdit::Biome(Biome::Lunar));
        assert!(panel.request(2).is_none());
        assert!(panel.finish(1, generated(&spec)), "another should follow");
        assert!(panel.ready().is_none(), "the preview is out of date");
        let spec = panel.request(3).unwrap();
        assert_eq!(spec.biome, Some(Biome::Lunar));
        assert!(!panel.finish(3, generated(&spec)));
        let preview = panel.ready().unwrap();
        assert_eq!(preview.generated.spec.biome, Some(Biome::Lunar));
        assert_eq!(preview.document.map.width, 16);
    }

    /// The seed the generator picks is kept, unless New seed was pressed meanwhile.
    #[test]
    fn picked_seeds_fill_the_form() {
        let mut panel = GeneratorPanel::new(None, 16, 16);
        let spec = panel.request(1).unwrap();
        panel.finish(1, generated(&spec));
        let seed = panel.spec.seed.expect("the picked seed is kept");
        assert_eq!(panel.seed, seed.to_string());
        let spec = panel.request(2).unwrap();
        assert_eq!(spec.seed, Some(seed));
        panel.edit(GeneratorEdit::NewSeed);
        panel.finish(2, generated(&spec));
        assert_eq!(
            panel.spec.seed, None,
            "New seed is not undone by a late result"
        );
    }

    #[test]
    fn stale_and_failed_results_are_ignored_or_reported() {
        let mut panel = GeneratorPanel::new(None, 16, 16);
        assert!(!panel.finish(7, Err("not ours".into())));
        assert!(panel.error.is_empty());
        panel.request(8);
        assert!(!panel.finish(8, Err("boom".into())));
        assert_eq!(panel.error, "boom");
        assert!(panel.ready().is_none());
    }
}
