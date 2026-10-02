//! The map being edited: terrain and settings changes with undo/redo, loading and saving.
//!
//! Every change is recorded as an [`Edit`] holding before and after values, so undo and redo
//! replay edits in either direction. Brush strokes accumulate into one edit until the stroke
//! ends, so a drag across many hexes undoes in one step.
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::{
        Arc, Weak,
        atomic::{AtomicU64, Ordering},
    },
};

use anyhow::{Context, Result, ensure};
use stompymux_rs::{
    BattleHex, BattleHexCoordinate, BattleMapAsset, MapCheckIssue, Terrain, check_map_source,
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
    before: BattleHex,
    after: BattleHex,
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

/// What a brush application writes into each hex it touches.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Brush {
    /// Terrain to paint, or `None` to keep each hex's terrain.
    pub terrain: Option<Terrain>,
    /// Elevation digit to paint, or `None` to keep each hex's elevation.
    pub elevation: Option<u8>,
    /// Hexes within this many steps of the center are painted.
    pub radius: u8,
}

impl Brush {
    /// The hex this brush turns `hex` into.
    fn apply(self, hex: BattleHex) -> BattleHex {
        let terrain = self.terrain.unwrap_or(hex.terrain());
        let elevation = self.elevation.unwrap_or(hex.elevation());
        BattleHex::new(terrain, elevation)
    }
}

/// An open map with its edit history.
pub struct Document {
    /// File the map was loaded from or last saved to; `None` for a new map.
    pub path: Option<PathBuf>,
    pub map: BattleMapAsset,
    /// Problems found in the file as it was on disk when loaded.
    pub load_issues: Vec<MapCheckIssue>,
    /// Whether the map differs from the file at `path`.
    pub dirty: bool,
    undo: Vec<Edit>,
    redo: Vec<Edit>,
    /// Changes made by the brush stroke in progress, by hex index.
    stroke: BTreeMap<usize, HexChange>,
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
    pub hexes: Weak<Vec<BattleHex>>,
    pub changes: Weak<Vec<u32>>,
}

/// Source of hex content ids, shared by every document so two never report the same one.
static NEXT_HEXES_ID: AtomicU64 = AtomicU64::new(1);

/// A hex content id no document has used.
fn next_hexes_id() -> u64 {
    NEXT_HEXES_ID.fetch_add(1, Ordering::Relaxed)
}

impl Document {
    /// A blank grassland map at elevation zero.
    pub fn new(width: u16, height: u16) -> Result<Self> {
        ensure!(
            (1..=1000).contains(&width) && (1..=1000).contains(&height),
            "map dimensions must be between 1 and 1000"
        );
        let hexes =
            vec![BattleHex::new(Terrain::Grassland, 0); usize::from(width) * usize::from(height)];
        Ok(Self::from_map(
            None,
            BattleMapAsset {
                width,
                height,
                flags: 0,
                gravity: 100,
                temperature: 20,
                hexes: Arc::new(hexes),
            },
            Vec::new(),
        ))
    }

    /// Load and decode a map file, keeping the file's own problems for display.
    pub fn open(path: &Path) -> Result<Self> {
        let source = std::fs::read(path).with_context(|| format!("reading {}", path.display()))?;
        let text = String::from_utf8_lossy(&source);
        let map =
            BattleMapAsset::parse(&text).with_context(|| format!("decoding {}", path.display()))?;
        Ok(Self::from_map(
            Some(path.to_path_buf()),
            map,
            check_map_source(&source),
        ))
    }

    fn from_map(
        path: Option<PathBuf>,
        map: BattleMapAsset,
        load_issues: Vec<MapCheckIssue>,
    ) -> Self {
        Self {
            path,
            map,
            load_issues,
            dirty: false,
            undo: Vec::new(),
            redo: Vec::new(),
            stroke: BTreeMap::new(),
            hexes_id: next_hexes_id(),
            changes: Arc::default(),
        }
    }

    /// Write the map to `path` in canonical form after checking that it decodes cleanly.
    pub fn save_to(&mut self, path: &Path) -> Result<()> {
        self.end_stroke();
        let source = self.map.to_source();
        let issues = check_map_source(source.as_bytes());
        ensure!(
            issues.is_empty(),
            "refusing to save a map with problems: {}",
            issues
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
                .join("; ")
        );
        std::fs::write(path, &source).with_context(|| format!("writing {}", path.display()))?;
        self.path = Some(path.to_path_buf());
        self.load_issues.clear();
        self.dirty = false;
        Ok(())
    }

    /// The hex at a coordinate, or `None` off the map.
    pub fn hex(&self, coordinate: BattleHexCoordinate) -> Option<BattleHex> {
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
    pub fn paint(&mut self, center: BattleHexCoordinate, brush: Brush) {
        let radius = i32::from(brush.radius);
        let width = i32::from(self.map.width);
        let height = i32::from(self.map.height);
        for y in (center.y - radius).max(0)..=(center.y + radius).min(height - 1) {
            for x in (center.x - radius).max(0)..=(center.x + radius).min(width - 1) {
                let coordinate = BattleHexCoordinate { x, y };
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
    fn set_hex(&mut self, index: usize, hex: BattleHex) {
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
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: BattleHexCoordinate = BattleHexCoordinate { x: 2, y: 2 };
    const MIDDLE: BattleHexCoordinate = BattleHexCoordinate { x: 1, y: 1 };

    fn brush(terrain: Terrain, elevation: u8, radius: u8) -> Brush {
        Brush {
            terrain: Some(terrain),
            elevation: Some(elevation),
            radius,
        }
    }

    /// A drag across many hexes undoes and redoes as one step.
    #[test]
    fn strokes_undo_and_redo_as_one_edit() {
        let mut document = Document::new(5, 5).unwrap();
        let blank = document.map.clone();
        document.paint(AT, brush(Terrain::Water, 2, 0));
        document.paint(
            BattleHexCoordinate { x: 3, y: 2 },
            brush(Terrain::Water, 2, 0),
        );
        document.paint(AT, brush(Terrain::Road, 1, 0));
        document.end_stroke();
        let painted = document.map.clone();
        assert_eq!(document.hex(AT), Some(BattleHex::new(Terrain::Road, 1)));
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
        corner.paint(
            BattleHexCoordinate { x: 0, y: 0 },
            brush(Terrain::Rough, 0, 1),
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

    /// Leaving a brush channel unset keeps that part of each hex.
    #[test]
    fn partial_brushes_keep_the_other_channel() {
        let mut document = Document::new(3, 3).unwrap();
        document.paint(MIDDLE, brush(Terrain::HeavyForest, 3, 0));
        document.paint(
            MIDDLE,
            Brush {
                terrain: None,
                elevation: Some(5),
                radius: 0,
            },
        );
        assert_eq!(
            document.hex(MIDDLE),
            Some(BattleHex::new(Terrain::HeavyForest, 5))
        );
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
        let path = directory.path().join("test.map");
        let mut document = Document::new(4, 3).unwrap();
        document.paint(MIDDLE, brush(Terrain::Fire, 1, 1));
        document.save_to(&path).unwrap();
        assert!(!document.dirty);
        let reloaded = Document::open(&path).unwrap();
        assert_eq!(reloaded.map, document.map);
        assert!(reloaded.load_issues.is_empty());
    }
}
