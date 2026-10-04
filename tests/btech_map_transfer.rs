//! Powered map transfers reuse administrative placement while preserving crew and combat material.
use crate::support;
use stompymux_rs::*;

#[tokio::test]
async fn transfer_preserves_running_controls_and_crew_across_all_admitted_chassis() {
    for source in [
        include_str!("fixtures/btech/mechs/JR7-D.toml"),
        include_str!("../game/mechs/Demolisher.toml"),
        include_str!("../game/mechs/Kestrel.toml"),
    ] {
        let (_dir, config, mut world) = support::isolated_world().await;
        let exterior = world.create(&config, "Outside".into(), Kind::Room);
        let interior = world.create(&config, "Inside".into(), Kind::Room);
        create_battle_map(
            &mut world,
            exterior,
            "outside",
            MapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, exterior, support::FIXTURE_DICE_SEED);
        create_battle_map(
            &mut world,
            interior,
            "inside",
            MapAsset::from_cells("2 2\n.3.3\n.3.3\n").unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, interior, support::FIXTURE_DICE_SEED);
        let occupant = world.create(&config, "Existing occupant".into(), Kind::Thing);
        create_battle_unit(
            &mut world,
            occupant,
            BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                .unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, occupant, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, occupant, interior, 0, 0).unwrap();
        let id = world.create(&config, "Traveler".into(), Kind::Thing);
        BattleUnitTemplate::parse("test", source)
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
        let _ = select_battle_hex_target(
            &mut world,
            id,
            ObjectId(1),
            HexCoordinate { x: 2, y: 2 },
            BattleHexTargetMode::Hex,
        )
        .unwrap();
        let key = if world.btech.vehicles().contains_key(&id) {
            "vehicles"
        } else {
            "constructed"
        };
        world
            .btech
            .rewrite_unit_record(id, |record| {
                let unit = record;
                unit["motion"]["heading"] = 90.into();
                unit["motion"]["desired_heading"] = 135.into();
                unit["motion"]["speed"] = 3.into();
                unit["motion"]["desired_speed"] = 4.into();
                if key == "constructed" {
                    unit["stagger"] = serde_json::json!({
                        "hits": [{"damage": 12, "remaining": 45, "counted": false}],
                        "elapsed": 15, "turn_damage": 0, "phase": 5, "checked_phase": null
                    });
                }
            })
            .unwrap();
        world.validate(&config).unwrap();
        let mut airborne = world.clone();
        if key == "constructed" {
            let _ = launch_battle_jump(&mut airborne, id, ObjectId(1), 90, 1.0).unwrap();
        } else if airborne.btech.vehicles()[&id].definition().is_vtol() {
            let _ = begin_battle_vtol_takeoff(&mut airborne, id, ObjectId(1), 0, false).unwrap();
        }
        if airborne.btech != world.btech {
            let pending = airborne.btech.clone();
            assert!(
                transfer_battle_unit(
                    &mut airborne,
                    id,
                    BattlePosition {
                        map: interior,
                        x: 1,
                        y: 1
                    }
                )
                .is_err()
            );
            assert_eq!(airborne.btech, pending);
        }
        // Forced descent cannot be bypassed through placement, entry, or exit.
        let mut falling = world.clone();
        let mut saved = serde_json::to_value(&falling.btech).unwrap();
        let unit = &mut saved[key][id.0.to_string()];
        if key == "constructed" {
            unit["free_fall"] = serde_json::to_value(BattleFreeFall::new(5)).unwrap();
            unit["motion"]["speed"] = 0.into();
            unit["motion"]["desired_speed"] = 0.into();
            unit["motion"]["desired_heading"] = unit["motion"]["heading"].clone();
        } else if falling.btech.vehicles()[&id].definition().is_vtol() {
            unit["vtol_flight"]["altitude"] = 5.0.into();
        } else {
            unit["ground_elevation"] = 5.0.into();
        }
        falling.btech = serde_json::from_value(saved).unwrap();
        if key == "vehicles" {
            begin_battle_vehicle_descent(&mut falling, id).unwrap();
        }
        falling.validate(&config).unwrap();
        persistence::save(&config.database(), &falling)
            .await
            .unwrap();
        let mut falling = persistence::load(&config.database()).await.unwrap();
        let before_fall = falling.btech.clone();
        for (index, error) in [
            transfer_battle_unit(
                &mut falling,
                id,
                BattlePosition {
                    map: interior,
                    x: 1,
                    y: 1,
                },
            )
            .unwrap_err(),
            battle_building_entry_destination_for_unit(&falling, id, ObjectId(1), None)
                .unwrap_err(),
            exit_battle_building(&mut falling, id).unwrap_err(),
        ]
        .into_iter()
        .enumerate()
        {
            let expected = if index == 1 {
                "While in mid-flight? No way."
            } else {
                "Finish airborne movement before transferring between maps"
            };
            assert!(error.to_string() == expected, "{source}: {error}");
            assert_eq!(falling.btech, before_fall);
            assert_eq!(falling.objects[&id].location, Some(exterior));
        }
        let before = serde_json::to_value(&world.btech).unwrap()[key][id.0.to_string()].clone();
        let destination = BattlePosition {
            map: interior,
            x: 1,
            y: 1,
        };
        let mut replay = world.clone();
        transfer_battle_unit(&mut world, id, destination).unwrap();
        transfer_battle_unit(&mut replay, id, destination).unwrap();
        assert_eq!(world.btech, replay.btech);
        assert_eq!(world.objects[&id].location, Some(interior));
        assert_eq!(world.objects[&ObjectId(1)].location, Some(id));
        let after = serde_json::to_value(&world.btech).unwrap()[key][id.0.to_string()].clone();
        for field in ["sections", "dice", "power", "pilot", "weapon_recycle"] {
            assert!(before.get(field).is_some(), "Missing {field}");
            assert_eq!(after[field], before[field], "Changed {field}");
        }
        if key == "constructed" {
            assert_eq!(after["stagger"], before["stagger"]);
            let mut administrative = world.clone();
            let mut saved = serde_json::to_value(&administrative.btech).unwrap();
            saved[key][id.0.to_string()]["power"] = serde_json::to_value(BattlePower::Off).unwrap();
            administrative.btech = serde_json::from_value(saved).unwrap();
            place_battle_unit(&mut administrative, id, exterior, 0, 0).unwrap();
            assert!(
                administrative.btech.constructed_units()[&id]
                    .stagger()
                    .hits
                    .is_empty()
            );
        }
        for field in ["heading", "desired_heading", "speed", "desired_speed"] {
            assert_eq!(after["motion"][field], before["motion"][field]);
        }
        assert_eq!(after["map_slot"], 1);
        assert!(after["target_lock"].is_null());
        assert_eq!(
            after["motion"]["point"],
            serde_json::to_value(HexCoordinate { x: 1, y: 1 }.center()).unwrap()
        );
        if let Some(unit) = world.btech.vehicles().get(&id)
            && let Some(flight) = unit.vtol_flight()
        {
            assert_eq!(flight.phase, BattleVtolFlightPhase::Landed);
            assert_eq!(flight.altitude, 3.0);
        }
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        let settled = world.clone();
        for destination in [
            BattlePosition {
                map: interior,
                x: 2,
                y: 0,
            },
            BattlePosition {
                map: ObjectId(99999),
                x: 0,
                y: 0,
            },
        ] {
            assert!(transfer_battle_unit(&mut world, id, destination).is_err());
            assert_eq!(world.btech, settled.btech);
            assert_eq!(world.objects[&id].location, settled.objects[&id].location);
        }
        assert!(place_battle_unit(&mut world, id, exterior, 0, 0).is_err());
        assert_eq!(world.btech, settled.btech);
    }
}
