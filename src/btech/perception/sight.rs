//! The clear-line rule shared by the sensor band and sight, as a pure function of traced facts.
use super::BattleDetectionChannel;
use crate::btech::{BattleLight, BattleTerrainLos};
use anyhow::Result;

/// Woods points on the path at which the line is no longer clear.
pub(super) const WOODS_LIMIT: u8 = 3;

/// Water hexes between an observer and an underwater target at which the line is no longer clear.
pub(super) const WATER_LIMIT: u8 = 6;

/// Partial cover adds this to a shot that must clear a ridge line.
pub(super) const PARTIAL_COVER: i16 = 3;

/// One observation's terrain trace and the observer's current reach, gathered once per pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SightFacts {
    pub terrain: BattleTerrainLos,
    pub distance: f64,
    pub target_underwater: bool,
    pub crosses_clouds: bool,
    pub light: BattleLight,
    /// Weather visibility, already capped by the battlefield ceiling.
    pub sight_range: u16,
    /// Night reach to an illuminated target, already capped by the battlefield ceiling.
    pub lit_sight_range: u16,
    /// Battlefield line-of-sight ceiling, at most sixty hexes.
    pub ceiling: u16,
    /// Effective all-conditions band; zero when the sensors cannot reach this target.
    pub sensor_range: u16,
    /// Additional cover from a hull-down target behind partial cover.
    pub hull_down: i16,
}

impl SightFacts {
    /// Hills, buildings, fire, smoke, dense woods and the cloud boundary all break the line.
    pub(super) fn clear_line(&self) -> bool {
        let terrain = self.terrain;
        !terrain.blocked
            && !terrain.fire
            && !terrain.smoke
            && terrain.woods < WOODS_LIMIT
            && !self.crosses_clouds
            && (!self.target_underwater || terrain.water < WATER_LIMIT)
            && self.distance <= f64::from(self.ceiling)
    }

    /// Concealment and cover contribute to aim regardless of how the target is perceived.
    pub(super) fn cover(&self) -> i16 {
        i16::from(self.terrain.woods)
            + i16::from(self.terrain.target_woods)
            + partial_cover(self.terrain, self.hull_down)
    }
}

/// Partial cover and hull-down posture, the physical cover a probe cannot see past.
pub(super) fn partial_cover(terrain: BattleTerrainLos, hull_down: i16) -> i16 {
    if terrain.partial_cover {
        PARTIAL_COVER + hull_down
    } else {
        0
    }
}

