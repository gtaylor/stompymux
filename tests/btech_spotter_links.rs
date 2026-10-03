//! Durable radio observer links use the same lifecycle for every supported chassis.
use crate::support;
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// An empty Arrow IV still qualifies for a datalink, independently of firing admission.
async fn fixture(
    source: &str,
    observer: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    let (dir, config, mut world, source, observer, index) =
        firing::fixture_with_target(source, Some(BattleWeapon::ClanArrowIv), observer).await;
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(observer);
    assign_battle_pilot(&mut world, observer, ObjectId(2)).unwrap();
    select_battle_spotter(&mut world, observer, ObjectId(2), Some(observer)).unwrap();
    firing::edit(&mut world, source, |state| {
        state["contacts"] = serde_json::json!({})
    });
    assert!(
        visible_battle_contact(&world, source, observer)
            .unwrap()
            .is_none()
    );
    (dir, config, world, source, observer, index)
}

/// Read both stores through one assertion adapter, without interpreting event internals.
fn selected(world: &World, id: ObjectId) -> Option<ObjectId> {
    world.btech.vehicles().get(&id).map_or_else(
        || world.btech.constructed_units()[&id].spotter(),
        |unit| unit.spotter(),
    )
}

/// Read the durable queue as exposed by ordinary owned-unit inspection.
fn pending(world: &World, id: ObjectId) -> bool {
    world.btech.vehicles().get(&id).map_or_else(
        || {
            world.btech.constructed_units()[&id]
                .spotter_events()
                .pending()
        },
        |unit| unit.spotter_events().pending(),
    )
}

