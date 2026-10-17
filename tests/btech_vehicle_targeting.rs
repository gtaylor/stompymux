//! Vehicle target selections own countdowns and survive transactional scan and persistence paths.
use crate::support;
use stompymux_rs::*;

/// Two Mechs and two vehicles at opposite ends of a north/south lane.
async fn fixture(
    tiles: &str,
    vehicle: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 4]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Sight lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "sight",
        MapAsset::from_cells(&format!("1 5\n{tiles}")).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for index in 0..4 {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if index < 2 {
            let mut definition =
                MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
                    .unwrap();
            definition
                .attributes
                .insert("specials".into(), "FlipArms Searchlight".into());
            create_battle_unit(&mut world, id, definition).unwrap();
        } else {
            create_battle_vehicle(
                &mut world,
                id,
                VehicleTemplate::parse("test", vehicle).unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut world, id, map, 0, if index % 2 == 0 { 4 } else { 0 }).unwrap();
        ids.push(id);
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, map, ids.try_into().unwrap())
}

/// Assign scenario power without introducing crew actions into sensor tests.
fn power(world: &mut World, ids: &[ObjectId], value: Power) {
    for id in ids {
        world.btech.set_unit_power(*id, value).unwrap();
    }
}

/// A close mixed formation guarantees contact acquisition without a random fixture dependency.
async fn formation() -> (tempfile::TempDir, Config, World, [ObjectId; 4]) {
    let (dir, config, mut world, map, ids) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/units/Demolisher.toml"),
    )
    .await;
    for id in ids {
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    }
    power(&mut world, &ids, Power::Running);
    (dir, config, world, ids)
}

#[tokio::test]
async fn vehicle_unit_and_coordinate_selections_replay_and_drive_scans() {
    let (_dir, config, initial, [mech, _, observer, vehicle]) = formation().await;
    for target in [mech, vehicle] {
        let mut world = initial.clone();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
        assign_battle_pilot(&mut world, observer, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        refresh_battle_contacts(&mut world, &[observer]).unwrap();
        let before = world.btech.clone();
        assert!(select_battle_target(&mut world, observer, ObjectId(2), Some(target)).is_err());
        assert!(select_battle_target(&mut world, observer, ObjectId(1), Some(observer)).is_err());
        assert_eq!(world.btech, before);
        select_battle_target(&mut world, observer, ObjectId(1), Some(target)).unwrap();
        assert_eq!(
            world.btech.vehicles()[&observer]
                .target_lock()
                .unwrap()
                .target,
            target
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(world.btech, restored.btech);
        let expected = scan_battle_unit(&world, observer, ObjectId(1), target, "").unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        assert!(support::run_text(&scripts, &config, ObjectId(1), 1, "scan").contains(&expected));
        let report = report_battle_unit(&scripts.world(), observer, ObjectId(1), target).unwrap();
        assert!(support::run_text(&scripts, &config, ObjectId(1), 1, "report").contains(&report));
        let before = scripts.world().btech.clone();
        scripts.drain_outbox();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.lock({},1,nil); error('abort')",
                    observer.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
        let text = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("lock #{}", target.0),
        );
        assert!(text.contains("Target set"), "{text}");
        let selected: i64 = scripts
            .eval_callback(&format!(
                "return btech.unit.state({}).target_lock.target",
                observer.0
            ))
            .unwrap();
        assert_eq!(selected, target.0);
        support::run_text(&scripts, &config, ObjectId(1), 1, "lock 0 0");
        assert!(
            scripts.world().btech.vehicles()[&observer]
                .target_lock()
                .is_none()
        );
        assert!(
            scripts.world().btech.vehicles()[&observer]
                .hex_lock()
                .is_some()
        );
        let scan = support::run_text(&scripts, &config, ObjectId(1), 1, "scan");
        assert!(scan.contains("Type: MECH"), "{scan}");
        for mode in ["H", "B", "I", "C"] {
            let text = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("lock 0 1 {mode}"),
            );
            assert!(text.contains("Target coordinates set"), "{text}");
            let scan = support::run_text(&scripts, &config, ObjectId(1), 1, "scan");
            assert!(!scan.contains("not constructed"), "{scan}");
        }
        support::run_text(&scripts, &config, ObjectId(1), 1, "lock -");
        assert!(
            scripts.world().btech.vehicles()[&observer]
                .target_selection()
                .is_none()
        );
    }
}

