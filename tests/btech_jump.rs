//! Jump thrust, gravity, continuous trajectory boundaries and detached persisted inspection.
use crate::support;
use stompymux_rs::{
    BattleJumpPath, BattlePoint, BattleSection, BattleSystem, BattleTemplate, BattleUnit,
};

/// Loose stock uses the shared load calculation for both projected and targeted jumps.
#[tokio::test]
async fn cargo_jump_admission_matches_native_lua_and_restart_at_one_mp() {
    use stompymux_rs::*;
    for (cargo_tech, quantity, allowed) in [
        (true, 6, true),
        (true, 7, true),
        (true, 8, false),
        (false, 7, false),
    ] {
        let (_dir, config, mut world, id) = runtime_fixture().await;
        let (target, _) = jump_observer(&mut world, &config, id);
        refresh_battle_contacts(&mut world, &[id]).unwrap();
        select_battle_target(&mut world, id, ObjectId(1), Some(target)).unwrap();
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut encoded["constructed"][id.0.to_string()];
        // Fix material mass at nominal: seven half-ton crates then cost exactly one MP.
        unit["live_mass"] = serde_json::json!(35 * 1024);
        if cargo_tech {
            unit["definition"]["attributes"]["specials"] = "FlipArms CargoTech".into();
        }
        world.btech = serde_json::from_value(encoded).unwrap();
        set_battle_inventory_named(&mut world, ObjectId(1), id, "Cockpit", 0, quantity).unwrap();
        let loss = 118.25 - battle_effective_maximum_speed(&world, id, true).unwrap();
        assert_eq!(loss <= 10.75, allowed);
        if cargo_tech && quantity == 7 {
            assert_eq!(loss, 10.75);
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let replay = persistence::load(&config.database()).await.unwrap();
        assert_eq!(world.btech, replay.btech);
        for targeted in [false, true] {
            let native = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let lua = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(replay.clone())),
            )
            .unwrap();
            let output = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                if targeted { "jump" } else { "jump 0 1" },
            );
            let call = if targeted {
                format!("btech.unit.dfa({},1,nil)", id.0)
            } else {
                format!("btech.unit.jump({},1,0,1)", id.0)
            };
            let result = lua.eval_callback::<()>(&call);
            assert_eq!(result.is_ok(), allowed, "{call}: {result:?}");
            assert_eq!(native.world().btech, lua.world().btech);
            if allowed {
                assert!(
                    native.world().btech.constructed_units()[&id]
                        .flight()
                        .is_some()
                );
            } else {
                assert!(
                    output.contains("No, with this cargo you won't!"),
                    "{output}"
                );
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("No, with this cargo you won't!")
                );
                assert_eq!(native.world().btech, world.btech);
                assert!(lua.drain_outbox().is_empty());
            }
        }
    }
}

/// Running cockpit on a clear north/south corridor, with a hill beside that corridor for LOS checks.
async fn runtime_fixture() -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    stompymux_rs::ObjectId,
) {
    use stompymux_rs::*;
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Jump runtime field".into(), Kind::Room);
    let mut source = "12 12\n".to_owned();
    for y in 0..12 {
        for x in 0..12 {
            source.push_str(if (x, y) == (4, 4) { ".3" } else { ".0" });
        }
        source.push('\n');
    }
    create_battle_map(
        &mut world,
        map,
        "jump.map",
        BattleMapAsset::parse(&source).unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Jump Jenner".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 5, 5).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

/// Conventional fall rules used by jump landing and lost-thrust handling.
fn jump_rules() -> stompymux_rs::BattleFallRules {
    stompymux_rs::BattleFallRules {
        vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
        stacking: stompymux_rs::BattleStackingRules::STANDARD,
        stagger: stompymux_rs::BattleStaggerMode::Retain,
        hit: stompymux_rs::BattleHitRules {
            inferno_penalty: false,
            exile_stun_mode: 0,
        },
        extended_piloting: true,
        toughness: false,
    }
}

#[tokio::test]
async fn destruction_and_thermal_shutdown_cancel_airborne_state() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    let (observer, _witness) = jump_observer(&mut world, &config, id);
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    advance_battle_jumps(
        &mut world,
        stompymux_rs::BattleMovementRules {
            fall: stompymux_rs::BattleFallRules {
                stacking: stompymux_rs::BattleStackingRules::STANDARD,
                ..jump_rules()
            },
            ..stompymux_rs::BattleMovementRules::STANDARD
        },
    )
    .unwrap();
    let mut destroyed = world.clone();
    apply_damage_phase(
        &mut destroyed,
        id,
        BattleSection::CenterTorso,
        u16::MAX,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    let unit = &destroyed.btech.constructed_units()[&id];
    assert!(unit.is_destroyed());
    assert!(unit.flight().is_none());
    assert_eq!(unit.jump_stabilization(), 0);
    assert_eq!(unit.power(), BattlePower::Off);
    destroyed.validate(&config).unwrap();
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            for _ in 0..3 {
                dice.d6();
            } // Unskilled Computer check.
            dice.two_d6() >= 6 && dice.d6() == 1 && dice.two_d6() == 7
        })
        .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let unit = &mut state["constructed"][id.0.to_string()];
    unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    unit["heat"] = serde_json::json!({"stored":24.0,"excess":14.0});
    unit["overheat_clock"] = serde_json::json!({"elapsed":30,"phase":0,"injury_due":false});
    world.btech = serde_json::from_value(state).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    let rules = BattleOverheatRules {
        vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
        stacking: BattleStackingRules::STANDARD,
        hit: jump_rules().hit,
        extended_piloting: true,
        stagger: BattleStaggerMode::Retain,
    };
    let replay = advance_battle_overheat(&mut restored, rules).unwrap();
    let reports = advance_battle_overheat(
        &mut world,
        BattleOverheatRules {
            vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
            stacking: stompymux_rs::BattleStackingRules::STANDARD,
            hit: jump_rules().hit,
            extended_piloting: true,
            stagger: BattleStaggerMode::Retain,
        },
    )
    .unwrap();
    assert_eq!(reports, replay);
    assert_eq!(world.btech, restored.btech);
    let observed: Vec<_> = reports[0]
        .messages()
        .into_iter()
        .filter(|(recipient, _)| *recipient == observer)
        .collect();
    assert_eq!(observed.len(), 1);
    assert!(observed[0].1.ends_with("falls from the sky!"));
    assert!(reports[0].shutdown && reports[0].fall.is_some());
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.flight().is_none());
    assert_eq!(unit.power(), BattlePower::Off);
    assert_eq!(unit.posture(), BattlePosture::Prone);
    assert_eq!(unit.jump_stabilization(), 0);
    world.validate(&config).unwrap();
}

/// Connect a cockpit client at the scenario's custom location.
async fn jump_client(address: std::net::SocketAddr) -> support::Client {
    let mut client = support::Client {
        socket: tokio::net::TcpStream::connect(address).await.unwrap(),
        pending: Vec::new(),
    };
    client.until("Who are you? ").await;
    client.send("#1").await;
    client.until("Password: ").await;
    client.send("secret").await;
    client.until("Jump Jenner").await;
    client
}

/// Advance the ordinary server clock and wait for its committed unit update.
async fn jump_tick(
    config: &stompymux_rs::Config,
    id: stompymux_rs::ObjectId,
) -> stompymux_rs::World {
    use std::time::Duration;
    let before = stompymux_rs::persistence::load(&config.database())
        .await
        .unwrap()
        .btech
        .constructed_units()[&id]
        .clone();
    tokio::time::pause();
    tokio::time::advance(Duration::from_secs(1)).await;
    tokio::time::resume();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let world = stompymux_rs::persistence::load(&config.database())
                .await
                .unwrap();
            if world.btech.constructed_units()[&id] != before {
                return world;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .expect("jump heartbeat did not commit")
}

#[tokio::test]
async fn tcp_jump_retries_failed_launch_and_flight_saves_then_resumes_after_restart() {
    use sqlx::Connection;
    use stompymux_rs::*;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, world, id) = runtime_fixture().await;
        persistence::save(&config.database(), &world).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("UPDATE player_state SET password_hash=? WHERE object_dbref=1").bind(accounts::hash("secret", &config).unwrap()).execute(&mut sql).await.unwrap();
        let (address, shutdown, task, _lua) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        let mut client = jump_client(address).await;
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER reject_launch BEFORE UPDATE ON btech_units WHEN NEW.dbref={} AND json_extract(NEW.live,'$.flight.travelled')=0 BEGIN SELECT RAISE(ABORT,'launch failure'); END",id.0))).execute(&mut sql).await.unwrap();
        client.send("jump 0 2").await;
        let failure = client.until("Unable to save your changes.").await;
        assert!(!failure.contains("You engage your jump jets."), "{failure}");
        assert!(persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].flight().is_none());
        sqlx::query("DROP TRIGGER reject_launch").execute(&mut sql).await.unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER reject_flight BEFORE UPDATE ON btech_units WHEN NEW.dbref={} AND json_extract(NEW.live,'$.flight.travelled')>0 BEGIN SELECT RAISE(ABORT,'flight failure'); END",id.0))).execute(&mut sql).await.unwrap();
        client.send("jump 0 2").await;
        client.until("You engage your jump jets.").await;
        let launched = persistence::load(&config.database()).await.unwrap();
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        let failed = persistence::load(&config.database()).await.unwrap();
        assert_eq!(failed.btech.constructed_units()[&id], launched.btech.constructed_units()[&id]);
        sqlx::query("DROP TRIGGER reject_flight").execute(&mut sql).await.unwrap();
        for _ in 0..10 { jump_tick(&config, id).await; }
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
        let saved = persistence::load(&config.database()).await.unwrap();
        let cursor = saved.btech.constructed_units()[&id].flight().unwrap();
        assert!(cursor.travelled() > 0.0 && !cursor.arrived());
        // Hold the restored cursor while login runs; real heartbeat time can advance during I/O.
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER hold_resume BEFORE UPDATE ON btech_units WHEN NEW.dbref={} AND json_extract(NEW.live,'$.flight') IS NOT json_extract(OLD.live,'$.flight') BEGIN SELECT RAISE(ABORT,'resume held'); END",id.0))).execute(&mut sql).await.unwrap();
        let (address, shutdown, task, _lua) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        let mut client = jump_client(address).await;
        let mut world = persistence::load(&config.database()).await.unwrap();
        assert_eq!(world.btech.constructed_units()[&id].flight(), Some(cursor));
        sqlx::query("DROP TRIGGER hold_resume").execute(&mut sql).await.unwrap();
        for _ in 0..30 {
            if world.btech.constructed_units()[&id].flight().is_none() { break; }
            world = jump_tick(&config, id).await;
        }
        assert!(world.btech.constructed_units()[&id].flight().is_none());
        assert_eq!(world.btech.constructed_units()[&id].position().unwrap().y, 3);
        client.until("You finish your jump.").await;
        for _ in 0..12 { world = jump_tick(&config, id).await; }
        assert_eq!(world.btech.constructed_units()[&id].jump_stabilization(), 0);
        client.until("You have finally stabilized after your jump.").await;
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn connected_jump_domain_updates_height_heat_landing_and_stabilization_after_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let observer = world.create(&config, "Observer".into(), Kind::Thing);
    world.objects.get_mut(&observer).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        observer,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, observer, map, 3, 4).unwrap();
    set_battle_speed(&mut world, id, ObjectId(1), 32.25).unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    let launch = world.btech.clone();
    assert!(
        advance_battle_motion(
            &mut world,
            BattleMovementRules {
                fasa_turning: false,
                slowdown: 2,
                ..stompymux_rs::BattleMovementRules::STANDARD
            }
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(world.btech, launch);
    assert_eq!(
        world.btech.constructed_units()[&id]
            .heat_rates(&world)
            .production,
        5.0
    );
    assert_eq!(
        world.btech.constructed_units()[&id].attacker_movement_modifier(false),
        3
    );
    for _ in 0..12 {
        assert!(
            advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..jump_rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                }
            )
            .unwrap()
            .is_empty()
        );
    }
    let sample = world.btech.constructed_units()[&id]
        .flight()
        .unwrap()
        .sample();
    assert!((sample.elevation - 5.0).abs() < 1e-10);
    let range = battle_unit_range(&world, observer, id).unwrap();
    assert!((range.spatial.powi(2) - range.horizontal.powi(2) - 1.0).abs() < 1e-10);
    assert!(
        !battle_unit_terrain_los(&world, observer, id)
            .unwrap()
            .blocked
    );
    assert!(
        ground_terrain_los(
            &world.btech.maps()[&map],
            BattleHexCoordinate { x: 3, y: 4 },
            BattleHexCoordinate { x: 5, y: 4 }
        )
        .unwrap()
        .blocked
    );
    let before = world.btech.clone();
    assert!(set_battle_speed(&mut world, id, ObjectId(1), 1.0).is_err());
    assert!(set_battle_heading(&mut world, id, ObjectId(2), 90.0).is_err());
    assert!(launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).is_err());
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    for _ in 0..12 {
        assert_eq!(
            advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..jump_rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                }
            )
            .unwrap(),
            advance_battle_jumps(
                &mut loaded,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..jump_rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                }
            )
            .unwrap()
        );
    }
    assert_eq!(world.btech, loaded.btech);
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.flight().is_none());
    assert_eq!(unit.position().unwrap().y, 3);
    assert_eq!(unit.jump_stabilization(), 12);
    assert_eq!(unit.motion().unwrap().desired_speed, 32.25);
    let mut shutdown = world.clone();
    stop_battle_unit(
        &mut shutdown,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert_eq!(
        shutdown.btech.constructed_units()[&id].jump_stabilization(),
        0
    );
    assert!(
        advance_battle_jumps(
            &mut shutdown,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..jump_rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            }
        )
        .unwrap()
        .is_empty()
    );
    shutdown.validate(&config).unwrap();
    assert_eq!(unit.attacker_movement_modifier(false), 2);
    assert_eq!(unit.heat_rates(&world).production, 0.0);
    assert!(launch_battle_jump(&mut world, id, ObjectId(1), 180, 1.0).is_err());
    for remaining in (0..12).rev() {
        let notices = advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..jump_rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
        assert_eq!(
            world.btech.constructed_units()[&id].jump_stabilization(),
            remaining
        );
        assert_eq!(notices.len(), usize::from(remaining == 0));
    }
    assert_eq!(
        world.btech.constructed_units()[&id].attacker_movement_modifier(false),
        0
    );
    launch_battle_jump(&mut world, id, ObjectId(1), 180, 1.0).unwrap();
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn native_and_lua_launch_share_state_and_reject_routes_or_callbacks_atomically() {
    use stompymux_rs::*;
    let (_dir, config, base, id) = runtime_fixture().await;
    let mut base = base;
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][id.0.to_string()]["stagger"]["hits"] =
        serde_json::json!([{"damage":20,"remaining":60,"counted":false}]);
    base.btech = serde_json::from_value(state).unwrap();
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
    )
    .unwrap();
    for command in [
        "jump",
        "jump north two",
        "jump 0 99",
        "jump 0 0",
        "jump 0 -1",
    ] {
        let text = support::run_text(&native, &config, ObjectId(1), 1, command);
        assert!(!text.contains("You engage your jump jets."), "{text}");
        assert_eq!(native.world().btech, base.btech);
    }
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.jump({},1,0,2); error('abort flight')",
            id.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, base.btech);
    let text = support::run_text(&native, &config, ObjectId(1), 1, "jump 0 2");
    assert!(text.contains("You engage your jump jets."), "{text}");
    assert!(
        lua.eval_callback::<bool>(&format!("return btech.unit.jump({},1,0,2)", id.0))
            .unwrap()
    );
    assert_eq!(native.world().btech, lua.world().btech);
    let before = lua.world().btech.clone();
    lua.eval_callback::<()>(&format!("local s=btech.unit.state({}); assert(s.airborne and s.flight); s.flight.travelled=999; assert(not pcall(function() btech.unit.jump({},1,0,2) end))", id.0,id.0)).unwrap();
    assert_eq!(lua.world().btech, before);
}

#[tokio::test]
async fn losing_all_jets_ends_flight_with_a_fall_and_saved_cursor_corruption_is_rejected() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    advance_battle_jumps(
        &mut world,
        stompymux_rs::BattleMovementRules {
            fall: stompymux_rs::BattleFallRules {
                stacking: stompymux_rs::BattleStackingRules::STANDARD,
                ..jump_rules()
            },
            ..stompymux_rs::BattleMovementRules::STANDARD
        },
    )
    .unwrap();
    let mut bad = serde_json::to_value(&world.btech).unwrap();
    bad["constructed"][id.0.to_string()]["motion"]["point"]["x"] = 0.into();
    let mut corrupt = world.clone();
    corrupt.btech = serde_json::from_value(bad).unwrap();
    assert!(corrupt.validate(&config).is_err());
    let jets: Vec<_> = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .systems
        .into_iter()
        .filter(|part| part.system == BattleSystem::JumpJet)
        .map(|part| part.location)
        .collect();
    for location in jets {
        destroy_battle_critical(&mut world, id, location).unwrap();
    }
    let point = world.btech.constructed_units()[&id].motion().unwrap().point;
    let notices = advance_battle_jumps(
        &mut world,
        stompymux_rs::BattleMovementRules {
            fall: stompymux_rs::BattleFallRules {
                stacking: stompymux_rs::BattleStackingRules::STANDARD,
                ..jump_rules()
            },
            ..stompymux_rs::BattleMovementRules::STANDARD
        },
    )
    .unwrap();
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.flight().is_none());
    assert_eq!(unit.posture(), BattlePosture::Prone);
    assert_eq!(unit.motion().unwrap().point, point);
    assert!(
        notices
            .iter()
            .any(|notice| notice.text.contains("lose control"))
    );
    world.validate(&config).unwrap();
}

/// A five-hex path with enough height variation to detect lost trajectory state.
fn flight_path() -> BattleJumpPath {
    BattleJumpPath::new(
        BattlePoint { x: 2.0, y: 3.0 },
        BattlePoint { x: 2.0, y: 8.0 },
        -1,
        4,
        5,
    )
    .unwrap()
}

#[tokio::test]
async fn projected_jump_checks_requested_range_before_destination_snapping() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    let start = world.btech.constructed_units()[&id].motion().unwrap().point;
    let short = BattleJumpPath::projected(start, 90, 3.0, 0, 0, 5).unwrap();
    assert!(short.distance() > 3.0);
    assert_eq!(short.apex(), 5);
    launch_battle_jump(&mut world, id, ObjectId(1), 90, 5.0).unwrap();
    let flight = world.btech.constructed_units()[&id].flight().unwrap();
    assert!(flight.path().distance() > 5.0);
    assert!(
        BattleJumpPath::new(start, flight.path().sample(1.0, 5).unwrap().point, 0, 0, 5).is_err()
    );
    let restored: BattleJumpFlight =
        serde_json::from_str(&serde_json::to_string(&flight).unwrap()).unwrap();
    assert_eq!(restored, flight);
    let mut bad = serde_json::to_value(flight).unwrap();
    bad["path"]["end"]["y"] = 0.into();
    assert!(serde_json::from_value::<BattleJumpFlight>(bad).is_err());
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    for _ in 0..63 {
        advance_battle_jumps(
            &mut loaded,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..jump_rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
    }
    assert!(loaded.btech.constructed_units()[&id].flight().is_none());
    assert_eq!(
        loaded.btech.constructed_units()[&id].position().unwrap().x,
        11
    );
    loaded.validate(&config).unwrap();
}

