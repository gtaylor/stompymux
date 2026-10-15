//! Self-destruction shares cockpit authority, scheduling, blast paths and committed replay across chassis.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Isolate event scheduling from unrelated movement and combat setup.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    world.btech.rewrite_unit_record(id, change).unwrap();
}

/// One running cockpit on a flat map, with an ordinary, present pilot and destructive ammunition.
async fn fixture(chassis: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    fixture_on(chassis, ".0").await
}

/// Terrain variants share the same cockpit and construction setup.
async fn fixture_on(chassis: &str, tile: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Self destruct field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "self destruct",
        MapAsset::from_cells(&format!("1 1\n{tile}\n")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, chassis.into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    match chassis {
        "biped" | "quad" => create_battle_unit(
            &mut world,
            id,
            MechTemplate::parse(
                "test",
                if chassis == "biped" {
                    include_str!("../game/units/JR7-D.toml")
                } else {
                    include_str!("../game/units/GOL-1H.toml")
                },
            )
            .unwrap(),
        )
        .unwrap(),
        _ => {
            let text = match chassis {
                "vtol" => include_str!("../game/units/Kestrel.toml").to_owned(),
                "wheel" => include_str!("../game/units/Demolisher.toml")
                    .replace("movement = \"track\"", "movement = \"wheel\""),
                "hover" => crate::support::templates::hovercraft(include_str!(
                    "../game/units/Demolisher.toml"
                )),
                "stationary" => include_str!("../game/units/Demolisher.toml")
                    .replace("movement = \"track\"", "movement = \"none\"")
                    .replace("walk_mp = 5", "walk_mp = 0"),
                _ => include_str!("../game/units/Demolisher.toml").to_owned(),
            };
            create_battle_vehicle(
                &mut world,
                id,
                VehicleTemplate::parse("test", &text).unwrap(),
            )
            .unwrap();
        }
    }
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    // A failed initial crew check followed by a successful recovery makes terminal replay observable.
    let seed = (0..=255)
        .find(|s| {
            let mut dice = Dice::seeded([*s; 32]);
            dice.consciousness_roll(false) < 10 && dice.consciousness_roll(false) >= 10
        })
        .unwrap();
    let combat_seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 8)
        .unwrap();
    edit(&mut world, id, |s| {
        s["crew_recovery"]["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        s["dice"] = serde_json::to_value(Dice::seeded([combat_seed; 32])).unwrap();
    });
    (dir, config, world, id)
}

/// Run a wizard override countdown through its two waiting ticks, returning the detonation tick.
fn detonate(scripts: &Scripts, config: &Config) -> Vec<SelfDestructOutcome> {
    for remaining in [2, 1] {
        let outcomes = advance_battle_self_destructs_action(scripts, config).unwrap();
        assert!(
            outcomes
                .iter()
                .all(|outcome| matches!(outcome, SelfDestructOutcome::Countdown { remaining: r, .. } if *r == remaining)),
            "{outcomes:?}"
        );
    }
    advance_battle_self_destructs_action(scripts, config).unwrap()
}

/// Airborne wreck placement uses terrain support, then raises the wreck and restarts descent.
#[tokio::test]
async fn self_destruct_airborne_vehicle_placement() {
    for chassis in ["vtol", "track", "wheel", "hover", "stationary"] {
        for (tile, altitude, expected) in [
            (".2", 20.0, 8),
            ("/4", 20.0, 10),
            ("/4", 2.0, 5),
            ("~3", 20.0, 3),
            ("-3", 20.0, 3),
        ] {
            let (_dir, config, mut world, id) = fixture_on(chassis, tile).await;
            edit(&mut world, id, |state| {
                if chassis != "vtol" {
                    state["ground_elevation"] = altitude.into();
                    return;
                }
                state["vtol_flight"] = serde_json::to_value(VtolFlight {
                    phase: VtolFlightPhase::Airborne,
                    altitude,
                    vertical_speed: 4.0,
                    fall: None,
                })
                .unwrap();
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            self_destruct_action(&scripts, &config, id, ObjectId(1), "ammo override").unwrap();
            let outcomes = detonate(&scripts, &config);
            assert!(matches!(outcomes[0], SelfDestructOutcome::Vehicle { .. }));
            let world = scripts.world();
            let unit = &world.btech.vehicles()[&id];
            let falling = !(tile == "/4" && altitude < 4.0);
            let hex = MapAsset::from_cells(&format!("1 1\n{tile}\n"))
                .unwrap()
                .hex(0, 0)
                .unwrap();
            assert_eq!(unit.elevation_level(hex), expected, "{chassis} {tile}");
            assert_eq!(unit.free_fall(), falling.then(|| FreeFall::new(expected)));
            if let Some(flight) = unit.vtol_flight() {
                assert_eq!(flight.vertical_speed, 0.0);
                assert_eq!(
                    flight.phase,
                    if falling {
                        VtolFlightPhase::Falling
                    } else {
                        VtolFlightPhase::Landed
                    }
                );
            }
            world.validate(&config).unwrap();
            let restored: BtechState =
                serde_json::from_value(serde_json::to_value(&world.btech).unwrap()).unwrap();
            assert_eq!(restored, world.btech);
        }
    }
}

/// Native and Lua select the same sequence; every supported chassis survives a mid-countdown restart.
#[tokio::test]
async fn self_destruct_cross_chassis_native_lua_and_restart() {
    for chassis in [
        "biped",
        "quad",
        "track",
        "wheel",
        "hover",
        "stationary",
        "vtol",
    ] {
        let (_dir, config, world, id) = fixture(chassis).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.explode({},1,'ammo'); error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, world.btech);
        let output = support::run_text(&scripts, &config, ObjectId(1), 1, "explode ammo override");
        assert!(
            output.contains("ammunition will explode in 3 seconds"),
            "{output}"
        );
        assert!(self_destructs_pending(&scripts.world()));
        let pilot: Option<i64> = scripts
            .eval_callback(&format!("return btech.unit.state({}).pilot", id.0))
            .unwrap();
        assert_eq!(pilot, None);
        let first = advance_battle_self_destructs_action(&scripts, &config).unwrap();
        assert!(matches!(
            &first[..],
            [SelfDestructOutcome::Countdown { remaining: 2, .. }]
        ));
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, saved.btech);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
        let second = advance_battle_self_destructs_action(&scripts, &config).unwrap();
        assert!(matches!(
            &second[..],
            [SelfDestructOutcome::Countdown { remaining: 1, .. }]
        ));
        let reports = advance_battle_self_destructs_action(&scripts, &config).unwrap();
        assert_eq!(reports.len(), 1);
        if chassis == "biped" || chassis == "quad" {
            let SelfDestructOutcome::Ammunition { impacts, .. } = &reports[0] else {
                panic!("{reports:?}")
            };
            assert!(!impacts.is_empty());
            let snapshot = scripts.world();
            let unit = &snapshot.btech.constructed_units()[&id];
            assert!(unit.self_destruct().is_none());
            assert!(unit.largest_ammunition_hazard_bin().unwrap().is_none());
            assert!(unit.is_destroyed());
        } else {
            let SelfDestructOutcome::Vehicle {
                damage,
                crew_injury,
                ..
            } = &reports[0]
            else {
                panic!("{reports:?}")
            };
            assert_eq!(damage.destroyed_sections, vec![VehicleSection::Rear]);
            assert_eq!(crew_injury.injuries, 4);
            let snapshot = scripts.world();
            let vehicle = &snapshot.btech.vehicles()[&id];
            assert!(vehicle.is_destroyed());
            // A wreck's crew makes no consciousness rolls, so no recovery attempt starts.
            assert_eq!(vehicle.crew_recovery().remaining, 0);
            assert_eq!(vehicle.elevation_level(Hex::new(Terrain::Clear, 0)), 6);
            assert!(vehicle.sections()[&VehicleSection::Front].internal > 0);
        }
        assert!(!self_destructs_pending(&scripts.world()));
        let after = scripts.world().clone();
        persistence::save(&config.database(), &after).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, after.btech);
        for _ in 0..30 {
            advance_battle_units(&mut restored, 0, true);
        }
        let remaining = restored
            .btech
            .constructed_units()
            .get(&id)
            .map(|u| u.crew_recovery().remaining)
            .or_else(|| {
                restored
                    .btech
                    .vehicles()
                    .get(&id)
                    .map(|u| u.crew_recovery().remaining)
            })
            .unwrap();
        assert_eq!(remaining, 0);
        restored.validate(&config).unwrap();
    }
}