/// Losing every perception channel, placement, removal, shutdown and destruction clear vehicle
/// locks; light changes alone do not.
#[tokio::test]
async fn vehicle_locks_clear_on_visibility_sensor_placement_and_power_changes() {
    let (_dir, config, mut initial, [mech, _, observer, vehicle]) = formation().await;
    let map = initial.btech.vehicles()[&observer].position().unwrap().map;
    // Sight still reaches a target sharing the observer's hex, so hold this one four hexes off.
    power(&mut initial, &[vehicle], Power::Off);
    place_battle_unit(&mut initial, vehicle, map, 0, 4).unwrap();
    power(&mut initial, &[vehicle], Power::Running);
    initial.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
    assign_battle_pilot(&mut initial, observer, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut initial, ObjectId(1), support::FIXTURE_DICE_SEED);
    refresh_battle_contacts(&mut initial, &[observer]).unwrap();
    select_battle_target(&mut initial, observer, ObjectId(1), Some(vehicle)).unwrap();
    let mut world = initial.clone();
    // Pitch black sees three hexes; with a sweep of 12, which finds nothing, nothing reaches
    // the target.
    world
        .btech
        .set_unit_sensor_sweep(observer, Some(12))
        .unwrap();
    set_battle_map_conditions(&mut world, map, Light::PitchBlack, Weather::Clear).unwrap();
    let events = refresh_battle_contacts(&mut world, &[observer]).unwrap();
    assert!(
        events
            .iter()
            .any(|event| event.target == vehicle && event.lock_lost)
    );
    assert!(
        world.btech.vehicles()[&observer]
            .target_selection()
            .is_none()
    );
    let mut world = initial.clone();
    power(&mut world, &[vehicle], Power::Off);
    place_battle_unit(&mut world, vehicle, map, 0, 1).unwrap();
    assert!(
        world.btech.vehicles()[&observer]
            .target_selection()
            .is_none()
    );
    let mut world = initial.clone();
    select_battle_target(&mut world, observer, ObjectId(1), Some(mech)).unwrap();
    power(&mut world, &[mech], Power::Off);
    remove_battle_unit(&mut world, mech, ObjectId(config.home())).unwrap();
    assert!(
        world.btech.vehicles()[&observer]
            .target_selection()
            .is_none()
    );
    let mut world = initial.clone();
    stop_battle_unit(&mut world, observer, ObjectId(1)).unwrap();
    assert!(
        world.btech.vehicles()[&observer]
            .target_selection()
            .is_none()
    );
    let mut world = initial.clone();
    // Light changes wait for the next scan, where moonless-night sight still reaches five hexes.
    set_battle_map_conditions(&mut world, map, Light::MoonlessNight, Weather::Clear).unwrap();
    assert!(world.btech.vehicles()[&observer].target_lock().is_some());
    refresh_battle_contacts(&mut world, &[observer]).unwrap();
    assert!(world.btech.vehicles()[&observer].target_lock().is_some());
    select_battle_hex_target(
        &mut world,
        observer,
        ObjectId(1),
        HexCoordinate { x: 0, y: 1 },
        HexTargetMode::Building,
    )
    .unwrap();
    let coordinate_target = world.btech.vehicles()[&observer].target_selection();
    set_battle_map_conditions(&mut world, map, Light::Day, Weather::Clear).unwrap();
    refresh_battle_contacts(&mut world, &[observer]).unwrap();
    assert_eq!(
        world.btech.vehicles()[&observer].target_selection(),
        coordinate_target
    );
    let mut world = initial.clone();
    world
        .objects
        .get_mut(&vehicle)
        .unwrap()
        .flags
        .insert(Flag::Going);
    persistence::save(&config.database(), &world).await.unwrap();
    persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
        dbck::plan(&world, raw, &config)
    })
    .await
    .unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(
        restored.btech.vehicles()[&observer]
            .target_selection()
            .is_none()
    );
}

#[tokio::test]
async fn vehicle_lock_snapshots_reject_invalid_targets_coordinates_and_power() {
    let (_dir, config, mut world, [_, _, observer, target]) = formation().await;
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
    assign_battle_pilot(&mut world, observer, ObjectId(1)).unwrap();
    refresh_battle_contacts(&mut world, &[observer]).unwrap();
    select_battle_target(&mut world, observer, ObjectId(1), Some(target)).unwrap();
    for value in [
        serde_json::json!({"target": observer.0}),
        serde_json::json!({"target":999999}),
        serde_json::json!({"hex":{"x":-1,"y":0},"mode":"hex"}),
    ] {
        let mut bad = world.clone();
        bad.btech
            .rewrite_unit_record(observer, |record| {
                record["target_lock"] = value;
            })
            .unwrap();
        assert!(bad.validate(&config).is_err());
    }
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][observer.0.to_string()]["power"] = serde_json::to_value(Power::Off).unwrap();
    assert!(serde_json::from_value::<BtechState>(saved).is_err());
}
