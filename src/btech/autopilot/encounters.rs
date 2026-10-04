//! Small seeded production-heartbeat encounters for comparing movement policies.
use super::*;
use crate::{
    BattlePower, BattleUnitTemplate, Config, HeartbeatHarness, Kind, MapAsset, ObjectId, World,
    persistence,
};
use anyhow::{Result, ensure};
use serde::Serialize;
use std::{io::Write, path::Path};

/// Deterministic outcome metrics; all durations count committed simulation ticks.
#[derive(Debug, Serialize)]
pub struct EncounterResult {
    pub scenario: String,
    pub chassis: String,
    pub seed: u8,
    pub ticks: usize,
    pub time_to_engage: Option<usize>,
    pub visible_ticks: usize,
    pub geometric_ticks: usize,
    pub ready_ticks: usize,
    pub arc_fraction: f64,
    pub distance: f64,
    pub settled_distance: f64,
    pub settled_ticks: usize,
    pub preferred_band_ticks: usize,
    pub final_range: Option<f64>,
    pub reversals: usize,
    pub overshoots: usize,
    pub replans: u64,
    pub shots: u64,
    pub damage: u64,
    pub state: AutopilotState,
}

/// Execute a fixed matrix. Trace records include complete gameplay checksums and tick metrics.
pub async fn run(
    ticks: usize,
    seeds: u8,
    scenario: Option<&str>,
    trace: Option<&Path>,
) -> Result<Vec<EncounterResult>> {
    run_policy(ticks, seeds, scenario, trace, Default::default()).await
}

