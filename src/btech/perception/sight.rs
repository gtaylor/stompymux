//! Sight: the clear-line rule of Total Warfare and the visual ranges of Tactical Operations,
//! as pure functions of traced facts, plus the cover an attack must clear.
use crate::btech::{Light, TerrainLos};
use anyhow::Result;

/// Woods and smoke points on the path at which the line is no longer clear (Total Warfare
/// p. 100: three light woods, or a light and a heavy, block line of sight).
pub(super) const WOODS_LIMIT: u8 = 3;

/// Intervening hexes of planted fields at which the line is no longer clear (Tactical
/// Operations p. 38).
pub(super) const FIELDS_LIMIT: u8 = 6;

/// Water hexes between an observer and an underwater target at which the line is no longer
/// clear.
pub(super) const WATER_LIMIT: u8 = 6;

/// Partial cover adds this to a shot that must clear a ridge line (Total Warfare p. 107).
pub(super) const PARTIAL_COVER: i16 = 1;

/// One observation's terrain trace and the observer's current reach, gathered once per pair.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct SightFacts {
    pub terrain: TerrainLos,
    pub distance: f64,
    pub target_underwater: bool,
    pub crosses_clouds: bool,
    pub light: Light,
    /// How far the observer sees under the conditions, its own searchlight included.
    pub sight_range: u16,
    /// How far the observer sees a target lit by another source.
    pub lit_sight_range: u16,
}

impl SightFacts {
    /// Hills, buildings, dense woods or smoke, deep crops and the cloud boundary all break the
    /// line.
    pub(super) fn clear_line(&self) -> bool {
        let terrain = self.terrain;
        !terrain.blocked
            && terrain.obscuring_points() < WOODS_LIMIT
            && terrain.fields < FIELDS_LIMIT
            && !self.crosses_clouds
            && (!self.target_underwater || terrain.water < WATER_LIMIT)
    }

    /// How far the observer sees along this line: its reach, less a hex for every point of
    /// smoke on the way (Tactical Operations p. 189).
    fn reach(&self, lit: bool) -> f64 {
        let base = if lit {
            self.lit_sight_range
        } else {
            self.sight_range
        };
        f64::from(base.saturating_sub(u16::from(self.terrain.smoke)))
    }
}

/// Partial cover and hull-down posture, the physical cover an attack must clear.
pub(super) fn partial_cover(terrain: TerrainLos, hull_down: i16) -> i16 {
    if terrain.partial_cover {
        PARTIAL_COVER + hull_down
    } else {
        0
    }
}

/// Concealment and cover an attack must clear: woods and smoke on the way and in the
/// target's hex, one for every two hexes of crops, and partial cover.
pub(super) fn cover(terrain: TerrainLos, hull_down: i16) -> i16 {
    i16::from(terrain.obscuring_points())
        + i16::from(terrain.target_woods)
        + i16::from(terrain.target_smoke)
        + i16::from(terrain.fields / 2)
        + partial_cover(terrain, hull_down)
}

/// Whether the observer sees the target: a clear line within its reach. `lit` is consulted
/// only when the target lies beyond ordinary sight but within the lit-target reach.
pub(super) fn sees(facts: &SightFacts, lit: impl FnOnce() -> Result<bool>) -> Result<bool> {
    if !facts.clear_line() {
        return Ok(false);
    }
    if facts.distance <= facts.reach(false) {
        return Ok(true);
    }
    if facts.distance > facts.reach(true) {
        return Ok(false);
    }
    lit()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A moonless night: five hexes of sight, forty-five to a lit target.
    fn facts(distance: f64) -> SightFacts {
        SightFacts {
            terrain: TerrainLos::default(),
            distance,
            target_underwater: false,
            crosses_clouds: false,
            light: Light::MoonlessNight,
            sight_range: 5,
            lit_sight_range: 45,
        }
    }

    fn resolve(facts: SightFacts, lit: bool) -> bool {
        sees(&facts, || Ok(lit)).unwrap()
    }

    /// Sight reaches its range, and a lit target farther out, but never a blocked one.
    #[test]
    fn sight_follows_range_and_illumination() {
        assert!(resolve(facts(5.0), false));
        assert!(!resolve(facts(5.5), false));
        assert!(resolve(facts(5.5), true));
        assert!(resolve(facts(45.0), true));
        assert!(!resolve(facts(45.5), true));
        let blocked = SightFacts {
            terrain: TerrainLos {
                blocked: true,
                ..TerrainLos::default()
            },
            ..facts(2.0)
        };
        assert!(!resolve(blocked, true));
    }

    /// Dense woods, crops, clouds and water each break the line; smoke also shortens sight.
    #[test]
    fn obstacles_break_or_shorten_the_line() {
        let dense = SightFacts {
            terrain: TerrainLos {
                woods: 2,
                smoke: 1,
                ..TerrainLos::default()
            },
            ..facts(2.0)
        };
        assert!(!resolve(dense, true));
        let smoky = SightFacts {
            terrain: TerrainLos {
                smoke: 2,
                ..TerrainLos::default()
            },
            ..facts(4.0)
        };
        assert!(!resolve(smoky, false), "two smoke points leave three hexes");
        assert!(resolve(
            SightFacts {
                distance: 3.0,
                ..smoky
            },
            false
        ));
        let crops = SightFacts {
            terrain: TerrainLos {
                fields: 6,
                ..TerrainLos::default()
            },
            ..facts(2.0)
        };
        assert!(!resolve(crops, true));
        assert!(!resolve(
            SightFacts {
                crosses_clouds: true,
                ..facts(2.0)
            },
            true
        ));
        let deep = SightFacts {
            target_underwater: true,
            terrain: TerrainLos {
                water: 6,
                ..TerrainLos::default()
            },
            ..facts(2.0)
        };
        assert!(!resolve(deep, true));
    }

    /// Cover adds up woods, smoke, crops, partial cover and hull-down posture.
    #[test]
    fn cover_accumulates() {
        let terrain = TerrainLos {
            woods: 1,
            smoke: 1,
            target_woods: 2,
            target_smoke: 1,
            fields: 5,
            partial_cover: true,
            ..TerrainLos::default()
        };
        assert_eq!(cover(terrain, 0), 8);
        assert_eq!(cover(terrain, 2), 10);
        assert_eq!(cover(TerrainLos::default(), 2), 0);
    }
}
