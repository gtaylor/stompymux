//! Chassis and pilot adjustments shared by ground acceleration and lateral travel.
use super::{LateralMode, Mech, MechChassis};
use crate::World;

impl Mech {
    /// Per-second speed change in either direction, including the assigned pilot's Speed Demon.
    pub fn ground_acceleration(&self, world: &World) -> f64 {
        self.ground_acceleration_at(world, self.movement_maximum_speed())
    }

    /// Pilot and chassis acceleration applied to a supplied movement ceiling.
    pub(super) fn ground_acceleration_at(&self, world: &World, maximum: f64) -> f64 {
        let steps = if self.chassis() == MechChassis::Quad {
            10.0
        } else {
            20.0
        };
        let advantage = self
            .pilot()
            .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Speed_Demon"));
        maximum / steps * if advantage { 1.25 } else { 1.0 }
    }

    /// Movement points a lateral shift adds to every hex a quad enters: one, on top of the
    /// terrain (Total Warfare p. 50). Like any movement cost it is one more hex-equivalent in
    /// the speed divisor.
    pub fn lateral_cost(&self) -> f64 {
        if self.chassis() != MechChassis::Quad || self.lateral().active == LateralMode::None {
            return 0.0;
        }
        1.0
    }
}
