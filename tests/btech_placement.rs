//! Battlefield placement, persisted containment, failure atomicity and map destruction.
use crate::support;
use stompymux_rs::{
    BattleMapAsset, BattleTemplate, Flag, Kind, ObjectId, Scripts, create_battle_map,
    create_battle_unit, dbck, persistence, place_battle_unit, reload_battle_map,
    remove_battle_unit,
};
const JENNER: &str = include_str!("fixtures/btech/mechs/JR7-D.toml");
const MAP: &str = "3 2\n.0.0.0\n.0~2.0\n";

#[tokio::test]
async fn placement_round_trips_coordinates_and_containment_without_shared_unit_state() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Battlefield".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test.map",
        BattleMapAsset::from_cells(MAP).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for name in ["Alpha", "Bravo"] {
        let id = world.create(&config, name.into(), Kind::Thing);
        let object = world.objects.get_mut(&id).unwrap();
        object.location = Some(ObjectId(config.start()));
        object.home = Some(ObjectId(config.home()));
        create_battle_unit(
            &mut world,
            id,
            BattleTemplate::parse("JR7-D", JENNER).unwrap(),
        )
        .unwrap();
        ids.push(id);
    }
    place_battle_unit(&mut world, ids[0], map, 0, 0).unwrap();
    place_battle_unit(&mut world, ids[1], map, 2, 1).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    for id in &ids {
        assert_eq!(loaded.objects[id].location, Some(map));
        assert_eq!(loaded.btech.units()[id].map, Some(map));
    }
    let before = loaded.clone();
    assert!(place_battle_unit(&mut loaded, ids[0], map, 3, 0).is_err());
    assert!(place_battle_unit(&mut loaded, ids[0], map, -1, 0).is_err());
    assert!(place_battle_unit(&mut loaded, ids[0], ObjectId(999999), 0, 0).is_err());
    assert!(
        reload_battle_map(
            &mut loaded,
            map,
            "test.map",
            BattleMapAsset::from_cells(MAP).unwrap()
        )
        .is_err()
    );
    assert_eq!(loaded.btech, before.btech);
    assert_eq!(loaded.objects[&ids[0]].location, Some(map));
    remove_battle_unit(&mut loaded, ids[0], ObjectId(config.start())).unwrap();
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert!(
        loaded.btech.constructed_units()[&ids[0]]
            .position()
            .is_none()
    );
    assert_eq!(
        loaded.btech.constructed_units()[&ids[1]]
            .position()
            .unwrap()
            .x,
        2
    );
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
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert!(!loaded.btech.maps().contains_key(&map));
    for id in ids {
        assert!(loaded.btech.constructed_units()[&id].position().is_none());
        assert_eq!(loaded.btech.units()[&id].map, None);
        assert_ne!(loaded.objects[&id].location, Some(map));
    }
}

#[tokio::test]
async fn placement_callbacks_roll_back_and_native_commands_report_coordinates() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Battlefield".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test.map",
        BattleMapAsset::from_cells(MAP).unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Jenner".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().location = Some(ObjectId(config.start()));
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse("JR7-D", JENNER).unwrap(),
    )
    .unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let before = scripts.world().clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.place({},{},1,1); error('abort')",
                id.0, map.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before.btech);
    assert_eq!(
        scripts.world().objects[&id].location,
        before.objects[&id].location
    );
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech unit-place #{}=#{},1,1", id.0, map.0),
    );
    assert!(text.contains("placed"), "{text}");
    let text = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech inspect #{}", id.0),
    );
    assert!(text.contains("Hex 1,1"), "{text}");
    let tuple: (i64, u16, u16) = scripts
        .eval_callback(&format!(
            "local p=btech.unit.state({}).position; return p.map,p.x,p.y",
            id.0
        ))
        .unwrap();
    assert_eq!(tuple, (map.0, 1, 1));
    let candidate = scripts.world().clone();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    let mut mismatched = candidate.clone();
    mismatched.objects.get_mut(&id).unwrap().location = Some(ObjectId(config.start()));
    assert!(
        persistence::save(&config.database(), &mismatched)
            .await
            .is_err()
    );
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        candidate.btech
    );
    scripts
        .eval_callback::<()>(&format!("btech.unit.remove({},{})", id.0, config.start()))
        .unwrap();
    assert!(
        scripts.world().btech.constructed_units()[&id]
            .position()
            .is_none()
    );
    reload_battle_map(
        &mut scripts.world_mut(),
        map,
        "changed.map",
        BattleMapAsset::from_cells(&format!("{MAP}1: 100 20\n")).unwrap(),
    )
    .unwrap();
    let candidate = scripts.world().clone();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database())
            .await
            .unwrap()
            .btech
            .maps()[&map]
            .flags,
        1
    );
}

