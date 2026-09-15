//! Chassis and pilot adjustments shared by ground acceleration and lateral travel.
use super::{BattleLateralMode, BattleMechChassis, BattleUnit};
use crate::World;

impl BattleUnit {
    /// Per-second speed change in either direction, including the assigned pilot's Speed Demon.
    pub fn ground_acceleration(&self, world: &World) -> f64 {
        self.ground_acceleration_at(world, self.update_maximum_speed())
    }

    /// Pilot and chassis acceleration applied to a supplied movement ceiling.
    pub(super) fn ground_acceleration_at(&self, world: &World, maximum: f64) -> f64 {
        let steps = if self.chassis() == BattleMechChassis::Quad {
            10.0
        } else {
            20.0
        };
        let advantage = self
            .pilot()
            .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Speed_Demon"));
        maximum / steps * if advantage { 1.25 } else { 1.0 }
    }

    /// Quad lateral travel subtracts one MP proportionally after terrain, heat and turning losses.
    /// Reverse movement retains its sign; a nonpositive remaining maximum stops lateral travel.
    pub fn lateral_speed(&self, target: f64) -> f64 {
        self.lateral_speed_at(target, self.update_maximum_speed())
    }

    /// Lateral movement uses the loaded ceiling after equipment bonuses.
    pub(super) fn lateral_speed_at(&self, target: f64, maximum: f64) -> f64 {
        if self.chassis() != BattleMechChassis::Quad
            || self.lateral().active == BattleLateralMode::None
        {
            return target;
        }
        if maximum <= 10.75 {
            return 0.0;
        }
        target * (maximum - 10.75) / maximum
    }
}
