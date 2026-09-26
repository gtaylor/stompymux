//! Sparse map lookup rows preserve allocation independently of mine and building definitions.
use super::{BattleHexCoordinate, StoredBattleMap};
use crate::World;
use anyhow::{Context, Result, ensure};
use std::{collections::BTreeMap, sync::Arc};

/// Independent lookup lanes packed into each two-bit hex cell.
#[derive(Clone, Copy)]
pub(crate) enum LookupKind {
    Mine = 1,
    Hangar = 2,
}

impl StoredBattleMap {
    /// Allocation itself is observable through LIST and resize admission, even without rows.
    pub fn has_lookup_object(&self) -> bool {
        self.lookup_bits.is_some()
    }

    /// Validate sparse row shape while retaining unused padding bits from authored bytes.
    pub(crate) fn validate_lookup_bits(&self) -> Result<()> {
        if let Some(rows) = &self.lookup_bits {
            ensure!(
                self.width > 0 && self.height > 0,
                "Invalid lookup map dimensions"
            );
            let bytes = usize::try_from(self.width)?.div_ceil(4);
            ensure!(
                rows.iter()
                    .all(|(&y, row)| i64::from(y) < self.height && row.len() == bytes),
                "Invalid map lookup row"
            );
        }
        Ok(())
    }

    /// Read one lane without allocating storage or rebuilding definitions.
    pub(crate) fn lookup_bit(
        &self,
        coordinate: BattleHexCoordinate,
        kind: LookupKind,
    ) -> Result<bool> {
        self.lookup_coordinate(coordinate)?;
        let Some(row) = self
            .lookup_bits
            .as_ref()
            .and_then(|rows| rows.get(&(coordinate.y as u32)))
        else {
            return Ok(false);
        };
        let x = coordinate.x as usize;
        Ok(
            row.get(x / 4).context("Invalid map lookup row")? & ((kind as u8) << (2 * (x % 4)))
                != 0,
        )
    }

    /// Set allocates a row; unset allocates only the object and retains any existing row.
    pub(crate) fn set_lookup_bit(
        &mut self,
        coordinate: BattleHexCoordinate,
        kind: LookupKind,
        enabled: bool,
    ) -> Result<()> {
        self.lookup_coordinate(coordinate)?;
        let bytes = usize::try_from(self.width)?.div_ceil(4);
        let rows = Arc::make_mut(
            self.lookup_bits
                .get_or_insert_with(|| Arc::new(BTreeMap::new())),
        );
        self.flags |= 1;
        let y = coordinate.y as u32;
        if enabled {
            rows.entry(y).or_insert_with(|| vec![0; bytes]);
        }
        if let Some(row) = rows.get_mut(&y) {
            let x = coordinate.x as usize;
            let mask = (kind as u8) << (2 * (x % 4));
            if enabled {
                row[x / 4] |= mask;
            } else {
                row[x / 4] &= !mask;
            }
        }
        Ok(())
    }

    /// Clear valid hex lanes while preserving allocation and padding outside map width.
    pub(crate) fn clear_lookup_kind(&mut self, kind: LookupKind) {
        let Some(rows) = self.lookup_bits.as_mut() else {
            return;
        };
        for row in Arc::make_mut(rows).values_mut() {
            for x in 0..self.width as usize {
                row[x / 4] &= !((kind as u8) << (2 * (x % 4)));
            }
        }
    }

