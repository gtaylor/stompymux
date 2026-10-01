//! Vehicle turret facing, native/Lua control transactions, damage locks, and saved replay.
use crate::support;
use stompymux_rs::*;

/// A running vehicle with a present pilot, initially disconnected from a session.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Test field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("Demolisher",include_str!("../game/mechs/Demolisher.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

#[tokio::test]
async fn turret_controls_follow_hull_turns_and_replay_through_native_and_lua() {
    let (_dir, config, mut world, id) = fixture().await;
    set_battle_turret(&mut world, id, ObjectId(1), -90.0).unwrap();
    assert_eq!(
        battle_turret_readout(&world, id, ObjectId(1)).unwrap(),
        270.0
    );
    let before = world.btech.clone();
    for heading in [f64::NAN, f64::INFINITY, 0.5, f64::from(i32::MAX) + 1.0] {
        assert!(set_battle_turret(&mut world, id, ObjectId(1), heading).is_err());
    }
    assert!(set_battle_turret(&mut world, id, ObjectId(2), 0.0).is_err());
    assert_eq!(world.btech, before);
    set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
    for _ in 0..20 {
        advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
    }
    assert_eq!(world.btech.vehicles()[&id].motion().unwrap().heading, 90.0);
    assert_eq!(battle_turret_readout(&world, id, ObjectId(1)).unwrap(), 0.0);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let output = support::run_text(&scripts, &config, ObjectId(1), 1, "turret 450");
    assert!(output.contains("Turret facing changed to 90"), "{output}");
    let output = support::run_text(&scripts, &config, ObjectId(1), 1, "turret");
    assert!(output.contains("currently facing 90"), "{output}");
    assert_eq!(
        scripts
            .eval_callback::<f64>(&format!("return btech.unit.turret({},1)", id.0))
            .unwrap(),
        90.0
    );
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.turret({},1,180); error('abort')",
                id.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    scripts
        .eval_callback::<()>(&format!("btech.unit.turret({},1,180)", id.0))
        .unwrap();
    let candidate = scripts.world().clone();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, candidate.btech);
    assert_eq!(
        battle_turret_readout(&loaded, id, ObjectId(1)).unwrap(),
        180.0
    );
}

#[tokio::test]
async fn turret_locks_persist_and_loss_removes_facing_and_prevents_control() {
    let (_dir, config, mut world, id) = fixture().await;
    set_battle_turret(&mut world, id, ObjectId(1), 120.0).unwrap();
    damage_battle_vehicle_motive(&mut world, id, BattleVehicleMotiveHit::Immobilize).unwrap();
    set_battle_turret(&mut world, id, ObjectId(1), 90.0).unwrap();
    lock_battle_vehicle_turret(&mut world, id).unwrap();
    let before = world.btech.clone();
    assert!(set_battle_turret(&mut world, id, ObjectId(1), 180.0).is_err());
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert!(loaded.btech.vehicles()[&id].turret_locked());
    assert_eq!(loaded.btech.vehicles()[&id].turret_heading(), Some(90.0));
    assert!(set_battle_turret(&mut loaded, id, ObjectId(1), 180.0).is_err());
    damage_battle_vehicle_phase(
        &mut loaded,
        id,
        BattleVehicleSection::Turret,
        100,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    assert!(!loaded.btech.vehicles()[&id].turret_locked());
    assert_eq!(loaded.btech.vehicles()[&id].turret_heading(), None);
    let before = loaded.btech.clone();
    assert!(lock_battle_vehicle_turret(&mut loaded, id).is_err());
    assert!(set_battle_turret(&mut loaded, id, ObjectId(1), 0.0).is_err());
    assert_eq!(loaded.btech, before);
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        loaded.btech
    );
    let original = serde_json::to_value(&loaded.btech.vehicles()[&id]).unwrap();
    for (field, value) in [
        ("turret_locked", serde_json::json!(true)),
        ("turret_offset", serde_json::json!(360)),
        ("turret_offset", serde_json::json!(10)),
    ] {
        let mut bad = original.clone();
        bad[field] = value;
        assert!(serde_json::from_value::<BattleVehicle>(bad).is_err());
    }
}

