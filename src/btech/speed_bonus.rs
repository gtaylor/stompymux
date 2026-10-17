//! Shared load policy and the environmental speed rules applied after equipment: map gravity
//! for everything, and extreme temperature for vehicles.
use anyhow::{Result, ensure};

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

/// Bound retained vehicle controls by the largest allowed gravity bonus.
pub(super) fn saved_limit(base: f64) -> f64 {
    gravity(base as f32, Some(50)).unwrap_or(f64::NAN)
}

/// Apply map gravity after chassis bonuses, preserving ordinary-map precision.
pub(super) fn on_map(
    world: &crate::World,
    position: Option<super::Position>,
    speed: f64,
) -> Result<f64> {
    ensure!(speed.is_finite() && speed >= 0.0, "Invalid effective speed");
    let map = position
        .and_then(|position| world.btech.maps().get(&position.map))
        .filter(|map| map.gravity != 100);
    match map {
        Some(map) => gravity(speed as f32, Some(map.gravity)),
        None => Ok(speed),
    }
}

/// Apply map gravity to both ceilings of a unit's envelope. Gravity is the last movement
/// modifier (Tactical Operations p. 53), so it scales what equipment, heat, temperature
/// and load left.
pub(super) fn envelope_on_map(
    world: &crate::World,
    position: Option<super::Position>,
    envelope: super::SpeedEnvelope,
) -> Result<super::SpeedEnvelope> {
    Ok(super::SpeedEnvelope {
        walking: on_map(world, position, envelope.walking)?,
        running: on_map(world, position, envelope.running)?,
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

/// Environmental rules operate last and retain single-precision arithmetic.
pub(super) fn gravity(speed: f32, gravity: Option<i64>) -> Result<f64> {
    let speed = if let Some(gravity) = gravity {
        ensure!((0..=255).contains(&gravity), "Invalid map gravity");
        speed * 100.0 / gravity.max(50) as f32
    } else {
        speed
    };
    ensure!(speed.is_finite() && speed >= 0.0, "Invalid effective speed");
    Ok(f64::from(speed))
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

    /// Gravity scales speed inversely, no lighter than half a G; invalid gravity is rejected.
    #[test]
    fn gravity_scaling_and_bounds() {
        for (g, multiplier) in [(0, 2.0), (49, 2.0), (50, 2.0), (100, 1.0), (200, 0.5)] {
            assert_eq!(gravity(64.5, Some(g)).unwrap(), 64.5 * multiplier);
        }
        assert_eq!(saved_limit(64.5), 129.0);
        assert!(gravity(1.0, Some(256)).is_err());
        assert!(gravity(f32::INFINITY, None).is_err());
    }
}