/// Exact ten-second completion survives restart and ten-second maintenance removes stale observers.
#[tokio::test]
async fn all_chassis_connect_restart_and_lose_stopped_observers() {
    let templates = firing::templates();
    for (i, template) in templates.iter().enumerate() {
        let (_dir, config, mut world, source, observer, _) =
            fixture(template, &templates[(i + 1) % templates.len()]).await;
        let notices =
            select_battle_spotter(&mut world, source, ObjectId(1), Some(observer)).unwrap();
        assert_eq!(notices.len(), 2);
        assert_eq!(notices[0].unit, observer);
        assert_eq!(
            notices[0].text,
            "Someone is trying to establish a data link with you!"
        );
        assert_eq!(notices[1].unit, source);
        assert_eq!(
            notices[1].text,
            "You attempt to establish a data link..... please stand by."
        );
        assert_eq!(selected(&world, source), None);
        assert!(pending(&world, source));
        for _ in 0..4 {
            assert!(advance_battle_spotter_links(&mut world).unwrap().is_empty());
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for _ in 4..9 {
            assert!(advance_battle_spotter_links(&mut world).unwrap().is_empty());
            assert!(
                advance_battle_spotter_links(&mut restored)
                    .unwrap()
                    .is_empty()
            );
        }
        let completion = advance_battle_spotter_links(&mut world).unwrap();
        assert_eq!(
            completion,
            advance_battle_spotter_links(&mut restored).unwrap()
        );
        assert_eq!(completion.len(), 2);
        assert!(
            completion[0]
                .text
                .starts_with("Data link established with ")
        );
        assert!(
            completion[1]
                .text
                .ends_with(", you now have a forward observer.")
        );
        assert_eq!(selected(&world, source), Some(observer));
        assert_eq!(
            serde_json::to_value(&world.btech).unwrap(),
            serde_json::to_value(&restored.btech).unwrap()
        );
        select_battle_spotter(&mut world, observer, ObjectId(2), None).unwrap();
        for _ in 0..9 {
            assert!(advance_battle_spotter_links(&mut world).unwrap().is_empty());
        }
        let lost = advance_battle_spotter_links(&mut world).unwrap();
        assert_eq!(lost.len(), 1);
        assert_eq!(lost[0].unit, source);
        assert_eq!(lost[0].text, "You have lost link with your spotter!");
        assert_eq!(selected(&world, source), None);
        assert!(!pending(&world, source));
        world.validate(&config).unwrap();
    }
}

/// Clearing selection and shutdown leave all previously queued connection attempts intact.
#[tokio::test]
async fn repeated_requests_survive_clear_and_shutdown() {
    for template in firing::templates() {
        let (_dir, config, mut world, source, observer, _) = fixture(&template, &template).await;
        for _ in 0..2 {
            select_battle_spotter(&mut world, source, ObjectId(1), Some(observer)).unwrap();
        }
        select_battle_spotter(&mut world, source, ObjectId(1), None).unwrap();
        stop_battle_unit(
            &mut world,
            source,
            ObjectId(1),
            BattleFallRules::configured(&config),
        )
        .unwrap();
        assert!(pending(&world, source));
        for _ in 0..9 {
            assert!(advance_battle_spotter_links(&mut world).unwrap().is_empty());
        }
        let notices = advance_battle_spotter_links(&mut world).unwrap();
        assert_eq!(notices.len(), 4);
        assert_eq!(selected(&world, source), Some(observer));
        world.validate(&config).unwrap();
    }
}

/// Recycling prevents radio setup, while the same weapon with no ammunition may request it.
#[tokio::test]
async fn numbered_artillery_recycling_blocks_connection() {
    for template in firing::templates() {
        let (_dir, _, mut world, source, observer, index) = fixture(&template, &template).await;
        firing::edit(&mut world, source, |state| {
            state["weapon_recycle"][index.to_string()] = 2.into();
        });
        let before = serde_json::to_value(&world.btech).unwrap();
        let error =
            select_battle_spotter(&mut world, source, ObjectId(1), Some(observer)).unwrap_err();
        assert_eq!(error.to_string(), "You do not have LOS to that target!");
        assert_eq!(serde_json::to_value(&world.btech).unwrap(), before);
    }
}

/// All four captured coordinates must change; a single moving participant does not cancel setup.
#[tokio::test]
async fn connection_movement_requires_all_four_coordinate_changes() {
    for template in firing::templates() {
        let (_dir, config, world, source, observer, _) = fixture(&template, &template).await;
        for mask in 0_u8..16 {
            let mut candidate = world.clone();
            select_battle_spotter(&mut candidate, source, ObjectId(1), Some(observer)).unwrap();
            for (unit_index, id) in [source, observer].into_iter().enumerate() {
                firing::edit(&mut candidate, id, |state| {
                    for (axis_index, axis) in ["x", "y"].into_iter().enumerate() {
                        if mask & (1 << (unit_index * 2 + axis_index)) != 0 {
                            let coordinate = &mut state["motion"]["point"][axis];
                            *coordinate = (coordinate.as_f64().unwrap() + 0.01).into();
                        }
                    }
                });
            }
            for _ in 0..9 {
                assert!(
                    advance_battle_spotter_links(&mut candidate)
                        .unwrap()
                        .is_empty()
                );
            }
            let notices = advance_battle_spotter_links(&mut candidate).unwrap();
            assert_eq!(notices.len(), 2);
            if mask == 15 {
                assert!(
                    notices.iter().all(|notice| notice.text
                        == "The data link was not established due to movement!")
                );
                assert_eq!(selected(&candidate, source), None);
                assert!(!pending(&candidate, source));
            } else {
                assert_eq!(selected(&candidate, source), Some(observer), "mask {mask}");
                assert!(pending(&candidate, source));
            }
            candidate.validate(&config).unwrap();
        }
    }
}

/// Observer radio sets the inclusive limit and completed links use its current radio on maintenance.
#[tokio::test]
async fn observer_radio_controls_range_attempt_notices_and_maintenance() {
    for template in firing::templates() {
        let (_dir, config, world, source, observer, _) = fixture(&template, &template).await;
        for (distance, allowed) in [(1.999, true), (2.0, true), (2.001, false)] {
            let mut candidate = world.clone();
            firing::edit(&mut candidate, observer, |state| {
                state["hardware"]["radio_range"] = 1.into()
            });
            firing::edit(&mut candidate, source, |state| {
                state["hardware"]["radio_range"] = 0.into();
                state["motion"]["point"]["y"] = (10.5 - distance).into();
                state["position"]["y"] = 8.into();
            });
            let before = serde_json::to_value(&candidate.btech).unwrap();
            let notices =
                select_battle_spotter(&mut candidate, source, ObjectId(1), Some(observer)).unwrap();
            assert_eq!(notices.len(), if allowed { 2 } else { 3 });
            if !allowed {
                assert_eq!(notices[2].text, "That target is our of data link range!");
                assert_eq!(serde_json::to_value(&candidate.btech).unwrap(), before);
                continue;
            }
            for _ in 0..10 {
                advance_battle_spotter_links(&mut candidate).unwrap();
            }
            assert_eq!(selected(&candidate, source), Some(observer));
            firing::edit(&mut candidate, observer, |state| {
                state["hardware"]["radio_range"] = 0.into()
            });
            for _ in 0..9 {
                assert!(
                    advance_battle_spotter_links(&mut candidate)
                        .unwrap()
                        .is_empty()
                );
            }
            let notices = advance_battle_spotter_links(&mut candidate).unwrap();
            assert_eq!(notices.len(), 1);
            assert_eq!(notices[0].text, "You have lost link with your spotter!");
            assert_eq!(selected(&candidate, source), None);
            candidate.validate(&config).unwrap();
        }
    }
}

/// Radio setup uses integer ten-hex bands before converting the delay to two-second ticks.
#[tokio::test]
async fn ten_hex_delay_boundary_is_not_rounded() {
    for template in firing::templates() {
        let (_dir, _, world, source, observer, _) = fixture(&template, &template).await;
        for (distance, seconds) in [(9.999, 10), (10.0, 12), (10.001, 12)] {
            let mut candidate = world.clone();
            firing::edit(&mut candidate, source, |state| {
                state["motion"]["point"]["y"] = (10.5 - distance).into();
                state["position"]["y"] = 0.into();
            });
            select_battle_spotter(&mut candidate, source, ObjectId(1), Some(observer)).unwrap();
            for _ in 1..seconds {
                assert!(
                    advance_battle_spotter_links(&mut candidate)
                        .unwrap()
                        .is_empty()
                );
            }
            assert_eq!(
                advance_battle_spotter_links(&mut candidate).unwrap().len(),
                2
            );
            assert_eq!(selected(&candidate, source), Some(observer));
        }
    }
}

/// Both entry points queue the same requests; aborted Lua callbacks restore state and both notices.
#[tokio::test]
async fn native_lua_radio_requests_share_state_and_atomic_output() {
    use std::{cell::RefCell, rc::Rc};
    for template in firing::templates() {
        let (_dir, config, world, source, observer, _) = fixture(&template, &template).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let call = format!("btech.unit.spot({},1,{})", source.0, observer.0);
        assert!(
            scripts
                .eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(
            serde_json::to_value(&scripts.world().btech).unwrap(),
            serde_json::to_value(&world.btech).unwrap()
        );
        assert!(scripts.drain_outbox().is_empty());
        scripts.eval_callback::<()>(&call).unwrap();
        let native_output = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("spot #{}", observer.0),
        );
        assert_eq!(
            serde_json::to_value(&scripts.world().btech).unwrap(),
            serde_json::to_value(&native.world().btech).unwrap()
        );
        let output = scripts.drain_outbox();
        assert_eq!(
            output
                .iter()
                .map(|(_, message)| message.source())
                .collect::<Vec<_>>()
                .join("\n"),
            native_output
        );
        assert!(!output.is_empty());
        let pending_count: usize = scripts.eval_callback(&format!("local s=btech.unit.state({}); local n=#s.spotter_events.events; s.spotter_events.events[1].remaining=1; return n",source.0)).unwrap();
        assert_eq!(pending_count, 1);
        assert_eq!(
            serde_json::to_value(&scripts.world().btech).unwrap(),
            serde_json::to_value(&native.world().btech).unwrap()
        );
    }
}

/// Destruction removes queued requests immediately, including when no later clock is advanced.
#[tokio::test]
async fn destruction_cancels_connections_for_all_chassis() {
    for template in firing::templates() {
        let (_dir, _, mut world, source, observer, _) = fixture(&template, &template).await;
        select_battle_spotter(&mut world, source, ObjectId(1), Some(observer)).unwrap();
        assert!(pending(&world, source));
        if world.btech.vehicles().contains_key(&source) {
            damage_battle_vehicle_phase(
                &mut world,
                source,
                BattleVehicleSection::Front,
                1000,
                BattleDamagePhase::Internal,
            )
            .unwrap();
        } else {
            apply_damage_phase(
                &mut world,
                source,
                BattleSection::CenterTorso,
                1000,
                BattleDamagePhase::Internal,
            )
            .unwrap();
        }
        assert!(!pending(&world, source));
        for _ in 0..12 {
            assert!(advance_battle_spotter_links(&mut world).unwrap().is_empty());
        }
        assert_eq!(selected(&world, source), None);
    }
}

/// Declaring or clearing one's own link preserves correction; stopping observation resets dependents only.
#[tokio::test]
async fn spotter_correction_resets_follow_selection_scope() {
    for template in firing::templates() {
        let (_dir, _, mut world, source, observer, _) = fixture(&template, &template).await;
        for id in [source, observer] {
            firing::edit(&mut world, id, |state| {
                state["artillery_adjustment"] = 2.into()
            });
        }
        let correction = |world: &World, id| {
            world.btech.vehicles().get(&id).map_or_else(
                || world.btech.constructed_units()[&id].artillery_adjustment(),
                |unit| unit.artillery_adjustment(),
            )
        };
        select_battle_spotter(&mut world, source, ObjectId(1), Some(source)).unwrap();
        assert_eq!(correction(&world, source), 2);
        select_battle_spotter(&mut world, source, ObjectId(1), None).unwrap();
        assert_eq!(correction(&world, source), 2);
        select_battle_spotter(&mut world, source, ObjectId(1), Some(observer)).unwrap();
        for _ in 0..10 {
            advance_battle_spotter_links(&mut world).unwrap();
        }
        assert_eq!(correction(&world, source), 2);
        select_battle_spotter(&mut world, observer, ObjectId(2), None).unwrap();
        assert_eq!(correction(&world, source), 0);
        assert_eq!(correction(&world, observer), 2);
        select_battle_spotter(&mut world, observer, ObjectId(2), Some(observer)).unwrap();
        firing::edit(&mut world, source, |state| {
            state["artillery_adjustment"] = 2.into();
            state["contacts"][observer.0.to_string()] = serde_json::json!({"identified":true});
        });
        assert!(
            visible_battle_contact(&world, source, observer)
                .unwrap()
                .is_some()
        );
        let notices =
            select_battle_spotter(&mut world, source, ObjectId(1), Some(observer)).unwrap();
        assert_eq!(notices.len(), 1);
        assert_eq!(correction(&world, source), 0);
        assert_eq!(correction(&world, observer), 2);
    }
}

/// Simultaneous requests finish in insertion order, even when that differs from observer dbref order.
#[tokio::test]
async fn simultaneous_requests_keep_insertion_order() {
    for template in firing::templates() {
        let (_dir, config, mut world, source, observer, _) = fixture(&template, &template).await;
        let second = world.create(&config, "Second observer".into(), Kind::Thing);
        world.objects.get_mut(&second).unwrap().home = Some(ObjectId(config.home()));
        BattleUnitTemplate::parse("test", &template)
            .unwrap()
            .create(&mut world, second)
            .unwrap();
        let map = world.btech.units()[&source].map.unwrap();
        place_battle_unit(&mut world, second, map, 0, 10).unwrap();
        firing::edit(&mut world, second, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            state["spotter"] = second.0.into();
        });
        select_battle_spotter(&mut world, source, ObjectId(1), Some(second)).unwrap();
        select_battle_spotter(&mut world, source, ObjectId(1), Some(observer)).unwrap();
        for _ in 0..9 {
            assert!(advance_battle_spotter_links(&mut world).unwrap().is_empty());
        }
        let notices = advance_battle_spotter_links(&mut world).unwrap();
        assert_eq!(
            notices.iter().map(|notice| notice.unit).collect::<Vec<_>>(),
            vec![second, source, observer, source]
        );
        assert_eq!(selected(&world, source), Some(observer));
        world.validate(&config).unwrap();
    }
}