#[test]
fn timed_flight_replays_after_serialization_and_snaps_to_the_endpoint() {
    use stompymux_rs::{BattleJumpFlight, BattleJumpOutcome};
    for (gravity, modifier, seconds) in [
        (100, 100, 60),
        (100, 0, 60),
        (100, -50, 60),
        (100, 50, 120),
        (100, 200, 30),
        (50, 100, 30),
        (200, 100, 120),
    ] {
        let capacity = jenner().jump_capacity(gravity).unwrap();
        let mut flight = BattleJumpFlight::new(flight_path());
        let mut restored = flight;
        for second in 1..=seconds {
            let previous = flight.sample();
            let step = flight.advance(capacity, modifier).unwrap();
            assert_eq!(step.from, previous);
            assert_eq!(step, restored.advance(capacity, modifier).unwrap());
            assert_eq!(
                step.outcome,
                if second == seconds {
                    BattleJumpOutcome::Landing
                } else {
                    BattleJumpOutcome::Airborne
                }
            );
            assert_eq!(step.to, flight.sample());
            // Exercise both a mid-flight checkpoint and a completed flight checkpoint.
            if second == 13 || second == seconds {
                restored = serde_json::from_str(&serde_json::to_string(&flight).unwrap()).unwrap();
                assert_eq!(restored, flight);
            }
        }
        assert!(flight.arrived());
        assert_eq!(flight.travelled(), 5.0);
        assert_eq!(flight.sample().point, BattlePoint { x: 2.0, y: 8.0 });
        assert_eq!(flight.sample().elevation, 4.0);
        let before = flight;
        assert!(flight.advance(capacity, modifier).is_err());
        assert_eq!(flight, before);
    }
}

#[test]
fn flight_uses_current_thrust_without_rewriting_the_previous_airborne_sample() {
    use stompymux_rs::{BattleJumpCapacity, BattleJumpFlight, BattleJumpOutcome};
    let mut flight = BattleJumpFlight::new(flight_path());
    let full = jenner().jump_capacity(100).unwrap();
    for _ in 0..15 {
        let _ = flight.advance(full, 100).unwrap();
    }
    let previous = flight.sample();
    let damaged = BattleJumpCapacity {
        speed: 32.25,
        movement_points: 3,
    };
    let before = flight;
    // A damaged jet changes the next integration step, not the last committed point.
    assert_eq!(flight.sample(), previous);
    let step = flight.advance(damaged, 100).unwrap();
    assert_eq!(step.from, previous);
    assert!((flight.travelled() - before.travelled() - 0.05).abs() < 1e-12);
    assert_eq!(
        step.to,
        flight.path().sample(flight.travelled() / 5.0, 3).unwrap()
    );
    let restored: BattleJumpFlight =
        serde_json::from_str(&serde_json::to_string(&flight).unwrap()).unwrap();
    assert_eq!(restored.sample(), flight.sample());
    assert_eq!(restored, flight);
    let before = flight;
    let lost = flight
        .advance(
            BattleJumpCapacity {
                speed: 0.0,
                movement_points: 0,
            },
            100,
        )
        .unwrap();
    assert_eq!(lost.outcome, BattleJumpOutcome::LostThrust);
    assert_eq!(lost.from, lost.to);
    assert_eq!(lost.from, before.sample());
    assert_eq!(flight, before);
    for bad in [
        BattleJumpCapacity {
            speed: -1.0,
            movement_points: 0,
        },
        BattleJumpCapacity {
            speed: f64::NAN,
            movement_points: 0,
        },
        BattleJumpCapacity {
            speed: f64::INFINITY,
            movement_points: 5,
        },
        BattleJumpCapacity {
            speed: 53.75,
            movement_points: 4,
        },
    ] {
        assert!(flight.advance(bad, 100).is_err());
        assert_eq!(flight, before);
    }
    let step = flight.advance(full, i64::MAX).unwrap();
    assert_eq!(step.outcome, BattleJumpOutcome::Landing);
    assert_eq!(step.to.point, BattlePoint { x: 2.0, y: 8.0 });
}

#[test]
fn saved_flight_rejects_corrupt_progress_and_reconstructs_launch_geometry() {
    use stompymux_rs::BattleJumpFlight;
    let flight = BattleJumpFlight::new(flight_path());
    let state = serde_json::to_value(flight).unwrap();
    assert!(state["path"].get("distance").is_none());
    assert!(state["path"].get("apex").is_none());
    for travelled in [-1.0, 5.0001, f64::INFINITY] {
        let mut corrupt = state.clone();
        corrupt["travelled"] = serde_json::json!(travelled);
        assert!(serde_json::from_value::<BattleJumpFlight>(corrupt).is_err());
    }
    for (key, value) in [
        ("movement_points", serde_json::json!(4)),
        ("end_elevation", serde_json::json!(5)),
        ("start_elevation", serde_json::json!(0.5)),
        ("apex", serde_json::json!(99)),
    ] {
        let mut corrupt = state.clone();
        corrupt["path"][key] = value;
        assert!(
            serde_json::from_value::<BattleJumpFlight>(corrupt).is_err(),
            "{key}"
        );
    }
    let mut corrupt = state;
    corrupt["path"]["end"] = corrupt["path"]["start"].clone();
    assert!(serde_json::from_value::<BattleJumpFlight>(corrupt).is_err());
}

/// Intact Jenner with five conventional jump jets.
fn jenner() -> BattleUnit {
    BattleUnit::from_template(
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap()
}

#[test]
fn gravity_and_effective_jet_losses_bound_jump_capacity() {
    let mut unit = jenner();
    for (gravity, points) in [
        (0, 10),
        (49, 10),
        (50, 10),
        (100, 5),
        (101, 4),
        (200, 2),
        (255, 1),
    ] {
        let capacity = unit.jump_capacity(gravity).unwrap();
        assert_eq!(capacity.movement_points, points);
        assert_eq!(capacity.speed, 53.75 * 100.0 / gravity.max(50) as f64);
    }
    assert!(unit.jump_capacity(-1).is_err());
    assert!(unit.jump_capacity(256).is_err());
    let jets: Vec<_> = unit
        .loadout()
        .unwrap()
        .systems
        .into_iter()
        .filter(|part| part.system == BattleSystem::JumpJet)
        .map(|part| part.location)
        .collect();
    for (lost, location) in jets.into_iter().enumerate() {
        unit.destroy_critical(location).unwrap();
        assert_eq!(
            unit.jump_capacity(100).unwrap().movement_points,
            4 - lost as u16
        );
        unit.destroy_critical(location).unwrap();
        assert_eq!(
            unit.jump_capacity(100).unwrap().movement_points,
            4 - lost as u16
        );
    }
    let atlas = BattleUnit::from_template(
        BattleTemplate::parse("AS7-D", include_str!("fixtures/btech/mechs/AS7-D.toml")).unwrap(),
    )
    .unwrap();
    assert_eq!(atlas.jump_capacity(50).unwrap().speed, 0.0);
    let mut unit = jenner();
    unit.damage_phase(
        BattleSection::CenterTorso,
        u16::MAX,
        stompymux_rs::BattleDamagePhase::Internal,
    );
    assert_eq!(unit.jump_capacity(50).unwrap().speed, 0.0);
}

#[test]
fn trajectory_preserves_endpoints_apex_and_lost_thrust_curve() {
    let start = BattlePoint { x: 2.0, y: 3.0 };
    let end = BattlePoint { x: 2.0, y: 8.0 };
    for (from, to) in [(0, 0), (-1, 4), (5, 0)] {
        let path = BattleJumpPath::new(start, end, from, to, 5).unwrap();
        assert_eq!(path.distance(), 5.0);
        assert_eq!(path.apex(), 4);
        assert_eq!(path.sample(0.0, 5).unwrap().point, start);
        assert_eq!(path.sample(1.0, 5).unwrap().point, end);
        assert_eq!(path.sample(0.0, 5).unwrap().elevation, f64::from(from));
        assert_eq!(path.sample(1.0, 5).unwrap().elevation, f64::from(to));
        assert_eq!(
            path.sample(0.5, 5).unwrap().elevation,
            f64::from(from + to) / 2.0 + 4.0
        );
        let baseline = f64::from(from) * 0.75 + f64::from(to) * 0.25;
        assert_eq!(path.sample(0.25, 5).unwrap().elevation, baseline + 3.75);
        assert_eq!(path.sample(0.25, 3).unwrap().elevation, baseline + 3.0);
        for progress in [-0.1, 1.1, f64::NAN, f64::INFINITY] {
            assert!(path.sample(progress, 5).is_err());
        }
    }
    assert!(BattleJumpPath::new(start, end, 0, 6, 5).is_err());
    assert!(BattleJumpPath::new(start, end, 6, 0, 5).is_err());
    assert!(BattleJumpPath::new(start, end, 0, 0, 4).is_err());
    assert!(BattleJumpPath::new(start, start, 0, 0, 5).is_err());
    assert!(BattleJumpPath::new(start, end, 0, 0, 0).is_err());
    assert!(
        BattleJumpPath::new(
            start,
            BattlePoint {
                x: f64::NAN,
                y: 0.0
            },
            0,
            0,
            5
        )
        .is_err()
    );
    let short = BattleJumpPath::new(start, BattlePoint { x: 2.0, y: 4.0 }, 0, 0, 8).unwrap();
    assert_eq!(short.apex(), 4); // Short jumps use the twice-range-plus-two cap.
}

#[tokio::test]
async fn flooded_capacity_and_lua_inspection_survive_restart_without_mutation() {
    use stompymux_rs::{
        BattleMapAsset, Kind, ObjectId, Scripts, create_battle_map, create_battle_unit,
        persistence, place_battle_unit,
    };
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Jump field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "jump.map",
        BattleMapAsset::parse("2 2\n.0.0\n.0.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Jump Jenner".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    // Flooding a previously damaged jet must not subtract its thrust a second time.
    stompymux_rs::destroy_battle_critical(
        &mut world,
        id,
        stompymux_rs::CriticalLocation {
            section: BattleSection::LeftTorso,
            slot: 0,
        },
    )
    .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["gravity"] = 200.into();
    state["maps"][map.0.to_string()]["flags"] = 0.into();
    state["constructed"][id.0.to_string()]["flooded_sections"] = serde_json::json!(["LeftTorso"]);
    world.btech = serde_json::from_value(state).unwrap();
    world.validate(&config).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id]
            .jump_capacity(200)
            .unwrap()
            .movement_points,
        1
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(loaded))).unwrap();
    let values: (f64, u16) = scripts.eval_callback(&format!("local c=btech.unit.state({}).jump_capacity; local speed=c.speed; c.speed=999; return speed,btech.unit.state({}).jump_capacity.movement_points", id.0, id.0)).unwrap();
    assert_eq!(values, (16.125, 1));
    assert_eq!(scripts.world().btech, world.btech);
}

/// Seed exactly one through-armor critical at the requested installed slot.
fn seed_airborne_critical(
    world: &mut stompymux_rs::World,
    id: stompymux_rs::ObjectId,
    location: stompymux_rs::CriticalLocation,
) -> stompymux_rs::BattleDice {
    use stompymux_rs::*;
    let candidates = world.btech.constructed_units()[&id].critical_candidates(location.section);
    let selected = candidates
        .iter()
        .position(|slot| *slot == location)
        .unwrap() as u16
        + 1;
    let count = candidates.len() as u16;
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            dice.two_d6(); // Material entry precedes the through-armor critical check.
            matches!(dice.two_d6(), 8 | 9) && dice.die(count).unwrap() == selected
        })
        .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let mut expected = BattleDice::seeded([seed; 32]);
    expected.two_d6(); // Material entry.
    expected.two_d6();
    expected.die(count).unwrap();
    expected
}

/// A small selected impact isolates critical consequences from hit-location selection.
fn airborne_hit(section: BattleSection) -> stompymux_rs::BattleHit {
    stompymux_rs::BattleHit {
        section,
        rear_armor: false,
        through_armor_critical: true,
        crew_stun: false,
    }
}

#[tokio::test]
async fn airborne_actuator_damage_defers_balance_and_last_jet_falls_immediately() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id) = runtime_fixture().await;
    let (observer, _witness) = jump_observer(&mut base, &config, id);
    launch_battle_jump(&mut base, id, ObjectId(1), 0, 2.0).unwrap();
    advance_battle_jumps(
        &mut base,
        stompymux_rs::BattleMovementRules {
            fall: stompymux_rs::BattleFallRules {
                stacking: stompymux_rs::BattleStackingRules::STANDARD,
                ..jump_rules()
            },
            ..stompymux_rs::BattleMovementRules::STANDARD
        },
    )
    .unwrap();
    for slot in 0..4 {
        let mut world = base.clone();
        let expected = seed_airborne_critical(
            &mut world,
            id,
            CriticalLocation {
                section: BattleSection::LeftLeg,
                slot,
            },
        );
        let flight = world.btech.constructed_units()[&id].flight();
        let report = resolve_battle_tactical_impact(
            &mut world,
            id,
            airborne_hit(BattleSection::LeftLeg),
            1,
            jump_rules(),
        )
        .unwrap();
        assert!(report.balance.is_empty());
        let observed: Vec<_> = report
            .notices
            .iter()
            .filter(|notice| notice.unit == observer)
            .collect();
        assert_eq!(observed.len(), 1);
        assert!(observed[0].text.ends_with(if slot == 0 {
            "'s hip locks into place!"
        } else {
            "'s Left Leg twists in an odd way!"
        }));
        assert_eq!(world.btech.constructed_units()[&id].flight(), flight);
        assert_eq!(
            serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
            serde_json::to_value(expected).unwrap()
        );
        world.validate(&config).unwrap();
    }
    for last in [false, true] {
        let mut world = base.clone();
        let location = CriticalLocation {
            section: BattleSection::LeftTorso,
            slot: 0,
        };
        if last {
            let jets: Vec<_> = world.btech.constructed_units()[&id]
                .loadout()
                .unwrap()
                .systems
                .into_iter()
                .filter(|part| part.system == BattleSystem::JumpJet && part.location != location)
                .map(|part| part.location)
                .collect();
            for jet in jets {
                destroy_battle_critical(&mut world, id, jet).unwrap();
            }
        }
        seed_airborne_critical(&mut world, id, location);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        if world.objects[&ObjectId(1)].flags.contains(Flag::Connected) {
            restored
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        let report = resolve_battle_tactical_impact(
            &mut world,
            id,
            airborne_hit(location.section),
            1,
            jump_rules(),
        )
        .unwrap();
        assert_eq!(
            resolve_battle_tactical_impact(
                &mut restored,
                id,
                airborne_hit(location.section),
                1,
                jump_rules()
            )
            .unwrap(),
            report
        );
        assert_eq!(world.btech, restored.btech);
        let observed: Vec<_> = report
            .notices
            .iter()
            .filter(|notice| notice.unit == observer)
            .collect();
        assert_eq!(observed.len(), 1 + usize::from(last));
        assert!(
            observed[0]
                .text
                .ends_with("'s Left Torso flares as superheated plasma spews out!")
        );
        if last {
            assert!(observed[1].text.ends_with("falls from the sky!"));
        }

        assert!(
            report
                .notices
                .iter()
                .any(|notice| notice.text == "One of your jump jet engines has shut down!")
        );
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.flight().is_none(), last);
        if last {
            assert_eq!(report.balance.len(), 1);
            assert!(report.balance[0].check.is_none());
            assert_eq!(report.balance[0].fall.as_ref().unwrap().damage, 4);
            assert_eq!(unit.posture(), BattlePosture::Prone);
            assert!(
                report.notices.iter().any(
                    |notice| notice.text == "Losing your last jump jet, you fall from the sky!"
                )
            );
        } else {
            assert!(report.balance.is_empty());
        }
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn airborne_gyro_falls_use_gravity_adjusted_capacity_and_replay_after_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id) = runtime_fixture().await;
    let (observer, _witness) = jump_observer(&mut base, &config, id);
    // A connected pilot without a skill profile cannot pass the damaged-gyro check.
    base.objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    for (gravity, damage) in [(100, 20), (200, 8)] {
        let mut world = base.clone();
        let map = world.btech.constructed_units()[&id].position().unwrap().map;
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["maps"][map.0.to_string()]["gravity"] = gravity.into();
        world.btech = serde_json::from_value(state).unwrap();
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
        advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..jump_rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
        seed_airborne_critical(
            &mut world,
            id,
            CriticalLocation {
                section: BattleSection::CenterTorso,
                slot: 3,
            },
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restarted = persistence::load(&config.database()).await.unwrap();
        // Reconnect the same pilot before replaying a player-visible combat action.
        restarted
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let report = resolve_battle_tactical_impact(
            &mut world,
            id,
            airborne_hit(BattleSection::CenterTorso),
            1,
            jump_rules(),
        )
        .unwrap();
        let observed: Vec<_> = report
            .notices
            .iter()
            .filter(|notice| notice.unit == observer)
            .collect();
        assert_eq!(observed.len(), 2);
        assert!(
            observed[0]
                .text
                .ends_with("emits a loud screech as its gyro buckles under the impact!")
        );
        assert!(observed[1].text.ends_with("falls from the sky!"));

        let replay = resolve_battle_tactical_impact(
            &mut restarted,
            id,
            airborne_hit(BattleSection::CenterTorso),
            1,
            jump_rules(),
        )
        .unwrap();
        assert_eq!(report, replay);
        assert_eq!(world.btech, restarted.btech);
        let balance = &report.balance[0];
        assert!(!balance.check.unwrap().success);
        assert_eq!(balance.fall.as_ref().unwrap().damage, damage);
        assert!(world.btech.constructed_units()[&id].flight().is_none());
        assert!(
            report
                .notices
                .iter()
                .any(|notice| notice.text == "You fall from the sky!")
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn airborne_target_modifier_uses_current_thrust_and_gravity_without_consuming_dice() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let observer = world.create(&config, "Aim observer".into(), Kind::Thing);
    world.objects.get_mut(&observer).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        observer,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, observer, map, 5, 8).unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    for (gravity, modifier) in [(50, 5), (100, 3), (200, 2)] {
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["maps"][map.0.to_string()]["gravity"] = gravity.into();
        world.btech = serde_json::from_value(state).unwrap();
        let before = world.btech.clone();
        let aim = battle_aim_modifiers(
            &world,
            observer,
            id,
            0,
            4,
            BattleAimRules {
                woods_damage: false,
                dig_bonus: 3,
                dig_only_front: false,
                hit_arc_mode: 0,
                fasa_turning: false,
                extended_movement: false,
                extended_ranges: false,
                hotload_half_minimum: false,
                override_weapon_arcs: false,
            },
        )
        .unwrap();
        assert_eq!(aim.target_movement, modifier);
        assert_eq!(
            world.btech.constructed_units()[&id].motion().unwrap().speed,
            0.0
        );
        assert_eq!(world.btech, before);
    }
}

#[tokio::test]
async fn traditional_airborne_stagger_consumes_damage_without_a_piloting_roll() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["stagger"]["turn_damage"] = 20.into();
    world.btech = serde_json::from_value(state).unwrap();
    let before = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap();
    let reports = advance_battle_stagger(
        &mut world,
        BattleStaggerRules {
            vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
            mode: BattleStaggerMode::Traditional,
            interval: 5,
            tonnage: true,
            hit: jump_rules().hit,
            extended_piloting: true,
        },
    )
    .unwrap();
    assert!(reports.is_empty());
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.stagger().turn_damage, 0);
    assert_eq!(unit.stagger().checked_phase, Some(1));
    let after = serde_json::to_value(unit).unwrap();
    assert_eq!(before["dice"], after["dice"]);
    assert_eq!(before["flight"], after["flight"]);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn second_airborne_gyro_forces_a_fall_even_with_zero_whole_jump_points() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    set_battle_character(
        &mut world,
        ObjectId(1),
        BattleCharacter {
            build: 5,
            reflexes: 4,
            intuition: 3,
            learn: 2,
            charisma: 1,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Piloting-Biped",
        BattleCharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    seed_airborne_critical(
        &mut world,
        id,
        CriticalLocation {
            section: BattleSection::CenterTorso,
            slot: 3,
        },
    );
    let first = resolve_battle_tactical_impact(
        &mut world,
        id,
        airborne_hit(BattleSection::CenterTorso),
        1,
        jump_rules(),
    )
    .unwrap();
    assert!(first.balance[0].check.unwrap().success);
    assert!(first.balance[0].fall.is_none());
    assert!(world.btech.constructed_units()[&id].flight().is_some());
    // One surviving jet still provides thrust, but gravity reduces it below a whole MP.
    let jets: Vec<_> = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .systems
        .into_iter()
        .filter(|part| part.system == BattleSystem::JumpJet)
        .take(4)
        .map(|part| part.location)
        .collect();
    for jet in jets {
        destroy_battle_critical(&mut world, id, jet).unwrap();
    }
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["gravity"] = 200.into();
    world.btech = serde_json::from_value(state).unwrap();
    seed_airborne_critical(
        &mut world,
        id,
        CriticalLocation {
            section: BattleSection::CenterTorso,
            slot: 4,
        },
    );
    let second = resolve_battle_tactical_impact(
        &mut world,
        id,
        airborne_hit(BattleSection::CenterTorso),
        1,
        jump_rules(),
    )
    .unwrap();
    assert!(second.balance[0].check.is_none());
    let fall = second.balance[0].fall.as_ref().unwrap();
    assert_eq!(fall.damage, 0);
    assert!(fall.groups.is_empty());
    assert!(fall.avoidance.unwrap().roll.is_some());
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.posture(), BattlePosture::Prone);
    assert!(unit.flight().is_none());
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn airborne_support_loss_preserves_thrust_and_prone_landing_after_restart() {
    use stompymux_rs::*;
    let (_dir, config, base, id) = runtime_fixture().await;
    for hips in [false, true] {
        let mut world = base.clone();
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
        advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..jump_rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
        let flight = world.btech.constructed_units()[&id].flight();
        for (index, section) in [BattleSection::LeftLeg, BattleSection::RightLeg]
            .into_iter()
            .enumerate()
        {
            let (hit, damage) = if hips {
                seed_airborne_critical(&mut world, id, CriticalLocation { section, slot: 0 });
                (airborne_hit(section), 1)
            } else {
                let mut hit = airborne_hit(section);
                hit.through_armor_critical = false;
                (hit, 14) // Jenner leg: six armor plus eight internal, without transfer.
            };
            let report =
                resolve_battle_tactical_impact(&mut world, id, hit, damage, jump_rules()).unwrap();
            assert!(report.balance.is_empty());
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(unit.flight(), flight);
            assert_eq!(
                unit.posture(),
                if index == 0 {
                    BattlePosture::Standing
                } else {
                    BattlePosture::Prone
                }
            );
            world.validate(&config).unwrap();
        }
        assert!(
            battle_stand_target(&world, id, ObjectId(1), true)
                .unwrap_err()
                .to_string()
                .contains("Land before")
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restarted = persistence::load(&config.database()).await.unwrap();
        let mut notices = Vec::new();
        for _ in 0..23 {
            let expected = advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..jump_rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                },
            )
            .unwrap();
            let actual = advance_battle_jumps(
                &mut restarted,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..jump_rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                },
            )
            .unwrap();
            assert_eq!(expected, actual);
            notices.extend(actual);
        }
        assert_eq!(world.btech, restarted.btech);
        let unit = &world.btech.constructed_units()[&id];
        assert!(unit.flight().is_none());
        assert_eq!(unit.posture(), BattlePosture::Prone);
        assert_eq!(unit.jump_stabilization(), 12);
        assert!(
            notices
                .iter()
                .any(|notice| notice.text.contains("harder to land"))
        );
        assert!(
            !notices
                .iter()
                .any(|notice| notice.text.contains("caused you to fall"))
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn last_jet_loss_still_ends_a_structurally_collapsed_jump() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    for section in [BattleSection::LeftLeg, BattleSection::RightLeg] {
        seed_airborne_critical(&mut world, id, CriticalLocation { section, slot: 0 });
        let _report =
            resolve_battle_tactical_impact(&mut world, id, airborne_hit(section), 1, jump_rules())
                .unwrap();
    }
    let location = CriticalLocation {
        section: BattleSection::LeftTorso,
        slot: 0,
    };
    let jets: Vec<_> = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .systems
        .into_iter()
        .filter(|part| part.system == BattleSystem::JumpJet && part.location != location)
        .map(|part| part.location)
        .collect();
    for jet in jets {
        destroy_battle_critical(&mut world, id, jet).unwrap();
    }
    seed_airborne_critical(&mut world, id, location);
    let report = resolve_battle_tactical_impact(
        &mut world,
        id,
        airborne_hit(location.section),
        1,
        jump_rules(),
    )
    .unwrap();
    assert_eq!(report.balance[0].fall.as_ref().unwrap().damage, 4);
    assert!(world.btech.constructed_units()[&id].flight().is_none());
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn airborne_fire_uses_shared_native_lua_transactions_and_saved_trajectories() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id) = runtime_fixture().await;
    let map = base.btech.constructed_units()[&id].position().unwrap().map;
    let target = base.create(&config, "Airborne opponent".into(), Kind::Thing);
    base.objects.get_mut(&target).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut base,
        target,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut base, target, map, 5, 3).unwrap();
    base.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
    assign_battle_pilot(&mut base, target, ObjectId(2)).unwrap();
    start_battle_unit(&mut base, target, ObjectId(2), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut base, 0);
    }
    base.objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    set_battle_character(
        &mut base,
        ObjectId(1),
        BattleCharacter {
            build: 5,
            reflexes: 4,
            intuition: 3,
            learn: 2,
            charisma: 1,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    set_battle_character_value(
        &mut base,
        ObjectId(1),
        "Gunnery-Laser",
        BattleCharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    for (shooter_jumps, target_jumps) in [(true, false), (false, true), (true, true)] {
        let mut world = base.clone();
        if shooter_jumps {
            launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap();
        }
        if target_jumps {
            launch_battle_jump(&mut world, target, ObjectId(2), 0, 1.0).unwrap();
        }
        for _ in 0..6 {
            advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..jump_rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                },
            )
            .unwrap();
        }
        refresh_battle_contacts(&mut world, &[id]).unwrap();
        assert!(
            world.btech.constructed_units()[&id]
                .contacts()
                .contains_key(&target)
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restarted = persistence::load(&config.database()).await.unwrap();
        restarted
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(restarted)),
        )
        .unwrap();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.fire({},1,0,{}); error('abort airborne shot')",
                id.0, target.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire 0 #{}", target.0),
        );
        assert_ne!(native.world().btech, world.btech, "{text}");
        let (attacker_modifier, target_modifier, hit): (i64, i64, bool) = lua
            .eval_callback(&format!(
                "local r=btech.unit.fire({},1,0,{}); return r.aim.attacker_movement, r.aim.target_movement, r.salvo ~= nil",
                id.0, target.0
            )).unwrap();
        assert_eq!(attacker_modifier, if shooter_jumps { 3 } else { 0 });
        assert_eq!(target_modifier, if target_jumps { 3 } else { 0 });
        assert!(hit);
        assert_eq!(native.world().btech, lua.world().btech);
        assert_eq!(
            native.world().btech.constructed_units()[&id].flight(),
            world.btech.constructed_units()[&id].flight()
        );
        native.world().validate(&config).unwrap();
    }
}

