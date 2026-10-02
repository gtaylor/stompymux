//! Map-file export separates terrain encoding from transactional cleanup and file publication.
use super::{BattleHexCoordinate, BattleMapAsset, StoredBattleMap, Terrain};
use anyhow::Result;
use serde::Serialize;
use std::sync::Arc;

/// Complete map-file bytes and stale underlying effects for the enclosing save action to clear.
/// Encoding is read-only: callers apply cleanup and publish the file through their transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleMapExport {
    /// Complete text in the map file format.
    pub source: String,
    /// Underlying stale fire or smoke to clear when the save action commits.
    pub stale_effects: Vec<BattleHexCoordinate>,
}

impl StoredBattleMap {
    /// Encode this map as a map file without writing files or changing live terrain.
    /// Fire and smoke overlays are not part of the map, so each hex saves its underlying
    /// terrain. Fire painted into the grid is kept only on maps with permanent fire; it and
    /// painted smoke otherwise save as clear ground and are reported as stale. Map objects
    /// are not saved.
    pub fn export_asset(&self) -> Result<BattleMapExport> {
        self.validate()?;
        let mut hexes = Vec::with_capacity((self.width * self.height) as usize);
        let mut stale_effects = Vec::new();
        for y in 0..self.height {
            for x in 0..self.width {
                let base = self.base_hex(x, y)?;
                let stale = match base.terrain() {
                    Terrain::Fire => !self.has_flag(super::BattleMapFlag::PermanentFire),
                    Terrain::Smoke => true,
                    _ => false,
                };
                if stale {
                    stale_effects.push(BattleHexCoordinate {
                        x: x as i32,
                        y: y as i32,
                    });
                    hexes.push(base.with_terrain(Terrain::Grassland));
                } else {
                    hexes.push(base);
                }
            }
        }
        let asset = BattleMapAsset {
            width: u16::try_from(self.width)?,
            height: u16::try_from(self.height)?,
            flags: i32::try_from(self.flags & super::BattleMapFlag::mask())?,
            gravity: u8::try_from(self.gravity)?,
            temperature: i8::try_from(self.temperature)?,
            hexes: Arc::new(hexes),
        };
        Ok(BattleMapExport {
            source: asset.to_file()?,
            stale_effects,
        })
    }
}
