//! Heartbeat control cadence, gravity stress and ordinary fall consequences replay from saved state.
use crate::support;
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Actual heartbeat rolls are private to the captured pilot, with the reference empty-pilot fallback.
#[tokio::test]
async fn control_feedback_is_ordered_and_respects_pilot_audience() {
    use std::{cell::RefCell, rc::Rc};
    for template in firing::templates().into_iter().take(2) {
        let (_dir, config, mut base, unit, _, _) =
            firing::fixture_with_target(&template, None, &template).await;
        damage_gyro(&mut base, unit);
        base.objects.get_mut(&ObjectId(2)).unwrap().location = Some(unit);
        for player in [ObjectId(1), ObjectId(2)] {
            base.objects
                .get_mut(&player)
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        for assigned in [false, true] {
            for success in [false, true] {
                let mut world = base.clone();
                if !assigned {
                    release_battle_pilot(&mut world, unit, ObjectId(1)).unwrap();
                }
                phase(&mut world, 7);
                let speed = world.btech.constructed_units()[&unit]
                    .mobility()
                    .maximum_speed;
                firing::edit(&mut world, unit, |state| {
                    state["motion"]["speed"] = speed.into();
                    state["motion"]["desired_speed"] = speed.into();
                });
                seed(&mut world, unit, success);
                let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                let reports = advance_battle_periodic_piloting_action(&scripts, &config).unwrap();
                assert_eq!(reports.len(), 1);
                assert_eq!(reports[0].pilot, assigned.then_some(ObjectId(1)));
                let mut expected = reports[0].check.messages().unwrap().to_vec();
                if let Some(messages) = reports[0]
                    .fall
                    .as_ref()
                    .and_then(|fall| fall.avoidance.as_ref())
                    .and_then(|check| check.messages())
                {
                    expected.extend(messages);
                }
                let messages = scripts.drain_outbox();
                let check = reports[0].check;
                let diagnostic = format!(
                    "Attempting to make pilot (noxp) skill roll. SPilot: {}, mods: {}, MechPilot: {}, BTH: {}",
                    check.skill, check.situational, check.damage, check.target
                );
                assert_eq!(
                    support::drain_traces(&scripts, logging::TraceTopic::PilotingRolls),
                    [diagnostic.clone()]
                );
                assert!(
                    !messages
                        .iter()
                        .any(|(_, text)| text.source().contains(&diagnostic))
                );

                for player in [ObjectId(1), ObjectId(2)] {
                    let feedback: Vec<_> = messages
                        .iter()
                        .filter(|(who, text)| {
                            *who == player
                                && (text.source().starts_with("You make a piloting")
                                    || text.source().starts_with("Modified Pilot Skill:"))
                        })
                        .map(|(_, text)| text.source().to_owned())
                        .collect();
                    if player == ObjectId(1) || !assigned {
                        assert_eq!(feedback, expected);
                    } else {
                        assert!(feedback.is_empty());
                    }
                }
                let pilot_messages: Vec<_> = messages
                    .iter()
                    .filter(|(who, _)| *who == ObjectId(1))
                    .map(|(_, text)| text.source())
                    .collect();
                assert_eq!(pilot_messages[0], expected[0]);
                assert_eq!(pilot_messages[1], expected[1]);
            }
        }
    }
}

/// Edit only the committed global phase, exercising the same persisted validation as database load.
fn phase(world: &mut World, value: u8) {
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["turn_clock"] = value.into();
    world.btech = serde_json::from_value(saved).unwrap();
}

/// Install one real damaged gyro without applying an unrelated immediate critical-balance action.
fn damage_gyro(world: &mut World, unit: ObjectId) {
    let part = world.btech.constructed_units()[&unit]
        .loadout()
        .unwrap()
        .systems
        .iter()
        .find(|p| p.system == System::Gyro)
        .unwrap()
        .location;
    destroy_battle_critical(world, unit, part).unwrap();
}

/// Force a first control result while leaving fall or impact draws on the same private stream.
fn seed(world: &mut World, unit: ObjectId, success: bool) {
    let seed = (0..=255)
        .find(|&seed| Dice::seeded([seed; 32]).two_d6() == if success { 12 } else { 2 })
        .unwrap();
    firing::edit(world, unit, |s| {
        s["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
    });
}

/// Damaged-running checks happen off the turn boundary and failed checks use the ordinary fall engine.
#[tokio::test]
async fn damaged_running_checks_use_each_heartbeat_and_preserve_reverse_and_walk_gates() {
    for template in firing::templates().into_iter().take(2) {
        let (_dir, config, mut base, unit, _, _) =
            firing::fixture_with_target(&template, None, &template).await;
        damage_gyro(&mut base, unit);
        let maximum = base.btech.constructed_units()[&unit]
            .mobility()
            .maximum_speed;
        let walk = maximum * 2.0 / 3.0;
        for speed in [-walk, 0.0, walk, walk + 0.2] {
            for success in [false, true] {
                let mut world = base.clone();
                phase(&mut world, 7);
                firing::edit(&mut world, unit, |s| {
                    s["motion"]["speed"] = speed.into();
                    s["motion"]["desired_speed"] = speed.into();
                });
                seed(&mut world, unit, success);
                persistence::save(&config.database(), &world).await.unwrap();
                let mut replay = persistence::load(&config.database()).await.unwrap();
                let before = world.btech.clone();
                let reports = advance_battle_periodic_piloting(&mut world, &config).unwrap();
                assert_eq!(
                    reports,
                    advance_battle_periodic_piloting(&mut replay, &config).unwrap()
                );
                assert_eq!(world.btech, replay.btech);
                if speed <= walk + 0.1 {
                    assert!(reports.is_empty());
                    assert_eq!(world.btech, before);
                    continue;
                }
                assert_eq!(reports.len(), 1);
                assert_eq!(reports[0].check.situational, 0);
                assert_eq!(reports[0].check.success, success);
                assert_eq!(reports[0].fall.is_some(), !success);
                if !success {
                    assert_eq!(
                        reports[0].notices[0].text,
                        "Your damaged mech falls as you try to run!"
                    );
                    assert_eq!(
                        world.btech.constructed_units()[&unit].posture(),
                        Posture::Prone
                    );
                    assert_eq!(
                        world.btech.constructed_units()[&unit]
                            .motion()
                            .unwrap()
                            .speed,
                        0.0
                    );
                }
            }
        }
    }
}

/// Installed boosters permit an overspeed state; the gravity rule still compares against unloaded speed.
#[tokio::test]
async fn gravity_stress_obeys_global_boundary_and_hits_each_chassis_leg_in_order() {
    for template in [
        include_str!("../game/units/CTF-3L.toml"),
        include_str!("../game/units/StalkingSpider-1.toml"),
    ] {
        let (_dir, config, mut base, unit, _, _) =
            firing::fixture_with_target(template, None, template).await;
        let map = base.btech.units()[&unit].map.unwrap();
        let maximum = base.btech.constructed_units()[&unit]
            .mobility()
            .maximum_speed;
        let legs = base.btech.constructed_units()[&unit].chassis().legs().len();
        let seed = (0..=255)
            .find(|&seed| {
                let mut dice = Dice::seeded([seed; 32]);
                dice.two_d6() <= 4 && (0..legs).all(|_| dice.two_d6() < 8)
            })
            .unwrap();
        firing::edit(&mut base, unit, |s| {
            s["motion"]["speed"] = (maximum + 1.0).into();
            s["motion"]["desired_speed"] = (maximum + 1.0).into();
            s["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        });
        for gravity in [50, 100, 150] {
            for tick in [0, 1, 28, 29] {
                let mut world = base.clone();
                world
                    .btech
                    .rewrite_map_record(map, |record| {
                        record["flags"] = 2.into();
                        record["gravity"] = gravity.into();
                    })
                    .unwrap();
                phase(&mut world, tick);
                let before = world.btech.clone();
                let reports = advance_battle_periodic_piloting(&mut world, &config).unwrap();
                if gravity == 100 || ![0, 29].contains(&tick) {
                    assert!(reports.is_empty());
                    assert_eq!(world.btech, before);
                    continue;
                }
                assert_eq!(reports.len(), 1);
                let report = &reports[0];
                assert_eq!(report.gravity_damage, 1);
                assert!(!report.check.success);
                assert!(report.fall.is_none());
                assert_eq!(report.impacts.len(), legs);
                assert_eq!(report.notices[0].text, "Your legs take some damage!");
                let order: Vec<_> = report
                    .impacts
                    .iter()
                    .map(|i| i.impact.phases[0].section)
                    .collect();
                assert_eq!(
                    order,
                    if legs == 4 {
                        vec![
                            MechSection::LeftArm,
                            MechSection::RightArm,
                            MechSection::LeftLeg,
                            MechSection::RightLeg,
                        ]
                    } else {
                        vec![MechSection::LeftLeg, MechSection::RightLeg]
                    }
                );
                for section in order {
                    let old = &before.constructed_units()[&unit].sections()[&section];
                    let new = &world.btech.constructed_units()[&unit].sections()[&section];
                    assert_eq!(new.armor, old.armor);
                    assert_eq!(new.internal, old.internal - 1);
                }
            }
        }
    }
}

/// The hot-myomer running threshold changes only on the guarded turn ticks.
#[tokio::test]
async fn hot_myomer_turn_threshold_and_shutdown_crew_gates() {
    let template = include_str!("../game/units/OTL-6D.toml");
    let (_dir, config, mut base, unit, _, _) =
        firing::fixture_with_target(template, None, template).await;
    damage_gyro(&mut base, unit);
    let speed = base.btech.constructed_units()[&unit]
        .mobility()
        .maximum_speed
        * 2.0
        / 3.0
        + 1.0;
    firing::edit(&mut base, unit, |s| {
        s["motion"]["speed"] = speed.into();
        s["motion"]["desired_speed"] = speed.into();
        s["heat"]["excess"] = 9.0.into();
        s["heat"]["stored"] = 9.0.into();
    });
    for tick in [1, 28, 29, 0] {
        let mut world = base.clone();
        phase(&mut world, tick);
        seed(&mut world, unit, true);
        let reports = advance_battle_periodic_piloting(&mut world, &config).unwrap();
        assert_eq!(reports.is_empty(), [29, 0].contains(&tick));
    }
    for template in firing::templates() {
        let (_dir, config, mut base, unit, _, _) =
            firing::fixture_with_target(&template, None, &template).await;
        stop_battle_unit(&mut base, unit, ObjectId(1), MovementRules::STANDARD.fall).unwrap();
        let recovery_seed = (0..=255)
            .find(|&seed| Dice::seeded([seed; 32]).two_d6() < 7)
            .unwrap();
        for unconscious in [false, true] {
            let mut ready = base.clone();
            if unconscious {
                firing::edit(&mut ready, unit, |s| {
                    s["crew_recovery"]["dice"] =
                        serde_json::to_value(Dice::seeded([recovery_seed; 32])).unwrap()
                });
                assert!(
                    !injure_battle_tactical_pilot(&mut ready, unit, 3, false)
                        .unwrap()
                        .consciousness
                        .unwrap()
                        .conscious
                );
            }
            for tick in [28, 29, 0, 1] {
                let mut world = ready.clone();
                phase(&mut world, tick);
                let reports = advance_battle_periodic_piloting(&mut world, &config).unwrap();
                let expected = unconscious && [29, 0].contains(&tick);
                assert_eq!(reports.len(), usize::from(expected));
                if expected {
                    assert_eq!(reports[0].check.roll, None);
                    assert_eq!(reports[0].check.situational, 3);
                    assert!(!reports[0].check.success);
                    assert!(reports[0].fall.is_some() || reports[0].vehicle_fall.is_some());
                }
            }
        }
    }
}

/// A failed committed fall retains the turn phase and all material/dice state until retry.
#[tokio::test(flavor = "current_thread")]
async fn server_clock_and_fall_retry_are_one_transaction() {
    use sqlx::{Connection, SqliteConnection};
    use std::{cell::Cell, rc::Rc};
    tokio::task::LocalSet::new().run_until(async {
        let template = include_str!("../game/units/JR7-D.toml");
        let (_dir,config,mut world,unit,_,_) = firing::fixture_with_target(template,None,template).await;
        damage_gyro(&mut world,unit); phase(&mut world,28); seed(&mut world,unit,false);
        firing::edit(&mut world,unit,|s| {s["motion"]["speed"] = 100.0.into();s["motion"]["desired_speed"] = 100.0.into();});
        world.accounts.get_mut(&ObjectId(1)).unwrap().hash = Some(accounts::hash("secret",&config).unwrap());
        persistence::save(&config.database(),&world).await.unwrap();
        let before = world.btech.constructed_units()[&unit].clone();
        let mut sql = SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()).foreign_keys(false)).await.unwrap();
        sqlx::query("CREATE TRIGGER deny_phase BEFORE UPDATE ON btech_simulation_clock BEGIN SELECT RAISE(ABORT,'phase failure'); END").execute(&mut sql).await.unwrap();
        let (addr,shutdown,task,_,mut heartbeats) = support::start(&config,Rc::new(Cell::new(1))).await;
        let mut client = support::Client {socket:tokio::net::TcpStream::connect(addr).await.unwrap(),pending:Vec::new()};
        client.until("Who are you? ").await; client.send("#1").await; client.until("Password: ").await; client.send("secret").await; client.until("Sighter").await;
        heartbeats.attempt().await;
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(serde_json::to_value(&loaded.btech).unwrap()["turn_clock"],28);
        assert_eq!(&loaded.btech.constructed_units()[&unit],&before);
        client.send("look").await;
        let rejected = client.until("Sighter").await;
        assert!(!rejected.contains("Your damaged mech falls"));
        assert!(!rejected.contains("You make a piloting skill roll!"));
        assert!(!rejected.contains("Modified Pilot Skill:"));
        sqlx::query("DROP TRIGGER deny_phase").execute(&mut sql).await.unwrap();
        let accepted = client.until_heartbeats("Your damaged mech falls as you try to run!", &mut heartbeats, 3).await;
        assert!(accepted.contains("You make a piloting skill roll!"));
        assert!(accepted.contains("Modified Pilot Skill:"));
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech.constructed_units()[&unit].posture(),Posture::Prone);
        assert_eq!(serde_json::to_value(&loaded.btech).unwrap()["turn_clock"],29);
        shutdown.send(ShutdownRequest::Sigterm).unwrap(); task.await.unwrap().unwrap();
    }).await;
}

/// A damaged hip triggers the same heartbeat check without requiring gyro damage.
#[tokio::test]
async fn damaged_hips_use_running_threshold_for_both_mech_chassis() {
    for template in firing::templates().into_iter().take(2) {
        let (_dir, config, mut base, unit, _, _) =
            firing::fixture_with_target(&template, None, &template).await;
        let mech = &base.btech.constructed_units()[&unit];
        let hip = mech
            .loadout()
            .unwrap()
            .systems
            .iter()
            .find(|p| {
                p.system == System::ShoulderOrHip
                    && mech.chassis().legs().contains(&p.location.section)
            })
            .unwrap()
            .location;
        destroy_battle_critical(&mut base, unit, hip).unwrap();
        assert_eq!(base.btech.constructed_units()[&unit].gyro_damage(), 0);
        let maximum = base.btech.constructed_units()[&unit]
            .mobility()
            .maximum_speed;
        let threshold = 2.0 * maximum / 3.0 + 0.1;
        for speed in [-maximum / 2.0, threshold, threshold + 0.01] {
            let mut world = base.clone();
            phase(&mut world, 7);
            firing::edit(&mut world, unit, |s| {
                s["motion"]["speed"] = speed.into();
                s["motion"]["desired_speed"] = speed.into();
            });
            seed(&mut world, unit, true);
            let before = world.btech.clone();
            let reports = advance_battle_periodic_piloting(&mut world, &config).unwrap();
            assert_eq!(reports.len(), usize::from(speed > threshold));
            if reports.is_empty() {
                assert_eq!(world.btech, before);
            } else {
                assert!(reports[0].check.success);
                assert!(reports[0].fall.is_none());
                assert_eq!(reports[0].check.situational, 0);
            }
        }
    }
}

/// Special rules gate gravity stress, and a successful control check does not apply leg damage.
#[tokio::test]
async fn gravity_success_and_disabled_special_rules_preserve_material() {
    for template in [
        include_str!("../game/units/CTF-3L.toml"),
        include_str!("../game/units/StalkingSpider-1.toml"),
    ] {
        let (_dir, config, base, unit, _, _) =
            firing::fixture_with_target(template, None, template).await;
        let map = base.btech.units()[&unit].map.unwrap();
        let maximum = base.btech.constructed_units()[&unit]
            .mobility()
            .maximum_speed;
        for special in [false, true] {
            let mut world = base.clone();
            world
                .btech
                .rewrite_map_record(map, |record| {
                    record["flags"] = if special { 2 } else { 0 }.into();
                    record["gravity"] = 50.into();
                })
                .unwrap();
            phase(&mut world, 29);
            firing::edit(&mut world, unit, |s| {
                s["motion"]["speed"] = (maximum + 1.0).into();
                s["motion"]["desired_speed"] = (maximum + 1.0).into();
            });
            seed(&mut world, unit, true);
            let before = world.btech.clone();
            let reports = advance_battle_periodic_piloting(&mut world, &config).unwrap();
            assert_eq!(reports.len(), usize::from(special));
            assert_eq!(
                world.btech.constructed_units()[&unit].sections(),
                before.constructed_units()[&unit].sections()
            );
            if special {
                assert!(reports[0].check.success);
                assert!(reports[0].impacts.is_empty());
                assert!(reports[0].notices.is_empty());
            } else {
                assert_eq!(world.btech, before);
            }
        }
    }
}

/// Idle ticks advance the clock without writing, a clean shutdown stores it, the phase
/// wraps at thirty, and a restart resumes without offline catch-up.
#[tokio::test(flavor = "current_thread")]
async fn idle_clock_wraps_is_stored_at_shutdown_and_resumes_from_saved_phase() {
    use sqlx::{Connection, SqliteConnection};
    use std::{cell::Cell, rc::Rc};
    tokio::task::LocalSet::new()
        .run_until(async {
            let (dir, _, mut world) = support::isolated_world().await;
            // The shipped default; the test fixture saves the clock every second.
            let config = support::with_clock_save_interval(dir.path(), 60);
            assert!(world.btech.constructed_units().is_empty());
            assert!(world.btech.vehicles().is_empty());
            // Expire the separate startup grace so the host takes its otherwise-idle branch.
            for _ in 0..31 {
                advance_battle_reactor_windows(&mut world);
            }
            assert!(!reactor_windows_pending(&world));
            phase(&mut world, 29);
            persistence::save(&config.database(), &world).await.unwrap();
            let mut sql = SqliteConnection::connect_with(
                &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
            )
            .await
            .unwrap();
            let saved: i64 = sqlx::query_scalar("PRAGMA data_version")
                .fetch_one(&mut sql)
                .await
                .unwrap();
            let (_, shutdown, task, _, mut heartbeats) =
                support::start(&config, Rc::new(Cell::new(1))).await;
            // Each heartbeat advances the stored idle clock one second.
            heartbeats.commit_n(2).await;
            let current: i64 = sqlx::query_scalar("PRAGMA data_version")
                .fetch_one(&mut sql)
                .await
                .unwrap();
            assert_eq!(current, saved, "idle ticks wrote to the database");
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
            let stored = persistence::load(&config.database()).await.unwrap();
            let seconds = stored.btech.simulation_time();
            assert!(seconds >= 2, "shutdown did not store the idle clock");
            let phase_at = |saved: &World| {
                serde_json::to_value(&saved.btech).unwrap()["turn_clock"]
                    .as_i64()
                    .unwrap()
            };
            assert_eq!(phase_at(&stored), (29 + seconds) % 30);
            // A large wall-clock change must not simulate thousands of offline turns.
            let (_, shutdown, task, _, mut heartbeats) =
                support::start(&config, Rc::new(Cell::new(900_000))).await;
            heartbeats.commit().await;
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
            let resumed = persistence::load(&config.database()).await.unwrap();
            let elapsed = resumed.btech.simulation_time() - seconds;
            assert!(
                (1..=3).contains(&elapsed),
                "restart simulated {elapsed} seconds"
            );
            assert_eq!(phase_at(&resumed), (29 + seconds + elapsed) % 30);
            sql.close().await.unwrap();
        })
        .await;
}