#[tokio::test]
async fn early_landing_native_lua_success_failure_and_rollback_share_current_point() {
    use stompymux_rs::*;
    let (_dir, config, base, id) = runtime_fixture().await;
    for success in [false, true] {
        let mut world = base.clone();
        world
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        set_battle_character(
            &mut world,
            ObjectId(1),
            BattleCharacter {
                build: 5,
                reflexes: 4,
                intuition: 3,
                learn: 2,
                charisma: 1,
                bruise: 0,
                lethal: 0,
            },
        )
        .unwrap();
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            "Piloting-Biped",
            BattleCharacterValue {
                value: if success { 30 } else { 0 },
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
        for _ in 0..9 {
            advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..jump_rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                },
            )
            .unwrap();
        }
        // Seed a failing ordinary skill-11 check while retaining real fall dice.
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() < 11)
            .unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        let point = world.btech.constructed_units()[&id].motion().unwrap().point;
        persistence::save(&config.database(), &world).await.unwrap();
        let mut reloaded = persistence::load(&config.database()).await.unwrap();
        reloaded
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(reloaded))).unwrap();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.land({},1); error('abort landing')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        for invalid in ["land nonsense", "land/quiet"] {
            support::run_text(&native, &config, ObjectId(1), 1, invalid);
            assert_eq!(native.world().btech, world.btech);
        }
        let text = support::run_text(&native, &config, ObjectId(1), 1, "land");
        assert!(
            text.contains(if success {
                "You are able to abort the jump."
            } else {
                "You don't quite make it."
            }),
            "{text}"
        );
        assert!(
            lua.eval_callback::<bool>(&format!("return btech.unit.land({},1)", id.0))
                .unwrap()
        );
        assert_eq!(native.world().btech, lua.world().btech);
        let landed = native.world();
        let unit = &landed.btech.constructed_units()[&id];
        assert!(unit.flight().is_none());
        assert_eq!(unit.motion().unwrap().point, point);
        assert_eq!(
            unit.posture(),
            if success {
                BattlePosture::Standing
            } else {
                BattlePosture::Prone
            }
        );
        assert_eq!(unit.jump_stabilization(), 12);
        landed.validate(&config).unwrap();
        let before = landed.btech.clone();
        drop(landed);
        assert!(
            lua.eval_callback::<bool>(&format!("return btech.unit.land({},1)", id.0))
                .is_err()
        );
        assert_eq!(lua.world().btech, before);
    }
}

#[tokio::test]
async fn early_landing_success_still_checks_damaged_landing_gear() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    destroy_battle_critical(
        &mut world,
        id,
        CriticalLocation {
            section: BattleSection::LeftLeg,
            slot: 1,
        },
    )
    .unwrap();
    // Disconnected pilot uses target six; damaged actuator adds one to both checks.
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            dice.two_d6() >= 7 && dice.two_d6() < 7
        })
        .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let notices = land_battle_jump(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules {
            fall: stompymux_rs::BattleFallRules {
                stacking: stompymux_rs::BattleStackingRules::STANDARD,
                ..jump_rules()
            },
            ..stompymux_rs::BattleMovementRules::STANDARD
        },
    )
    .unwrap();
    assert!(
        notices
            .iter()
            .any(|notice| notice.text == "You are able to abort the jump.")
    );
    assert!(
        notices.iter().any(|notice| notice.text
            == "Your damaged leg actuators have caused you to fall upon landing!")
    );
    assert_eq!(
        world.btech.constructed_units()[&id].posture(),
        BattlePosture::Prone
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn tcp_early_landing_save_failure_restores_flight_and_dice_before_retry() {
    use sqlx::Connection;
    use stompymux_rs::*;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world, id) = runtime_fixture().await;
        set_battle_character(&mut world, ObjectId(1), BattleCharacter { build: 5, reflexes: 4, intuition: 3, learn: 2, charisma: 1, bruise: 0, lethal: 0 }).unwrap();
        set_battle_character_value(&mut world, ObjectId(1), "Piloting-Biped", BattleCharacterValue { value: 30, experience: 0, last_used: 0 }).unwrap();
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
        for _ in 0..6 { advance_battle_jumps(&mut world, stompymux_rs::BattleMovementRules {fall: stompymux_rs::BattleFallRules { stacking: stompymux_rs::BattleStackingRules::STANDARD, ..jump_rules() },  ..stompymux_rs::BattleMovementRules::STANDARD }).unwrap(); }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("UPDATE player_state SET password_hash=? WHERE object_dbref=1").bind(accounts::hash("secret", &config).unwrap()).execute(&mut sql).await.unwrap();
        // Also freeze flight ticks so a rejected action can be compared with the exact saved cursor.
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER reject_landing BEFORE UPDATE ON btech_units WHEN NEW.dbref={} BEGIN SELECT RAISE(ABORT,'landing failure'); END",id.0))).execute(&mut sql).await.unwrap();
        let (address, shutdown, task, _lua) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        let mut client = jump_client(address).await;
        let before = persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].clone();
        client.send("land").await;
        let mut failure = client.until("Unable to save your changes.").await;
        client.send("say landing-failure-fence").await;
        failure.push_str(&client.until("You say \"landing-failure-fence\"").await);
        assert!(!failure.contains("You abort your full jump"), "{failure}");
        assert!(!failure.contains("You make a piloting skill roll!"), "{failure}");
        assert!(!failure.contains("Modified Pilot Skill:"), "{failure}");
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id], before);
        sqlx::query("DROP TRIGGER reject_landing").execute(&mut sql).await.unwrap();
        client.send("land").await;
        let feedback = client.until("You finish your jump.").await;
        assert!(feedback.contains("You make a piloting skill roll!"), "{feedback}");
        assert!(feedback.contains("Modified Pilot Skill:"), "{feedback}");
        let landed = persistence::load(&config.database()).await.unwrap();
        assert!(landed.btech.constructed_units()[&id].flight().is_none());
        assert_eq!(landed.btech.constructed_units()[&id].posture(), BattlePosture::Standing);
        let mut expected_dice: BattleDice = serde_json::from_value(serde_json::to_value(&before).unwrap()["dice"].clone()).unwrap();
        expected_dice.two_d6();
        assert_eq!(serde_json::to_value(&landed.btech.constructed_units()[&id]).unwrap()["dice"], serde_json::to_value(expected_dice).unwrap());
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
        assert!(persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].flight().is_none());
    }).await;
}

