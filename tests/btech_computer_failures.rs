//! Live computer failures preserve sensor timers, target state, shutdown and database replay.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc, sync::OnceLock};
use stompymux_rs::*;

/// Find reproducible real random streams once; host tests do not bypass the failure selector.
fn stream(effect: BattleComputerFailure) -> BattleDice {
    static STREAMS: OnceLock<Vec<(BattleComputerFailure, BattleDice)>> = OnceLock::new();
    STREAMS
        .get_or_init(|| {
            let mut dice = BattleDice::seeded([37; 32]);
            let mut found = Vec::new();
            for _ in 0..10_000_000 {
                let before = dice.clone();
                let effect = select_computer_failure(
                    &mut dice,
                    BattleComputerFailureInput {
                        parts_enabled: true,
                        clan: false,
                        has_target: true,
                        tactical_range: 20,
                        long_range: 40,
                        scanner_range: 20,
                    },
                )
                .unwrap();
                if let Some(effect) = effect {
                    if !found.iter().any(|(old, _)| *old == effect) {
                        found.push((effect, before));
                    }
                    if found.len() == 6 {
                        return found;
                    }
                }
            }
            panic!("all failure outcomes must be found");
        })
        .iter()
        .find(|(kind, _)| *kind == effect)
        .unwrap()
        .1
        .clone()
}

/// Set a turn-boundary stream and installed quality through the saved representation.
fn prepare(world: &mut World, id: ObjectId, effect: BattleComputerFailure) {
    firing::edit(world, id, |unit| {
        unit["dice"] = serde_json::to_value(stream(effect)).unwrap();
        unit["definition"]["attributes"]["computer"] = "1".into();
    });
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["turn_clock"] = 29.into();
    world.btech = serde_json::from_value(state).unwrap();
}

fn ranges(world: &World, id: ObjectId) -> BattleSensorRanges {
    world
        .btech
        .constructed_units()
        .get(&id)
        .map(|u| u.sensor_ranges())
        .unwrap_or_else(|| world.btech.vehicles()[&id].sensor_ranges())
}

/// Both anatomy stores receive independent saved timers and recover while shut down with parts disabled.
#[tokio::test]
async fn display_failures_recover_in_order_and_survive_restart() {
    for template in firing::templates()
        .into_iter()
        .enumerate()
        .filter(|(i, _)| *i != 5)
        .map(|(_, t)| t)
    {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(&template, None, &template).await;
        prepare(&mut world, unit, BattleComputerFailure::AllDisplays);
        let original = ranges(&world, unit);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        advance_battle_computer_failures_action(&scripts, &config).unwrap();
        assert_eq!(
            ranges(&scripts.world(), unit),
            BattleSensorRanges {
                tactical: 0,
                long_range: 0,
                scan: 0
            }
        );
        let saved = serde_json::to_value(&scripts.world().btech).unwrap();
        let events = saved["sensor_recoveries"].as_array().unwrap();
        assert_eq!(events.len(), 3);
        for (event, display) in events.iter().zip(["tactical", "long_range", "scanner"]) {
            assert_eq!(event["display"], display);
            assert!((30..=200).contains(&event["remaining"].as_u64().unwrap()));
        }
        stop_battle_unit_action(&scripts, &config, unit, ObjectId(1)).unwrap();
        let world_snapshot = scripts.world().clone();
        persistence::save(&config.database(), &world_snapshot)
            .await
            .unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, scripts.world().btech);
        let replay = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
        let config = parts_disabled(_dir.path());
        for _ in 0..200 {
            advance_battle_computer_failures_action(&scripts, &config).unwrap();
            advance_battle_computer_failures_action(&replay, &config).unwrap();
        }
        assert_eq!(replay.world().btech, scripts.world().btech);
        assert_eq!(
            ranges(&scripts.world(), unit),
            BattleSensorRanges {
                tactical: original.tactical,
                long_range: 127,
                scan: 127
            }
        );
        assert!(
            serde_json::to_value(&scripts.world().btech).unwrap()["sensor_recoveries"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        let world_snapshot = scripts.world().clone();
        persistence::save(&config.database(), &world_snapshot)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            scripts.world().btech
        );
    }
}

