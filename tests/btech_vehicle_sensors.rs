//! Vehicle sensor selection, equipment fallback and persisted timer transactions.
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
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 30).unwrap();
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

/// A mixed optical pair suited to a dark battlefield.
fn amplified() -> BattleSensorPair {
    BattleSensorPair {
        primary: BattleSensorMode::LightAmplification,
        secondary: BattleSensorMode::Visual,
    }
}

#[tokio::test]
async fn vehicle_sensor_commands_and_lua_share_switching_and_restart_state() {
    let (_dir, config, world, id) = fixture(include_str!("../game/mechs/Demolisher")).await;
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert!(
        support::run_text(&native, &config, ObjectId(1), 1, "sensor")
            .contains("Vislight in 360 degree scanning mode (R:Visual)")
    );
    let output = support::run_text(&native, &config, ObjectId(1), 1, "sensor L V");
    assert!(output.contains("Wanted"), "{output}");
    lua.eval_callback::<bool>(&format!("return btech.unit.sensors({},1,'L','V')", id.0))
        .unwrap();
    assert_eq!(native.world().btech, lua.world().btech);
    let before = lua.world().clone();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.sensors({},1,'I','V'); error('abort')",
            id.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, before.btech);
    let remaining: u8 = lua
        .eval_callback(&format!(
            "return btech.unit.state({}).sensor_selection.pending.remaining",
            id.0
        ))
        .unwrap();
    assert_eq!(remaining, 10);
    let mut world = before;
    for _ in 0..4 {
        assert!(advance_battle_sensor_selection(&mut world).is_empty());
    }
    let pending = world.btech.vehicles()[&id].sensor_selection().pending;
    select_battle_optical_sensors(&mut world, id, ObjectId(1), BattleSensorPair::default())
        .unwrap();
    assert_eq!(
        world.btech.vehicles()[&id].sensor_selection().pending,
        pending
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    for tick in 1..=6 {
        let notices = advance_battle_sensor_selection(&mut world);
        assert_eq!(notices, advance_battle_sensor_selection(&mut restored));
        assert_eq!(world.btech, restored.btech);
        assert_eq!(notices.len(), usize::from(tick == 6));
    }
    assert_eq!(
        world.btech.vehicles()[&id].sensor_selection().active,
        amplified()
    );
    let map = world.btech.vehicles()[&id].position().unwrap().map;
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 30).unwrap();
    assert_eq!(
        world.btech.vehicles()[&id].sensor_selection().active,
        BattleSensorPair::default()
    );
    let before = world.btech.clone();
    assert!(select_battle_optical_sensors(&mut world, id, ObjectId(1), amplified()).is_err());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn vehicle_sensor_equipment_loss_reconciles_active_and_pending_modes() {
    let text = include_str!("../game/mechs/Demolisher")
        .replace("ICEEngine_Tech", "ICEEngine_Tech AntiAircraft")
        .replace(
            "Front_Side\n",
            "Front_Side\n CRIT_1-2 { BeagleProbe - - }\n CRIT_3 { BloodhoundProbe - - }\n",
        );
    let (_dir, config, mut world, id) = fixture(&text).await;
    let pair = BattleSensorPair {
        primary: BattleSensorMode::BeagleProbe,
        secondary: BattleSensorMode::BloodhoundProbe,
    };
    assert!(
        world.btech.vehicles()[&id]
            .has_active_probe(BattleActiveProbe::Bloodhound)
            .unwrap()
    );
    select_battle_optical_sensors(&mut world, id, ObjectId(1), pair).unwrap();
    for _ in 0..10 {
        advance_battle_sensor_selection(&mut world);
    }
    assert_eq!(world.btech.vehicles()[&id].sensor_selection().active, pair);
    let mut section_loss = world.clone();
    damage_battle_vehicle_phase(
        &mut section_loss,
        id,
        BattleVehicleSection::Front,
        8,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    assert_eq!(
        section_loss.btech.vehicles()[&id].sensor_selection().active,
        BattleSensorPair::default()
    );
    destroy_battle_vehicle_critical(
        &mut world,
        id,
        VehicleCriticalLocation {
            section: BattleVehicleSection::Front,
            slot: 0,
        },
    )
    .unwrap();
    assert_eq!(world.btech.vehicles()[&id].sensor_selection().active, pair);
    destroy_battle_vehicle_critical(
        &mut world,
        id,
        VehicleCriticalLocation {
            section: BattleVehicleSection::Front,
            slot: 1,
        },
    )
    .unwrap();
    assert_eq!(
        world.btech.vehicles()[&id]
            .sensor_selection()
            .active
            .primary,
        BattleSensorMode::Visual
    );
    let before = world.btech.clone();
    assert!(select_battle_optical_sensors(&mut world, id, ObjectId(1), pair).is_err());
    assert_eq!(world.btech, before);
    let pending = BattleSensorPair {
        primary: BattleSensorMode::BloodhoundProbe,
        secondary: BattleSensorMode::Visual,
    };
    select_battle_optical_sensors(&mut world, id, ObjectId(1), pending).unwrap();
    destroy_battle_vehicle_critical(
        &mut world,
        id,
        VehicleCriticalLocation {
            section: BattleVehicleSection::Front,
            slot: 2,
        },
    )
    .unwrap();
    assert_eq!(
        world.btech.vehicles()[&id].sensor_selection().active,
        BattleSensorPair::default()
    );
    for _ in 0..10 {
        assert!(advance_battle_sensor_selection(&mut world).is_empty());
    }
    assert!(
        world.btech.vehicles()[&id]
            .sensor_selection()
            .pending
            .is_none()
    );
    let radar = BattleSensorPair {
        primary: BattleSensorMode::Radar,
        secondary: BattleSensorMode::Visual,
    };
    select_battle_optical_sensors(&mut world, id, ObjectId(1), radar).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        world.btech,
        persistence::load(&config.database()).await.unwrap().btech
    );
}

