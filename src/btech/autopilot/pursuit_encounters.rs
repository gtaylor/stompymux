//! Opt-in long-distance pursuit experiments with legally driven, reproducible opponents.
use super::*;
use crate::{BattleHexCoordinate, BattlePosition, Config, HeartbeatHarness, ObjectId};
use anyhow::{Result, ensure};
use serde::Serialize;
use std::{io::Write, path::Path, sync::Arc};

/// Stable scenario identifiers, separate from established encounter defaults.
pub const SCENARIOS: &[&str] = &[
    "distant",
    "retreat",
    "lateral",
    "reversals",
    "circuit",
    "short_occlusions",
    "expiry",
    "intercept_move",
];

/// Frozen supplemental cases; excluded from the established pursuit defaults.
pub const EXTENDED_SCENARIOS: &[&str] = &[
    "slow_crossing",
    "fast_crossing",
    "opposite_crossing",
    "stop_start",
    "gradual_turns",
    "damaged_pursuit",
];

/// One uninterrupted observed-contact interval. Missing milestones remain null.
#[derive(Default, Debug, Serialize)]
pub struct Episode {
    pub start: usize,
    pub end: usize,
    pub first_geometry: Option<usize>,
    pub first_arc: Option<usize>,
    pub first_ready: Option<usize>,
    pub first_shot: Option<usize>,
    pub geometry_ticks: usize,
    pub ready_ticks: usize,
    pub arc_ticks: usize,
    pub arc_sum: f64,
    pub distance: f64,
    pub preferred_band_ticks: usize,
    pub range_error_mean: f64,
    pub range_error_p95: f64,
    #[serde(skip)]
    range_errors: Vec<f64>,
}

/// Detached metrics never participate in gameplay decisions.
#[derive(Default, Debug, Serialize)]
pub struct PursuitResult {
    /// Script events record requested controls, not instantaneous target motion.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub script_events: Vec<(usize, String)>,
    pub schema: u8,
    pub scenario: String,
    pub chassis: String,
    pub seed: u8,
    pub fire: bool,
    pub direct: bool,
    /// Explicit requested policy; every committed heartbeat is checked against it.
    pub pursuit_policy: super::interception::PursuitPolicy,
    pub ticks: usize,
    pub first_geometry: Option<usize>,
    pub first_arc: Option<usize>,
    pub first_ready: Option<usize>,
    pub first_shot: Option<usize>,
    pub approach_distance: Option<f64>,
    pub target_distance: f64,
    pub target_destroyed_at: Option<usize>,
    pub stationary_route_length: Option<f64>,
    pub stationary_detour_distance: Option<f64>,
    pub distance: f64,
    pub visible_ticks: usize,
    pub ready_ticks: usize,
    pub arc_ticks: usize,
    pub arc_fraction: f64,
    pub preferred_band_ticks: usize,
    pub range_error_mean: f64,
    pub range_error_p95: f64,
    pub settled_distance: f64,
    pub settled_ticks: usize,
    pub reversals: usize,
    pub replans: u64,
    pub prediction_ticks: u64,
    pub prediction_fallbacks: u64,
    pub shots: u64,
    pub script_rejections: usize,
    pub outcome: String,
    pub episodes: Vec<Episode>,
}

fn center(x: u16, y: u16) -> crate::BattlePoint {
    BattleHexCoordinate {
        x: i32::from(x),
        y: i32::from(y),
    }
    .center()
}

/// Fixture impairment, applied to a setup clone for speed selection and live at tick 30.
fn impair(world: &mut crate::World, unit: ObjectId, chassis: &str) -> Result<()> {
    if chassis == "mech" {
        crate::btech::destroy_unit_critical(
            world,
            unit,
            crate::btech::CriticalLocation {
                section: crate::BattleSection::LeftLeg,
                slot: 2,
            },
        )?;
    } else {
        crate::btech::damage_vehicle_motive(
            world,
            unit,
            crate::btech::BattleVehicleMotiveHit::SpeedLoss { movement_points: 1 },
        )?;
    }
    Ok(())
}

