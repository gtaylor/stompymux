//! Seeded multi-participant encounters through the production heartbeat and admission rules.
use super::*;
use crate::btech::UnitTemplateExt;
use crate::{Config, HeartbeatHarness, ObjectId, Position, Power, World};
use anyhow::{Context, Result, ensure};
use serde::Serialize;
use std::{collections::BTreeMap, io::Write, path::Path};

/// Stable fixture identifiers; impossible passage is diagnostic, never a success gate.
pub const SCENARIOS: &[&str] = &[
    "crossing",
    "duel",
    "occluded",
    "bottleneck",
    "pursuers",
    "passage",
    "no_passing_space",
    "mobility_damage",
    "mixed_arcs",
    "late_clearance",
    "alternating_clearance",
    "waiting_controllers",
];

/// Clearance measurements are separate from the established destination-clearance metric.
#[derive(Debug, Clone, Default, Serialize)]
pub struct CongestionEncounter {
    pub first_watched_clearance: Option<usize>,
    pub clearance_to_search: Option<usize>,
    pub clearance_to_movement: Option<usize>,
    pub checks: u64,
    pub early_starts: u64,
    pub timed_starts: u64,
    pub deferrals: u64,
}

/// Detached per-participant measurements with explicit exposure and terminal reasons.
#[derive(Debug, Clone, Serialize)]
pub struct ParticipantResult {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub congestion: Option<CongestionEncounter>,
    pub schema: u8,
    pub scenario: String,
    pub chassis: String,
    pub seed: u8,
    pub role: String,
    pub ticks: usize,
    pub sampled_ticks: usize,
    pub first_geometry: Option<usize>,
    pub first_shot: Option<usize>,
    pub visible_ticks: usize,
    pub longest_contact_gap: usize,
    pub reacquisitions: usize,
    pub geometric_ticks: usize,
    pub arc_fraction: f64,
    pub preferred_band_ticks: usize,
    pub range_error_sum: f64,
    pub range_samples: usize,
    pub distance: f64,
    pub excess_distance: Option<f64>,
    pub settled_distance: f64,
    pub reversals: usize,
    pub replans: usize,
    pub longest_no_progress: usize,
    pub clearance_tick: Option<usize>,
    pub recovery_ticks: Option<usize>,
    pub shots: u64,
    pub script_rejections: usize,
    pub outcome: String,
    pub blocking_reason: Option<String>,
}

struct Participant {
    id: ObjectId,
    target: ObjectId,
    result: ParticipantResult,
    previous: crate::Point,
    direction: i8,
    stationary: usize,
    stagnant: usize,
    remaining: Option<f64>,
    destination: Option<Position>,
    shortest: Option<f64>,
    terminal: bool,
    seen: bool,
    contact_gap: usize,
}

/// Install validated orders without invoking the player takeover boundary.
/// Stock template reference each scenario chassis is built from.
fn stock_reference(chassis: &str) -> &'static str {
    if chassis == "mech" {
        "JR7-D"
    } else {
        "Demolisher"
    }
}

fn orders(world: &mut World, id: ObjectId, orders: Vec<AutopilotOrder>, fire: bool) -> Result<()> {
    let mut controller = AutopilotController::with_config(AutopilotConfig {
        fire_mode: if fire {
            AutopilotFireMode::AssignedTarget
        } else {
            AutopilotFireMode::Hold
        },
        preferred_range: Some(AutopilotRangeBand {
            minimum: 2,
            maximum: 3,
        }),
        ..Default::default()
    })?;
    if !orders.is_empty() {
        controller.submit(orders, AutopilotSubmissionMode::Replace, None)?;
        controller.resume(None)?;
    }
    world.btech.controllers.insert(id, controller);
    Ok(())
}

/// Setup-only placement; scenario events never relocate running units.
pub(super) fn place(world: &mut World, id: ObjectId, map: ObjectId, x: i64, y: i64) -> Result<()> {
    if let Some(u) = world.btech.constructed.get_mut(&id) {
        u.power = Power::Off;
    }
    if let Some(u) = world.btech.vehicles.get_mut(&id) {
        u.power = Power::Off;
    }
    crate::btech::place_unit(world, id, map, x, y)?;
    if let Some(u) = world.btech.constructed.get_mut(&id) {
        u.power = Power::Running;
    }
    if let Some(u) = world.btech.vehicles.get_mut(&id) {
        u.power = Power::Running;
    }
    Ok(())
}

