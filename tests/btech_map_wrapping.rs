//! Wrapped movement shares virtual-coordinate traversal and the saved linked-map marker.
use stompymux_rs::*;
mod support;

#[tokio::test]
async fn linked_map_movement_replays_for_mechs_ground_vehicles_and_aircraft() {
    for source in [
        include_str!("fixtures/btech/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
        include_str!("../game/mechs/Kestrel"),
    ] {
        for heading in [90, 270] {
            let (_dir, config, mut world) = support::isolated_world().await;
            let map = world.create(&config, "Wrapped field".into(), Kind::Room);
            create_battle_map(
                &mut world,
                map,
                "wrapped",
                BattleMapAsset::parse("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
            )
            .unwrap();
            set_battle_map_wrapping(&mut world, map, true).unwrap();
            let id = world.create(&config, "Traveler".into(), Kind::Thing);
            BattleUnitTemplate::parse(source)
                .unwrap()
                .create(&mut world, id)
                .unwrap();
            place_battle_unit(&mut world, id, map, if heading == 90 { 2 } else { 0 }, 1).unwrap();
            world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
            assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
            start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
            for _ in 0..5 {
                advance_battle_units(&mut world, 0);
            }
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            saved["maps"][map.0.to_string()]["movement_modifier"] = 6450.into();
            let key = if world.btech.vehicles().contains_key(&id) {
                "vehicles"
            } else {
                "constructed"
            };
            let state = &mut saved[key][id.0.to_string()];
            state["motion"]["heading"] = heading.into();
            state["motion"]["desired_heading"] = heading.into();
            state["motion"]["speed"] = 43.into();
            state["motion"]["desired_speed"] = 43.into();
            if world
                .btech
                .vehicles()
                .get(&id)
                .is_some_and(|unit| unit.definition().is_vtol())
            {
                state["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
                    phase: BattleVtolFlightPhase::Airborne,
                    altitude: 5.0,
                    vertical_speed: 0.0,
                    fall: None,
                })
                .unwrap();
            }
            world.btech = serde_json::from_value(saved).unwrap();
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            assert!(replay.btech.maps()[&map].wrapping());
            let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
            let other = advance_battle_motion(&mut replay, BattleMovementRules::STANDARD).unwrap();
            assert_eq!(notices, other);
            assert_eq!(world.btech, replay.btech);
            assert!(
                !notices
                    .iter()
                    .any(|notice| notice.text.contains("edge")
                        || notice.text.contains("off this map"))
            );
            let (position, motion) = if let Some(unit) = world.btech.vehicles().get(&id) {
                (unit.position().unwrap(), unit.motion().unwrap())
            } else {
                let unit = &world.btech.constructed_units()[&id];
                (unit.position().unwrap(), unit.motion().unwrap())
            };
            assert!(
                if heading == 90 {
                    position.x < 2
                } else {
                    position.x > 0
                },
                "{position:?}"
            );
            assert!(motion.speed > 0.0);
            assert_eq!(
                motion.point,
                BattleHexCoordinate {
                    x: i32::from(position.x),
                    y: i32::from(position.y)
                }
                .center()
            );
            world.validate(&config).unwrap();
            set_battle_map_wrapping(&mut world, map, false).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            assert!(
                !persistence::load(&config.database())
                    .await
                    .unwrap()
                    .btech
                    .maps()[&map]
                    .wrapping()
            );
        }
    }
}

/// A wrapped hill is detected at the seam, then elevation avoidance restores the departure hex.
#[tokio::test]
async fn aircraft_rolls_back_when_the_opposite_edge_is_too_high() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Seam hill".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "hill",
        BattleMapAsset::parse("3 1\n^9.0.0\n").unwrap(),
    )
    .unwrap();
    set_battle_map_wrapping(&mut world, map, true).unwrap();
    let id = world.create(&config, "Aircraft".into(), Kind::Thing);
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse(include_str!("../game/mechs/Kestrel")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 2, 0).unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["maps"][map.0.to_string()]["movement_modifier"] = 6450.into();
    let unit = &mut saved["vehicles"][id.0.to_string()];
    unit["power"] = serde_json::to_value(BattlePower::Running).unwrap();
    unit["motion"]["heading"] = 90.into();
    unit["motion"]["desired_heading"] = 90.into();
    unit["motion"]["speed"] = 100.into();
    unit["motion"]["desired_speed"] = 100.into();
    unit["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
        phase: BattleVtolFlightPhase::Airborne,
        altitude: 5.0,
        vertical_speed: 0.0,
        fall: None,
    })
    .unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut replay = persistence::load(&config.database()).await.unwrap();
    let mut rules = BattleMovementRules::STANDARD.fall;
    rules.vehicle_impact.criticals.enabled = false;
    let outcome = advance_battle_vtol_environment(&mut world, id, false, rules).unwrap();
    assert_eq!(
        advance_battle_vtol_environment(&mut replay, id, false, rules).unwrap(),
        outcome
    );
    assert_eq!(replay.btech, world.btech);
    let BattleVtolEnvironment::Obstacle {
        path:
            BattleVtolPath::Contact {
                hex,
                contact: BattleVtolSurfaceContact::Elevation,
                ..
            },
        fall: None,
        ..
    } = outcome
    else {
        panic!("Missing seam collision: {outcome:?}");
    };
    assert_eq!(hex, BattleHexCoordinate { x: 0, y: 0 });
    let unit = &world.btech.vehicles()[&id];
    assert_eq!(unit.position().unwrap().x, 2);
    assert_eq!(unit.vtol_flight().unwrap().altitude, 0.0);
    assert_eq!(
        unit.vtol_flight().unwrap().phase,
        BattleVtolFlightPhase::Landed
    );
    assert_eq!(unit.motion().unwrap().speed, 0.0);
    assert_eq!(unit.motion().unwrap().desired_speed, 100.0);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn imported_link_marker_payloads_survive_unchanged_saves() {
    use sqlx::Connection;
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Imported wrapping".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "import",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let options = sqlx::sqlite::SqliteConnectOptions::new().filename(config.database());
    let mut connection = sqlx::SqliteConnection::connect_with(&options)
        .await
        .unwrap();
    sqlx::query("INSERT INTO btech_map_objects VALUES (?,7,4,12,13,-1,14,15,16)")
        .bind(map.0)
        .execute(&mut connection)
        .await
        .unwrap();
    sqlx::query("INSERT INTO btech_map_objects VALUES (?,7,8,12,13,-1,24,25,26)")
        .bind(map.0)
        .execute(&mut connection)
        .await
        .unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech.maps()[&map].linked_markers().len(), 2);
    assert_eq!(
        loaded.btech.maps()[&map].linked_markers()[&4].coordinate,
        BattleHexCoordinate { x: 12, y: 13 }
    );
    assert!(loaded.btech.maps()[&map].wrapping());
    loaded.objects.get_mut(&map).unwrap().name = "Renamed wrapping".into();
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    let payload: (i64,i64,i64,i64,i64,i64) = sqlx::query_as("SELECT ordinal,x,y,data_char,data_short,data_int FROM btech_map_objects WHERE map_dbref=? AND object_type=7 AND ordinal=4").bind(map.0).fetch_one(&mut connection).await.unwrap();
    assert_eq!(payload, (4, 12, 13, 14, 15, 16));
    set_battle_linked_marker(
        &mut loaded,
        map,
        4,
        Some(BattleHexCoordinate { x: -10, y: 99 }),
    )
    .unwrap();
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    let payload: (i64,i64,i64,i64,i64) = sqlx::query_as("SELECT x,y,data_char,data_short,data_int FROM btech_map_objects WHERE map_dbref=? AND object_type=7 AND ordinal=4").bind(map.0).fetch_one(&mut connection).await.unwrap();
    assert_eq!(payload, (-10, 99, 14, 15, 16));
    set_battle_linked_marker(&mut loaded, map, 4, None).unwrap();
    assert!(loaded.btech.maps()[&map].wrapping());
    assert_eq!(loaded.btech.maps()[&map].linked_markers().len(), 1);
    let before = loaded.btech.clone();
    set_battle_map_wrapping(&mut loaded, map, true).unwrap();
    assert_eq!(loaded.btech, before);
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, before);

    set_battle_map_wrapping(&mut loaded, map, false).unwrap();
    persistence::save(&config.database(), &loaded)
        .await
        .unwrap();
    let count: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM btech_map_objects WHERE map_dbref=? AND object_type=7",
    )
    .bind(map.0)
    .fetch_one(&mut connection)
    .await
    .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn native_and_lua_wrapping_controls_share_authority_state_and_rollback() {
    use std::{cell::RefCell, rc::Rc};
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Configured wrapping".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "configured",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let initial = scripts.world().btech.clone();
    let command = format!("@btech map-wrapping #{}=on", map.0);
    let denied = support::run_text(&scripts, &config, ObjectId(2), 1, &command);
    assert!(!denied.contains("wrapping enabled"), "{denied}");
    assert_eq!(scripts.world().btech, initial);
    let output = support::run_text(&scripts, &config, ObjectId(1), 1, &command);
    assert!(output.contains("wrapping enabled"), "{output}");
    let native = scripts.world().btech.clone();
    scripts.world_mut().btech = initial.clone();
    let enabled = scripts
        .eval_callback::<bool>(&format!(
            "btech.map.wrapping({}, true); return btech.map.inspect({}).wrapping",
            map.0, map.0
        ))
        .unwrap();
    assert!(enabled);
    assert_eq!(scripts.world().btech, native);
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.map.wrapping({}, false); error('abort')",
                map.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, native);
    for invalid in ["'off'", "1", "nil"] {
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.map.wrapping({}, {invalid})", map.0))
                .is_err()
        );
        assert_eq!(scripts.world().btech, native);
    }
    let output = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech map-wrapping #{}=maybe", map.0),
    );
    assert!(output.contains("Usage:"), "{output}");
    assert_eq!(scripts.world().btech, native);
    let output = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        &format!("@btech map-wrapping #{}=off", map.0),
    );
    assert!(output.contains("wrapping disabled"), "{output}");
    assert_eq!(scripts.world().btech, initial);
    scripts.world().validate(&config).unwrap();
}

