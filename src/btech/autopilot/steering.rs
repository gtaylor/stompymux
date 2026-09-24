//! Deterministic ground steering with route lookahead and ordinary motion forecasts.
use super::{navigation::Hex, observations::AutopilotObservation};
use crate::{
    BattleHexCoordinate, BattleMotion, BattleNotice, BattlePoint, BattlePosition, Config, ObjectId,
    World,
};
use anyhow::{Context, Result};

/// Transient direction latch and measurable control progress, never serialized.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SteeringState {
    pub reverse: bool,
    pub changed_at: i64,
    pub last_speed: f64,
    pub last_error: f64,
}

impl Default for SteeringState {
    fn default() -> Self {
        Self {
            reverse: false,
            changed_at: -4,
            last_speed: 0.0,
            last_error: 0.0,
        }
    }
}

pub(crate) fn motion(world: &World, id: ObjectId) -> Option<BattleMotion> {
    world
        .btech
        .constructed_units()
        .get(&id)
        .and_then(|u| u.motion())
        .or_else(|| world.btech.vehicles().get(&id).and_then(|u| u.motion()))
}

fn angle(left: f64, right: f64) -> f64 {
    ((left - right + 180.0).rem_euclid(360.0) - 180.0).abs()
}
fn center(hex: Hex) -> BattlePoint {
    BattleHexCoordinate {
        x: i32::from(hex.x),
        y: i32::from(hex.y),
    }
    .center()
}

/// Reuse the authoritative pure proposals; terrain entry and dice remain live-movement responsibilities.
fn project(
    world: &World,
    config: &Config,
    id: ObjectId,
    mut motion: BattleMotion,
    _map: crate::ObjectId,
) -> Result<BattleMotion> {
    let coordinate = motion.point.containing_hex()?;
    let rules = crate::BattleMovementRules {
        fasa_turning: config.battletech.fasaturn != 0,
        slowdown: config.battletech.slowdown,
        tsm_tow_bonus: config.battletech.tsm_tow_bonus != 0,
        tsm_sprint_bonus: config.battletech.tsm_sprint_bonus != 0,
        ..crate::BattleMovementRules::STANDARD
    };
    if world.btech.constructed_units().contains_key(&id) {
        let proposal =
            crate::btech::propose_mech_ground_motion(world, id, motion, coordinate, rules)?;
        motion = proposal.motion;
        motion.point = proposal.destination;
        Ok(motion)
    } else {
        crate::btech::propose_vehicle_ground_motion(world, id, motion, coordinate, rules)
    }
}

/// Validate every crossed hex; a lookahead segment cannot jump across blocked terrain.
fn safe_segment(
    world: &World,
    id: ObjectId,
    map: ObjectId,
    start: BattlePoint,
    end: BattlePoint,
) -> bool {
    let Ok(cells) = start.trace(end) else {
        return false;
    };
    let Ok(mut previous) = start.containing_hex() else {
        return false;
    };
    for cell in cells {
        let (Ok(x), Ok(y), Ok(px), Ok(py)) = (
            u16::try_from(cell.x),
            u16::try_from(cell.y),
            u16::try_from(previous.x),
            u16::try_from(previous.y),
        ) else {
            return false;
        };
        if (x, y) != (px, py)
            && !super::traversal::assess(
                world,
                id,
                BattlePosition { map, x: px, y: py },
                BattlePosition { map, x, y },
            )
            .eligible
        {
            return false;
        }
        previous = cell;
    }
    true
}

/// Select a bounded lookahead without leaving the planned route corridor.
fn lookahead(
    route: &[Hex],
    index: usize,
    point: BattlePoint,
    heading: f64,
    smooth: bool,
) -> Result<BattlePoint> {
    let mut goal = center(*route.get(index).context("Missing route waypoint")?);
    let mut previous = goal;
    let first = point.bearing(goal)?.unwrap_or(heading);
    for (offset, next) in route.iter().skip(index + 1).take(8).enumerate() {
        let next = center(*next);
        if smooth {
            let corridor = &route[index.saturating_sub(1)..=index + 1 + offset];
            if !point.trace(next)?.iter().all(|cell| {
                corridor
                    .iter()
                    .any(|h| i32::from(h.x) == cell.x && i32::from(h.y) == cell.y)
            }) {
                continue;
            }
        } else if angle(previous.bearing(next)?.unwrap_or(first), first) > 5.0 {
            break;
        }
        goal = next;
        previous = next;
    }
    Ok(goal)
}

