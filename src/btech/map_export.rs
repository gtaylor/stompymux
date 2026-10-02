//! Map-file export encodes a live map's terrain and permanent fire and smoke as a map file.
use super::StoredBattleMap;
use super::map_file::overlay_fits;
use anyhow::Result;
use std::sync::Arc;

impl StoredBattleMap {
    /// Encode this map as map-file text without writing files or changing the map.
    /// Each hex saves its terrain. Permanent fire and smoke over clear ground save as `&` and
    /// `:`; fire and smoke that will burn out or drift away, or that cover anything else, are
    /// left out. Map objects are not saved.
    pub fn export_asset(&self) -> Result<String> {
        self.validate()?;
        let mut hexes = Vec::with_capacity((self.width * self.height) as usize);
        for y in 0..self.height {
            for x in 0..self.width {
                let base = self.base_hex(x, y)?;
                let permanent = self
                    .decorations
                    .get(&((y * self.width + x) as u32))
                    .filter(|effect| effect.remaining == 0)
                    .map(|effect| base.with_overlay(Some(effect.kind)))
                    .filter(|&hex| overlay_fits(hex));
                hexes.push(permanent.unwrap_or(base));
            }
        }
        let asset = super::BattleMapAsset {
            width: u16::try_from(self.width)?,
            height: u16::try_from(self.height)?,
            flags: i32::try_from(self.flags & super::BattleMapFlag::mask())?,
            gravity: u8::try_from(self.gravity)?,
            temperature: i8::try_from(self.temperature)?,
            hexes: Arc::new(hexes),
        };
        asset.to_file()
    }
}