/// Resolve the sensor band and sight for one clear line, returning the channel and its aim.
///
/// Inside the sensor band darkness and weather do not matter. Beyond it, weather visibility
/// sets the reach; at night an unlit target costs +1 and a lit one can be seen three times
/// as far. `lit` is only consulted when the answer depends on it.
pub(super) fn perceive(
    facts: &SightFacts,
    lit: impl FnOnce() -> Result<bool>,
) -> Result<Option<(BattleDetectionChannel, i16)>> {
    if !facts.clear_line() {
        return Ok(None);
    }
    let cover = facts.cover();
    if facts.sensor_range > 0 && facts.distance <= f64::from(facts.sensor_range) {
        return Ok(Some((BattleDetectionChannel::Sensors, cover)));
    }
    if facts.light != BattleLight::Night {
        return Ok((facts.distance <= f64::from(facts.sight_range))
            .then_some((BattleDetectionChannel::Sight, cover)));
    }
    if facts.distance > f64::from(facts.lit_sight_range.max(facts.sight_range)) {
        return Ok(None);
    }
    if lit()? {
        return Ok(Some((BattleDetectionChannel::Sight, cover)));
    }
    Ok((facts.distance <= f64::from(facts.sight_range))
        .then_some((BattleDetectionChannel::Sight, cover + 1)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use BattleDetectionChannel::{Sensors, Sight};

    /// A clear day with thirty hexes of visibility and the default fifteen-hex band.
    fn facts(distance: f64) -> SightFacts {
        SightFacts {
            terrain: BattleTerrainLos::default(),
            distance,
            target_underwater: false,
            crosses_clouds: false,
            light: BattleLight::Day,
            sight_range: 30,
            lit_sight_range: 30,
            ceiling: 60,
            sensor_range: 15,
            hull_down: 0,
        }
    }

    fn resolve(facts: SightFacts, lit: bool) -> Option<(BattleDetectionChannel, i16)> {
        perceive(&facts, || Ok(lit)).unwrap()
    }

    /// Sensors reach through darkness and weather; sight follows visibility and light.
    #[test]
    fn bands_follow_distance_light_and_illumination() {
        let night = |distance| SightFacts {
            light: BattleLight::Night,
            sight_range: 5,
            lit_sight_range: 15,
            sensor_range: 10,
            ..facts(distance)
        };
        for (facts, lit, expected) in [
            (facts(15.0), false, Some((Sensors, 0))),
            (facts(15.01), false, Some((Sight, 0))),
            (facts(30.0), false, Some((Sight, 0))),
            (facts(30.01), true, None),
            (night(10.0), false, Some((Sensors, 0))),
            (night(4.0), false, Some((Sensors, 0))),
            (night(12.0), false, None),
            (night(12.0), true, Some((Sight, 0))),
            (night(15.01), true, None),
            (
                SightFacts {
                    sensor_range: 0,
                    ..night(5.0)
                },
                false,
                Some((Sight, 1)),
            ),
            (
                SightFacts {
                    sensor_range: 0,
                    ..night(5.0)
                },
                true,
                Some((Sight, 0)),
            ),
            (
                SightFacts {
                    light: BattleLight::Twilight,
                    ..night(12.0)
                },
                true,
                None,
            ),
            (
                SightFacts {
                    sensor_range: 0,
                    ..night(0.0)
                },
                false,
                Some((Sight, 1)),
            ),
        ] {
            assert_eq!(resolve(facts, lit), expected, "{facts:?} lit={lit}");
        }
    }

    /// Terrain, fire, smoke, woods, clouds, water and the ceiling each break the line.
    #[test]
    fn obstacles_break_the_clear_line_for_both_bands() {
        let obstacles = [
            BattleTerrainLos {
                blocked: true,
                ..Default::default()
            },
            BattleTerrainLos {
                fire: true,
                ..Default::default()
            },
            BattleTerrainLos {
                smoke: true,
                ..Default::default()
            },
            BattleTerrainLos {
                woods: WOODS_LIMIT,
                ..Default::default()
            },
        ];
        for terrain in obstacles {
            assert_eq!(
                resolve(
                    SightFacts {
                        terrain,
                        ..facts(1.0)
                    },
                    true
                ),
                None
            );
        }
        assert_eq!(
            resolve(
                SightFacts {
                    crosses_clouds: true,
                    ..facts(1.0)
                },
                true
            ),
            None
        );
        let underwater = |water| SightFacts {
            terrain: BattleTerrainLos {
                water,
                ..Default::default()
            },
            target_underwater: true,
            ..facts(1.0)
        };
        assert!(resolve(underwater(WATER_LIMIT - 1), false).is_some());
        assert_eq!(resolve(underwater(WATER_LIMIT), false), None);
        assert_eq!(
            resolve(
                SightFacts {
                    ceiling: 10,
                    ..facts(10.5)
                },
                true
            ),
            None
        );
    }

    /// Woods on the path and in the target hex, partial cover and hull-down all add to aim.
    #[test]
    fn cover_accumulates_woods_partial_cover_and_hull_down() {
        let terrain = BattleTerrainLos {
            woods: 2,
            target_woods: 1,
            partial_cover: true,
            ..Default::default()
        };
        assert_eq!(
            resolve(
                SightFacts {
                    terrain,
                    hull_down: 2,
                    ..facts(3.0)
                },
                false
            ),
            Some((Sensors, 8))
        );
        assert_eq!(partial_cover(terrain, 2), 5);
        assert_eq!(
            partial_cover(
                BattleTerrainLos {
                    partial_cover: false,
                    ..terrain
                },
                2
            ),
            0
        );
    }

    /// Illumination is not computed when the answer does not depend on it.
    #[test]
    fn illumination_is_only_consulted_at_night_beyond_the_band() {
        let unused = || -> Result<bool> { panic!("illumination should not be needed") };
        assert!(perceive(&facts(5.0), unused).unwrap().is_some());
        assert!(perceive(&facts(20.0), unused).unwrap().is_some());
        assert!(
            perceive(
                &SightFacts {
                    light: BattleLight::Night,
                    ..facts(5.0)
                },
                unused
            )
            .unwrap()
            .is_some()
        );
        let failing = || -> Result<bool> { anyhow::bail!("lighting failed") };
        assert!(
            perceive(
                &SightFacts {
                    light: BattleLight::Night,
                    ..facts(20.0)
                },
                failing
            )
            .is_err()
        );
    }
}