#[tokio::test]
async fn wrapped_jump_paths_keep_distance_and_replay_through_all_four_edges() {
    for (bearing, x, y) in [(0, 2, 0), (90, 4, 2), (180, 2, 4), (270, 0, 2)] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let map = world.create(&config, "Jump wrapping".into(), Kind::Room);
        create_battle_map(
            &mut world,
            map,
            "jump",
            BattleMapAsset::parse(
                "5 5\n.0.0.0.0.0\n.0.0.0.0.0\n.0.0.0.0.0\n.0.0.0.0.0\n.0.0.0.0.0\n",
            )
            .unwrap(),
        )
        .unwrap();
        let id = world.create(&config, "Jump traveler".into(), Kind::Thing);
        create_battle_unit(
            &mut world,
            id,
            BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, x, y).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        let before = world.btech.clone();
        assert!(launch_battle_jump(&mut world, id, ObjectId(1), bearing, 2.0).is_err());
        assert_eq!(world.btech, before);
        set_battle_map_wrapping(&mut world, map, true).unwrap();
        let _ = launch_battle_jump(&mut world, id, ObjectId(1), bearing, 2.0).unwrap();
        let launched = world.btech.clone();
        assert!(set_battle_map_wrapping(&mut world, map, false).is_err());
        assert_eq!(world.btech, launched);
        let path = world.btech.constructed_units()[&id]
            .flight()
            .unwrap()
            .path();
        let endpoint = path
            .sample(1.0, path.movement_points())
            .unwrap()
            .point
            .containing_hex()
            .unwrap();
        let destination = BattleHexCoordinate {
            x: endpoint.x.rem_euclid(5),
            y: endpoint.y.rem_euclid(5),
        };
        let mut crossed = false;
        let mut previous_distance = 0.0;
        for _ in 0..60 {
            let mut replay = world.clone();
            replay.btech =
                serde_json::from_value(serde_json::to_value(&world.btech).unwrap()).unwrap();
            let result = advance_battle_jumps(&mut world, BattleMovementRules::STANDARD).unwrap();
            assert_eq!(
                advance_battle_jumps(&mut replay, BattleMovementRules::STANDARD).unwrap(),
                result
            );
            assert_eq!(world.btech, replay.btech);
            world.validate(&config).unwrap();
            let unit = &world.btech.constructed_units()[&id];
            let Some(flight) = unit.flight() else {
                break;
            };
            assert_eq!(flight.path(), path);
            assert!(flight.travelled() > previous_distance);
            previous_distance = flight.travelled();
            let virtual_point = path
                .sample(flight.travelled() / path.distance(), path.movement_points())
                .unwrap()
                .point;
            if virtual_point != flight.sample().point {
                crossed = true;
                assert_eq!(unit.motion().unwrap().point, flight.sample().point);
                persistence::save(&config.database(), &world).await.unwrap();
                let loaded = persistence::load(&config.database()).await.unwrap();
                loaded.validate(&config).unwrap();
                assert_eq!(loaded.btech, world.btech);
                world = loaded;
            }
        }
        assert!(crossed, "No airborne seam crossing at bearing {bearing}");
        let unit = &world.btech.constructed_units()[&id];
        assert!(unit.flight().is_none());
        let position = unit.position().unwrap();
        assert_eq!(
            (i32::from(position.x), i32::from(position.y)),
            (destination.x, destination.y)
        );
        assert_eq!(unit.motion().unwrap().point, destination.center());
    }
}