#[tokio::test]
async fn jammed_turret_repairs_replay_and_block_fire_until_all_attempts_finish() {
    let (_dir, config, mut world, id) = fixture().await;
    assert!(begin_battle_turret_repair(&mut world, id, ObjectId(1)).is_err());
    jam_battle_vehicle_turret(&mut world, id).unwrap();
    assert!(world.btech.vehicles()[&id].turret_jammed());
    assert!(set_battle_turret(&mut world, id, ObjectId(1), 90.0).is_err());
    assert!(
        world.btech.vehicles()[&id]
            .weapon_readiness(0)
            .unwrap()
            .ready
    );
    begin_battle_turret_repair(&mut world, id, ObjectId(1)).unwrap();
    let before = world.btech.clone();
    assert!(
        reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true)
            .unwrap_err()
            .to_string()
            .contains("unjamming")
    );
    assert_eq!(before, world.btech);
    for _ in 0..10 {
        advance_battle_units(&mut world, 0);
    }
    begin_battle_turret_repair(&mut world, id, ObjectId(1)).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let mut notices = Vec::new();
    for _ in 0..50 {
        let step = advance_battle_units(&mut world, 0);
        assert_eq!(step, advance_battle_units(&mut loaded, 0));
        notices.extend(step);
    }
    assert_eq!(notices.len(), 1);
    assert!(!world.btech.vehicles()[&id].turret_jammed());
    assert_eq!(world.btech.vehicles()[&id].turret_repairs(), &[10]);
    assert!(
        !world.btech.vehicles()[&id]
            .weapon_readiness(0)
            .unwrap()
            .ready
    );
    for _ in 0..10 {
        let step = advance_battle_units(&mut world, 0);
        assert_eq!(step, advance_battle_units(&mut loaded, 0));
        notices.extend(step);
    }
    assert_eq!(notices.len(), 2);
    assert_eq!(world.btech, loaded.btech);
    assert!(
        world.btech.vehicles()[&id]
            .weapon_readiness(0)
            .unwrap()
            .ready
    );
    set_battle_turret(&mut world, id, ObjectId(1), 90.0).unwrap();
}

#[tokio::test]
async fn turret_repair_expires_offline_and_cannot_clear_a_second_hit_lock() {
    let (_dir, _config, mut world, id) = fixture().await;
    jam_battle_vehicle_turret(&mut world, id).unwrap();
    begin_battle_turret_repair(&mut world, id, ObjectId(1)).unwrap();
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    for _ in 0..60 {
        assert!(advance_battle_units(&mut world, 0).is_empty());
    }
    assert!(world.btech.vehicles()[&id].turret_jammed());
    assert!(world.btech.vehicles()[&id].turret_repairs().is_empty());
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    begin_battle_turret_repair(&mut world, id, ObjectId(1)).unwrap();
    jam_battle_vehicle_turret(&mut world, id).unwrap();
    assert!(world.btech.vehicles()[&id].turret_locked());
    assert!(!world.btech.vehicles()[&id].turret_jammed());
    let checkpoint = world.btech.clone();
    jam_battle_vehicle_turret(&mut world, id).unwrap();
    assert_eq!(world.btech, checkpoint);
    let mut notices = Vec::new();
    for _ in 0..60 {
        notices.extend(advance_battle_units(&mut world, 0));
    }
    assert_eq!(notices.len(), 1);
    assert_eq!(notices[0].text, "You are unable to unjam the turret!");
    assert!(world.btech.vehicles()[&id].turret_locked());
    assert!(begin_battle_turret_repair(&mut world, id, ObjectId(1)).is_err());
    let original = serde_json::to_value(&world.btech.vehicles()[&id]).unwrap();
    let mut both = original.clone();
    both["turret_jammed"] = true.into();
    let both: BattleVehicle = serde_json::from_value(both).unwrap();
    assert!(both.turret_locked() && both.turret_jammed());
    for (field, value) in [
        ("turret_repairs", serde_json::json!([0])),
        ("turret_repairs", serde_json::json!([61])),
        ("turret_repairs", serde_json::json!(vec![60; 257])),
    ] {
        let mut bad = original.clone();
        bad[field] = value;
        assert!(serde_json::from_value::<BattleVehicle>(bad).is_err());
    }
}

