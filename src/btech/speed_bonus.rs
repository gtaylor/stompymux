//! Shared load policy and the environmental speed rules applied after equipment: map gravity
//! for everything, and extreme temperature for vehicles.
use anyhow::Result;

/// Host policy shared by load accounting and myomer towing assistance.
#[derive(Clone, Copy)]
pub(crate) struct SpeedPolicy {
    pub tsm_tow_bonus: bool,
}

impl SpeedPolicy {
    /// Standalone simulation defaults match the configured game defaults.
    pub const STANDARD: Self = Self {
        tsm_tow_bonus: true,
    };

    /// Read the towing policy from the live host configuration.
    pub fn configured(config: &crate::Config) -> Self {
        Self {
            tsm_tow_bonus: config.battletech.tsm_tow_bonus != 0,
        }
    }
}

/// The lightest gravity a map's percentage can stand for in movement arithmetic: a map of no
/// gravity at all is treated as one hundredth of a G so the division stays defined, as for
/// jumping.
const LIGHTEST_GRAVITY: i64 = 1;

/// Bound saved controls by the fastest an envelope of `base` km/h can become under the
/// lightest gravity: divided by its G-rating, with a margin for rounding the walking MP and
/// refiguring running from it.
pub(super) fn saved_limit(base: f64) -> f64 {
    base * 100.0 / LIGHTEST_GRAVITY as f64 + 3.0 * super::MP_KPH
}

/// Walking (cruising) MP at `gravity` percent of standard: divided by the G-rating and rounded
/// to the nearest whole number, halves down (Tactical Operations: Advanced Rules p. 53).
pub(super) fn gravity_walking_mp(walking_mp: f64, gravity: i64) -> f64 {
    let scaled = walking_mp * 100.0 / gravity.max(LIGHTEST_GRAVITY) as f64;
    (scaled - 0.5).ceil().max(0.0)
}

/// An envelope at `gravity` percent of standard: gravity changes the walking (cruising) MP
/// after every other movement modifier, and running (flanking) is figured afresh from the
/// walking MP left, keeping any running offset the envelope carried (Tactical Operations:
/// Advanced Rules p. 53). A unit gravity leaves without MP cannot move.
pub(super) fn under_gravity(envelope: super::SpeedEnvelope, gravity: i64) -> super::SpeedEnvelope {
    if gravity == 100 || envelope.is_stopped() {
        return envelope;
    }
    let walking = gravity_walking_mp(envelope.walking_mp(), gravity);
    if walking == 0.0 {
        return super::SpeedEnvelope::STOPPED;
    }
    envelope.with_walking_mp_delta(walking - envelope.walking_mp())
}

/// The map gravity at `position`, in percent of standard.
pub(super) fn map_gravity(world: &crate::World, position: Option<super::Position>) -> Option<i64> {
    position
        .and_then(|position| world.btech.maps().get(&position.map))
        .map(|map| map.gravity)
}

/// Apply map gravity to both ceilings of a vehicle's envelope. Gravity is the last movement
/// modifier (Tactical Operations p. 53), so it works on what temperature and load left.
pub(super) fn envelope_on_map(
    world: &crate::World,
    position: Option<super::Position>,
    envelope: super::SpeedEnvelope,
) -> Result<super::SpeedEnvelope> {
    envelope.validate()?;
    Ok(match map_gravity(world, position) {
        Some(gravity) => under_gravity(envelope, gravity),
        None => envelope,
    })
}

/// Apply extreme temperature to a vehicle's damage-adjusted envelope: beyond −30 °C or 50 °C
/// a vehicle loses a cruising MP for every started ten degrees, and its flank speed is
/// recomputed from the cruising speed left (Tactical Operations: Advanced Rules p. 60). The
/// loss is in whole MP, before towed load and gravity scale the result.
pub(super) fn vehicle_temperature(
    world: &crate::World,
    position: Option<super::Position>,
    envelope: super::SpeedEnvelope,
) -> super::SpeedEnvelope {
    let Some(map) = position.and_then(|position| world.btech.maps().get(&position.map)) else {
        return envelope;
    };
    temperature_limited(envelope, map.temperature())
}

/// A vehicle's envelope at `temperature`, starting from `envelope`.
fn temperature_limited(envelope: super::SpeedEnvelope, temperature: i8) -> super::SpeedEnvelope {
    let steps = super::extreme_temperature_steps(temperature).unsigned_abs();
    if steps == 0 || envelope.is_stopped() {
        return envelope;
    }
    let limited = envelope.with_walking_mp_delta(-f64::from(steps));
    if limited.walking <= 0.0 {
        return super::SpeedEnvelope::STOPPED;
    }
    limited
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each started ten degrees beyond the comfortable band costs a vehicle one cruising MP,
    /// with flank speed recomputed from what is left.
    #[test]
    fn extreme_temperatures_cost_vehicles_cruising_mp() {
        let mp = |envelope: super::super::SpeedEnvelope| {
            (envelope.walking / 10.75, envelope.running / 10.75)
        };
        // A 5/8 vehicle: 53.75 km/h cruising, 86 km/h flank.
        let tank = super::super::SpeedEnvelope::from_walking_mp(5.0);
        assert_eq!(mp(temperature_limited(tank, 20)), (5.0, 8.0));
        assert_eq!(mp(temperature_limited(tank, 50)), (5.0, 8.0));
        assert_eq!(mp(temperature_limited(tank, 55)), (4.0, 6.0));
        assert_eq!(mp(temperature_limited(tank, -40)), (4.0, 6.0));
        assert_eq!(mp(temperature_limited(tank, -41)), (3.0, 5.0));
        assert_eq!(
            temperature_limited(tank, i8::MIN),
            super::super::SpeedEnvelope::STOPPED
        );
        assert_eq!(
            temperature_limited(super::super::SpeedEnvelope::STOPPED, i8::MIN),
            super::super::SpeedEnvelope::STOPPED
        );
    }

    /// Gravity divides walking MP by the G-rating, rounded to the nearest whole MP with halves
    /// down, and running is figured afresh from what is left (Tactical Operations: Advanced
    /// Rules p. 53): a 4/6 moves 5/8 at 0.75 G, 3/5 at 1.25 G and 16/24 at 0.25 G, with no
    /// lower bound on the gravity.
    #[test]
    fn gravity_rounds_walking_and_refigures_running() {
        let mp = |envelope: super::super::SpeedEnvelope| {
            (envelope.walking / 10.75, envelope.running / 10.75)
        };
        let unit = super::super::SpeedEnvelope::from_walking_mp(4.0);
        for (gravity, expected) in [
            (100, (4.0, 6.0)),
            (75, (5.0, 8.0)),
            (125, (3.0, 5.0)),
            (25, (16.0, 24.0)),
            (160, (2.0, 3.0)),
            (200, (2.0, 3.0)),
            (800, (0.0, 0.0)),
        ] {
            assert_eq!(mp(under_gravity(unit, gravity)), expected, "{gravity}");
        }
        // 5 / 2 = 2.5 rounds down.
        let five = super::super::SpeedEnvelope::from_walking_mp(5.0);
        assert_eq!(mp(under_gravity(five, 200)), (2.0, 3.0));
        assert_eq!(gravity_walking_mp(4.0, 0), 400.0);
        assert!(saved_limit(64.5) >= under_gravity(unit, 1).running);
    }
}