/// Safety, cooldowns and override cannot bypass physical cockpit ownership; stop is always available.
#[tokio::test]
async fn self_destruct_admission_stop_and_atomic_cancel() {
    let (_dir, config, world, id) = fixture("biped").await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    for args in ["anything", "reactor"] {
        assert!(
            self_destruct_action(&scripts, &config, id, ObjectId(1), args)
                .unwrap_err()
                .to_string()
                .contains("Usage")
        );
        assert_eq!(scripts.world().btech, world.btech);
    }
    assert!(self_destruct_action(&scripts, &config, id, ObjectId(2), "ammo override").is_err());
    assert_eq!(scripts.world().btech, world.btech);
    let before_flag = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.explode_safe({},true); error('abort')",
                id.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before_flag);
    set_battle_self_destruct_safe(&mut scripts.world_mut(), id, true).unwrap();
    assert!(
        self_destruct_action(&scripts, &config, id, ObjectId(1), "ammo")
            .unwrap_err()
            .to_string()
            .contains("possibility")
    );
    edit(&mut scripts.world_mut(), id, |s| {
        s["weapon_recycle"]["0"] = 1.into();
        s["limb_recycle"]["LeftArm"] = 1.into();
    });
    scripts.world().validate(&config).unwrap();
    assert!(
        self_destruct_action(&scripts, &config, id, ObjectId(1), "ammo")
            .unwrap_err()
            .to_string()
            .contains("weapons recycling")
    );
    self_destruct_action(&scripts, &config, id, ObjectId(1), "ammo ignored override").unwrap();
    assert_eq!(
        scripts.world().btech.constructed_units()[&id]
            .self_destruct()
            .unwrap()
            .remaining,
        3
    );
    assert!(
        self_destruct_action(&scripts, &config, id, ObjectId(1), "ammo override")
            .unwrap_err()
            .to_string()
            .contains("already undergoing")
    );
    let pending = scripts.world().btech.clone();
    assert!(self_destruct_action(&scripts, &config, id, ObjectId(2), "stop override").is_err());
    assert_eq!(scripts.world().btech, pending);
    assign_battle_pilot(&mut scripts.world_mut(), id, ObjectId(1)).unwrap();
    support::seed_object_dice(
        &mut scripts.world_mut(),
        ObjectId(1),
        support::FIXTURE_DICE_SEED,
    );
    let assigned = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.explode({},1,'stop override'); error('abort')",
                id.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, assigned);
    // Aborting never waits for weapon or limb recovery.
    self_destruct_action(&scripts, &config, id, ObjectId(1), "stop").unwrap();
    assert!(!self_destructs_pending(&scripts.world()));
    assert!(
        self_destruct_action(&scripts, &config, id, ObjectId(1), "stop override")
            .unwrap_err()
            .to_string()
            .contains("isn't undergoing")
    );
    *scripts.world_mut() = world;
    self_destruct_action(&scripts, &config, id, ObjectId(1), "ammo").unwrap();
    assert_eq!(
        scripts.world().btech.constructed_units()[&id]
            .self_destruct()
            .unwrap()
            .remaining,
        SELF_DESTRUCT_DELAY
    );
    assign_battle_pilot(&mut scripts.world_mut(), id, ObjectId(1)).unwrap();
    support::seed_object_dice(
        &mut scripts.world_mut(),
        ObjectId(1),
        support::FIXTURE_DICE_SEED,
    );
    self_destruct_action(&scripts, &config, id, ObjectId(1), "stop").unwrap();
    self_destruct_action(&scripts, &config, id, ObjectId(1), "ammo").unwrap();
    assign_battle_pilot(&mut scripts.world_mut(), id, ObjectId(1)).unwrap();
    support::seed_object_dice(
        &mut scripts.world_mut(),
        ObjectId(1),
        support::FIXTURE_DICE_SEED,
    );
    stop_battle_unit(
        &mut scripts.world_mut(),
        id,
        ObjectId(1),
        MovementRules::STANDARD.fall,
    )
    .unwrap();
    assert!(matches!(
        &advance_battle_self_destructs_action(&scripts, &config).unwrap()[..],
        [SelfDestructOutcome::Cancelled { .. }]
    ));
}

