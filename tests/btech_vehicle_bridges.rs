//! Vehicle deck and hovercraft under-span travel retain distinct support height through replay.
use crate::support;
use stompymux_rs::*;

/// A running vehicle faces east toward a sequence of bridge spans.
async fn fixture(hover: bool) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Bridge".into(), Kind::Room);
    let row = if hover {
        ".0.0~5/3/4/3/2/3~7.0.0.0"
    } else {
        ".2.2.2/3/3/3/3/3.2.2.2.2"
    };
    create_battle_map(
        &mut world,
        map,
        "bridge",
        BattleMapAsset::parse(&format!("12 3\n{row}\n{row}\n{row}\n")).unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse(
            "test",
            if hover {
                include_str!("../game/mechs/Fulcrum.toml")
            } else {
                include_str!("../game/mechs/Flatbed_Truck.toml")
            },
        )
        .unwrap(),
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
    set_battle_speed(&mut world, id, ObjectId(1), 53.75).unwrap();
    (dir, config, world, id, map)
}

#[tokio::test]
async fn vehicles_cross_decks_and_hover_under_spans_without_changing_water_height() {
    for (hover, ice) in [(false, false), (true, false), (true, true)] {
        let (_dir, config, mut world, id, map) = fixture(hover).await;
        if ice {
            let mut encoded = serde_json::to_value(&world.btech).unwrap();
            for row in 0..3 {
                for column in [2, 8] {
                    encoded["maps"][map.0.to_string()]["terrain"][row * 12 + column]["terrain"] =
                        serde_json::to_value(Terrain::Ice).unwrap();
                }
            }
            world.btech = serde_json::from_value(encoded).unwrap();
        }
        let mut on_bridge = false;
        let mut exited = false;
        for _ in 0..150 {
            let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
            assert!(notices.is_empty(), "{hover}: {notices:?}");
            let x = world.btech.vehicles()[&id].position().unwrap().x;
            if (3..8).contains(&x) {
                assert_eq!(world.btech.vehicles()[&id].under_bridge(), hover);
                assert_eq!(
                    battle_unit_elevation(&world, id).unwrap(),
                    Some(if hover { 0 } else { 3 })
                );
                if !on_bridge {
                    persistence::save(&config.database(), &world).await.unwrap();
                    let mut restored = persistence::load(&config.database()).await.unwrap();
                    let mut next = world.clone();
                    assert_eq!(
                        advance_battle_motion(&mut restored, BattleMovementRules::STANDARD)
                            .unwrap(),
                        advance_battle_motion(&mut next, BattleMovementRules::STANDARD).unwrap()
                    );
                    assert_eq!(restored.btech, next.btech);
                    on_bridge = true;
                }
            }
            if x >= 8 {
                assert!(!world.btech.vehicles()[&id].under_bridge());
                assert_eq!(
                    battle_unit_elevation(&world, id).unwrap(),
                    Some(if hover { 0 } else { 2 })
                );
                exited = true;
                break;
            }
        }
        assert!(on_bridge && exited);
    }
}

#[tokio::test]
async fn low_spans_stop_hovercraft_and_inconsistent_saved_underpass_state_is_rejected() {
    let (_dir, config, mut world, id, map) = fixture(true).await;
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    for row in 0..3 {
        encoded["maps"][map.0.to_string()]["terrain"][row * 12 + 5]["elevation"] = 1.into();
    }
    world.btech = serde_json::from_value(encoded).unwrap();
    let mut stopped = false;
    for _ in 0..100 {
        let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        if notices
            .iter()
            .any(|notice| notice.text.contains("underside of the bridge in front"))
        {
            stopped = true;
            break;
        }
    }
    assert!(stopped);
    assert!(world.btech.vehicles()[&id].under_bridge());
    assert!(!world.btech.vehicles()[&id].motion().unwrap().active());
    persistence::save(&config.database(), &world).await.unwrap();
    let position = world.btech.vehicles()[&id].position().unwrap();
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["maps"][map.0.to_string()]["terrain"]
        [usize::from(position.y) * 12 + usize::from(position.x)]["terrain"] =
        serde_json::to_value(Terrain::Grassland).unwrap();
    world.btech = serde_json::from_value(encoded).unwrap();
    assert!(persistence::save(&config.database(), &world).await.is_err());
    assert!(
        persistence::load(&config.database())
            .await
            .unwrap()
            .btech
            .vehicles()[&id]
            .under_bridge()
    );
}

#[tokio::test]
async fn low_spans_resolve_control_or_impact_before_restoring_hovercraft_position() {
    for (success, skid, piloted) in [
        (true, false, true),
        (false, false, true),
        (false, true, true),
        (true, true, false),
    ] {
        let (_dir, config, mut world, id, map) = fixture(true).await;
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let value = &mut saved["vehicles"][id.0.to_string()];
        value["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        value["piloting_damage"] = if success { 0 } else { 100 }.into();
        if !piloted {
            value["pilot"] = serde_json::Value::Null;
        }
        for row in 0..3 {
            saved["maps"][map.0.to_string()]["terrain"][row * 12 + 5]["elevation"] = 1.into();
        }
        world.btech = serde_json::from_value(saved).unwrap();
        let rules = BattleMovementRules {
            skid_cliff: skid,
            ..BattleMovementRules::STANDARD
        };
        let mut collided = false;
        for _ in 0..150 {
            let before = world.clone();
            let notices = advance_battle_motion(&mut world, rules).unwrap();
            if !notices
                .iter()
                .any(|n| n.text.contains("underside of the bridge in front"))
            {
                continue;
            }
            collided = true;
            let unit = &world.btech.vehicles()[&id];
            assert_eq!(unit.position(), before.btech.vehicles()[&id].position());
            assert_eq!(
                unit.motion().unwrap().point,
                before.btech.vehicles()[&id].motion().unwrap().point
            );
            assert_eq!(battle_unit_elevation(&world, id).unwrap(), Some(0));
            assert!(unit.under_bridge());
            assert!(!unit.motion().unwrap().active());
            assert!(!unit.flooded());
            let mut expected = before.clone();
            if piloted {
                let _check =
                    roll_battle_piloting(&mut expected, id, 0, rules.fall.extended_piloting)
                        .unwrap();
            }
            if success {
                assert_eq!(unit.sections(), before.btech.vehicles()[&id].sections());
                assert!(
                    notices
                        .iter()
                        .any(|n| n.text.contains("manage to stop before slamming"))
                );
            } else {
                assert_ne!(unit.sections(), before.btech.vehicles()[&id].sections());
                assert!(
                    notices
                        .iter()
                        .any(|n| n.text.contains("drive right into the underside"))
                );
                let _fall = resolve_battle_vehicle_fall(&mut expected, id, 1, rules.fall).unwrap();
            }
            assert_eq!(
                serde_json::to_value(unit).unwrap()["dice"],
                serde_json::to_value(&expected.btech.vehicles()[&id]).unwrap()["dice"]
            );
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut restored, rules).unwrap()
            );
            assert_eq!(world.btech, restored.btech);
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
            break;
        }
        assert!(collided);
    }
}
