//! Deterministic ground steering with route lookahead and ordinary motion forecasts.
use super::{navigation::Hex, observations::AutopilotObservation};
use crate::{
    BattleMotion, BattleNotice, BattlePosition, Config, HexCoordinate, ObjectId, Point, World,
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
fn center(hex: Hex) -> Point {
    HexCoordinate {
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
fn safe_segment(world: &World, id: ObjectId, map: ObjectId, start: Point, end: Point) -> bool {
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
    point: Point,
    heading: f64,
    smooth: bool,
) -> Result<Point> {
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

/// Pure direction choice shared by execution and bounded candidate forecasts.
fn direction(
    world: &World,
    id: ObjectId,
    current: BattleMotion,
    bearing: f64,
    combat: Option<(&AutopilotObservation, BattlePosition)>,
    pursuit: Option<((f64, f64), f64)>,
    state: &SteeringState,
    time: i64,
) -> Result<bool> {
    let mut reverse = false;
    if let Some((observation, target)) = combat {
        let target_point = HexCoordinate {
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
        if let Some((velocity, map_speed)) = pursuit {
            let delta = (
                target_point.x - current.point.x,
                target_point.y - current.point.y,
            );
            let radial = (velocity.0 * delta.0 + velocity.1 * delta.1) / range.max(1e-9);
            if radial > map_speed * (2.0 / 3.0) {
                reverse = false;
            }
        }
        if reverse != state.reverse && time - state.changed_at < 4 {
            reverse = state.reverse;
        }
    }
    Ok(reverse)
}

/// Brake before changing physical direction, then apply the ordinary reverse cap.
fn direction_speed(speed: f64, reverse: bool, cap: f64) -> f64 {
    if speed.abs() > 0.1 && (speed < 0.0) != reverse {
        0.0
    } else {
        cap * if reverse { -2.0 / 3.0 } else { 1.0 }
    }
}

/// Evaluate mount bearing at a supplied own motion without reading target capabilities.
pub(crate) fn actual_arc(
    world: &World,
    id: ObjectId,
    observation: &AutopilotObservation,
    motion: BattleMotion,
    target: Point,
) -> bool {
    let Ok(range) = motion.point.range(target) else {
        return false;
    };
    let Ok(bearing) = motion.point.bearing(target) else {
        return false;
    };
    let torso = world
        .btech
        .constructed_units()
        .get(&id)
        .map_or(crate::BattleTorso::Center, |u| u.facing().torso);
    let turret = world.btech.vehicles().get(&id).and_then(|u| {
        u.turret_heading().map(|heading| {
            if u.turret_locked() || u.turret_jammed() {
                (heading + motion.heading - u.motion().map_or(motion.heading, |m| m.heading))
                    .rem_euclid(360.0)
            } else {
                heading
            }
        })
    });
    super::alignment::score(
        world,
        id,
        observation,
        motion.heading,
        bearing.unwrap_or(motion.heading),
        range,
        torso,
        turret,
    ) > 0.0
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
    pursuit: Option<((f64, f64), f64)>,
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
    let reverse = direction(world, id, current, bearing, combat, pursuit, state, time)?;
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
    let maximum = direction_speed(current.speed, reverse, cap);
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

/// Forecast at most sixteen ordinary own-motion proposals; no state or dice are changed.
/// The remaining-distance estimate is a navigation score, never weapon admission.
pub(crate) fn pursuit_score(
    world: &World,
    config: &Config,
    id: ObjectId,
    aim: BattlePosition,
    observed: BattlePosition,
    velocity: (f64, f64),
    uncertainty: f64,
    cap: f64,
    speed: f64,
    radius: u16,
    context: Option<(&AutopilotObservation, &SteeringState, i64)>,
) -> Option<(f64, f64)> {
    let _timing = super::diagnostics::pursuit("score");
    if speed <= 0.0 {
        return None;
    }
    let mut simulated = motion(world, id)?;
    let mut state = context.map_or_else(SteeringState::default, |c| c.1.clone());
    let goal = center(Hex::new(aim.x, aim.y));
    let target = center(Hex::new(observed.x, observed.y));
    let mut elapsed = 0.0;
    let mut turning: f64 = 0.0;
    for _ in 0..16 {
        let range = simulated.point.range(goal).ok()?;
        let forecast_target = Point {
            x: target.x + velocity.0 * elapsed,
            y: target.y + velocity.1 * elapsed,
        };
        if range <= f64::from(radius)
            && context.is_none_or(|c| actual_arc(world, id, c.0, simulated, forecast_target))
        {
            break;
        }
        let bearing = simulated
            .point
            .bearing(goal)
            .ok()?
            .unwrap_or(simulated.heading);
        let time = context.map_or(0, |c| c.2) + elapsed as i64;
        let reverse = direction(
            world,
            id,
            simulated,
            bearing,
            context.map(|c| (c.0, observed)),
            Some((velocity, speed)),
            &state,
            time,
        )
        .ok()?;
        if reverse != state.reverse && simulated.speed.abs() <= 0.1 {
            state.reverse = reverse;
            state.changed_at = time;
        }
        let heading = (bearing + if reverse { 180.0 } else { 0.0 }).rem_euclid(360.0);
        simulated.desired_heading = heading;
        simulated.desired_speed =
            if range <= f64::from(radius) || angle(simulated.heading, heading) >= 45.0 {
                0.0
            } else if range - f64::from(radius) < speed * 2.0 {
                direction_speed(simulated.speed, reverse, cap) * 0.25
            } else {
                direction_speed(simulated.speed, reverse, cap)
            };
        let previous_heading = simulated.heading;
        simulated = {
            let _timing = super::diagnostics::pursuit("score_projection");
            project(world, config, id, simulated, aim.map).ok()?
        };
        turning = turning.max(angle(previous_heading, simulated.heading));
        elapsed += 1.0;
    }
    let remaining_range = simulated.point.range(goal).ok()?;
    let approach = (remaining_range - f64::from(radius)).max(0.0) / speed;
    let arrival = elapsed + approach;
    // Arrival is at the goal region's edge; do not credit unmodeled travel to its center.
    let fraction = if remaining_range > 0.0 {
        approach * speed / remaining_range
    } else {
        0.0
    };
    let terminal = Point {
        x: simulated.point.x + (goal.x - simulated.point.x) * fraction,
        y: simulated.point.y + (goal.y - simulated.point.y) * fraction,
    };
    let future = Point {
        x: target.x + velocity.0 * arrival.min(120.0),
        y: target.y + velocity.1 * arrival.min(120.0),
    };
    let residual = remaining_pursuit_seconds(terminal, future, velocity, speed, f64::from(radius))?;
    // Remaining alignment is an estimate from the same bounded physical forecast.
    // No extra proposals, weapon readiness checks, or combat dice are consumed.
    let alignment = if let Some((observation, _, _)) = context
        && !actual_arc(
            world,
            id,
            observation,
            BattleMotion {
                point: terminal,
                ..simulated
            },
            future,
        ) {
        let bearing = terminal.bearing(future).ok()?.unwrap_or(simulated.heading);
        let heading = (bearing + if state.reverse { 180.0 } else { 0.0 }).rem_euclid(360.0);
        (angle(simulated.heading, heading) - 45.0).max(0.0) / turning.max(1.0)
    } else {
        0.0
    };
    let stopped = segment_entry(simulated.point, terminal, target, f64::from(radius))
        .map(|fraction| elapsed + approach * fraction)
        .unwrap_or(arrival + (terminal.range(target).ok()? - f64::from(radius)).max(0.0) / speed);
    Some((
        arrival + residual + alignment + uncertainty / speed,
        stopped,
    ))
}

/// First entry into a stationary firing region along a bounded straight segment.
fn segment_entry(start: Point, end: Point, target: Point, radius: f64) -> Option<f64> {
    let x = start.x - target.x;
    let y = start.y - target.y;
    let c = x * x + y * y - radius * radius;
    if c <= 0.0 {
        return Some(0.0);
    }
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let a = dx * dx + dy * dy;
    if a <= 1e-12 {
        return None;
    }
    let b = 2.0 * (x * dx + y * dy);
    let discriminant = b * b - 4.0 * a * c;
    if discriminant < 0.0 {
        return None;
    }
    let t = (-b - discriminant.sqrt()) / (2.0 * a);
    (0.0..=1.0).contains(&t).then_some(t)
}

/// Constant-speed pure-pursuit closure estimate after the bounded physical forecast.
/// This is an open-terrain estimate, not a route-optimality or reachability claim.
fn remaining_pursuit_seconds(
    own: Point,
    target: Point,
    velocity: (f64, f64),
    speed: f64,
    radius: f64,
) -> Option<f64> {
    let range = own.range(target).ok()?;
    if range <= radius {
        return Some(0.0);
    }
    let denominator = speed * speed - velocity.0 * velocity.0 - velocity.1 * velocity.1;
    if denominator <= 1e-9 {
        return None;
    }
    let radial = ((target.x - own.x) * velocity.0 + (target.y - own.y) * velocity.1) / range;
    Some((range - radius) * (speed + radial) / denominator)
}

#[cfg(test)]
mod pursuit_estimate_tests {
    use super::*;
    #[tokio::test]
    async fn forecast_is_bounded_read_only_and_geometry_ignores_recycle() {
        let root = super::super::benchmark::copy_game_root().unwrap();
        let config = Config::load(&root).unwrap();
        let base = crate::persistence::load(&config.database()).await.unwrap();
        let (world, id, target, _) = super::super::encounters::fixture(
            &config,
            base,
            include_str!("../../../game/mechs/JR7-D.toml"),
            "approach",
            1,
        )
        .unwrap();
        let mut observation = super::super::observations::observe(&world, id, 1).unwrap();
        let own = observation.position.unwrap();
        let target = crate::btech::scanner::scanner_unit(&world, target)
            .unwrap()
            .position
            .unwrap();
        let before = world.btech.clone();
        let scope = super::super::diagnostics::Scope::begin(true);
        let _ = pursuit_score(
            &world,
            &config,
            id,
            target,
            target,
            (0.0, 0.0),
            0.0,
            100.0,
            0.2,
            3,
            Some((&observation, &SteeringState::default(), 1)),
        );
        let mut diagnostics = super::super::diagnostics::AutopilotDiagnostics::default();
        scope.finish(&mut diagnostics);
        assert!(
            diagnostics
                .pursuit
                .get("score_projection")
                .map_or(0, |s| s.calls)
                <= 16
        );
        assert_eq!(world.btech, before);
        let point = center(Hex::new(own.x, own.y));
        let motion = motion(&world, id).unwrap();
        let target_point = Point {
            x: point.x,
            y: point.y - 2.0,
        };
        let mut visible = Vec::new();
        for heading in (0..360).step_by(45) {
            visible.push(actual_arc(
                &world,
                id,
                &observation,
                BattleMotion {
                    heading: heading as f64,
                    ..motion
                },
                target_point,
            ));
        }
        assert!(visible.contains(&true) && visible.contains(&false));
        for weapon in &mut observation.own.weapons {
            weapon.ready = false;
            weapon.recycle_remaining = 100;
        }
        for (index, heading) in (0..360).step_by(45).enumerate() {
            assert_eq!(
                visible[index],
                actual_arc(
                    &world,
                    id,
                    &observation,
                    BattleMotion {
                        heading: heading as f64,
                        ..motion
                    },
                    target_point
                )
            );
        }
    }
    #[test]
    fn physical_direction_change_brakes_and_reverse_speed_is_bounded() {
        assert_eq!(direction_speed(-1.0, false, 90.0), 0.0);
        assert_eq!(direction_speed(1.0, true, 90.0), 0.0);
        assert_eq!(direction_speed(0.0, true, 90.0), -60.0);
        assert_eq!(direction_speed(-1.0, true, 90.0), -60.0);
        assert_eq!(direction_speed(0.0, false, 90.0), 90.0);
    }
    #[test]
    fn stationary_region_entry_stops_before_a_farther_navigation_goal() {
        let start = Point { x: 0.0, y: 0.0 };
        let end = Point { x: 10.0, y: 0.0 };
        assert_eq!(
            segment_entry(start, end, Point { x: 5.0, y: 0.0 }, 1.0),
            Some(0.4)
        );
        assert_eq!(
            segment_entry(start, end, Point { x: 5.0, y: 1.0 }, 1.0),
            Some(0.5)
        );
        assert!(segment_entry(start, end, Point { x: 5.0, y: 2.0 }, 1.0).is_none());
        assert_eq!(segment_entry(start, end, start, 1.0), Some(0.0));
    }
    #[test]
    fn remaining_closure_accounts_for_retreat_and_crossing() {
        let own = Point { x: 0.0, y: 0.0 };
        let target = Point { x: 10.0, y: 0.0 };
        assert_eq!(
            remaining_pursuit_seconds(own, target, (0.0, 0.0), 1.0, 0.0),
            Some(10.0)
        );
        assert_eq!(
            remaining_pursuit_seconds(own, target, (0.5, 0.0), 1.0, 0.0),
            Some(20.0)
        );
        assert!(remaining_pursuit_seconds(own, target, (0.0, 0.5), 1.0, 0.0).unwrap() > 10.0);
        assert!(remaining_pursuit_seconds(own, target, (1.0, 0.0), 1.0, 0.0).is_none());
    }
}