/// The cockpit countdown runs a full minute; loss of its final hazardous bin silently cancels it.
#[tokio::test]
async fn self_destruct_countdown_and_live_supply_cancellation() {
    for chassis in ["biped", "track"] {
        let (_dir, config, world, id) = fixture(chassis).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        self_destruct_action(&scripts, &config, id, ObjectId(1), "ammo").unwrap();
        let saved = scripts.world().clone();
        edit(&mut scripts.world_mut(), id, |state| {
            for amount in state["ammunition"].as_array_mut().unwrap() {
                *amount = 0.into();
            }
        });
        assert!(matches!(
            &advance_battle_self_destructs_action(&scripts, &config).unwrap()[..],
            [SelfDestructOutcome::Cancelled { .. }]
        ));
        *scripts.world_mut() = saved;
        for remaining in (1..SELF_DESTRUCT_DELAY).rev() {
            let reports = advance_battle_self_destructs_action(&scripts, &config).unwrap();
            assert!(
                matches!(
                    &reports[..],
                    [SelfDestructOutcome::Countdown { remaining: r, .. }] if *r == remaining
                ),
                "{reports:?}"
            );
        }
        let reports = advance_battle_self_destructs_action(&scripts, &config).unwrap();
        match (&reports[..], chassis) {
            ([SelfDestructOutcome::Ammunition { impacts, .. }], "biped") => {
                assert!(!impacts.is_empty())
            }
            ([SelfDestructOutcome::Vehicle { .. }], "track") => {}
            _ => panic!("{reports:?}"),
        }
        assert!(!self_destructs_pending(&scripts.world()));
        scripts.world().validate(&config).unwrap();
    }
}