/// A cold battlefield retries the same completion after a failed save without publishing early output.
#[tokio::test(flavor = "current_thread")]
async fn server_retries_connection_after_failed_commit() {
    use sqlx::{Connection, SqliteConnection};
    use std::{cell::Cell, rc::Rc};
    tokio::task::LocalSet::new().run_until(async {
        for template in [include_str!("../game/mechs/JR7-D.toml"), include_str!("../game/mechs/Demolisher.toml")] {
            let (_dir, config, mut world, source, observer, _) = fixture(template, template).await;
            select_battle_spotter(&mut world, source, ObjectId(1), Some(observer)).unwrap();
            stop_battle_unit(&mut world, source, ObjectId(1), BattleFallRules::configured(&config)).unwrap();
            stop_battle_unit(&mut world, observer, ObjectId(2), BattleFallRules::configured(&config)).unwrap();
            assign_battle_pilot(&mut world, source, ObjectId(1)).unwrap();
            firing::edit(&mut world, source, |state| state["spotter_events"]["events"][0]["remaining"] = 1.into());
            world.accounts.get_mut(&ObjectId(1)).unwrap().hash = Some(accounts::hash("secret", &config).unwrap());
            persistence::save(&config.database(), &world).await.unwrap();
            let mut sql = SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()).foreign_keys(false)).await.unwrap();
            let trigger = if world.btech.vehicles().contains_key(&source) {
                "CREATE TRIGGER deny_spotter BEFORE UPDATE ON btech_vehicles BEGIN SELECT RAISE(ABORT,'spotter failure'); END"
            } else {
                "CREATE TRIGGER deny_spotter BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'spotter failure'); END"
            };
            sqlx::query(trigger).execute(&mut sql).await.unwrap();
            let (addr, shutdown, task, _) = support::start(&config, Rc::new(Cell::new(1))).await;
            let mut client = support::Client { socket:tokio::net::TcpStream::connect(addr).await.unwrap(), pending:Vec::new() };
            client.until("Who are you? ").await;
            client.send("#1").await;
            client.until("Password: ").await;
            client.send("secret").await;
            client.until("Sighter").await;
            support::attempt_heartbeat().await;
            let saved = persistence::load(&config.database()).await.unwrap();
            assert_eq!(selected(&saved, source), None);
            assert!(pending(&saved, source));
            client.send("look").await;
            let output = client.until("Sighter").await;
            assert!(!output.contains("Data link established with"));
            sqlx::query("DROP TRIGGER deny_spotter").execute(&mut sql).await.unwrap();
            client.until(", you now have a forward observer.").await;
            let saved = persistence::load(&config.database()).await.unwrap();
            assert_eq!(selected(&saved, source), Some(observer));
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        }
    }).await;
}