#[tokio::test]
async fn fixturret_native_and_lua_share_transactional_repair_state() {
    let (_dir, config, mut world, id) = fixture().await;
    jam_battle_vehicle_turret(&mut world, id).unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!("btech.unit.fixturret({},1); error('abort')", id.0))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "fixturret")
            .contains("start to repair")
    );
    assert!(
        scripts
            .eval_callback::<bool>(&format!("return btech.unit.fixturret({},1)", id.0))
            .unwrap()
    );
    assert_eq!(
        scripts.world().btech.vehicles()[&id].turret_repairs(),
        &[60, 60]
    );
    let count: usize = scripts
        .eval_callback(&format!(
            "return #btech.unit.state({}).turret_repairs",
            id.0
        ))
        .unwrap();
    assert_eq!(count, 2);
    let mut destroyed = scripts.world().clone();
    damage_battle_vehicle_phase(
        &mut destroyed,
        id,
        BattleVehicleSection::Turret,
        100,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    assert!(destroyed.btech.vehicles()[&id].turret_repairs().is_empty());
    assert!(!destroyed.btech.vehicles()[&id].turret_jammed());
}

#[tokio::test]
async fn powered_off_turret_repair_retries_failed_server_ticks() {
    use sqlx::Connection;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir,config,mut world,id)=fixture().await;
        jam_battle_vehicle_turret(&mut world,id).unwrap();
        begin_battle_turret_repair(&mut world,id,ObjectId(1)).unwrap();
        stop_battle_unit(&mut world,id,ObjectId(1),BattleMovementRules::STANDARD.fall).unwrap();
        for _ in 0..58 {advance_battle_units(&mut world, 0);}
        persistence::save(&config.database(),&world).await.unwrap();
        let mut sql=sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        // A running countdown keeps its timer row still; refuse the commit at the snapshot stamp.
        sqlx::raw_sql("CREATE TRIGGER deny_turret BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(ABORT,'turret failure'); END;").execute(&mut sql).await.unwrap();
        let (_address,shutdown,task,_lua)=support::start(&config,std::rc::Rc::new(std::cell::Cell::new(1))).await;
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.vehicles()[&id].turret_repairs(), &[2]);
        sqlx::query("DROP TRIGGER deny_turret").execute(&mut sql).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5),async {
            loop {
                if persistence::load(&config.database()).await.unwrap().btech.vehicles()[&id].turret_repairs().is_empty(){break;}
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }).await.unwrap();
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