/// Drive toward a route lookahead, reserving enough distance to stop at its end.
/// Returns true when braking or turning is making measurable progress.
pub(crate) fn drive(
    world: &mut World,
    config: &Config,
    id: ObjectId,
    route: &[Hex],
    index: usize,
    cap: f64,
    combat: Option<(&AutopilotObservation, BattlePosition)>,
    smooth_route: bool,
    state: &mut SteeringState,
    time: i64,
    notices: &mut Vec<BattleNotice>,
) -> Result<bool> {
    let current = motion(world, id).context("Missing steering motion")?;
    let map = crate::btech::scanner::scanner_unit(world, id)
        .and_then(|u| u.position)
        .context("Unplaced steering unit")?
        .map;
    let goal = lookahead(route, index, current.point, current.heading, smooth_route)?;
    let bearing = current.point.bearing(goal)?.unwrap_or(current.heading);
    let distance = current.point.range(goal)?;
    let mut reverse = false;
    if let Some((observation, target)) = combat {
        let target_point = BattleHexCoordinate {
            x: i32::from(target.x),
            y: i32::from(target.y),
        }
        .center();
        let target_bearing = current
            .point
            .bearing(target_point)?
            .unwrap_or(current.heading);
        let range = current.point.range(target_point)?;
        let torso = world
            .btech
            .constructed_units()
            .get(&id)
            .map_or(crate::BattleTorso::Center, |u| u.facing().torso);
        let turret = world
            .btech
            .vehicles()
            .get(&id)
            .and_then(|u| u.turret_heading());
        let turret_at = |heading: f64| {
            turret.map(|value| {
                if world.btech.vehicles()[&id].turret_locked()
                    || world.btech.vehicles()[&id].turret_jammed()
                {
                    (value + heading - current.heading).rem_euclid(360.0)
                } else {
                    target_bearing
                }
            })
        };
        let forward = super::alignment::score(
            world,
            id,
            observation,
            bearing,
            target_bearing,
            range,
            torso,
            turret_at(bearing),
        );
        let back = super::alignment::score(
            world,
            id,
            observation,
            (bearing + 180.0) % 360.0,
            target_bearing,
            range,
            torso,
            turret_at((bearing + 180.0) % 360.0),
        );
        reverse = back > forward + 1e-9
            || ((back - forward).abs() < 1e-9 && angle(current.heading, bearing) > 100.0);
        if reverse != state.reverse && time - state.changed_at < 4 {
            reverse = state.reverse;
        }
    }
    // A fresh/replanned controller can inherit reverse motion from ordinary controls.
    // Compare physical velocity too; a reset direction latch must not bypass braking.
    if current.speed.abs() > 0.1 && ((current.speed < 0.0) != reverse) {
        notices.push(crate::btech::motion::set_speed_autopilot(world, id, 0.0)?);
        let progress = current.speed.abs() + 0.01 < state.last_speed.abs();
        state.last_speed = current.speed;
        return Ok(progress);
    }
    if reverse != state.reverse {
        if current.speed.abs() > 0.1 {
            notices.push(crate::btech::motion::set_speed_autopilot(world, id, 0.0)?);
            let progress = current.speed.abs() + 0.01 < state.last_speed.abs();
            state.last_speed = current.speed;
            return Ok(progress);
        }
        state.reverse = reverse;
        state.changed_at = time;
    }
    let heading = (bearing + if reverse { 180.0 } else { 0.0 }).rem_euclid(360.0);
    let error = angle(current.heading, heading);
    let maximum = cap * if reverse { -2.0 / 3.0 } else { 1.0 };
    let mut desired = 0.0;
    {
        let _loadouts =
            crate::btech::loadout_context::LoadoutScope::participants(&world.btech, [id, id]);
        if error < 45.0 {
            for fraction in [1.0, 0.75, 0.5, 0.25, 0.125] {
                let mut simulated = current;
                simulated.desired_heading = heading;
                simulated.desired_speed = maximum * fraction;
                let mut traveled = 0.0;
                let mut stopped = false;
                let mut safe = true;
                for tick in 0..32 {
                    if tick > 0 {
                        simulated.desired_speed = 0.0;
                    }
                    let Ok(next) = project(world, config, id, simulated, map) else {
                        safe = false;
                        break;
                    };
                    traveled += simulated.point.range(next.point)?;
                    if traveled > distance + 0.03
                        || !safe_segment(world, id, map, simulated.point, next.point)
                    {
                        safe = false;
                        break;
                    }
                    simulated = next;
                    if simulated.speed.abs() < 0.01 {
                        stopped = true;
                        break;
                    }
                }
                if safe && stopped {
                    desired = maximum * fraction;
                    break;
                }
            }
        }
    }
    let progress =
        current.speed.abs() + 0.01 < state.last_speed.abs() || error + 0.1 < state.last_error;
    state.last_speed = current.speed;
    state.last_error = error;
    if angle(current.desired_heading, heading) > 0.1 {
        notices.push(crate::btech::motion::set_heading_autopilot(
            world, id, heading,
        )?);
    }
    if (current.desired_speed - desired).abs() > 0.01 {
        notices.push(crate::btech::motion::set_speed_autopilot(
            world, id, desired,
        )?);
    }
    Ok(progress)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn smoothing_follows_a_staircase_without_leaving_its_cells() {
        let start = center(Hex::new(2, 10));
        let end = center(Hex::new(8, 6));
        let route: Vec<_> = start
            .trace(end)
            .unwrap()
            .into_iter()
            .map(|cell| Hex::new(cell.x as u16, cell.y as u16))
            .collect();
        assert!(route.len() <= 10);
        assert_eq!(lookahead(&route, 1, start, 0.0, true).unwrap(), end);
        assert_ne!(lookahead(&route, 1, start, 0.0, false).unwrap(), end);
    }

    #[test]
    fn smoothing_cannot_cut_across_cells_outside_the_route() {
        let route = [
            Hex::new(2, 10),
            Hex::new(2, 9),
            Hex::new(2, 8),
            Hex::new(2, 7),
            Hex::new(3, 7),
            Hex::new(4, 7),
            Hex::new(5, 7),
        ];
        let start = center(route[0]);
        let goal = lookahead(&route, 1, start, 0.0, true).unwrap();
        assert_ne!(goal, center(*route.last().unwrap()));
        for cell in start.trace(goal).unwrap() {
            assert!(route.contains(&Hex::new(cell.x as u16, cell.y as u16)));
        }
    }

    #[test]
    fn smoothing_inspects_at_most_eight_additional_waypoints() {
        let route: Vec<_> = (0..40).map(|x| Hex::new(x, 10)).collect();
        let goal = lookahead(&route, 1, center(route[0]), 90.0, true).unwrap();
        assert!(goal.containing_hex().unwrap().x <= 9);
    }
}
