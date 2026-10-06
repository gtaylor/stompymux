//! Hiding shares cockpit authority, cached visibility, elapsed events and cover loss across chassis.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Edit isolated scenario facts without bypassing final world validation.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    world.btech.rewrite_unit_record(id, change).unwrap();
}

/// Observe common hiding state without exposing anatomy in assertions.
fn hiding(world: &World, id: ObjectId) -> (Option<u16>, bool) {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return (unit.hide_elapsed(), unit.signature().hidden);
    }
    let unit = &world.btech.constructed_units()[&id];
    (unit.hide_elapsed(), unit.signature().hidden)
}

/// One running wizard-piloted unit on forest cover, with deterministic owned dice.
async fn fixture(
    source: &str,
    camouflage: bool,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Camouflage field".into(), Kind::Room);
    let row = format!("{}0", Terrain::LightWoods.symbol()).repeat(3) + "\n";
    create_battle_map(
        &mut world,
        map,
        "cover",
        MapAsset::from_cells(&format!("3 3\n{}", row.repeat(3))).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, "Hiding unit".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    let source = match (camouflage, source.contains("specials = [")) {
        (false, _) => source.to_owned(),
        (true, true) => source.replacen("specials = [", "specials = [\"Camo_Tech\", ", 1),
        (true, false) => format!("specials = [\"Camo_Tech\"]\n{source}"),
    };
    UnitTemplate::parse("test", &source)
        .unwrap()
        .create(&mut world, id)
        .unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, id, map, 1, 2).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    edit(&mut world, id, |unit| {
        unit["dice"] = serde_json::to_value(Dice::seeded([42; 32])).unwrap()
    });
    world.validate(&config).unwrap();
    (dir, config, world, map, id)
}

/// Both interfaces schedule the same inclusive timer, and saved events replay identically.
#[tokio::test]
async fn hiding_native_lua_all_chassis_timing_and_restart() {
    let tracked = include_str!("../game/units/Demolisher.toml");
    let wheel = tracked.replace("movement = \"track\"", "movement = \"wheel\"");
    let hover = tracked.replace("movement = \"track\"", "movement = \"hover\"");
    let stationary = tracked
        .replace("movement = \"track\"", "movement = \"none\"")
        .replace("walk_mp = 5", "walk_mp = 0");
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/GOL-1H.toml"),
        tracked,
        wheel.as_str(),
        hover.as_str(),
        stationary.as_str(),
        include_str!("../game/units/Kestrel.toml"),
    ] {
        for camouflage in [false, true] {
            let (_dir, config, world, _map, id) = fixture(source, camouflage).await;
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let call = format!("btech.unit.hide({},1)", id.0);
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, world.btech);
            assert!(lua.drain_outbox().is_empty());
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                "hide ignored trailing text",
            );
            assert!(text.contains("start to hide"), "{text}");
            lua.eval_callback::<()>(&call).unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            assert_eq!(hiding(&native.world(), id), (Some(0), false));
            assert_eq!(
                lua.eval_callback::<u16>(&format!(
                    "return btech.unit.state({}).hide_elapsed",
                    id.0
                ))
                .unwrap(),
                0
            );
            let mut world = native.world().clone();
            let before = world.btech.clone();
            assert!(
                begin_battle_hiding(&mut world, id, ObjectId(1))
                    .unwrap_err()
                    .to_string()
                    .contains("already")
            );
            assert_eq!(world.btech, before);
            for _ in 0..7 {
                assert!(advance_battle_hiding(&mut world).unwrap().is_empty());
            }
            persistence::save(&config.database(), &world).await.unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            let vtol = world
                .btech
                .vehicles()
                .get(&id)
                .is_some_and(|unit| unit.definition().is_vtol());
            let limit = if vtol { 40 } else { 50 } * if camouflage { 1 } else { 2 };
            for tick in 8..=limit + 1 {
                let notices = advance_battle_hiding(&mut world).unwrap();
                assert_eq!(notices, advance_battle_hiding(&mut restored).unwrap());
                assert_eq!(world.btech, restored.btech);
                if tick <= limit {
                    assert!(notices.is_empty());
                    assert_eq!(hiding(&world, id), (Some(tick), false));
                } else {
                    assert_eq!(notices.len(), 1);
                    assert_eq!(notices[0].text, "You are now hidden!");
                }
            }
            assert_eq!(hiding(&world, id), (None, true));
            assert!(!hiding_pending(&world));
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
        }
    }
}

