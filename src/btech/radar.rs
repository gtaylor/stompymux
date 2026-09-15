//! Radar contacts use absolute altitude, local surface clearance and dedicated AntiAircraft equipment.
use super::*;
use crate::{Flag, ObjectId, World};
use anyhow::{Context, Result, ensure};

/// Integer target heights used by radar across supported chassis.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BattleRadarTarget {
    /// Flying chassis receive the radar aim bonus even below altitude ten.
    pub flying_type: bool,
    pub elevation: i32,
    pub height_above_surface: i64,
}

impl BattleRadarTarget {
    /// Radar ignores smoke, fire and electronic interference, but requires clear terrain LOS.
    /// Below altitude ten, range must be strictly less than altitude squared.
    pub fn evaluate(
        self,
        terrain: BattleTerrainLos,
        distance: f64,
        disabled: bool,
    ) -> Result<BattleSensorReport> {
        self.evaluate_with_maximum(terrain, distance, disabled, 180)
    }

    /// Fixed installations use the same altitude rules with their longer sensor reach.
    fn evaluate_with_maximum(
        self,
        terrain: BattleTerrainLos,
        distance: f64,
        disabled: bool,
        maximum: u16,
    ) -> Result<BattleSensorReport> {
        ensure!(
            distance.is_finite() && distance >= 0.0,
            "Invalid radar distance"
        );
        ensure!(
            terrain.woods <= 15 && terrain.target_woods <= 2,
            "Invalid radar woods counts"
        );
        let eligible = !disabled
            && !terrain.blocked
            && self.elevation > 2
            && self.height_above_surface > 1
            && distance <= f64::from(maximum)
            && (self.elevation >= 10 || distance < f64::from(self.elevation).powi(2));
        Ok(BattleSensorReport {
            eligible,
            acquisition_factor: if eligible {
                (180.0 - distance).clamp(10.0, 90.0) as u8
            } else {
                0
            },
            aim_modifier: i16::from(terrain.woods + terrain.target_woods)
                + if terrain.partial_cover { 2 } else { 0 }
                - if self.elevation >= 10 || self.flying_type {
                    3
                } else {
                    0
                },
        })
    }

    /// Water and bridge sensor surfaces use negative depth; intact ice uses sea level above the sheet.
    fn above_tile(elevation: i32, tile: BattleHex, flying_type: bool) -> Self {
        let base = sensor_base(tile);
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
fn sensor_base(tile: BattleHex) -> i32 {
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

/// Inspect current radar eligibility and aim without consuming dice or modifying contact state.
pub fn radar_contact(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
) -> Result<BattleSensorReport> {
    let report = evaluate_contact(world, observer, target)?;
    Ok(super::visibility::sensor_report(world, target, report))
}

/// Evaluate this sensor's hardware and signature without applying operator visibility.
fn evaluate_contact(
    world: &World,
    observer: ObjectId,
    target: ObjectId,
) -> Result<BattleSensorReport> {
    for id in [observer, target] {
        ensure!(
            world
                .objects
                .get(&id)
                .is_some_and(|object| !object.flags.contains(Flag::Going)),
            "Unit is unavailable"
        );
    }
    let observing =
        super::scanner::scanner_unit(world, observer).context("Observer is not constructed")?;
    let origin = super::los::unit_sight_point(world, observer)?;
    let observed = super::los::unit_sight_point(world, target)?;
    let range = unit_range(world, observer, target)?;
    let position = observed.position;
    let map = &world.btech.maps()[&position.map];
    let tile = map.base_hex(i64::from(position.x), i64::from(position.y))?;
    let elevation = observed.level;
    // Automatic contact scans retain the map ceiling until either endpoint reaches altitude eleven.
    let beyond_map = range.spatial > f64::from(u16::try_from(map.maximum_visibility)?)
        && elevation < 11
        && origin.level < 11;
    let terrain = unit_terrain_los(world, observer, target)?;
    let mut report = BattleRadarTarget::above_tile(
        elevation,
        tile,
        world
            .btech
            .vehicles()
            .get(&target)
            .is_some_and(|unit| unit.definition().is_vtol()),
    )
    .evaluate_with_maximum(
        terrain,
        range.spatial,
        !observing.radar || map.optical_sensor_disabled(BattleSensorMode::Radar) || beyond_map,
        super::sensors::sensor_maximum(world, observer, 180),
    )?;
    report.aim_modifier += super::hull_down::cover_modifier(world, target, terrain.partial_cover);
    Ok(report)
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
}