/// Execute the same frozen fixtures with an explicit isolated-harness policy.
pub async fn run_policy(
    ticks: usize,
    seeds: u8,
    scenario: Option<&str>,
    trace: Option<&Path>,
    policy: super::interception::PursuitPolicy,
) -> Result<Vec<EncounterResult>> {
    ensure!(
        ticks > 0 && seeds > 0,
        "Encounter ticks and seeds must be positive"
    );
    let mut trace = trace.map(std::fs::File::create).transpose()?;
    let mut results = Vec::new();
    for name in [
        "approach",
        "long_approach",
        "close",
        "behind",
        "corner",
        "obstacle_pursuit",
        "fallback",
        "moving",
        "attack_move",
        "jammed",
    ] {
        if scenario.is_some_and(|selected| selected != name) {
            continue;
        }
        for (chassis, source) in [
            (
                "mech",
                include_str!("../../../game/mechs/JR7-D.toml").to_owned(),
            ),
            (
                "tracked",
                include_str!("../../../game/mechs/Demolisher.toml").to_owned(),
            ),
            (
                "wheeled",
                include_str!("../../../game/mechs/Demolisher.toml")
                    .replace("movement = \"track\"", "movement = \"wheel\""),
            ),
            (
                "hover",
                include_str!("../../../game/mechs/Demolisher.toml")
                    .replace("movement = \"track\"", "movement = \"hover\""),
            ),
        ] {
            for seed in 1..=seeds {
                let root = super::benchmark::copy_game_root()?;
                let config = Config::load(&root)?;
                let base = persistence::load(&config.database()).await?;
                let (world, shooter, target, map) =
                    fixture(&config, base.clone(), &source, name, seed)?;
                persistence::save(&config.database(), &world).await?;
                let mut harness = HeartbeatHarness::new(config.clone(), world)?;
                let mut result = EncounterResult {
                    scenario: name.into(),
                    chassis: chassis.into(),
                    seed,
                    ticks,
                    time_to_engage: None,
                    visible_ticks: 0,
                    geometric_ticks: 0,
                    ready_ticks: 0,
                    arc_fraction: 0.0,
                    distance: 0.0,
                    settled_distance: 0.0,
                    settled_ticks: 0,
                    preferred_band_ticks: 0,
                    final_range: None,
                    reversals: 0,
                    overshoots: 0,
                    replans: 0,
                    shots: 0,
                    damage: 0,
                    state: AutopilotState::Executing,
                };
                let mut stable = 0;
                let mut settled = false;
                let mut last_direction = 0;
                let mut last_side = 0;
                let initial_material = material(&harness.world(), target);
                for tick in 1..=ticks {
                    if ((name == "moving" && tick % 30 == 0)
                        || (name == "attack_move" && tick == 60))
                        && !harness.world().btech.constructed_units()[&target].is_destroyed()
                    {
                        settled = false;
                        stable = 0;
                        let mut world = harness.scripts().world_mut();
                        world.btech.constructed.get_mut(&target).unwrap().power = BattlePower::Off;
                        crate::btech::place_unit(
                            &mut world,
                            target,
                            map,
                            if name == "attack_move" {
                                6
                            } else {
                                6 + (tick / 30 % 2) as i64
                            },
                            if name == "attack_move" { 0 } else { 3 },
                        )?;
                        world.btech.constructed.get_mut(&target).unwrap().power =
                            BattlePower::Running;
                    }
                    let before = harness.world();
                    let point = crate::btech::scanner::scanner_unit(&before, shooter)
                        .unwrap()
                        .point
                        .unwrap();
                    let metrics = harness
                        .step_pursuit_policy(tick as i64, false, trace.is_some(), false, policy)
                        .await;
                    ensure!(
                        metrics.autopilot.pursuit_policy == policy,
                        "Encounter pursuit policy mismatch"
                    );
                    ensure!(metrics.committed, "Encounter heartbeat did not commit");
                    let after = harness.world();
                    let observation = observations::observe(&after, shooter, tick as i64)?;
                    let unit = crate::btech::scanner::scanner_unit(&after, shooter).unwrap();
                    let traveled = point.range(unit.point.unwrap())?;
                    result.distance += traveled;
                    if settled {
                        result.settled_distance += traveled;
                        result.settled_ticks += 1;
                    }
                    let (visible, geometric, ready, coverage) =
                        opportunity(&after, shooter, target)?;
                    result.visible_ticks += usize::from(visible);
                    result.geometric_ticks += usize::from(geometric);
                    result.ready_ticks +=
                        usize::from(ready || metrics.autopilot.autonomous_shots > 0);
                    result.arc_fraction += coverage;
                    let range = observation
                        .contacts
                        .iter()
                        .find(|c| c.unit == target)
                        .map(|c| c.range);
                    let band = after.btech.controllers()[&shooter]
                        .config()
                        .preferred_range
                        .unwrap();
                    let in_band = observation
                        .position
                        .zip(
                            observation
                                .contacts
                                .iter()
                                .find(|c| c.unit == target)
                                .map(|c| c.position),
                        )
                        .is_some_and(|(own, target)| {
                            let distance = navigation::Hex::new(own.x, own.y)
                                .distance(navigation::Hex::new(target.x, target.y));
                            distance >= u32::from(band.minimum)
                                && distance <= u32::from(band.maximum)
                        });
                    result.final_range = range;
                    result.preferred_band_ticks += usize::from(in_band);
                    stable =
                        if geometric && (in_band || name == "fallback") && unit.speed.abs() < 0.1 {
                            stable + 1
                        } else {
                            0
                        };
                    settled |= stable >= 5;
                    let direction = if unit.speed.abs() < 0.1 {
                        0
                    } else if unit.speed > 0.0 {
                        1
                    } else {
                        -1
                    };
                    if direction != 0 && last_direction != 0 && direction != last_direction {
                        result.reversals += 1;
                    }
                    if direction != 0 {
                        last_direction = direction;
                    }
                    let side = range.map_or(0, |r| {
                        if r < f64::from(band.minimum) {
                            -1
                        } else if r > f64::from(band.maximum) + 0.5 {
                            1
                        } else {
                            0
                        }
                    });
                    if side != 0 && last_side != 0 && side != last_side {
                        result.overshoots += 1;
                    }
                    if side != 0 {
                        last_side = side;
                    }
                    result.replans += metrics.autopilot.replans;
                    result.shots += metrics.autopilot.autonomous_shots;
                    if metrics.autopilot.autonomous_shots > 0 {
                        result.time_to_engage.get_or_insert(tick);
                    }
                    result.state = after.btech.controllers()[&shooter].state();
                    if let Some(trace) = trace.as_mut() {
                        let bytes =
                            serde_json::to_vec(&(&after.btech, &metrics.autopilot.notice_trace))?;
                        let digest = bytes.iter().fold(0xcbf29ce484222325u64, |hash, byte| {
                            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
                        });
                        writeln!(
                            trace,
                            "{}",
                            serde_json::json!({"scenario":name,"chassis":chassis,"seed":seed,"tick":tick,"digest":format!("{digest:016x}"),"visible":visible,"geometric":geometric,"ready":ready,"coverage":coverage,"distance":traveled,"shots":metrics.autopilot.autonomous_shots})
                        )?;
                    }
                }
                result.arc_fraction /= result.visible_ticks.max(1) as f64;
                result.damage = initial_material.saturating_sub(material(&harness.world(), target));
                eprintln!(
                    "encounter {name}/{chassis}/{seed}: engage={:?}, arcs={:.2}, distance={:.2}",
                    result.time_to_engage, result.arc_fraction, result.distance
                );
                eprintln!("result: {}", serde_json::to_string(&result)?);
                results.push(result);
            }
        }
    }
    ensure!(!results.is_empty(), "Unknown encounter scenario");
    Ok(results)
}