/// Mode admission is independent of power; motion, settling, damage gates and rollback remain separate.
#[tokio::test]
async fn automatic_turret_controls_tracking_gates_and_restart() {
    let (_dir, config, mut world, id) = fixture().await;
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][id.0.to_string()]["power"] = serde_json::to_value(BattlePower::Off).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.autoturret({},1); error('abort')",
                id.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "autoturret ignored");
    assert!(text.contains("now ON"), "{text}");
    assert!(!battle_automatic_turrets_pending(&scripts.world()));
    assert!(toggle_battle_automatic_turret(&mut scripts.world_mut(), id, ObjectId(2)).is_err());
    let mut world = scripts.world().clone();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][id.0.to_string()]["power"] =
        serde_json::to_value(BattlePower::Running).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
    select_battle_hex_target(
        &mut world,
        id,
        ObjectId(1),
        BattleHexCoordinate { x: 0, y: 0 },
        BattleHexTargetMode::Hex,
    )
    .unwrap();
    assert!(battle_automatic_turrets_pending(&world));
    advance_battle_automatic_turrets(&mut world);
    assert_eq!(world.btech.vehicles()[&id].turret_heading(), Some(180.0));
    assert_eq!(
        world.btech.vehicles()[&id]
            .target_selection()
            .unwrap()
            .remaining(),
        8
    );
    assert!(!battle_automatic_turrets_pending(&world));
    set_battle_turret(&mut world, id, ObjectId(1), 90.0).unwrap();
    for (field, value) in [
        ("power", serde_json::to_value(BattlePower::Off).unwrap()),
        ("turret_jammed", serde_json::json!(true)),
        ("turret_locked", serde_json::json!(true)),
    ] {
        let mut stopped = world.clone();
        let mut state = serde_json::to_value(&stopped.btech).unwrap();
        state["vehicles"][id.0.to_string()][field] = value;
        if field == "power" {
            state["vehicles"][id.0.to_string()]["target_lock"] = serde_json::Value::Null;
        }
        stopped.btech = serde_json::from_value(state).unwrap();
        let before = stopped.btech.clone();
        assert!(!battle_automatic_turrets_pending(&stopped), "{field}");
        advance_battle_automatic_turrets(&mut stopped);
        assert_eq!(stopped.btech, before, "{field}");
    }
    let mut unconscious = world.clone();
    let mut recovery = serde_json::to_value(world.btech.vehicles()[&id].crew_recovery()).unwrap();
    recovery["remaining"] = 1.into();
    recovery["mode"] = serde_json::to_value(BattleRecoveryMode::Tactical { injuries: 1 }).unwrap();
    let mut state = serde_json::to_value(&unconscious.btech).unwrap();
    state["recoveries"]["1"] = recovery;
    unconscious.btech = serde_json::from_value(state).unwrap();
    unconscious.validate(&config).unwrap();
    let before = unconscious.btech.clone();
    advance_battle_automatic_turrets(&mut unconscious);
    assert_eq!(unconscious.btech, before);
    let mut stunned = world.clone();
    let mut snapshot = serde_json::to_value(&stunned.btech).unwrap();
    snapshot["vehicles"][id.0.to_string()]["crew_stun_remaining"] = 1.into();
    stunned.btech = serde_json::from_value(snapshot).unwrap();
    advance_battle_automatic_turrets(&mut stunned);
    assert_eq!(stunned.btech.vehicles()[&id].turret_heading(), Some(180.0));
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["motion"]["heading"] = 120.5.into();
    world.btech = serde_json::from_value(state).unwrap();
    advance_battle_automatic_turrets(&mut world);
    assert_eq!(world.btech.vehicles()[&id].turret_heading(), Some(180.0));
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    let scripts =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    let enabled: bool = scripts
        .eval_callback(&format!(
            "return btech.unit.state({}).automatic_turret",
            id.0
        ))
        .unwrap();
    assert!(enabled);
    scripts
        .eval_callback::<()>(&format!("btech.unit.autoturret({},1)", id.0))
        .unwrap();
    assert!(!scripts.world().btech.vehicles()[&id].automatic_turret());
}