/// Stored command order, corrupt-state rejection, and a later failed action preserve the entire tick.
#[tokio::test]
async fn self_destruct_order_and_failed_tick_replay() {
    let (_dir, config, mut world, first) = fixture("biped").await;
    let map = world.btech.constructed_units()[&first]
        .position()
        .unwrap()
        .map;
    let second = world.create(&config, "Second cockpit".into(), Kind::Thing);
    world.objects.get_mut(&second).unwrap().home = Some(ObjectId(config.home()));
    // Ammunition stored in the head makes the detonation lethal, forcing an evacuation.
    let template = include_str!("../game/units/JR7-D.toml")
        .replace(
            "    { at = 1, item = \"Ammo_IS.SRM-4\", rounds = 25 },\n",
            "",
        )
        .replace(
            "    { at = 4, item = \"HeatSink\" },",
            "    { at = 4, item = \"Ammo_IS.SRM-4\", rounds = 25 },",
        );
    create_battle_unit(
        &mut world,
        second,
        MechTemplate::parse("JR7-D", &template).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, second, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, second, map, 0, 0).unwrap();
    let pilot = ObjectId(2);
    world.objects.get_mut(&pilot).unwrap().location = Some(second);
    assign_battle_pilot(&mut world, second, pilot).unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, second, pilot, true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    // Reverse identifier order; cancellation outcomes make order observable without blast damage.
    self_destruct_action(&scripts, &config, second, pilot, "ammo override").unwrap();
    self_destruct_action(&scripts, &config, first, ObjectId(1), "ammo override").unwrap();
    let pending = scripts.world().clone();
    let mut corrupt = serde_json::to_value(&pending.btech).unwrap();
    corrupt["constructed"][first.0.to_string()]["self_destruct"]["order"] = 0.into();
    let mut invalid = pending.clone();
    invalid.btech = serde_json::from_value(corrupt).unwrap();
    assert!(invalid.validate(&config).is_err());
    for remaining in [0, SELF_DESTRUCT_DELAY + 1] {
        let mut invalid = pending.clone();
        edit(&mut invalid, first, |s| {
            s["self_destruct"]["remaining"] = remaining.into()
        });
        assert!(invalid.validate(&config).is_err());
    }
    for id in [first, second] {
        edit(&mut scripts.world_mut(), id, |s| {
            s["power"] = serde_json::to_value(Power::Off).unwrap()
        });
    }
    let reports = advance_battle_self_destructs_action(&scripts, &config).unwrap();
    assert_eq!(
        reports,
        vec![
            SelfDestructOutcome::Cancelled { unit: second },
            SelfDestructOutcome::Cancelled { unit: first }
        ]
    );
    *scripts.world_mut() = pending;
    // The head detonation is lethal; its casualty callback fails after mutations.
    scripts
        .world_mut()
        .objects
        .get_mut(&second)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    scripts
        .world_mut()
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    for remaining in [2, 1] {
        let reports = advance_battle_self_destructs_action(&scripts, &config).unwrap();
        assert_eq!(
            reports,
            vec![
                SelfDestructOutcome::Countdown {
                    unit: second,
                    remaining
                },
                SelfDestructOutcome::Countdown {
                    unit: first,
                    remaining
                }
            ]
        );
    }
    let before = scripts.world().clone();
    let parents: mlua::Table = scripts
        .inspect_lua()
        .named_registry_value("mux.parents")
        .unwrap();
    let parent: mlua::Table = parents
        .get(scripts.world().objects[&second].lua_parent.as_str())
        .unwrap();
    let previous: mlua::Value = parent.get("events").unwrap();
    let events = scripts.inspect_lua().create_table().unwrap();
    events
        .set(
            "on_leave",
            scripts
                .inspect_lua()
                .create_function(|_, _: mlua::Value| -> mlua::Result<()> {
                    Err(mlua::Error::external("self-destruct departure failure"))
                })
                .unwrap(),
        )
        .unwrap();
    parent.set("events", events).unwrap();
    assert!(advance_battle_self_destructs_action(&scripts, &config).is_err());
    assert_eq!(scripts.world().btech, before.btech);
    assert_eq!(scripts.world().objects[&pilot].location, Some(second));
    parent.set("events", previous).unwrap();
    let reports = advance_battle_self_destructs_action(&scripts, &config).unwrap();
    assert!(matches!(
        &reports[..],
        [
            SelfDestructOutcome::Ammunition { unit: a, .. },
            SelfDestructOutcome::Ammunition { unit: b, .. }
        ] if *a == second && *b == first
    ));
    assert_eq!(
        scripts.world().btech.constructed_units()[&second].sections()[&MechSection::Head].internal,
        0
    );
    assert_ne!(scripts.world().objects[&pilot].location, Some(second));
    assert!(!self_destructs_pending(&scripts.world()));
}