fn material(world: &World, id: ObjectId) -> u64 {
    world.btech.constructed_units().get(&id).map_or(0, |u| {
        u.sections
            .values()
            .map(|s| u64::from(s.armor) + u64::from(s.rear) + u64::from(s.internal))
            .sum()
    })
}

/// Sample geometry separately from transient weapon readiness, without rolling attack dice.
pub(crate) fn opportunity(
    world: &World,
    shooter: ObjectId,
    target: ObjectId,
) -> Result<(bool, bool, bool, f64)> {
    let observation = observations::observe(world, shooter, world.btech.simulation_time())?;
    let Some(contact) = observation
        .contacts
        .iter()
        .find(|c| c.unit == target && !c.known_destroyed)
    else {
        return Ok((false, false, false, 0.0));
    };
    let range = crate::btech::unit_range(world, shooter, target)?;
    let los = crate::btech::unit_terrain_los(world, shooter, target)?;
    let mut total = 0.0;
    let mut aligned = 0.0;
    let mut ready = false;
    for (index, weapon) in observation.own.weapons.iter().enumerate() {
        if !weapon.intact
            || weapon.spent
            || weapon.jammed
            || weapon.weapon.is_ams()
            || weapon.weapon.is_artillery()
        {
            continue;
        }
        let profile = weapon.weapon.profile();
        if profile.ammunition_per_ton > 0 && weapon.ammunition == 0 {
            continue;
        }
        let weight = f64::from(profile.damage) * f64::from(profile.missiles.max(1));
        total += weight;
        let bears = if world.btech.constructed_units().contains_key(&shooter) {
            crate::btech::weapon_bears_on(world, shooter, target, index)?
        } else {
            world.btech.vehicles()[&shooter].weapon_bears_on(index, range.bearing.unwrap_or(0.0))?
        };
        if bears && !los.blocked && contact.range <= f64::from(profile.long_range) {
            aligned += weight;
            let (mode, damage_heat, stored) =
                if let Some(unit) = world.btech.constructed_units().get(&shooter) {
                    (
                        unit.effective_fire_mode(index)?,
                        unit.weapon_damage_effects(index)?.heat,
                        unit.heat().stored,
                    )
                } else {
                    let unit = &world.btech.vehicles()[&shooter];
                    (unit.effective_fire_mode(index)?, 0, unit.weapon_heat())
                };
            let projected = u16::from(mode.launch_heat(
                weapon.weapon,
                (mode == crate::BattleFireMode::Gatling).then_some(6),
                true,
            )) + u16::from(damage_heat);
            let ceiling = world.btech.controllers()[&shooter].config().heat_ceiling;
            ready |= weapon.ready
                && observation.own.power == BattlePower::Running
                && stored + f64::from(projected) <= f64::from(ceiling);
        }
    }
    Ok((
        true,
        aligned > 0.0,
        ready,
        if total > 0.0 { aligned / total } else { 0.0 },
    ))
}

