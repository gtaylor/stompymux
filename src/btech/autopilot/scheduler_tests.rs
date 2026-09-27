//! Scheduler regressions under deliberately reduced shared budgets.
use super::*;
use crate::{BattleMapAsset, BattleUnitTemplate, Kind};

fn fixture() -> (Config, World, Vec<ObjectId>) {
    let config =
        Config::load(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/game"))
            .unwrap();
    let mut world = World::default();
    let map = world.create(&config, "Scheduler test".into(), Kind::Room);
    crate::create_battle_map(
        &mut world,
        map,
        "scheduler",
        BattleMapAsset::parse(&format!("20 20\n{}", (".0".repeat(20) + "\n").repeat(20))).unwrap(),
    )
    .unwrap();
    let mut units = Vec::new();
    for row in [2, 8, 14] {
        let id = world.create(&config, format!("Scheduler unit {row}"), Kind::Thing);
        BattleUnitTemplate::parse(include_str!("../../../game/mechs/JR7-D"))
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        crate::place_battle_unit(&mut world, id, map, 1, row).unwrap();
        world.btech.constructed.get_mut(&id).unwrap().power = BattlePower::Running;
        let mut controller = super::super::AutopilotController::new();
        controller
            .submit(
                vec![AutopilotOrder::Move {
                    destination: BattlePosition {
                        map,
                        x: 18,
                        y: row as u16,
                    },
                    arrival_radius: 0,
                }],
                super::super::AutopilotSubmissionMode::Replace,
                None,
            )
            .unwrap();
        controller.resume(None).unwrap();
        world.btech.controllers.insert(id, controller);
        units.push(id);
    }
    (config, world, units)
}

#[test]
fn one_expansion_budget_rotates_service_until_every_route_is_found() {
    let (config, mut world, ids) = fixture();
    let mut found = std::collections::BTreeSet::new();
    for tick in 1..=400 {
        let mut metrics = AutopilotRuntimeMetrics::default();
        advance_budgeted(&mut world, &config, tick, Some(&mut metrics), 1, 1200).unwrap();
        assert!(metrics.expansions <= 1);
        assert!(total_search_records(&world) <= 1200);
        for id in &ids {
            if world
                .btech
                .autopilot_plans
                .get(id)
                .is_some_and(|p| !p.route.is_empty())
            {
                found.insert(*id);
                world
                    .btech
                    .controllers
                    .get_mut(id)
                    .unwrap()
                    .pause(None)
                    .unwrap();
            }
        }
        if found.len() == ids.len() {
            return;
        }
    }
    panic!("rotating scheduler starved a route under a one-expansion global budget");
}

#[test]
fn single_frontier_reservation_defers_other_jobs_without_deadlock_or_failure() {
    let (config, mut world, ids) = fixture();
    let mut found = std::collections::BTreeSet::new();
    for tick in 1..=400 {
        advance_budgeted(&mut world, &config, tick, None, 1, 400).unwrap();
        let frontiers = world
            .btech
            .autopilot_plans
            .values()
            .filter(|p| p.search.is_some())
            .count();
        assert!(frontiers <= 1);
        assert!(total_search_records(&world) <= 400);
        assert!(
            world
                .btech
                .controllers()
                .values()
                .all(|c| c.state() != AutopilotState::Blocked)
        );
        for id in &ids {
            if world
                .btech
                .autopilot_plans
                .get(id)
                .is_some_and(|p| !p.route.is_empty())
            {
                found.insert(*id);
                world
                    .btech
                    .controllers
                    .get_mut(id)
                    .unwrap()
                    .pause(None)
                    .unwrap();
            }
        }
        if found.len() == ids.len() {
            return;
        }
    }
    panic!("deferred route never acquired the released reservation");
}

#[test]
fn valid_combat_target_is_retained_between_reassessment_ticks_but_loss_is_immediate() {
    use super::super::observations::{AutopilotContact, AutopilotOwnReadiness};
    let observer = ObjectId(10);
    let old = ObjectId(20);
    let challenger = ObjectId(21);
    let contact = |unit, range| AutopilotContact {
        unit,
        position: BattlePosition {
            map: ObjectId(1),
            x: 1,
            y: 1,
        },
        friendly: false,
        identified: true,
        known_destroyed: false,
        range,
        seen_at: 2,
    };
    let mut observation = AutopilotObservation {
        unit: observer,
        time: 2,
        position: None,
        heading: None,
        speed: 0.0,
        own: AutopilotOwnReadiness {
            power: BattlePower::Running,
            maximum_speed: 0.0,
            heat: None,
            weapons: Vec::new(),
        },
        contacts: vec![contact(old, 10.0), contact(challenger, 1.0)],
        remembered: Vec::new(),
    };
    let config = AutopilotConfig {
        fire_mode: super::super::AutopilotFireMode::Opportunistic,
        ..Default::default()
    };
    assert_eq!(combat_target(&config, &observation, Some(old)), Some(old));
    observation.time = 4;
    assert_eq!(
        combat_target(&config, &observation, Some(old)),
        Some(challenger)
    );
    observation.time = 5;
    observation.contacts[0].known_destroyed = true;
    assert_eq!(
        combat_target(&config, &observation, Some(old)),
        Some(challenger)
    );
    observation.contacts[0].known_destroyed = false;
    observation.contacts[0].friendly = true;
    assert_eq!(
        combat_target(&config, &observation, Some(old)),
        Some(challenger)
    );
}

#[test]
fn projected_heat_accounts_for_burst_modes_damage_and_gatling_bound_without_dice() {
    use crate::{BattleFireMode as Mode, BattleWeapon as Weapon};
    assert_eq!(
        projected_launch_heat(Weapon::MediumLaser, Mode::Normal, 2),
        u16::from(Weapon::MediumLaser.profile().heat) + 2
    );
    assert_eq!(
        projected_launch_heat(Weapon::UltraAc5, Mode::Ultra, 0),
        2 * u16::from(Weapon::UltraAc5.profile().heat)
    );
    assert_eq!(
        projected_launch_heat(Weapon::MachineGun, Mode::Gatling, 1),
        7
    );
}

/// Attribution is opt-in and never mistakes an opposing unit's shot for the focal unit.
#[test]
fn shot_attribution_is_per_unit_and_optional() {
    let mut metrics = AutopilotRuntimeMetrics::default();
    metrics.record_shot(ObjectId(1));
    assert!(metrics.shots_by_unit.is_empty());
    metrics.capture_outcomes = true;
    metrics.record_shot(ObjectId(2));
    metrics.record_shot(ObjectId(2));
    metrics.record_shot(ObjectId(3));
    assert_eq!(metrics.shots_by_unit[&ObjectId(2)], 2);
    assert_eq!(metrics.shots_by_unit[&ObjectId(3)], 1);
    assert!(!metrics.shots_by_unit.contains_key(&ObjectId(1)));
    let mut merged = AutopilotRuntimeMetrics::default();
    merged.merge(&metrics);
    assert_eq!(merged.shots_by_unit, metrics.shots_by_unit);
}

/// Isolated corridor with four known blockers and one controlled observer.
fn crowded_fixture() -> (Config, World, Vec<ObjectId>) {
    let config = Config::load("tests/fixtures/game").unwrap();
    crowded_world(config, World::default())
}

/// Preserve a supplied seed world's required objects when using the heartbeat harness.
fn crowded_world(config: Config, mut world: World) -> (Config, World, Vec<ObjectId>) {
    let map = world.create(&config, "Corridor".into(), Kind::Room);
    crate::create_battle_map(
        &mut world,
        map,
        "corridor",
        BattleMapAsset::parse(&format!("1 8\n{}", ".0\n".repeat(8))).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for index in 0..5 {
        let id = world.create(&config, format!("unit {index}"), Kind::Thing);
        BattleUnitTemplate::parse(include_str!("../../../game/mechs/JR7-D"))
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        crate::place_battle_unit(&mut world, id, map, 0, if index == 0 { 0 } else { 3 }).unwrap();
        let unit = world.btech.constructed.get_mut(&id).unwrap();
        unit.power = BattlePower::Running;
        unit.signature.team = 1;
        ids.push(id);
    }
    let mut controller = super::super::AutopilotController::new();
    controller
        .submit(
            vec![AutopilotOrder::Move {
                destination: BattlePosition { map, x: 0, y: 7 },
                arrival_radius: 0,
            }],
            super::super::AutopilotSubmissionMode::Replace,
            None,
        )
        .unwrap();
    controller.resume(None).unwrap();
    world.btech.controllers.insert(ids[0], controller);
    (config, world, ids)
}

/// Congestion waits are bounded, consume no search work while waiting, and recover on clearance.
#[test]
fn crowded_route_retries_then_recovers_or_blocks() {
    let (config, mut world, ids) = crowded_fixture();
    let map = world.btech.constructed_units()[&ids[0]]
        .position()
        .unwrap()
        .map;
    advance(&mut world, &config, 1).unwrap();
    assert_eq!(
        world.btech.controllers()[&ids[0]].state(),
        AutopilotState::Executing
    );
    assert_eq!(world.btech.autopilot_plans[&ids[0]].congestion.deadline, 31);
    let mut permanently_crowded = world.clone();
    let mut metrics = AutopilotRuntimeMetrics::default();
    advance_with_metrics(&mut world, &config, 2, &mut metrics).unwrap();
    assert_eq!(metrics.expansions, 0);
    metrics.capture_outcomes = true;
    advance_with_metrics(&mut world, &config, 6, &mut metrics).unwrap();
    assert_eq!(metrics.congestion_by_unit[&ids[0]].clearance_checks, 1);
    // Moving one of four blockers still leaves three friends in the watched cell.
    world.btech.constructed.get_mut(&ids[1]).unwrap().power = BattlePower::Off;
    crate::place_battle_unit(&mut world, ids[1], map, 0, 4).unwrap();
    advance_with_metrics(&mut world, &config, 11, &mut metrics).unwrap();
    assert_eq!(metrics.expansions, 0);
    assert_eq!(metrics.congestion_by_unit[&ids[0]].early_starts, 0);
    for (index, id) in ids.iter().skip(1).enumerate() {
        world.btech.constructed.get_mut(id).unwrap().power = BattlePower::Off;
        crate::place_battle_unit(&mut world, *id, map, 0, 4 + index as i64).unwrap();
    }
    // Clearance qualifies, but quota denial must not consume the early attempt.
    advance_budgeted(&mut world, &config, 16, Some(&mut metrics), 256, 0).unwrap();
    assert_eq!(world.btech.autopilot_plans[&ids[0]].congestion.early, 0);
    assert_eq!(metrics.congestion_by_unit[&ids[0]].resource_deferrals, 1);
    advance_with_metrics(&mut world, &config, 17, &mut metrics).unwrap();
    assert!(!world.btech.autopilot_plans[&ids[0]].route.is_empty());
    assert_eq!(metrics.congestion_by_unit[&ids[0]].early_starts, 1);
    assert_eq!(metrics.congestion_by_unit[&ids[0]].timed_starts, 0);
    assert_eq!(
        world.btech.autopilot_plans[&ids[0]].congestion,
        Default::default()
    );
    for tick in [31, 61, 91] {
        advance(&mut permanently_crowded, &config, tick).unwrap();
    }
    assert_eq!(
        permanently_crowded.btech.controllers()[&ids[0]].state(),
        AutopilotState::Blocked
    );
}

/// A replaced intention and a serialized restart cannot inherit a clearance watch.
#[test]
fn congestion_watches_do_not_survive_replacement_or_restart() {
    let (config, mut world, ids) = crowded_fixture();
    advance(&mut world, &config, 1).unwrap();
    assert!(world.btech.autopilot_plans[&ids[0]].congestion.waiting);
    let mut taken_over = world.clone();
    assert!(super::super::manual_takeover(&mut taken_over, ids[0]).unwrap());
    assert!(!taken_over.btech.autopilot_plans.contains_key(&ids[0]));
    let mut removed = world.clone();
    let hangar = removed.create(&config, "Hangar".into(), Kind::Room);
    removed.btech.constructed.get_mut(&ids[0]).unwrap().power = BattlePower::Off;
    crate::btech::placement::remove_unit(&mut removed, ids[0], hangar).unwrap();
    assert!(!removed.btech.autopilot_plans.contains_key(&ids[0]));
    let mut destroyed = world.clone();
    destroyed
        .btech
        .constructed
        .get_mut(&ids[0])
        .unwrap()
        .sections
        .get_mut(&crate::BattleSection::CenterTorso)
        .unwrap()
        .internal = 0;
    advance(&mut destroyed, &config, 2).unwrap();
    assert!(!destroyed.btech.autopilot_plans.contains_key(&ids[0]));
    let restarted: World = serde_json::from_value(serde_json::to_value(&world).unwrap()).unwrap();
    assert!(restarted.btech.autopilot_plans.is_empty());
    world
        .btech
        .controllers
        .get_mut(&ids[0])
        .unwrap()
        .submit(
            vec![AutopilotOrder::Hold],
            super::super::AutopilotSubmissionMode::Replace,
            None,
        )
        .unwrap();
    advance(&mut world, &config, 2).unwrap();
    assert!(!world.btech.autopilot_plans.contains_key(&ids[0]));
}

/// Opt-in runtime-only measurement isolates polling from SQLite, combat and motion.
#[test]
#[ignore = "release-only 100-waiter polling benchmark"]
fn waiting_controller_polling_benchmark() {
    let (config, mut base, ids) = crowded_fixture();
    let pos = base.btech.constructed_units()[&ids[0]].position().unwrap();
    let controller = base.btech.controllers()[&ids[0]].clone();
    for n in 1..100 {
        let id = base.create(&config, format!("waiter {n}"), Kind::Thing);
        BattleUnitTemplate::parse(include_str!("../../../game/mechs/JR7-D"))
            .unwrap()
            .create(&mut base, id)
            .unwrap();
        crate::place_battle_unit(&mut base, id, pos.map, 0, 0).unwrap();
        let unit = base.btech.constructed.get_mut(&id).unwrap();
        unit.power = BattlePower::Running;
        unit.signature.team = 1;
        base.btech.controllers.insert(id, controller.clone());
    }
    advance(&mut base, &config, 1).unwrap();
    assert_eq!(
        base.btech
            .autopilot_plans
            .values()
            .filter(|p| p.congestion.waiting)
            .count(),
        100
    );
    for polling in [false, true] {
        let mut times = Vec::new();
        let mut poll_times = Vec::new();
        for _ in 0..20 {
            let mut world = base.clone();
            if !polling {
                for plan in world.btech.autopilot_plans.values_mut() {
                    plan.congestion.early = 3;
                }
            }
            for tick in 2..=30 {
                let mut metrics = AutopilotRuntimeMetrics {
                    capture_outcomes: true,
                    ..Default::default()
                };
                let start = Instant::now();
                advance_with_metrics(&mut world, &config, tick, &mut metrics).unwrap();
                let elapsed = start.elapsed().as_secs_f64() * 1000.0;
                assert_eq!(metrics.expansions, 0);
                assert_eq!(metrics.controller_ticks, 100);
                if tick > 5 {
                    times.push(elapsed);
                    if metrics
                        .congestion_by_unit
                        .values()
                        .any(|m| m.clearance_checks > 0)
                    {
                        poll_times.push(elapsed);
                    }
                }
            }
        }
        times.sort_by(f64::total_cmp);
        poll_times.sort_by(f64::total_cmp);
        println!(
            "waiting_polling={polling},samples={},p50_ms={:.3},p95_ms={:.3},poll_tick_p95_ms={:?}",
            times.len(),
            times[times.len() / 2],
            times[(times.len() * 95).div_ceil(100) - 1],
            poll_times.get((poll_times.len() * 95).div_ceil(100).saturating_sub(1))
        );
    }
}

/// A rejected persistence transaction cannot publish clearance observations or retry state.
#[tokio::test(flavor = "current_thread")]
async fn congestion_polling_rollback_discards_transient_work() {
    let root = super::super::benchmark::copy_game_root().unwrap();
    // Store the clock every tick, so the rejected tick reaches the database.
    let toml = root.path().join("stompymux.toml");
    let text = std::fs::read_to_string(&toml).unwrap();
    std::fs::write(
        &toml,
        text.replace("clock_save_interval = 60\n", "clock_save_interval = 1\n"),
    )
    .unwrap();
    let config = Config::load(&root).unwrap();
    assert_eq!(config.database.clock_save_interval, 1);
    let world = crate::persistence::load(&config.database()).await.unwrap();
    let (_, world, ids) = crowded_world(config.clone(), world);
    crate::persistence::save(&config.database(), &world)
        .await
        .unwrap();
    let mut harness = crate::HeartbeatHarness::new(config.clone(), world).unwrap();
    for tick in 1..=5 {
        assert!(harness.step(tick).await.committed);
    }
    assert!(
        harness.world().btech.autopilot_plans[&ids[0]]
            .congestion
            .waiting
    );
    let before = serde_json::to_value(harness.world()).unwrap();
    let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(config.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER clearance_fail BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(FAIL,'clearance rollback'); END;")
        .execute(&mut db).await.unwrap();
    let rejected = harness.step_diagnostic(6, false, true).await;
    assert!(!rejected.committed);
    assert!(rejected.autopilot.congestion_by_unit.is_empty());
    assert!(rejected.autopilot.notice_trace.is_empty());
    assert!(harness.world().btech.autopilot_plans.is_empty());
    assert_eq!(serde_json::to_value(harness.world()).unwrap(), before);
}

/// Standing posture is not movement readiness until the rising countdown expires.
#[test]
fn rising_mech_waits_without_failing_or_spending_navigation_work() {
    let (config, mut world, ids) = crowded_fixture();
    advance(&mut world, &config, 1).unwrap();
    let unit = world.btech.constructed.get_mut(&ids[0]).unwrap();
    unit.stand_timer = Some(crate::btech::BattleStandTimer::Rising { remaining: 4 });
    assert_eq!(unit.posture(), crate::btech::BattlePosture::Standing);
    let before = world.battle_roll_statistics().unwrap();
    let mut metrics = AutopilotRuntimeMetrics::default();
    advance_with_metrics(&mut world, &config, 6, &mut metrics).unwrap();
    assert_eq!(
        world.btech.controllers()[&ids[0]].state(),
        AutopilotState::Executing
    );
    assert_eq!(metrics.expansions, 0);
    assert_eq!(world.battle_roll_statistics().unwrap(), before);
    assert_eq!(world.btech.autopilot_plans[&ids[0]].congestion.poll_at, 6);
    world
        .btech
        .constructed
        .get_mut(&ids[0])
        .unwrap()
        .stand_timer = None;
    metrics.capture_outcomes = true;
    advance_with_metrics(&mut world, &config, 7, &mut metrics).unwrap();
    assert_eq!(metrics.congestion_by_unit[&ids[0]].clearance_checks, 1);
}