/// A failed database commit cannot consume a countdown or publish an early detonation after restart.
#[tokio::test(flavor = "current_thread")]
async fn self_destruct_server_restart_retries_failed_commit() {
    use sqlx::Connection;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir,config,mut world,id)=fixture("biped").await;
        world.accounts.get_mut(&ObjectId(1)).unwrap().hash=Some(accounts::hash("secret",&config).unwrap());
        let scripts=Scripts::new(&config,Rc::new(RefCell::new(world))).unwrap();
        self_destruct_action(&scripts,&config,id,ObjectId(1),"ammo override").unwrap();
        let saved=scripts.world().clone();
        persistence::save(&config.database(),&saved).await.unwrap();
        let (address,shutdown,task,_lua,mut heartbeats)=support::start(&config,Rc::new(std::cell::Cell::new(1))).await;
        let mut client=support::Client {socket:tokio::net::TcpStream::connect(address).await.unwrap(),pending:Vec::new()};
        client.until("Who are you? ").await;
        client.send("#1").await;
        client.until("Password: ").await;
        client.send("secret").await;
        client.until("biped").await;
        let mut sql=sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("CREATE TRIGGER deny_self_destruct BEFORE UPDATE ON btech_units BEGIN SELECT RAISE(ABORT,'self-destruct commit failure'); END").execute(&mut sql).await.unwrap();
        heartbeats.attempt().await;
        let persisted=persistence::load(&config.database()).await.unwrap();
        assert_eq!(persisted.btech.constructed_units()[&id].self_destruct().unwrap().remaining,3);
        client.send("status").await;
        let output=client.until("Self-destruction: 3s remaining").await;
        assert!(!output.contains("Self-destruction in 2 second"));
        sqlx::query("DROP TRIGGER deny_self_destruct").execute(&mut sql).await.unwrap();
        let loaded=heartbeats.until_saved(&config,8,|loaded| loaded.btech.constructed_units()[&id].is_destroyed()).await;
        assert!(loaded.btech.constructed_units()[&id].self_destruct().is_none());
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

