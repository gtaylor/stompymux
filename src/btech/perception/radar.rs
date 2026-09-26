//! Anti-aircraft radar: long-range tracking of airborne targets using absolute altitude.
use crate::btech::{BattleHex, BattleTerrainLos, BattleUnit, BattleVehicle, Terrain};
use anyhow::{Result, ensure};

/// Radar reach on every chassis; the line-of-sight trace caps radar-equipped pairs here too.
pub const RADAR_RANGE: u16 = 180;

/// Integer target heights used by radar across supported chassis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleRadarTarget {
    /// Flying chassis receive the tracking bonus even below altitude ten.
    pub flying_type: bool,
    pub elevation: i32,
    pub height_above_surface: i64,
}

impl BattleRadarTarget {
    /// Aim contribution when radar reaches the target, or `None` when it cannot.
    ///
    /// Radar ignores darkness, smoke, fire and electronic interference but needs clear terrain.
    /// Targets at or below altitude two, or within one level of the surface, are invisible to
    /// it, and below altitude ten the range must be strictly less than altitude squared.
    pub fn evaluate(
        self,
        terrain: BattleTerrainLos,
        distance: f64,
        maximum: u16,
    ) -> Result<Option<i16>> {
        ensure!(
            distance.is_finite() && distance >= 0.0,
            "Invalid radar distance"
        );
        let reaches = !terrain.blocked
            && self.elevation > 2
            && self.height_above_surface > 1
            && distance <= f64::from(maximum)
            && (self.elevation >= 10 || distance < f64::from(self.elevation).powi(2));
        if !reaches {
            return Ok(None);
        }
        let tracking = if self.elevation >= 10 || self.flying_type {
            3
        } else {
            0
        };
        Ok(Some(
            i16::from(terrain.woods)
                + i16::from(terrain.target_woods)
                + if terrain.partial_cover { 2 } else { 0 }
                - tracking,
        ))
    }

    /// Water and bridge surfaces use negative depth; intact ice uses sea level above the sheet.
    pub(crate) fn above_tile(elevation: i32, tile: BattleHex, flying_type: bool) -> Self {
        let base = surface_datum(tile);
        let upper = if tile.terrain == Terrain::Ice {
            0
        } else {
            base
        };
        let lower = if tile.terrain == Terrain::Bridge {
            -1
        } else {
            base
        };
        Self {
            flying_type,
            elevation,
            height_above_surface: i64::from(elevation)
                - i64::from(if elevation >= upper { upper } else { lower }),
        }
    }
}

/// Terrain datum for radar's surface-clearance calculation.
fn surface_datum(tile: BattleHex) -> i32 {
    let magnitude = i32::from(tile.elevation);
    if matches!(
        tile.terrain,
        Terrain::Water | Terrain::Ice | Terrain::Bridge
    ) {
        return -magnitude;
    }
    magnitude
}

impl BattleUnit {
    /// Radar is available only on a chassis carrying the AntiAircraft technology flag.
    pub fn has_radar(&self) -> bool {
        self.definition().has_special("AntiAircraft")
    }
}

impl BattleVehicle {
    /// Radar is available only on a chassis carrying the AntiAircraft technology flag.
    pub fn has_radar(&self) -> bool {
        self.definition().has_special("AntiAircraft")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sensor clearance keeps the game's distinct land, water, ice and bridge datums.
    #[test]
    fn radar_surface_clearance() {
        for (terrain, elevation, expected) in [
            (Terrain::Grassland, 5, 1),
            (Terrain::Water, 5, 9),
            (Terrain::Ice, 5, 5),
            (Terrain::Ice, -2, 2),
            (Terrain::Bridge, 5, 9),
            (Terrain::Bridge, -5, -4),
        ] {
            assert_eq!(
                BattleRadarTarget::above_tile(
                    elevation,
                    BattleHex {
                        terrain,
                        elevation: 4
                    },
                    false
                )
                .height_above_surface,
                expected
            );
        }
    }

    /// Altitude gates reach, and high or flying targets receive the tracking bonus.
    #[test]
    fn altitude_gates_reach_and_tracking() {
        let clear = BattleTerrainLos::default();
        let target = |elevation, flying_type| BattleRadarTarget {
            flying_type,
            elevation,
            height_above_surface: i64::from(elevation),
        };
        assert_eq!(target(2, true).evaluate(clear, 1.0, 180).unwrap(), None);
        assert_eq!(
            target(5, false).evaluate(clear, 24.9, 180).unwrap(),
            Some(0)
        );
        assert_eq!(target(5, false).evaluate(clear, 25.0, 180).unwrap(), None);
        assert_eq!(
            target(5, true).evaluate(clear, 20.0, 180).unwrap(),
            Some(-3)
        );
        assert_eq!(
            target(10, false).evaluate(clear, 180.0, 180).unwrap(),
            Some(-3)
        );
        assert_eq!(target(10, false).evaluate(clear, 180.1, 180).unwrap(), None);
        let blocked = BattleTerrainLos {
            blocked: true,
            ..Default::default()
        };
        assert_eq!(target(20, true).evaluate(blocked, 1.0, 180).unwrap(), None);
        let covered = BattleTerrainLos {
            woods: 1,
            target_woods: 2,
            partial_cover: true,
            ..Default::default()
        };
        assert_eq!(
            target(20, false).evaluate(covered, 1.0, 180).unwrap(),
            Some(2)
        );
    }
}