/// Camouflage grants ordinary pilots admission; hostile cached observations stop the event without rolls.
#[tokio::test]
async fn hiding_authority_and_cached_observer_rules() {
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/Demolisher.toml"),
    ] {
        for camouflage in [false, true] {
            let (_dir, config, mut world, map, id) = fixture(source, camouflage).await;
            release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
            world
                .objects
                .get_mut(&ObjectId(2))
                .unwrap()
                .flags
                .remove(Flag::Wizard);
            world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
            assign_battle_pilot(&mut world, id, ObjectId(2)).unwrap();
            support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
            let before = world.btech.clone();
            let result = begin_battle_hiding(&mut world, id, ObjectId(2));
            if !camouflage {
                assert!(result.unwrap_err().to_string().contains("aren't capable"));
                assert_eq!(world.btech, before);
                continue;
            }
            result.unwrap();
            let observer = world.create(&config, "Observer".into(), Kind::Thing);
            world.objects.get_mut(&observer).unwrap().home = Some(ObjectId(config.home()));
            UnitTemplate::parse("Hunter", include_str!("../game/units/Hunter.toml"))
                .unwrap()
                .create(&mut world, observer)
                .unwrap();
            support::seed_object_dice(&mut world, observer, support::FIXTURE_DICE_SEED);
            place_battle_unit(&mut world, observer, map, 1, 0).unwrap();
            world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
            assign_battle_pilot(&mut world, observer, ObjectId(1)).unwrap();
            support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
            start_battle_unit(&mut world, observer, ObjectId(1), true).unwrap();
            for _ in 0..5 {
                advance_battle_units(&mut world, 0);
            }
            edit(&mut world, observer, |unit| {
                unit["signature"]["team"] = serde_json::json!(1);
                unit["contacts"] = serde_json::json!({id.0.to_string(): {"identified":false}});
            });
            for condition in [
                "enemy",
                "friendly",
                "off",
                "observer",
                "unacquired",
                "invisible",
                "clairvoyant",
            ] {
                let mut test = world.clone();
                set_battle_visibility(
                    &mut test,
                    observer,
                    Visibility {
                        invisible: condition == "invisible",
                        clairvoyant: condition == "clairvoyant",
                    },
                )
                .unwrap();
                edit(&mut test, observer, |unit| match condition {
                    "friendly" => unit["signature"]["team"] = serde_json::json!(0),
                    "off" => unit["power"] = serde_json::to_value(Power::Off).unwrap(),
                    "observer" => unit["observer"] = serde_json::json!(true),
                    "unacquired" => unit["contacts"] = serde_json::json!({}),
                    _ => {}
                });
                test.validate(&config).unwrap();
                let unchanged_observer = test.btech.vehicles()[&observer].clone();
                let notices = advance_battle_hiding(&mut test).unwrap();
                assert_eq!(test.btech.vehicles()[&observer], unchanged_observer);
                if condition == "enemy" {
                    assert!(notices[0].text.contains("spidey sense"));
                    assert_eq!(hiding(&test, id), (None, false));
                } else {
                    assert!(notices.is_empty());
                    assert_eq!(hiding(&test, id), (Some(1), false));
                }
            }
        }
    }
}

