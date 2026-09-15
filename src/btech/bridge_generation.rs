//! Asset activation identifies road spans bounded by water or ice along opposite search directions.
use super::{BattleMapAsset, Terrain};
use anyhow::{Result, ensure};
use std::sync::Arc;

impl BattleMapAsset {
    /// Convert qualifying roads while preserving elevations and the parsed source's metadata.
    /// The map flag 128 disables generation. Search corridors traverse only roads and bridges.
    pub fn generate_bridges(&mut self) -> Result<usize> {
        ensure!(
            (1..=1000).contains(&self.width)
                && (1..=1000).contains(&self.height)
                && self.hexes.len() == usize::from(self.width) * usize::from(self.height),
            "Invalid map grid"
        );
        if self.flags & 128 != 0 {
            return Ok(0);
        }
        let mut spans = Vec::new();
        for x in 0..i32::from(self.width) {
            for y in 0..i32::from(self.height) {
                if self
                    .hex(x, y)
                    .is_none_or(|hex| hex.terrain != Terrain::Road)
                {
                    continue;
                }
                // The reference searches three steps in each direction independently;
                // it accepts a corridor with water three steps away on both ends.
                if [((0, -1), (0, 1)), ((1, 0), (-1, 1)), ((1, 1), (-1, 0))]
                    .into_iter()
                    .any(|(forward, backward)| {
                        self.water_end(x, y, forward) && self.water_end(x, y, backward)
                    })
                {
                    spans.push(y as usize * usize::from(self.width) + x as usize);
                }
            }
        }
        let count = spans.len();
        if !spans.is_empty() {
            let tiles = Arc::make_mut(&mut self.hexes);
            for index in spans {
                tiles[index].terrain = Terrain::Bridge;
            }
        }
        Ok(count)
    }

    /// Bridge search uses its own coordinate stepping, including the western-edge adjustment.
    fn water_end(&self, mut x: i32, mut y: i32, direction: (i32, i32)) -> bool {
        for _ in 0..3 {
            x += direction.0;
            y += direction.1;
            if x == 0 && direction.0 != 0 {
                y -= 1;
            }
            match self.hex(x, y).map(|hex| hex.terrain) {
                Some(Terrain::Water | Terrain::Ice) => return true,
                Some(Terrain::Road | Terrain::Bridge) => {}
                _ => return false,
            }
        }
        false
    }
}