/// Actual target loss uses the shared targeting service; shutdown uses ordinary power consequences.
#[tokio::test]
async fn target_loss_shutdown_and_ineligible_checks_use_live_state() {
    for index in [0, 2, 5, 6] {
        let template = &firing::templates()[index];
        for effect in [
            BattleComputerFailure::LoseTarget,
            BattleComputerFailure::Shutdown,
        ] {
            let (_dir, config, mut world, unit, _, _) =
                firing::fixture_with_target(template, None, template).await;
            prepare(&mut world, unit, effect);
            let before = world.btech.clone();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            advance_battle_computer_failures_action(&scripts, &config).unwrap();
            if index == 5 {
                assert_eq!(before, scripts.world().btech);
                continue;
            }
            if effect == BattleComputerFailure::LoseTarget {
                let target = scripts
                    .world()
                    .btech
                    .constructed_units()
                    .get(&unit)
                    .map(|u| u.target_lock())
                    .unwrap_or_else(|| scripts.world().btech.vehicles()[&unit].target_lock());
                assert!(target.is_none());
            } else {
                let power = scripts
                    .world()
                    .btech
                    .constructed_units()
                    .get(&unit)
                    .map(|u| u.power())
                    .unwrap_or_else(|| scripts.world().btech.vehicles()[&unit].power());
                assert_eq!(power, BattlePower::Off);
            }
        }
    }
}

/// A later unit error restores earlier recovery, failure, random draws and staged output together.
#[tokio::test]
async fn later_failure_rolls_back_the_entire_computer_action() {
    let template = &firing::templates()[0];
    let (_dir, config, mut world, unit, target, _) =
        firing::fixture_with_target(template, None, template).await;
    prepare(&mut world, unit, BattleComputerFailure::AllDisplays);
    firing::edit(&mut world, target, |state| {
        state["definition"]["attributes"]["computer"] = "13".into();
    });
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let error = advance_battle_computer_failures_action(&scripts, &config).unwrap_err();
    assert!(error.to_string().contains("catalogue entry"));
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    firing::edit(&mut scripts.world_mut(), target, |state| {
        state["definition"]["attributes"]["computer"] = "5".into();
    });
    advance_battle_computer_failures_action(&scripts, &config).unwrap();
    assert_eq!(ranges(&scripts.world(), unit).tactical, 0);
}

/// An administrative repair can be followed by another outage without replacing the first timer.
#[tokio::test]
async fn overlapping_recoveries_preserve_insertion_order() {
    let template = &firing::templates()[2];
    let (_dir, config, mut world, unit, _, _) =
        firing::fixture_with_target(template, None, template).await;
    prepare(&mut world, unit, BattleComputerFailure::Tactical);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    advance_battle_computer_failures_action(&scripts, &config).unwrap();
    firing::edit(&mut scripts.world_mut(), unit, |state| {
        state["hardware"]["tactical"]["value"] = 37.into();
    });
    prepare(
        &mut scripts.world_mut(),
        unit,
        BattleComputerFailure::Tactical,
    );
    advance_battle_computer_failures_action(&scripts, &config).unwrap();
    let mut state = serde_json::to_value(&scripts.world().btech).unwrap();
    assert_eq!(state["sensor_recoveries"].as_array().unwrap().len(), 2);
    for event in state["sensor_recoveries"].as_array_mut().unwrap() {
        event["remaining"] = 1.into();
    }
    scripts.world_mut().btech = serde_json::from_value(state).unwrap();
    let config = parts_disabled(_dir.path());
    advance_battle_computer_failures_action(&scripts, &config).unwrap();
    assert_eq!(ranges(&scripts.world(), unit).tactical, 37);
}

/// Load the real host parts switch while retaining the isolated database and scripts.
fn parts_disabled(dir: &std::path::Path) -> Config {
    let path = dir.join("stompymux.toml");
    let mut settings: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    settings["battletech"]
        .as_table_mut()
        .unwrap()
        .insert("parts".into(), 0.into());
    std::fs::write(path, toml::to_string(&settings).unwrap()).unwrap();
    Config::load(dir).unwrap()
}

