//! The map being edited: layer and settings changes with undo/redo, loading and saving.
//!
//! Every change is recorded as an [`Edit`] holding before and after values, so undo and redo
//! replay edits in either direction. Brush strokes accumulate into one edit until the stroke
//! ends, so a drag across many hexes undoes in one step; a settings slider drag does the same.
//!
//! Each [`Paint`] changes one layer of a hex, clearing whatever the new layer cannot stand
//! with, and never produces a hex that fails [`Hex::validate`]: a paint that would is skipped
//! for that hex. Since a map file stores every layer of a valid hex, whatever the brushes make
//! can be saved.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

use anyhow::{Context, Result, ensure};
use stompymux_map::{
    Condition, DecorationKind, Foliage, Ground, Hex, HexCoordinate, Light, MapAsset, Route,
    Structure, Water, Wind,
};

/// A map's battlefield-wide settings, edited together: rule flags, gravity, temperature, and
/// the optional light, visibility and wind, where `None` keeps a live map's own value when the
/// map is reloaded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapSettings {
    pub flags: i32,
    /// Percent of standard gravity.
    pub gravity: u8,
    /// Degrees Celsius.
    pub temperature: i8,
    pub light: Option<Light>,
    /// Weather visibility in hexes, up to [`stompymux_map::MAX_VISIBILITY`].
    pub visibility: Option<u8>,
    pub wind: Option<Wind>,
}

/// One hex's value before and after a change, by row-major index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct HexChange {
    index: usize,
    before: Hex,
    after: Hex,
}

/// One undoable change.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Edit {
    Hexes(Vec<HexChange>),
    Settings {
        before: MapSettings,
        after: MapSettings,
    },
}

/// What a brush does to each hex it touches. Level and the fire or smoke overlay carry over
/// every other paint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paint {
    /// Set the ground height, keeping everything on the ground.
    Level(u8),
    /// Replace the ground or water; see [`Paint::apply`] for what each clears.
    Terrain(TerrainFeature),
    /// Grow or clear foliage, where the ground supports it and no building or wall stands.
    Foliage(Option<Foliage>),
    /// Lay or remove a road or rail line, on dry hexes without a building or wall.
    Route(Option<Route>),
    /// Build or remove a structure. Buildings and walls drain water and clear foliage and
    /// routes; a bridge brings still water one level deep to a dry hex. Removing a structure
    /// leaves the terrain under it.
    Structure(Option<Structure>),
    /// Lay or clear ice, snow or mud.
    Condition(Option<Condition>),
    /// Start or put out fire or smoke.
    Overlay(Option<DecorationKind>),
}

/// A hex's base terrain: a kind of ground, or water.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainFeature {
    Ground(Ground),
    Water(Water),
}

impl TerrainFeature {
    /// The base terrain under `hex`'s foliage, route, structure and conditions.
    pub fn of(hex: Hex) -> Self {
        match hex.water() {
            Some(water) => Self::Water(water),
            None => Self::Ground(hex.ground()),
        }
    }
}

impl Paint {
    /// The hex this paint turns `hex` into, or `hex` unchanged when the result would not be a
    /// valid hex.
    pub fn apply(self, hex: Hex) -> Hex {
        let painted = self.paint(hex);
        if painted.validate().is_err() {
            return hex;
        }
        painted
    }

    /// The hex this paint makes of `hex`, valid or not.
    fn paint(self, hex: Hex) -> Hex {
        match self {
            Self::Level(level) => hex.with_level(level),
            Self::Terrain(TerrainFeature::Ground(ground)) => paint_ground(hex, ground),
            Self::Terrain(TerrainFeature::Water(water)) => flood(hex, water),
            Self::Foliage(foliage) => hex.with_foliage(foliage),
            Self::Route(route) => hex.with_route(route),
            Self::Structure(None) => hex.with_structure(None),
            Self::Structure(Some(bridge)) if bridge.is_bridge() => {
                let wet = match hex.water() {
                    Some(_) => hex,
                    None => flood(hex, Water::still(1)),
                };
                wet.with_structure(Some(bridge))
            }
            Self::Structure(Some(structure)) => hex
                .with_water(None)
                .with_foliage(None)
                .with_route(None)
                .with_structure(Some(structure)),
            Self::Condition(condition) => hex.with_condition(condition),
            Self::Overlay(overlay) => hex.with_overlay(overlay),
        }
    }
}

