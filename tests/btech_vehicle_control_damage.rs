//! Vehicle control damage feeds piloting and section-specific firing modifiers across restart.
use crate::support;
use stompymux_rs::*;

/// A running vehicle with a present pilot, initially disconnected from a session.
async fn fixture(template: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
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
        BattleVehicleTemplate::parse(template).unwrap(),
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
async fn vehicle_control_damage_stacks_without_changing_skills_or_construction() {
    use BattleVehicleControlHit as H;
    let text = include_str!("../game/mechs/Demolisher").replace(
        "Front_Side\n",
        "Front_Side\n    CRIT_1 { IS.MediumLaser - - }\n",
    );
    let (_dir, config, mut world, id) = fixture(&text).await;
    let definition = world.btech.vehicles()[&id].definition().clone();
    let loadout = world.btech.vehicles()[&id].loadout().unwrap();
    let front = loadout
        .weapons
        .iter()
        .position(|mount| mount.criticals[0].section == BattleVehicleSection::Front)
        .unwrap();
    let turret = loadout
        .weapons
        .iter()
        .position(|mount| mount.criticals[0].section == BattleVehicleSection::Turret)
        .unwrap();
    let baseline = roll_battle_piloting(&mut world, id, -1, true).unwrap();
    for hit in [
        H::Driver,
        H::Driver,
        H::Sensors,
        H::Sensors,
        H::Stabilizers {
            section: BattleVehicleSection::Turret,
        },
    ] {
        damage_battle_vehicle_controls(&mut world, id, hit).unwrap();
    }
    let checkpoint = world.btech.clone();
    damage_battle_vehicle_controls(
        &mut world,
        id,
        H::Stabilizers {
            section: BattleVehicleSection::Turret,
        },
    )
    .unwrap();
    assert_eq!(world.btech, checkpoint);
    assert_eq!(world.btech.vehicles()[&id].definition(), &definition);
    let check = roll_battle_piloting(&mut world, id, -1, true).unwrap();
    assert_eq!(check.damage, 4);
    assert_eq!(check.skill, baseline.skill);
    assert_eq!(check.target, baseline.target + 4);
    set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
    let vehicle = &world.btech.vehicles()[&id];
    assert_eq!(vehicle.weapon_movement_modifier(front, true).unwrap(), 1);
    assert_eq!(vehicle.weapon_movement_modifier(turret, true).unwrap(), 2);
    assert_eq!(vehicle.weapon_control_modifier(front, true).unwrap(), 3);
    assert_eq!(vehicle.weapon_control_modifier(turret, true).unwrap(), 4);
    assert_eq!(vehicle.weapon_control_modifier(turret, false).unwrap(), 2);
    assert!(vehicle.weapon_control_modifier(99, false).is_err());
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(
        roll_battle_piloting(&mut loaded, id, 3, true).unwrap(),
        roll_battle_piloting(&mut world, id, 3, true).unwrap()
    );
    assert_eq!(loaded.btech, world.btech);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(loaded))).unwrap();
    let values: (u8, u8) = scripts
        .eval_callback(&format!(
            "local u=btech.unit.state({}); return u.piloting_damage,u.gunnery_damage",
            id.0
        ))
        .unwrap();
    assert_eq!(values, (4, 2));
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["motion"]["speed"] = 53.75.into();
    state["vehicles"][id.0.to_string()]["motion"]["desired_speed"] = 53.75.into();
    world.btech = serde_json::from_value(state).unwrap();
    assert_eq!(
        world.btech.vehicles()[&id]
            .weapon_control_modifier(turret, true)
            .unwrap(),
        6
    );
    assert_eq!(
        world.btech.vehicles()[&id]
            .weapon_control_modifier(front, true)
            .unwrap(),
        4
    );
}

