//! Scenario map changes preserve live units and roll back invalid destination changes.
use crate::support;
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Running chassis keep crew and controls across same-map and cross-map ID assignment.
#[tokio::test]
async fn map_assignment_preserves_running_chassis_and_restarts() {
    for source in firing::templates() {
        let (_dir, config, mut world, unit, other, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let map = world.create(&config, "Destination".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "destination",
            BattleMapAsset::from_cells(&format!("1 12\n{}", ".2\n".repeat(12))).unwrap(),
        )
        .unwrap();
        let key = if world.btech.vehicles().contains_key(&unit) {
            "vehicles"
        } else {
            "constructed"
        };
        let stationary =
            world.btech.vehicles().get(&unit).is_some_and(|unit| {
                unit.definition().movement == BattleVehicleMovement::Stationary
            });
        firing::edit(&mut world, unit, |state| {
            state["motion"]["heading"] = 90.0.into();
            state["motion"]["desired_heading"] = 90.0.into();
            if !stationary {
                state["motion"]["speed"] = 1.0.into();
                state["motion"]["desired_speed"] = 2.0.into();
            }
            if key == "constructed" {
                state["tag"]["remaining"] = 5.into();
            }
        });
        world.validate(&config).unwrap();
        let before = serde_json::to_value(&world.btech).unwrap();
        let report = reassign_battle_map(&mut world, unit, map, Some("QX")).unwrap();
        assert_eq!(report.label, "QX");
        assert!(!report.reset_origin);
        assert_eq!(report.position.y, 11);
        assert_eq!(world.objects[&unit].location, Some(map));
        assert_eq!(battle_unit_elevation(&world, unit).unwrap(), Some(0));
        let after = serde_json::to_value(&world.btech).unwrap();
        for field in ["power", "pilot", "motion", "sections", "dice", "tag"] {
            assert_eq!(
                before[key][unit.0.to_string()][field],
                after[key][unit.0.to_string()][field],
                "{field}"
            );
        }
        assert!(after[key][unit.0.to_string()]["target_lock"].is_null());
        assert!(
            after[key][unit.0.to_string()]["contacts"]
                .as_object()
                .unwrap()
                .is_empty()
        );
        let report = reassign_battle_map(&mut world, unit, map, Some("QY")).unwrap();
        assert_eq!(report.label, "QY");
        assert_eq!(
            serde_json::to_value(&world.btech).unwrap()[key][unit.0.to_string()]["map_slot"],
            0
        );
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        for invalid in [ObjectId(-2), ObjectId(1), ObjectId(99999), other] {
            let before = world.clone();
            assert!(reassign_battle_map(&mut world, unit, invalid, None).is_err());
            assert_eq!(world.btech, before.btech);
            assert_eq!(
                world.objects[&unit].location,
                before.objects[&unit].location
            );
        }
        let small = world.create(&config, "Small".into(), Kind::Room);
        create_battle_map(
            &mut world,
            small,
            "small",
            BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
        )
        .unwrap();
        let report = reassign_battle_map(&mut world, unit, small, Some("QX")).unwrap();
        assert!(report.reset_origin);
        assert_eq!((report.position.x, report.position.y), (0, 0));
        world.validate(&config).unwrap();
    }
}

/// Unplaced, stopped construction can enter through the same scenario service.
#[tokio::test]
async fn new_units_use_destination_surface() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Destination".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "small",
        BattleMapAsset::from_cells("1 1\n.2\n").unwrap(),
    )
    .unwrap();
    for source in firing::templates() {
        let unit = world.create(&config, "Unit".into(), Kind::Thing);
        world.objects.get_mut(&unit).unwrap().home = Some(ObjectId(config.home()));
        BattleUnitTemplate::parse("test", &source)
            .unwrap()
            .create(&mut world, unit)
            .unwrap();
        let report = reassign_battle_map(&mut world, unit, map, Some("AB")).unwrap();
        assert_eq!(report.position.map, map);
        assert_eq!(battle_unit_elevation(&world, unit).unwrap(), Some(2));
        world.validate(&config).unwrap();
    }
}