/// Map membership follows insertion slots, reuses holes, and survives restart independently of object IDs.
#[tokio::test]
async fn map_slots_control_occupant_order_and_reuse() {
    use stompymux_rs::*;
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Slots".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "slots.map",
        BattleMapAsset::from_cells(MAP).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for name in ["Observer", "Earlier ID", "Later ID", "Replacement"] {
        let id = world.create(&config, name.into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().location = Some(ObjectId(config.start()));
        create_battle_unit(
            &mut world,
            id,
            BattleTemplate::parse("JR7-D", JENNER).unwrap(),
        )
        .unwrap();
        ids.push(id);
    }
    let [observer, earlier, later, replacement] = <[ObjectId; 4]>::try_from(ids).unwrap();
    place_battle_unit(&mut world, observer, map, 0, 0).unwrap();
    place_battle_unit(&mut world, later, map, 2, 0).unwrap();
    place_battle_unit(&mut world, earlier, map, 2, 0).unwrap();
    let hex = BattleHexCoordinate { x: 2, y: 0 };
    assert_eq!(
        battle_map_unit_order(&world, map).unwrap(),
        [observer, later, earlier]
    );
    assert_eq!(
        battle_hex_occupant(&world, observer, hex).unwrap(),
        Some(later)
    );
    assert_eq!(world.btech.constructed_units()[&later].map_slot(), Some(1));
    place_battle_unit(&mut world, later, map, 2, 0).unwrap();
    assert_eq!(world.btech.constructed_units()[&later].map_slot(), Some(1));
    let before = world.btech.clone();
    assert!(place_battle_unit(&mut world, replacement, map, 99, 0).is_err());
    assert_eq!(world.btech, before);
    remove_battle_unit(&mut world, later, ObjectId(config.start())).unwrap();
    assert_eq!(world.btech.constructed_units()[&later].map_slot(), None);
    place_battle_unit(&mut world, replacement, map, 2, 0).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&replacement].map_slot(),
        Some(1)
    );
    assert_eq!(
        battle_hex_occupant(&world, observer, hex).unwrap(),
        Some(replacement)
    );
    place_battle_unit(&mut world, later, map, 2, 0).unwrap();
    assert_eq!(world.btech.constructed_units()[&later].map_slot(), Some(3));
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(
        battle_map_unit_order(&loaded, map).unwrap(),
        [observer, replacement, earlier, later]
    );
    assert_eq!(
        battle_hex_occupant(&loaded, observer, hex).unwrap(),
        Some(replacement)
    );
    for invalid in ["missing", "duplicate", "unplaced"] {
        let mut state = serde_json::to_value(&world.btech).unwrap();
        match invalid {
            "missing" => {
                state["constructed"][earlier.0.to_string()]["map_slot"] = serde_json::Value::Null
            }
            "duplicate" => state["constructed"][earlier.0.to_string()]["map_slot"] = 1.into(),
            _ => state["constructed"][earlier.0.to_string()]["position"] = serde_json::Value::Null,
        }
        let mut candidate = world.clone();
        candidate.btech = serde_json::from_value(state).unwrap();
        assert!(candidate.validate(&config).is_err(), "{invalid}");
    }
    let other_map = world.create(&config, "Other slots".into(), Kind::Room);
    create_battle_map(
        &mut world,
        other_map,
        "other.map",
        BattleMapAsset::from_cells(MAP).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, earlier, other_map, 0, 0).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&earlier].map_slot(),
        Some(0)
    );
    assert_eq!(
        battle_map_unit_order(&world, map).unwrap(),
        [observer, replacement, later]
    );
    world.validate(&config).unwrap();
    place_battle_unit(&mut world, earlier, map, 2, 0).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&earlier].map_slot(),
        Some(2)
    );
    world.validate(&config).unwrap();
}