#[tokio::test]
async fn damage_history_does_not_block_launch_or_add_an_unrelated_landing_roll() {
    use stompymux_rs::*;
    let (_dir, config, base, id) = runtime_fixture().await;
    for mode in [
        BattleStaggerMode::Traditional,
        BattleStaggerMode::Retain,
        BattleStaggerMode::Consume,
    ] {
        for damage in [1, 20, 40] {
            let mut world = base.clone();
            let mut state = serde_json::to_value(&world.btech).unwrap();
            let history = &mut state["constructed"][id.0.to_string()]["stagger"];
            if mode == BattleStaggerMode::Traditional {
                history["turn_damage"] = damage.into();
            } else {
                history["hits"] =
                    serde_json::json!([{"damage":damage,"remaining":60,"counted":false}]);
            }
            world.btech = serde_json::from_value(state).unwrap();
            let before = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap();
            launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut restarted = persistence::load(&config.database()).await.unwrap();
            let rules = BattleFallRules {
                stagger: mode,
                ..jump_rules()
            };
            for _ in 0..12 {
                assert_eq!(
                    advance_battle_jumps(
                        &mut world,
                        stompymux_rs::BattleMovementRules {
                            fall: stompymux_rs::BattleFallRules {
                                stacking: stompymux_rs::BattleStackingRules::STANDARD,
                                ..rules
                            },
                            ..stompymux_rs::BattleMovementRules::STANDARD
                        }
                    )
                    .unwrap(),
                    advance_battle_jumps(
                        &mut restarted,
                        stompymux_rs::BattleMovementRules {
                            fall: stompymux_rs::BattleFallRules {
                                stacking: stompymux_rs::BattleStackingRules::STANDARD,
                                ..rules
                            },
                            ..stompymux_rs::BattleMovementRules::STANDARD
                        }
                    )
                    .unwrap()
                );
            }
            assert_eq!(world.btech, restarted.btech);
            let unit = &world.btech.constructed_units()[&id];
            assert!(unit.flight().is_none());
            assert_eq!(unit.posture(), BattlePosture::Standing);
            let after = serde_json::to_value(unit).unwrap();
            assert_eq!(before["dice"], after["dice"]);
            // Flight updates leave aging and consumption to the stagger heartbeat.
            assert_eq!(before["stagger"], after["stagger"]);
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn rolling_damage_can_still_force_a_fall_after_launch() {
    use stompymux_rs::*;
    let (_dir, config, base, id) = runtime_fixture().await;
    for mode in [BattleStaggerMode::Retain, BattleStaggerMode::Consume] {
        let mut world = base.clone();
        world
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["stagger"]["hits"] =
            serde_json::json!([{"damage":20,"remaining":60,"counted":false}]);
        world.btech = serde_json::from_value(state).unwrap();
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
        let reports = advance_battle_stagger(
            &mut world,
            BattleStaggerRules {
                vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
                mode,
                interval: 1,
                tonnage: true,
                hit: jump_rules().hit,
                extended_piloting: true,
            },
        )
        .unwrap();
        assert_eq!(reports.len(), 1);
        assert!(!reports[0].check.success); // Connected, untrained pilot has target 18 plus stagger.
        assert_eq!(reports[0].fall.as_ref().unwrap().damage, 4);
        let unit = &world.btech.constructed_units()[&id];
        assert!(unit.flight().is_none());
        assert_eq!(unit.posture(), BattlePosture::Prone);
        assert!(unit.stagger().hits.is_empty());
        world.validate(&config).unwrap();
    }
}

/// Replace fixture terrain before takeoff with explicit endpoint heights and an optional intervening hill.
fn jump_hills(
    world: &mut stompymux_rs::World,
    id: stompymux_rs::ObjectId,
    start: u8,
    end: u8,
    hill: u8,
) {
    use stompymux_rs::*;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let mut source = "12 12\n".to_owned();
    for y in 0..12 {
        for x in 0..12 {
            let height = match (x, y) {
                (5, 5) => start,
                (5, 3) => end,
                (5, 4) => hill,
                _ => 0,
            };
            source.push_str(&format!(".{height}"));
        }
        source.push('\n');
    }
    stop_battle_unit(
        world,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let home = world.objects[&id].home.unwrap();
    remove_battle_unit(world, id, home).unwrap();
    reload_battle_map(
        world,
        map,
        "hills.map",
        BattleMapAsset::parse(&source).unwrap(),
    )
    .unwrap();
    place_battle_unit(world, id, map, 5, 5).unwrap();
    assign_battle_pilot(world, id, ObjectId(1)).unwrap();
    start_battle_unit(world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(world, 0);
    }
}

#[tokio::test]
async fn jumps_use_destination_height_and_enforce_ascent_and_descent_capacity() {
    use stompymux_rs::*;
    let (_dir, _config, base, id) = runtime_fixture().await;
    for (start, end) in [(0, 3), (3, 0), (0, 5), (5, 0)] {
        let (_case_dir, config, mut world, id) = runtime_fixture().await;
        jump_hills(&mut world, id, start, end, 0);
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
        let flight = world.btech.constructed_units()[&id].flight().unwrap();
        assert_eq!(
            flight.path().sample(1.0, 5).unwrap().elevation,
            f64::from(end)
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restarted = persistence::load(&config.database()).await.unwrap();
        for _ in 0..24 {
            assert_eq!(
                advance_battle_jumps(
                    &mut world,
                    stompymux_rs::BattleMovementRules {
                        fall: stompymux_rs::BattleFallRules {
                            stacking: stompymux_rs::BattleStackingRules::STANDARD,
                            ..jump_rules()
                        },
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    }
                )
                .unwrap(),
                advance_battle_jumps(
                    &mut restarted,
                    stompymux_rs::BattleMovementRules {
                        fall: stompymux_rs::BattleFallRules {
                            stacking: stompymux_rs::BattleStackingRules::STANDARD,
                            ..jump_rules()
                        },
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    }
                )
                .unwrap()
            );
        }
        assert_eq!(world.btech, restarted.btech);
        let unit = &world.btech.constructed_units()[&id];
        assert!(unit.flight().is_none());
        assert_eq!(unit.position().unwrap().y, 3);
        assert_eq!(unit.posture(), BattlePosture::Standing);
        world.validate(&config).unwrap();
    }
    for (start, end) in [(0, 6), (6, 0)] {
        let mut world = base.clone();
        jump_hills(&mut world, id, start, end, 0);
        let before = world.btech.clone();
        assert!(launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).is_err());
        assert_eq!(world.btech, before);
    }
}

#[tokio::test]
async fn hill_collision_rolls_back_the_transition_then_lands_or_falls() {
    use stompymux_rs::*;
    let (_dir, config, base, id) = runtime_fixture().await;
    for safe in [false, true] {
        let mut world = base.clone();
        jump_hills(&mut world, id, 0, 0, 9);
        if safe {
            world
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .insert(Flag::Connected);
            set_battle_character(
                &mut world,
                ObjectId(1),
                BattleCharacter {
                    build: 5,
                    reflexes: 4,
                    intuition: 3,
                    learn: 2,
                    charisma: 1,
                    bruise: 0,
                    lethal: 0,
                },
            )
            .unwrap();
            set_battle_character_value(
                &mut world,
                ObjectId(1),
                "Piloting-Biped",
                BattleCharacterValue {
                    value: 30,
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
        }
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restarted = persistence::load(&config.database()).await.unwrap();
        if safe {
            restarted
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        let mut collided = false;
        for _ in 0..24 {
            let point = world.btech.constructed_units()[&id].motion().unwrap().point;
            let notices = advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..jump_rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                },
            )
            .unwrap();
            assert_eq!(
                notices,
                advance_battle_jumps(
                    &mut restarted,
                    stompymux_rs::BattleMovementRules {
                        fall: stompymux_rs::BattleFallRules {
                            stacking: stompymux_rs::BattleStackingRules::STANDARD,
                            ..jump_rules()
                        },
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    }
                )
                .unwrap()
            );
            assert_eq!(world.btech, restarted.btech);
            if world.btech.constructed_units()[&id].flight().is_some() {
                continue;
            }
            collided = true;
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(unit.motion().unwrap().point, point);
            assert_eq!(unit.position().unwrap().y, 5);
            assert_eq!(
                unit.posture(),
                if safe {
                    BattlePosture::Standing
                } else {
                    BattlePosture::Prone
                }
            );
            assert_eq!(unit.jump_stabilization(), 12);
            assert!(
                notices
                    .iter()
                    .any(|notice| notice.text.contains("too high"))
            );
            assert!(notices.iter().any(|notice| notice.text.contains(if safe {
                "land safely"
            } else {
                "crash into the obstacle"
            })));
            break;
        }
        assert!(collided);
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn dry_terrain_jump_routes_share_adapters_heat_and_restartable_landing() {
    use stompymux_rs::*;
    for terrain in [
        Terrain::LightForest,
        Terrain::HeavyForest,
        Terrain::Rough,
        Terrain::Mountains,
        Terrain::Snow,
        Terrain::Smoke,
        Terrain::Fire,
    ] {
        let (_dir, config, mut world, id) = runtime_fixture().await;
        let map = world.btech.constructed_units()[&id].position().unwrap().map;
        let home = world.objects[&id].home.unwrap();
        stop_battle_unit(
            &mut world,
            id,
            ObjectId(1),
            stompymux_rs::BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        remove_battle_unit(&mut world, id, home).unwrap();
        let row = format!("{}0", terrain.symbol()).repeat(12) + "\n";
        reload_battle_map(
            &mut world,
            map,
            "dry.map",
            BattleMapAsset::parse(&format!("12 12\n{}", row.repeat(12))).unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 5, 5).unwrap();
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        let before = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap();
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let text = support::run_text(&native, &config, ObjectId(1), 1, "jump 0 2");
        assert!(
            text.contains("You engage your jump jets."),
            "{terrain:?}: {text}"
        );
        assert!(
            lua.eval_callback::<bool>(&format!("return btech.unit.jump({},1,0,2)", id.0))
                .unwrap()
        );
        assert_eq!(native.world().btech, lua.world().btech);
        let mut flight_world = native.world().clone();
        assert_eq!(
            flight_world.btech.constructed_units()[&id]
                .heat_rates(&flight_world)
                .production,
            if terrain == Terrain::Fire { 10.0 } else { 5.0 }
        );
        for _ in 0..12 {
            advance_battle_jumps(
                &mut flight_world,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..jump_rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                },
            )
            .unwrap();
        }
        persistence::save(&config.database(), &flight_world)
            .await
            .unwrap();
        let mut restarted = persistence::load(&config.database()).await.unwrap();
        for _ in 0..12 {
            assert_eq!(
                advance_battle_jumps(
                    &mut flight_world,
                    stompymux_rs::BattleMovementRules {
                        fall: stompymux_rs::BattleFallRules {
                            stacking: stompymux_rs::BattleStackingRules::STANDARD,
                            ..jump_rules()
                        },
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    }
                )
                .unwrap(),
                advance_battle_jumps(
                    &mut restarted,
                    stompymux_rs::BattleMovementRules {
                        fall: stompymux_rs::BattleFallRules {
                            stacking: stompymux_rs::BattleStackingRules::STANDARD,
                            ..jump_rules()
                        },
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    }
                )
                .unwrap()
            );
        }
        assert_eq!(flight_world.btech, restarted.btech);
        let unit = &flight_world.btech.constructed_units()[&id];
        assert!(unit.flight().is_none());
        assert_eq!(unit.position().unwrap().y, 3);
        assert_eq!(unit.posture(), BattlePosture::Standing);
        assert_eq!(serde_json::to_value(unit).unwrap()["dice"], before["dice"]);
        assert_eq!(
            unit.heat_rates(&flight_world).production,
            if terrain == Terrain::Fire { 5.0 } else { 0.0 }
        );
        flight_world.validate(&config).unwrap();
    }
}

/// Dry takeoff and a column of water with a configurable depth and takeoff tile.
async fn water_jump_fixture(
    depth: u8,
    wet_start: bool,
) -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    stompymux_rs::ObjectId,
) {
    use stompymux_rs::*;
    let (dir, config, mut world, id) = runtime_fixture().await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let home = world.objects[&id].home.unwrap();
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    remove_battle_unit(&mut world, id, home).unwrap();
    let mut source = "12 12\n".to_owned();
    for y in 0..12 {
        for x in 0..12 {
            if x == 5 && (y < 5 || (y == 5 && wet_start)) {
                source.push_str(&format!("~{depth}"));
            } else {
                source.push_str(".0");
            }
        }
        source.push('\n');
    }
    reload_battle_map(
        &mut world,
        map,
        "water-jump.map",
        BattleMapAsset::parse(&source).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 5, 5).unwrap();
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

#[tokio::test]
async fn water_jump_avoids_airborne_flooding_then_floods_and_cools_on_landing() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = water_jump_fixture(3, false).await;
    apply_damage_phase(
        &mut world,
        id,
        BattleSection::LeftTorso,
        8,
        BattleDamagePhase::Armor { rear: false },
    )
    .unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 3.0).unwrap();
    for _ in 0..12 {
        advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..jump_rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
    }
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.position().unwrap().y, 4);
    assert!(unit.flight().unwrap().sample().elevation > 0.0);
    assert_eq!(unit.heat_rates(&world).dissipation, 10.0);
    assert!(
        flood_battle_unit(&mut world, id, jump_rules())
            .unwrap()
            .is_empty()
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restarted = persistence::load(&config.database()).await.unwrap();
    let mut notices = Vec::new();
    for _ in 0..60 {
        if world.btech.constructed_units()[&id].flight().is_none() {
            break;
        }
        let expected = advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..jump_rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
        assert_eq!(
            expected,
            advance_battle_jumps(
                &mut restarted,
                stompymux_rs::BattleMovementRules {
                    fall: stompymux_rs::BattleFallRules {
                        stacking: stompymux_rs::BattleStackingRules::STANDARD,
                        ..jump_rules()
                    },
                    ..stompymux_rs::BattleMovementRules::STANDARD
                }
            )
            .unwrap()
        );
        notices.extend(expected);
    }
    assert_eq!(world.btech, restarted.btech);
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.flight().is_none());
    assert_eq!(unit.position().unwrap().y, 2);
    assert!(unit.flooded_sections().contains(&BattleSection::LeftTorso));
    assert_eq!(unit.heat_rates(&world).dissipation, 16.0);
    assert!(
        notices
            .iter()
            .any(|notice| notice.text.contains("Water floods into your Left Torso"))
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn shallow_water_launches_and_airborne_fire_above_deep_water_are_supported() {
    use stompymux_rs::*;
    for depth in [1, 2] {
        let (_dir, config, mut world, id) = water_jump_fixture(depth, true).await;
        let before = world.btech.clone();
        let result = launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0);
        if depth == 1 {
            assert!(result.is_ok());
            world.validate(&config).unwrap();
        } else {
            assert!(result.is_err());
            assert_eq!(world.btech, before);
        }
    }
    let (_dir, config, mut world, id) = water_jump_fixture(3, false).await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let target = world.create(&config, "Shore target".into(), Kind::Thing);
    world.objects.get_mut(&target).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        target,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, target, map, 6, 2).unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 3.0).unwrap();
    for _ in 0..12 {
        advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..jump_rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
    }
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("fire 0 #{}", target.0),
    );
    assert!(
        scripts.world().btech.constructed_units()[&id]
            .weapon_readiness(0)
            .unwrap()
            .recycle_remaining
            > 0,
        "{text}"
    );
    assert!(
        scripts.world().btech.constructed_units()[&id]
            .flight()
            .is_some()
    );
    scripts.world().validate(&config).unwrap();
}

#[tokio::test]
async fn submerged_hex_entry_floods_breached_legs_without_canceling_thrust() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = water_jump_fixture(5, false).await;
    for section in [BattleSection::LeftLeg, BattleSection::RightLeg] {
        apply_damage_phase(
            &mut world,
            id,
            section,
            6,
            BattleDamagePhase::Armor { rear: false },
        )
        .unwrap();
    }
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 5.0).unwrap();
    let mut flooded_in_flight = false;
    for _ in 0..60 {
        advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: stompymux_rs::BattleFallRules {
                    stacking: stompymux_rs::BattleStackingRules::STANDARD,
                    ..jump_rules()
                },
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
        let unit = &world.btech.constructed_units()[&id];
        if unit.flight().is_some() && unit.flooded_sections().contains(&BattleSection::RightLeg) {
            assert_eq!(unit.posture(), BattlePosture::Prone);
            assert!(unit.flooded_sections().contains(&BattleSection::LeftLeg));
            flooded_in_flight = true;
        }
        if unit.flight().is_none() {
            break;
        }
    }
    assert!(flooded_in_flight);
    assert!(world.btech.constructed_units()[&id].flight().is_none());
    world.validate(&config).unwrap();
}

/// Shutdown changes flight mode atomically; every countdown and impact survives a restart.
#[tokio::test]
async fn airborne_shutdown_native_lua_and_saved_descent_agree() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 4.0).unwrap();
    for _ in 0..4 {
        advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: jump_rules(),
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
    }
    let before = world.btech.clone();
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert!(
        lua.eval_callback::<bool>(&format!("btech.unit.stop({},1); error('abort')", id.0))
            .is_err()
    );
    assert_eq!(lua.world().btech, before);
    let text = support::run_text(&native, &config, ObjectId(1), 1, "shutdown");
    assert!(text.contains("You start free-fall"), "{text}");
    assert!(
        lua.eval_callback::<bool>(&format!("return btech.unit.stop({},1)", id.0))
            .unwrap()
    );
    assert_eq!(native.world().btech, lua.world().btech);
    let mut world = native.world().clone();
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.power(), BattlePower::Off);
    assert!(unit.pilot().is_none());
    assert!(unit.flight().is_none());
    assert!(unit.airborne());
    let point = unit.motion().unwrap().point;
    let mut impacts = 0;
    for _ in 0..18 {
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: jump_rules(),
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
        assert_eq!(
            advance_battle_jumps(
                &mut restored,
                stompymux_rs::BattleMovementRules {
                    fall: jump_rules(),
                    ..stompymux_rs::BattleMovementRules::STANDARD
                }
            )
            .unwrap(),
            notices
        );
        assert_eq!(restored.btech, world.btech);
        assert_eq!(
            world.btech.constructed_units()[&id].motion().unwrap().point,
            point
        );
        impacts += notices
            .iter()
            .filter(|notice| notice.text == "You hit the ground!")
            .count();
    }
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(impacts, 1);
    assert!(!unit.airborne());
    assert_eq!(unit.posture(), BattlePosture::Prone);
    assert_eq!(unit.jump_stabilization(), 0);
}

/// An impact failure retries the same scheduled event; administrative removal clears it.
#[tokio::test]
async fn free_fall_impact_failure_is_atomic_and_removal_cancels_descent() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap();
    stop_battle_unit(&mut world, id, ObjectId(1), jump_rules()).unwrap();
    for _ in 0..2 {
        assert!(
            advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: jump_rules(),
                    ..stompymux_rs::BattleMovementRules::STANDARD
                }
            )
            .unwrap()
            .is_empty()
        );
    }
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let before = world.btech.clone();
    assert!(
        advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: jump_rules(),
                ..stompymux_rs::BattleMovementRules::STANDARD
            }
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .remove(Flag::InCharacter);
    let mut removed = world.clone();
    remove_battle_unit(&mut removed, id, ObjectId(config.home())).unwrap();
    assert!(
        advance_battle_jumps(
            &mut removed,
            stompymux_rs::BattleMovementRules {
                fall: jump_rules(),
                ..stompymux_rs::BattleMovementRules::STANDARD
            }
        )
        .unwrap()
        .is_empty()
    );
    persistence::save(&config.database(), &removed)
        .await
        .unwrap();
    assert!(
        advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: jump_rules(),
                ..stompymux_rs::BattleMovementRules::STANDARD
            }
        )
        .unwrap()
        .iter()
        .any(|notice| notice.text == "You hit the ground!")
    );
    assert!(!world.btech.constructed_units()[&id].airborne());
}

/// Falling chooses the bridge underside and ice top, independently of movement rate or gravity.
#[tokio::test]
async fn free_fall_surface_contact_and_engine_restart_keep_the_event_cadence() {
    use stompymux_rs::*;
    for (terrain, elevation, impact_second) in [
        (Terrain::Grassland, 0, 6),
        (Terrain::Water, 3, 9),
        (Terrain::Ice, 3, 6),
        (Terrain::Bridge, 9, 9),
        (Terrain::Bridge, 2, 6),
    ] {
        let (_dir, config, mut base, id) = runtime_fixture().await;
        launch_battle_jump(&mut base, id, ObjectId(1), 0, 1.0).unwrap();
        stop_battle_unit(&mut base, id, ObjectId(1), jump_rules()).unwrap();
        let map = base.btech.constructed_units()[&id].position().unwrap().map;
        let mut world = base;
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["free_fall"] = serde_json::json!({
            "elevation": 5, "speed": 1, "remaining": 3,
        });
        let field = &mut state["maps"][map.0.to_string()];
        field["gravity"] = 200.into();
        field["movement_modifier"] = 800.into();
        for tile in field["terrain"].as_array_mut().unwrap() {
            *tile = serde_json::to_value(BattleHex { terrain, elevation }).unwrap();
        }
        world.btech = serde_json::from_value(state).unwrap();
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        for second in 1..=impact_second {
            advance_battle_units(&mut world, 0);
            if second == 5 {
                assert_eq!(
                    world.btech.constructed_units()[&id].power(),
                    BattlePower::Running
                );
                assert!(world.btech.constructed_units()[&id].airborne());
                assert!(launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).is_err());
                assert!(set_battle_speed(&mut world, id, ObjectId(1), 1.0).is_err());
            }
            persistence::save(&config.database(), &world).await.unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            let events = advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: jump_rules(),
                    ..stompymux_rs::BattleMovementRules::STANDARD
                },
            )
            .unwrap();
            assert_eq!(
                advance_battle_jumps(
                    &mut restored,
                    stompymux_rs::BattleMovementRules {
                        fall: jump_rules(),
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    }
                )
                .unwrap(),
                events
            );
            assert_eq!(world.btech, restored.btech);
            assert_eq!(
                events
                    .iter()
                    .any(|notice| notice.text == "You hit the ground!"),
                second == impact_second,
                "{terrain:?} at {elevation}, second {second}"
            );
        }
        assert!(!world.btech.constructed_units()[&id].airborne());
    }
}

/// Landing ends airborne movement but leaves the already scheduled fall event due.
#[tokio::test]
async fn restarted_free_fall_can_land_and_retains_its_pending_impact() {
    use stompymux_rs::*;
    for roll in [2, 11] {
        let (_dir, config, mut world, id) = runtime_fixture().await;
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap();
        stop_battle_unit(&mut world, id, ObjectId(1), jump_rules()).unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["free_fall"]["elevation"] = 50.into();
        world.btech = serde_json::from_value(state).unwrap();
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        let before = world.btech.clone();
        assert!(
            land_battle_jump(
                &mut world,
                id,
                ObjectId(1),
                stompymux_rs::BattleMovementRules {
                    fall: jump_rules(),
                    ..stompymux_rs::BattleMovementRules::STANDARD
                }
            )
            .is_err()
        );
        assert_eq!(world.btech, before);
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
            advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: jump_rules(),
                    ..stompymux_rs::BattleMovementRules::STANDARD
                },
            )
            .unwrap();
        }
        let dice = (0..=u8::MAX)
            .find_map(|seed| {
                let dice = BattleDice::seeded([seed; 32]);
                let mut check = dice.clone();
                (check.two_d6() == roll).then_some(dice)
            })
            .unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["dice"] = serde_json::to_value(dice).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        let before = world.btech.clone();
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        assert!(
            lua.eval_callback::<bool>(&format!("btech.unit.land({},1); error('abort')", id.0))
                .is_err()
        );
        assert_eq!(lua.world().btech, before);
        let text = support::run_text(&native, &config, ObjectId(1), 1, "land");
        assert!(
            text.contains(if roll == 11 {
                "You are able to abort"
            } else {
                "You don't quite make it"
            }),
            "{text}"
        );
        assert!(
            lua.eval_callback::<bool>(&format!("return btech.unit.land({},1)", id.0))
                .unwrap()
        );
        assert_eq!(native.world().btech, lua.world().btech);
        let mut landed = native.world().clone();
        let unit = &landed.btech.constructed_units()[&id];
        assert!(!unit.airborne());
        assert!(unit.free_fall().unwrap().grounded());
        assert_eq!(unit.jump_stabilization(), 12);
        assert_eq!(
            unit.posture(),
            if roll == 11 {
                BattlePosture::Standing
            } else {
                BattlePosture::Prone
            }
        );
        persistence::save(&config.database(), &landed)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_jumps(
            &mut landed,
            stompymux_rs::BattleMovementRules {
                fall: jump_rules(),
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
        assert_eq!(
            advance_battle_jumps(
                &mut restored,
                stompymux_rs::BattleMovementRules {
                    fall: jump_rules(),
                    ..stompymux_rs::BattleMovementRules::STANDARD
                }
            )
            .unwrap(),
            notices
        );
        assert_eq!(landed.btech, restored.btech);
        assert!(
            notices
                .iter()
                .any(|notice| notice.text == "You hit the ground!")
        );
        assert!(landed.btech.constructed_units()[&id].free_fall().is_none());
        assert_eq!(
            landed.btech.constructed_units()[&id].posture(),
            BattlePosture::Prone
        );
    }
}

