//! Hovercraft surface travel preserves water and ice, support height, and saved motion replay.
use stompymux_rs::*;
mod support;

#[tokio::test]
async fn hovercraft_cross_deep_water_and_level_shore_with_mixed_unit_range() {
    cross_surface(".0.0.0~1~3~7~9~4~2~1.0.0.0.0.0.0.0.0.0.0").await;
}

#[tokio::test]
async fn hovercraft_cross_ice_and_water_without_breaking_ice_or_losing_speed() {
    cross_surface(".0.0.0-1-3-7~9-4-2-1.0.0.0.0.0.0.0.0.0.0").await;
}

/// Cross a surface corridor with deep-water geometry and a restart during travel.
async fn cross_surface(row: &str) {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Shore".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "shore",
        BattleMapAsset::parse(&format!("20 3\n{row}\n{row}\n{row}\n")).unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Hovercraft".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse(include_str!("../game/mechs/Fulcrum")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 2, 1).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
    for _ in 0..10 {
        advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
    }
    set_battle_speed(&mut world, id, ObjectId(1), 107.5).unwrap();
    let mut smoke = serde_json::to_value(&world.btech).unwrap();
    smoke["maps"][map.0.to_string()]["decorations"]["25"] = serde_json::to_value(
        BattleDecoration::new(BattleDecorationKind::Smoke, 120, None),
    )
    .unwrap();
    world.btech = serde_json::from_value(smoke).unwrap();
    let mech = world.create(&config, "Submerged Jenner".into(), Kind::Thing);
    world.objects.get_mut(&mech).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        mech,
        BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, mech, map, 6, 2).unwrap();
    let terrain_before =
        serde_json::to_value(&world.btech.maps()[&map]).unwrap()["terrain"].clone();
    let mut visited_water = false;
    let mut replayed = false;
    let mut reached_shore = false;
    for _ in 0..100 {
        let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        assert!(notices.is_empty(), "{notices:?}");
        let position = world.btech.vehicles()[&id].position().unwrap();
        assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(0));
        if (3..10).contains(&position.x) {
            visited_water = true;
            let range = battle_unit_range(&world, id, mech).unwrap();
            assert!((range.spatial - range.horizontal.hypot(9.0 / 5.0)).abs() < 1e-12);
            if !replayed {
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                let mut expected = world.clone();
                assert_eq!(
                    advance_battle_motion(&mut restored, BattleMovementRules::STANDARD).unwrap(),
                    advance_battle_motion(&mut expected, BattleMovementRules::STANDARD).unwrap()
                );
                assert_eq!(restored.btech, expected.btech);
                replayed = true;
            }
        }
        if position.x >= 10 {
            reached_shore = true;
            break;
        }
    }
    assert!(visited_water && replayed && reached_shore);
    assert_eq!(
        serde_json::to_value(&world.btech.maps()[&map]).unwrap()["terrain"],
        terrain_before,
        "Hover travel must not fracture ice"
    );
    assert_eq!(world.btech.vehicles()[&id].motion().unwrap().speed, 107.5);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert_eq!(
        scripts
            .eval_callback::<i32>(&format!("return btech.unit.state({}).elevation", id.0))
            .unwrap(),
        0
    );
}