/// Clearing a selected link ends its check silently even if the retained observer is being deleted.
#[tokio::test]
async fn cleared_maintenance_does_not_report_deleted_observers() {
    for template in firing::templates() {
        let (_dir, _, world, source, observer, _) = fixture(&template, &template).await;
        for cleared in [false, true] {
            let mut candidate = world.clone();
            select_battle_spotter(&mut candidate, source, ObjectId(1), Some(observer)).unwrap();
            for _ in 0..10 {
                advance_battle_spotter_links(&mut candidate).unwrap();
            }
            if cleared {
                select_battle_spotter(&mut candidate, source, ObjectId(1), None).unwrap();
            }
            candidate
                .objects
                .get_mut(&observer)
                .unwrap()
                .flags
                .insert(Flag::Going);
            for _ in 0..9 {
                assert!(
                    advance_battle_spotter_links(&mut candidate)
                        .unwrap()
                        .is_empty()
                );
            }
            let notices = advance_battle_spotter_links(&mut candidate).unwrap();
            if cleared {
                assert!(notices.is_empty());
            } else {
                assert_eq!(notices.len(), 1);
                assert_eq!(notices[0].text, "You have lost link with your spotter!");
            }
            assert_eq!(selected(&candidate, source), None);
            assert!(!pending(&candidate, source));
            assert!(
                advance_battle_spotter_links(&mut candidate)
                    .unwrap()
                    .is_empty()
            );
        }
    }
}

