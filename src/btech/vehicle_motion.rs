//! Ground-vehicle motion proposals share turning and acceleration with the Mech motion engine.
use super::{GroundMovement, Hex, Motion, SpeedEnvelope, VehicleMovement, VehicleTemplate};
use anyhow::{Result, ensure};

/// Inputs independent of vehicle construction, supplied by world configuration and pilot state.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VehicleMotionRules {
    pub speed_demon: bool,
    /// Movement points the battlefield adds to every hex entered beyond the terrain's own:
    /// fog slows everything that drives (Tactical Operations p. 55), a building's floors
    /// cost by its class (Total Warfare p. 167), and each level climbed or descended to enter
    /// the hex costs two (TW pp. 48, 52).
    pub extra_cost: u8,
}

impl VehicleMotionRules {
    /// Ordinary ground rules without pilot advantages or battlefield movement costs.
    pub const STANDARD: Self = Self {
        speed_demon: false,
        extra_cost: 0,
    };
}

/// One-second motion proposals for intact and damaged vehicles.
pub trait VehicleTemplateMotion {
    /// Propose one second of intact vehicle motion on the supplied current hex.
    /// This does not admit terrain entry, resolve hazards or mutate world state. The
    /// movement adapter must trace the proposed segment and resolve each crossed hex.
    fn ground_motion_step(
        &self,
        motion: Motion,
        hex: Hex,
        rules: VehicleMotionRules,
    ) -> Result<Motion>;

    /// Share motion arithmetic between intact proposals and damaged live vehicles, inside
    /// a cruising and flanking `envelope`.
    fn motion_within(
        &self,
        motion: Motion,
        hex: Hex,
        rules: VehicleMotionRules,
        envelope: SpeedEnvelope,
    ) -> Result<Motion>;

    /// Shared heading and acceleration, before either ground or flight path projection.
    fn control_within(
        &self,
        motion: Motion,
        hex: Hex,
        rules: VehicleMotionRules,
        envelope: SpeedEnvelope,
    ) -> Result<Motion>;
}

/// The widest envelope saved controls may retain for a vehicle built as `template` and now
/// moving inside `envelope`: the larger of either under the lightest gravity the rules allow.
fn retained(template: &VehicleTemplate, envelope: SpeedEnvelope) -> SpeedEnvelope {
    let construction = construction_envelope(template);
    SpeedEnvelope {
        walking: super::speed_bonus::saved_limit(construction.walking.max(envelope.walking)),
        running: super::speed_bonus::saved_limit(construction.running.max(envelope.running)),
    }
}

/// The intact envelope of a template: its cruising MP and the flank speed derived from it
/// (Total Warfare p. 53).
fn construction_envelope(template: &VehicleTemplate) -> SpeedEnvelope {
    SpeedEnvelope::from_walking_mp(f64::from(template.walk_mp))
}

impl VehicleTemplateMotion for VehicleTemplate {
    fn ground_motion_step(
        &self,
        motion: Motion,
        hex: Hex,
        rules: VehicleMotionRules,
    ) -> Result<Motion> {
        self.motion_within(motion, hex, rules, construction_envelope(self))
    }

    fn motion_within(
        &self,
        mut motion: Motion,
        hex: Hex,
        rules: VehicleMotionRules,
        envelope: SpeedEnvelope,
    ) -> Result<Motion> {
        motion = self.control_within(motion, hex, rules, envelope)?;
        motion.point = motion.project_step()?;
        motion.validate_within(super::vehicle_speed::paved_headroom(retained(
            self, envelope,
        )))?;
        Ok(motion)
    }