/// A rejected firing attempt still reveals cover; aborted Lua callbacks restore it with the timer.
#[tokio::test]
async fn hiding_fire_intent_survives_rejection_and_rolls_back_with_callbacks() {
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/Demolisher.toml"),
        include_str!("../game/units/Kestrel.toml"),
    ] {
        let (_dir, config, mut world, _, id) = fixture(source, false).await;
        edit(&mut world, id, |unit| {
            unit["signature"]["hidden"] = serde_json::json!(true);
            unit["hide_elapsed"] = serde_json::json!(7);
        });
        edit_battle_tic(&mut world, id, ObjectId(1), 0, TicEdit::Add(vec![0])).unwrap();
        for command in ["fire 0 invalid", "fire 999999 invalid", "firetic 0 invalid"] {
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let text = support::run_text(&native, &config, ObjectId(1), 1, command);
            assert!(text.contains("break out of your cover"), "{text}");
            assert!(!text.contains("You fire"));
            let mut expected = world.clone();
            edit(&mut expected, id, |unit| {
                unit["signature"]["hidden"] = serde_json::json!(false);
                unit["hide_elapsed"] = serde_json::Value::Null;
            });
            assert_eq!(native.world().btech, expected.btech);
        }
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let call = format!("btech.unit.tic_fire({},1,{{0}},999999)", id.0);
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        assert!(lua.drain_outbox().is_empty());
        lua.eval_callback::<()>(&call).unwrap();
        assert_eq!(hiding(&lua.world(), id), (None, false));
        world.validate(&config).unwrap();
    }
}

/// Armor hits reveal established cover while preparation survives; shutdown cancels only preparation.
#[tokio::test]
async fn hiding_damage_and_shutdown_distinguish_cover_from_preparation() {
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/GOL-1H.toml"),
        include_str!("../game/units/Demolisher.toml"),
        include_str!("../game/units/Kestrel.toml"),
    ] {
        let (_dir, config, mut world, _, id) = fixture(source, false).await;
        edit(&mut world, id, |unit| {
            unit["signature"]["hidden"] = serde_json::json!(true);
            unit["hide_elapsed"] = serde_json::json!(3);
        });
        for amount in [0, 1] {
            let mut test = world.clone();
            let notices = if test.btech.vehicles().contains_key(&id) {
                let mut rules = VehicleImpactRules::STANDARD.criticals;
                rules.enabled = false;
                resolve_battle_vehicle_armor_damage(
                    &mut test,
                    id,
                    VehicleArmorHit {
                        damage_class: DamageClass::Ordinary,
                        section: VehicleSection::Front,
                        amount: u32::from(amount),
                        through_armor_critical: false,
                        armor_piercing: None,
                    },
                    rules,
                )
                .unwrap()
                .notices
            } else {
                resolve_battle_tactical_impact(
                    &mut test,
                    id,
                    Hit {
                        section: MechSection::CenterTorso,
                        rear_armor: false,
                        through_armor_critical: false,
                        crew_stun: false,
                    },
                    amount,
                    FallRules::configured(&config),
                )
                .unwrap()
                .notices
            };
            assert_eq!(hiding(&test, id), (Some(3), amount == 0));
            assert_eq!(
                notices
                    .iter()
                    .any(|notice| notice.text.contains("cover is ruined")),
                amount > 0
            );
            test.validate(&config).unwrap();
        }
        stop_battle_unit(&mut world, id, ObjectId(1), FallRules::configured(&config)).unwrap();
        assert_eq!(hiding(&world, id), (None, true));
        world.validate(&config).unwrap();
    }
}

/// Real hex crossings reveal and cancel hiding; sub-hex motion leaves cover intact.
#[tokio::test]
async fn hiding_movement_waits_for_a_hex_crossing() {
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/GOL-1H.toml"),
        include_str!("../game/units/Demolisher.toml"),
        include_str!("../game/units/Kestrel.toml"),
    ] {
        let (_dir, config, mut world, _, id) = fixture(source, false).await;
        let vtol = world
            .btech
            .vehicles()
            .get(&id)
            .is_some_and(|unit| unit.definition().is_vtol());
        edit(&mut world, id, |unit| {
            unit["signature"]["hidden"] = serde_json::json!(true);
            unit["motion"]["speed"] = serde_json::json!(10.75);
            unit["motion"]["desired_speed"] = serde_json::json!(10.75);
            if vtol {
                unit["vtol_flight"] = serde_json::to_value(VtolFlight {
                    phase: VtolFlightPhase::Airborne,
                    altitude: 1.0,
                    ..Default::default()
                })
                .unwrap();
            } else {
                unit["hide_elapsed"] = serde_json::json!(3);
            }
        });
        world.validate(&config).unwrap();
        assert!(
            advance_battle_motion(&mut world, MovementRules::STANDARD)
                .unwrap()
                .iter()
                .all(|n| !n.text.contains("break your cover"))
        );
        assert!(hiding(&world, id).1);
        let mut crossed = false;
        for _ in 0..90 {
            let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
            if !hiding(&world, id).1 {
                assert!(notices.iter().any(|n| n.text.contains("break your cover")));
                crossed = true;
                break;
            }
        }
        assert!(crossed);
        assert_eq!(hiding(&world, id), (None, false));
        world.validate(&config).unwrap();
    }
}

