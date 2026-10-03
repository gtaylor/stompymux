//! Read-only Mech ground proposals share live movement arithmetic with trajectory prediction.
use super::{BattleHexCoordinate, BattleMotion, BattleMovementRules, BattlePoint};
use crate::{ObjectId, World};
use anyhow::{Context, Result, ensure};

/// One second of control changes and its proposed endpoint, before terrain-entry consequences.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BattleGroundMotionProposal {
    /// Updated controls retain the starting point until the caller resolves the segment.
    pub motion: BattleMotion,
    pub destination: BattlePoint,
    /// A zero movement ceiling requires live damage reconciliation instead of traversal.
    pub immobilized: bool,
}

/// Calculate motion without consuming dice, applying damage, publishing notices or editing the world.
/// The supplied coordinate is the current terrain cell, allowing successive prediction steps.
/// Map entry, obstacles, collisions and hazards remain the traversal caller's responsibility.
pub fn propose_mech_ground_motion(
    world: &World,
    id: ObjectId,
    mut motion: BattleMotion,
    coordinate: BattleHexCoordinate,
    rules: BattleMovementRules,
) -> Result<BattleGroundMotionProposal> {
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
    let base = super::load::movement_maximum(
        world,
        id,
        unit.mobility().maximum_speed,
        rules.tsm_tow_bonus,
    )?;
    let effective =
        super::speed_bonus::on_map(world, Some(position), unit.movement_maximum_at(base))?;
    let maximum = unit.update_from_maximum(effective);
    if super::load::carries_load(world, id) {
        motion.limit_load(maximum, maximum);
    }
    if maximum == 0.0 {
        return Ok(BattleGroundMotionProposal {
            motion,
            destination: motion.point,
            immobilized: true,
        });
    }
    let turning_maximum = unit.turning_from_maximum(effective);
    let old_heading = motion.heading;
    motion.turn_toward(
        turning_maximum,
        rules.fasa_turning,
        unit.chassis().turn_multiplier(),
    );
    let divisor = current.ground_speed_divisor(false);
    let desired = motion.desired_speed;
    if unit.definition().has_triple_myomer() && motion.desired_speed >= maximum {
        motion.desired_speed = maximum;
    }
    let mut target = desired * unit.movement_heat_multiplier(maximum) / divisor;
    if motion.heading != old_heading {
        let remaining = (motion.desired_heading - motion.heading + 180.0).rem_euclid(360.0) - 180.0;
        target = super::motion_controls::turning_throttle(target, remaining, rules.slowdown);
    }
    target = unit.lateral_speed_at(target, maximum);
    let acceleration = unit.ground_acceleration_at(world, maximum);
    motion.accelerate(target, acceleration, maximum);
    let rate = if map.movement_modifier > 0 {
        map.movement_modifier as f64 / 100.0
    } else {
        1.0
    };
    // Beyond the battlefield diagonal every path must already cross a map edge.
    let distance =
        (motion.speed.abs() / 645.0 * rate).min((map.width as f64).hypot(map.height as f64) + 2.0);
    let proposed = motion.point.project(
        motion.heading
            + f64::from(unit.lateral().active.offset())
            + if motion.speed < 0.0 { 180.0 } else { 0.0 },
        distance,
    )?;
    Ok(BattleGroundMotionProposal {
        motion,
        destination: proposed,
        immobilized: false,
    })
}

/// Propose loaded ground-vehicle motion using the same controls as live traversal.
pub fn propose_vehicle_ground_motion(
    world: &World,
    id: ObjectId,
    old: BattleMotion,
    coordinate: BattleHexCoordinate,
    rules: BattleMovementRules,
) -> Result<BattleMotion> {
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
    let maximum =
        super::load::movement_maximum(world, id, vehicle.maximum_speed(), rules.tsm_tow_bonus)?;
    let maximum = super::speed_bonus::on_map(world, Some(position), maximum)?;
    let mut loaded = old;
    if super::load::carries_load(world, id) {
        loaded.limit_load(
            maximum,
            if maximum > 0.0 && (current.is_road() || current.has_bridge()) {
                maximum + 10.75
            } else {
                maximum
            },
        );
    }
    let mut next = vehicle.definition().motion_at_maximum(
        loaded,
        if vehicle.definition().is_vtol() {
            super::BattleHex::at_level(0)
        } else {
            map.hex(i64::from(coordinate.x), i64::from(coordinate.y))?
        },
        super::BattleVehicleMotionRules {
            fasa_turning: rules.fasa_turning,
            slowdown: rules.slowdown,
            speed_demon: advantage,
            movement_modifier: map.movement_modifier,
        },
        maximum,
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