/// Impact schedules stabilization even with the engine off; restarting does not reset it.
#[tokio::test]
async fn powered_off_free_fall_stabilization_counts_down_through_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([1; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap();
    stop_battle_unit(&mut world, id, ObjectId(1), jump_rules()).unwrap();
    for _ in 0..3 {
        advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: jump_rules(),
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
    }
    assert_eq!(
        world.btech.constructed_units()[&id].jump_stabilization(),
        12
    );
    for _ in 0..5 {
        assert!(
            advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: jump_rules(),
                    ..stompymux_rs::BattleMovementRules::STANDARD
                }
            )
            .unwrap()
            .is_empty()
        );
    }
    persistence::save(&config.database(), &world).await.unwrap();
    world = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech.constructed_units()[&id].jump_stabilization(), 7);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
        assert!(
            advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: jump_rules(),
                    ..stompymux_rs::BattleMovementRules::STANDARD
                }
            )
            .unwrap()
            .is_empty()
        );
    }
    assert_eq!(world.btech.constructed_units()[&id].jump_stabilization(), 2);
    assert!(
        advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: jump_rules(),
                ..stompymux_rs::BattleMovementRules::STANDARD
            }
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(
        advance_battle_jumps(
            &mut world,
            stompymux_rs::BattleMovementRules {
                fall: jump_rules(),
                ..stompymux_rs::BattleMovementRules::STANDARD
            }
        )
        .unwrap(),
        vec![BattleNotice {
            unit: id,
            text: "You have finally stabilized after your jump.".to_owned()
        }]
    );
}

/// The live server commits shutdown and delayed impact before publishing their notices.
#[tokio::test]
async fn tcp_free_fall_retries_shutdown_and_impact_saves_across_restart() {
    use sqlx::Connection;
    use stompymux_rs::*;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world, id) = runtime_fixture().await;
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["dice"] = serde_json::to_value(BattleDice::seeded([1; 32])).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("UPDATE player_state SET password_hash=? WHERE object_dbref=1").bind(accounts::hash("secret", &config).unwrap()).execute(&mut sql).await.unwrap();
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER reject_shutdown BEFORE UPDATE ON btech_units WHEN NEW.dbref={} BEGIN SELECT RAISE(ABORT,'shutdown failure'); END",id.0))).execute(&mut sql).await.unwrap();
        let (address, shutdown, task, _lua) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        let mut client = jump_client(address).await;
        let before = persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].clone();
        client.send("shutdown").await;
        let mut output = client.until("Unable to save your changes.").await;
        client.send("say shutdown-fence").await;
        output.push_str(&client.until("You say \"shutdown-fence\"").await);
        assert!(!output.contains("You start free-fall"), "{output}");
        assert!(!output.contains("All systems shut down"), "{output}");
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id], before);
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER reject_impact BEFORE UPDATE ON btech_units WHEN NEW.dbref={} AND json_extract(OLD.live,'$.free_fall') IS NOT NULL AND json_extract(NEW.live,'$.free_fall') IS NULL BEGIN SELECT RAISE(ABORT,'impact failure'); END",id.0))).execute(&mut sql).await.unwrap();
        sqlx::query("DROP TRIGGER reject_shutdown").execute(&mut sql).await.unwrap();
        client.send("shutdown").await;
        client.until("You start free-fall").await;
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
        let saved = persistence::load(&config.database()).await.unwrap();
        let cursor = saved.btech.constructed_units()[&id].free_fall().unwrap();
        assert_eq!(saved.btech.constructed_units()[&id].power(), BattlePower::Off);
        assert!(saved.btech.constructed_units()[&id].pilot().is_none());
        // Hold the resumed timer through login so a live tick cannot outrun the restart assertion.
        sqlx::query(sqlx::AssertSqlSafe(format!("CREATE TRIGGER hold_restart BEFORE UPDATE ON btech_units WHEN NEW.dbref={} BEGIN SELECT RAISE(ABORT,'restart checkpoint'); END",id.0))).execute(&mut sql).await.unwrap();
        let (address, shutdown, task, _lua) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        let mut client = jump_client(address).await;
        let mut saved = persistence::load(&config.database()).await.unwrap();
        assert_eq!(saved.btech.constructed_units()[&id].free_fall(), Some(cursor));
        sqlx::query("DROP TRIGGER hold_restart").execute(&mut sql).await.unwrap();
        for _ in 0..3 {
            if serde_json::to_value(saved.btech.constructed_units()[&id].free_fall()).unwrap()["remaining"] == 1 { break; }
            saved = jump_tick(&config, id).await;
        }
        let before = saved.btech.constructed_units()[&id].clone();
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        client.send("say impact-fence").await;
        let output = client.until("You say \"impact-fence\"").await;
        assert!(!output.contains("You hit the ground!"), "{output}");
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id], before);
        let mut expected = saved;
        let mut expected_fall = jump_rules();
        expected_fall.hit = stompymux_rs::BattleFallRules::configured(&config).hit;
        advance_battle_jumps(&mut expected, stompymux_rs::BattleMovementRules {fall: expected_fall,  ..stompymux_rs::BattleMovementRules::STANDARD }).unwrap();
        sqlx::query("DROP TRIGGER reject_impact").execute(&mut sql).await.unwrap();
        let landed = jump_tick(&config, id).await;
        client.until("You hit the ground!").await;
        let unit = &landed.btech.constructed_units()[&id];
        assert!(unit.free_fall().is_none());
        assert_eq!(unit.posture(), BattlePosture::Prone);
        assert_eq!(unit.sections(), expected.btech.constructed_units()[&id].sections());
        assert_eq!(serde_json::to_value(unit).unwrap()["dice"], serde_json::to_value(&expected.btech.constructed_units()[&id]).unwrap()["dice"]);
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
        assert!(persistence::load(&config.database()).await.unwrap().btech.constructed_units()[&id].free_fall().is_none());
    }).await;
}

/// The protected hardened-gyro hit does not interrupt flight; the next hit uses the ordinary airborne fall check.
#[tokio::test]
async fn hardened_gyro_airborne_protection_and_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["definition"]["attributes"]["specials"] =
        "HDGYRO".into();
    // Keep the test retrofit at nominal mass so launch tests gyro protection, not overload.
    state["constructed"][id.0.to_string()]["live_mass"] = serde_json::json!(35 * 1024);
    world.btech = serde_json::from_value(state).unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    advance_battle_jumps(
        &mut world,
        stompymux_rs::BattleMovementRules {
            fall: jump_rules(),
            ..stompymux_rs::BattleMovementRules::STANDARD
        },
    )
    .unwrap();
    for slot in [3, 4] {
        seed_airborne_critical(
            &mut world,
            id,
            CriticalLocation {
                section: BattleSection::CenterTorso,
                slot,
            },
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        restored
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let flight = world.btech.constructed_units()[&id].flight();
        let report = resolve_battle_tactical_impact(
            &mut world,
            id,
            airborne_hit(BattleSection::CenterTorso),
            1,
            jump_rules(),
        )
        .unwrap();
        assert_eq!(
            resolve_battle_tactical_impact(
                &mut restored,
                id,
                airborne_hit(BattleSection::CenterTorso),
                1,
                jump_rules()
            )
            .unwrap(),
            report
        );
        assert_eq!(world.btech, restored.btech);
        if slot == 3 {
            assert!(report.balance.is_empty());
            assert_eq!(world.btech.constructed_units()[&id].flight(), flight);
            assert_eq!(world.btech.constructed_units()[&id].gyro_damage(), 0);
        } else {
            assert!(!report.balance[0].check.unwrap().success);
            assert!(report.balance[0].fall.is_some());
            assert!(world.btech.constructed_units()[&id].flight().is_none());
        }
    }
}

/// A running cockpit with an acquired view and connected occupant for routing assertions.
fn jump_observer(
    world: &mut stompymux_rs::World,
    config: &stompymux_rs::Config,
    subject: stompymux_rs::ObjectId,
) -> (stompymux_rs::ObjectId, stompymux_rs::ObjectId) {
    use stompymux_rs::*;
    let observer = world.create(config, "Jump observer".into(), Kind::Thing);
    create_battle_unit(
        world,
        observer,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    let map = world.btech.constructed_units()[&subject]
        .position()
        .unwrap()
        .map;
    place_battle_unit(world, observer, map, 6, 5).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][observer.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    world.btech = serde_json::from_value(state).unwrap();
    refresh_battle_contacts(world, &[observer]).unwrap();
    assert!(
        visible_battle_contact(world, observer, subject)
            .unwrap()
            .is_some()
    );
    let witness = world.create(config, "Jump witness".into(), Kind::Player);
    let player = world.objects.get_mut(&witness).unwrap();
    player.location = Some(observer);
    player.flags.insert(Flag::Connected);
    (observer, witness)
}

/// Launch, early landing and normal/damaged completion share observer routing and saved replay.
#[tokio::test]
async fn jump_observers_cover_launch_landings_damage_and_transaction_replay() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id) = runtime_fixture().await;
    let (observer, witness) = jump_observer(&mut base, &config, id);
    let messages = |scripts: &Scripts| {
        scripts
            .drain_outbox()
            .into_iter()
            .map(|(recipient, text)| (recipient, text.source().to_owned()))
            .collect::<Vec<_>>()
    };
    for scenario in ["normal", "early", "crash", "leg", "actuator", "gyro"] {
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        assert!(
            lua.eval_callback::<()>(&format!("btech.unit.jump({},1,0,2); error('abort')", id.0))
                .is_err()
        );
        assert_eq!(lua.world().btech, base.btech);
        assert!(messages(&lua).is_empty());
        commands::run(&native, &config, ObjectId(1), 1, "jump 0 2").unwrap();
        lua.eval_callback::<bool>(&format!("return btech.unit.jump({},1,0,2)", id.0))
            .unwrap();
        let launch = messages(&native);
        assert_eq!(messages(&lua), launch);
        assert_eq!(
            launch
                .iter()
                .filter(|(recipient, text)| *recipient == witness
                    && text.ends_with("engages jumpjets!"))
                .count(),
            1
        );
        assert_eq!(native.world().btech, lua.world().btech);
        let mut world = native.world().clone();
        match scenario {
            "leg" => {
                let mut hit = airborne_hit(BattleSection::LeftLeg);
                hit.through_armor_critical = false;
                resolve_battle_tactical_impact(&mut world, id, hit, 14, jump_rules()).unwrap();
            }
            "actuator" | "gyro" => {
                let location = if scenario == "actuator" {
                    CriticalLocation {
                        section: BattleSection::LeftLeg,
                        slot: 1,
                    }
                } else {
                    CriticalLocation {
                        section: BattleSection::CenterTorso,
                        slot: 3,
                    }
                };
                destroy_battle_critical(&mut world, id, location).unwrap();
            }
            _ => {}
        }
        let seed = (0..=255)
            .find(|seed| (BattleDice::seeded([*seed; 32]).two_d6() >= 6) == (scenario == "early"))
            .unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let suffix = match scenario {
            "crash" => "attempts a landing, but crashes to the ground!",
            "leg" => "lands, unbalanced, and falls down!",
            "actuator" => "lands, stumbles, and falls down!",
            "gyro" => "lands, twists awkwardly, and falls down!",
            _ => "lands gracefully.",
        };
        if matches!(scenario, "early" | "crash") {
            restored
                .objects
                .get_mut(&witness)
                .unwrap()
                .flags
                .insert(Flag::Connected);
            let native = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let lua =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
            assert!(
                lua.eval_callback::<()>(&format!("btech.unit.land({},1); error('abort')", id.0))
                    .is_err()
            );
            assert_eq!(lua.world().btech, world.btech);
            assert!(messages(&lua).is_empty());
            commands::run(&native, &config, ObjectId(1), 1, "land").unwrap();
            lua.eval_callback::<bool>(&format!("return btech.unit.land({},1)", id.0))
                .unwrap();
            let notices = messages(&native);
            assert_eq!(messages(&lua), notices);
            let pilot_messages: Vec<_> = notices
                .iter()
                .filter(|(who, _)| *who == ObjectId(1))
                .map(|(_, text)| text.as_str())
                .collect();
            assert_eq!(
                pilot_messages[0],
                "You abort your full jump and attempt to land early"
            );
            assert_eq!(pilot_messages[1], "You make a piloting skill roll!");
            assert_eq!(
                pilot_messages[2],
                format!(
                    "Modified Pilot Skill: BTH 6\tRoll: {}",
                    BattleDice::seeded([seed; 32]).two_d6()
                )
            );
            let witnessed: Vec<_> = notices
                .iter()
                .filter(|(recipient, _)| *recipient == witness)
                .collect();
            assert_eq!(witnessed.len(), 1, "{scenario}: {notices:?}");
            assert!(witnessed[0].1.ends_with(suffix));
            assert_eq!(native.world().btech, lua.world().btech);
        } else {
            let mut witnessed = Vec::new();
            for _ in 0..60 {
                let notices = advance_battle_jumps(
                    &mut world,
                    stompymux_rs::BattleMovementRules {
                        fall: jump_rules(),
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    },
                )
                .unwrap();
                assert_eq!(
                    advance_battle_jumps(
                        &mut restored,
                        stompymux_rs::BattleMovementRules {
                            fall: jump_rules(),
                            ..stompymux_rs::BattleMovementRules::STANDARD
                        }
                    )
                    .unwrap(),
                    notices
                );
                assert_eq!(world.btech, restored.btech);
                witnessed.extend(notices.into_iter().filter(|notice| notice.unit == observer));
            }
            assert_eq!(witnessed.len(), 1, "{scenario}: {witnessed:?}");
            assert!(
                witnessed[0].text.ends_with(suffix),
                "{scenario}: {witnessed:?}"
            );
            assert!(!world.btech.constructed_units()[&id].airborne());
        }
    }
}

/// Five improved jets use ten contiguous slots while retaining the template's five MP thrust.
fn improved_jet_template() -> BattleTemplate {
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let jet = template.sections[&BattleSection::LeftTorso].criticals[&0].clone();
    for section in template.sections.values_mut() {
        section
            .criticals
            .retain(|_, part| part.equipment != "JumpJet");
    }
    for slot in 0..10 {
        template
            .sections
            .get_mut(&BattleSection::LeftTorso)
            .unwrap()
            .criticals
            .insert(slot, jet.clone());
    }
    template
        .attributes
        .insert("specials".into(), "FlipArms ImprovedJJ_Tech".into());
    template
}

/// Improved installations reject incomplete pairs, mixed brands and unsupported declared thrust.
#[test]
fn improved_jet_construction_pairs_and_mass() {
    let template = improved_jet_template();
    let improved = BattleUnit::from_template(template.clone()).unwrap();
    let ordinary = BattleUnit::from_template(
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    assert_eq!(improved.jump_capacity(100).unwrap().movement_points, 5);
    assert_eq!(
        improved.mass().unwrap().equipment - ordinary.mass().unwrap().equipment,
        5 * 512
    );
    for case in ["missing", "split", "brand", "capacity"] {
        let mut invalid = template.clone();
        match case {
            "missing" => {
                invalid
                    .sections
                    .get_mut(&BattleSection::LeftTorso)
                    .unwrap()
                    .criticals
                    .remove(&1);
            }
            "split" => {
                let part = invalid
                    .sections
                    .get_mut(&BattleSection::LeftTorso)
                    .unwrap()
                    .criticals
                    .remove(&1)
                    .unwrap();
                invalid
                    .sections
                    .get_mut(&BattleSection::RightTorso)
                    .unwrap()
                    .criticals
                    .insert(1, part);
            }
            "brand" => {
                invalid
                    .sections
                    .get_mut(&BattleSection::LeftTorso)
                    .unwrap()
                    .criticals
                    .get_mut(&1)
                    .unwrap()
                    .brand = Some(1);
            }
            _ => invalid.jump_speed -= 10.75,
        }
        assert!(BattleUnit::from_template(invalid).is_err(), "{case}");
    }
}

/// Improved jet damage uses one MP per pair across native/Lua launch, heat, flight and restart.
#[tokio::test]
async fn improved_jet_group_loss_flight_and_adapter_replay() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id) = runtime_fixture().await;
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][id.0.to_string()]["definition"] =
        serde_json::to_value(improved_jet_template()).unwrap();
    base.btech = serde_json::from_value(state).unwrap();
    base.validate(&config).unwrap();
    let (observer, _witness) = jump_observer(&mut base, &config, id);
    let mut partial = base.clone();
    let mut state = serde_json::to_value(&partial.btech).unwrap();
    state["constructed"][id.0.to_string()]["lost_criticals"] =
        serde_json::json!([{"section":"LeftTorso","slot":0}]);
    partial.btech = serde_json::from_value(state).unwrap();
    assert!(partial.validate(&config).is_err());
    let mut flooded = base.clone();
    let mut state = serde_json::to_value(&flooded.btech).unwrap();
    state["constructed"][id.0.to_string()]["flooded_sections"] = serde_json::json!(["LeftTorso"]);
    flooded.btech = serde_json::from_value(state).unwrap();
    assert_eq!(
        flooded.btech.constructed_units()[&id].system_hits(BattleSystem::JumpJet),
        5
    );
    assert_eq!(
        flooded.btech.constructed_units()[&id]
            .jump_capacity(100)
            .unwrap()
            .speed,
        0.0
    );
    let mut severed = base.clone();
    let original_mass = severed.btech.constructed_units()[&id].mass().unwrap();
    apply_damage_phase(
        &mut severed,
        id,
        BattleSection::LeftTorso,
        8,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    let unit = &severed.btech.constructed_units()[&id];
    assert_eq!(unit.system_hits(BattleSystem::JumpJet), 5);
    assert_eq!(unit.jump_capacity(100).unwrap().movement_points, 0);
    // The torso's ten jet slots and the attached arm's two medium lasers are gone.
    assert_eq!(
        original_mass.equipment - unit.mass().unwrap().equipment,
        7 * 1024
    );
    for slot in 0..10 {
        for last in [false, true] {
            let mut before = base.clone();
            let location = CriticalLocation {
                section: BattleSection::LeftTorso,
                slot,
            };
            if last {
                for first in (0..10).step_by(2).filter(|first| *first / 2 != slot / 2) {
                    destroy_battle_critical(
                        &mut before,
                        id,
                        CriticalLocation {
                            section: BattleSection::LeftTorso,
                            slot: first,
                        },
                    )
                    .unwrap();
                }
            }
            // The deliberately crowded jet retrofit is not a balanced construction.
            // Reset its administrative test mass after each pre-launch critical change.
            let mut encoded = serde_json::to_value(&before.btech).unwrap();
            encoded["constructed"][id.0.to_string()]["live_mass"] = serde_json::json!(35 * 1024);
            before.btech = serde_json::from_value(encoded).unwrap();
            let native = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(before.clone())),
            )
            .unwrap();
            let lua = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(before.clone())),
            )
            .unwrap();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.jump({},1,0,1); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before.btech);
            assert!(lua.drain_outbox().is_empty());
            commands::run(&native, &config, ObjectId(1), 1, "jump 0 1").unwrap();
            lua.eval_callback::<bool>(&format!("return btech.unit.jump({},1,0,1)", id.0))
                .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            assert_eq!(native.drain_outbox(), lua.drain_outbox());
            let mut world = native.world().clone();
            assert!(world.btech.constructed_units()[&id].airborne());
            assert_eq!(
                world.btech.constructed_units()[&id]
                    .heat_rates(&world)
                    .production,
                if last { 3.0 } else { 5.0 }
            );
            let mass = world.btech.constructed_units()[&id].mass().unwrap();
            seed_airborne_critical(&mut world, id, location);
            persistence::save(&config.database(), &world).await.unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            if world.objects[&ObjectId(1)].flags.contains(Flag::Connected) {
                restored
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
            }
            let report = resolve_battle_tactical_impact(
                &mut world,
                id,
                airborne_hit(location.section),
                1,
                jump_rules(),
            )
            .unwrap();
            assert_eq!(
                resolve_battle_tactical_impact(
                    &mut restored,
                    id,
                    airborne_hit(location.section),
                    1,
                    jump_rules()
                )
                .unwrap(),
                report
            );
            assert_eq!(world.btech, restored.btech);
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(
                unit.system_hits(BattleSystem::JumpJet),
                if last { 5 } else { 1 }
            );
            assert_eq!(
                unit.jump_capacity(100).unwrap().movement_points,
                if last { 0 } else { 4 }
            );
            assert_eq!(unit.airborne(), !last);
            assert_eq!(unit.mass().unwrap().equipment, mass.equipment);
            if !last {
                assert_eq!(unit.heat_rates(&world).production, 4.0);
            }
            let observed: Vec<_> = report
                .notices
                .iter()
                .filter(|notice| notice.unit == observer)
                .collect();
            assert_eq!(
                observed
                    .iter()
                    .filter(|notice| notice.text.contains("superheated plasma"))
                    .count(),
                1
            );
            assert_eq!(
                observed
                    .iter()
                    .filter(|notice| notice.text.ends_with("falls from the sky!"))
                    .count(),
                usize::from(last)
            );
            let before_repeat = world.btech.clone();
            for lost in [slot / 2 * 2, slot / 2 * 2 + 1] {
                assert!(
                    destroy_battle_critical(
                        &mut world,
                        id,
                        CriticalLocation {
                            section: location.section,
                            slot: lost
                        }
                    )
                    .unwrap()
                    .is_none()
                );
            }
            assert_eq!(world.btech, before_repeat);
            world.validate(&config).unwrap();
        }
    }
}