    fn control_within(
        &self,
        mut motion: Motion,
        hex: Hex,
        rules: VehicleMotionRules,
        envelope: SpeedEnvelope,
    ) -> Result<Motion> {
        envelope.validate()?;
        let maximum = envelope.running;
        // Roads increase actual speed, while commands retain the chassis throttle envelope.
        let retained = retained(self, envelope);
        motion.validate_within(super::vehicle_speed::paved_headroom(retained))?;
        ensure!(
            motion.desired_speed >= -retained.walking && motion.desired_speed <= retained.running,
            "Speed exceeds vehicle limits"
        );
        if self.movement == VehicleMovement::Stationary || maximum == 0.0 {
            ensure!(!motion.active(), "Stationary vehicle cannot move");
            return Ok(motion);
        }
        let old_heading = motion.heading;
        // Paved-surface speed can exceed the turning maximum. Preserve the high-speed turn rule.
        if motion.speed > maximum {
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
            motion.turn_toward(maximum, envelope.walking);
        }
        let mut target = motion.desired_speed.abs()
            / (terrain_divisor(hex, self.movement, self.has_special("Waterproof_Tech"))
                + f64::from(rules.extra_cost));
        // Ground vehicles on pavement, a road or a bridge deck gain a point of movement
        // (Total Warfare p. 61).
        if hex.is_paved() && self.movement != VehicleMovement::Vtol {
            target *= super::vehicle_speed::paved_bonus(envelope);
        }
        if motion.heading != old_heading {
            let remaining =
                ((motion.desired_heading - motion.heading + 180.0).rem_euclid(360.0) - 180.0).abs();
            target = super::motion_controls::turning_throttle(target, remaining);
        }
        target *= motion.desired_speed.signum();
        let acceleration = maximum / 20.0 * if rules.speed_demon { 1.25 } else { 1.0 };
        motion.accelerate(target, acceleration, maximum);
        motion.validate_within(super::vehicle_speed::paved_headroom(retained))?;
        Ok(motion)
    }
}

impl super::Vehicle {
    /// Propose motion using the current damage-adjusted throttle envelope.
    pub fn ground_motion_step(
        &self,
        motion: Motion,
        hex: Hex,
        rules: VehicleMotionRules,
    ) -> Result<Motion> {
        self.definition()
            .motion_within(motion, hex, rules, self.speed_envelope())
    }
}

/// Movement points an amphibious vehicle spends to enter any water hex, which it crosses on
/// the surface whatever the depth (Total Warfare p. 52 note 7, p. 56).
const AMPHIBIOUS_WATER_COST: f64 = 2.0;

/// Speed divisor a vehicle's hex imposes on its desired throttle. An amphibious tracked or
/// wheeled vehicle rides the surface of open water at a flat two points per hex instead of
/// wading or crawling along the bed.
fn terrain_divisor(hex: Hex, movement: VehicleMovement, amphibious: bool) -> f64 {
    if amphibious
        && hex.is_open_water()
        && matches!(
            movement,
            VehicleMovement::Tracked | VehicleMovement::Wheeled
        )
    {
        return AMPHIBIOUS_WATER_COST;
    }
    hex.ground_speed_divisor(ground_movement(movement))
}

/// The terrain cost column a vehicle's motive system uses; fixed installations and grounded
/// VTOLs count as tracked.
pub(super) fn ground_movement(movement: VehicleMovement) -> GroundMovement {
    match movement {
        VehicleMovement::Wheeled => GroundMovement::Wheeled,
        VehicleMovement::Hover => GroundMovement::Hover,
        VehicleMovement::Tracked | VehicleMovement::Stationary | VehicleMovement::Vtol => {
            GroundMovement::Tracked
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::btech::Terrain;

    /// Sand halves only wheeled throttle; other movement types treat it as clear ground.
    #[test]
    fn sand_slows_only_wheeled_vehicles() {
        assert_eq!(
            terrain_divisor(Hex::new(Terrain::Sand, 0), VehicleMovement::Wheeled, false),
            2.0
        );
        for movement in [VehicleMovement::Tracked, VehicleMovement::Hover] {
            assert_eq!(
                terrain_divisor(Hex::new(Terrain::Sand, 0), movement, false),
                1.0
            );
        }
        assert_eq!(
            terrain_divisor(Hex::new(Terrain::Rough, 0), VehicleMovement::Tracked, false),
            2.0
        );
    }

    /// An amphibious tank crosses any depth of water on the surface at two points per hex
    /// (Total Warfare p. 52 note 7); a hovercraft already skims it for one.
    #[test]
    fn amphibious_vehicles_cross_water_on_the_surface_at_two_points() {
        for depth in [1, 2, 4] {
            let water = Hex::new(Terrain::Water, depth);
            assert_eq!(terrain_divisor(water, VehicleMovement::Tracked, true), 2.0);
            assert_eq!(terrain_divisor(water, VehicleMovement::Wheeled, true), 2.0);
            assert_eq!(terrain_divisor(water, VehicleMovement::Hover, true), 1.0);
            assert_eq!(
                terrain_divisor(water, VehicleMovement::Tracked, false),
                if depth == 1 { 2.0 } else { 4.0 }
            );
        }
        assert_eq!(
            terrain_divisor(Hex::new(Terrain::Clear, 0), VehicleMovement::Tracked, true),
            1.0
        );
    }
}
