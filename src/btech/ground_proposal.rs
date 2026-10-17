//! Read-only Mech ground proposals share live movement arithmetic with trajectory prediction.
use super::{HexCoordinate, Motion, MovementRules, Point, VehicleTemplateMotion};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// One second of control changes and its proposed endpoint, before terrain-entry consequences.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundMotionProposal {
    /// Updated controls retain the starting point until the caller resolves the segment.
    pub motion: Motion,
    pub destination: Point,
    /// A zero movement ceiling requires live damage reconciliation instead of traversal.
    pub immobilized: bool,
}

/// Calculate motion without consuming dice, applying damage, publishing notices or editing the world.
/// The supplied coordinate is the current terrain cell, allowing successive prediction steps.
/// Map entry, obstacles, collisions and hazards remain the traversal caller's responsibility.
pub fn propose_mech_ground_motion(
    world: &World,
    id: ObjectId,
    mut motion: Motion,
    coordinate: HexCoordinate,
    rules: MovementRules,
) -> Result<GroundMotionProposal> {
    let unit = world
        .btech
        .constructed_units()
        .get(&id)
        .context("Mech is unavailable")?;
    let position = unit.position().context("Mech is not placed")?;
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map is unavailable")?;
    ensure!(
        [
            motion.heading,
            motion.desired_heading,
            motion.speed,
            motion.desired_speed
        ]
        .iter()
        .all(|v| v.is_finite()),
        "Invalid unit motion"
    );
    motion.point.range(motion.point)?;
    let current = map.hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let policy = super::SpeedPolicy {
        tsm_tow_bonus: rules.tsm_tow_bonus,
    };
    let envelope = super::motion_controls::throttle_envelope(world, id, policy)?;
    let maximum = envelope.running;
    if super::load::carries_load(world, id) {
        motion.limit_to(envelope);
    }
    if maximum == 0.0 {
        return Ok(GroundMotionProposal {
            motion,
            destination: motion.point,
            immobilized: true,
        });
    }
    let old_heading = motion.heading;
    motion.turn_toward(envelope.running, envelope.walking);
    // Fog adds to the cost of every hex a walker enters (Tactical Operations p. 55), so does
    // the building it walks through (Total Warfare p. 167), and so does every level it
    // climbed or descended to get in (TW pp. 48 and 52).
    let divisor = current.ground_speed_divisor(super::GroundMovement::Legged)
        + f64::from(map.weather_condition()?.movement_cost())
        + f64::from(unit.level_entry_cost())
        + f64::from(super::building_interior::floor_cost_at(
            current,
            unit.elevation_level(current),
        ));
    // Heat, damage and equipment cap the throttle rather than scaling it: a request within
    // the current envelope is honoured as made (Total Warfare p. 159).
    let mut target = envelope.clamp(motion.desired_speed) / divisor;
    if motion.heading != old_heading {
        let remaining = (motion.desired_heading - motion.heading + 180.0).rem_euclid(360.0) - 180.0;
        target = super::motion_controls::turning_throttle(target, remaining);
    }
    target = unit.lateral_speed_at(target, maximum);
    // Heat caps the throttle without slowing how quickly the unit reaches it.
    let acceleration = unit.ground_acceleration_at(
        world,
        super::motion_controls::acceleration_ceiling(world, id, policy)?,
    );
    motion.accelerate(target, acceleration, maximum);
    // Beyond the battlefield diagonal every path must already cross a map edge.
    let distance =
        (motion.speed.abs() / 645.0).min((map.width as f64).hypot(map.height as f64) + 2.0);
    let proposed = motion.point.project(
        motion.heading
            + f64::from(unit.lateral().active.offset())
            + if motion.speed < 0.0 { 180.0 } else { 0.0 },
        distance,
    )?;
    Ok(GroundMotionProposal {
        motion,
        destination: proposed,
        immobilized: false,
    })
}

/// Propose loaded ground-vehicle motion using the same controls as live traversal.
pub fn propose_vehicle_ground_motion(
    world: &World,
    id: ObjectId,
    old: Motion,
    coordinate: HexCoordinate,
    rules: MovementRules,
) -> Result<Motion> {
    let vehicle = world
        .btech
        .vehicles()
        .get(&id)
        .context("Vehicle is unavailable")?;
    let position = vehicle.position().context("Vehicle is not placed")?;
    let map = world
        .btech
        .maps()
        .get(&position.map)
        .context("Map is unavailable")?;
    let current = map.base_hex(i64::from(coordinate.x), i64::from(coordinate.y))?;
    let advantage = vehicle
        .pilot()
        .is_some_and(|pilot| super::skills::boolean_advantage(world, pilot, "Speed_Demon"));
    let envelope = super::motion_controls::throttle_envelope(
        world,
        id,
        super::SpeedPolicy {
            tsm_tow_bonus: rules.tsm_tow_bonus,
        },
    )?;
    let mut loaded = old;
    if super::load::carries_load(world, id) {
        loaded.limit_load(
            envelope,
            if !envelope.is_stopped() && (current.is_road() || current.has_bridge()) {
                super::vehicle_speed::paved_headroom(envelope)
            } else {
                envelope
            },
        );
    }
    let mut next = vehicle.definition().motion_within(
        loaded,
        if vehicle.definition().is_vtol() {
            super::Hex::at_level(0)
        } else {
            map.hex(i64::from(coordinate.x), i64::from(coordinate.y))?
        },
        super::VehicleMotionRules {
            speed_demon: advantage,
            extra_cost: map.weather_condition()?.movement_cost()
                + vehicle.level_entry_cost()
                + super::building_interior::floor_cost_at(
                    current,
                    vehicle.elevation_level(current),
                ),
        },
        envelope,
    )?;
    let distance = old.point.range(next.point)?;
    let limit = (map.width as f64).hypot(map.height as f64) + 2.0;
    if distance > limit {
        next.point = old.point.project(
            next.heading + if next.speed < 0.0 { 180.0 } else { 0.0 },
            limit,
        )?;
    }
    Ok(next)
}