    /// Rebuild mine coverage with the same geometry used by mine activation.
    pub(crate) fn rebuild_mine_lookup(&mut self) -> Result<()> {
        self.clear_lookup_kind(LookupKind::Mine);
        let mines: Vec<_> = self.minefields.values().copied().collect();
        for mine in mines {
            let radius = mine.coverage_radius();
            if radius < 0 {
                continue;
            }
            let left = (i64::from(mine.coordinate.x) - radius).max(0);
            let right = (i64::from(mine.coordinate.x) + radius).min(self.width - 1);
            let top = (i64::from(mine.coordinate.y) - radius).max(0);
            let bottom = (i64::from(mine.coordinate.y) + radius).min(self.height - 1);
            for y in top..=bottom {
                for x in left..=right {
                    let coordinate = BattleHexCoordinate {
                        x: i32::try_from(x)?,
                        y: i32::try_from(y)?,
                    };
                    if mine.covers(coordinate)? {
                        self.set_lookup_bit(coordinate, LookupKind::Mine, true)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Check coordinates independently of terrain decoding during persistence restoration.
    fn lookup_coordinate(&self, coordinate: BattleHexCoordinate) -> Result<()> {
        ensure!(
            coordinate.x >= 0
                && coordinate.y >= 0
                && i64::from(coordinate.x) < self.width
                && i64::from(coordinate.y) < self.height,
            "Map lookup coordinate is out of range"
        );
        Ok(())
    }
}

/// Server startup reconciles mine bits after read-only restoration, retaining hangar rows.
pub(crate) fn rebuild_mine_lookups(world: &mut World) -> Result<()> {
    for map in world.btech.maps.values_mut() {
        map.rebuild_mine_lookup()?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Lookup operations also work before a terrain dictionary is decoded.
    fn map() -> StoredBattleMap {
        serde_json::from_value(serde_json::json!({
            "name":"lookup", "width":5, "height":3, "gravity":100,
            "temperature":20, "flags":0, "light":2, "visibility":30,
            "maximum_visibility":60, "cloud_base":0, "sensor_flags":0
        }))
        .unwrap()
    }

    /// Allocation, zero rows and padding are separate from the logical bits.
    #[test]
    fn sparse_lifetime_lanes_padding_and_read_only_queries() {
        let mut map = map();
        let a = BattleHexCoordinate { x: 0, y: 1 };
        let b = BattleHexCoordinate { x: 3, y: 1 };
        let c = BattleHexCoordinate { x: 4, y: 2 };
        assert!(!map.lookup_bit(a, LookupKind::Mine).unwrap());
        assert!(!map.has_lookup_object());
        map.set_lookup_bit(c, LookupKind::Hangar, false).unwrap();
        assert!(map.lookup_bits.as_ref().unwrap().is_empty());
        map.set_lookup_bit(a, LookupKind::Mine, true).unwrap();
        map.set_lookup_bit(b, LookupKind::Hangar, true).unwrap();
        map.set_lookup_bit(c, LookupKind::Hangar, true).unwrap();
        assert_eq!(map.lookup_bits.as_ref().unwrap()[&1], vec![129, 0]);
        let before = map.clone();
        assert!(map.lookup_bit(c, LookupKind::Hangar).unwrap());
        assert!(!map.lookup_bit(c, LookupKind::Mine).unwrap());
        assert_eq!(map, before);
        Arc::make_mut(map.lookup_bits.as_mut().unwrap())
            .get_mut(&2)
            .unwrap()[1] |= 252;
        map.clear_lookup_kind(LookupKind::Mine);
        assert_eq!(map.lookup_bits.as_ref().unwrap()[&1], vec![128, 0]);
        map.clear_lookup_kind(LookupKind::Hangar);
        assert_eq!(map.lookup_bits.as_ref().unwrap()[&1], vec![0, 0]);
        assert_eq!(map.lookup_bits.as_ref().unwrap()[&2], vec![0, 252]);
        assert_eq!(map.lookup_bits.as_ref().unwrap().len(), 2);
        map.validate_lookup_bits().unwrap();
        let before = map.clone();
        assert!(
            map.set_lookup_bit(BattleHexCoordinate { x: 5, y: 0 }, LookupKind::Mine, true)
                .is_err()
        );
        assert_eq!(map, before);
    }

    /// Rebuilding shares the existing geometries and never erases hangar lanes.
    #[test]
    fn mine_rebuilding_matches_definition_coverage() {
        use crate::btech::{BattleMineKind, BattleMinefield};
        for kind in [
            BattleMineKind::Standard,
            BattleMineKind::Command,
            BattleMineKind::Inferno,
            BattleMineKind::Vibra,
            BattleMineKind::Trigger,
        ] {
            for extra in [-5, 0, 1, 3, 90, 100, 110] {
                let mut map = map();
                let center = BattleHexCoordinate { x: 2, y: 1 };
                map.set_lookup_bit(center, LookupKind::Hangar, true)
                    .unwrap();
                let mine = BattleMinefield {
                    coordinate: center,
                    kind,
                    strength: 5,
                    extra,
                    owner: crate::ObjectId(-1),
                };
                Arc::make_mut(&mut map.minefields).insert(0, mine);
                map.rebuild_mine_lookup().unwrap();
                for y in 0..3 {
                    for x in 0..5 {
                        let p = BattleHexCoordinate { x, y };
                        assert_eq!(
                            map.lookup_bit(p, LookupKind::Mine).unwrap(),
                            mine.covers(p).unwrap(),
                            "{kind:?} {extra}: {p:?}"
                        );
                    }
                }
                assert!(map.lookup_bit(center, LookupKind::Hangar).unwrap());
                Arc::make_mut(&mut map.minefields).clear();
                map.rebuild_mine_lookup().unwrap();
                assert!(!map.lookup_bit(center, LookupKind::Mine).unwrap());
                assert!(map.lookup_bit(center, LookupKind::Hangar).unwrap());
            }
        }
    }
}
