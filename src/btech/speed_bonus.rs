//! Shared reference-order running-speed bonuses after load and before environmental gravity.
use anyhow::{Result, ensure};

/// Host policy shared by load accounting and sprint-sensitive myomer conversion.
#[derive(Clone, Copy)]
pub(crate) struct SpeedPolicy {
    pub tsm_tow_bonus: bool,
    pub tsm_sprint_bonus: bool,
}

impl SpeedPolicy {
    /// Standalone simulation defaults match the configured game defaults.
    pub const STANDARD: Self = Self {
        tsm_tow_bonus: true,
        tsm_sprint_bonus: true,
    };

    /// Read both related policies from the same live host configuration.
    pub fn configured(config: &crate::Config) -> Self {
        Self {
            tsm_tow_bonus: config.battletech.tsm_tow_bonus != 0,
            tsm_sprint_bonus: config.battletech.tsm_sprint_bonus != 0,
        }
    }
}

/// Live inputs to one calculation, never a second saved copy of unit or pilot state.
#[derive(Clone, Copy, Default)]
pub(super) struct SpeedBonuses {
    pub masc: bool,
    pub supercharger: bool,
    pub sprint: bool,
    pub hot_myomer: bool,
    pub tsm_sprint_bonus: bool,
    pub speed_demon: bool,
}

impl SpeedBonuses {
    /// Boosts add to running MP; myomer rounding precedes the pilot's one-MP sprint bonus.
    pub fn apply(self, maximum: f64) -> Result<f32> {
        ensure!(
            maximum.is_finite() && maximum >= 0.0,
            "Invalid unloaded maximum speed"
        );
        let mut speed = maximum as f32;
        let boosts = u8::from(self.masc) + u8::from(self.supercharger) + u8::from(self.sprint);
        if boosts != 0 {
            speed = (2.0 * speed / 3.0) * (1.5 + 0.5 * f32::from(boosts));
        }
        if self.hot_myomer && (!self.sprint || self.tsm_sprint_bonus) {
            speed = (((speed / 1.5 / 10.75).round_ties_even() + 1.0) * 1.5).ceil() * 10.75;
        }
        if self.sprint && self.speed_demon {
            speed += 10.75;
        }
        ensure!(speed.is_finite(), "Invalid effective speed");
        Ok(speed)
    }
}

/// Apply enabled map gravity after chassis bonuses, preserving ordinary-map precision.
pub(super) fn on_map(
    world: &crate::World,
    position: Option<super::BattlePosition>,
    speed: f64,
) -> Result<f64> {
    ensure!(speed.is_finite() && speed >= 0.0, "Invalid effective speed");
    let map = position
        .and_then(|position| world.btech.maps().get(&position.map))
        .filter(|map| map.uses_special_rules() && map.gravity != 100);
    match map {
        Some(map) => gravity(speed as f32, Some(map.gravity)),
        None => Ok(speed),
    }
}

/// Environmental rules operate last and retain the reference's single-precision arithmetic.
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

    /// Three simultaneous boosts add; Speed Demon belongs only to sprint and precedes gravity.
    #[test]
    fn additive_boosts_pilot_and_gravity_order() {
        for mask in 0..8u8 {
            let bonus = SpeedBonuses {
                masc: mask & 1 != 0,
                supercharger: mask & 2 != 0,
                sprint: mask & 4 != 0,
                ..Default::default()
            };
            let expected = [64.5, 86.0, 107.5, 129.0][mask.count_ones() as usize];
            assert_eq!(bonus.apply(64.5).unwrap(), expected);
            let pilot = SpeedBonuses {
                speed_demon: true,
                ..bonus
            }
            .apply(64.5)
            .unwrap();
            assert_eq!(pilot, expected + if bonus.sprint { 10.75 } else { 0.0 });
            for (g, multiplier) in [(0, 2.0), (49, 2.0), (50, 2.0), (100, 1.0), (200, 0.5)] {
                assert_eq!(
                    gravity(pilot, Some(g)).unwrap(),
                    f64::from(pilot) * multiplier
                );
            }
        }
    }

    /// Configuration gates myomer only during sprint, and walking-MP ties round to even.
    #[test]
    fn hot_myomer_policy_and_rounding() {
        for enabled in [false, true] {
            let ordinary = SpeedBonuses {
                hot_myomer: true,
                tsm_sprint_bonus: enabled,
                ..Default::default()
            };
            assert_eq!(ordinary.apply(40.3125).unwrap(), 53.75); // 2.5 walking MP -> 2.
            assert_eq!(ordinary.apply(56.4375).unwrap(), 86.0); // 3.5 walking MP -> 4.
            let sprint = SpeedBonuses {
                sprint: true,
                speed_demon: true,
                ..ordinary
            };
            assert_eq!(
                sprint.apply(64.5).unwrap(),
                if enabled { 107.5 } else { 96.75 }
            );
        }
        assert!(SpeedBonuses::default().apply(f64::INFINITY).is_err());
        assert!(SpeedBonuses::default().apply(-1.0).is_err());
        assert!(gravity(1.0, Some(256)).is_err());
    }
}
