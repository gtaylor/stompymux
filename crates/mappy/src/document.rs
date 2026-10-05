//! The map being edited: layer and settings changes with undo/redo, loading and saving.
//!
//! Every change is recorded as an [`Edit`] holding before and after values, so undo and redo
//! replay edits in either direction. Brush strokes accumulate into one edit until the stroke
//! ends, so a drag across many hexes undoes in one step.
//!
//! Brushes keep hexes within what the map file format can store, but maps loaded from
//! elsewhere may not be: the file keeps one feature per hex, so woods on rough ground, or a
//! bridge with no water under it, would not survive a save. [`file_holds`] asks the game's own
//! encoder and decoder, the document keeps the set of hexes that fail, and saving refuses
//! while any do.
use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
    sync::{
        Arc, LazyLock, Mutex, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

use anyhow::{Context, Result, ensure};
use stompymux_map::{
    DecorationKind, Ground, Hex, HexCoordinate, MapAsset, Structure, Water, Woods,
};

/// A map's flags, gravity and temperature, edited together.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MapSettings {
    pub flags: i32,
    pub gravity: u8,
    pub temperature: i8,
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

/// What a brush does to each hex it touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Paint {
    /// Set the ground height, keeping everything on the ground.
    Level(u8),
    /// Replace the hex's base terrain. Woods and snow stay on clear ground and ice stays on
    /// water; other terrain clears them. A map file keeps one feature per hex, so this also
    /// knocks down buildings and walls, and bridges unless the new terrain is water.
    Terrain(TerrainFeature),
    /// Lay a cover over the base terrain, or clear it with `None`; see [`Cover`].
    Cover(Option<Cover>),
    /// Build or remove a structure. Buildings and walls stand on clear ground, so building one
    /// clears the hex's terrain; a bridge spans water, so it floods a dry hex one level deep.
    /// Removing a structure leaves the terrain under it.
    Structure(Option<Structure>),
    /// Start or put out fire or smoke.
    Overlay(Option<DecorationKind>),
}

/// A hex's base terrain: a kind of ground, or water of some depth. Snow is a [`Cover`]
/// instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerrainFeature {
    Ground(Ground),
    Water { depth: u8 },
}

impl TerrainFeature {
    /// The base terrain under `hex`'s cover, structure and overlay.
    pub fn of(hex: Hex) -> Self {
        if let Some(water) = hex.water() {
            return Self::Water { depth: water.depth };
        }
        match hex.ground() {
            Ground::Snow => Self::Ground(Ground::Clear),
            ground => Self::Ground(ground),
        }
    }
}

/// What can lie over a hex's base terrain, shown in Mappy as overlays. Woods and snow cover dry
/// ground, which a map file then stores as clear; ice freezes water. Fire and smoke are the
/// map's own overlays and are painted separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cover {
    Woods(Woods),
    Snow,
    Ice,
}

impl Cover {
    /// The cover over `hex`, if any.
    pub fn of(hex: Hex) -> Option<Self> {
        if let Some(water) = hex.water() {
            return water.frozen.then_some(Self::Ice);
        }
        if let Some(woods) = hex.woods() {
            return Some(Self::Woods(woods));
        }
        (hex.ground() == Ground::Snow).then_some(Self::Snow)
    }
}