/// Jump movement ages charge intent without adding distance; timeout and flight replay survive storage.
#[tokio::test]
async fn airborne_charge_timer_and_saved_replay() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id) = runtime_fixture().await;
    let (target, _) = jump_observer(&mut base, &config, id);
    refresh_battle_contacts(&mut base, &[id]).unwrap();
    select_battle_charge(
        &mut base,
        id,
        ObjectId(1),
        BattleChargeSelection::Target(target),
    )
    .unwrap();
    launch_battle_jump(&mut base, id, ObjectId(1), 0, 2.0).unwrap();
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][id.0.to_string()]["charge"]["elapsed"] = 59.into();
    state["constructed"][id.0.to_string()]["charge"]["distance"] = 3.25.into();
    base.btech = serde_json::from_value(state).unwrap();
    for new_rules in [false, true] {
        let mut world = base.clone();
        let rules = BattleMovementRules {
            fall: jump_rules(),
            charge: BattleChargePolicy {
                new_rules,
                ..BattleChargePolicy::STANDARD
            },
            ..BattleMovementRules::STANDARD
        };
        advance_battle_jumps(&mut world, rules).unwrap();
        let charge = world.btech.constructed_units()[&id].charge();
        assert_eq!(charge.target, Some(target));
        assert_eq!(charge.elapsed, if new_rules { 60 } else { 59 });
        assert_eq!(charge.distance, 3.25);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_jumps(&mut world, rules).unwrap();
        assert_eq!(notices, advance_battle_jumps(&mut loaded, rules).unwrap());
        assert_eq!(world.btech, loaded.btech);
        assert_eq!(
            notices
                .iter()
                .any(|notice| notice.text.contains("timed out")),
            new_rules
        );
        assert_eq!(
            world.btech.constructed_units()[&id].charge(),
            if new_rules {
                BattleChargeState::default()
            } else {
                charge
            }
        );
        world.validate(&config).unwrap();
    }
}

/// In-range airborne attempts reset intent without attack dice, damage or physical recovery.
#[tokio::test]
async fn airborne_charge_rejection_clears_one_or_both_selections() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id) = runtime_fixture().await;
    let (target, _) = jump_observer(&mut base, &config, id);
    let map = base.btech.constructed_units()[&id].position().unwrap().map;
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][target.0.to_string()]["power"] = serde_json::json!({"state":"off"});
    base.btech = serde_json::from_value(state).unwrap();
    place_battle_unit(&mut base, target, map, 5, 5).unwrap();
    refresh_battle_contacts(&mut base, &[id]).unwrap();
    launch_battle_jump(&mut base, id, ObjectId(1), 0, 2.0).unwrap();
    select_battle_charge(
        &mut base,
        id,
        ObjectId(1),
        BattleChargeSelection::Target(target),
    )
    .unwrap();
    for mutual in [false, true] {
        let mut world = base.clone();
        if mutual {
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["constructed"][target.0.to_string()]["charge"]["target"] = id.0.into();
            world.btech = serde_json::from_value(state).unwrap();
        }
        let before = serde_json::to_value(&world.btech).unwrap();
        let notices = advance_battle_jumps(
            &mut world,
            BattleMovementRules {
                fall: jump_rules(),
                ..BattleMovementRules::STANDARD
            },
        )
        .unwrap();
        assert!(
            notices
                .iter()
                .any(|notice| notice.text.contains("airborne")),
            "{notices:?}"
        );
        for unit_id in [id, target] {
            let unit = &world.btech.constructed_units()[&unit_id];
            assert_eq!(unit.charge(), BattleChargeState::default());
            assert!(unit.limb_recycle().is_empty());
            let after = serde_json::to_value(unit).unwrap();
            assert_eq!(
                before["constructed"][unit_id.0.to_string()]["dice"],
                after["dice"]
            );
            assert_eq!(
                before["constructed"][unit_id.0.to_string()]["sections"],
                after["sections"]
            );
        }
        world.validate(&config).unwrap();
    }
}

/// The final jump update checks charge eligibility after landing has cleared airborne state.
#[tokio::test]
async fn airborne_charge_landing_checks_ground_eligibility() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    let (target, _) = jump_observer(&mut world, &config, id);
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][target.0.to_string()]["power"] = serde_json::json!({"state":"off"});
    world.btech = serde_json::from_value(state).unwrap();
    place_battle_unit(&mut world, target, map, 5, 3).unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    let rules = BattleMovementRules {
        fall: jump_rules(),
        ..BattleMovementRules::STANDARD
    };
    // Detect the last step through the public trajectory rather than a hard-coded tick count.
    loop {
        let unit = &world.btech.constructed_units()[&id];
        let mut flight = unit.flight().unwrap();
        let map = &world.btech.maps()[&map];
        if flight
            .advance(
                unit.jump_capacity(map.gravity).unwrap(),
                map.movement_modifier,
            )
            .unwrap()
            .outcome
            == BattleJumpOutcome::Landing
        {
            break;
        }
        advance_battle_jumps(&mut world, rules).unwrap();
    }
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    select_battle_charge(
        &mut world,
        id,
        ObjectId(1),
        BattleChargeSelection::Target(target),
    )
    .unwrap();
    let notices = advance_battle_jumps(&mut world, rules).unwrap();
    assert!(!world.btech.constructed_units()[&id].airborne());
    assert_eq!(
        world.btech.constructed_units()[&id].charge(),
        BattleChargeState::default()
    );
    assert!(
        notices
            .iter()
            .any(|notice| notice.text.contains("finish your jump")),
        "{notices:?}"
    );
    assert!(
        !notices
            .iter()
            .any(|notice| notice.text.contains("airborne"))
    );
    assert!(
        notices
            .iter()
            .any(|notice| notice.text.contains("fast enough")),
        "{notices:?}"
    );
    select_battle_charge(
        &mut world,
        id,
        ObjectId(1),
        BattleChargeSelection::Target(target),
    )
    .unwrap();
    let charge = world.btech.constructed_units()[&id].charge();
    advance_battle_jumps(
        &mut world,
        BattleMovementRules {
            charge: BattleChargePolicy {
                new_rules: true,
                ..BattleChargePolicy::STANDARD
            },
            ..rules
        },
    )
    .unwrap();
    assert_eq!(world.btech.constructed_units()[&id].charge(), charge);
    world.validate(&config).unwrap();
}

/// Targeted flight preserves launch-time range/apex and destination while its target moves.
#[tokio::test]
async fn dfa_launch_fixed_destination_and_saved_intent() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    let (target, _) = jump_observer(&mut world, &config, id);
    let start = world.btech.constructed_units()[&id].motion().unwrap().point;
    let center = world.btech.constructed_units()[&target]
        .motion()
        .unwrap()
        .point;
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][target.0.to_string()]["motion"]["point"] =
        serde_json::to_value(BattlePoint {
            x: center.x + (start.x - center.x) * 0.25,
            y: center.y + (start.y - center.y) * 0.25,
        })
        .unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    select_battle_target(&mut world, id, ObjectId(1), Some(target)).unwrap();
    let before = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap();
    let notices = launch_battle_dfa(&mut world, id, ObjectId(1), None).unwrap();
    assert!(notices[0].text.contains("Death From Above"));
    let flight = world.btech.constructed_units()[&id].flight().unwrap();
    assert_eq!(flight.dfa_target(), Some(target));
    assert_eq!(flight.path().sample(1.0, 5).unwrap().point, center);
    assert_eq!(flight.path().apex(), 3);
    assert_eq!(
        before["dice"],
        serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"]
    );
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][target.0.to_string()]["motion"]["speed"] = 21.5.into();
    state["constructed"][target.0.to_string()]["motion"]["desired_speed"] = 21.5.into();
    world.btech = serde_json::from_value(state).unwrap();
    let rules = BattleMovementRules {
        fall: jump_rules(),
        ..BattleMovementRules::STANDARD
    };
    advance_battle_motion(&mut world, rules).unwrap();
    advance_battle_jumps(&mut world, rules).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id]
            .flight()
            .unwrap()
            .path(),
        flight.path()
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech, loaded.btech);
    assert_eq!(
        advance_battle_jumps(&mut world, rules).unwrap(),
        advance_battle_jumps(&mut loaded, rules).unwrap()
    );
    assert_eq!(world.btech, loaded.btech);
    assert_eq!(
        loaded.btech.constructed_units()[&id]
            .flight()
            .unwrap()
            .dfa_target(),
        Some(target)
    );
    world.validate(&config).unwrap();
}

/// Target selection failures are atomic and cancellation removes intent with the flight.
#[tokio::test]
async fn dfa_launch_rejection_and_shutdown() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    let (target, _) = jump_observer(&mut world, &config, id);
    let before = world.btech.clone();
    for selected in [None, Some(id), Some(ObjectId(999999))] {
        assert!(launch_battle_dfa(&mut world, id, ObjectId(1), selected).is_err());
        assert_eq!(world.btech, before);
    }
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    launch_battle_dfa(&mut world, id, ObjectId(1), Some(target)).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id]
            .flight()
            .unwrap()
            .dfa_target(),
        Some(target)
    );
    let mut invalid = world.clone();
    let mut state = serde_json::to_value(&invalid.btech).unwrap();
    state["constructed"][id.0.to_string()]["flight"]["dfa_target"] = id.0.into();
    invalid.btech = serde_json::from_value(state).unwrap();
    assert!(invalid.validate(&config).is_err());
    let before = world.btech.clone();
    assert!(launch_battle_dfa(&mut world, id, ObjectId(1), Some(target)).is_err());
    assert_eq!(world.btech, before);
    stop_battle_unit(&mut world, id, ObjectId(1), jump_rules()).unwrap();
    assert!(world.btech.constructed_units()[&id].flight().is_none());
    world.validate(&config).unwrap();
}

/// Stored targeted paths reject contradictory admission modes and invalid ranges.
#[test]
fn dfa_launch_path_validation() {
    use stompymux_rs::*;
    let start = BattleHexCoordinate { x: 5, y: 5 }.center();
    let end = BattleHexCoordinate { x: 6, y: 5 }.center();
    for range in [f64::NAN, f64::INFINITY, 0.0, -1.0, 6.0] {
        assert!(BattleJumpPath::targeted(start, end, range, 0, 0, 5).is_err());
    }
    let path = BattleJumpPath::targeted(start, end, 0.75, 0, 0, 5).unwrap();
    let mut stored = serde_json::to_value(path).unwrap();
    stored["projection"] = serde_json::json!({"bearing":0,"range":0.75});
    assert!(serde_json::from_value::<BattleJumpPath>(stored).is_err());
    let stored = serde_json::to_value(path).unwrap();
    assert_eq!(
        path,
        serde_json::from_value::<BattleJumpPath>(stored).unwrap()
    );
}

/// A targeted jump dispatches exactly one landing attack with airborne aim and no ordinary stabilization.
#[tokio::test]
async fn dfa_landing_dispatch_and_saved_replay() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    let (target, _) = jump_observer(&mut world, &config, id);
    // A connected skilled pilot isolates the landing attack from incidental control failure.
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    set_battle_character(
        &mut world,
        ObjectId(1),
        BattleCharacter {
            build: 5,
            reflexes: 4,
            intuition: 3,
            learn: 2,
            charisma: 1,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Piloting-Biped",
        BattleCharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    launch_battle_dfa(&mut world, id, ObjectId(1), Some(target)).unwrap();
    let rules = BattleMovementRules {
        physical_pilot_skill: false,
        fall: jump_rules(),
        ..BattleMovementRules::STANDARD
    };
    loop {
        let unit = &world.btech.constructed_units()[&id];
        let mut flight = unit.flight().unwrap();
        let map = &world.btech.maps()[&unit.position().unwrap().map];
        if flight
            .advance(
                unit.jump_capacity(map.gravity).unwrap(),
                map.movement_modifier,
            )
            .unwrap()
            .outcome
            == BattleJumpOutcome::Landing
        {
            break;
        }
        advance_battle_jumps(&mut world, rules).unwrap();
    }
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    loaded
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let notices = advance_battle_jumps(&mut world, rules).unwrap();
    assert_eq!(notices, advance_battle_jumps(&mut loaded, rules).unwrap());
    assert_eq!(world.btech, loaded.btech);
    assert_eq!(
        notices
            .iter()
            .filter(|notice| notice.text.contains("DFA: BTH 8"))
            .count(),
        1,
        "{notices:?}"
    );
    assert!(
        !notices
            .iter()
            .any(|notice| notice.text.contains("finish your jump")
                || notice.text.contains("lands gracefully"))
    );
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.flight().is_none());
    assert_eq!(unit.jump_stabilization(), 0);
    assert_eq!(unit.limb_recycle().len(), 6);
    assert_eq!(unit.posture(), BattlePosture::Prone);
    assert!(advance_battle_jumps(&mut world, rules).unwrap().is_empty());
    world.validate(&config).unwrap();
}

/// A moved target cancels the attack and completes a normal landing without consuming an attack roll.
#[tokio::test]
async fn dfa_landing_moved_target_falls_back() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    let (target, _) = jump_observer(&mut world, &config, id);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    launch_battle_dfa(&mut world, id, ObjectId(1), Some(target)).unwrap();
    let map = world.btech.constructed_units()[&target]
        .position()
        .unwrap()
        .map;
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][target.0.to_string()]["power"] = serde_json::json!({"state":"off"});
    world.btech = serde_json::from_value(state).unwrap();
    place_battle_unit(&mut world, target, map, 9, 9).unwrap();
    let before =
        serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"].clone();
    let rules = BattleMovementRules {
        physical_pilot_skill: false,
        fall: jump_rules(),
        ..BattleMovementRules::STANDARD
    };
    let mut notices = Vec::new();
    while world.btech.constructed_units()[&id].flight().is_some() {
        notices.extend(advance_battle_jumps(&mut world, rules).unwrap());
    }
    assert!(
        notices
            .iter()
            .any(|notice| notice.text.contains("target has moved")),
        "{notices:?}"
    );
    assert!(
        notices
            .iter()
            .any(|notice| notice.text.contains("finish your jump"))
    );
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.jump_stabilization(), 12);
    assert!(unit.limb_recycle().is_empty());
    assert_eq!(serde_json::to_value(unit).unwrap()["dice"], before);
    world.validate(&config).unwrap();
}

/// Native and Lua DFA selection share default/explicit targeting and transactional output.
#[tokio::test]
async fn dfa_landing_commands_and_rollback() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id) = runtime_fixture().await;
    let (target, _) = jump_observer(&mut base, &config, id);
    refresh_battle_contacts(&mut base, &[id]).unwrap();
    select_battle_target(&mut base, id, ObjectId(1), Some(target)).unwrap();
    for explicit in [false, true] {
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        let call = format!(
            "btech.unit.dfa({},1,{})",
            id.0,
            if explicit {
                target.0.to_string()
            } else {
                "nil".into()
            }
        );
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, base.btech);
        assert!(lua.drain_outbox().is_empty());
        commands::run(
            &native,
            &config,
            ObjectId(1),
            1,
            &if explicit {
                format!("jump #{}", target.0)
            } else {
                "jump".into()
            },
        )
        .unwrap();
        assert!(
            lua.eval_callback::<bool>(&format!("return {call}"))
                .unwrap()
        );
        assert_eq!(native.world().btech, lua.world().btech);
        let messages = |scripts: &Scripts| {
            scripts
                .drain_outbox()
                .into_iter()
                .map(|(to, message)| (to, message.source().to_owned()))
                .collect::<Vec<_>>()
        };
        assert_eq!(messages(&native), messages(&lua));
        assert_eq!(
            native.world().btech.constructed_units()[&id]
                .flight()
                .unwrap()
                .dfa_target(),
            Some(target)
        );
    }
}

/// A controlled early landing in the target hex dispatches DFA once using configured pilot aim.
#[tokio::test]
async fn dfa_landing_early_attack_uses_shared_policy() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    let (target, _) = jump_observer(&mut world, &config, id);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    set_battle_character(
        &mut world,
        ObjectId(1),
        BattleCharacter {
            build: 5,
            reflexes: 4,
            intuition: 3,
            learn: 2,
            charisma: 1,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Piloting-Biped",
        BattleCharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    launch_battle_dfa(&mut world, id, ObjectId(1), Some(target)).unwrap();
    let rules = BattleMovementRules {
        physical_pilot_skill: true,
        fall: jump_rules(),
        ..BattleMovementRules::STANDARD
    };
    while world.btech.constructed_units()[&id].position()
        != world.btech.constructed_units()[&target].position()
    {
        advance_battle_jumps(&mut world, rules).unwrap();
    }
    assert!(world.btech.constructed_units()[&id].flight().is_some());
    let notices = land_battle_jump(&mut world, id, ObjectId(1), rules).unwrap();
    assert_eq!(
        notices
            .iter()
            .filter(|notice| notice.text.contains("DFA: BTH"))
            .count(),
        1,
        "{notices:?}"
    );
    assert!(
        notices
            .iter()
            .any(|notice| notice.text.contains("legs first")),
        "{notices:?}"
    );
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.flight().is_none());
    assert_eq!(unit.jump_stabilization(), 0);
    assert_eq!(unit.limb_recycle().len(), 6);
    world.validate(&config).unwrap();
}

/// The server's airborne action retries character impacts without losing health, dice or crew moves.
#[tokio::test]
async fn character_free_fall_action_replays_and_rolls_back_casualties() {
    use stompymux_rs::*;
    for fatal in [false, true] {
        let (_dir, config, mut world, id) = runtime_fixture().await;
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).unwrap();
        stop_battle_unit(&mut world, id, ObjectId(1), jump_rules()).unwrap();
        let movement = BattleMovementRules {
            fall: jump_rules(),
            ..BattleMovementRules::STANDARD
        };
        for _ in 0..2 {
            advance_battle_jumps(&mut world, movement).unwrap();
        }
        let pilot = ObjectId(2);
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, pilot).unwrap();
        set_battle_character(
            &mut world,
            pilot,
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: if fatal { 50 } else { 0 },
                lethal: if fatal { 40 } else { 0 },
            },
        )
        .unwrap();
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        let seed = (0..=255)
            .find(|byte| BattleDice::seeded([*byte; 32]).two_d6() == 2)
            .unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        let baseline = world.clone();
        assert!(advance_battle_jumps(&mut world, movement).is_err());
        assert_eq!(world.btech, baseline.btech);
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        if fatal {
            scripts.world_mut().objects.remove(&afterlife);
            assert!(advance_battle_jumps_action(&scripts, &config, movement).is_err());
            assert_eq!(scripts.world().btech, baseline.btech);
            assert_eq!(scripts.world().objects[&pilot].location, Some(id));
            assert!(scripts.drain_outbox().is_empty());
            *scripts.world_mut() = baseline.clone();
        }
        persistence::save(&config.database(), &baseline)
            .await
            .unwrap();
        let replay = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(
                persistence::load(&config.database()).await.unwrap(),
            )),
        )
        .unwrap();
        advance_battle_jumps_action(&scripts, &config, movement).unwrap();
        advance_battle_jumps_action(&replay, &config, movement).unwrap();
        assert_eq!(scripts.world().btech, replay.world().btech);
        let candidate = scripts.world().clone();
        let unit = &candidate.btech.constructed_units()[&id];
        assert!(unit.free_fall().is_none());
        assert_eq!(unit.posture(), BattlePosture::Prone);
        assert_eq!(unit.character_pilot_status().unwrap().killed, fatal);
        assert_eq!(
            candidate.objects[&pilot].location,
            Some(if fatal { afterlife } else { id })
        );
        candidate.validate(&config).unwrap();
        persistence::save(&config.database(), &candidate)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            candidate.btech
        );
    }
}