/// Pilot ownership applies to in-character crews; wizard and out-of-character occupants have exceptions.
#[tokio::test]
async fn self_destruct_character_ownership_and_wizard_exception() {
    let (_dir, config, mut world, id) = fixture("track").await;
    let pilot = ObjectId(2);
    release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&pilot).unwrap().location = Some(id);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(
        self_destruct_action(&scripts, &config, id, pilot, "ammo")
            .unwrap_err()
            .to_string()
            .contains("only the pilot")
    );
    assign_battle_pilot(&mut scripts.world_mut(), id, pilot).unwrap();
    self_destruct_action(&scripts, &config, id, pilot, "ammo").unwrap();
    assert!(
        self_destruct_action(&scripts, &config, id, pilot, "stop")
            .unwrap_err()
            .to_string()
            .contains("only the pilot")
    );
    self_destruct_action(&scripts, &config, id, ObjectId(1), "stop").unwrap();
    scripts
        .world_mut()
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .remove(Flag::InCharacter);
    self_destruct_action(&scripts, &config, id, pilot, "ammo").unwrap();
    self_destruct_action(&scripts, &config, id, pilot, "stop").unwrap();
    assert!(!self_destructs_pending(&scripts.world()));
}

/// A destroyed ground vehicle with no crew recovery must still descend on an idle server after reload.
#[tokio::test(flavor = "current_thread")]
async fn self_destruct_ground_wreck_descends_after_restart() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_dir, config, mut world, id) = fixture("track").await;
            edit(&mut world, id, |state| {
                state["ground_elevation"] = 20.into();
                state["pilot_injuries"] = 2.into();
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            self_destruct_action(&scripts, &config, id, ObjectId(1), "ammo override").unwrap();
            detonate(&scripts, &config);
            let saved = scripts.world().clone();
            let unit = &saved.btech.vehicles()[&id];
            assert!(unit.is_destroyed());
            assert_eq!(unit.crew_recovery().remaining, 0);
            assert_eq!(unit.free_fall(), Some(FreeFall::new(6)));
            persistence::save(&config.database(), &saved).await.unwrap();
            let (_, shutdown, task, _, mut heartbeats) =
                support::start(&config, Rc::new(std::cell::Cell::new(1))).await;
            let loaded = heartbeats
                .until_saved(&config, 30, |loaded| {
                    loaded.btech.vehicles()[&id].free_fall().is_none()
                })
                .await;
            assert_eq!(
                loaded.btech.vehicles()[&id].elevation_level(Hex::new(Terrain::Clear, 0)),
                0
            );
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}