/// The numbered artillery lookup rejects its first destroyed critical without consuming a request.
#[tokio::test]
async fn destroyed_first_artillery_critical_prevents_radio_setup() {
    for template in firing::templates() {
        let (_dir, config, mut world, source, observer, index) =
            fixture(&template, &template).await;
        if let Some(unit) = world.btech.vehicles().get(&source) {
            let location = unit.loadout().unwrap().weapons[index].criticals[0];
            destroy_battle_vehicle_critical(&mut world, source, location).unwrap();
        } else {
            let location = world.btech.constructed_units()[&source]
                .loadout()
                .unwrap()
                .weapons[index]
                .criticals[0];
            destroy_battle_critical(&mut world, source, location).unwrap();
        }
        let before = serde_json::to_value(&world.btech).unwrap();
        assert_eq!(
            select_battle_spotter(&mut world, source, ObjectId(1), Some(observer))
                .unwrap_err()
                .to_string(),
            "You do not have LOS to that target!"
        );
        assert_eq!(serde_json::to_value(&world.btech).unwrap(), before);
        world.validate(&config).unwrap();
    }
}

/// A physical recycle in the artillery's section rejects setup, but another section does not.
#[tokio::test]
async fn artillery_section_recycling_controls_radio_eligibility() {
    for template in firing::templates().into_iter().take(2) {
        let (_dir, _, world, source, observer, _) = fixture(&template, &template).await;
        for (section, allowed) in [
            (BattleSection::LeftTorso, false),
            (BattleSection::RightArm, true),
        ] {
            let mut candidate = world.clone();
            firing::edit(&mut candidate, source, |state| {
                let key = serde_json::to_value(section)
                    .unwrap()
                    .as_str()
                    .unwrap()
                    .to_owned();
                state["limb_recycle"][key] = 2.into();
            });
            let before = serde_json::to_value(&candidate.btech).unwrap();
            let result = select_battle_spotter(&mut candidate, source, ObjectId(1), Some(observer));
            assert_eq!(result.is_ok(), allowed);
            if !allowed {
                assert_eq!(serde_json::to_value(&candidate.btech).unwrap(), before);
            }
        }
    }
}