/// An idle vehicle keeps its hide event alive through the real persisted server heartbeat.
#[tokio::test(flavor = "current_thread")]
async fn hiding_idle_server_heartbeat_finishes_saved_event() {
    use std::cell::Cell;
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_dir, config, mut world, _, id) =
                fixture(include_str!("../game/units/Demolisher.toml"), true).await;
            begin_battle_hiding(&mut world, id, ObjectId(1)).unwrap();
            edit(&mut world, id, |unit| {
                unit["hide_elapsed"] = serde_json::json!(49)
            });
            persistence::save(&config.database(), &world).await.unwrap();
            let (_address, shutdown, task, _, mut heartbeats) =
                support::start(&config, Rc::new(Cell::new(1))).await;
            heartbeats
                .until_saved(&config, 8, |saved| hiding(saved, id) == (None, true))
                .await;
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}

/// Failed persistence cannot publish hide completion ahead of its saved counter.
#[tokio::test]
async fn hiding_failed_save_preserves_pending_event_for_retry() {
    use sqlx::Connection;
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/Demolisher.toml"),
    ] {
        let (_dir, config, mut world, _, id) = fixture(source, true).await;
        begin_battle_hiding(&mut world, id, ObjectId(1)).unwrap();
        edit(&mut world, id, |unit| {
            unit["hide_elapsed"] = serde_json::json!(50)
        });
        persistence::save(&config.database(), &world).await.unwrap();
        let before = world.btech.clone();
        let report = advance_battle_hiding(&mut world).unwrap();
        assert_eq!(report[0].text, "You are now hidden!");
        let mut sql = sqlx::SqliteConnection::connect_with(
            &sqlx::sqlite::SqliteConnectOptions::new().filename(config.database()),
        )
        .await
        .unwrap();
        let table = if world.btech.vehicles().contains_key(&id) {
            "btech_vehicles"
        } else {
            "btech_units"
        };
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER reject_hide BEFORE UPDATE ON {table} WHEN NEW.dbref={} BEGIN SELECT RAISE(ABORT,'hide save failure'); END",id.0))).execute(&mut sql).await.unwrap();
        assert!(persistence::save(&config.database(), &world).await.is_err());
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, before);
        assert_eq!(report, advance_battle_hiding(&mut restored).unwrap());
        assert_eq!(restored.btech, world.btech);
        sqlx::query("DROP TRIGGER reject_hide")
            .execute(&mut sql)
            .await
            .unwrap();
        persistence::save(&config.database(), &restored)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