/// A full destination rejects entry before either durable dice or source membership changes.
#[tokio::test]
async fn map_capacity_counts_all_chassis_and_allows_existing_members() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Full map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "full",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let sources = firing::templates();
    let mut first = None;
    let mut overflow = None;
    for index in 0..251 {
        let id = world.create(&config, "Unit".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        BattleUnitTemplate::parse("test", &sources[index % sources.len()])
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        if index < 250 {
            place_battle_unit(&mut world, id, map, 0, 0).unwrap();
            first.get_or_insert(id);
        } else {
            overflow = Some(id);
        }
    }
    let before = world.clone();
    let error = reassign_battle_map(&mut world, overflow.unwrap(), map, None).unwrap_err();
    assert!(error.to_string().contains("too many mechs"), "{error:#}");
    assert_eq!(world.btech, before.btech);
    reassign_battle_map(&mut world, first.unwrap(), map, Some("ZZ")).unwrap();
    world.validate(&config).unwrap();
}

/// An active jump retains its route and sub-hex cursor on an equally sized destination.
#[tokio::test]
async fn map_assignment_preserves_airborne_progress_and_restart() {
    for index in [0, 6] {
        let source = &firing::templates()[index];
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(source, None, source).await;
        if index == 0 {
            launch_battle_jump(&mut world, unit, ObjectId(1), 0, 3.0).unwrap();
            advance_battle_jumps(&mut world, BattleMovementRules::STANDARD).unwrap();
        } else {
            let _ = begin_battle_vtol_takeoff(&mut world, unit, ObjectId(1), 0, false).unwrap();
            advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        }
        let map = world.create(&config, "Airspace".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "airspace",
            BattleMapAsset::from_cells(&format!("1 12\n{}", ".0\n".repeat(12))).unwrap(),
        )
        .unwrap();
        let key = if index == 0 {
            "constructed"
        } else {
            "vehicles"
        };
        let before = serde_json::to_value(&world.btech).unwrap();
        reassign_battle_map(&mut world, unit, map, Some("XY")).unwrap();
        let after = serde_json::to_value(&world.btech).unwrap();
        for field in ["motion", "flight", "vtol_flight", "power", "pilot", "dice"] {
            assert_eq!(
                before[key][unit.0.to_string()][field],
                after[key][unit.0.to_string()][field],
                "{field}"
            );
        }
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        advance_battle_jumps(&mut world, BattleMovementRules::STANDARD).unwrap();
        advance_battle_jumps(&mut restored, BattleMovementRules::STANDARD).unwrap();
        advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        advance_battle_motion(&mut restored, BattleMovementRules::STANDARD).unwrap();
        assert_eq!(world.btech, restored.btech);
    }
}

/// Naming either member releases a mixed tow without silently moving its partner.
#[tokio::test]
async fn map_assignment_releases_tows_across_all_chassis_pairings() {
    let sources = firing::templates();
    for source in &sources {
        for target in &sources {
            let (_dir, config, mut world) = support::isolated_world().await;
            let old = world.create(&config, "Old map".into(), Kind::Room);
            let new = world.create(&config, "New map".into(), Kind::Room);
            for map in [old, new] {
                create_battle_map(
                    &mut world,
                    map,
                    "map",
                    BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
                )
                .unwrap();
            }
            let a = world.create(&config, "Carrier".into(), Kind::Thing);
            let b = world.create(&config, "Tow".into(), Kind::Thing);
            for (id, source) in [(a, source), (b, target)] {
                world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
                BattleUnitTemplate::parse("test", source)
                    .unwrap()
                    .create(&mut world, id)
                    .unwrap();
                place_battle_unit(&mut world, id, old, 0, 0).unwrap();
            }
            set_battle_tow(&mut world, a, Some(b)).unwrap();
            for (id, partner) in [(a, b), (b, a)] {
                let mut changed = world.clone();
                reassign_battle_map(&mut changed, id, new, Some("XY")).unwrap();
                assert!(changed.btech.tows().is_empty());
                assert_eq!(changed.objects[&id].location, Some(new));
                assert_eq!(changed.objects[&partner].location, Some(old));
                changed.validate(&config).unwrap();
            }
        }
    }
}
