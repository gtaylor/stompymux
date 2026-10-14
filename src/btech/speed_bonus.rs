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

/// Apply map gravity to both ceilings of a Mech's envelope. Gravity is the last movement
/// modifier (Tactical Operations p. 53), so it scales what equipment and heat left.
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

/// Apply map gravity and then extreme temperature to a vehicle's speed: beyond −30 °C or
/// 50 °C a vehicle loses a cruising MP for every started ten degrees, and its flank speed is
/// recomputed from the cruising speed left.
pub(super) fn vehicle_on_map(
    world: &crate::World,
    position: Option<super::Position>,
    speed: f64,
) -> Result<f64> {
    let speed = on_map(world, position, speed)?;
    let Some(map) = position.and_then(|position| world.btech.maps().get(&position.map)) else {
        return Ok(speed);
    };
    Ok(temperature_limited(speed, map.temperature()))
}

/// A vehicle's top speed in km/h at `temperature`, starting from `speed`.
fn temperature_limited(speed: f64, temperature: i8) -> f64 {
    let steps = super::extreme_temperature_steps(temperature).unsigned_abs();
    if steps == 0 || speed <= 0.0 {
        return speed;
    }
    let cruising = (speed / 1.5 / 10.75).round_ties_even();
    let remaining = (cruising - f64::from(steps)).max(0.0);
    ((remaining * 1.5).ceil() * 10.75).min(speed)
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
        // A 5/8 vehicle: 86 km/h flank.
        assert_eq!(temperature_limited(86.0, 20), 86.0);
        assert_eq!(temperature_limited(86.0, 50), 86.0);
        assert_eq!(temperature_limited(86.0, 55), 64.5);
        assert_eq!(temperature_limited(86.0, -40), 64.5);
        assert_eq!(temperature_limited(86.0, -41), 53.75);
        assert_eq!(temperature_limited(86.0, i8::MIN), 0.0);
        assert_eq!(temperature_limited(0.0, i8::MIN), 0.0);
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