pub(super) fn destroyed(world: &World, id: ObjectId) -> bool {
    world
        .btech
        .constructed_units()
        .get(&id)
        .map(|u| u.is_destroyed())
        .or_else(|| world.btech.vehicles().get(&id).map(|u| u.is_destroyed()))
        .unwrap_or(true)
}

/// Independent reproducible streams avoid correlated shooter/target rolls.
pub(super) fn seed_unit(world: &mut World, id: ObjectId, seed: u8, role: u8) {
    let mut bytes = [seed; 32];
    bytes[0] = role;
    bytes[31] = seed.wrapping_add(role);
    if let Some(u) = world.btech.constructed.get_mut(&id) {
        u.dice = crate::Dice::seeded(bytes);
        let mut recovery = serde_json::to_value(&u.crew_recovery).expect("serialize recovery");
        recovery["dice"] =
            serde_json::to_value(crate::Dice::seeded(bytes)).expect("serialize dice");
        u.crew_recovery = serde_json::from_value(recovery).expect("recovery with seeded dice");
    }
    if let Some(u) = world.btech.vehicles.get_mut(&id) {
        u.dice = crate::Dice::seeded(bytes);
        let mut recovery = serde_json::to_value(&u.crew_recovery).expect("serialize recovery");
        recovery["dice"] =
            serde_json::to_value(crate::Dice::seeded(bytes)).expect("serialize dice");
        u.crew_recovery = serde_json::from_value(recovery).expect("recovery with seeded dice");
    }
}

/// Scripted target follows real motion controls; turn before accelerating near an endpoint.
fn script(world: &mut World, id: ObjectId, tick: usize, cap: f64) -> Result<bool> {
    if destroyed(world, id) {
        return Ok(false);
    }
    let motion = steering::motion(world, id).context("Scripted unit has no motion")?;
    let goal = crate::HexCoordinate {
        x: if (tick / 60).is_multiple_of(2) { 9 } else { 2 },
        y: 3,
    }
    .center();
    let bearing = motion.point.bearing(goal)?.unwrap_or(motion.heading);
    let error = ((bearing - motion.heading + 180.0).rem_euclid(360.0) - 180.0).abs();
    if crate::btech::motion::set_heading_autopilot(world, id, bearing).is_err() {
        return Ok(true);
    }
    Ok(crate::btech::motion::set_speed_autopilot(
        world,
        id,
        if error > 30.0 {
            0.0
        } else {
            (motion.point.range(goal)? * 35.0).min(cap)
        },
    )
    .is_err())
}

/// Shortest geometric route uses authoritative eligibility, not Euclidean distance through walls.
fn shortest(world: &World, id: ObjectId, from: Position, to: Position) -> Option<f64> {
    let start = navigation::GridHex::new(from.x, from.y);
    let goal = navigation::GridHex::new(to.x, to.y);
    let mut distances = BTreeMap::from([(start, 0_u32)]);
    let mut queue = std::collections::VecDeque::from([start]);
    while let Some(h) = queue.pop_front() {
        let cost = distances[&h];
        if h == goal {
            return Some(f64::from(cost));
        }
        for y in h.y.saturating_sub(1)..=h.y.saturating_add(1) {
            for x in h.x.saturating_sub(1)..=h.x.saturating_add(1) {
                let next = navigation::GridHex::new(x, y);
                if h.distance(next) != 1 || distances.contains_key(&next) {
                    continue;
                }
                if traversal::assess(
                    world,
                    id,
                    Position {
                        map: from.map,
                        x: h.x,
                        y: h.y,
                    },
                    Position {
                        map: from.map,
                        x,
                        y,
                    },
                )
                .eligible
                {
                    distances.insert(next, cost + 1);
                    queue.push_back(next);
                }
            }
        }
    }
    None
}

