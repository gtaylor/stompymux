//! Shared running-speed equipment bonuses after load and before environmental gravity.
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

/// Live equipment inputs to the speed calculation.
#[derive(Clone, Copy, Default)]
pub(super) struct SpeedBonuses {
    pub masc: bool,
    pub supercharger: bool,
    pub hot_myomer: bool,
}

impl SpeedBonuses {
    /// Equipment boosts add to running MP before myomer rounding.
    pub fn apply(self, maximum: f64) -> Result<f32> {
        ensure!(
            maximum.is_finite() && maximum >= 0.0,
            "Invalid unloaded maximum speed"
        );
        let mut speed = maximum as f32;
        let boosts = u8::from(self.masc) + u8::from(self.supercharger);
        if boosts != 0 {
            speed = (2.0 * speed / 3.0) * (1.5 + 0.5 * f32::from(boosts));
        }
        if self.hot_myomer {
            speed = (((speed / 1.5 / 10.75).round_ties_even() + 1.0) * 1.5).ceil() * 10.75;
        }
        ensure!(speed.is_finite(), "Invalid effective speed");
        Ok(speed)
    }
}

/// Bound retained controls by installed equipment and the largest allowed gravity bonus.
pub(super) fn saved_limit(base: f64, masc: bool, supercharger: bool, myomer: bool) -> f64 {
    SpeedBonuses {
        masc,
        supercharger,
        hot_myomer: myomer,
    }
    .apply(base)
    .and_then(|speed| gravity(speed, Some(50)))
    .unwrap_or(f64::NAN)
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

    /// Both equipment boosts add before environmental gravity scales the result.
    #[test]
    fn additive_boosts_and_gravity_order() {
        for mask in 0..4u8 {
            let bonus = SpeedBonuses {
                masc: mask & 1 != 0,
                supercharger: mask & 2 != 0,
                ..Default::default()
            };
            let expected = [64.5, 86.0, 107.5][mask.count_ones() as usize];
            assert_eq!(bonus.apply(64.5).unwrap(), expected);
            for (g, multiplier) in [(0, 2.0), (49, 2.0), (50, 2.0), (100, 1.0), (200, 0.5)] {
                assert_eq!(
                    gravity(expected, Some(g)).unwrap(),
                    f64::from(expected) * multiplier
                );
            }
        }
    }

    /// Myomer walking-MP ties round to even; invalid speed and gravity are rejected.
    #[test]
    fn hot_myomer_rounding() {
        let bonus = SpeedBonuses {
            hot_myomer: true,
            ..Default::default()
        };
        assert_eq!(bonus.apply(40.3125).unwrap(), 53.75);
        assert_eq!(bonus.apply(56.4375).unwrap(), 86.0);
        assert!(SpeedBonuses::default().apply(f64::INFINITY).is_err());
        assert!(SpeedBonuses::default().apply(-1.0).is_err());
        assert!(gravity(1.0, Some(256)).is_err());
    }
}
