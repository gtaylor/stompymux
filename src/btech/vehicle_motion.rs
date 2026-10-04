//! Ground-vehicle motion proposals share turning and acceleration with the Mech motion engine.
use super::{BattleMotion, BattleVehicleMovement, BattleVehicleTemplate, Hex};
use anyhow::{Result, ensure};

/// Inputs independent of vehicle construction, supplied by world configuration and pilot state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BattleVehicleMotionRules {
    pub fasa_turning: bool,
    pub slowdown: i64,
    pub speed_demon: bool,
    /// Positive map movement percentage; nonpositive values use the ordinary rate.
    pub movement_modifier: i64,
}

impl BattleVehicleMotionRules {
    /// Ordinary ground rules without pilot advantages or map rate changes.
    pub const STANDARD: Self = Self {
        fasa_turning: false,
        slowdown: 2,
        speed_demon: false,
        movement_modifier: 0,
    };
}

impl BattleVehicleTemplate {
    /// Propose one second of intact vehicle motion on the supplied current hex.
    /// This does not admit terrain entry, resolve hazards or mutate world state. The
    /// movement adapter must trace the proposed segment and resolve each crossed hex.
    pub fn ground_motion_step(
        &self,
        motion: BattleMotion,
        hex: Hex,
        rules: BattleVehicleMotionRules,
    ) -> Result<BattleMotion> {
        self.motion_at_maximum(motion, hex, rules, self.max_speed)
    }

    /// Share motion arithmetic between intact proposals and damaged live vehicles.
    pub(super) fn motion_at_maximum(
        &self,
        mut motion: BattleMotion,
        hex: Hex,
        rules: BattleVehicleMotionRules,
        maximum: f64,
    ) -> Result<BattleMotion> {
        motion = self.control_at_maximum(motion, hex, rules, maximum)?;
        let rate = if rules.movement_modifier > 0 {
            rules.movement_modifier as f64 / 100.0
        } else {
            1.0
        };
        motion.point = motion.project_step(rate)?;
        motion.validate(
            super::speed_bonus::saved_limit(self.max_speed.max(maximum), false, false, false)
                + 10.75,
        )?;
        Ok(motion)
    }

    /// Shared heading and acceleration, before either ground or flight path projection.
    pub(super) fn control_at_maximum(
        &self,
        mut motion: BattleMotion,
        hex: Hex,
        rules: BattleVehicleMotionRules,
        maximum: f64,
    ) -> Result<BattleMotion> {
        ensure!(
            maximum.is_finite() && maximum >= 0.0,
            "Invalid vehicle maximum speed"
        );
        // Roads increase actual speed, while commands retain the chassis throttle envelope.
        motion.validate(
            super::speed_bonus::saved_limit(self.max_speed.max(maximum), false, false, false)
                + 10.75,
        )?;
        let retained =
            super::speed_bonus::saved_limit(self.max_speed.max(maximum), false, false, false);
        ensure!(
            motion.desired_speed >= -retained * 2.0 / 3.0 && motion.desired_speed <= retained,
            "Speed exceeds vehicle limits"
        );
        if self.movement == BattleVehicleMovement::Stationary || maximum == 0.0 {
            ensure!(!motion.active(), "Stationary vehicle cannot move");
            return Ok(motion);
        }
        let old_heading = motion.heading;
        // Paved-surface speed can exceed the turning maximum. Preserve the high-speed turn rule.
        if !rules.fasa_turning && motion.speed > maximum {
            let difference =
                (motion.desired_heading - motion.heading + 180.0).rem_euclid(360.0) - 180.0;
            let base = maximum / 10.75;
            let turn = if difference.abs() > base {
                base * (1.0 - maximum / motion.speed / 2.0)
            } else {
                base
            };
            motion.heading = (motion.heading + difference.clamp(-turn, turn)).rem_euclid(360.0);
        } else {
            motion.turn_toward(maximum, rules.fasa_turning, 1.0);
        }
        let mut target = motion.desired_speed.abs() / terrain_divisor(hex, self.movement);
        if (hex.is_road() || hex.has_bridge())
            && matches!(
                self.movement,
                BattleVehicleMovement::Tracked | BattleVehicleMovement::Wheeled
            )
        {
            target *= (maximum + 10.75) / maximum;
        }
        if motion.heading != old_heading {
            let remaining =
                ((motion.desired_heading - motion.heading + 180.0).rem_euclid(360.0) - 180.0).abs();
            target = super::motion_controls::turning_throttle(target, remaining, rules.slowdown);
        }
        target *= motion.desired_speed.signum();
        let acceleration = maximum / 20.0 * if rules.speed_demon { 1.25 } else { 1.0 };
        motion.accelerate(target, acceleration, maximum);
        motion.validate(
            super::speed_bonus::saved_limit(self.max_speed.max(maximum), false, false, false)
                + 10.75,
        )?;
        Ok(motion)
    }
}

impl super::BattleVehicle {
    /// Propose motion using the current damage-adjusted throttle envelope.
    pub fn ground_motion_step(
        &self,
        motion: BattleMotion,
        hex: Hex,
        rules: BattleVehicleMotionRules,
    ) -> Result<BattleMotion> {
        self.definition()
            .motion_at_maximum(motion, hex, rules, self.maximum_speed())
    }
}

/// Speed divisor a vehicle's hex imposes on its desired throttle.
fn terrain_divisor(hex: Hex, movement: BattleVehicleMovement) -> f64 {
    hex.ground_speed_divisor(movement == BattleVehicleMovement::Wheeled)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::Terrain;

    /// Sand halves only wheeled throttle; other movement types treat it as clear ground.
    #[test]
    fn sand_slows_only_wheeled_vehicles() {
        assert_eq!(
            terrain_divisor(Hex::new(Terrain::Sand, 0), BattleVehicleMovement::Wheeled),
            2.0
        );
        for movement in [BattleVehicleMovement::Tracked, BattleVehicleMovement::Hover] {
            assert_eq!(terrain_divisor(Hex::new(Terrain::Sand, 0), movement), 1.0);
        }
        assert_eq!(
            terrain_divisor(Hex::new(Terrain::Rough, 0), BattleVehicleMovement::Tracked),
            2.0
        );
    }
}