/// Character landing immersion publishes a flooded cockpit casualty under the airborne checkpoint.
#[tokio::test]
async fn character_water_landing_evacuates_and_retries() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = water_jump_fixture(3, false).await;
    apply_damage_phase(
        &mut world,
        id,
        BattleSection::Head,
        7,
        BattleDamagePhase::Armor { rear: false },
    )
    .unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 3.0).unwrap();
    let movement = BattleMovementRules {
        fall: jump_rules(),
        ..BattleMovementRules::STANDARD
    };
    for second in 0..60 {
        let mut next = world.clone();
        advance_battle_jumps(&mut next, movement).unwrap();
        if next.btech.constructed_units()[&id].flight().is_none() {
            break;
        }
        assert!(second < 59, "Jump did not reach its landing");
        world = next;
    }
    release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    let pilot = ObjectId(2);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&pilot).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, pilot).unwrap();
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    set_battle_character(
        &mut world,
        pilot,
        BattleCharacter {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    let baseline = world.clone();
    assert!(advance_battle_jumps(&mut world, movement).is_err());
    assert_eq!(world.btech, baseline.btech);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    scripts.world_mut().objects.remove(&afterlife);
    assert!(advance_battle_jumps_action(&scripts, &config, movement).is_err());
    assert_eq!(scripts.world().btech, baseline.btech);
    assert_eq!(scripts.world().objects[&pilot].location, Some(id));
    assert!(scripts.drain_outbox().is_empty());
    persistence::save(&config.database(), &baseline)
        .await
        .unwrap();
    let replay = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(
            persistence::load(&config.database()).await.unwrap(),
        )),
    )
    .unwrap();
    *scripts.world_mut() = baseline;
    advance_battle_jumps_action(&scripts, &config, movement).unwrap();
    advance_battle_jumps_action(&replay, &config, movement).unwrap();
    assert_eq!(scripts.world().btech, replay.world().btech);
    let candidate = scripts.world().clone();
    let unit = &candidate.btech.constructed_units()[&id];
    assert!(unit.flight().is_none());
    assert!(unit.flooded_sections().contains(&BattleSection::Head));
    assert!(unit.is_destroyed());
    assert_eq!(candidate.objects[&pilot].location, Some(afterlife));
    candidate.validate(&config).unwrap();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        candidate.btech
    );
}

/// Landing dispatch publishes character head casualties once and restores the whole tick on failure.
#[tokio::test]
async fn character_dfa_landing_dispatch_rolls_back_and_replays() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    let (target, _) = jump_observer(&mut world, &config, id);
    // A connected skilled pilot isolates the landing attack from incidental control failure.
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    set_battle_character(
        &mut world,
        ObjectId(1),
        BattleCharacter {
            build: 5,
            reflexes: 4,
            intuition: 3,
            learn: 2,
            charisma: 1,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Piloting-Biped",
        BattleCharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    launch_battle_dfa(&mut world, id, ObjectId(1), Some(target)).unwrap();
    let rules = BattleMovementRules {
        physical_pilot_skill: false,
        fall: jump_rules(),
        ..BattleMovementRules::STANDARD
    };
    loop {
        let unit = &world.btech.constructed_units()[&id];
        let mut flight = unit.flight().unwrap();
        let map = &world.btech.maps()[&unit.position().unwrap().map];
        if flight
            .advance(
                unit.jump_capacity(map.gravity).unwrap(),
                map.movement_modifier,
            )
            .unwrap()
            .outcome
            == BattleJumpOutcome::Landing
        {
            break;
        }
        advance_battle_jumps(&mut world, rules).unwrap();
    }

    let pilot = ObjectId(2);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    world.objects.get_mut(&pilot).unwrap().location = Some(target);
    assign_battle_pilot(&mut world, target, pilot).unwrap();
    world
        .objects
        .get_mut(&target)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    set_battle_character(
        &mut world,
        pilot,
        BattleCharacter {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 50,
            lethal: 40,
        },
    )
    .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    for unit in [id, target] {
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = BattleDice::seeded([*seed; 32]);
                if unit == id {
                    dice.two_d6() == 12
                } else {
                    dice.d6() == 6
                }
            })
            .unwrap();
        state["constructed"][unit.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    }
    world.btech = serde_json::from_value(state).unwrap();
    let baseline = world.clone();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    scripts.world_mut().objects.remove(&afterlife);
    assert!(advance_battle_jumps_action(&scripts, &config, rules).is_err());
    assert_eq!(scripts.world().btech, baseline.btech);
    assert_eq!(scripts.world().objects[&pilot].location, Some(target));
    assert!(scripts.drain_outbox().is_empty());
    persistence::save(&config.database(), &baseline)
        .await
        .unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    for player in [ObjectId(1), pilot] {
        restored.objects.get_mut(&player).unwrap().flags = baseline.objects[&player].flags.clone();
    }
    let replay =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    *scripts.world_mut() = baseline;
    advance_battle_jumps_action(&scripts, &config, rules).unwrap();
    advance_battle_jumps_action(&replay, &config, rules).unwrap();
    assert_eq!(scripts.world().btech, replay.world().btech);
    assert_eq!(scripts.world().objects[&pilot].location, Some(afterlife));
    let notices = scripts.drain_outbox();
    assert_eq!(
        notices
            .iter()
            .filter(|(_, message)| message.source().starts_with("DFA: BTH"))
            .count(),
        1
    );
    let candidate = scripts.world().clone();
    assert!(candidate.btech.constructed_units()[&target].is_destroyed());
    assert!(candidate.btech.constructed_units()[&id].flight().is_none());
    assert_eq!(
        candidate.btech.constructed_units()[&id]
            .limb_recycle()
            .len(),
        6
    );
    candidate.validate(&config).unwrap();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        candidate.btech
    );
}

/// Character native/Lua jump selection completes ordinary and DFA flights with transactional output.
#[tokio::test]
async fn character_jump_commands_land_and_rollback() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id) = runtime_fixture().await;
    let (target, _) = jump_observer(&mut base, &config, id);
    refresh_battle_contacts(&mut base, &[id]).unwrap();
    select_battle_target(&mut base, id, ObjectId(1), Some(target)).unwrap();
    for (unit, pilot) in [(id, ObjectId(1)), (target, ObjectId(2))] {
        base.objects
            .get_mut(&unit)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        base.objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        base.objects.get_mut(&pilot).unwrap().location = Some(unit);
        assign_battle_pilot(&mut base, unit, pilot).unwrap();
        set_battle_character(
            &mut base,
            pilot,
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: 0,
                lethal: 0,
            },
        )
        .unwrap();
        for name in ["Piloting-Biped", "Piloting-Battlemech"] {
            set_battle_character_value(
                &mut base,
                pilot,
                name,
                BattleCharacterValue {
                    value: 30,
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
        }
    }
    for mode in 0..3 {
        let explicit = mode == 1;
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        let call = match mode {
            0 => format!("btech.unit.dfa({},1,nil)", id.0),
            1 => format!("btech.unit.dfa({},1,{})", id.0, target.0),
            _ => format!("btech.unit.jump({},1,0,1)", id.0),
        };
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, base.btech);
        assert!(lua.drain_outbox().is_empty());
        commands::run(
            &native,
            &config,
            ObjectId(1),
            1,
            &if explicit {
                format!("jump #{}", target.0)
            } else if mode == 0 {
                "jump".into()
            } else {
                "jump 0 1".into()
            },
        )
        .unwrap();
        assert!(
            lua.eval_callback::<bool>(&format!("return {call}"))
                .unwrap()
        );
        assert_eq!(native.world().btech, lua.world().btech);
        let messages = |scripts: &Scripts| {
            scripts
                .drain_outbox()
                .into_iter()
                .map(|(to, message)| (to, message.source().to_owned()))
                .collect::<Vec<_>>()
        };
        assert_eq!(messages(&native), messages(&lua));
        assert_eq!(
            native.world().btech.constructed_units()[&id]
                .flight()
                .unwrap()
                .dfa_target(),
            (mode != 2).then_some(target)
        );
        let rules = BattleMovementRules {
            physical_pilot_skill: false,
            fall: jump_rules(),
            ..BattleMovementRules::STANDARD
        };
        for _ in 0..100 {
            if native.world().btech.constructed_units()[&id]
                .flight()
                .is_none()
            {
                break;
            }
            advance_battle_jumps_action(&native, &config, rules).unwrap();
            advance_battle_jumps_action(&lua, &config, rules).unwrap();
        }
        assert!(
            native.world().btech.constructed_units()[&id]
                .flight()
                .is_none()
        );
        assert_eq!(native.world().btech, lua.world().btech);
        assert_eq!(messages(&native), messages(&lua));
        let candidate = native.world().clone();
        candidate.validate(&config).unwrap();
        persistence::save(&config.database(), &candidate)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            candidate.btech
        );
    }
}

/// Automatic damaged-gear landings publish control XP atomically with flight completion and stabilization.
#[tokio::test]
async fn character_landing_control_experience_and_restart() {
    use stompymux_rs::*;
    for obstacle in [false, true] {
        for extended in [false, true] {
            for (section, slot, skill_level) in [
                (BattleSection::LeftLeg, 1, 0),
                (BattleSection::LeftLeg, 0, 1),
                (BattleSection::CenterTorso, 3, 2),
            ] {
                let (_dir, config, mut world, id) = runtime_fixture().await;
                if obstacle {
                    jump_hills(&mut world, id, 0, 0, 9);
                }
                world
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
                world
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
                set_battle_character(
                    &mut world,
                    ObjectId(1),
                    BattleCharacter {
                        build: 5,
                        reflexes: 5,
                        intuition: 5,
                        learn: 5,
                        charisma: 5,
                        bruise: 0,
                        lethal: 0,
                    },
                )
                .unwrap();
                let skill = if extended {
                    "Piloting-Biped"
                } else {
                    "Piloting-Battlemech"
                };
                set_battle_character_value(
                    &mut world,
                    ObjectId(1),
                    skill,
                    BattleCharacterValue {
                        value: skill_level,
                        experience: 0,
                        last_used: 0,
                    },
                )
                .unwrap();
                launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
                destroy_battle_critical(&mut world, id, CriticalLocation { section, slot })
                    .unwrap();
                let seed = (0..=255)
                    .find(|seed| {
                        let mut dice = BattleDice::seeded([*seed; 32]);
                        dice.two_d6() == 12 && dice.two_d6() == 12
                    })
                    .unwrap();
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["constructed"][id.0.to_string()]["dice"] =
                    serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
                world.btech = serde_json::from_value(state).unwrap();
                let mut channel = Channel::new("MechPilotXP".into());
                channel.users.push(communication::Membership {
                    who: ObjectId(1),
                    listening: true,
                });
                channel.messages = if obstacle { i64::MAX - 1 } else { i64::MAX };
                world.channels.insert("MechPilotXP".into(), channel);
                let scripts =
                    Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world)))
                        .unwrap();
                let rules = BattleMovementRules {
                    fall: BattleFallRules {
                        extended_piloting: extended,
                        ..jump_rules()
                    },
                    ..BattleMovementRules::STANDARD
                };
                let mut rejected = false;
                let mut restarted = false;
                for tick in 0..80 {
                    let before = scripts.world().clone();
                    if advance_battle_jumps_action(&scripts, &config, rules).is_err() {
                        assert!(!rejected);
                        rejected = true;
                        assert_eq!(scripts.world().btech, before.btech);
                        assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
                        assert!(scripts.drain_outbox().is_empty());
                        scripts
                            .world_mut()
                            .channels
                            .get_mut("MechPilotXP")
                            .unwrap()
                            .messages = 0;
                        advance_battle_jumps_action(&scripts, &config, rules).unwrap();
                    }
                    if tick == 1 {
                        let saved = scripts.world().clone();
                        persistence::save(&config.database(), &saved).await.unwrap();
                        let mut loaded = persistence::load(&config.database()).await.unwrap();
                        assert_eq!(loaded.btech, saved.btech);
                        loaded
                            .objects
                            .get_mut(&ObjectId(1))
                            .unwrap()
                            .flags
                            .insert(Flag::Connected);
                        *scripts.world_mut() = loaded;
                        restarted = true;
                    }
                    if !scripts.world().btech.constructed_units()[&id].airborne() {
                        break;
                    }
                    scripts.drain_outbox();
                }
                assert!(rejected && restarted);
                let output = scripts.drain_outbox();
                let pilot_output: Vec<_> = output
                    .iter()
                    .filter(|(who, _)| *who == ObjectId(1))
                    .map(|(_, text)| text.source())
                    .collect();
                let rolls: Vec<_> = pilot_output
                    .iter()
                    .enumerate()
                    .filter_map(|(index, text)| {
                        (*text == "You make a piloting skill roll!").then_some(index)
                    })
                    .collect();
                assert_eq!(rolls.len(), if obstacle { 2 } else { 1 });
                for &index in &rolls {
                    assert!(index > 0);
                    assert!(pilot_output[index + 1].starts_with("Modified Pilot Skill: BTH "));
                    assert!(pilot_output[index + 1].ends_with("\tRoll: 12"));
                }
                let gear = *rolls.last().unwrap();
                assert_eq!(
                    pilot_output[gear - 1],
                    if section == BattleSection::CenterTorso {
                        "Your damaged gyro makes it harder to land"
                    } else {
                        "Your damaged leg actuators make it harder to land"
                    }
                );
                if obstacle {
                    assert!(
                        pilot_output[rolls[0] - 1]
                            .contains("You attempt to jump over elevation that is too high!")
                    );
                    assert!(pilot_output[rolls[0] + 2].contains("You land safely."));
                }
                assert!(!output.iter().any(|(who, text)| *who != ObjectId(1)
                    && (text.source().starts_with("You make a piloting")
                        || text.source().starts_with("Modified Pilot Skill:"))));
                let candidate = scripts.world().clone();
                assert!(!candidate.btech.constructed_units()[&id].airborne());
                assert_eq!(
                    candidate.btech.constructed_units()[&id].posture(),
                    BattlePosture::Standing
                );
                assert_eq!(
                    candidate.btech.character_values()[&ObjectId(1)][skill].experience_balance(),
                    if obstacle { 4 } else { 2 }
                );
                assert_eq!(
                    candidate.channels["MechPilotXP"].history.len(),
                    if obstacle { 2 } else { 1 }
                );
                assert_eq!(
                    text::plain_with(
                        scripts.palette(),
                        &candidate.channels["MechPilotXP"].history[0].message
                    ),
                    format!("[MechPilotXP] GOD gained 2 {skill} XP")
                );
                persistence::save(&config.database(), &candidate)
                    .await
                    .unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    candidate.btech
                );
            }
        }
    }
}

/// Native and Lua early landing share character XP, injuries, nested gear checks and fatal rollback.
#[tokio::test]
async fn character_manual_landing_adapters_and_casualty_rollback() {
    use stompymux_rs::*;
    fn stable(value: &impl serde::Serialize) -> serde_json::Value {
        fn clear(value: &mut serde_json::Value) {
            match value {
                serde_json::Value::Object(fields) => {
                    for (name, value) in fields {
                        if name == "last_used" {
                            *value = 0.into();
                        } else {
                            clear(value);
                        }
                    }
                }
                serde_json::Value::Array(values) => {
                    for value in values {
                        clear(value);
                    }
                }
                _ => (),
            }
        }
        let mut result = serde_json::to_value(value).unwrap();
        clear(&mut result);
        result
    }
    for extended in [false, true] {
        for case in ["success", "gear", "failure", "fatal"] {
            let (dir, _config, mut world, id) = runtime_fixture().await;
            let path = dir.path().join("stompymux.toml");
            let mut settings: toml::Value =
                toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            settings["battletech"]
                .as_table_mut()
                .unwrap()
                .insert("extended_piloting".into(), i64::from(extended).into());
            std::fs::write(path, toml::to_string(&settings).unwrap()).unwrap();
            let config = Config::load(dir.path()).unwrap();
            let pilot = ObjectId(2);
            world.objects.get_mut(&pilot).unwrap().location = Some(id);
            world
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .insert(Flag::Connected);
            world
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .remove(Flag::Wizard);
            release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
            assign_battle_pilot(&mut world, id, pilot).unwrap();
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            let successful = matches!(case, "success" | "gear");
            set_battle_character(
                &mut world,
                pilot,
                BattleCharacter {
                    build: 5,
                    reflexes: if successful { 4 } else { 0 },
                    intuition: if successful { 5 } else { 0 },
                    learn: 5,
                    charisma: 5,
                    bruise: if case == "fatal" { 50 } else { 0 },
                    lethal: if case == "fatal" { 40 } else { 0 },
                },
            )
            .unwrap();
            launch_battle_jump(&mut world, id, pilot, 0, 2.0).unwrap();
            if case == "gear" {
                destroy_battle_critical(
                    &mut world,
                    id,
                    CriticalLocation {
                        section: BattleSection::LeftLeg,
                        slot: 1,
                    },
                )
                .unwrap();
            }
            let dice = (0..=u16::MAX)
                .find_map(|seed| {
                    let mut bytes = [0; 32];
                    bytes[..2].copy_from_slice(&seed.to_le_bytes());
                    let dice = BattleDice::seeded(bytes);
                    let mut probe = dice.clone();
                    (probe.two_d6() == 12 && probe.two_d6() == 12).then_some(dice)
                })
                .unwrap();
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["constructed"][id.0.to_string()]["dice"] = serde_json::to_value(dice).unwrap();
            world.btech = serde_json::from_value(state).unwrap();
            let mut channel = Channel::new("MechPilotXP".into());
            channel.users.push(communication::Membership {
                who: pilot,
                listening: true,
            });
            world.channels.insert("MechPilotXP".into(), channel);
            let before = world.clone();
            assert!(
                land_battle_jump(&mut world, id, pilot, BattleMovementRules::STANDARD).is_err()
            );
            assert_eq!(world.btech, before.btech);
            let native =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let lua = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(before.clone())),
            )
            .unwrap();
            let call = format!("btech.unit.land({},2)", id.0);
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, before.btech);
            assert!(lua.world().channels["MechPilotXP"].history.is_empty());
            assert!(lua.drain_outbox().is_empty());
            if successful {
                lua.world_mut()
                    .channels
                    .get_mut("MechPilotXP")
                    .unwrap()
                    .messages = if case == "gear" {
                    i64::MAX - 1
                } else {
                    i64::MAX
                };
                assert!(
                    lua.eval_callback::<bool>(&format!("return {call}"))
                        .is_err()
                );
                assert_eq!(lua.world().btech, before.btech);
                assert!(lua.world().channels["MechPilotXP"].history.is_empty());
                assert!(lua.drain_outbox().is_empty());
                *lua.world_mut() = before.clone();
            }
            let afterlife = ObjectId(config.battletech.afterlife_dbref);
            if case == "fatal" {
                lua.world_mut().objects.remove(&afterlife);
                assert!(
                    lua.eval_callback::<bool>(&format!("return {call}"))
                        .is_err()
                );
                assert_eq!(lua.world().btech, before.btech);
                assert_eq!(lua.world().objects[&pilot].location, Some(id));
                assert!(lua.drain_outbox().is_empty());
                *lua.world_mut() = before.clone();
            }
            commands::run(&native, &config, pilot, 1, "land").unwrap();
            assert!(
                lua.eval_callback::<bool>(&format!("return {call}"))
                    .unwrap()
            );
            assert_eq!(stable(&native.world().btech), stable(&lua.world().btech));
            let output = |scripts: &Scripts| {
                scripts
                    .drain_outbox()
                    .into_iter()
                    .map(|(who, text)| (who, text.source().to_owned()))
                    .collect::<Vec<_>>()
            };
            assert_eq!(output(&native), output(&lua));
            let candidate = native.world().clone();
            let unit = &candidate.btech.constructed_units()[&id];
            assert!(!unit.airborne());
            assert_eq!(unit.is_destroyed(), case == "fatal");
            assert_eq!(
                candidate.objects[&pilot].location,
                Some(if case == "fatal" { afterlife } else { id })
            );
            let skill = if extended {
                "Piloting-Biped"
            } else {
                "Piloting-Battlemech"
            };
            let earned = candidate
                .btech
                .character_values()
                .get(&pilot)
                .and_then(|values| values.get(skill))
                .copied()
                .unwrap_or_default()
                .experience_balance();
            assert_eq!(
                earned,
                if case == "gear" {
                    4
                } else if successful {
                    2
                } else {
                    0
                }
            );
            assert_eq!(
                candidate.channels["MechPilotXP"].history.len(),
                if case == "gear" {
                    2
                } else {
                    usize::from(successful)
                }
            );
            persistence::save(&config.database(), &candidate)
                .await
                .unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                candidate.btech
            );
        }
    }
}

