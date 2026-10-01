//! Map-file export separates terrain encoding from transactional cleanup and file publication.
use super::{BattleDecorationKind, BattleHexCoordinate, StoredBattleMap, Terrain};
use anyhow::Result;
use serde::Serialize;
use std::fmt::Write;

/// Complete map-file bytes and stale underlying effects for the enclosing save action to clear.
/// Encoding is read-only: callers apply cleanup and publish the file through their transaction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BattleMapExport {
    /// Complete text in the map asset format.
    pub source: String,
    /// Underlying stale fire or smoke to clear when the save action commits.
    pub stale_effects: Vec<BattleHexCoordinate>,
}

impl StoredBattleMap {
    /// Encode the reference map-file format without writing files or changing live terrain.
    /// Temporary fire reloads as grass; smoke reveals its underlying tile. Object metadata is omitted.
    pub fn export_asset(&self) -> Result<BattleMapExport> {
        self.validate()?;
        let mut source =
            String::with_capacity((self.width * self.height * 2 + self.height + 64) as usize);
        writeln!(source, "{} {}", self.width, self.height)?;
        let mut stale_effects = Vec::new();
        for y in 0..self.height {
            for x in 0..self.width {
                let coordinate = BattleHexCoordinate {
                    x: x as i32,
                    y: y as i32,
                };
                let base = self.base_hex(x, y)?;
                let visible = self.hex(x, y)?;
                let effect = self.decoration(coordinate)?;
                let symbol = match visible.terrain {
                    Terrain::Grassland => '.',
                    Terrain::Fire
                        if effect
                            .is_some_and(|effect| effect.kind == BattleDecorationKind::Fire) =>
                    {
                        '>'
                    }
                    Terrain::Fire if !self.has_flag(super::BattleMapFlag::PermanentFire) => {
                        stale_effects.push(coordinate);
                        '.'
                    }
                    Terrain::Smoke => match base.terrain {
                        Terrain::Smoke => {
                            stale_effects.push(coordinate);
                            '.'
                        }
                        Terrain::Grassland => '.',
                        terrain => terrain.symbol(),
                    },
                    terrain => terrain.symbol(),
                };
                source.push(symbol);
                source.push(char::from(b'0' + base.elevation));
            }
            source.push('\n');
        }
        let flags = self.flags & !1;
        if flags != 0 {
            writeln!(source, "{flags}: {} {}", self.gravity, self.temperature)?;
        }
        Ok(BattleMapExport {
            source,
            stale_effects,
        })
    }
}
