//! Pure mount-arc scoring and admitted torso/turret alignment for observed targets.
use super::observations::AutopilotObservation;
use crate::btech::{BattleMountArcs, BattleVehicleMountArcs};
use crate::{BattleNotice, BattlePosition, BattleTorso, ObjectId, World};

/// Persistent mount capability ignores temporary recycle/heat to avoid maneuver oscillation.
pub(crate) fn available(weapon: &crate::BattleWeaponReadiness) -> bool {
    weapon.intact
        && !weapon.spent
        && !weapon.jammed
        && !weapon.weapon.is_ams()
        && !weapon.weapon.is_artillery()
        && (weapon.weapon.profile().ammunition_per_ton == 0 || weapon.ammunition > 0)
}

/// Estimated effectiveness shares range factors with the pure target selection policy.
pub(crate) fn effectiveness(weapon: crate::BattleWeapon, range: f64) -> f64 {
    let p = weapon.profile();
    if range > f64::from(p.long_range) {
        return 0.0;
    }
    f64::from(p.damage)
        * f64::from(p.missiles.max(1))
        * if range <= f64::from(p.short_range) {
            1.0
        } else if range <= f64::from(p.medium_range) {
            0.66
        } else {
            0.33
        }
        * if range < f64::from(p.minimum_range) {
            0.5
        } else {
            1.0
        }
}

/// Score a hypothetical own hull heading without mutating a unit or inspecting enemy capabilities.
pub(crate) fn score(
    world: &World,
    id: ObjectId,
    observation: &AutopilotObservation,
    heading: f64,
    bearing: f64,
    range: f64,
    torso: BattleTorso,
    turret: Option<f64>,
) -> f64 {
    if let Some(unit) = world.btech.constructed_units().get(&id) {
        let Ok(loadout) = unit.loadout() else {
            return 0.0;
        };
        let mut facing = unit.facing();
        facing.torso = torso;
        return loadout
            .weapons
            .iter()
            .zip(&observation.own.weapons)
            .filter(|(mount, ready)| {
                available(ready)
                    && mount
                        .bears_on(unit.chassis(), heading, bearing, facing)
                        .unwrap_or(false)
            })
            .map(|(mount, _)| effectiveness(mount.weapon, range))
            .sum();
    }
    let Some(unit) = world.btech.vehicles().get(&id) else {
        return 0.0;
    };
    let Ok(loadout) = unit.loadout() else {
        return 0.0;
    };
    loadout
        .weapons
        .iter()
        .zip(&observation.own.weapons)
        .filter(|(mount, ready)| {
            available(ready) && mount.bears_on(heading, bearing, turret).unwrap_or(false)
        })
        .map(|(mount, _)| effectiveness(mount.weapon, range))
        .sum()
}

/// Align only legal own controls. Rejected commands retain their ordinary no-takeover semantics.
pub(crate) fn align(
    world: &mut World,
    id: ObjectId,
    target: BattlePosition,
    observation: &AutopilotObservation,
    allow_hull: bool,
    notices: &mut Vec<BattleNotice>,
) {
    let Some(scanner) = crate::btech::scanner::scanner_unit(world, id) else {
        return;
    };
    let Some(point) = scanner.point else { return };
    let target_point = crate::HexCoordinate {
        x: i32::from(target.x),
        y: i32::from(target.y),
    }
    .center();
    let bearing = point.bearing(target_point).ok().flatten().unwrap_or(0.0);
    let range = point.range(target_point).unwrap_or(f64::MAX);
    let heading = scanner.heading.unwrap_or(0.0);
    let current_torso = world
        .btech
        .constructed_units()
        .get(&id)
        .map_or(BattleTorso::Center, |u| u.facing().torso);
    let current_turret = world
        .btech
        .vehicles()
        .get(&id)
        .and_then(|u| u.turret_heading());
    let can_twist = world.btech.constructed_units().get(&id).is_some_and(|u| {
        u.chassis() != crate::BattleMechChassis::Quad && u.posture() != crate::BattlePosture::Prone
    });
    let can_turret =
        world.btech.vehicles().get(&id).is_some_and(|u| {
            u.turret_heading().is_some() && !u.turret_locked() && !u.turret_jammed()
        });
    let turret = if can_turret {
        Some(bearing.round().rem_euclid(360.0))
    } else {
        current_turret
    };
    let mut best = (
        heading,
        current_torso,
        score(
            world,
            id,
            observation,
            heading,
            bearing,
            range,
            current_torso,
            turret,
        ),
    );
    let headings = [
        heading,
        bearing,
        (bearing + 90.0) % 360.0,
        (bearing + 180.0) % 360.0,
        (bearing + 270.0) % 360.0,
    ];
    for next in headings.into_iter().take(if allow_hull { 5 } else { 1 }) {
        for torso in [
            current_torso,
            BattleTorso::Center,
            BattleTorso::Left,
            BattleTorso::Right,
        ]
        .into_iter()
        .take(if can_twist { 4 } else { 1 })
        {
            let rotated_turret = if can_turret {
                turret
            } else {
                turret.map(|t| (t + next - heading).rem_euclid(360.0))
            };
            let value = score(
                world,
                id,
                observation,
                next,
                bearing,
                range,
                torso,
                rotated_turret,
            );
            if value > best.2 + 1e-9 {
                best = (next, torso, value);
            }
        }
    }
    if best.0 != heading
        && let Ok(notice) = crate::btech::motion::set_heading_autopilot(world, id, best.0)
    {
        notices.push(notice);
    }
    if best.1 != current_torso {
        let direction = if matches!(
            (current_torso, best.1),
            (BattleTorso::Left, BattleTorso::Right) | (BattleTorso::Right, BattleTorso::Left)
        ) {
            BattleTorso::Center
        } else {
            best.1
        };
        if let Ok(notice) = crate::btech::arcs::rotate_torso_autopilot(world, id, direction) {
            notices.push(notice);
        }
    }
    if can_turret
        && turret != current_turret
        && let Ok(notice) =
            crate::btech::vehicle_turret::set_turret_autopilot(world, id, turret.unwrap())
    {
        notices.push(notice);
    }
}