/// `hex` with its ground made of `ground`: any water drains, taking a bridge with it, and
/// whatever cannot lie on the new ground goes: foliage on pavement, heavy industry and magma,
/// and routes and conditions on magma.
fn paint_ground(hex: Hex, ground: Ground) -> Hex {
    let mut hex = hex.with_ground(ground);
    if hex.water().is_some() {
        hex = hex.with_water(None);
        if hex.has_bridge() {
            hex = hex.with_structure(None);
        }
    }
    if !ground.supports_foliage() {
        hex = hex.with_foliage(None);
    }
    if ground == Ground::Magma {
        hex = hex.with_route(None).with_condition(None);
    }
    hex
}

/// `hex` under `water`: clear ground beneath, no foliage, route, building or wall, a bridge
/// kept, and ice kept as the water's frozen surface while snow and mud wash away.
fn flood(hex: Hex, water: Water) -> Hex {
    let structure = hex.structure().filter(|structure| structure.is_bridge());
    let condition = hex
        .condition()
        .filter(|condition| *condition == Condition::Ice);
    hex.with_ground(Ground::Clear)
        .with_water(Some(water))
        .with_foliage(None)
        .with_route(None)
        .with_structure(structure)
        .with_condition(condition)
}

/// A paint and the area it covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Brush {
    pub paint: Paint,
    /// Hexes within this many steps of the center are painted.
    pub radius: u8,
}

/// An open map with its edit history.
pub struct Document {
    /// File the map was loaded from or last saved to; `None` for a new map.
    pub path: Option<PathBuf>,
    pub map: MapAsset,
    /// Whether the map differs from the file at `path`.
    pub dirty: bool,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// Changes made by the brush stroke in progress, by hex index.
    stroke: BTreeMap<usize, HexChange>,
    /// Settings from before the settings drag in progress, if one is.
    settings_drag: Option<MapSettings>,
    /// Names the hex contents that `changes` starts from; see [`HexFeed`].
    hexes_id: u64,
    /// Index of every hex written since `hexes_id` was assigned, in order.
    changes: Arc<Vec<u32>>,
}

/// What a renderer needs to keep its own copy of the hexes current without cloning them.
///
/// A renderer that has applied the first `n` entries of `changes` on top of the hexes as they
/// were when `id` was assigned only needs to reread the hexes named by the entries after `n`.
/// When `id` differs from the one it last saw, it rereads everything. The references are weak
/// so that a renderer holding one between frames never makes the next edit clone the map.
#[derive(Debug, Clone)]
pub struct HexFeed {
    pub id: u64,
    pub hexes: Weak<Vec<Hex>>,
    pub changes: Weak<Vec<u32>>,
}

/// Source of hex content ids, shared by every document so two never report the same one.
static NEXT_HEXES_ID: AtomicU64 = AtomicU64::new(1);

/// A hex content id no document has used.
fn next_hexes_id() -> u64 {
    NEXT_HEXES_ID.fetch_add(1, Ordering::Relaxed)
}

impl Document {
    /// A blank map of clear ground at level zero, keeping a live map's light, visibility and
    /// wind.
    pub fn new(width: u16, height: u16) -> Result<Self> {
        ensure!(
            (1..=1000).contains(&width) && (1..=1000).contains(&height),
            "map dimensions must be between 1 and 1000"
        );
        let hexes = vec![Hex::at_level(0); usize::from(width) * usize::from(height)];
        Ok(Self::from_map(
            None,
            MapAsset {
                width,
                height,
                flags: 0,
                gravity: 100,
                temperature: 20,
                light: None,
                visibility: None,
                wind: None,
                hexes: Arc::new(hexes),
                points_of_interest: Vec::new(),
            },
        ))
    }

    /// Load and decode a map file.
    pub fn open(path: &Path) -> Result<Self> {
        let source =
            std::fs::read_to_string(path).with_context(|| format!("reading {}", path.display()))?;
        let map =
            MapAsset::parse(&source).with_context(|| format!("decoding {}", path.display()))?;
        Ok(Self::from_map(Some(path.to_path_buf()), map))
    }

    fn from_map(path: Option<PathBuf>, map: MapAsset) -> Self {
        Self {
            path,
            map,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            stroke: BTreeMap::new(),
            settings_drag: None,
            hexes_id: next_hexes_id(),
            changes: Arc::default(),
        }
    }