/// Tracking samples current target coordinates and ignores invalid or cross-map targets.
#[tokio::test]
async fn automatic_turret_tracks_moving_units_and_hexes() {
    let (_dir, config, mut world, id) = fixture().await;
    let map = world.create(&config, "Tracking field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "tracking",
        BattleMapAsset::parse("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 1, 1).unwrap();
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    let target = world.create(&config, "Tracking target".into(), Kind::Thing);
    world.objects.get_mut(&target).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        target,
        BattleTemplate::parse("JR7-D",include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, target, map, 1, 0).unwrap();
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    select_battle_target(&mut world, id, ObjectId(1), Some(target)).unwrap();
    toggle_battle_automatic_turret(&mut world, id, ObjectId(1)).unwrap();
    set_battle_turret(&mut world, id, ObjectId(1), 90.0).unwrap();
    advance_battle_automatic_turrets(&mut world);
    assert_eq!(world.btech.vehicles()[&id].turret_heading(), Some(0.0));
    let mut moved = serde_json::to_value(&world.btech).unwrap();
    moved["constructed"][target.0.to_string()]["position"]["y"] = 2.into();
    moved["constructed"][target.0.to_string()]["motion"]["point"] =
        serde_json::to_value(BattleHexCoordinate { x: 1, y: 2 }.center()).unwrap();
    world.btech = serde_json::from_value(moved).unwrap();
    world.validate(&config).unwrap();
    advance_battle_automatic_turrets(&mut world);
    assert_eq!(world.btech.vehicles()[&id].turret_heading(), Some(180.0));
    let elsewhere = world.create(&config, "Other field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        elsewhere,
        "other",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, target, elsewhere, 0, 0).unwrap();
    assert!(!battle_automatic_turrets_pending(&world));
    advance_battle_automatic_turrets(&mut world);
    assert_eq!(world.btech.vehicles()[&id].turret_heading(), Some(180.0));
    select_battle_hex_target(
        &mut world,
        id,
        ObjectId(1),
        BattleHexCoordinate { x: 2, y: 1 },
        BattleHexTargetMode::Hex,
    )
    .unwrap();
    advance_battle_automatic_turrets(&mut world);
    assert_eq!(world.btech.vehicles()[&id].turret_heading(), Some(121.0));
}

/// Tracking uses vehicle anatomy rather than propulsion type; rotorcraft without a turret reject it.
#[tokio::test]
async fn automatic_tracking_is_shared_by_ground_and_rotorcraft() {
    let ground = include_str!("../game/mechs/Demolisher.toml");
    let vtol = include_str!("../game/mechs/Kestrel.toml");
    for source in [
        ground.into(),
        ground.replace("movement = \"track\"", "movement = \"wheel\""),
        ground.replace("movement = \"track\"", "movement = \"hover\""),
        ground
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("53.75", "0.0"),
        format!("{vtol}\nTurret\n Armor {{ 1 }}\n Internals {{ 1 }}\n"),
        vtol.into(),
    ] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let map = world.create(&config, "Field".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "field",
            BattleMapAsset::parse("1 1\n.0\n").unwrap(),
        )
        .unwrap();
        let id = world.create(&config, "Tracking carrier".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_vehicle(
            &mut world,
            id,
            BattleVehicleTemplate::parse("test",&source).unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        if world.btech.vehicles()[&id].turret_heading().is_none() {
            assert!(toggle_battle_automatic_turret(&mut world, id, ObjectId(1)).is_err());
            let mut invalid = serde_json::to_value(&world.btech).unwrap();
            invalid["vehicles"][id.0.to_string()]["automatic_turret"] = true.into();
            assert!(serde_json::from_value::<BtechState>(invalid).is_err());
            continue;
        }
        toggle_battle_automatic_turret(&mut world, id, ObjectId(1)).unwrap();
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        select_battle_hex_target(
            &mut world,
            id,
            ObjectId(1),
            BattleHexCoordinate { x: 0, y: 0 },
            BattleHexTargetMode::Hex,
        )
        .unwrap();
        advance_battle_automatic_turrets(&mut world);
        assert_eq!(world.btech.vehicles()[&id].turret_heading(), Some(180.0));
        let mut damaged = serde_json::to_value(&world.btech).unwrap();
        damaged["vehicles"][id.0.to_string()]["sections"]["turret"]["armor"] = 0.into();
        damaged["vehicles"][id.0.to_string()]["sections"]["turret"]["internal"] = 0.into();
        damaged["vehicles"][id.0.to_string()]["turret_offset"] = 0.into();
        for (index, bin) in world.btech.vehicles()[&id]
            .loadout()
            .unwrap()
            .ammunition
            .iter()
            .enumerate()
        {
            if bin.location.section == BattleVehicleSection::Turret {
                damaged["vehicles"][id.0.to_string()]["ammunition"][index] = 0.into();
            }
        }

        world.btech = serde_json::from_value(damaged).unwrap();
        assert!(!battle_automatic_turrets_pending(&world));
        assert!(toggle_battle_automatic_turret(&mut world, id, ObjectId(1)).is_err());
        world.validate(&config).unwrap();
    }
}

/// A settled selection still tracks after restart through the committed server heartbeat.
#[tokio::test(flavor = "current_thread")]
async fn automatic_turret_runs_after_restart() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_dir, config, mut world, id) = fixture().await;
            toggle_battle_automatic_turret(&mut world, id, ObjectId(1)).unwrap();
            select_battle_hex_target(
                &mut world,
                id,
                ObjectId(1),
                BattleHexCoordinate { x: 0, y: 0 },
                BattleHexTargetMode::Hex,
            )
            .unwrap();
            for _ in 0..8 {
                advance_battle_target_locks(&mut world);
            }
            persistence::save(&config.database(), &world).await.unwrap();
            let (_address, shutdown, task, _lua) =
                support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
            tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    let saved = persistence::load(&config.database()).await.unwrap();
                    if saved.btech.vehicles()[&id].turret_heading() == Some(180.0) {
                        break;
                    }
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }
            })
            .await
            .unwrap();
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}
