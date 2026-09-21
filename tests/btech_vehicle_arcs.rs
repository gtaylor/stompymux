//! Hull and turret firing arcs, damage eligibility, angle boundaries and persisted facing.
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
        BattleVehicleTemplate::parse(include_str!("../game/mechs/Demolisher")).unwrap(),
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

#[test]
fn vehicle_mount_arcs_partition_hull_and_keep_turret_narrow() {
    let target = BattleVehicle::new(
        BattleVehicleTemplate::parse(include_str!("../game/mechs/Demolisher")).unwrap(),
    )
    .unwrap();
    let mut mount = target.loadout().unwrap().weapons[0].clone();
    for (section, inside, outside) in [
        (
            BattleVehicleSection::Front,
            vec![0.0, 60.0, 300.0, 359.0],
            vec![61.0, 180.0, 299.0],
        ),
        (
            BattleVehicleSection::Rear,
            vec![121.0, 180.0, 239.0],
            vec![120.0, 240.0, 0.0],
        ),
        (
            BattleVehicleSection::Left,
            vec![240.0, 270.0, 299.0],
            vec![239.0, 300.0, 90.0],
        ),
        (
            BattleVehicleSection::Right,
            vec![61.0, 90.0, 120.0],
            vec![60.0, 121.0, 270.0],
        ),
        (
            BattleVehicleSection::Turret,
            vec![330.0, 0.0, 30.0],
            vec![329.0, 31.0, 180.0],
        ),
    ] {
        mount.criticals[0].section = section;
        for rear in [false, true] {
            mount.rear_mount = rear;
            for angle in &inside {
                assert!(
                    mount.bears_on(0.0, *angle, Some(0.0)).unwrap(),
                    "{section:?} {angle}"
                );
            }
            for angle in &outside {
                assert!(
                    !mount.bears_on(0.0, *angle, Some(0.0)).unwrap(),
                    "{section:?} {angle}"
                );
            }
            for angle in &inside {
                assert!(mount.bears_on(720.0, *angle - 360.0, Some(-360.0)).unwrap());
            }
        }
    }
    assert!(mount.bears_on(0.0, 30.49, Some(0.0)).unwrap());
    assert!(!mount.bears_on(0.0, 30.5, Some(0.0)).unwrap());
    assert!(mount.bears_on(0.0, 31.0, Some(1.99)).unwrap());
    assert!(!mount.bears_on(0.0, 0.0, None).unwrap());
    for (heading, bearing, turret) in [
        (f64::NAN, 0.0, Some(0.0)),
        (0.0, f64::INFINITY, None),
        (0.0, 0.0, Some(f64::NAN)),
    ] {
        assert!(mount.bears_on(heading, bearing, turret).is_err());
    }
    mount.criticals.clear();
    assert!(mount.bears_on(0.0, 0.0, Some(0.0)).is_err());
}

#[tokio::test]
async fn vehicle_weapon_geometry_uses_saved_locked_turret_and_section_survival() {
    let (_dir, config, mut world, id) = fixture().await;
    set_battle_turret(&mut world, id, ObjectId(1), 90.0).unwrap();
    assert!(
        world.btech.vehicles()[&id]
            .weapon_bears_on(0, 90.0)
            .unwrap()
    );
    assert!(!world.btech.vehicles()[&id].weapon_bears_on(0, 0.0).unwrap());
    lock_battle_vehicle_turret(&mut world, id).unwrap();
    set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
    for _ in 0..20 {
        advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
    }
    assert!(
        world.btech.vehicles()[&id]
            .weapon_bears_on(0, 180.0)
            .unwrap()
    );
    assert!(
        !world.btech.vehicles()[&id]
            .weapon_bears_on(0, 90.0)
            .unwrap()
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert!(
        loaded.btech.vehicles()[&id]
            .weapon_bears_on(0, 180.0)
            .unwrap()
    );
    assert!(
        loaded.btech.vehicles()[&id]
            .weapon_bears_on(99, 180.0)
            .is_err()
    );
    damage_battle_vehicle_phase(
        &mut loaded,
        id,
        BattleVehicleSection::Turret,
        100,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    assert!(
        !loaded.btech.vehicles()[&id]
            .weapon_bears_on(0, 180.0)
            .unwrap()
    );
    let unplaced = BattleVehicle::new(loaded.btech.vehicles()[&id].definition().clone()).unwrap();
    assert!(unplaced.weapon_bears_on(0, 0.0).is_err());
}