#[tokio::test]
async fn vehicle_control_penalties_saturate_and_reject_invalid_state() {
    use BattleVehicleControlHit as H;
    let (_dir, _config, mut world, id) = fixture(include_str!("../game/mechs/Demolisher")).await;
    for _ in 0..130 {
        damage_battle_vehicle_controls(&mut world, id, H::Driver).unwrap();
        damage_battle_vehicle_controls(&mut world, id, H::Sensors).unwrap();
    }
    assert_eq!(world.btech.vehicles()[&id].piloting_damage(), 127);
    assert_eq!(world.btech.vehicles()[&id].gunnery_damage(), 127);
    let before = world.btech.clone();
    damage_battle_vehicle_controls(&mut world, id, H::Driver).unwrap();
    damage_battle_vehicle_controls(&mut world, id, H::Sensors).unwrap();
    assert_eq!(world.btech, before);
    let original = serde_json::to_value(&world.btech.vehicles()[&id]).unwrap();
    for field in ["piloting_damage", "gunnery_damage"] {
        let mut bad = original.clone();
        bad[field] = 128.into();
        assert!(serde_json::from_value::<BattleVehicle>(bad).is_err());
    }
    damage_battle_vehicle_phase(
        &mut world,
        id,
        BattleVehicleSection::Turret,
        100,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    let before = world.btech.clone();
    assert!(
        damage_battle_vehicle_controls(
            &mut world,
            id,
            H::Stabilizers {
                section: BattleVehicleSection::Turret
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
        .insert(Flag::Going);
    assert!(damage_battle_vehicle_controls(&mut world, id, H::Driver).is_err());
    assert!(damage_battle_vehicle_controls(&mut world, ObjectId(-1), H::Sensors).is_err());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn crew_stun_limits_controls_restarts_and_recovers_after_shutdown() {
    let (_dir, config, mut world, id) = fixture(include_str!("../game/mechs/Demolisher")).await;
    let pilot = ObjectId(1);
    let maximum = world.btech.vehicles()[&id].maximum_speed();
    set_battle_speed(&mut world, id, pilot, maximum).unwrap();
    damage_battle_vehicle_controls(&mut world, id, BattleVehicleControlHit::Commander).unwrap();
    let vehicle = &world.btech.vehicles()[&id];
    assert_eq!(vehicle.crew_stun_remaining(), 60);
    assert_eq!(vehicle.piloting_damage(), 1);
    assert_eq!(vehicle.gunnery_damage(), 1);
    assert_eq!(
        vehicle.motion().unwrap().desired_speed,
        maximum * 2.0 / 3.0 - 0.1
    );
    assert_eq!(vehicle.motion().unwrap().speed, 0.0);
    assert!(!vehicle.weapon_readiness(0).unwrap().ready);
    let checkpoint = world.btech.clone();
    assert!(
        reserve_battle_vehicle_weapon(&mut world, id, pilot, 0, true)
            .unwrap_err()
            .to_string()
            .contains("stunned")
    );
    assert!(set_battle_speed(&mut world, id, pilot, maximum).is_err());
    assert_eq!(world.btech, checkpoint);
    set_battle_heading(&mut world, id, pilot, 90.0).unwrap();
    set_battle_turret(&mut world, id, pilot, 90.0).unwrap();
    set_battle_speed(&mut world, id, pilot, -maximum * 2.0 / 3.0).unwrap();
    for _ in 0..59 {
        advance_battle_units(&mut world, 0);
    }
    assert_eq!(world.btech.vehicles()[&id].crew_stun_remaining(), 1);
    damage_battle_vehicle_controls(&mut world, id, BattleVehicleControlHit::CrewStun).unwrap();
    assert_eq!(
        world.btech.vehicles()[&id].motion().unwrap().desired_speed,
        -maximum * 2.0 / 3.0
    );
    stop_battle_unit(&mut world, id, pilot, BattleMovementRules::STANDARD.fall).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let mut recovery_notices = Vec::new();
    for _ in 0..60 {
        let notices = advance_battle_units(&mut world, 0);
        assert_eq!(notices, advance_battle_units(&mut loaded, 0));
        recovery_notices.extend(notices);
    }
    assert_eq!(recovery_notices.len(), 1);
    assert!(recovery_notices[0].text.contains("Your head clears"));
    assert_eq!(world.btech, loaded.btech);
    assert_eq!(world.btech.vehicles()[&id].crew_stun_remaining(), 0);
    assert!(advance_battle_units(&mut world, 0).is_empty());
    let mut bad = serde_json::to_value(&world.btech.vehicles()[&id]).unwrap();
    bad["crew_stun_remaining"] = serde_json::json!(61);
    assert!(serde_json::from_value::<BattleVehicle>(bad).is_err());
}

#[tokio::test]
async fn powered_off_crew_recovery_retries_failed_server_ticks() {
    use sqlx::Connection;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir,config,mut world,id)=fixture(include_str!("../game/mechs/Demolisher")).await;
        damage_battle_vehicle_controls(&mut world,id,BattleVehicleControlHit::CrewStun).unwrap();
        stop_battle_unit(&mut world,id,ObjectId(1),BattleMovementRules::STANDARD.fall).unwrap();
        for _ in 0..58 {advance_battle_units(&mut world, 0);}
        persistence::save(&config.database(),&world).await.unwrap();
        let mut sql=sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_stun BEFORE UPDATE ON btech_vehicles BEGIN SELECT RAISE(ABORT,'stun failure'); END;").execute(&mut sql).await.unwrap();
        let (_address,shutdown,task,_lua)=support::start(&config,std::rc::Rc::new(std::cell::Cell::new(1))).await;
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.vehicles()[&id].crew_stun_remaining(),2);
        sqlx::query("DROP TRIGGER deny_stun").execute(&mut sql).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5),async {
            loop {
                if persistence::load(&config.database()).await.unwrap().btech.vehicles()[&id].crew_stun_remaining() == 0{break;}
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }).await.unwrap();
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

#[tokio::test]
async fn hull_destruction_cancels_crew_recovery() {
    let (_dir, _config, mut world, id) = fixture(include_str!("../game/mechs/Demolisher")).await;
    damage_battle_vehicle_controls(&mut world, id, BattleVehicleControlHit::CrewStun).unwrap();
    damage_battle_vehicle_phase(
        &mut world,
        id,
        BattleVehicleSection::Front,
        100,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    assert_eq!(world.btech.vehicles()[&id].crew_stun_remaining(), 0);
    assert!(advance_battle_units(&mut world, 0).is_empty());
    let mut bad = serde_json::to_value(&world.btech.vehicles()[&id]).unwrap();
    bad["crew_stun_remaining"] = serde_json::json!(1);
    assert!(serde_json::from_value::<BattleVehicle>(bad).is_err());
}