impl Paint {
    /// The hex this paint turns `hex` into. Level and overlay always carry over.
    fn apply(self, hex: Hex) -> Hex {
        let rebuilt = |ground, woods, water, structure| {
            Hex::from_layers(hex.level(), ground, woods, water, structure)
                .with_overlay(hex.overlay())
        };
        match self {
            Self::Level(level) => hex.with_level(level),
            Self::Terrain(TerrainFeature::Ground(ground)) => {
                let (ground, woods) = match Cover::of(hex) {
                    Some(Cover::Woods(woods)) if ground == Ground::Clear => (ground, Some(woods)),
                    Some(Cover::Snow) if ground == Ground::Clear => (Ground::Snow, None),
                    _ => (ground, None),
                };
                rebuilt(ground, woods, None, None)
            }
            Self::Terrain(TerrainFeature::Water { depth }) => {
                let water = Water {
                    depth,
                    frozen: Cover::of(hex) == Some(Cover::Ice),
                };
                let bridge = hex.structure().filter(|_| hex.has_bridge());
                rebuilt(Ground::Clear, None, Some(water), bridge)
            }
            Self::Cover(None) => {
                let ground = match hex.ground() {
                    Ground::Snow => Ground::Clear,
                    ground => ground,
                };
                let water = hex.water().map(|water| Water {
                    frozen: false,
                    ..water
                });
                rebuilt(ground, None, water, hex.structure())
            }
            Self::Cover(Some(Cover::Ice)) => {
                let Some(water) = hex.water() else {
                    return hex;
                };
                hex.with_water(Some(Water {
                    frozen: true,
                    ..water
                }))
            }
            Self::Cover(Some(Cover::Woods(_) | Cover::Snow))
                if hex.water().is_some() || hex.structure().is_some() =>
            {
                hex
            }
            Self::Cover(Some(Cover::Woods(woods))) => {
                rebuilt(Ground::Clear, Some(woods), None, None)
            }
            Self::Cover(Some(Cover::Snow)) => rebuilt(Ground::Snow, None, None, None),
            Self::Structure(None) => hex.with_structure(None),
            Self::Structure(Some(bridge @ Structure::Bridge { .. })) => {
                let water = hex.water().unwrap_or(Water {
                    depth: 1,
                    frozen: false,
                });
                rebuilt(Ground::Clear, None, Some(water), Some(bridge))
            }
            Self::Structure(Some(structure)) => rebuilt(Ground::Clear, None, None, Some(structure)),
            Self::Overlay(overlay) => hex.with_overlay(overlay),
        }
    }
}

/// A paint and the area it covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Brush {
    pub paint: Paint,
    /// Hexes within this many steps of the center are painted.
    pub radius: u8,
}

