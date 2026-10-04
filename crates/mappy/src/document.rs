//! The map being edited: layer and settings changes with undo/redo, loading and saving.
//!
//! Every change is recorded as an [`Edit`] holding before and after values, so undo and redo
//! replay edits in either direction. Brush strokes accumulate into one edit until the stroke
//! ends, so a drag across many hexes undoes in one step.
//!
//! Hexes are edited layer by layer, which can build hexes the map file format cannot store:
//! the file keeps one feature per hex, so woods on rough ground, or a bridge with no water
//! under it, would not survive a save. [`file_holds`] asks the game's own encoder and
//! decoder, the document keeps the set of hexes that fail, and saving refuses while any do.
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

/// What a brush application writes into each hex it touches. Each layer is `None` to leave
/// that layer of every hex alone; optional layers are `Some(None)` to clear them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Brush {
    pub level: Option<u8>,
    pub ground: Option<Ground>,
    pub woods: Option<Option<Woods>>,
    pub water: Option<Option<Water>>,
    pub structure: Option<Option<Structure>>,
    pub overlay: Option<Option<DecorationKind>>,
    /// Hexes within this many steps of the center are painted.
    pub radius: u8,
}

impl Brush {
    /// A brush that paints every layer of `hex`; what the eyedropper should produce.
    #[cfg(test)]
    pub fn matching(hex: Hex, radius: u8) -> Self {
        Self {
            level: Some(hex.level()),
            ground: Some(hex.ground()),
            woods: Some(hex.woods()),
            water: Some(hex.water()),
            structure: Some(hex.structure()),
            overlay: Some(hex.overlay()),
            radius,
        }
    }

    /// The hex this brush turns `hex` into.
    fn apply(self, mut hex: Hex) -> Hex {
        if let Some(level) = self.level {
            hex = hex.with_level(level);
        }
        if let Some(ground) = self.ground {
            hex = hex.with_ground(ground);
        }
        if let Some(woods) = self.woods {
            hex = hex.with_woods(woods);
        }
        if let Some(water) = self.water {
            hex = hex.with_water(water);
        }
        if let Some(structure) = self.structure {
            hex = hex.with_structure(structure);
        }
        if let Some(overlay) = self.overlay {
            hex = hex.with_overlay(overlay);
        }
        hex
    }
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
        let radius = i32::from(brush.radius);
        let width = i32::from(self.map.width);
        let height = i32::from(self.map.height);
        for y in (center.y - radius).max(0)..=(center.y + radius).min(height - 1) {
            for x in (center.x - radius).max(0)..=(center.x + radius).min(width - 1) {
                let coordinate = HexCoordinate { x, y };
                if coordinate.distance(center) > u64::from(brush.radius) {
                    continue;
                }
                let index = y as usize * self.map.width as usize + x as usize;
                let before = self.map.hexes[index];
                let after = brush.apply(before);
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

    /// A brush painting every layer of the hex the compact symbol-and-digit notation describes.
    fn brush(terrain: Terrain, value: u8, radius: u8) -> Brush {
        Brush::matching(Hex::new(terrain, value), radius)
    }

    /// A drag across many hexes undoes and redoes as one step.
    #[test]
    fn strokes_undo_and_redo_as_one_edit() {
        let mut document = Document::new(5, 5).unwrap();
        let blank = document.map.clone();
        document.paint(AT, brush(Terrain::Water, 2, 0));
        document.paint(HexCoordinate { x: 3, y: 2 }, brush(Terrain::Water, 2, 0));
        document.paint(AT, brush(Terrain::Road, 1, 0));
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
        document.paint(AT, brush(Terrain::Rough, 0, 1));
        let painted = document
            .map
            .hexes
            .iter()
            .filter(|hex| hex.terrain() == Terrain::Rough)
            .count();
        assert_eq!(painted, 7);
        let mut corner = Document::new(5, 5).unwrap();
        corner.paint(HexCoordinate { x: 0, y: 0 }, brush(Terrain::Rough, 0, 1));
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

    /// A brush changes only the layers it sets and leaves the rest of each hex alone.
    #[test]
    fn brushes_change_only_their_layers() {
        let mut document = Document::new(3, 3).unwrap();
        document.paint(MIDDLE, brush(Terrain::HeavyForest, 3, 0));
        let raise = Brush {
            level: Some(20),
            ..Brush::default()
        };
        document.paint(MIDDLE, raise);
        assert_eq!(
            document.hex(MIDDLE),
            Some(Hex::new(Terrain::HeavyForest, 20))
        );
        let clear = Brush {
            woods: Some(None),
            ground: Some(Ground::Rough),
            ..Brush::default()
        };
        document.paint(MIDDLE, clear);
        assert_eq!(document.hex(MIDDLE), Some(Hex::new(Terrain::Rough, 20)));
    }

    /// Hexes whose layers a map file cannot store are tracked as they are painted, and
    /// saving refuses until none remain.
    #[test]
    fn unsavable_hexes_are_tracked_and_block_saving() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("test.toml");
        let mut document = Document::new(3, 3).unwrap();
        let woods_on_rough = Brush {
            ground: Some(Ground::Rough),
            woods: Some(Some(Woods::Light)),
            ..Brush::default()
        };
        document.paint(MIDDLE, woods_on_rough);
        assert_eq!(*document.unsavable(), BTreeSet::from([4]));
        let error = document.save_to(&path).unwrap_err().to_string();
        assert!(error.contains("1,1"), "{error}");
        assert!(!path.exists());
        document.undo();
        assert!(document.unsavable().is_empty());
        document.save_to(&path).unwrap();
    }

    /// Bridges are storable over water and not over dry ground, fire and smoke over anything,
    /// and one feature per hex: woods on rough ground are not storable.
    #[test]
    fn file_holds_follows_the_map_file_rules() {
        let bridge = Some(Structure::Bridge { deck: 2 });
        assert!(file_holds(
            Hex::new(Terrain::Water, 1).with_structure(bridge)
        ));
        assert!(!file_holds(Hex::at_level(0).with_structure(bridge)));
        let fire = Some(DecorationKind::Fire);
        assert!(file_holds(Hex::at_level(4).with_overlay(fire)));
        assert!(file_holds(Hex::new(Terrain::Rough, 4).with_overlay(fire)));
        let water_on_a_hill = Hex::new(Terrain::Water, 2).with_level(6);
        assert!(file_holds(water_on_a_hill.with_structure(bridge)));
        assert!(!file_holds(
            Hex::new(Terrain::Rough, 4).with_woods(Some(Woods::Light))
        ));
        assert!(file_holds(Hex::new(Terrain::Building, 30).with_level(5)));
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
        document.paint(MIDDLE, brush(Terrain::Sand, 0, 0));
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
        document.paint(MIDDLE, brush(Terrain::Grassland, 0, 0));
        document.paint(MIDDLE, brush(Terrain::Rough, 0, 0));
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
            document.paint(MIDDLE, brush(Terrain::Rough, 0, 1));
            document.paint(MIDDLE, brush(Terrain::Sand, 0, 1));
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
        document.paint(MIDDLE, brush(Terrain::Fire, 1, 1));
        document.save_to(&path).unwrap();
        assert!(!document.dirty);
        let reloaded = Document::open(&path).unwrap();
        assert_eq!(reloaded.map, document.map);
        assert!(reloaded.unsavable().is_empty());
    }
}
