//! Mixed Mech/vehicle placement preserves shared membership and durable containment.
use crate::support;
use stompymux_rs::*;

#[tokio::test]
async fn vehicles_share_map_slots_and_survive_placement_replay_and_map_purge() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Battlefield".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::parse("3 2\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    let mech = world.create(&config, "Jenner".into(), Kind::Thing);
    let vehicle = world.create(&config, "Demolisher".into(), Kind::Thing);
    for id in [mech, vehicle] {
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        world.objects.get_mut(&id).unwrap().location = Some(ObjectId(config.start()));
    }
    create_battle_unit(
        &mut world,
        mech,
        BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap(),
    )
    .unwrap();
    create_battle_vehicle(
        &mut world,
        vehicle,
        BattleVehicleTemplate::parse(include_str!("../game/mechs/Demolisher")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, vehicle, map, 0, 1).unwrap();
    place_battle_unit(&mut world, mech, map, 1, 1).unwrap();
    assert_eq!(world.btech.vehicles()[&vehicle].map_slot(), Some(0));
    assert_eq!(world.btech.constructed_units()[&mech].map_slot(), Some(1));
    place_battle_unit(&mut world, vehicle, map, 2, 0).unwrap();
    assert_eq!(world.btech.vehicles()[&vehicle].map_slot(), Some(0));
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    let before = loaded.clone();
    for (x, y) in [(-1, 0), (3, 0), (0, 2)] {
        assert!(place_battle_unit(&mut loaded, vehicle, map, x, y).is_err());
    }
    assert!(remove_battle_unit(&mut loaded, vehicle, map).is_err());
    assert_eq!(loaded.btech, before.btech);
    assert_eq!(loaded.objects[&vehicle].location, Some(map));
    loaded.objects.get_mut(&vehicle).unwrap().location = Some(ObjectId(config.start()));
    assert!(
        persistence::save(&config.database(), &loaded)
            .await
            .is_err()
    );
    loaded = before;
    remove_battle_unit(&mut loaded, mech, ObjectId(config.start())).unwrap();
    assert!(
        reload_battle_map(
            &mut loaded,
            map,
            "test",
            BattleMapAsset::parse("3 2\n.0.0.0\n.0.0.0\n").unwrap()
        )
        .is_err()
    );
    remove_battle_unit(&mut loaded, vehicle, ObjectId(config.start())).unwrap();
    place_battle_unit(&mut loaded, mech, map, 0, 0).unwrap();
    place_battle_unit(&mut loaded, vehicle, map, 1, 0).unwrap();
    assert_eq!(loaded.btech.constructed_units()[&mech].map_slot(), Some(0));
    assert_eq!(loaded.btech.vehicles()[&vehicle].map_slot(), Some(1));
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    loaded
        .objects
        .get_mut(&map)
        .unwrap()
        .flags
        .insert(Flag::Going);
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
        dbck::plan(&loaded, raw, &config)
    })
    .await
    .unwrap();
    let purged = persistence::load(&config.database()).await.unwrap();
    assert!(purged.btech.vehicles()[&vehicle].position().is_none());
    assert!(purged.btech.vehicles()[&vehicle].map_slot().is_none());
    assert!(purged.btech.units()[&vehicle].map.is_none());
    assert_ne!(purged.objects[&vehicle].location, Some(map));
}