#[tokio::test]
async fn named_jump_fields_retain_launch_after_landing_and_restart() {
    use std::{cell::RefCell, rc::Rc};
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    let before = world.btech.clone();
    assert!(launch_battle_jump(&mut world, id, ObjectId(1), 180, 1.0).is_err());
    assert_eq!(world.btech, before);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report =
        view_battle_unit_fields_action(&scripts, &config, ObjectId(1), id, "jump").unwrap();
    assert_eq!(report.fields.len(), 2);
    assert_eq!(report.fields[0].value.as_deref(), Some("0"));
    assert_eq!(report.fields[1].value.as_deref(), Some("645"));
    let lua: mlua::Table = scripts
        .eval_callback(&format!("return btech.unit.fields(1,{},'jump')", id.0))
        .unwrap();
    assert_eq!(
        serde_json::to_value(lua).unwrap(),
        serde_json::to_value(&report).unwrap()
    );
    let mut landed = scripts.world().clone();
    for _ in 0..100 {
        if landed.btech.constructed_units()[&id].flight().is_none() {
            break;
        }
        advance_battle_jumps(&mut landed, BattleMovementRules::STANDARD).unwrap();
    }
    assert!(landed.btech.constructed_units()[&id].flight().is_none());
    persistence::save(&config.database(), &landed)
        .await
        .unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, landed.btech);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
    let final_report =
        view_battle_unit_fields_action(&scripts, &config, ObjectId(1), id, "jump").unwrap();
    assert_eq!(final_report, report);
}

/// Field edits share a remaining route, retain the live sample and replay through landing.
#[tokio::test]
async fn jump_course_fields_redirect_without_moving_the_committed_cursor() {
    use std::{cell::RefCell, rc::Rc};
    use stompymux_rs::*;
    for chassis in ["Biped", "Quad"] {
        let (_dir, config, world, id) = runtime_fixture().await;
        let setup = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(&setup, &config, ObjectId(1), id, "mechmovetype", chassis)
            .unwrap();
        for (field, value) in [("jumpheading", "123"), ("jumplength", "-3")] {
            set_battle_unit_field_action(&setup, &config, ObjectId(1), id, field, value).unwrap();
            let report =
                view_battle_unit_fields_action(&setup, &config, ObjectId(1), id, field).unwrap();
            assert_eq!(report.fields[0].value.as_deref(), Some(value));
            assert!(
                setup.world().btech.constructed_units()[&id]
                    .flight()
                    .is_none()
            );
        }
        let mut world = setup.world().clone();
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 4.0).unwrap();
        let rules = BattleMovementRules {
            fall: jump_rules(),
            ..BattleMovementRules::STANDARD
        };
        for _ in 0..10 {
            advance_battle_jumps(&mut world, rules).unwrap();
        }
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for (field, value) in [
            ("jumpheading", "90"),
            ("jumplength", "900"),
            ("jumpheading", "180"),
            ("jumplength", "1000"),
        ] {
            let before = lua.world().btech.clone();
            let cursor = before.constructed_units()[&id].flight().unwrap();
            lua.drain_outbox();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'{field}','{value}'); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            commands::run(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("@setmech {field} {value}"),
            )
            .unwrap();
            lua.eval_callback::<()>(&format!(
                "btech.unit.set_field(1,{},'{field}','{value}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            let current = lua.world().btech.constructed_units()[&id].flight().unwrap();
            assert_eq!(current.sample(), cursor.sample());
            assert_eq!(current.total_travelled(), cursor.total_travelled());
            let after = lua.world().btech.clone();
            let actual = serde_json::to_value(&after).unwrap();
            let mut expected = serde_json::to_value(&before).unwrap();
            for key in ["flight", "last_jump"] {
                expected["constructed"][id.0.to_string()][key] =
                    actual["constructed"][id.0.to_string()][key].clone();
            }
            assert_eq!(expected, actual);

            set_battle_unit_field_action(&lua, &config, ObjectId(1), id, field, value).unwrap();
            assert_eq!(lua.world().btech, after);
            let saved = lua.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
        let before = lua.world().btech.clone();
        for (field, value) in [
            ("jumpheading", "360"),
            ("jumpheading", "-1"),
            ("jumplength", "32768"),
            ("jumplength", "30000"),
            ("jumplength", "NaN"),
        ] {
            assert!(
                set_battle_unit_field_action(&lua, &config, ObjectId(1), id, field, value).is_err()
            );
            assert_eq!(lua.world().btech, before);
        }
        let mut world = lua.world().clone();
        let cursor = world.btech.constructed_units()[&id].flight().unwrap();
        let notices = advance_battle_jumps(&mut world, rules).unwrap();
        assert!(
            notices
                .iter()
                .all(|notice| !notice.text.contains("finish your jump"))
        );
        let next = world.btech.constructed_units()[&id].flight().unwrap();
        assert_ne!(next.sample().point, cursor.sample().point);
        assert_ne!(next.sample().elevation, cursor.sample().elevation);
        let mut replay = lua.world().clone();
        assert_eq!(advance_battle_jumps(&mut replay, rules).unwrap(), notices);
        assert_eq!(replay.btech, world.btech);
        for _ in 0..120 {
            if world.btech.constructed_units()[&id].flight().is_none() {
                break;
            }
            advance_battle_jumps(&mut world, rules).unwrap();
        }
        assert!(world.btech.constructed_units()[&id].flight().is_none());
        world.validate(&config).unwrap();
    }
}

/// A nonpositive or already-completed length schedules ordinary landing at the current location.
#[tokio::test]
async fn shortened_jump_fields_land_on_the_next_tick_after_restart() {
    use std::{cell::RefCell, rc::Rc};
    use stompymux_rs::*;
    for length in ["0", "-32768", "1", "270"] {
        let (_dir, config, mut world, id) = runtime_fixture().await;
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 4.0).unwrap();
        let rules = BattleMovementRules {
            fall: jump_rules(),
            ..BattleMovementRules::STANDARD
        };
        for _ in 0..10 {
            advance_battle_jumps(&mut world, rules).unwrap();
        }
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let cursor = scripts.world().btech.constructed_units()[&id]
            .flight()
            .unwrap();
        let position = scripts.world().btech.constructed_units()[&id].position();
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "jumplength", length)
            .unwrap();
        assert_eq!(
            scripts.world().btech.constructed_units()[&id]
                .flight()
                .unwrap()
                .sample(),
            cursor.sample()
        );
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "jumpheading", "90")
            .unwrap();
        let saved = scripts.world().clone();
        let cancel = Scripts::new(&config, Rc::new(RefCell::new(saved.clone()))).unwrap();
        set_battle_unit_field_action(&cancel, &config, ObjectId(1), id, "jumplength", "1000")
            .unwrap();
        let mut continuing = cancel.world().clone();
        advance_battle_jumps(&mut continuing, rules).unwrap();
        assert!(continuing.btech.constructed_units()[&id].flight().is_some());

        persistence::save(&config.database(), &saved).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        let mut expected = saved;
        let notices = advance_battle_jumps(&mut expected, rules).unwrap();
        assert_eq!(advance_battle_jumps(&mut replay, rules).unwrap(), notices);
        assert_eq!(expected.btech, replay.btech);
        assert!(replay.btech.constructed_units()[&id].flight().is_none());
        assert_eq!(replay.btech.constructed_units()[&id].position(), position);
        assert!(
            notices
                .iter()
                .any(|notice| notice.text.contains("finish your jump"))
        );
        replay.validate(&config).unwrap();
    }
}

/// Redirected DFA retains intent and uses normal landing admission at its new location.
#[tokio::test]
async fn jump_fields_keep_dfa_intent_without_chasing_the_target() {
    use std::{cell::RefCell, rc::Rc};
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    let (target, _) = jump_observer(&mut world, &config, id);
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    launch_battle_dfa(&mut world, id, ObjectId(1), Some(target)).unwrap();
    let rules = BattleMovementRules {
        fall: jump_rules(),
        ..BattleMovementRules::STANDARD
    };
    for _ in 0..3 {
        advance_battle_jumps(&mut world, rules).unwrap();
    }
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "jumpheading", "270").unwrap();
    assert_eq!(
        scripts.world().btech.constructed_units()[&id]
            .flight()
            .unwrap()
            .dfa_target(),
        Some(target)
    );
    let mut world = scripts.world().clone();
    let target_before = world.btech.constructed_units()[&target].clone();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut replay = persistence::load(&config.database()).await.unwrap();
    for _ in 0..60 {
        if world.btech.constructed_units()[&id].flight().is_none() {
            break;
        }
        assert_eq!(
            advance_battle_jumps(&mut world, rules).unwrap(),
            advance_battle_jumps(&mut replay, rules).unwrap()
        );
        assert_eq!(world.btech, replay.btech);
    }
    assert!(world.btech.constructed_units()[&id].flight().is_none());
    assert_eq!(world.btech.constructed_units()[&target], target_before);
    assert_ne!(
        world.btech.constructed_units()[&id].position(),
        world.btech.constructed_units()[&target].position()
    );
    world.validate(&config).unwrap();
}

/// A redirected route crosses a wrapping seam through the existing boundary and landing code.
#[tokio::test]
async fn jump_fields_cross_wrapping_seams_and_restart() {
    use std::{cell::RefCell, rc::Rc};
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    set_battle_map_wrapping(&mut world, map, true).unwrap();
    let relocate = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    set_battle_unit_field_action(&relocate, &config, ObjectId(1), id, "x", "11").unwrap();
    let mut world = relocate.world().clone();
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 4.0).unwrap();
    let rules = BattleMovementRules {
        fall: jump_rules(),
        ..BattleMovementRules::STANDARD
    };
    for _ in 0..3 {
        advance_battle_jumps(&mut world, rules).unwrap();
    }
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "jumpheading", "90").unwrap();
    let mut world = scripts.world().clone();
    let mut crossed = false;
    for _ in 0..120 {
        if world.btech.constructed_units()[&id].flight().is_none() {
            break;
        }
        advance_battle_jumps(&mut world, rules).unwrap();
        if !crossed && world.btech.constructed_units()[&id].position().unwrap().x < 5 {
            crossed = true;
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            assert_eq!(replay.btech, world.btech);
            assert_eq!(
                advance_battle_jumps(&mut replay, rules).unwrap(),
                advance_battle_jumps(&mut world, rules).unwrap()
            );
            assert_eq!(replay.btech, world.btech);
            let other = world.create(&config, "Redirected scenario".into(), Kind::Room);
            let asset = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
            create_battle_map(
                &mut world,
                other,
                "redirect.map",
                BattleMapAsset::parse(&asset).unwrap(),
            )
            .unwrap();
            reassign_battle_map(&mut world, id, other, None).unwrap();
            let edit = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let before = edit.world().btech.constructed_units()[&id]
                .flight()
                .unwrap()
                .sample();
            set_battle_unit_field_action(&edit, &config, ObjectId(1), id, "jumpheading", "180")
                .unwrap();
            world = edit.world().clone();
            assert_eq!(
                world.btech.constructed_units()[&id]
                    .flight()
                    .unwrap()
                    .sample(),
                before
            );
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
        }
    }
    assert!(crossed);
    assert!(world.btech.constructed_units()[&id].flight().is_none());
    world.validate(&config).unwrap();
}

/// A new course uses ordinary terrain collision and preserves deterministic restart outcomes.
#[tokio::test]
async fn jump_fields_redirect_into_existing_collision_handling() {
    use std::{cell::RefCell, rc::Rc};
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = runtime_fixture().await;
    jump_hills(&mut world, id, 0, 0, 9);
    launch_battle_jump(&mut world, id, ObjectId(1), 90, 4.0).unwrap();
    let rules = BattleMovementRules {
        fall: jump_rules(),
        ..BattleMovementRules::STANDARD
    };
    for _ in 0..3 {
        advance_battle_jumps(&mut world, rules).unwrap();
    }
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "jumpheading", "0").unwrap();
    let mut world = scripts.world().clone();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut replay = persistence::load(&config.database()).await.unwrap();
    for _ in 0..24 {
        let point = world.btech.constructed_units()[&id].motion().unwrap().point;
        assert_eq!(
            advance_battle_jumps(&mut world, rules).unwrap(),
            advance_battle_jumps(&mut replay, rules).unwrap()
        );
        assert_eq!(world.btech, replay.btech);
        if world.btech.constructed_units()[&id].flight().is_none() {
            assert_eq!(
                world.btech.constructed_units()[&id].motion().unwrap().point,
                point
            );
            break;
        }
    }
    assert!(world.btech.constructed_units()[&id].flight().is_none());
    assert_eq!(
        world.btech.constructed_units()[&id].position().unwrap().y,
        5
    );
    world.validate(&config).unwrap();
}

/// Damage replacement changes thrust without moving the saved trajectory or resolving a fall early.
#[tokio::test]
async fn damage_replacement_during_jump_defers_lost_thrust_and_replays_restoration() {
    use std::{cell::RefCell, rc::Rc};
    use stompymux_rs::*;
    for chassis in ["Biped", "Quad"] {
        let (_dir, config, world, id) = runtime_fixture().await;
        let setup = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(&setup, &config, ObjectId(1), id, "mechmovetype", chassis)
            .unwrap();
        let mut world = setup.world().clone();
        launch_battle_jump(&mut world, id, ObjectId(1), 0, 4.0).unwrap();
        let rules = BattleMovementRules {
            fall: jump_rules(),
            ..BattleMovementRules::STANDARD
        };
        for _ in 0..10 {
            advance_battle_jumps(&mut world, rules).unwrap();
        }
        let cursor = world.btech.constructed_units()[&id].flight().unwrap();
        let dice =
            serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"].clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(
            &scripts,
            &config,
            ObjectId(1),
            id,
            "mechdamage",
            "C:2/0,C:2/1,C:3/1,C:3/2,C:4/11",
        )
        .unwrap();
        {
            let world = scripts.world();
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(unit.flight(), Some(cursor));
            assert_eq!(unit.jump_capacity(100).unwrap().speed, 0.0);
            assert_eq!(serde_json::to_value(unit).unwrap()["dice"], dice);
            world.validate(&config).unwrap();
        }
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let mut falling = saved.clone();
        let notices = advance_battle_jumps(&mut falling, rules).unwrap();
        assert!(
            notices
                .iter()
                .any(|notice| notice.text.contains("lose control of your jump"))
        );
        assert_eq!(notices, advance_battle_jumps(&mut loaded, rules).unwrap());
        assert_eq!(falling.btech, loaded.btech);
        assert!(falling.btech.constructed_units()[&id].flight().is_none());
        falling.validate(&config).unwrap();

        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "mechdamage", "").unwrap();
        let mut restored = scripts.world().clone();
        assert_eq!(
            restored.btech.constructed_units()[&id].flight(),
            Some(cursor)
        );
        assert_eq!(
            restored.btech.constructed_units()[&id]
                .jump_capacity(100)
                .unwrap()
                .speed,
            53.75
        );
        let notices = advance_battle_jumps(&mut restored, rules).unwrap();
        assert!(
            !notices
                .iter()
                .any(|notice| notice.text.contains("lose control"))
        );
        assert_ne!(
            restored.btech.constructed_units()[&id]
                .flight()
                .unwrap()
                .sample(),
            cursor.sample()
        );
        for _ in 0..120 {
            if restored.btech.constructed_units()[&id].flight().is_none() {
                break;
            }
            advance_battle_jumps(&mut restored, rules).unwrap();
        }
        assert!(restored.btech.constructed_units()[&id].flight().is_none());
        restored.validate(&config).unwrap();
    }
}

/// A running pilot's free-fall protection roll stays private and follows the impact warning.
#[tokio::test]
async fn free_fall_landing_feedback_is_private_and_replays() {
    use stompymux_rs::*;
    for roll in [2, 12] {
        let (_dir, config, mut world, id) = runtime_fixture().await;
        for player in [ObjectId(1), ObjectId(2)] {
            world.objects.get_mut(&player).unwrap().location = Some(id);
            world
                .objects
                .get_mut(&player)
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        let byte = (0..=255)
            .find(|byte| BattleDice::seeded([*byte; 32]).two_d6() == roll)
            .unwrap();
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["constructed"][id.0.to_string()]["free_fall"] =
            serde_json::to_value(BattleFreeFall::new(1)).unwrap();
        encoded["constructed"][id.0.to_string()]["free_fall"]["remaining"] = 1.into();
        encoded["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([byte; 32])).unwrap();
        world.btech = serde_json::from_value(encoded).unwrap();
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for player in [ObjectId(1), ObjectId(2)] {
            restored
                .objects
                .get_mut(&player)
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        let replay =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        let movement = BattleMovementRules {
            fall: jump_rules(),
            ..BattleMovementRules::STANDARD
        };
        advance_battle_jumps_action(&scripts, &config, movement).unwrap();
        advance_battle_jumps_action(&replay, &config, movement).unwrap();
        assert_eq!(scripts.world().btech, replay.world().btech);
        assert!(
            scripts.world().btech.constructed_units()[&id]
                .free_fall()
                .is_none()
        );
        let output = scripts.drain_outbox();
        let replay_output = replay.drain_outbox();
        assert_eq!(
            output
                .iter()
                .map(|(who, message)| (*who, message.source()))
                .collect::<Vec<_>>(),
            replay_output
                .iter()
                .map(|(who, message)| (*who, message.source()))
                .collect::<Vec<_>>()
        );
        let pilot: Vec<_> = output
            .iter()
            .filter(|(who, _)| *who == ObjectId(1))
            .map(|(_, message)| message.source())
            .collect();
        let index = pilot
            .iter()
            .position(|message| *message == "You make a piloting skill roll!")
            .unwrap();
        assert!(pilot[..index].contains(&"You hit the ground!"));
        assert!(pilot[index + 1].ends_with(&format!("\tRoll: {roll}")));
        assert!(!output.iter().any(|(who, message)| *who == ObjectId(2)
            && (message.source().starts_with("Modified Pilot Skill:")
                || message.source() == "You make a piloting skill roll!")));
        assert!(
            output
                .iter()
                .any(|(who, message)| *who == ObjectId(2)
                    && message.source() == "You hit the ground!")
        );
    }
}
