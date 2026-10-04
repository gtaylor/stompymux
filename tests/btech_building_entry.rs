//! Shared building admission, delayed readiness and durable rechecks for each included chassis.
use crate::support;
use stompymux_rs::*;

#[tokio::test]
async fn entry_delay_replays_and_rechecks_live_routes_for_every_chassis() {
    for source in [
        include_str!("fixtures/btech/mechs/JR7-D.toml"),
        include_str!("../game/mechs/Demolisher.toml"),
        include_str!("../game/mechs/Kestrel.toml"),
    ] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let exterior = world.create(&config, "Exterior".into(), Kind::Room);
        let interior = world.create(&config, "Interior".into(), Kind::Room);
        for map in [exterior, interior] {
            create_battle_map(
                &mut world,
                map,
                "entry",
                MapAsset::from_cells("2 2\n.0.0\n.0.0\n").unwrap(),
            )
            .unwrap();
            support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
        }
        set_building_entrance(
            &mut world,
            exterior,
            0,
            Some(BuildingEntrance {
                coordinate: HexCoordinate { x: 0, y: 0 },
                interior,
                data_char: 0,
                data_short: 0,
                data_int: 0,
            }),
        )
        .unwrap();
        set_battle_building_entry_point(
            &mut world,
            interior,
            0,
            Some(BuildingEntryPoint {
                coordinate: HexCoordinate { x: 1, y: 1 },
                direction: b'n',
                object: ObjectId(-1),
                data_short: 0,
                data_int: 0,
            }),
        )
        .unwrap();
        let id = world.create(&config, "Traveler".into(), Kind::Thing);
        UnitTemplate::parse("test", source)
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, id, exterior, 0, 0).unwrap();
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        let key = if world.btech.vehicles().contains_key(&id) {
            "vehicles"
        } else {
            "constructed"
        };
        let maximum = world.btech.vehicles().get(&id).map_or_else(
            || {
                world.btech.constructed_units()[&id]
                    .effective_maximum_speed(world.btech.maps().get(&exterior))
                    .unwrap()
            },
            Vehicle::maximum_speed,
        );
        // Forward and reverse motion use the same strict threshold.
        for speed in [maximum / 3.0, -maximum / 3.0] {
            let mut speeding = world.clone();
            speeding
                .btech
                .rewrite_unit_record(id, |record| {
                    record["motion"]["speed"] = speed.into();
                })
                .unwrap();
            let before = speeding.btech.clone();
            assert!(
                begin_battle_building_entry(&mut speeding, id, ObjectId(1), None, true).is_err()
            );
            assert_eq!(speeding.btech, before);
        }
        let mut malformed = serde_json::to_value(&world.btech).unwrap();
        malformed[key][id.0.to_string()]["building_entry"] =
            serde_json::json!({"direction": null, "remaining": 19});
        if let Ok(state) = serde_json::from_value(malformed) {
            let mut invalid = world.clone();
            invalid.btech = state;
            assert!(invalid.validate(&config).is_err());
        }
        let before = world.btech.clone();
        assert!(
            begin_battle_building_entry(&mut world, id, ObjectId(1), Some(b'N'), false).is_err()
        );
        assert_eq!(world.btech, before);
        // Strict integer half-integrity boundary, shared by all chassis.
        for (integrity, flags, allowed) in [(50, 0, false), (49, 0, true), (49, 8, false)] {
            set_building_state(
                &mut world,
                interior,
                BuildingState {
                    integrity,
                    maximum_integrity: 101,
                    flags,
                    regeneration: 1,
                },
            )
            .unwrap();
            assert_eq!(
                battle_building_entry_lock_allows(&world, interior, false).unwrap(),
                allowed
            );
            assert!(battle_building_entry_lock_allows(&world, interior, true).unwrap());
        }
        let destination =
            begin_battle_building_entry(&mut world, id, ObjectId(1), Some(b'N'), true).unwrap();
        assert_eq!(destination.map, interior);
        let pending = world.btech.clone();
        assert!(begin_battle_building_entry(&mut world, id, ObjectId(1), None, true).is_err());
        assert_eq!(world.btech, pending);
        for _ in 0..9 {
            assert!(
                advance_battle_building_entry(&mut world, id)
                    .unwrap()
                    .is_none()
            );
        }
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        // Loading resumes the damaged interior's missing repair interval, independently of entry.
        let mut expected = serde_json::to_value(&world.btech).unwrap();
        expected["maps"][interior.0.to_string()]["building_repair"] = 120.into();
        assert_eq!(serde_json::to_value(&replay.btech).unwrap(), expected);
        world.btech = replay.btech.clone();
        for tick in 10..=18 {
            let ready = advance_battle_building_entry(&mut world, id).unwrap();
            assert_eq!(
                ready,
                advance_battle_building_entry(&mut replay, id).unwrap()
            );
            assert_eq!(ready.is_some(), tick == 18);
        }
        assert_eq!(world.objects[&id].location, Some(exterior));
        assert_eq!(
            battle_building_entry(&world, id).unwrap(),
            BuildingEntry {
                direction: Some(b'n'),
                remaining: 0
            }
        );
        // A ready event remains durable until the host's movement transaction consumes it.
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        assert!(
            advance_battle_building_entry(&mut world, id)
                .unwrap()
                .is_some()
        );
        set_battle_building_entry_point(
            &mut world,
            interior,
            0,
            Some(BuildingEntryPoint {
                coordinate: HexCoordinate { x: 0, y: 1 },
                direction: b'n',
                object: ObjectId(-1),
                data_short: 0,
                data_int: 0,
            }),
        )
        .unwrap();
        let refreshed =
            battle_building_entry_destination_for_unit(&world, id, ObjectId(1), Some(b'n'))
                .unwrap();
        assert_ne!(refreshed, destination);
        set_building_entrance(&mut world, exterior, 0, None).unwrap();
        assert!(
            battle_building_entry_destination_for_unit(&world, id, ObjectId(1), Some(b'n'))
                .is_err()
        );
        clear_battle_building_entry(&mut world, id).unwrap();
        assert!(battle_building_entry(&world, id).is_none());
        world.validate(&config).unwrap();
    }
}