/// Cover and posture gates reject without mutations; the pending event checks height on every tick.
#[tokio::test]
async fn hiding_admission_and_elevation_checks_are_atomic() {
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/Demolisher.toml"),
        include_str!("../game/units/Kestrel.toml"),
    ] {
        let (_dir, config, world, map, id) = fixture(source, false).await;
        for (speed, accepted) in [(10.75, true), (10.7501, false)] {
            let mut test = world.clone();
            edit(&mut test, id, |unit| {
                unit["motion"]["speed"] = serde_json::json!(speed)
            });
            let before = test.btech.clone();
            let result = begin_battle_hiding(&mut test, id, ObjectId(1));
            assert_eq!(result.is_ok(), accepted);
            if !accepted {
                assert!(result.unwrap_err().to_string().contains("complete stop"));
                assert_eq!(test.btech, before);
            }
        }
        let mut terrain = world.clone();
        let mut state = serde_json::to_value(&terrain.btech).unwrap();
        state["maps"][map.0.to_string()]["terrain"][7] =
            serde_json::to_value(Hex::new(Terrain::Clear, 0)).unwrap();
        terrain.btech = serde_json::from_value(state).unwrap();
        let before = terrain.btech.clone();
        assert!(
            begin_battle_hiding(&mut terrain, id, ObjectId(1))
                .unwrap_err()
                .to_string()
                .contains("isn't going to work")
        );
        assert_eq!(terrain.btech, before);
        let mut high = world.clone();
        begin_battle_hiding(&mut high, id, ObjectId(1)).unwrap();
        let vtol = high
            .btech
            .vehicles()
            .get(&id)
            .is_some_and(|u| u.definition().is_vtol());
        edit(&mut high, id, |unit| {
            if vtol {
                unit["vtol_flight"] = serde_json::to_value(VtolFlight {
                    altitude: 1.0,
                    ..Default::default()
                })
                .unwrap();
            } else {
                unit["ground_elevation"] = serde_json::json!(1.0);
            }
        });
        high.validate(&config).unwrap();
        assert!(
            advance_battle_hiding(&mut high).unwrap()[0]
                .text
                .contains("spidey sense")
        );
        assert_eq!(hiding(&high, id), (None, false));
        if vtol {
            let mut airborne = world.clone();
            edit(&mut airborne, id, |unit| {
                unit["vtol_flight"] = serde_json::to_value(VtolFlight {
                    phase: VtolFlightPhase::Airborne,
                    altitude: 1.0,
                    ..Default::default()
                })
                .unwrap()
            });
            assert!(
                begin_battle_hiding(&mut airborne, id, ObjectId(1))
                    .unwrap_err()
                    .to_string()
                    .contains("must be landed")
            );
        }
    }
}

/// A crash reached through another hex loses cover to movement before any armor packet runs.
#[tokio::test]
async fn hiding_aircraft_crash_orders_crossing_before_damage_and_replays() {
    for crossed in [false, true] {
        let (_dir, config, mut world, map, id) =
            fixture(include_str!("../game/units/Kestrel.toml"), false).await;
        world
            .btech
            .rewrite_map_record(map, |record| {
                record["movement_modifier"] = 10000.into();
                record["terrain"][4] =
                    serde_json::to_value(Hex::new(Terrain::LightWoods, 0)).unwrap();
                record["terrain"][7] = serde_json::to_value(Hex::new(Terrain::Clear, 1)).unwrap();
            })
            .unwrap();
        edit(&mut world, id, |unit| {
            unit["signature"]["hidden"] = true.into();
            unit["hide_elapsed"] = 7.into();
            unit["motion"]["speed"] = if crossed { 100.0 } else { 0.0 }.into();
            unit["motion"]["desired_speed"] = if crossed { 100.0 } else { 0.0 }.into();
            unit["vtol_flight"] = serde_json::to_value(VtolFlight {
                phase: VtolFlightPhase::Airborne,
                altitude: 1.5,
                vertical_speed: if crossed { 0.0 } else { -129.0 },
                ..Default::default()
            })
            .unwrap();
        });
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        let mut rules = MovementRules::STANDARD.fall;
        rules.vehicle_impact.criticals.enabled = false;
        if crossed {
            let movement_rules = MovementRules {
                fall: rules,
                ..MovementRules::STANDARD
            };
            let live = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            advance_battle_motion_action(&live, &config, movement_rules).unwrap();
            assert_eq!(hiding(&live.world(), id), (None, false));
            // The invalid location is caught by per-operation validation, which runs only in
            // debug builds; release relies on the commit check.
            if cfg!(debug_assertions) {
                let mut invalid = world.clone();
                invalid.objects.get_mut(&id).unwrap().location = Some(ObjectId(1));
                let failed = Scripts::new(&config, Rc::new(RefCell::new(invalid))).unwrap();
                let before = failed.world().btech.clone();
                let error =
                    advance_battle_motion_action(&failed, &config, movement_rules).unwrap_err();
                assert!(
                    error
                        .to_string()
                        .contains("Placed vehicle location differs"),
                    "{error}"
                );
                assert_eq!(failed.world().btech, before);
                assert!(failed.drain_outbox().is_empty());
            }
        }
        let result = advance_battle_vtol_environment(&mut world, id, false, rules).unwrap();
        assert_eq!(
            advance_battle_vtol_environment(&mut replay, id, false, rules).unwrap(),
            result
        );
        assert_eq!(world.btech, replay.btech);
        let (fall, notices) = match result {
            VtolEnvironment::Crashed { fall, .. } if !crossed => {
                let notices = fall.feedback.notices.clone();
                (fall, notices)
            }
            VtolEnvironment::Obstacle {
                fall: Some(fall),
                notices,
                ..
            } if crossed => (fall, notices),
            result => panic!("Expected an aircraft crash (crossed={crossed}): {result:?}"),
        };
        assert!(!fall.groups.is_empty());
        assert_eq!(
            world.btech.vehicles()[&id].position().unwrap().y,
            if crossed { 1 } else { 2 }
        );
        let movement = notices
            .iter()
            .filter(|n| n.text.contains("break your cover"))
            .count();
        let damage = notices
            .iter()
            .filter(|n| n.text.contains("cover is ruined"))
            .count();
        assert_eq!((movement, damage), if crossed { (1, 0) } else { (0, 1) });
        if crossed {
            let movement = notices
                .iter()
                .position(|n| n.text.contains("break your cover"))
                .unwrap();
            let crash = notices
                .iter()
                .position(|n| n.text.contains("bad feeling"))
                .unwrap();
            assert!(movement < crash);
            assert_eq!(hiding(&world, id), (None, false));
        }
        world.validate(&config).unwrap();
    }
}