/// Standard encounter target; custom pursuit fixtures select a legal template at setup.
pub(super) fn fixture(
    config: &Config,
    world: World,
    source: &str,
    scenario: &str,
    seed: u8,
) -> Result<(World, ObjectId, ObjectId, ObjectId)> {
    fixture_with_target(
        config,
        world,
        source,
        include_str!("../../../game/mechs/AS7-S2.toml"),
        scenario,
        seed,
    )
}

pub(super) fn fixture_with_target(
    config: &Config,
    mut world: World,
    source: &str,
    target_source: &str,
    scenario: &str,
    seed: u8,
) -> Result<(World, ObjectId, ObjectId, ObjectId)> {
    let map = world.create(config, "encounter".into(), Kind::Room);
    let mut terrain = "12 12\n".to_owned();
    for y in 0..12 {
        for x in 0..12 {
            terrain.push_str(
                if matches!(scenario, "corner" | "obstacle_pursuit")
                    && x == 5
                    && (4..9).contains(&y)
                {
                    "=0"
                } else {
                    ".0"
                },
            );
        }
        terrain.push('\n');
    }
    crate::btech::create_map(
        &mut world,
        map,
        "encounter",
        MapAsset::from_cells(&terrain)?,
    )?;
    world.btech.maps.get_mut(&map).unwrap().fire_dice = Some(crate::BattleDice::seeded([seed; 32]));
    let shooter = world.create(config, "shooter".into(), Kind::Thing);
    let target = world.create(config, "target".into(), Kind::Thing);
    let start = match scenario {
        "close" => (6, 4),
        "long_approach" => (6, 11),
        "behind" | "jammed" => (6, 6),
        "corner" | "obstacle_pursuit" => (3, 6),
        _ => (6, 10),
    };
    for (id, template, x, y, team) in [
        (shooter, source, start.0, start.1, 1),
        (
            target,
            target_source,
            6,
            if scenario == "obstacle_pursuit" {
                6
            } else if scenario == "long_approach" {
                1
            } else if scenario == "attack_move" {
                5
            } else {
                3
            },
            2,
        ),
    ] {
        BattleUnitTemplate::parse("encounter", template)?
            .create(&mut world, id)
            .map_err(|e| anyhow::anyhow!("{scenario} {id:?} construction: {e:#}"))?;
        crate::btech::place_unit(&mut world, id, map, x, y)?;
        if let Some(unit) = world.btech.constructed.get_mut(&id) {
            unit.power = BattlePower::Running;
            unit.dice = crate::BattleDice::seeded([seed; 32]);
            unit.signature.team = team;
            let mut recovery = serde_json::to_value(&unit.crew_recovery)?;
            recovery["dice"] = serde_json::to_value(crate::BattleDice::seeded([seed; 32]))?;
            unit.crew_recovery = serde_json::from_value(recovery)?;
        } else {
            let unit = world.btech.vehicles.get_mut(&id).unwrap();
            unit.power = BattlePower::Running;
            unit.dice = crate::BattleDice::seeded([seed; 32]);
            unit.signature.team = team;
            let mut recovery = serde_json::to_value(&unit.crew_recovery)?;
            recovery["dice"] = serde_json::to_value(crate::BattleDice::seeded([seed; 32]))?;
            unit.crew_recovery = serde_json::from_value(recovery)?;
        }
    }
    if matches!(scenario, "behind" | "jammed") {
        if let Some(unit) = world.btech.constructed.get_mut(&shooter) {
            unit.motion.as_mut().unwrap().heading = 180.0;
            unit.motion.as_mut().unwrap().desired_heading = 180.0;
        } else {
            let unit = world.btech.vehicles.get_mut(&shooter).unwrap();
            unit.motion.as_mut().unwrap().heading = 180.0;
            unit.motion.as_mut().unwrap().desired_heading = 180.0;
            if scenario == "jammed" {
                unit.set_turret_conditions(true, false)?;
            }
        }
    }
    crate::btech::refresh_contacts(&mut world, &[shooter])?;
    let band = AutopilotRangeBand {
        minimum: if scenario == "fallback" {
            20
        } else if scenario == "obstacle_pursuit" {
            1
        } else {
            2
        },
        maximum: if scenario == "fallback" {
            21
        } else if scenario == "obstacle_pursuit" {
            1
        } else {
            3
        },
    };
    let mut controller = AutopilotController::with_config(AutopilotConfig {
        fire_mode: AutopilotFireMode::Opportunistic,
        preferred_range: Some(band),
        heat_ceiling: 8,
        ..Default::default()
    })?;
    controller.submit(
        vec![if scenario == "attack_move" {
            AutopilotOrder::AttackMove {
                destination: crate::BattlePosition { map, x: 6, y: 1 },
                arrival_radius: 0,
            }
        } else {
            AutopilotOrder::Attack {
                target,
                range: None,
            }
        }],
        AutopilotSubmissionMode::Replace,
        None,
    )?;
    controller.resume(None)?;
    world.btech.controllers.insert(shooter, controller);
    Ok((world, shooter, target, map))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pure control decisions leave dice untouched and use admitted uncrewed controls.
    #[tokio::test]
    async fn alignment_and_range_use_own_capability_without_consuming_dice() {
        let root = super::super::benchmark::copy_game_root().unwrap();
        let config = Config::load(&root).unwrap();
        let base = persistence::load(&config.database()).await.unwrap();
        for source in [
            include_str!("../../../game/mechs/JR7-D.toml"),
            include_str!("../../../game/mechs/Demolisher.toml"),
        ] {
            let (mut world, id, target, _) =
                fixture(&config, base.clone(), source, "behind", 1).unwrap();
            let observation = observations::observe(&world, id, 0).unwrap();
            let band = super::super::engagement::preferred(&observation);
            let mut recycling = observation.clone();
            for weapon in &mut recycling.own.weapons {
                weapon.ready = false;
                weapon.recycle_remaining = 30;
            }
            assert_eq!(band, super::super::engagement::preferred(&recycling));
            let dice = serde_json::to_value(&world.btech).unwrap();
            let target_position = observation
                .contacts
                .iter()
                .find(|c| c.unit == target)
                .unwrap()
                .position;
            let mut notices = Vec::new();
            super::super::alignment::align(
                &mut world,
                id,
                target_position,
                &observation,
                true,
                &mut notices,
            );
            assert!(!notices.is_empty());
            assert_eq!(
                world.btech.controllers()[&id].state(),
                AutopilotState::Executing
            );
            let after = serde_json::to_value(&world.btech).unwrap();
            for category in ["constructed", "vehicles"] {
                for (key, unit) in dice[category].as_object().unwrap() {
                    assert_eq!(unit["dice"], after[category][key]["dice"]);
                }
            }
        }
    }

    /// Engagement searches go around impassable walls, and braking forecasts do not move units.
    #[tokio::test]
    async fn obstacle_region_and_short_braking_distance_use_ordinary_controls() {
        let root = super::super::benchmark::copy_game_root().unwrap();
        let config = Config::load(&root).unwrap();
        let base = persistence::load(&config.database()).await.unwrap();
        let (mut world, id, _, _) = fixture(
            &config,
            base,
            include_str!("../../../game/mechs/JR7-D.toml"),
            "obstacle_pursuit",
            1,
        )
        .unwrap();
        super::super::runtime::advance(&mut world, &config, 1).unwrap();
        let plan = &world.btech.autopilot_plans[&id];
        assert!(!plan.route.is_empty());
        assert!(plan.route.iter().any(|cell| cell.y < 4 || cell.y >= 9));
        assert!(
            plan.route
                .iter()
                .all(|cell| cell.x != 5 || cell.y < 4 || cell.y >= 9)
        );
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        let maximum = unit.mobility().maximum_speed;
        let motion = unit.motion.as_mut().unwrap();
        motion.heading = 0.0;
        motion.desired_heading = 0.0;
        motion.speed = maximum;
        motion.desired_speed = maximum;
        let before = motion.point;
        super::super::steering::drive(
            &mut world,
            &config,
            id,
            &[navigation::Hex::new(3, 5)],
            0,
            maximum,
            None,
            false,
            None,
            &mut Default::default(),
            2,
            &mut Vec::new(),
        )
        .unwrap();
        let motion = world.btech.constructed_units()[&id].motion().unwrap();
        assert_eq!(motion.desired_speed, 0.0);
        assert_eq!(motion.point, before);
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        unit.motion.as_mut().unwrap().speed = -maximum / 2.0;
        unit.motion.as_mut().unwrap().desired_speed = -maximum / 2.0;
        super::super::steering::drive(
            &mut world,
            &config,
            id,
            &[navigation::Hex::new(3, 2)],
            0,
            maximum,
            None,
            false,
            None,
            &mut Default::default(),
            3,
            &mut Vec::new(),
        )
        .unwrap();
        assert_eq!(
            world.btech.constructed_units()[&id]
                .motion()
                .unwrap()
                .desired_speed,
            0.0
        );
    }

    /// A nearly cardinal hull must not create the invalid 360-degree turret endpoint.
    #[tokio::test]
    async fn turret_alignment_canonicalizes_rounding_at_zero() {
        let root = super::super::benchmark::copy_game_root().unwrap();
        let config = Config::load(&root).unwrap();
        let base = persistence::load(&config.database()).await.unwrap();
        let (mut world, id, _, _) = fixture(
            &config,
            base,
            include_str!("../../../game/mechs/Demolisher.toml"),
            "behind",
            1,
        )
        .unwrap();
        world
            .btech
            .vehicles
            .get_mut(&id)
            .unwrap()
            .motion
            .as_mut()
            .unwrap()
            .heading = f64::EPSILON;
        crate::btech::vehicle_turret::set_turret_autopilot(&mut world, id, 0.0).unwrap();
        assert_eq!(world.btech.vehicles()[&id].turret_offset, 0.0);
        world.validate(&config).unwrap();
        assert_eq!(
            world.btech.controllers()[&id].state(),
            AutopilotState::Executing
        );
    }

    /// Damaged turret rotation is rejected without pausing the controller.
    #[tokio::test]
    async fn jammed_turret_requires_hull_alignment_and_preserves_takeover_rules() {
        let root = super::super::benchmark::copy_game_root().unwrap();
        let config = Config::load(&root).unwrap();
        let base = persistence::load(&config.database()).await.unwrap();
        let (mut world, id, target, _) = fixture(
            &config,
            base,
            include_str!("../../../game/mechs/Demolisher.toml"),
            "jammed",
            1,
        )
        .unwrap();
        assert!(crate::btech::vehicle_turret::set_turret_autopilot(&mut world, id, 0.0).is_err());
        let observation = observations::observe(&world, id, 0).unwrap();
        let position = observation
            .contacts
            .iter()
            .find(|c| c.unit == target)
            .unwrap()
            .position;
        super::super::alignment::align(
            &mut world,
            id,
            position,
            &observation,
            true,
            &mut Vec::new(),
        );
        assert_ne!(
            world.btech.vehicles()[&id]
                .motion()
                .unwrap()
                .desired_heading,
            180.0
        );
        assert_eq!(
            world.btech.controllers()[&id].state(),
            AutopilotState::Executing
        );
    }
}
