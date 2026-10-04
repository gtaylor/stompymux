//! Map-file export encodes a live map's terrain and permanent fire and smoke as a map file.
use super::StoredBattleMap;
use anyhow::Result;
use std::sync::Arc;

impl StoredBattleMap {
    /// Encode this map as map-file text without writing files or changing the map.
    /// Each hex saves its terrain, and permanent fire and smoke save in the overlay grid. Fire
    /// and smoke that will burn out or drift away are left out. Points of interest are saved;
    /// other map objects are not.
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
                    .map(|effect| base.with_overlay(Some(effect.kind)));
                hexes.push(permanent.unwrap_or(base));
            }
        }
        let asset = super::MapAsset {
            width: u16::try_from(self.width)?,
            height: u16::try_from(self.height)?,
            flags: i32::try_from(self.flags & super::MapFlag::mask())?,
            gravity: u8::try_from(self.gravity)?,
            temperature: i8::try_from(self.temperature)?,
            hexes: Arc::new(hexes),
            points_of_interest: self.points_of_interest.to_vec(),
        };
        asset.to_file()
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        DecorationKind, Hex, HexCoordinate, MapAsset, Terrain, state::map_from_asset,
    };

    /// Load map-file text as a live map, requiring it to validate.
    fn stored(source: &str) -> super::StoredBattleMap {
        let map = map_from_asset("test", MapAsset::parse(source).unwrap()).unwrap();
        map.validate().unwrap();
        map
    }

    /// Buildings on high ground and water and bridges on a plateau validate as live maps.
    #[test]
    fn raised_structures_and_water_validate() {
        stored("terrain = '@'\nlevel = 'a'\nstructure_height = 'b'\n");
        stored(
            "terrain = '~-~'\nlevel = '432'\ndepth = '231'\n\n[[bridges]]\ndeck = 2\nhexes = [[2, 0]]\n",
        );
    }

    /// Fire and smoke from the overlay grid become permanent decorations over the terrain,
    /// and export back into the overlay grid.
    #[test]
    fn overlay_grid_becomes_permanent_decorations() {
        let source = "terrain = '.`~'\nlevel = '120'\ndepth = '..2'\noverlay = '&:.'\n";
        let map = stored(source);
        assert_eq!(
            map.base_hex(1, 0).unwrap(),
            Hex::new(Terrain::LightForest, 2)
        );
        assert_eq!(
            map.hex(1, 0).unwrap().overlay(),
            Some(DecorationKind::Smoke)
        );
        let effect = map
            .decoration(HexCoordinate { x: 1, y: 0 })
            .unwrap()
            .unwrap();
        assert_eq!((effect.kind, effect.remaining), (DecorationKind::Smoke, 0));
        assert_eq!(
            MapAsset::parse(&map.export_asset().unwrap()).unwrap(),
            MapAsset::parse(source).unwrap()
        );
    }

    /// Points of interest survive loading as a live map and exporting it again.
    #[test]
    fn points_of_interest_survive_export() {
        let source = "terrain = '..'\nlevel = '00'\n\n[[points_of_interest]]\ntype = 'objective'\nname = 'Ford'\nx = 1\ny = 0\nelevation = -2\n";
        let asset = MapAsset::parse(source).unwrap();
        let map = stored(source);
        assert_eq!(*map.points_of_interest, asset.points_of_interest);
        assert_eq!(
            MapAsset::parse(&map.export_asset().unwrap()).unwrap(),
            asset
        );
    }
}