/// Waypoint driving uses ordinary control admission, never moves units administratively.
fn drive(
    world: &mut crate::World,
    id: ObjectId,
    points: &[(u16, u16)],
    cursor: &mut usize,
    cap: f64,
) -> Result<bool> {
    if adversarial::destroyed(world, id) {
        return Ok(false);
    }
    let m = steering::motion(world, id).expect("placed target");
    if m.point
        .range(center(points[*cursor].0, points[*cursor].1))?
        < 0.6
    {
        *cursor = (*cursor + 1) % points.len();
    }
    let goal = center(points[*cursor].0, points[*cursor].1);
    let bearing = m.point.bearing(goal)?.unwrap_or(m.heading);
    let angle = ((bearing - m.heading + 180.0).rem_euclid(360.0) - 180.0).abs();
    let speed = if angle > 30.0 {
        0.0
    } else {
        (m.point.range(goal)? * 35.0).min(cap)
    };
    Ok(
        crate::btech::motion::set_heading_autopilot(world, id, bearing).is_err()
            || crate::btech::motion::set_speed_autopilot(world, id, speed).is_err(),
    )
}

/// Setup-only discrete route reference to an unobstructed weapon-range region.
fn shortest_firing_route(
    world: &crate::World,
    id: ObjectId,
    target: BattlePosition,
    maximum: u8,
) -> Option<f64> {
    use std::collections::{BTreeSet, VecDeque};
    let own = observations::observe(world, id, world.btech.simulation_time())
        .ok()?
        .position?;
    let start = navigation::Hex::new(own.x, own.y);
    let mut queue = VecDeque::from([(start, 0_u32)]);
    let mut seen = BTreeSet::from([start]);
    let map = &world.btech.maps()[&own.map];
    while let Some((h, d)) = queue.pop_front() {
        let here = BattleHexCoordinate {
            x: i32::from(h.x),
            y: i32::from(h.y),
        };
        let there = BattleHexCoordinate {
            x: i32::from(target.x),
            y: i32::from(target.y),
        };
        if here.center().range(there.center()).ok()? <= f64::from(maximum)
            && crate::btech::ground_terrain_los(map, here, there).is_ok_and(|l| !l.blocked)
        {
            return Some(f64::from(d));
        }
        for next in h.neighbors() {
            if seen.contains(&next) {
                continue;
            }
            if traversal::assess(
                world,
                id,
                BattlePosition {
                    map: own.map,
                    x: h.x,
                    y: h.y,
                },
                BattlePosition {
                    map: own.map,
                    x: next.x,
                    y: next.y,
                },
            )
            .eligible
            {
                seen.insert(next);
                queue.push_back((next, d + 1));
            }
        }
    }
    None
}