/// The real heartbeat retries the same failure after a database error without publishing it early.
#[tokio::test(flavor = "current_thread")]
async fn server_failure_and_recovery_queue_commit_together() {
    use sqlx::{Connection, SqliteConnection};
    use std::{cell::Cell, time::Duration};
    tokio::task::LocalSet::new().run_until(async {
        let template = &firing::templates()[2];
        let (_dir, config, mut world, unit, _, _) = firing::fixture_with_target(template, None, template).await;
        prepare(&mut world, unit, BattleComputerFailure::AllDisplays);
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["turn_clock"] = 28.into();
        world.btech = serde_json::from_value(state).unwrap();
        world.accounts.get_mut(&ObjectId(1)).unwrap().hash = Some(accounts::hash("secret", &config).unwrap());
        persistence::save(&config.database(), &world).await.unwrap();
        let before = world.btech.vehicles()[&unit].clone();
        let mut sql = SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("CREATE TRIGGER deny_computer BEFORE UPDATE ON btech_simulation_clock BEGIN SELECT RAISE(ABORT,'computer save failure'); END").execute(&mut sql).await.unwrap();
        let (addr, shutdown, task, _) = support::start(&config, Rc::new(Cell::new(1))).await;
        let mut client = support::Client { socket: tokio::net::TcpStream::connect(addr).await.unwrap(), pending: Vec::new() };
        client.until("Who are you? ").await;
        client.send("#1").await;
        client.until("Password: ").await;
        client.send("secret").await;
        client.until("Sighter").await;
        tokio::time::sleep(Duration::from_millis(1200)).await;
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(&loaded.btech.vehicles()[&unit], &before);
        assert!(serde_json::to_value(&loaded.btech).unwrap()["sensor_recoveries"].as_array().unwrap().is_empty());
        client.send("look").await;
        assert!(!client.until("Sighter").await.contains("all your displays die"));
        sqlx::query("DROP TRIGGER deny_computer").execute(&mut sql).await.unwrap();
        client.until("all your displays die!").await;
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(ranges(&loaded, unit), BattleSensorRanges { tactical: 0, long_range: 0, scan: 0 });
        assert_eq!(serde_json::to_value(&loaded.btech).unwrap()["sensor_recoveries"].as_array().unwrap().len(), 3);
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

/// An outage gates only its display; cached contacts and target selection survive read-only requests.
#[tokio::test]
async fn display_queries_follow_live_outage_and_recovery_boundaries() {
    for index in [0, 2, 6] {
        let template = &firing::templates()[index];
        for effect in [
            BattleComputerFailure::Tactical,
            BattleComputerFailure::LongRange,
            BattleComputerFailure::Scanner,
            BattleComputerFailure::AllDisplays,
        ] {
            let (_dir, config, mut world, unit, target, _) =
                firing::fixture_with_target(template, None, template).await;
            prepare(&mut world, unit, effect);
            let before = serde_json::to_value(&world.btech).unwrap();
            let store = if index == 0 {
                "constructed"
            } else {
                "vehicles"
            };
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            assert!(scan_battle_unit(&scripts.world(), unit, ObjectId(1), target, "").is_ok());
            advance_battle_computer_failures_action(&scripts, &config).unwrap();
            scripts.drain_outbox();
            let snapshot = scripts.world().btech.clone();
            for (kind, failed) in [
                (
                    BattleViewKind::Tactical,
                    matches!(
                        effect,
                        BattleComputerFailure::Tactical | BattleComputerFailure::AllDisplays
                    ),
                ),
                (
                    BattleViewKind::LongRange,
                    matches!(
                        effect,
                        BattleComputerFailure::LongRange | BattleComputerFailure::AllDisplays
                    ),
                ),
            ] {
                let result = resolve_battle_view_center(
                    &scripts.world(),
                    unit,
                    ObjectId(1),
                    kind,
                    BattleViewCenter::OwnUnit,
                );
                assert_eq!(result.is_err(), failed);
                if failed {
                    assert_eq!(
                        result.unwrap_err().to_string(),
                        "Your system seems to be inoperational."
                    );
                }
            }
            let failed = matches!(
                effect,
                BattleComputerFailure::Scanner | BattleComputerFailure::AllDisplays
            );
            let result = scan_battle_unit(&scripts.world(), unit, ObjectId(1), target, "");
            assert_eq!(result.is_err(), failed);
            let lua: Result<String, _> = scripts.eval_callback(&format!(
                "return btech.unit.scan({},1,{})",
                unit.0, target.0
            ));
            assert_eq!(lua.is_err(), failed);
            if failed {
                assert!(
                    lua.unwrap_err()
                        .to_string()
                        .contains("Your system seems to be inoperational.")
                );
                assert!(
                    support::run_text(&scripts, &config, ObjectId(1), 1, "scan AB")
                        .contains("Your system seems to be inoperational.")
                );
            }
            assert_eq!(snapshot, scripts.world().btech);
            let after = serde_json::to_value(&scripts.world().btech).unwrap();
            for field in ["contacts", "target_lock"] {
                assert_eq!(
                    after[store][unit.0.to_string()][field],
                    before[store][unit.0.to_string()][field]
                );
            }
            let mut state = after;
            for event in state["sensor_recoveries"].as_array_mut().unwrap() {
                event["remaining"] = 1.into();
            }
            scripts.world_mut().btech = serde_json::from_value(state).unwrap();
            let config = parts_disabled(_dir.path());
            advance_battle_computer_failures_action(&scripts, &config).unwrap();
            let recovered = scripts.world().btech.clone();
            assert!(scan_battle_unit(&scripts.world(), unit, ObjectId(1), target, "").is_ok());
            for kind in [BattleViewKind::Tactical, BattleViewKind::LongRange] {
                assert!(
                    resolve_battle_view_center(
                        &scripts.world(),
                        unit,
                        ObjectId(1),
                        kind,
                        BattleViewCenter::OwnUnit
                    )
                    .is_ok()
                );
            }
            assert_eq!(recovered, scripts.world().btech);
        }
    }
}

/// Destroyed chassis still restore hardware without announcing recovery; object purge removes timers.
#[tokio::test]
async fn destroyed_recovery_and_object_deletion_preserve_lifecycle() {
    for index in [0, 2, 6] {
        let template = &firing::templates()[index];
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(template, None, template).await;
        prepare(&mut world, unit, BattleComputerFailure::AllDisplays);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        advance_battle_computer_failures_action(&scripts, &config).unwrap();
        let original = scripts.world().clone();
        stop_battle_unit_action(&scripts, &config, unit, ObjectId(1)).unwrap();
        scripts.drain_outbox();
        firing::edit(&mut scripts.world_mut(), unit, |state| {
            state["power"] = serde_json::to_value(BattlePower::Off).unwrap();
            state["pilot_killed"] = true.into();
        });
        let mut state = serde_json::to_value(&scripts.world().btech).unwrap();
        for event in state["sensor_recoveries"].as_array_mut().unwrap() {
            event["remaining"] = 1.into();
        }
        scripts.world_mut().btech = serde_json::from_value(state).unwrap();
        let config = parts_disabled(_dir.path());
        advance_battle_computer_failures_action(&scripts, &config).unwrap();
        assert_eq!(ranges(&scripts.world(), unit).scan, 127);
        assert!(
            scripts
                .drain_outbox()
                .iter()
                .all(|(_, text)| !text.source().contains("operational again"))
        );
        let mut deleted = original.clone();
        deleted
            .objects
            .get_mut(&unit)
            .unwrap()
            .flags
            .insert(Flag::Going);
        let links = dbck::rebuild_links(&deleted, &deleted.links);
        let (cleaned, report) = dbck::plan(&deleted, &links, &config).unwrap();
        assert!(report.plan.purges.contains(&unit));
        assert!(
            serde_json::to_value(&cleaned.btech).unwrap()["sensor_recoveries"]
                .as_array()
                .unwrap()
                .is_empty()
        );
        assert_eq!(
            serde_json::to_value(&original.btech).unwrap()["sensor_recoveries"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        persistence::save(&config.database(), &deleted)
            .await
            .unwrap();
        persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
            dbck::plan(&deleted, raw, &config)
        })
        .await
        .unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert!(!loaded.btech.units().contains_key(&unit));
        assert!(
            serde_json::to_value(&loaded.btech).unwrap()["sensor_recoveries"]
                .as_array()
                .unwrap()
                .is_empty()
        );
    }
}

/// A forced shutdown keeps the same movement damage, falling state and crew effects as cockpit shutdown.
#[tokio::test]
async fn forced_shutdown_preserves_moving_and_airborne_consequences() {
    for index in [0, 2, 6] {
        let template = &firing::templates()[index];
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(template, None, template).await;
        prepare(&mut world, unit, BattleComputerFailure::Shutdown);
        firing::edit(&mut world, unit, |state| {
            state["motion"]["speed"] = 30.0.into();
            state["motion"]["desired_speed"] = 30.0.into();
            if index == 6 {
                state["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
                    phase: BattleVtolFlightPhase::Airborne,
                    altitude: 5.0,
                    vertical_speed: 0.0,
                    fall: None,
                })
                .unwrap();
            }
        });
        let mut direct = world.clone();
        let mut dice = stream(BattleComputerFailure::Shutdown);
        assert_eq!(
            select_computer_failure(
                &mut dice,
                BattleComputerFailureInput {
                    parts_enabled: true,
                    clan: false,
                    has_target: true,
                    tactical_range: 16,
                    long_range: 32,
                    scanner_range: 16,
                }
            )
            .unwrap(),
            Some(BattleComputerFailure::Shutdown)
        );
        firing::edit(&mut direct, unit, |state| {
            state["dice"] = serde_json::to_value(dice).unwrap()
        });
        let direct = Scripts::new(&config, Rc::new(RefCell::new(direct))).unwrap();
        let fault = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        stop_battle_unit_action(&direct, &config, unit, ObjectId(1)).unwrap();
        advance_battle_computer_failures_action(&fault, &config).unwrap();
        if index == 0 {
            assert_eq!(
                direct.world().btech.constructed_units()[&unit],
                fault.world().btech.constructed_units()[&unit]
            );
        } else {
            assert_eq!(
                direct.world().btech.vehicles()[&unit],
                fault.world().btech.vehicles()[&unit]
            );
        }
        assert_eq!(
            serde_json::to_value(&direct.world().objects).unwrap(),
            serde_json::to_value(&fault.world().objects).unwrap()
        );
        let expected = direct.drain_outbox();
        let actual: Vec<_> = fault
            .drain_outbox()
            .into_iter()
            .filter(|(_, text)| !text.source().contains("*SNAP* *CRACKLE*"))
            .collect();
        assert_eq!(actual, expected);
        if index == 6 {
            assert_eq!(
                fault.world().btech.vehicles()[&unit]
                    .vtol_flight()
                    .unwrap()
                    .phase,
                BattleVtolFlightPhase::Falling
            );
            for _ in 0..30 {
                advance_battle_motion_action(&direct, &config, BattleMovementRules::STANDARD)
                    .unwrap();
                advance_battle_motion_action(&fault, &config, BattleMovementRules::STANDARD)
                    .unwrap();
                assert_eq!(
                    direct.world().btech.vehicles()[&unit],
                    fault.world().btech.vehicles()[&unit]
                );
                assert_eq!(
                    serde_json::to_value(&direct.world().objects).unwrap(),
                    serde_json::to_value(&fault.world().objects).unwrap()
                );
                assert_eq!(direct.drain_outbox(), fault.drain_outbox());
                if fault.world().btech.vehicles()[&unit]
                    .vtol_flight()
                    .unwrap()
                    .phase
                    == BattleVtolFlightPhase::Landed
                {
                    break;
                }
            }
            assert_eq!(
                fault.world().btech.vehicles()[&unit]
                    .vtol_flight()
                    .unwrap()
                    .phase,
                BattleVtolFlightPhase::Landed
            );
        }
    }
}