/// Operator hold rejects native/Lua/Rust firing before parsing or revealing across every chassis.
#[tokio::test]
async fn weapons_hold_controls_admission_cover_and_saved_state() {
    let tracked = include_str!("../game/units/Demolisher.toml");
    let chassis = [
        include_str!("../game/units/JR7-D.toml").to_owned(),
        include_str!("../game/units/GOL-1H.toml").to_owned(),
        tracked.to_owned(),
        tracked.replace("movement = \"track\"", "movement = \"wheel\""),
        tracked.replace("movement = \"track\"", "movement = \"hover\""),
        tracked
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 5", "walk_mp = 0"),
        include_str!("../game/units/Kestrel.toml").to_owned(),
    ];
    for source in chassis {
        let (_dir, config, mut world, _, id) = fixture(&source, false).await;
        edit(&mut world, id, |unit| {
            unit["signature"]["hidden"] = true.into();
            unit["hide_elapsed"] = 7.into();
        });
        edit_battle_tic(&mut world, id, ObjectId(1), 0, TicEdit::Add(vec![0])).unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        assert!(!weapons_hold(&world, id).unwrap());
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("@btech unit-weapons-hold #{}=on", id.0),
        );
        assert!(text.contains("weapons hold enabled"), "{text}");
        let call = format!("btech.unit.weapons_hold({},true)", id.0);
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        assert!(
            lua.eval_callback::<bool>(&format!("return {call}"))
                .unwrap()
        );
        assert_eq!(native.world().btech, lua.world().btech);
        assert!(
            lua.eval_callback::<bool>(&format!(
                "return btech.unit.weapons_hold({}) and btech.unit.state({}).weapons_hold",
                id.0, id.0
            ))
            .unwrap()
        );
        let held = native.world().btech.clone();
        let ready = support::run_text(&native, &config, ObjectId(1), 1, "weapons");
        for command in [
            "fire",
            "fire nonsense",
            "fire 99999 invalid",
            "fire 0 1 2 3",
            "firetic",
            "firetic nonsense",
            "firetic 99999 invalid",
            "firetic 0 invalid",
        ] {
            let text = support::run_text(&native, &config, ObjectId(1), 1, command);
            assert!(
                text.contains("Currently in weapons hold."),
                "{command}: {text}"
            );
            assert!(!text.contains("break out of your cover"));
            assert_eq!(native.world().btech, held);
        }
        for call in [
            format!("btech.unit.fire({},1,999999,999999)", id.0),
            format!("btech.unit.tic_fire({},1,{{}},999999)", id.0),
        ] {
            let error = lua.eval_callback::<()>(&call).unwrap_err();
            assert!(
                error.to_string().contains("Currently in weapons hold."),
                "{error}"
            );
            assert_eq!(lua.world().btech, held);
            assert!(lua.drain_outbox().is_empty());
        }
        let error = fire_battle_tics(&native, &config, id, ObjectId(1), vec![], None).unwrap_err();
        assert!(error.to_string().contains("Currently in weapons hold."));
        assert_eq!(native.world().btech, held);
        let status = support::run_text(&native, &config, ObjectId(1), 1, "status info");
        assert!(status.contains("WEAPONS HOLD"), "{status}");
        let snapshot = native.world().clone();
        persistence::save(&config.database(), &snapshot)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, held);
        stop_battle_unit(
            &mut restored,
            id,
            ObjectId(1),
            FallRules::configured(&config),
        )
        .unwrap();
        assert!(weapons_hold(&restored, id).unwrap());
        set_battle_weapons_hold(&mut restored, id, false).unwrap();
        assert!(!weapons_hold(&restored, id).unwrap());
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("@btech unit-weapons-hold #{}=off", id.0),
        );
        assert!(text.contains("weapons hold disabled"), "{text}");
        assert_eq!(
            ready,
            support::run_text(&native, &config, ObjectId(1), 1, "weapons")
        );
        assert_eq!(native.world().btech, world.btech);
        let text = support::run_text(&native, &config, ObjectId(1), 1, "fire 999999 invalid");
        assert!(text.contains("break out of your cover"), "{text}");
        assert_eq!(hiding(&native.world(), id), (None, false));
    }
}