/// Run an opt-in matrix. Full-state checksums exclude timing and contain ordered outcomes.
pub async fn run(
    ticks: usize,
    seeds: u8,
    scenario: Option<&str>,
    trace: Option<&Path>,
) -> Result<Vec<ParticipantResult>> {
    run_policy(ticks, seeds, scenario, trace, Default::default()).await
}

/// Execute the same frozen fixtures with an explicit isolated-harness policy.
pub async fn run_policy(
    ticks: usize,
    seeds: u8,
    scenario: Option<&str>,
    trace: Option<&Path>,
    policy: super::interception::PursuitPolicy,
) -> Result<Vec<ParticipantResult>> {
    ensure!(
        ticks > 0 && seeds > 0,
        "Encounter ticks and seeds must be positive"
    );
    ensure!(
        scenario.is_none_or(|s| SCENARIOS.contains(&s)),
        "Unknown adversarial scenario"
    );
    let mut output = Vec::new();
    let mut trace = trace.map(std::fs::File::create).transpose()?;
    for &name in SCENARIOS.iter().filter(|&&s| {
        scenario.map_or(
            !matches!(
                s,
                "late_clearance" | "alternating_clearance" | "waiting_controllers"
            ),
            |n| n == s,
        )
    }) {
        let scenario_name = name;
        let name = if matches!(
            name,
            "late_clearance" | "alternating_clearance" | "waiting_controllers"
        ) {
            "bottleneck"
        } else {
            name
        };
        for (chassis, mut source) in [
            (
                "mech",
                include_str!("../../../game/units/JR7-D.toml").to_owned(),
            ),
            (
                "tracked",
                include_str!("../../../game/units/Demolisher.toml").to_owned(),
            ),
            (
                "wheeled",
                include_str!("../../../game/units/Demolisher.toml")
                    .replace("movement = \"track\"", "movement = \"wheel\""),
            ),
            (
                "hover",
                include_str!("../../../game/units/Demolisher.toml")
                    .replace("movement = \"track\"", "movement = \"hover\""),
            ),
        ] {
            if name == "mixed_arcs" {
                if chassis == "mech" {
                    source = source.replacen(
                        "item = \"IS.MediumLaser\" }",
                        "item = \"IS.MediumLaser\", modes = [\"RearMount\"] }",
                        1,
                    );
                } else {
                    source = source
                        .replace(
                            "{ at = \"1-2\", item = \"IS.AC/20\" }",
                            "{ at = 1, item = \"IS.MediumLaser\" }",
                        )
                        .replace(
                            "[sections.front_side]\narmor = 40\n",
                            "[sections.front_side]\narmor = 40\nslots = [{ at = \"1-2\", item = \"IS.AC/20\" }]\n",
                        );
                }
            }
            for seed in 1..=seeds {
                let root = super::benchmark::copy_game_root()?;
                let config = Config::load(&root)?;
                let base = crate::persistence::load(&config.database()).await?;
                let (mut world, focal, target, old_map) =
                    super::encounters::fixture(&config, base, &source, "approach", seed)?;
                let corridor = matches!(
                    name,
                    "bottleneck" | "pursuers" | "passage" | "no_passing_space"
                );
                let map = world.create(&config, "adversarial map".into(), crate::Kind::Room);
                let mut terrain = "12 12\n".to_owned();
                for y in 0..12 {
                    for x in 0..12 {
                        let wall = if scenario_name == "waiting_controllers" {
                            ![2, 6, 10].contains(&y)
                        } else if corridor {
                            y != 6
                                && !(!matches!(name, "no_passing_space" | "bottleneck")
                                    && (if name == "bottleneck" {
                                        x == 6
                                    } else {
                                        (5..=7).contains(&x)
                                    })
                                    && y == 5)
                        } else {
                            name == "occluded" && x == 5 && (4..=8).contains(&y)
                        };
                        terrain.push_str(if wall {
                            if name == "occluded" { "=3" } else { "=0" }
                        } else {
                            ".0"
                        });
                    }
                    terrain.push('\n');
                }
                crate::btech::create_map(
                    &mut world,
                    map,
                    "adversarial",
                    crate::MapAsset::from_cells(&terrain)?,
                )?;
                let dice = world.btech.maps()[&old_map].fire_dice.clone();
                world.btech.maps.get_mut(&map).unwrap().fire_dice = dice;
                place(&mut world, focal, map, 2, if corridor { 6 } else { 9 })?;
                place(
                    &mut world,
                    target,
                    map,
                    if corridor {
                        8
                    } else if name == "occluded" {
                        3
                    } else {
                        7
                    },
                    if corridor { 6 } else { 3 },
                )?;
                let mut ids = vec![
                    ("focal".to_owned(), focal, target),
                    ("opponent".to_owned(), target, focal),
                ];
                let destination = Position { map, x: 9, y: 6 };
                let fixed = matches!(name, "bottleneck" | "passage" | "no_passing_space");
                orders(
                    &mut world,
                    focal,
                    vec![if fixed {
                        AutopilotOrder::Move {
                            destination,
                            arrival_radius: 0,
                        }
                    } else {
                        AutopilotOrder::Attack {
                            target,
                            range: None,
                        }
                    }],
                    !fixed,
                )?;
                orders(
                    &mut world,
                    target,
                    if name == "duel" {
                        vec![AutopilotOrder::Attack {
                            target: focal,
                            range: None,
                        }]
                    } else if name == "passage" {
                        vec![AutopilotOrder::Move {
                            destination: Position { map, x: 2, y: 6 },
                            arrival_radius: 0,
                        }]
                    } else {
                        vec![]
                    },
                    name == "duel",
                )?;
                if name == "occluded" {
                    world.btech.controllers.get_mut(&focal).unwrap().configure(
                        AutopilotConfigPatch {
                            speed_percent: Some(25),
                            ..Default::default()
                        },
                        None,
                    )?;
                }
                if matches!(name, "pursuers" | "bottleneck" | "no_passing_space") {
                    for n in 0..2 {
                        let id = world.create(&config, format!("ally {n}"), crate::Kind::Thing);
                        crate::UnitTemplate::parse(stock_reference(chassis), &source)?
                            .create(&mut world, id)?;
                        if let Some(u) = world.btech.constructed.get_mut(&id) {
                            u.signature.team = 1;
                        }
                        if let Some(u) = world.btech.vehicles.get_mut(&id) {
                            u.signature.team = 1;
                        }
                        place(
                            &mut world,
                            id,
                            map,
                            if name == "pursuers" { 1 + n } else { 6 },
                            6,
                        )?;
                        orders(
                            &mut world,
                            id,
                            if name == "pursuers" {
                                vec![AutopilotOrder::Attack {
                                    target,
                                    range: None,
                                }]
                            } else {
                                vec![AutopilotOrder::Hold]
                            },
                            name == "pursuers",
                        )?;
                        ids.push((
                            format!(
                                "{}_{}",
                                if name == "pursuers" {
                                    "pursuer"
                                } else {
                                    "blocker"
                                },
                                n + 1
                            ),
                            id,
                            target,
                        ));
                    }
                }
                if matches!(name, "bottleneck" | "no_passing_space") {
                    place(&mut world, target, map, 6, 6)?;
                    world
                        .btech
                        .constructed
                        .get_mut(&target)
                        .unwrap()
                        .signature
                        .team = 1;
                }
                if scenario_name == "waiting_controllers" {
                    for (n, row) in [2, 10].into_iter().enumerate() {
                        let id = world.create(
                            &config,
                            format!("waiting follower {n}"),
                            crate::Kind::Thing,
                        );
                        crate::UnitTemplate::parse(stock_reference(chassis), &source)?
                            .create(&mut world, id)?;
                        if let Some(u) = world.btech.constructed.get_mut(&id) {
                            u.signature.team = 1;
                        }
                        if let Some(u) = world.btech.vehicles.get_mut(&id) {
                            u.signature.team = 1;
                        }
                        place(&mut world, id, map, 2, row)?;
                        orders(
                            &mut world,
                            id,
                            vec![AutopilotOrder::Move {
                                destination: Position {
                                    map,
                                    x: 9,
                                    y: row as u16,
                                },
                                arrival_radius: 0,
                            }],
                            false,
                        )?;
                        let mut first_blocker = None;
                        for k in 0..3 {
                            let blocker = world.create(
                                &config,
                                format!("lane {n} blocker {k}"),
                                crate::Kind::Thing,
                            );
                            crate::UnitTemplate::parse(stock_reference(chassis), &source)?
                                .create(&mut world, blocker)?;
                            if let Some(u) = world.btech.constructed.get_mut(&blocker) {
                                u.signature.team = 1;
                            }
                            if let Some(u) = world.btech.vehicles.get_mut(&blocker) {
                                u.signature.team = 1;
                            }
                            place(&mut world, blocker, map, 6, row)?;
                            orders(&mut world, blocker, vec![AutopilotOrder::Hold], false)?;
                            ids.push((format!("blocker_lane{n}_{k}"), blocker, id));
                            first_blocker.get_or_insert(blocker);
                        }
                        ids.push((format!("waiting_{n}"), id, first_blocker.unwrap()));
                    }
                }
                for (index, (_, id, _)) in ids.iter().enumerate() {
                    seed_unit(&mut world, *id, seed, index as u8 + 1);
                }
                // Author valid observed-target orders: each attacker initially faces its opponent.
                for (_, id, enemy) in &ids {
                    let point = steering::motion(&world, *id).unwrap().point;
                    let heading = point
                        .bearing(steering::motion(&world, *enemy).unwrap().point)?
                        .unwrap_or(0.0);
                    if let Some(u) = world.btech.constructed.get_mut(id) {
                        let motion = u.motion.as_mut().unwrap();
                        motion.heading = heading;
                        motion.desired_heading = heading;
                    }
                    if let Some(u) = world.btech.vehicles.get_mut(id) {
                        let motion = u.motion.as_mut().unwrap();
                        motion.heading = heading;
                        motion.desired_heading = heading;
                    }
                }
                crate::btech::refresh_contacts(
                    &mut world,
                    &ids.iter().map(|(_, id, _)| *id).collect::<Vec<_>>(),
                )?;
                let mut awaiting_contact = std::collections::BTreeSet::new();
                if name == "pursuers" {
                    // Prime controller memory through a real hold heartbeat before submitting
                    // observed-target intentions. A sensor miss must not author an invalid attack.
                    for (_, id, _) in ids.iter().filter(|(_, id, _)| *id != target) {
                        orders(&mut world, *id, vec![AutopilotOrder::Hold], false)?;
                        awaiting_contact.insert(*id);
                    }
                }
                world.validate(&config)?;
                let mut participants = Vec::new();
                for (role, id, enemy) in ids {
                    let own = crate::btech::scanner::scanner_unit(&world, id).unwrap();
                    let goal = if (id == focal && fixed) || role.starts_with("waiting_") {
                        Some(Position {
                            y: own.position.unwrap().y,
                            ..destination
                        })
                    } else if id == target && name == "passage" {
                        Some(Position { map, x: 2, y: 6 })
                    } else {
                        None
                    };
                    let lower = goal.and_then(|g| shortest(&world, id, own.position.unwrap(), g));
                    if goal.is_some() && name == "passage" {
                        ensure!(lower.is_some(), "Controlled fixture lacks a legal route");
                    }
                    participants.push(Participant {
                        id,
                        target: enemy,
                        previous: own.point.unwrap(),
                        direction: 0,
                        stationary: 0,
                        stagnant: 0,
                        remaining: None,
                        destination: goal,
                        shortest: lower,
                        terminal: false,
                        seen: false,
                        contact_gap: 0,
                        result: ParticipantResult {
                            congestion: None,
                            schema: 1,
                            scenario: scenario_name.into(),
                            chassis: chassis.into(),
                            seed,
                            role,
                            ticks,
                            sampled_ticks: 0,
                            first_geometry: None,
                            first_shot: None,
                            visible_ticks: 0,
                            longest_contact_gap: 0,
                            reacquisitions: 0,
                            geometric_ticks: 0,
                            arc_fraction: 0.0,
                            preferred_band_ticks: 0,
                            range_error_sum: 0.0,
                            range_samples: 0,
                            distance: 0.0,
                            excess_distance: None,
                            settled_distance: 0.0,
                            reversals: 0,
                            replans: 0,
                            longest_no_progress: 0,
                            clearance_tick: None,
                            recovery_ticks: None,
                            shots: 0,
                            script_rejections: 0,
                            outcome: "window_end".into(),
                            blocking_reason: None,
                        },
                    });
                }
                crate::persistence::save(&config.database(), &world).await?;
                let mut harness = HeartbeatHarness::new(config.clone(), world)?;
                for tick in 1..=ticks {
                    {
                        let mut world = harness.scripts().world_mut();
                        // Observe relevant clearance independently of the runtime polling cadence.
                        for p in &mut participants {
                            if let Some(plan) = world.btech.autopilot_plans.get(&p.id)
                                && plan.congestion.waiting
                                && !plan.congestion.cells.is_empty()
                                && traversal::watched_clearance(
                                    &world,
                                    p.id,
                                    map,
                                    &plan.congestion.cells,
                                )
                            {
                                p.result
                                    .congestion
                                    .get_or_insert_with(Default::default)
                                    .first_watched_clearance
                                    .get_or_insert(tick);
                            }
                        }
                        if tick > 1 {
                            for id in awaiting_contact.iter().copied().collect::<Vec<_>>() {
                                let order = AutopilotOrder::Attack {
                                    target,
                                    range: None,
                                };
                                if super::orders::validate_for_unit(&world, id, &order).is_err() {
                                    continue;
                                }
                                let controller = world.btech.controllers.get_mut(&id).unwrap();
                                controller.configure(
                                    AutopilotConfigPatch {
                                        fire_mode: Some(AutopilotFireMode::AssignedTarget),
                                        ..Default::default()
                                    },
                                    None,
                                )?;
                                controller.submit(
                                    vec![order],
                                    AutopilotSubmissionMode::Replace,
                                    None,
                                )?;
                                controller.resume(None)?;
                                awaiting_contact.remove(&id);
                            }
                        }
                        if matches!(name, "crossing" | "occluded")
                            && script(
                                &mut world,
                                target,
                                tick,
                                if name == "occluded" { 53.75 } else { 25.0 },
                            )?
                        {
                            participants
                                .iter_mut()
                                .find(|p| p.id == target)
                                .unwrap()
                                .result
                                .script_rejections += 1;
                        }
                        if name == "bottleneck"
                            && tick
                                == if scenario_name == "late_clearance" {
                                    if chassis == "mech" { 22 } else { 14 }
                                } else {
                                    10
                                }
                        {
                            orders(
                                &mut world,
                                target,
                                vec![AutopilotOrder::Move {
                                    destination: Position { map, x: 10, y: 6 },
                                    arrival_radius: 0,
                                }],
                                false,
                            )?;
                        }
                        if name == "bottleneck"
                            && tick
                                == if scenario_name == "late_clearance" {
                                    if chassis == "mech" { 22 } else { 14 }
                                } else {
                                    10
                                }
                        {
                            for p in participants
                                .iter()
                                .filter(|p| p.result.role.starts_with("blocker"))
                            {
                                let row = crate::btech::scanner::scanner_unit(&world, p.id)
                                    .unwrap()
                                    .position
                                    .unwrap()
                                    .y;
                                orders(
                                    &mut world,
                                    p.id,
                                    vec![AutopilotOrder::Move {
                                        destination: Position { map, x: 10, y: row },
                                        arrival_radius: 0,
                                    }],
                                    false,
                                )?;
                            }
                        }
                        if scenario_name == "alternating_clearance" && matches!(tick, 35 | 60) {
                            for p in participants
                                .iter()
                                .filter(|p| p.id == target || p.result.role.starts_with("blocker"))
                            {
                                orders(
                                    &mut world,
                                    p.id,
                                    vec![AutopilotOrder::Move {
                                        destination: Position {
                                            map,
                                            x: if tick == 35 { 6 } else { 10 },
                                            y: 6,
                                        },
                                        arrival_radius: 0,
                                    }],
                                    false,
                                )?;
                            }
                        }
                        if name == "mobility_damage" && tick == 30 && !destroyed(&world, focal) {
                            if chassis == "mech" {
                                crate::btech::destroy_unit_critical(
                                    &mut world,
                                    focal,
                                    crate::btech::CriticalLocation {
                                        section: crate::MechSection::LeftLeg,
                                        slot: 2,
                                    },
                                )?;
                            } else {
                                crate::btech::damage_vehicle_motive(
                                    &mut world,
                                    focal,
                                    crate::btech::VehicleMotiveHit::SpeedLoss {
                                        movement_points: 1,
                                    },
                                )?;
                            }
                        }
                    }
                    let metrics = harness
                        .step_pursuit_policy(tick as i64, false, true, false, policy)
                        .await;
                    ensure!(
                        metrics.autopilot.pursuit_policy == policy,
                        "Encounter pursuit policy mismatch"
                    );
                    ensure!(
                        metrics.committed,
                        "{name}/{chassis}/{seed} tick {tick} failed commit"
                    );
                    ensure!(
                        metrics.autopilot.expansions <= 25_600
                            && metrics.autopilot.max_controller_expansions <= 256
                            && metrics.autopilot.peak_search_records <= 1_000_000,
                        "Resource budget exceeded"
                    );
                    let world = harness.world();
                    let cleared = name == "bottleneck"
                        && participants
                            .iter()
                            .filter(|p| p.id == target || p.result.role.starts_with("blocker"))
                            .all(|p| {
                                crate::btech::scanner::scanner_unit(&world, p.id)
                                    .and_then(|u| u.position)
                                    .is_some_and(|pos| pos.x >= 10)
                            });
                    for p in &mut participants {
                        if p.terminal {
                            continue;
                        }
                        // A unit may fire and then be destroyed later in the same committed tick.
                        let shots = metrics
                            .autopilot
                            .shots_by_unit
                            .get(&p.id)
                            .copied()
                            .unwrap_or(0);
                        p.result.shots += shots;
                        if shots > 0 {
                            p.result.first_shot.get_or_insert(tick);
                        }
                        if destroyed(&world, p.id) {
                            p.result.outcome = "destroyed".into();
                            p.terminal = true;
                            continue;
                        }
                        let Some(own) = crate::btech::scanner::scanner_unit(&world, p.id) else {
                            p.result.outcome = "removed".into();
                            p.terminal = true;
                            continue;
                        };
                        let Some(point) = own.point else {
                            p.result.outcome = "unplaced".into();
                            p.terminal = true;
                            continue;
                        };
                        p.result.sampled_ticks += 1;
                        let distance = p.previous.range(point)?;
                        p.previous = point;
                        p.result.distance += distance;
                        let (visible, geometry, _, coverage) =
                            super::encounters::opportunity(&world, p.id, p.target)?;
                        if visible {
                            if p.seen && p.contact_gap > 0 {
                                p.result.reacquisitions += 1;
                            }
                            p.seen = true;
                            p.contact_gap = 0;
                        } else if p.seen {
                            p.contact_gap += 1;
                            p.result.longest_contact_gap =
                                p.result.longest_contact_gap.max(p.contact_gap);
                        }
                        p.result.visible_ticks += usize::from(visible);
                        p.result.geometric_ticks += usize::from(geometry);
                        p.result.arc_fraction += coverage;
                        if geometry {
                            p.result.first_geometry.get_or_insert(tick);
                        }
                        let observation = observations::observe(&world, p.id, tick as i64)?;
                        let range = observation
                            .contacts
                            .iter()
                            .find(|c| c.unit == p.target && !c.known_destroyed)
                            .map(|c| c.range);
                        if let Some(range) = range {
                            p.result.range_samples += 1;
                            p.result.range_error_sum +=
                                (2.0 - range).max(0.0) + (range - 3.0).max(0.0);
                            p.result.preferred_band_ticks +=
                                usize::from((2.0..=3.0).contains(&range));
                        }
                        if p.stationary >= 5 && geometry {
                            p.result.settled_distance += distance;
                        }
                        p.stationary = if geometry && distance < 0.01 {
                            p.stationary + 1
                        } else {
                            0
                        };
                        let direction = if own.speed.abs() < 0.1 {
                            0
                        } else if own.speed < 0.0 {
                            -1
                        } else {
                            1
                        };
                        if direction != 0 {
                            if p.direction != 0 && p.direction != direction {
                                p.result.reversals += 1;
                            }
                            p.direction = direction;
                        }
                        let remaining = p
                            .destination
                            .map(|g| {
                                point.range(
                                    crate::HexCoordinate {
                                        x: i32::from(g.x),
                                        y: i32::from(g.y),
                                    }
                                    .center(),
                                )
                            })
                            .transpose()?;
                        let progress = if let Some(r) = remaining {
                            p.remaining.is_none_or(|last| r + 0.02 < last)
                        } else {
                            distance > 0.02 || geometry
                        };
                        p.remaining = remaining;
                        p.stagnant = if progress { 0 } else { p.stagnant + 1 };
                        p.result.longest_no_progress = p.result.longest_no_progress.max(p.stagnant);
                        if cleared {
                            p.result.clearance_tick.get_or_insert(tick);
                        }
                        if let Some(clear) = p.result.clearance_tick
                            && progress
                            && tick > clear
                        {
                            p.result.recovery_ticks.get_or_insert(tick - clear);
                        }
                        p.result.replans += metrics
                            .autopilot
                            .replans_by_unit
                            .get(&p.id)
                            .copied()
                            .unwrap_or(0) as usize;
                        if let Some(counters) = metrics.autopilot.congestion_by_unit.get(&p.id) {
                            let c = p.result.congestion.get_or_insert_with(Default::default);
                            c.checks += counters.clearance_checks;
                            c.early_starts += counters.early_starts;
                            c.timed_starts += counters.timed_starts;
                            c.deferrals += counters.resource_deferrals;
                            if let Some(clear) = c.first_watched_clearance
                                && counters.early_starts + counters.timed_starts > 0
                            {
                                c.clearance_to_search
                                    .get_or_insert(tick.saturating_sub(clear));
                            }
                        }
                        if let Some(c) = p.result.congestion.as_mut()
                            && let Some(clear) = c.first_watched_clearance
                            && distance > 0.02
                        {
                            c.clearance_to_movement
                                .get_or_insert(tick.saturating_sub(clear));
                        }
                        let controller = &world.btech.controllers()[&p.id];
                        if controller.state() == AutopilotState::Blocked {
                            p.result.blocking_reason = controller
                                .blocking_reason()
                                .map(|reason| format!("{reason:?}"));
                            p.result.outcome = "blocked".into();
                        }
                        if controller.state() == AutopilotState::Idle {
                            p.result.outcome = "completed".into();
                            p.terminal = true;
                        }
                    }
                    if let Some(file) = trace.as_mut() {
                        let bytes = serde_json::to_vec(&(
                            &world.btech,
                            &metrics.autopilot.notice_trace,
                            &metrics.autopilot.shots_by_unit,
                        ))?;
                        let digest = bytes.iter().fold(0xcbf29ce484222325u64, |hash, b| {
                            (hash ^ u64::from(*b)).wrapping_mul(0x100000001b3)
                        });
                        writeln!(
                            file,
                            "{}",
                            serde_json::json!({"schema":1,"scenario":scenario_name,"chassis":chassis,"seed":seed,"tick":tick,"digest":format!("{digest:016x}"),"participants":participants.iter().map(|p|&p.result).collect::<Vec<_>>(),"navigation":participants.iter().map(|p|serde_json::json!({"role":p.result.role,"motion":super::steering::motion(&world,p.id),"plan":world.btech.autopilot_plans.get(&p.id).map(|x|serde_json::json!({"route":x.route.iter().map(|h|(h.x,h.y)).collect::<Vec<_>>(),"index":x.route_index,"stagnant":x.stagnant_ticks,"retries":x.recovery_attempts,"goal":x.goal.map(|(h,r)|(h.x,h.y,r))}))})).collect::<Vec<_>>()})
                        )?;
                    }
                }
                for mut p in participants {
                    p.result.arc_fraction /= p.result.visible_ticks.max(1) as f64;
                    p.result.excess_distance = p.shortest.map(|d| (p.result.distance - d).max(0.0));
                    eprintln!("result: {}", serde_json::to_string(&p.result)?);
                    output.push(p.result);
                }
            }
        }
    }
    Ok(output)
}