/// Run each fixture independently; `direct` only changes the harness decision-policy input.
pub async fn run(
    ticks: usize,
    seeds: u8,
    scenario: Option<&str>,
    trace: Option<&Path>,
    direct: bool,
    fire_filter: Option<bool>,
    policy: super::interception::PursuitPolicy,
    seed_start: u8,
    extended: bool,
) -> Result<Vec<PursuitResult>> {
    ensure!(ticks > 0 && seeds > 0, "Positive ticks and seeds required");
    let scenarios = if extended {
        EXTENDED_SCENARIOS
    } else {
        SCENARIOS
    };
    let seed_end = seed_start
        .checked_add(seeds - 1)
        .ok_or_else(|| anyhow::anyhow!("Seed range overflow"))?;
    ensure!(seed_start > 0, "Seed start must be positive");
    ensure!(
        scenario.is_none_or(|s| scenarios.contains(&s)),
        "Unknown pursuit scenario"
    );
    let mut file = trace.map(std::fs::File::create).transpose()?;
    let mut results = Vec::new();
    for &name in scenarios
        .iter()
        .filter(|&&s| scenario.is_none_or(|n| n == s))
    {
        for (chassis, source) in [
            ("mech", include_str!("../../../game/mechs/JR7-D").to_owned()),
            (
                "tracked",
                include_str!("../../../game/mechs/Demolisher").to_owned(),
            ),
            (
                "wheeled",
                include_str!("../../../game/mechs/Demolisher").replace("{ Track }", "{ Wheel }"),
            ),
            (
                "hover",
                include_str!("../../../game/mechs/Demolisher").replace("{ Track }", "{ Hover }"),
            ),
        ] {
            for seed in seed_start..=seed_end {
                for fire in [false, true]
                    .into_iter()
                    .filter(|f| fire_filter.is_none_or(|v| v == *f))
                {
                    let root = benchmark::copy_game_root()?;
                    let config = Config::load(&root)?;
                    let base = crate::persistence::load(&config.database()).await?;
                    let (mut world, focal, target, old_map) = encounters::fixture_with_target(
                        &config,
                        base,
                        &source,
                        if name == "short_occlusions" {
                            include_str!("../../../game/mechs/JR7-D")
                        } else {
                            include_str!("../../../game/mechs/AS7-S2")
                        },
                        "approach",
                        seed,
                    )?;
                    let map = world.create(&config, "pursuit".into(), crate::Kind::Room);
                    let mut terrain = "48 48\n".to_owned();
                    for y in 0..48 {
                        for x in 0..48 {
                            let wall = match name {
                                "circuit" => (23..=25).contains(&x) && (17..=29).contains(&y),
                                "short_occlusions" => x == 23 && y == 11,
                                "expiry" => x == 23 && (12..=36).contains(&y),
                                _ => false,
                            };
                            terrain.push_str(if wall { "=3" } else { ".0" });
                        }
                        terrain.push('\n');
                    }
                    crate::btech::create_map(
                        &mut world,
                        map,
                        "pursuit",
                        crate::BattleMapAsset::parse(&terrain)?,
                    )?;
                    let dice = world.btech.maps()[&old_map].fire_dice.clone();
                    Arc::make_mut(&mut world.btech.maps)
                        .get_mut(&map)
                        .unwrap()
                        .fire_dice = dice;
                    let start = if name == "opposite_crossing" {
                        (34, 26)
                    } else if name == "intercept_move" {
                        (18, 22)
                    } else {
                        (12, 26)
                    };
                    let enemy = if name == "opposite_crossing" {
                        (24, 10)
                    } else if name == "expiry" {
                        (21, 10)
                    } else if name == "intercept_move" {
                        (22, 22)
                    } else {
                        (22, 10)
                    };
                    adversarial::place(&mut world, focal, map, start.0, start.1)?;
                    adversarial::place(&mut world, target, map, enemy.0, enemy.1)?;
                    adversarial::seed_unit(&mut world, focal, seed, 1);
                    adversarial::seed_unit(&mut world, target, seed, 2);
                    crate::btech::refresh_contacts(&mut world, &[focal, target])?;
                    let obs = observations::observe(&world, focal, world.btech.simulation_time())?;
                    let contact = obs
                        .contacts
                        .iter()
                        .find(|c| c.unit == target && c.identified)
                        .ok_or_else(|| {
                            anyhow::anyhow!("{name}/{chassis}: initial target not identified")
                        })?;
                    let maximum = obs
                        .own
                        .weapons
                        .iter()
                        .filter(|w| alignment::available(w))
                        .map(|w| w.weapon.profile().long_range)
                        .max()
                        .unwrap();
                    ensure!(
                        name == "intercept_move" || contact.range >= f64::from(maximum) + 4.0,
                        "{name}: initial separation too short"
                    );
                    let stationary_route_length = if name == "distant" {
                        shortest_firing_route(&world, focal, contact.position, maximum)
                    } else {
                        None
                    };
                    let order = if name == "intercept_move" {
                        AutopilotOrder::AttackMove {
                            destination: BattlePosition { map, x: 40, y: 22 },
                            arrival_radius: 0,
                        }
                    } else {
                        AutopilotOrder::Attack {
                            target,
                            range: None,
                        }
                    };
                    orders::validate_for_unit(&world, focal, &order)?;
                    let mut controller = AutopilotController::with_config(AutopilotConfig {
                        speed_percent: if name == "expiry" { 25 } else { 100 },
                        fire_mode: if !fire {
                            AutopilotFireMode::Hold
                        } else if name == "intercept_move" {
                            AutopilotFireMode::Opportunistic
                        } else {
                            AutopilotFireMode::AssignedTarget
                        },
                        preferred_range: Some(AutopilotRangeBand {
                            minimum: 2,
                            maximum: 3,
                        }),
                        ..Default::default()
                    })?;
                    controller.submit(vec![order], AutopilotSubmissionMode::Replace, None)?;
                    controller.resume(None)?;
                    Arc::make_mut(&mut world.btech.controllers).insert(focal, controller);
                    Arc::make_mut(&mut world.btech.controllers)
                        .insert(target, AutopilotController::new());
                    crate::persistence::save(&config.database(), &world).await?;
                    let mut cap = if name == "short_occlusions" {
                        90.0
                    } else {
                        let fraction = match name {
                            "slow_crossing" => 0.25,
                            "fast_crossing" => 0.75,
                            _ => 0.55,
                        };
                        let requested = obs.own.maximum_speed * fraction;
                        if extended {
                            requested
                        } else {
                            requested.min(40.0)
                        }
                    };
                    if extended {
                        let target_obs =
                            observations::observe(&world, target, world.btech.simulation_time())?;
                        cap = cap.min(target_obs.own.maximum_speed);
                    }
                    if name == "damaged_pursuit" {
                        let mut damaged = world.clone();
                        impair(&mut damaged, focal, chassis)?;
                        cap = cap.min(
                            observations::observe(
                                &damaged,
                                focal,
                                damaged.btech.simulation_time(),
                            )?
                            .own
                            .maximum_speed
                                * 0.55,
                        );
                    }
                    let points = match name {
                        "retreat" => vec![(22, 2), (42, 2), (42, 40), (2, 40), (2, 2)],
                        "lateral" | "reversals" => vec![(42, 10), (4, 10)],
                        "circuit" => vec![(28, 10), (29, 33), (20, 33), (20, 10)],
                        "short_occlusions" => vec![(40, 10), (20, 10)],
                        "expiry" => vec![(26, 10), (26, 28), (40, 28)],
                        "intercept_move" => vec![(22, 40), (42, 40)],
                        "slow_crossing" | "fast_crossing" | "stop_start" | "damaged_pursuit" => {
                            vec![(42, 10), (4, 10)]
                        }
                        "opposite_crossing" => vec![(4, 10), (42, 10)],
                        "gradual_turns" => {
                            vec![(34, 10), (42, 22), (34, 34), (18, 34), (10, 22), (18, 10)]
                        }
                        _ => vec![(enemy.0 as u16, enemy.1 as u16)],
                    };
                    let mut harness = HeartbeatHarness::new(config, world)?;
                    let mut result = PursuitResult {
                        schema: 1,
                        scenario: name.into(),
                        chassis: chassis.into(),
                        seed,
                        fire,
                        direct,
                        pursuit_policy: policy,
                        outcome: "window_end".into(),
                        stationary_route_length,
                        ..Default::default()
                    };
                    let mut cursor = 0;
                    let mut errors = Vec::new();
                    let mut visible_before = false;
                    let mut stable = 0;
                    let mut last_heading: Option<f64> = None;
                    let mut previous_turn = 0.0_f64;
                    for tick in 1..=ticks {
                        if name == "damaged_pursuit" && tick == 30 {
                            let mut world = harness.scripts().world_mut();
                            impair(&mut world, focal, chassis)?;
                            cap = cap.min(
                                observations::observe(
                                    &world,
                                    focal,
                                    world.btech.simulation_time(),
                                )?
                                .own
                                .maximum_speed
                                    * 0.55,
                            );
                            result.script_events.push((tick, "mobility_damage".into()));
                        }
                        if name == "short_occlusions" && tick == 200 {
                            let mut world = harness.scripts().world_mut();
                            let order = AutopilotOrder::Patrol {
                                waypoints: [
                                    (23, 9),
                                    (25, 10),
                                    (25, 12),
                                    (23, 13),
                                    (21, 12),
                                    (21, 10),
                                ]
                                .into_iter()
                                .map(|(x, y)| BattlePosition { map, x, y })
                                .collect(),
                            };
                            orders::validate_for_unit(&world, target, &order)?;
                            let controller = Arc::make_mut(&mut world.btech.controllers)
                                .get_mut(&target)
                                .unwrap();
                            controller.submit(
                                vec![order],
                                AutopilotSubmissionMode::Replace,
                                None,
                            )?;
                            controller.resume(None)?;
                        }
                        if name != "distant" && name != "short_occlusions" {
                            if name == "reversals" && tick % 60 == 0 {
                                cursor = (cursor + 1) % points.len();
                            }
                            let stopping = name == "stop_start" && (tick - 1) % 150 >= 90;
                            if name == "stop_start" && matches!((tick - 1) % 150, 0 | 90) {
                                result.script_events.push((
                                    tick,
                                    if stopping {
                                        "stop_requested"
                                    } else {
                                        "move_requested"
                                    }
                                    .into(),
                                ));
                            }
                            let old_cursor = cursor;
                            result.script_rejections += usize::from(drive(
                                &mut harness.scripts().world_mut(),
                                target,
                                &points,
                                &mut cursor,
                                if stopping { 0.0 } else { cap },
                            )?);
                            if name == "gradual_turns" && cursor != old_cursor {
                                result.script_events.push((tick, "turn_requested".into()));
                            }
                        }
                        let before = harness.world();
                        let point = steering::motion(&before, focal).unwrap().point;
                        let target_point = steering::motion(&before, target).map(|m| m.point);
                        let metrics = harness
                            .step_pursuit_policy(tick as i64, false, true, direct, policy)
                            .await;
                        ensure!(metrics.committed, "Pursuit tick failed commit");
                        ensure!(
                            metrics.autopilot.pursuit_policy == policy
                                && metrics.autopilot.direct_pursuit == direct,
                            "Requested and resolved pursuit policies differ"
                        );
                        let after = harness.world();
                        let motion = steering::motion(&after, focal).unwrap();
                        let traveled = point.range(motion.point)?;
                        result.ticks = tick;
                        result.distance += traveled;
                        result.target_distance += target_point
                            .zip(steering::motion(&after, target).map(|m| m.point))
                            .map(|(a, b)| a.range(b))
                            .transpose()?
                            .unwrap_or(0.0);
                        result.replans += metrics
                            .autopilot
                            .replans_by_unit
                            .get(&focal)
                            .copied()
                            .unwrap_or(0);
                        result.prediction_ticks += metrics.autopilot.prediction_ticks;
                        result.prediction_fallbacks += metrics.autopilot.prediction_fallbacks;
                        let shots = metrics
                            .autopilot
                            .shots_by_unit
                            .get(&focal)
                            .copied()
                            .unwrap_or(0);
                        result.shots += shots;
                        if shots > 0 {
                            result.first_shot.get_or_insert(tick);
                        }
                        let obs =
                            observations::observe(&after, focal, after.btech.simulation_time())?;
                        let (visible, arc, ready, coverage) =
                            encounters::opportunity(&after, focal, target)?;
                        let range = obs
                            .contacts
                            .iter()
                            .find(|c| c.unit == target)
                            .map(|c| c.range);
                        let geometry = range.is_some_and(|r| r <= f64::from(maximum))
                            && crate::btech::unit_terrain_los(&after, focal, target)
                                .is_ok_and(|l| !l.blocked);
                        if geometry {
                            if result.first_geometry.is_none() {
                                result.stationary_detour_distance =
                                    stationary_route_length.map(|d| (result.distance - d).max(0.0));
                            }
                            result.first_geometry.get_or_insert(tick);
                        }
                        if arc {
                            result.first_arc.get_or_insert(tick);
                        }
                        if ready || shots > 0 {
                            result.first_ready.get_or_insert(tick);
                            result.approach_distance.get_or_insert(result.distance);
                            result.ready_ticks += 1;
                        }
                        result.visible_ticks += usize::from(visible);
                        result.arc_ticks += usize::from(arc);
                        result.arc_fraction += coverage;
                        if visible {
                            if !visible_before {
                                result.episodes.push(Episode {
                                    start: tick,
                                    ..Default::default()
                                });
                            }
                            let e = result.episodes.last_mut().unwrap();
                            e.end = tick;
                            if geometry {
                                e.first_geometry.get_or_insert(tick);
                                e.geometry_ticks += 1;
                            }
                            if arc {
                                e.first_arc.get_or_insert(tick);
                            }
                            if ready || shots > 0 {
                                e.first_ready.get_or_insert(tick);
                                e.ready_ticks += 1;
                            }
                            if shots > 0 {
                                e.first_shot.get_or_insert(tick);
                            }
                            e.arc_sum += coverage;
                            e.arc_ticks += usize::from(arc);
                            e.distance += traveled;
                            if let Some(r) = range {
                                let error = (2.0 - r).max(0.0) + (r - 3.0).max(0.0);
                                e.range_errors.push(error);
                                e.preferred_band_ticks += usize::from(error == 0.0);
                            }
                        }
                        visible_before = visible;
                        if let Some(r) = range {
                            let error = (2.0 - r).max(0.0) + (r - 3.0).max(0.0);
                            errors.push(error);
                            if error == 0.0 {
                                result.preferred_band_ticks += 1;
                            }
                        }
                        if arc && traveled < 0.02 {
                            stable += 1;
                        } else if stable < 5 {
                            stable = 0;
                        }
                        if stable >= 5 {
                            result.settled_ticks += 1;
                            result.settled_distance += traveled;
                        }
                        if let Some(last) = last_heading {
                            let turn =
                                (motion.heading - last + 180.0_f64).rem_euclid(360.0) - 180.0;
                            if turn.abs() > 1.0 {
                                if previous_turn * turn < 0.0 {
                                    result.reversals += 1;
                                }
                                previous_turn = turn;
                            }
                        }
                        last_heading = Some(motion.heading);
                        if adversarial::destroyed(&after, target) {
                            result.target_destroyed_at.get_or_insert(tick);
                        }
                        let c = &after.btech.controllers()[&focal];
                        let terminal = if adversarial::destroyed(&after, focal) {
                            Some("destroyed".into())
                        } else if name != "intercept_move" && adversarial::destroyed(&after, target)
                        {
                            Some("target_destroyed".into())
                        } else if c.state() == AutopilotState::Blocked {
                            Some(
                                c.blocking_reason()
                                    .map(|r| format!("{r:?}"))
                                    .unwrap_or_else(|| "blocked".into()),
                            )
                        } else if c.state() == AutopilotState::Idle {
                            Some("completed".into())
                        } else {
                            None
                        };
                        if let Some(t) = &terminal {
                            result.outcome = t.clone();
                        }
                        if let Some(f) = file.as_mut() {
                            let bytes = serde_json::to_vec(&(
                                &after.btech,
                                &metrics.autopilot.notice_trace,
                                &metrics.autopilot.shots_by_unit,
                            ))?;
                            let digest = bytes.iter().fold(0xcbf29ce484222325_u64, |h, b| {
                                (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
                            });
                            writeln!(
                                f,
                                "{}",
                                serde_json::json!({"scenario":name,"chassis":chassis,"seed":seed,"fire":fire,"tick":tick,"digest":format!("{digest:016x}"),"result":result,
                                "pursuit": after.btech.autopilot_plans.get(&focal).map(|p| serde_json::json!({
                                    "evidence":p.pursuit.evidence(),"goal":p.goal,"route_index":p.route_index,
                                    "replacement_pending":p.replacement_pending,
                                    "search_expanded":p.search.as_ref().map(|s|s.total_expanded()),
                                    "motion":super::steering::motion(&after,focal).map(|m|serde_json::json!({
                                        "heading":m.heading,"desired_heading":m.desired_heading,
                                        "heading_error":((m.heading-m.desired_heading+180.0).rem_euclid(360.0)-180.0).abs(),
                                        "speed":m.speed,"desired_speed":m.desired_speed,
                                        "braking":m.desired_speed.abs()+0.01<m.speed.abs()
                                    })),
                                    "navigation_recoveries":p.recovery_attempts,"stagnant_ticks":p.stagnant_ticks
                                }))})
                            )?;
                        }
                        if terminal.is_some() {
                            break;
                        }
                    }
                    for e in &mut result.episodes {
                        e.range_error_mean =
                            e.range_errors.iter().sum::<f64>() / e.range_errors.len().max(1) as f64;
                        e.range_errors.sort_by(f64::total_cmp);
                        e.range_error_p95 = e
                            .range_errors
                            .get(e.range_errors.len() * 95 / 100)
                            .copied()
                            .unwrap_or(0.0);
                    }
                    result.arc_fraction /= result.visible_ticks.max(1) as f64;
                    result.range_error_mean =
                        errors.iter().sum::<f64>() / errors.len().max(1) as f64;
                    errors.sort_by(f64::total_cmp);
                    result.range_error_p95 =
                        errors.get(errors.len() * 95 / 100).copied().unwrap_or(0.0);
                    eprintln!("pursuit: {}", serde_json::to_string(&result)?);
                    results.push(result);
                }
            }
        }
    }
    Ok(results)
}