/// Cockpit authority and startup precede hold, while administrative edits reject invalid subjects.
#[tokio::test]
async fn weapons_hold_authority_and_rejected_edits_are_atomic() {
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/Demolisher.toml"),
        include_str!("../game/units/Kestrel.toml"),
    ] {
        let (_dir, config, mut world, map, id) = fixture(source, false).await;
        world
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let command = format!("@btech unit-weapons-hold #{}=on", id.0);
        support::run_text(&scripts, &config, ObjectId(2), 2, &command);
        assert_eq!(scripts.world().btech, world.btech);
        let text = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech unit-weapons-hold #{}=maybe", id.0),
        );
        assert!(text.contains("Usage:"), "{text}");
        assert_eq!(scripts.world().btech, world.btech);
        for subject in [map, ObjectId(999999)] {
            assert!(set_battle_weapons_hold(&mut scripts.world_mut(), subject, true).is_err());
            assert_eq!(scripts.world().btech, world.btech);
        }
        let mut going = world.clone();
        going
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::Going);
        assert!(set_battle_weapons_hold(&mut going, id, true).is_err());
        assert_eq!(going.btech, world.btech);
        set_battle_weapons_hold(&mut scripts.world_mut(), id, true).unwrap();
        let held = scripts.world().btech.clone();
        for command in ["fire nonsense", "firetic nonsense"] {
            let text = support::run_text(&scripts, &config, ObjectId(2), 2, command);
            assert!(!text.contains("Currently in weapons hold."), "{text}");
            assert_eq!(scripts.world().btech, held);
        }
        stop_battle_unit(
            &mut scripts.world_mut(),
            id,
            ObjectId(1),
            FallRules::configured(&config),
        )
        .unwrap();
        assign_battle_pilot(&mut scripts.world_mut(), id, ObjectId(1)).unwrap();
        support::seed_object_dice(
            &mut scripts.world_mut(),
            ObjectId(1),
            support::FIXTURE_DICE_SEED,
        );
        let stopped = scripts.world().btech.clone();
        for command in ["fire nonsense", "firetic nonsense"] {
            let text = support::run_text(&scripts, &config, ObjectId(1), 1, command);
            assert!(text.contains("Unit must be started"), "{text}");
            assert!(!text.contains("Currently in weapons hold."));
            assert_eq!(scripts.world().btech, stopped);
        }
    }
}