    /// Write the map to `path` after checking that the file will load back as this map.
    pub fn save_to(&mut self, path: &Path) -> Result<()> {
        self.end_stroke();
        let source = self.map.to_file()?;
        let decoded = MapAsset::parse(&source).context("the saved map would not load")?;
        ensure!(
            decoded == self.map,
            "the saved map would not load back unchanged"
        );
        std::fs::write(path, &source).with_context(|| format!("writing {}", path.display()))?;
        self.path = Some(path.to_path_buf());
        self.dirty = false;
        Ok(())
    }

    /// The hex at a coordinate, or `None` off the map.
    pub fn hex(&self, coordinate: HexCoordinate) -> Option<Hex> {
        self.map.hex(coordinate.x, coordinate.y)
    }

    /// Current battlefield-wide settings.
    pub fn settings(&self) -> MapSettings {
        MapSettings {
            flags: self.map.flags,
            gravity: self.map.gravity,
            temperature: self.map.temperature,
            light: self.map.light,
            visibility: self.map.visibility,
            wind: self.map.wind,
        }
    }

    /// Paint `brush` centered on `center` as part of the current stroke.
    pub fn paint(&mut self, center: HexCoordinate, brush: Brush) {
        self.paint_with(center, brush.radius, |hex| brush.paint.apply(hex));
    }

    /// Replace each hex within `radius` steps of `center` with `apply` of it, as part of the
    /// current stroke.
    pub fn paint_with(&mut self, center: HexCoordinate, radius: u8, apply: impl Fn(Hex) -> Hex) {
        self.end_settings_drag();
        let steps = u64::from(radius);
        let radius = i32::from(radius);
        let width = i32::from(self.map.width);
        let height = i32::from(self.map.height);
        for y in (center.y - radius).max(0)..=(center.y + radius).min(height - 1) {
            for x in (center.x - radius).max(0)..=(center.x + radius).min(width - 1) {
                let coordinate = HexCoordinate { x, y };
                if coordinate.distance(center) > steps {
                    continue;
                }
                let index = y as usize * self.map.width as usize + x as usize;
                let before = self.map.hexes[index];
                let after = apply(before);
                if before == after {
                    continue;
                }
                self.set_hex(index, after);
                self.stroke
                    .entry(index)
                    .and_modify(|change| change.after = after)
                    .or_insert(HexChange {
                        index,
                        before,
                        after,
                    });
            }
        }
    }

    /// Close the current brush stroke or settings drag into one undoable edit.
    pub fn end_stroke(&mut self) {
        self.end_settings_drag();
        let changes: Vec<_> = std::mem::take(&mut self.stroke)
            .into_values()
            .filter(|change| change.before != change.after)
            .collect();
        if changes.is_empty() {
            return;
        }
        self.record(Edit::Hexes(changes));
    }

    /// Replace the settings as one undoable edit.
    pub fn set_settings(&mut self, after: MapSettings) {
        self.end_stroke();
        let before = self.settings();
        if before == after {
            return;
        }
        self.apply(&Edit::Settings { before, after }, true);
        self.record(Edit::Settings { before, after });
    }

    /// Replace the settings as part of a drag, such as a slider being moved, which
    /// [`Document::end_stroke`] closes into one undoable edit.
    pub fn drag_settings(&mut self, after: MapSettings) {
        let before = self.settings();
        if self.settings_drag.is_none() {
            self.end_stroke();
            self.settings_drag = Some(before);
        }
        self.apply(&Edit::Settings { before, after }, true);
    }

    /// Record the settings drag in progress, if it changed anything.
    fn end_settings_drag(&mut self) {
        let Some(before) = self.settings_drag.take() else {
            return;
        };
        let after = self.settings();
        if before == after {
            return;
        }
        self.record(Edit::Settings { before, after });
    }