#[tokio::test]
async fn vehicle_sensor_guards_and_corrupt_snapshots_reject_without_mutation() {
    let (_dir, _config, mut world, id) = fixture(include_str!("../game/mechs/Demolisher")).await;
    let before = world.btech.clone();
    assert!(select_battle_optical_sensors(&mut world, id, ObjectId(2), amplified()).is_err());
    assert!(
        select_battle_optical_sensors(
            &mut world,
            id,
            ObjectId(1),
            BattleSensorPair {
                primary: BattleSensorMode::Radar,
                secondary: BattleSensorMode::Visual
            }
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
    select_battle_optical_sensors(&mut world, id, ObjectId(1), amplified()).unwrap();
    for remaining in [0, 11, 255] {
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["vehicles"][id.0.to_string()]["sensor_selection"]["pending"]["remaining"] =
            remaining.into();
        assert!(serde_json::from_value::<BtechState>(state).is_err());
    }
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["sensor_selection"]["active"]["primary"] = "radar".into();
    assert!(serde_json::from_value::<BtechState>(state).is_err());
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    for _ in 0..10 {
        assert!(advance_battle_sensor_selection(&mut world).is_empty());
    }
    assert!(
        world.btech.vehicles()[&id]
            .sensor_selection()
            .pending
            .is_none()
    );
    assert_eq!(
        world.btech.vehicles()[&id].sensor_selection().active,
        BattleSensorPair::default()
    );
}

#[tokio::test]
async fn powered_off_vehicle_sensor_timer_retries_failed_server_saves() {
    use sqlx::Connection;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir,config,mut world,id)=fixture(include_str!("../game/mechs/Demolisher")).await;
        select_battle_optical_sensors(&mut world,id,ObjectId(1),amplified()).unwrap();
        stop_battle_unit(&mut world,id,ObjectId(1),BattleMovementRules::STANDARD.fall).unwrap();
        for _ in 0..8 {advance_battle_sensor_selection(&mut world);}
        persistence::save(&config.database(),&world).await.unwrap();
        let mut sql=sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::raw_sql("CREATE TRIGGER deny_sensors BEFORE UPDATE ON btech_vehicles BEGIN SELECT RAISE(ABORT,'sensor failure'); END;").execute(&mut sql).await.unwrap();
        let (_address,shutdown,task,_lua)=support::start(&config,std::rc::Rc::new(std::cell::Cell::new(1))).await;
        tokio::time::sleep(std::time::Duration::from_millis(1200)).await;
        assert_eq!(persistence::load(&config.database()).await.unwrap().btech.vehicles()[&id].sensor_selection().pending.unwrap().remaining,2);
        sqlx::query("DROP TRIGGER deny_sensors").execute(&mut sql).await.unwrap();
        tokio::time::timeout(std::time::Duration::from_secs(5),async {
            loop {
                if persistence::load(&config.database()).await.unwrap().btech.vehicles()[&id].sensor_selection().pending.is_none(){break;}
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            }
        }).await.unwrap();
        shutdown.send(ShutdownRequest::Sigterm).unwrap();task.await.unwrap().unwrap();
    }).await;
}