/// Whether a map file stores `hex` exactly, found by saving and reloading a one-hex map
/// through the game's map file encoder and decoder. Answers are remembered, since a map has
/// few distinct hexes.
pub fn file_holds(hex: Hex) -> bool {
    static ANSWERS: LazyLock<Mutex<BTreeMap<Hex, bool>>> = LazyLock::new(Mutex::default);
    let mut answers = ANSWERS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    *answers.entry(hex).or_insert_with(|| {
        let map = MapAsset {
            width: 1,
            height: 1,
            flags: 0,
            gravity: 100,
            temperature: 20,
            hexes: Arc::new(vec![hex]),
            points_of_interest: Vec::new(),
        };
        map.to_file()
            .and_then(|source| MapAsset::parse(&source))
            .is_ok_and(|decoded| decoded.hexes[0] == hex)
    })
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
    /// Indices of hexes a map file cannot store; see [`file_holds`].
    unsavable: BTreeSet<usize>,
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
    /// A blank map of clear ground at level zero.
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
        let unsavable = map
            .hexes
            .iter()
            .enumerate()
            .filter(|(_, hex)| !file_holds(**hex))
            .map(|(index, _)| index)
            .collect();
        Self {
            path,
            map,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            stroke: BTreeMap::new(),
            unsavable,
            hexes_id: next_hexes_id(),
            changes: Arc::default(),
        }
    }

    /// Write the map to `path` after checking that the file will load back as this map.
    pub fn save_to(&mut self, path: &Path) -> Result<()> {
        self.end_stroke();
        if let Some(&first) = self.unsavable.first() {
            let width = usize::from(self.map.width);
            anyhow::bail!(
                "{} hex(es) have layers a map file cannot store, the first at {},{}",
                self.unsavable.len(),
                first % width,
                first / width
            );
        }
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

    /// Hexes a map file cannot store, by row-major index.
    pub fn unsavable(&self) -> &BTreeSet<usize> {
        &self.unsavable
    }

    /// The hex at a coordinate, or `None` off the map.
    pub fn hex(&self, coordinate: HexCoordinate) -> Option<Hex> {
        self.map.hex(coordinate.x, coordinate.y)
    }

    /// Current flags, gravity and temperature.
    pub fn settings(&self) -> MapSettings {
        MapSettings {
            flags: self.map.flags,
            gravity: self.map.gravity,
            temperature: self.map.temperature,
        }
    }

    /// Paint `brush` centered on `center` as part of the current stroke.
    pub fn paint(&mut self, center: HexCoordinate, brush: Brush) {
        self.paint_with(center, brush.radius, |hex| brush.paint.apply(hex));
    }

    /// Replace each hex within `radius` steps of `center` with `apply` of it, as part of the
    /// current stroke.
    pub fn paint_with(&mut self, center: HexCoordinate, radius: u8, apply: impl Fn(Hex) -> Hex) {
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

    /// Close the current stroke into one undoable edit.
    pub fn end_stroke(&mut self) {
        let changes: Vec<_> = std::mem::take(&mut self.stroke)
            .into_values()
            .filter(|change| change.before != change.after)
            .collect();
        if changes.is_empty() {
            return;
        }
        self.record(Edit::Hexes(changes));
    }

    /// Replace the flags, gravity and temperature as one undoable edit.
    pub fn set_settings(&mut self, after: MapSettings) {
        self.end_stroke();
        let before = self.settings();
        if before == after {
            return;
        }
        self.apply(&Edit::Settings { before, after }, true);
        self.record(Edit::Settings { before, after });
    }

    /// Whether there is an edit to undo.
    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty() || !self.stroke.is_empty()
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
        if file_holds(hex) {
            self.unsavable.remove(&index);
        } else {
            self.unsavable.insert(index);
        }
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
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use stompymux_map::Terrain;

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
        let mut document = Document::new(5, 5).unwrap();
        place(&mut document, AT, Terrain::Rough, 0, 1);
        let painted = document
            .map
            .hexes
            .iter()
            .filter(|hex| hex.terrain() == Terrain::Rough)
            .count();
        assert_eq!(painted, 7);
        let mut corner = Document::new(5, 5).unwrap();
        place(
            &mut corner,
            HexCoordinate { x: 0, y: 0 },
            Terrain::Rough,
            0,
            1,
        );
        assert!(
            corner
                .map
                .hexes
                .iter()
                .filter(|hex| hex.terrain() == Terrain::Rough)
                .count()
                < 7
        );
    }

    /// Each paint changes only its own layer, and level and overlay survive every other paint.
    #[test]
    fn paints_change_only_their_layers() {
        let mut document = Document::new(3, 3).unwrap();
        place(&mut document, MIDDLE, Terrain::HeavyForest, 3, 0);
        document.paint(MIDDLE, brush(Paint::Level(20)));
        let woods = Hex::new(Terrain::HeavyForest, 20);
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
            Some(Hex::new(Terrain::Rough, 20).with_overlay(smoke))
        );
    }

    /// Terrain replaces the base, keeping covers that suit it and bridges only over water.
    #[test]
    fn terrain_replaces_the_base_and_what_cannot_stand_on_it() {
        let clear = Paint::Terrain(TerrainFeature::Ground(Ground::Clear));
        let rough = Paint::Terrain(TerrainFeature::Ground(Ground::Rough));
        let water = Paint::Terrain(TerrainFeature::Water { depth: 3 });
        let bridge = Some(Structure::Bridge { deck: 4 });
        let cases = [
            (
                Hex::new(Terrain::LightForest, 2),
                rough,
                Hex::new(Terrain::Rough, 2),
            ),
            (
                Hex::new(Terrain::LightForest, 2),
                clear,
                Hex::new(Terrain::LightForest, 2),
            ),
            (
                Hex::new(Terrain::Snow, 2),
                clear,
                Hex::new(Terrain::Snow, 2),
            ),
            (
                Hex::new(Terrain::Snow, 2),
                water,
                Hex::new(Terrain::Water, 3).with_level(2),
            ),
            (Hex::new(Terrain::Ice, 1), water, Hex::new(Terrain::Ice, 3)),
            (
                Hex::new(Terrain::Ice, 1),
                rough,
                Hex::new(Terrain::Rough, 0),
            ),
            (
                Hex::new(Terrain::Building, 9),
                rough,
                Hex::new(Terrain::Rough, 0),
            ),
            (
                Hex::new(Terrain::Bridge, 4),
                rough,
                Hex::new(Terrain::Rough, 0),
            ),
            (
                Hex::new(Terrain::Bridge, 4),
                water,
                Hex::new(Terrain::Water, 3).with_structure(bridge),
            ),
        ];
        for (before, paint, after) in cases {
            assert_eq!(paint.apply(before), after, "{paint:?} over {before:?}");
            assert!(file_holds(after), "{paint:?} over {before:?}");
        }
    }

    /// Woods and snow cover dry, unbuilt hexes, ice freezes water, and clearing a cover
    /// leaves the base terrain.
    #[test]
    fn covers_lie_only_where_they_fit() {
        let snow = Paint::Cover(Some(Cover::Snow));
        let woods = Paint::Cover(Some(Cover::Woods(Woods::Heavy)));
        let ice = Paint::Cover(Some(Cover::Ice));
        let none = Paint::Cover(None);
        let wall = Hex::new(Terrain::Wall, 3);
        let bridge = Hex::new(Terrain::Bridge, 2);
        let frozen_bridge = bridge.with_water(Some(Water {
            depth: 1,
            frozen: true,
        }));
        let cases = [
            (
                Hex::new(Terrain::Rough, 2),
                woods,
                Hex::new(Terrain::HeavyForest, 2),
            ),
            (
                Hex::new(Terrain::LightForest, 2),
                snow,
                Hex::new(Terrain::Snow, 2),
            ),
            (
                Hex::new(Terrain::Water, 2),
                snow,
                Hex::new(Terrain::Water, 2),
            ),
            (wall, woods, wall),
            (Hex::new(Terrain::Water, 2), ice, Hex::new(Terrain::Ice, 2)),
            (
                Hex::new(Terrain::Rough, 2),
                ice,
                Hex::new(Terrain::Rough, 2),
            ),
            (bridge, ice, frozen_bridge),
            (frozen_bridge, none, bridge),
            (
                Hex::new(Terrain::Snow, 2),
                none,
                Hex::new(Terrain::Grassland, 2),
            ),
            (
                Hex::new(Terrain::HeavyForest, 2),
                none,
                Hex::new(Terrain::Grassland, 2),
            ),
        ];
        for (before, paint, after) in cases {
            assert_eq!(paint.apply(before), after, "{paint:?} over {before:?}");
            assert!(file_holds(after), "{paint:?} over {before:?}");
        }
    }

    /// Buildings and walls clear the terrain under them, bridges bring water with them, and
    /// removing a structure leaves the terrain.
    #[test]
    fn structures_keep_hexes_storable() {
        let building = Paint::Structure(Some(Structure::Building { height: 5 }));
        let bridge = Paint::Structure(Some(Structure::Bridge { deck: 2 }));
        let woods = Hex::new(Terrain::LightForest, 3);
        assert_eq!(
            building.apply(woods),
            Hex::new(Terrain::Building, 5).with_level(3)
        );
        assert_eq!(
            bridge.apply(woods),
            Hex::new(Terrain::Bridge, 2).with_level(3)
        );
        let deep = Hex::new(Terrain::Water, 4);
        assert_eq!(
            bridge.apply(deep),
            deep.with_structure(Some(Structure::Bridge { deck: 2 }))
        );
        assert_eq!(Paint::Structure(None).apply(bridge.apply(deep)), deep);
        for hex in [woods, deep, Hex::new(Terrain::Ice, 2)] {
            for paint in [building, bridge, Paint::Structure(None)] {
                assert!(file_holds(paint.apply(hex)), "{paint:?} over {hex:?}");
            }
        }
    }

    /// Hexes a map file cannot store are tracked as they are painted, and saving refuses until
    /// none remain.
    #[test]
    fn unsavable_hexes_are_tracked_and_block_saving() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.toml");
        let mut document = Document::new(3, 3).unwrap();
        let woods_on_rough = Hex::new(Terrain::Rough, 0).with_woods(Some(Woods::Light));
        document.paint_with(MIDDLE, 0, |_| woods_on_rough);
        assert_eq!(*document.unsavable(), BTreeSet::from([4]));
        let error = document.save_to(&path).unwrap_err().to_string();
        assert!(error.contains("1,1"), "{error}");
        assert!(!path.exists());
        document.undo();
        assert!(document.unsavable().is_empty());
        document.save_to(&path).unwrap();
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
        place(&mut document, MIDDLE, Terrain::Grassland, 0, 0);
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

    /// Saved maps load back unchanged and the document is no longer dirty.
    #[test]
    fn saved_maps_reload_identically() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.toml");
        let mut document = Document::new(4, 3).unwrap();
        place(&mut document, MIDDLE, Terrain::Fire, 1, 1);
        document.save_to(&path).unwrap();
        assert!(!document.dirty);
        let reloaded = Document::open(&path).unwrap();
        assert_eq!(reloaded.map, document.map);
        assert!(reloaded.unsavable().is_empty());
    }
}