    /// Whether there is an edit to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
            || !self.stroke.is_empty()
            || self
                .settings_drag
                .is_some_and(|before| before != self.settings())
    }

    /// Whether there is an undone edit to redo.
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    /// Revert the most recent edit.
    pub fn undo(&mut self) {
        self.end_stroke();
        let Some(edit) = self.undo.pop() else {
            return;
        };
        self.apply(&edit, false);
        self.redo.push(edit);
        self.dirty = true;
    }

    /// Reapply the most recently undone edit.
    pub fn redo(&mut self) {
        self.end_stroke();
        let Some(edit) = self.redo.pop() else {
            return;
        };
        self.apply(&edit, true);
        self.undo.push(edit);
        self.dirty = true;
    }

    /// Push a finished edit, which invalidates anything that could be redone.
    fn record(&mut self, edit: Edit) {
        self.undo.push(edit);
        self.redo.clear();
        self.dirty = true;
    }

    /// The hexes and their change log, for a renderer to follow; see [`HexFeed`].
    pub fn hex_feed(&self) -> HexFeed {
        HexFeed {
            id: self.hexes_id,
            hexes: Arc::downgrade(&self.map.hexes),
            changes: Arc::downgrade(&self.changes),
        }
    }

    /// Write one hex and log the change.
    ///
    /// Only weak references are handed out, so `make_mut` never clones. Once the log is as
    /// long as the map, rereading every hex costs a renderer no more than replaying the log,
    /// so it starts over under a new id instead of growing without bound.
    fn set_hex(&mut self, index: usize, hex: Hex) {
        if self.changes.len() >= self.map.hexes.len() {
            self.hexes_id = next_hexes_id();
            self.changes = Arc::default();
        }
        Arc::make_mut(&mut self.map.hexes)[index] = hex;
        Arc::make_mut(&mut self.changes).push(index as u32);
    }

    /// Set the map to an edit's after (`forward`) or before values.
    fn apply(&mut self, edit: &Edit, forward: bool) {
        match edit {
            Edit::Hexes(changes) => {
                for change in changes {
                    let hex = if forward { change.after } else { change.before };
                    self.set_hex(change.index, hex);
                }
            }
            Edit::Settings { before, after } => {
                let settings = if forward { after } else { before };
                self.map.flags = settings.flags;
                self.map.gravity = settings.gravity;
                self.map.temperature = settings.temperature;
                self.map.light = settings.light;
                self.map.visibility = settings.visibility;
                self.map.wind = settings.wind;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stompymux_map::{ConstructionClass, Flow, StructureKind, Terrain};

    const AT: HexCoordinate = HexCoordinate { x: 2, y: 2 };
    const MIDDLE: HexCoordinate = HexCoordinate { x: 1, y: 1 };

    /// Paint the hex the compact symbol-and-digit notation describes over every hex within
    /// `radius` of `center`.
    fn place(
        document: &mut Document,
        center: HexCoordinate,
        terrain: Terrain,
        value: u8,
        radius: u8,
    ) {
        document.paint_with(center, radius, |_| Hex::new(terrain, value));
    }

    /// A brush of `paint` covering one hex.
    fn brush(paint: Paint) -> Brush {
        Brush { paint, radius: 0 }
    }

    /// Check each `(before, paint, after)` case, and that every result is a valid hex.
    fn check(cases: &[(Hex, Paint, Hex)]) {
        for &(before, paint, after) in cases {
            let painted = paint.apply(before);
            assert_eq!(painted, after, "{paint:?} over {before:?}");
            painted
                .validate()
                .unwrap_or_else(|error| panic!("{paint:?} over {before:?}: {error}"));
        }
    }

    /// Clear ground at level 2 with the given layers.
    fn dry() -> Hex {
        Hex::at_level(2)
    }

    /// A drag across many hexes undoes and redoes as one step.
    #[test]
    fn strokes_undo_and_redo_as_one_edit() {
        let mut document = Document::new(5, 5).unwrap();
        let blank = document.map.clone();
        place(&mut document, AT, Terrain::Water, 2, 0);
        place(
            &mut document,
            HexCoordinate { x: 3, y: 2 },
            Terrain::Water,
            2,
            0,
        );
        place(&mut document, AT, Terrain::Road, 1, 0);
        document.end_stroke();
        let painted = document.map.clone();
        assert_eq!(document.hex(AT), Some(Hex::new(Terrain::Road, 1)));
        document.undo();
        assert_eq!(document.map, blank);
        document.redo();
        assert_eq!(document.map, painted);
        assert!(!document.can_redo());
    }

    /// A radius-one brush paints the center and its six neighbors, clipped to the map.
    #[test]
    fn brush_radius_follows_hex_distance_and_map_bounds() {
        let rough = |document: &Document| {
            document
                .map
                .hexes
                .iter()
                .filter(|hex| hex.terrain() == Terrain::Rough)
                .count()
        };
        let mut document = Document::new(5, 5).unwrap();
        place(&mut document, AT, Terrain::Rough, 0, 1);
        assert_eq!(rough(&document), 7);
        let mut corner = Document::new(5, 5).unwrap();
        place(
            &mut corner,
            HexCoordinate { x: 0, y: 0 },
            Terrain::Rough,
            0,
            1,
        );
        assert!(rough(&corner) < 7);
    }

    /// Level and overlay survive every other paint, and each paint changes its own layer.
    #[test]
    fn paints_change_only_their_layers() {
        let mut document = Document::new(3, 3).unwrap();
        place(&mut document, MIDDLE, Terrain::HeavyWoods, 3, 0);
        document.paint(MIDDLE, brush(Paint::Level(20)));
        let woods = Hex::new(Terrain::HeavyWoods, 20);
        assert_eq!(document.hex(MIDDLE), Some(woods));
        let smoke = Some(DecorationKind::Smoke);
        document.paint(MIDDLE, brush(Paint::Overlay(smoke)));
        assert_eq!(document.hex(MIDDLE), Some(woods.with_overlay(smoke)));
        document.paint(
            MIDDLE,
            brush(Paint::Terrain(TerrainFeature::Ground(Ground::Rough))),
        );
        assert_eq!(
            document.hex(MIDDLE),
            Some(woods.with_ground(Ground::Rough).with_overlay(smoke))
        );
        document.paint(MIDDLE, brush(Paint::Route(Some(Route::DirtRoad))));
        document.paint(MIDDLE, brush(Paint::Condition(Some(Condition::Mud))));
        assert_eq!(
            document.hex(MIDDLE),
            Some(
                woods
                    .with_ground(Ground::Rough)
                    .with_route(Some(Route::DirtRoad))
                    .with_condition(Some(Condition::Mud))
                    .with_overlay(smoke)
            )
        );
    }

    /// Ground drains water and its bridge, and clears what cannot lie on it; water clears
    /// what cannot stand in it but keeps bridges and ice.
    #[test]
    fn terrain_replaces_the_base_and_what_cannot_stand_on_it() {
        let ground = |ground| Paint::Terrain(TerrainFeature::Ground(ground));
        let water = |depth, flow| Paint::Terrain(TerrainFeature::Water(Water { depth, flow }));
        let woods = dry().with_foliage(Some(Foliage::LightWoods));
        let road = dry().with_route(Some(Route::PavedRoad));
        let snowy = dry().with_condition(Some(Condition::DeepSnow));
        let bridge = Hex::new(Terrain::Bridge, 4);
        let ice = Hex::new(Terrain::Ice, 1);
        let building = Hex::new(Terrain::Building, 9);
        let deep = Water::still(3);
        check(&[
            (
                woods,
                ground(Ground::Rough),
                woods.with_ground(Ground::Rough),
            ),
            (
                woods,
                ground(Ground::Pavement),
                dry().with_ground(Ground::Pavement),
            ),
            (
                woods,
                ground(Ground::HeavyIndustrial),
                dry().with_ground(Ground::HeavyIndustrial),
            ),
            (
                road.with_foliage(Some(Foliage::PlantedFields))
                    .with_condition(Some(Condition::ThinSnow)),
                ground(Ground::Magma),
                dry().with_ground(Ground::Magma),
            ),
            (snowy, ground(Ground::Sand), snowy.with_ground(Ground::Sand)),
            (
                bridge,
                ground(Ground::Rough),
                Hex::at_level(0).with_ground(Ground::Rough),
            ),
            (
                ice,
                ground(Ground::Tundra),
                Hex::at_level(0)
                    .with_ground(Ground::Tundra)
                    .with_condition(Some(Condition::Ice)),
            ),
            (
                building,
                ground(Ground::Rubble),
                building.with_ground(Ground::Rubble),
            ),
            (
                woods
                    .with_route(Some(Route::Rail))
                    .with_ground(Ground::Swamp),
                water(3, Flow::Still),
                dry().with_water(Some(deep)),
            ),
            (building, water(3, Flow::Still), Hex::new(Terrain::Water, 3)),
            (snowy, water(3, Flow::Still), dry().with_water(Some(deep))),
            (
                ice,
                water(2, Flow::Torrent),
                ice.with_water(Some(Water {
                    depth: 2,
                    flow: Flow::Torrent,
                })),
            ),
            (
                bridge,
                water(3, Flow::Rapids),
                bridge.with_water(Some(Water {
                    depth: 3,
                    flow: Flow::Rapids,
                })),
            ),
            (dry(), water(0, Flow::Rapids), dry()),
        ]);
    }

    /// Foliage only grows on dry ground that supports it with no building or wall.
    #[test]
    fn foliage_grows_only_where_it_can() {
        let jungle = Paint::Foliage(Some(Foliage::HeavyJungle));
        let grown = |hex: Hex| hex.with_foliage(Some(Foliage::HeavyJungle));
        let rough = dry().with_ground(Ground::Rough);
        let road = dry().with_route(Some(Route::GravelRoad));
        let pavement = dry().with_ground(Ground::Pavement);
        let water = Hex::new(Terrain::Water, 2);
        let wall = Hex::new(Terrain::Wall, 3);
        let woods = grown(rough);
        check(&[
            (rough, jungle, grown(rough)),
            (road, jungle, grown(road)),
            (pavement, jungle, pavement),
            (water, jungle, water),
            (wall, jungle, wall),
            (woods, Paint::Foliage(None), rough),
        ]);
    }

    /// Routes run only through dry hexes without a building or wall, and not over magma.
    #[test]
    fn routes_run_only_over_dry_open_hexes() {
        let rail = Paint::Route(Some(Route::Rail));
        let laid = |hex: Hex| hex.with_route(Some(Route::Rail));
        let woods = Hex::new(Terrain::UltraHeavyWoods, 1);
        let magma = dry().with_ground(Ground::Magma);
        let bridge = Hex::new(Terrain::Bridge, 2);
        let building = Hex::new(Terrain::Building, 2);
        check(&[
            (woods, rail, laid(woods)),
            (magma, rail, magma),
            (bridge, rail, bridge),
            (building, rail, building),
            (laid(woods), Paint::Route(None), woods),
        ]);
    }

    /// Buildings and walls drain water and clear foliage and routes, bridges bring water to
    /// dry hexes, and removing a structure leaves the terrain.
    #[test]
    fn structures_clear_what_they_cannot_stand_with() {
        let heavy = |kind, height| Structure::new(kind, height, ConstructionClass::Heavy);
        let building = Paint::Structure(Some(heavy(StructureKind::Building, 5)));
        let bridge = Paint::Structure(Some(heavy(StructureKind::Bridge, 2)));
        let woods = dry()
            .with_ground(Ground::Rough)
            .with_foliage(Some(Foliage::LightWoods))
            .with_route(Some(Route::DirtRoad));
        let deep = Hex::new(Terrain::Ice, 4);
        let spanned = deep.with_structure(Some(heavy(StructureKind::Bridge, 2)));
        check(&[
            (
                woods,
                building,
                dry()
                    .with_ground(Ground::Rough)
                    .with_structure(Some(heavy(StructureKind::Building, 5))),
            ),
            (
                deep,
                building,
                Hex::at_level(0)
                    .with_condition(Some(Condition::Ice))
                    .with_structure(Some(heavy(StructureKind::Building, 5))),
            ),
            (
                woods,
                bridge,
                dry()
                    .with_water(Some(Water::still(1)))
                    .with_structure(Some(heavy(StructureKind::Bridge, 2))),
            ),
            (deep, bridge, spanned),
            (spanned, Paint::Structure(None), deep),
            (woods, Paint::Structure(None), woods),
        ]);
    }

    /// Snow and mud lie only on dry ground that is not magma; ice also freezes water.
    #[test]
    fn conditions_lie_only_where_they_fit() {
        let condition = |condition| Paint::Condition(Some(condition));
        let water = Hex::new(Terrain::Water, 2);
        let magma = dry().with_ground(Ground::Magma);
        let roof = Hex::new(Terrain::Building, 3);
        check(&[
            (
                dry(),
                condition(Condition::Mud),
                dry().with_condition(Some(Condition::Mud)),
            ),
            (water, condition(Condition::DeepSnow), water),
            (magma, condition(Condition::ThinSnow), magma),
            (magma, condition(Condition::Ice), magma),
            (water, condition(Condition::Ice), Hex::new(Terrain::Ice, 2)),
            (
                roof,
                condition(Condition::ThinSnow),
                roof.with_condition(Some(Condition::ThinSnow)),
            ),
            (Hex::new(Terrain::Ice, 2), Paint::Condition(None), water),
        ]);
    }

    /// Settings changes are undoable and a new edit clears the redo history.
    #[test]
    fn settings_edits_undo_and_clear_redo() {
        let mut document = Document::new(2, 2).unwrap();
        let original = document.settings();
        let changed = MapSettings {
            flags: 2,
            gravity: 50,
            temperature: -40,
            light: Some(Light::Night),
            visibility: Some(12),
            wind: Some(Wind {
                direction: 270,
                speed: 15,
            }),
        };
        document.set_settings(changed);
        assert_eq!(document.settings(), changed);
        document.undo();
        assert_eq!(document.settings(), original);
        assert!(document.can_redo());
        place(&mut document, MIDDLE, Terrain::Sand, 0, 0);
        document.end_stroke();
        assert!(!document.can_redo());
    }

    /// A settings drag, like a slider being moved, undoes as one edit.
    #[test]
    fn settings_drags_undo_as_one_edit() {
        let mut document = Document::new(2, 2).unwrap();
        let original = document.settings();
        for gravity in [90, 80, 70] {
            document.drag_settings(MapSettings {
                gravity,
                ..document.settings()
            });
        }
        assert!(document.can_undo());
        document.end_stroke();
        assert_eq!(document.settings().gravity, 70);
        document.undo();
        assert_eq!(document.settings(), original);
        assert!(!document.can_undo());
    }

    /// The feed logs each written hex, and a renderer holding it never makes edits copy the map.
    #[test]
    fn hex_feed_logs_changes_without_copying_the_map() {
        let mut document = Document::new(3, 3).unwrap();
        assert_ne!(
            document.hex_feed().id,
            Document::new(3, 3).unwrap().hex_feed().id
        );
        let feed = document.hex_feed();
        let buffer = document.map.hexes.as_ptr();
        place(&mut document, MIDDLE, Terrain::Clear, 0, 0);
        place(&mut document, MIDDLE, Terrain::Rough, 0, 0);
        assert_eq!(document.map.hexes.as_ptr(), buffer);
        let after = document.hex_feed();
        assert_eq!(after.id, feed.id);
        assert_eq!(*after.changes.upgrade().unwrap(), [4]);
        document.end_stroke();
        document.undo();
        assert_eq!(*document.hex_feed().changes.upgrade().unwrap(), [4, 4]);
    }

    /// Once the log is as long as the map it restarts under a new id.
    #[test]
    fn hex_feed_restarts_when_the_log_outgrows_the_map() {
        let mut document = Document::new(3, 3).unwrap();
        let id = document.hex_feed().id;
        for _ in 0..5 {
            place(&mut document, MIDDLE, Terrain::Rough, 0, 1);
            place(&mut document, MIDDLE, Terrain::Sand, 0, 1);
        }
        let feed = document.hex_feed();
        assert_ne!(feed.id, id);
        assert!(feed.changes.upgrade().unwrap().len() < 9);
    }

    /// Saved maps, every layer and setting included, load back unchanged and the document is
    /// no longer dirty.
    #[test]
    fn saved_maps_reload_identically() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.toml");
        let mut document = Document::new(4, 3).unwrap();
        place(&mut document, MIDDLE, Terrain::Fire, 1, 1);
        let layered = Hex::at_level(3)
            .with_ground(Ground::Rough)
            .with_foliage(Some(Foliage::HeavyJungle))
            .with_route(Some(Route::DirtRoad))
            .with_condition(Some(Condition::ThinSnow));
        document.paint_with(HexCoordinate { x: 3, y: 0 }, 0, |_| layered);
        let rapids = Hex::at_level(1)
            .with_water(Some(Water {
                depth: 2,
                flow: Flow::Rapids,
            }))
            .with_condition(Some(Condition::Ice))
            .with_structure(Some(Structure::new(
                StructureKind::Bridge,
                3,
                ConstructionClass::Hardened,
            )));
        document.paint_with(HexCoordinate { x: 3, y: 2 }, 0, |_| rapids);
        document.set_settings(MapSettings {
            light: Some(Light::Twilight),
            visibility: Some(30),
            wind: Some(Wind {
                direction: 90,
                speed: 10,
            }),
            ..document.settings()
        });
        document.save_to(&path).unwrap();
        assert!(!document.dirty);
        let reloaded = Document::open(&path).unwrap();
        assert_eq!(reloaded.map, document.map);
    }
}
