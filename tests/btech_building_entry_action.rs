//! Host entry countdowns share movement callbacks and rollback across all admitted chassis.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Create a piloted running unit at an authored entrance.
async fn fixture(
    source: &str,
) -> (
    tempfile::TempDir,
    Config,
    World,
    ObjectId,
    ObjectId,
    ObjectId,
) {
    let (_dir, config, mut world) = support::isolated_world().await;
    let exterior = world.create(&config, "Exterior".into(), Kind::Room);
    let interior = world.create(&config, "Interior".into(), Kind::Room);
    for map in [exterior, interior] {
        create_battle_map(
            &mut world,
            map,
            "entry",
            BattleMapAsset::parse("2 2\n.0.0\n.0.0\n").unwrap(),
        )
        .unwrap();
    }
    set_building_entrance(
        &mut world,
        exterior,
        0,
        Some(BattleBuildingEntrance {
            coordinate: BattleHexCoordinate { x: 0, y: 0 },
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
        Some(BattleBuildingEntryPoint {
            coordinate: BattleHexCoordinate { x: 1, y: 1 },
            direction: b'n',
            object: ObjectId(-1),
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    let id = world.create(&config, "Traveler".into(), Kind::Thing);
    BattleUnitTemplate::parse(source)
        .unwrap()
        .create(&mut world, id)
        .unwrap();
    place_battle_unit(&mut world, id, exterior, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (_dir, config, world, id, exterior, interior)
}

/// Expose loaded parent modules only to this callback fault-injection fixture.
fn host(config: &Config, world: World) -> Scripts {
    let scripts = Scripts::new(config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .inspect_lua()
        .globals()
        .set(
            "_parents",
            scripts
                .inspect_lua()
                .named_registry_value::<mlua::Table>("mux.parents")
                .unwrap(),
        )
        .unwrap();
    scripts
}

#[tokio::test]
async fn delayed_entry_moves_all_chassis_and_rolls_back_arrival_failure() {
    for source in [
        include_str!("fixtures/btech/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
        include_str!("../game/mechs/Kestrel"),
    ] {
        let (_dir, config, world, id, exterior, interior) = fixture(source).await;
        let scripts = host(&config, world);
        assert!(begin_battle_building_entry_action(&scripts, id, ObjectId(1), Some(b'n')).unwrap());
        assert!(battle_building_entries_pending(&scripts.world()));
        scripts.drain_outbox();
        for _ in 0..17 {
            advance_battle_building_entries_action(&scripts).unwrap();
        }
        assert_eq!(scripts.world().objects[&id].location, Some(exterior));
        let before = scripts.world().clone();
        scripts.inspect_lua().load("_parents['default_thing.lua'].events={on_move=function(ctx) mux.world.object(ctx.object):set_description('leak'); error('arrival failed') end}").exec().unwrap();
        assert!(advance_battle_building_entries_action(&scripts).is_err());
        assert_eq!(scripts.world().btech, before.btech);
        assert_eq!(
            scripts.world().objects[&id].location,
            before.objects[&id].location
        );
        assert_eq!(
            scripts.world().objects[&id].description,
            before.objects[&id].description
        );
        assert!(scripts.drain_outbox().is_empty());
        scripts
            .inspect_lua()
            .load("_parents['default_thing.lua'].events={}")
            .exec()
            .unwrap();
        advance_battle_building_entries_action(&scripts).unwrap();
        assert_eq!(scripts.world().objects[&id].location, Some(interior));
        assert_eq!(scripts.world().objects[&ObjectId(1)].location, Some(id));
        assert!(!battle_building_entries_pending(&scripts.world()));
        scripts.world().validate(&config).unwrap();
        let completed = scripts.world().clone();
        persistence::save(&config.database(), &completed)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            completed.btech
        );
    }
}

#[tokio::test]
async fn changed_enter_lock_consumes_event_without_moving() {
    let (_dir, config, world, id, exterior, _interior) =
        fixture(include_str!("../game/mechs/Demolisher")).await;
    let scripts = host(&config, world);
    assert!(begin_battle_building_entry_action(&scripts, id, ObjectId(1), None).unwrap());
    scripts
        .inspect_lua()
        .load("_parents['default_room.lua'].locks={enter=function() return false end}")
        .exec()
        .unwrap();
    for _ in 0..18 {
        advance_battle_building_entries_action(&scripts).unwrap();
    }
    assert_eq!(scripts.world().objects[&id].location, Some(exterior));
    assert!(!battle_building_entries_pending(&scripts.world()));
    assert!(!begin_battle_building_entry_action(&scripts, id, ObjectId(1), None).unwrap());
    assert!(!battle_building_entries_pending(&scripts.world()));
}

#[tokio::test]
async fn native_and_lua_entry_share_admission_parsing_and_rollback() {
    for source in [
        include_str!("fixtures/btech/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
        include_str!("../game/mechs/Kestrel"),
    ] {
        let (_dir, config, world, id, _exterior, _interior) = fixture(source).await;
        let scripts = host(&config, world.clone());
        let output = support::run_text(&scripts, &config, ObjectId(1), 1, "enterbase N");
        assert!(
            output.contains("doors at your hex start to open"),
            "{output}"
        );
        let native = scripts.world().btech.clone();
        *scripts.world_mut() = world.clone();
        assert!(
            scripts
                .eval_callback::<bool>(&format!("return btech.unit.enterbase({},1,'N')", id.0))
                .unwrap()
        );
        assert_eq!(scripts.world().btech, native);
        scripts.drain_outbox();
        *scripts.world_mut() = world.clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.enterbase({},1); error('abort entry')",
                    id.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, world.btech);
        assert!(scripts.drain_outbox().is_empty());
        for input in ["enterbase n e", "enterbase/bad"] {
            let output = support::run_text(&scripts, &config, ObjectId(1), 1, input);
            assert!(
                output == "Invalid arguments to command!" || output.contains("no switches"),
                "{output}"
            );
            assert_eq!(scripts.world().btech, world.btech);
        }
        assert!(
            scripts
                .eval_callback::<bool>(&format!("return btech.unit.enterbase({},2)", id.0))
                .is_err()
        );
        assert_eq!(scripts.world().btech, world.btech);
        let output = support::run_text(&scripts, &config, ObjectId(1), 1, "enterbase north");
        assert!(
            output.contains("doors at your hex start to open"),
            "{output}"
        );
        assert_eq!(
            battle_building_entry(&scripts.world(), id)
                .unwrap()
                .direction,
            None
        );
    }
}

#[tokio::test]
async fn entry_feedback_uses_hex_visibility_and_captured_contacts() {
    for source in [
        include_str!("fixtures/btech/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
        include_str!("../game/mechs/Kestrel"),
    ] {
        let (_dir, config, mut world, id, exterior, interior) = fixture(source).await;
        world.objects.get_mut(&interior).unwrap().name = "North Depot".into();
        let observer = world.create(&config, "Observer".into(), Kind::Thing);
        create_battle_vehicle(
            &mut world,
            observer,
            BattleVehicleTemplate::parse(include_str!("../game/mechs/Demolisher")).unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, observer, exterior, 1, 0).unwrap();
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(observer);
        assign_battle_pilot(&mut world, observer, ObjectId(2)).unwrap();
        start_battle_unit(&mut world, observer, ObjectId(2), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        for acquired in [false, true] {
            let mut observed = world.clone();
            if acquired {
                refresh_battle_contacts(&mut observed, &[observer]).unwrap();
                assert!(
                    visible_battle_contact(&observed, observer, id)
                        .unwrap()
                        .is_some()
                );
            }
            let scripts = host(&config, observed);
            assert!(begin_battle_building_entry_action(&scripts, id, ObjectId(1), None).unwrap());
            let messages = scripts.drain_outbox();
            assert!(
                messages.iter().any(|(who, text)| *who == ObjectId(1)
                    && text.source().contains("doors at your hex"))
            );
            assert!(
                messages.iter().any(
                    |(who, text)| *who == ObjectId(2) && text.source().contains("doors at 0,0")
                )
            );
            for _ in 0..18 {
                advance_battle_building_entries_action(&scripts).unwrap();
            }
            let messages = scripts.drain_outbox();
            assert!(messages.iter().any(|(who, text)| *who == ObjectId(1)
                && text.source().contains("You enter the North Depot.")));
            assert_eq!(
                messages.iter().any(|(who, text)| *who == ObjectId(2)
                    && text
                        .source()
                        .contains("has entered the North Depot at 0,0.")),
                acquired
            );
            assert!(
                !scripts.world().btech.vehicles()[&observer]
                    .contacts()
                    .contains_key(&id)
            );
        }
    }
}

#[tokio::test]
async fn interior_arrival_uses_normal_acquisition_without_extra_dice() {
    for source in [
        include_str!("fixtures/btech/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
        include_str!("../game/mechs/Kestrel"),
    ] {
        let (_dir, config, mut world, id, exterior, interior) = fixture(source).await;
        let observer = world.create(&config, "Interior observer".into(), Kind::Thing);
        create_battle_vehicle(
            &mut world,
            observer,
            BattleVehicleTemplate::parse(include_str!("../game/mechs/Demolisher")).unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, observer, interior, 0, 0).unwrap();
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(observer);
        assign_battle_pilot(&mut world, observer, ObjectId(2)).unwrap();
        start_battle_unit(&mut world, observer, ObjectId(2), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        assert!(!battle_contact_observers(&world).contains(&observer));
        let scripts = host(&config, world);
        begin_battle_building_entry_action(&scripts, id, ObjectId(1), None).unwrap();
        scripts.drain_outbox();
        for _ in 0..17 {
            assert!(
                advance_battle_building_entries_action(&scripts)
                    .unwrap()
                    .is_empty()
            );
        }
        let arrivals = advance_battle_building_entries_action(&scripts).unwrap();
        assert_eq!(arrivals.len(), 1);
        scripts.drain_outbox();
        let placed = scripts.world().btech.clone();
        publish_battle_building_arrivals(&scripts, arrivals.clone()).unwrap();
        assert!(scripts.drain_outbox().is_empty());
        assert_eq!(scripts.world().btech, placed);
        let observers = battle_contact_observers(&scripts.world());
        assert!(observers.contains(&observer));
        let events = refresh_battle_contacts(&mut scripts.world_mut(), &observers).unwrap();
        assert!(
            events
                .iter()
                .any(|event| event.observer == observer && event.target == id && event.acquired)
        );
        let acquired = scripts.world().btech.clone();
        publish_battle_building_arrivals(&scripts, arrivals.clone()).unwrap();
        assert_eq!(scripts.world().btech, acquired);
        let messages = scripts.drain_outbox();
        assert!(messages.iter().any(|(who, text)| *who == ObjectId(2)
            && text.source().contains("has entered the Interior at 0,0.")));
        transfer_battle_unit(
            &mut scripts.world_mut(),
            id,
            BattlePosition {
                map: exterior,
                x: 0,
                y: 0,
            },
        )
        .unwrap();
        publish_battle_building_arrivals(&scripts, arrivals).unwrap();
        assert!(scripts.drain_outbox().is_empty());
        scripts.world().validate(&config).unwrap();
    }
}

#[tokio::test]
async fn building_exit_shares_placement_and_continues_vtol_flight() {
    for source in [
        include_str!("fixtures/btech/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
        include_str!("../game/mechs/Kestrel"),
    ] {
        let (_dir, config, mut world, id, exterior, interior) = fixture(source).await;
        set_battle_building_exit(&mut world, interior, 0, Some(exterior)).unwrap();
        transfer_battle_unit(
            &mut world,
            id,
            BattlePosition {
                map: interior,
                x: 1,
                y: 1,
            },
        )
        .unwrap();
        let blocked = world.clone();
        assert!(
            exit_battle_building(&mut world, id)
                .unwrap_err()
                .to_string()
                .contains("rubble")
        );
        assert_eq!(world.btech, blocked.btech);
        set_building_state(
            &mut world,
            interior,
            BattleBuildingState {
                integrity: 1,
                maximum_integrity: 100,
                flags: 0,
                regeneration: 1,
            },
        )
        .unwrap();
        let occupant = world.create(&config, "Exterior occupant".into(), Kind::Thing);
        create_battle_unit(
            &mut world,
            occupant,
            BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, occupant, exterior, 1, 0).unwrap();
        let vehicle = world.btech.vehicles().get(&id);
        let vtol = vehicle.is_some_and(|unit| unit.definition().is_vtol());
        let key = if vehicle.is_some() {
            "vehicles"
        } else {
            "constructed"
        };
        let maximum = vehicle.map_or_else(
            || world.btech.constructed_units()[&id].definition().max_speed,
            BattleVehicle::maximum_speed,
        );
        for airborne in [false, true]
            .into_iter()
            .filter(|airborne| !airborne || vtol)
        {
            let mut candidate = world.clone();
            let mut saved = serde_json::to_value(&candidate.btech).unwrap();
            saved["maps"][exterior.0.to_string()]["gravity"] = 200.into();
            let flags = saved["maps"][exterior.0.to_string()]["flags"]
                .as_u64()
                .unwrap();
            saved["maps"][exterior.0.to_string()]["flags"] = (flags | 2).into();
            let unit = &mut saved[key][id.0.to_string()];
            unit["motion"]["heading"] = 90.into();
            unit["motion"]["desired_heading"] = 135.into();
            unit["motion"]["speed"] = (maximum * 0.75).into();
            unit["motion"]["desired_speed"] = maximum.into();
            if airborne {
                unit["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
                unit["vtol_flight"]["altitude"] = 4.into();
                unit["vtol_flight"]["vertical_speed"] = 6.into();
            }
            candidate.btech = serde_json::from_value(saved).unwrap();
            candidate.validate(&config).unwrap();
            let before =
                serde_json::to_value(&candidate.btech).unwrap()[key][id.0.to_string()].clone();
            let destination = exit_battle_building(&mut candidate, id).unwrap();
            assert_eq!(
                destination,
                BattlePosition {
                    map: exterior,
                    x: 0,
                    y: 0
                }
            );
            let after =
                serde_json::to_value(&candidate.btech).unwrap()[key][id.0.to_string()].clone();
            for field in ["power", "pilot", "dice", "sections", "weapon_recycle"] {
                assert_eq!(before[field], after[field], "{field}");
            }
            for field in ["heading", "desired_heading", "desired_speed"] {
                assert_eq!(before["motion"][field], after["motion"][field]);
            }
            assert_eq!(after["map_slot"], 1);
            assert_eq!(candidate.objects[&ObjectId(1)].location, Some(id));
            let expected = if key == "constructed" {
                candidate.btech.constructed_units()[&id]
                    .effective_maximum_speed(candidate.btech.maps().get(&exterior))
                    .unwrap()
            } else {
                // Destination special gravity halves the shared effective ceiling for vehicles too.
                maximum * 0.5
            };
            assert_eq!(
                after["motion"]["speed"].as_f64().unwrap(),
                (maximum * 0.75).min(expected)
            );
            if vtol {
                let flight = candidate.btech.vehicles()[&id].vtol_flight().unwrap();
                assert_eq!(flight.phase, BattleVtolFlightPhase::Airborne);
                assert_eq!(flight.altitude, 1.0);
                assert_eq!(flight.vertical_speed, if airborne { 6.0 } else { 0.0 });
            }
            candidate.validate(&config).unwrap();
            persistence::save(&config.database(), &candidate)
                .await
                .unwrap();
            let mut expected = serde_json::to_value(&candidate.btech).unwrap();
            expected["maps"][interior.0.to_string()]["building_repair"] = 120.into();
            assert_eq!(
                serde_json::to_value(persistence::load(&config.database()).await.unwrap().btech)
                    .unwrap(),
                expected
            );
        }
    }
}

#[tokio::test]
async fn host_exits_share_teleport_policy_and_callback_rollback() {
    for source in [
        include_str!("fixtures/btech/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
        include_str!("../game/mechs/Kestrel"),
    ] {
        let (_dir, config, mut world, id, exterior, interior) = fixture(source).await;
        set_battle_building_exit(&mut world, interior, 0, Some(exterior)).unwrap();
        set_building_state(
            &mut world,
            interior,
            BattleBuildingState {
                integrity: 100,
                maximum_integrity: 100,
                flags: 0,
                regeneration: 1,
            },
        )
        .unwrap();
        transfer_battle_unit(
            &mut world,
            id,
            BattlePosition {
                map: interior,
                x: 1,
                y: 1,
            },
        )
        .unwrap();
        let observer = world.create(&config, "Exit observer".into(), Kind::Thing);
        create_battle_vehicle(
            &mut world,
            observer,
            BattleVehicleTemplate::parse(include_str!("../game/mechs/Demolisher")).unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, observer, exterior, 1, 0).unwrap();
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(observer);
        assign_battle_pilot(&mut world, observer, ObjectId(2)).unwrap();
        start_battle_unit(&mut world, observer, ObjectId(2), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        let scripts = host(&config, world.clone());
        scripts
            .inspect_lua()
            .load("_parents['default_room.lua'].locks={teleport=function() return false end}")
            .exec()
            .unwrap();
        assert!(exit_battle_building_action(&scripts, id).unwrap().is_none());
        assert_eq!(scripts.world().btech, world.btech);
        assert_eq!(scripts.world().objects[&id].location, Some(interior));
        assert!(
            scripts
                .drain_outbox()
                .iter()
                .any(|(who, text)| *who == ObjectId(1)
                    && text.source().contains("teleportation was denied"))
        );
        scripts.inspect_lua().load("_parents['default_room.lua'].locks={}; _parents['default_thing.lua'].events={on_move=function(ctx) mux.world.object(ctx.object):set_description('exit leak'); error('exit callback') end}").exec().unwrap();
        assert!(exit_battle_building_action(&scripts, id).is_err());
        assert_eq!(scripts.world().btech, world.btech);
        assert_eq!(scripts.world().objects[&id].location, Some(interior));
        assert_eq!(
            scripts.world().objects[&id].description,
            world.objects[&id].description
        );
        assert!(scripts.drain_outbox().is_empty());
        scripts
            .inspect_lua()
            .load("_parents['default_thing.lua'].events={}")
            .exec()
            .unwrap();
        let arrival = exit_battle_building_action(&scripts, id).unwrap().unwrap();
        assert_eq!(scripts.world().objects[&id].location, Some(exterior));
        assert_eq!(scripts.world().objects[&ObjectId(1)].location, Some(id));
        assert!(
            scripts
                .drain_outbox()
                .iter()
                .any(|(who, text)| *who == ObjectId(1)
                    && text.source().contains("You have left the Interior."))
        );
        let observers = battle_contact_observers(&scripts.world());
        refresh_battle_contacts(&mut scripts.world_mut(), &observers).unwrap();
        publish_battle_building_arrivals(&scripts, vec![arrival]).unwrap();
        assert!(
            scripts
                .drain_outbox()
                .iter()
                .any(|(who, text)| *who == ObjectId(2)
                    && text.source().contains("has left the Interior at 0,0."))
        );
        scripts.world().validate(&config).unwrap();
        let completed = scripts.world().clone();
        persistence::save(&config.database(), &completed)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            completed.btech
        );
    }
}

#[tokio::test]
async fn movement_edges_dispatch_shared_exits_and_keep_blocked_units_stopped() {
    for source in [
        include_str!("fixtures/btech/mechs/JR7-D").to_owned(),
        include_str!("../game/mechs/GOL-1H").to_owned(),
        include_str!("../game/mechs/Demolisher").to_owned(),
        include_str!("../game/mechs/Demolisher").replace("{ Track }", "{ Wheel }"),
        include_str!("../game/mechs/Demolisher").replace("{ Track }", "{ Hover }"),
        include_str!("../game/mechs/Kestrel").to_owned(),
    ] {
        for heading in [0, 90, 180, 270] {
            let (_dir, config, mut world, id, exterior, interior) = fixture(&source).await;
            set_battle_building_exit(&mut world, interior, 0, Some(exterior)).unwrap();
            set_building_state(
                &mut world,
                interior,
                BattleBuildingState {
                    integrity: 100,
                    maximum_integrity: 100,
                    flags: 0,
                    regeneration: 1,
                },
            )
            .unwrap();
            transfer_battle_unit(
                &mut world,
                id,
                BattlePosition {
                    map: interior,
                    x: if heading == 270 { 0 } else { 1 },
                    y: if heading == 0 { 0 } else { 1 },
                },
            )
            .unwrap();
            let key = if world.btech.vehicles().contains_key(&id) {
                "vehicles"
            } else {
                "constructed"
            };
            let vtol = world
                .btech
                .vehicles()
                .get(&id)
                .is_some_and(|unit| unit.definition().is_vtol());
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            saved["maps"][interior.0.to_string()]["movement_modifier"] = 6450.into();
            let unit = &mut saved[key][id.0.to_string()];
            unit["motion"]["heading"] = heading.into();
            unit["motion"]["desired_heading"] = heading.into();
            unit["motion"]["speed"] = 43.into();
            unit["motion"]["desired_speed"] = 43.into();
            if vtol {
                unit["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
                    phase: BattleVtolFlightPhase::Airborne,
                    altitude: 5.0,
                    vertical_speed: 0.0,
                    fall: None,
                })
                .unwrap();
            }
            world.btech = serde_json::from_value(saved).unwrap();
            world.validate(&config).unwrap();
            let scripts = host(&config, world.clone());
            scripts.inspect_lua().load("_parents['default_thing.lua'].events={on_move=function() error('boundary callback') end}").exec().unwrap();
            assert!(
                advance_battle_motion_action(&scripts, &config, BattleMovementRules::STANDARD)
                    .is_err()
            );
            assert_eq!(scripts.world().btech, world.btech);
            assert!(scripts.drain_outbox().is_empty());
            scripts
                .inspect_lua()
                .load("_parents['default_thing.lua'].events={}")
                .exec()
                .unwrap();
            let arrivals =
                advance_battle_motion_action(&scripts, &config, BattleMovementRules::STANDARD)
                    .unwrap();
            assert_eq!(arrivals.len(), 1, "heading {heading}");
            assert_eq!(scripts.world().objects[&id].location, Some(exterior));
            let result = serde_json::to_value(&scripts.world().btech).unwrap();
            assert!(
                result[key][id.0.to_string()]["motion"]["speed"]
                    .as_f64()
                    .unwrap()
                    > 0.0
            );
            assert_eq!(
                result[key][id.0.to_string()]["motion"]["desired_speed"]
                    .as_f64()
                    .unwrap(),
                43.0
            );
            let messages = scripts.drain_outbox();
            assert_eq!(
                messages
                    .iter()
                    .filter(|(who, text)| *who == ObjectId(1)
                        && text.source().contains("THE INTERIOR has CF of 100."))
                    .count(),
                usize::from(!vtol),
                "surface exit reports CF once; aircraft above the entrance stay quiet"
            );
            assert!(
                !messages
                    .iter()
                    .any(|(_, text)| text.source().contains("movement stopped")
                        || text.source().contains("cannot move off"))
            );
            scripts.world().validate(&config).unwrap();
            let completed = scripts.world().clone();
            persistence::save(&config.database(), &completed)
                .await
                .unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                completed.btech
            );
            *scripts.world_mut() = world.clone();
            scripts
                .inspect_lua()
                .load("_parents['default_room.lua'].locks={teleport=function() return false end}")
                .exec()
                .unwrap();
            assert!(
                advance_battle_motion_action(&scripts, &config, BattleMovementRules::STANDARD)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(scripts.world().objects[&id].location, Some(interior));
            let denied = serde_json::to_value(&scripts.world().btech).unwrap();
            assert_eq!(
                denied[key][id.0.to_string()]["motion"]["speed"]
                    .as_f64()
                    .unwrap(),
                0.0
            );
            assert!(
                scripts
                    .drain_outbox()
                    .iter()
                    .any(|(_, text)| text.source().contains("teleportation was denied"))
            );
            scripts
                .inspect_lua()
                .load("_parents['default_room.lua'].locks={}")
                .exec()
                .unwrap();
            *scripts.world_mut() = world.clone();
            set_battle_map_wrapping(&mut scripts.world_mut(), interior, true).unwrap();
            assert!(
                advance_battle_motion_action(&scripts, &config, BattleMovementRules::STANDARD)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(scripts.world().objects[&id].location, Some(interior));
            scripts.drain_outbox();
            *scripts.world_mut() = world;
            set_building_state(
                &mut scripts.world_mut(),
                interior,
                BattleBuildingState {
                    integrity: 0,
                    maximum_integrity: 100,
                    flags: 0,
                    regeneration: 1,
                },
            )
            .unwrap();
            assert!(
                advance_battle_motion_action(&scripts, &config, BattleMovementRules::STANDARD)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(scripts.world().objects[&id].location, Some(interior));
            let result = serde_json::to_value(&scripts.world().btech).unwrap();
            assert_eq!(
                result[key][id.0.to_string()]["motion"]["speed"]
                    .as_f64()
                    .unwrap(),
                0.0
            );
            assert!(
                scripts
                    .drain_outbox()
                    .iter()
                    .any(|(_, text)| text.source().contains("rubble"))
            );
        }
    }
}

#[tokio::test]
async fn in_character_unpiloted_exits_warn_without_inventing_destruction() {
    for source in [
        include_str!("fixtures/btech/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
        include_str!("../game/mechs/Kestrel"),
    ] {
        for in_character in [false, true] {
            for assigned in [false, true] {
                let (_dir, config, mut world, id, exterior, interior) = fixture(source).await;
                set_battle_building_exit(&mut world, interior, 0, Some(exterior)).unwrap();
                set_building_state(
                    &mut world,
                    interior,
                    BattleBuildingState {
                        integrity: 100,
                        maximum_integrity: 100,
                        flags: 0,
                        regeneration: 1,
                    },
                )
                .unwrap();
                transfer_battle_unit(
                    &mut world,
                    id,
                    BattlePosition {
                        map: interior,
                        x: 1,
                        y: 1,
                    },
                )
                .unwrap();
                if in_character {
                    world
                        .objects
                        .get_mut(&id)
                        .unwrap()
                        .flags
                        .insert(Flag::InCharacter);
                }
                if !assigned {
                    release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
                }
                world.validate(&config).unwrap();
                let key = if world.btech.vehicles().contains_key(&id) {
                    "vehicles"
                } else {
                    "constructed"
                };
                let before =
                    serde_json::to_value(&world.btech).unwrap()[key][id.0.to_string()].clone();
                let scripts = host(&config, world.clone());
                scripts.inspect_lua().load("_parents['default_thing.lua'].events={on_move=function() error('exit failure') end}").exec().unwrap();
                assert!(exit_battle_building_action(&scripts, id).is_err());
                assert!(scripts.drain_outbox().is_empty());
                assert_eq!(scripts.world().btech, world.btech);
                scripts
                    .inspect_lua()
                    .load("_parents['default_thing.lua'].events={}")
                    .exec()
                    .unwrap();
                assert!(exit_battle_building_action(&scripts, id).unwrap().is_some());
                let messages = scripts.drain_outbox();
                let expected = in_character && !assigned;
                for phrase in [
                    "INTRUDER ALERT!",
                    "Automatic self-destruct sequence initiated",
                ] {
                    assert_eq!(
                        messages.iter().any(
                            |(who, text)| *who == ObjectId(1) && text.source().contains(phrase)
                        ),
                        expected
                    );
                }
                let after = serde_json::to_value(&scripts.world().btech).unwrap()[key]
                    [id.0.to_string()]
                .clone();
                for field in ["power", "pilot", "sections", "dice"] {
                    assert_eq!(before[field], after[field], "{field}");
                }
                assert_eq!(scripts.world().objects[&id].location, Some(exterior));
                scripts.world().validate(&config).unwrap();
            }
        }
    }
}

/// Add a shutdown passenger unit to a carrier's battlefield without pickup gameplay policy.
fn attach(world: &mut World, config: &Config, carrier: ObjectId, source: &str) -> ObjectId {
    let target = world.create(config, "Tow target".into(), Kind::Thing);
    BattleUnitTemplate::parse(source)
        .unwrap()
        .create(world, target)
        .unwrap();
    let position = world
        .btech
        .vehicles()
        .get(&carrier)
        .and_then(|unit| unit.position())
        .or_else(|| {
            world
                .btech
                .constructed_units()
                .get(&carrier)
                .and_then(|unit| unit.position())
        })
        .unwrap();
    place_battle_unit(
        world,
        target,
        position.map,
        i64::from(position.x),
        i64::from(position.y),
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
    set_battle_tow(world, carrier, Some(target)).unwrap();
    target
}

#[tokio::test]
async fn towing_pairs_enter_and_exit_together_across_all_chassis_and_restart() {
    let chassis = [
        include_str!("fixtures/btech/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
        include_str!("../game/mechs/Kestrel"),
    ];
    for (index, source) in chassis.iter().enumerate() {
        for target_source in chassis {
            let (_dir, config, mut world, carrier, exterior, interior) = fixture(source).await;
            set_battle_building_exit(&mut world, interior, 0, Some(exterior)).unwrap();
            set_building_state(
                &mut world,
                interior,
                BattleBuildingState {
                    integrity: 100,
                    maximum_integrity: 100,
                    flags: 0,
                    regeneration: 1,
                },
            )
            .unwrap();
            let target = attach(&mut world, &config, carrier, target_source);
            let scripts = host(&config, world);
            scripts.inspect_lua().load("_parents['default_thing.lua'].events={on_move=function(ctx) mux.world.object(ctx.object):set_description('moved') end}").exec().unwrap();
            if index == 0 {
                support::run_text(&scripts, &config, ObjectId(1), 1, "enterbase N");
            } else {
                assert!(
                    scripts
                        .eval_callback::<bool>(&format!(
                            "return btech.unit.enterbase({},1,'N')",
                            carrier.0
                        ))
                        .unwrap()
                );
            }
            for _ in 0..18 {
                advance_battle_building_entries_action(&scripts).unwrap();
            }
            for unit in [carrier, target] {
                assert_eq!(scripts.world().objects[&unit].location, Some(interior));
                assert_eq!(
                    scripts.world().objects[&unit].description.as_deref(),
                    Some("moved")
                );
            }
            assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(target));
            assert_eq!(scripts.world().btech.tows().get(&carrier), Some(&target));
            scripts.world().validate(&config).unwrap();
            let snapshot = scripts.world().clone();
            persistence::save(&config.database(), &snapshot)
                .await
                .unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            let restart = host(&config, restored);
            let arrival = exit_battle_building_action(&scripts, carrier)
                .unwrap()
                .unwrap();
            exit_battle_building_action(&restart, carrier)
                .unwrap()
                .unwrap();
            assert_eq!(scripts.world().btech, restart.world().btech);
            for unit in [carrier, target] {
                assert_eq!(scripts.world().objects[&unit].location, Some(exterior));
            }
            assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(target));
            let target_motion = scripts
                .world()
                .btech
                .vehicles()
                .get(&target)
                .and_then(|unit| unit.motion())
                .or_else(|| {
                    scripts
                        .world()
                        .btech
                        .constructed_units()
                        .get(&target)
                        .and_then(|unit| unit.motion())
                })
                .unwrap();
            assert_eq!(target_motion.desired_speed, 0.0);
            scripts.world().validate(&config).unwrap();
            publish_battle_building_arrivals(&scripts, vec![arrival]).unwrap();
        }
    }
}

#[tokio::test]
async fn tow_transfer_denial_and_either_arrival_callback_restore_the_entire_pair() {
    let (_dir, config, mut world, carrier, exterior, interior) =
        fixture(include_str!("fixtures/btech/mechs/JR7-D")).await;
    set_battle_building_exit(&mut world, interior, 0, Some(exterior)).unwrap();
    set_building_state(
        &mut world,
        interior,
        BattleBuildingState {
            integrity: 100,
            maximum_integrity: 100,
            flags: 0,
            regeneration: 1,
        },
    )
    .unwrap();
    let target = attach(
        &mut world,
        &config,
        carrier,
        include_str!("../game/mechs/Kestrel"),
    );
    transfer_battle_unit(
        &mut world,
        carrier,
        BattlePosition {
            map: interior,
            x: 1,
            y: 1,
        },
    )
    .unwrap();
    let mut invalid = world.clone();
    invalid
        .objects
        .get_mut(&target)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert!(
        transfer_battle_unit(
            &mut invalid,
            carrier,
            BattlePosition {
                map: exterior,
                x: 0,
                y: 0
            }
        )
        .is_err()
    );
    assert_eq!(invalid.btech, world.btech);
    assert_eq!(invalid.objects[&carrier].location, Some(interior));
    let mut independent = world.clone();
    assert!(
        transfer_battle_unit(
            &mut independent,
            target,
            BattlePosition {
                map: exterior,
                x: 0,
                y: 0
            }
        )
        .is_err()
    );
    assert_eq!(independent.btech, world.btech);
    let scripts = host(&config, world.clone());
    for denied in [carrier, target] {
        scripts.inspect_lua().load(format!("_parents['default_room.lua'].locks={{teleport=function(ctx) return ctx.subject ~= {} end}}",denied.0)).exec().unwrap();
        assert!(
            exit_battle_building_action(&scripts, carrier)
                .unwrap()
                .is_none()
        );
        assert_eq!(scripts.world().btech, world.btech);
        for unit in [carrier, target] {
            assert_eq!(scripts.world().objects[&unit].location, Some(interior));
        }
        scripts.drain_outbox();
    }
    scripts
        .inspect_lua()
        .load("_parents['default_room.lua'].locks={}")
        .exec()
        .unwrap();
    for failing in [carrier, target] {
        scripts.inspect_lua().load(format!("_parents['default_thing.lua'].events={{on_move=function(ctx) mux.world.object(ctx.object):set_description('leak'); if ctx.object == {} then error('pair callback failed') end end}}", failing.0)).exec().unwrap();
        assert!(exit_battle_building_action(&scripts, carrier).is_err());
        assert_eq!(scripts.world().btech, world.btech);
        for unit in [carrier, target] {
            assert_eq!(scripts.world().objects[&unit].location, Some(interior));
            assert_eq!(
                scripts.world().objects[&unit].description,
                world.objects[&unit].description
            );
        }
        assert!(scripts.drain_outbox().is_empty());
    }
    scripts
        .inspect_lua()
        .load("_parents['default_thing.lua'].events={}")
        .exec()
        .unwrap();
    assert!(
        exit_battle_building_action(&scripts, carrier)
            .unwrap()
            .is_some()
    );
    assert_eq!(scripts.world().objects[&target].location, Some(exterior));
}

/// Edit only actual motion so entry tests distinguish admission from throttle requests.
fn actual_speed(world: &mut World, id: ObjectId, speed: f64) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let class = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    state[class][id.0.to_string()]["motion"]["speed"] = speed.into();
    world.btech = serde_json::from_value(state).unwrap();
}

#[tokio::test]
async fn building_speed_limits_include_tow_load_at_admission_recheck_and_exit() {
    for source in [
        include_str!("fixtures/btech/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
        include_str!("../game/mechs/Kestrel"),
    ] {
        let (_dir, config, mut world, id, exterior, interior) = fixture(source).await;
        set_battle_building_exit(&mut world, interior, 0, Some(exterior)).unwrap();
        set_building_state(
            &mut world,
            interior,
            BattleBuildingState {
                integrity: 100,
                maximum_integrity: 100,
                flags: 0,
                regeneration: 1,
            },
        )
        .unwrap();
        let unloaded = battle_effective_maximum_speed(&world, id, true).unwrap();
        let target = attach(
            &mut world,
            &config,
            id,
            include_str!("../game/mechs/Savannah_Master"),
        );
        let maximum = battle_effective_maximum_speed(&world, id, true).unwrap();
        assert!(maximum >= 10.75 && maximum < unloaded);
        for sign in [-1.0, 1.0] {
            actual_speed(&mut world, id, sign * maximum / 5.0);
            let scripts = host(&config, world.clone());
            let before = scripts.world().btech.clone();
            assert!(
                support::run_text(&scripts, &config, ObjectId(1), 1, "enterbase N")
                    .contains("moving too fast")
            );
            assert!(
                scripts
                    .eval_callback::<bool>(&format!("return btech.unit.enterbase({},1,'N')", id.0))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, before);
            actual_speed(&mut scripts.world_mut(), id, sign * maximum / 5.0 * 0.99);
            assert!(
                begin_battle_building_entry_action(&scripts, id, ObjectId(1), Some(b'n')).unwrap()
            );
            actual_speed(&mut scripts.world_mut(), id, sign * maximum / 5.0);
            for _ in 0..18 {
                advance_battle_building_entries_action(&scripts).unwrap();
            }
            assert_eq!(scripts.world().objects[&id].location, Some(exterior));
            assert!(!battle_building_entries_pending(&scripts.world()));
        }
        actual_speed(&mut world, id, unloaded);
        transfer_battle_unit(
            &mut world,
            id,
            BattlePosition {
                map: interior,
                x: 1,
                y: 1,
            },
        )
        .unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        let live = host(&config, world);
        let replay = host(&config, restored);
        for scripts in [&live, &replay] {
            exit_battle_building_action(scripts, id).unwrap().unwrap();
        }
        assert_eq!(live.world().btech, replay.world().btech);
        for unit in [id, target] {
            let speed = live
                .world()
                .btech
                .vehicles()
                .get(&unit)
                .and_then(|u| u.motion())
                .or_else(|| {
                    live.world()
                        .btech
                        .constructed_units()
                        .get(&unit)
                        .and_then(|u| u.motion())
                })
                .unwrap()
                .speed;
            assert_eq!(speed, maximum);
        }
        live.world().validate(&config).unwrap();
    }
}

#[tokio::test]
async fn building_host_honors_configured_hot_myomer_load_assistance() {
    let (_dir, config, mut world, id, exterior, interior) =
        fixture(include_str!("fixtures/btech/mechs/JR7-D")).await;
    set_battle_building_exit(&mut world, interior, 0, Some(exterior)).unwrap();
    set_building_state(
        &mut world,
        interior,
        BattleBuildingState {
            integrity: 100,
            maximum_integrity: 100,
            flags: 0,
            regeneration: 1,
        },
    )
    .unwrap();
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    let mut remaining = 6;
    for section in [BattleSection::LeftTorso, BattleSection::RightTorso] {
        let layout = definition.sections.get_mut(&section).unwrap();
        for slot in 0..12 {
            if remaining == 0 || layout.criticals.contains_key(&slot) {
                continue;
            }
            layout.criticals.insert(
                slot,
                CriticalDefinition {
                    equipment: "TripleStrengthMyomer".into(),
                    data: "-".into(),
                    modes: vec![],
                    brand: None,
                },
            );
            remaining -= 1;
        }
    }
    assert_eq!(remaining, 0);
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["constructed"][id.0.to_string()]["definition"] =
        serde_json::to_value(definition).unwrap();
    encoded["constructed"][id.0.to_string()]["heat"]["excess"] = 9.0.into();
    world.btech = serde_json::from_value(encoded).unwrap();
    attach(
        &mut world,
        &config,
        id,
        include_str!("fixtures/btech/mechs/JR7-D"),
    );
    let disabled = battle_effective_maximum_speed(&world, id, false).unwrap();
    let enabled = battle_effective_maximum_speed(&world, id, true).unwrap();
    assert!(disabled >= 10.75 && disabled < enabled);
    actual_speed(&mut world, id, (disabled + enabled) / 10.0);
    for bonus in [false, true] {
        let path = config.root.join("stompymux.toml");
        let mut settings: toml::Value =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        settings
            .as_table_mut()
            .unwrap()
            .entry("battletech")
            .or_insert_with(|| toml::Value::Table(Default::default()))
            .as_table_mut()
            .unwrap()
            .insert(
                "tsm_tow_bonus".into(),
                toml::Value::Integer(i64::from(bonus)),
            );
        std::fs::write(&path, toml::to_string(&settings).unwrap()).unwrap();
        let configured = Config::load(&config.root).unwrap();
        let scripts = host(&configured, world.clone());
        let result =
            scripts.eval_callback::<bool>(&format!("return btech.unit.enterbase({},1,'N')", id.0));
        assert_eq!(result.is_ok(), bonus);
        if bonus {
            for _ in 0..18 {
                advance_battle_building_entries_action(&scripts).unwrap();
            }
            assert_eq!(scripts.world().objects[&id].location, Some(interior));
        } else {
            transfer_battle_unit(
                &mut scripts.world_mut(),
                id,
                BattlePosition {
                    map: interior,
                    x: 1,
                    y: 1,
                },
            )
            .unwrap();
        }
        actual_speed(&mut scripts.world_mut(), id, 118.25);
        exit_battle_building_action(&scripts, id).unwrap().unwrap();
        let maximum = if bonus { enabled } else { disabled };
        assert_eq!(
            scripts.world().btech.constructed_units()[&id]
                .motion()
                .unwrap()
                .speed,
            maximum
        );
    }
}

/// Install cover and a pending hide without advancing its independent one-second event.
fn cover(world: &mut World, ids: &[ObjectId]) {
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    for id in ids {
        let key = if world.btech.vehicles().contains_key(id) {
            "vehicles"
        } else {
            "constructed"
        };
        saved[key][id.0.to_string()]["signature"]["hidden"] = true.into();
        saved[key][id.0.to_string()]["hide_elapsed"] = 7.into();
    }
    world.btech = serde_json::from_value(saved).unwrap();
}

/// Observe the same cover facts for either chassis store.
fn cover_state(world: &World, id: ObjectId) -> (bool, Option<u16>) {
    if let Some(unit) = world.btech.vehicles().get(&id) {
        return (unit.signature().hidden, unit.hide_elapsed());
    }
    let unit = &world.btech.constructed_units()[&id];
    (unit.signature().hidden, unit.hide_elapsed())
}

/// Explicit entry preserves cover; edge movement reveals only the carrier, including after restart.
#[tokio::test]
async fn cover_distinguishes_entry_and_tow_mirroring_from_edge_movement() {
    let tracked = include_str!("../game/mechs/Demolisher");
    let chassis = [
        include_str!("../game/mechs/JR7-D").to_owned(),
        include_str!("../game/mechs/GOL-1H").to_owned(),
        tracked.to_owned(),
        tracked.replace("{ Track }", "{ Wheel }"),
        tracked.replace("{ Track }", "{ Hover }"),
        include_str!("../game/mechs/Kestrel").to_owned(),
        tracked
            .replace("{ Track }", "{ None }")
            .replace("{ 53.75 }", "{ 0 }"),
    ];
    for source in chassis.iter().take(6) {
        for target_source in &chassis {
            // Towing hardware admits every tested mass pairing without bypassing load rules.
            let equipped = format!("{source}\nSpecials {{ SalvageTech Carrier_Tech }}\n");
            let (_dir, config, mut world, carrier, exterior, interior) = fixture(&equipped).await;
            set_battle_building_exit(&mut world, interior, 0, Some(exterior)).unwrap();
            set_building_state(
                &mut world,
                interior,
                BattleBuildingState {
                    integrity: 100,
                    maximum_integrity: 100,
                    flags: 0,
                    regeneration: 1,
                },
            )
            .unwrap();
            let target = attach(&mut world, &config, carrier, target_source);
            cover(&mut world, &[carrier, target]);
            let scripts = host(&config, world);
            begin_battle_building_entry_action(&scripts, carrier, ObjectId(1), Some(b'n')).unwrap();
            for _ in 0..18 {
                advance_battle_building_entries_action(&scripts).unwrap();
            }
            for id in [carrier, target] {
                assert_eq!(scripts.world().objects[&id].location, Some(interior));
                assert_eq!(cover_state(&scripts.world(), id), (true, Some(7)));
            }
            let mut saved = serde_json::to_value(&scripts.world().btech).unwrap();
            saved["maps"][interior.0.to_string()]["movement_modifier"] = 645000.into();
            let key = if scripts.world().btech.vehicles().contains_key(&carrier) {
                "vehicles"
            } else {
                "constructed"
            };
            let unit = &mut saved[key][carrier.0.to_string()];
            unit["motion"]["heading"] = 180.into();
            unit["motion"]["desired_heading"] = 180.into();
            unit["motion"]["speed"] = 1.into();
            unit["motion"]["desired_speed"] = 1.into();
            if scripts
                .world()
                .btech
                .vehicles()
                .get(&carrier)
                .is_some_and(|u| u.definition().is_vtol())
            {
                unit["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
                    phase: BattleVtolFlightPhase::Airborne,
                    altitude: 5.0,
                    ..Default::default()
                })
                .unwrap();
            }
            scripts.world_mut().btech = serde_json::from_value(saved).unwrap();
            scripts.drain_outbox();
            let before = scripts.world().clone();
            scripts.inspect_lua().load("_parents['default_thing.lua'].events={on_move=function() error('cover rollback') end}").exec().unwrap();
            let attempted =
                advance_battle_motion_action(&scripts, &config, BattleMovementRules::STANDARD);
            assert!(
                attempted.is_err(),
                "carrier={} target={} result={attempted:?} notices={:?}",
                source.lines().next().unwrap(),
                target_source.lines().next().unwrap(),
                scripts.drain_outbox()
            );
            assert_eq!(scripts.world().btech, before.btech);
            assert!(scripts.drain_outbox().is_empty());
            scripts
                .inspect_lua()
                .load("_parents['default_thing.lua'].events={}")
                .exec()
                .unwrap();
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let restart = host(
                &config,
                persistence::load(&config.database()).await.unwrap(),
            );
            for run in [&scripts, &restart] {
                assert_eq!(
                    advance_battle_motion_action(run, &config, BattleMovementRules::STANDARD)
                        .unwrap()
                        .len(),
                    1
                );
                assert_eq!(cover_state(&run.world(), carrier), (false, None));
                assert_eq!(cover_state(&run.world(), target), (true, Some(7)));
                for id in [carrier, target] {
                    assert_eq!(run.world().objects[&id].location, Some(exterior));
                }
                let notices = run.drain_outbox();
                assert_eq!(
                    notices
                        .iter()
                        .filter(|(who, text)| *who == ObjectId(1)
                            && text.source().contains("break your cover"))
                        .count(),
                    1
                );
                assert!(
                    !notices.iter().any(|(who, text)| *who == ObjectId(2)
                        && text.source().contains("break your cover"))
                );
                run.world().validate(&config).unwrap();
            }
            assert_eq!(scripts.world().btech, restart.world().btech);
        }
    }
}

/// Entry formats shared transfer prohibitions without creating or changing an entry event.
#[tokio::test]
async fn entry_posture_and_flight_refusals_match_native_and_lua() {
    for case in 0..4 {
        let source = if case == 2 {
            include_str!("../game/mechs/Kestrel")
        } else {
            include_str!("../game/mechs/JR7-D")
        };
        let (_dir, config, mut world, id, _, _) = fixture(source).await;
        let expected = match case {
            0 => {
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["constructed"][id.0.to_string()]["posture"] =
                    serde_json::to_value(BattlePosture::Prone).unwrap();
                world.btech = serde_json::from_value(state).unwrap();
                "Crawl inside? I think not. Stand first."
            }
            1 => {
                launch_battle_jump(&mut world, id, ObjectId(1), 90, 1.0).unwrap();
                "While in mid-jump? No way."
            }
            2 => {
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["vehicles"][id.0.to_string()]["vtol_flight"]["phase"] =
                    serde_json::json!({"kind":"airborne"});
                world.btech = serde_json::from_value(state).unwrap();
                "You need to land before you can enter the hangar."
            }
            _ => {
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["constructed"][id.0.to_string()]["stand_timer"] =
                    serde_json::json!({"state":"rising", "remaining":5});
                world.btech = serde_json::from_value(state).unwrap();
                "Crawl inside? I think not. Stand first."
            }
        };
        world.validate(&config).unwrap();
        let before = world.btech.clone();
        let scripts = host(&config, world);
        assert_eq!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "enterbase"),
            expected
        );
        let error = scripts
            .eval_callback::<bool>(&format!("return btech.unit.enterbase({},1)", id.0))
            .unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
    }
}
