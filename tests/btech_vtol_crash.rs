//! Crash material reuses fall packets and preserves transactional replay and world placement checks.
use crate::support;
use stompymux_rs::*;

/// Install placed material for component tests without admitting an aircraft to live simulation.
fn aircraft(world: &mut World, id: ObjectId, map: ObjectId, falling: bool) {
    let unit = BattleVehicle::new(
        BattleVehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml"))
            .unwrap(),
    )
    .unwrap();
    let mut saved = serde_json::to_value(unit).unwrap();
    saved["position"] = serde_json::json!({"map":map.0,"x":0,"y":0});
    saved["map_slot"] = 0.into();
    saved["power"] = serde_json::to_value(if falling {
        BattlePower::Off
    } else {
        BattlePower::Running
    })
    .unwrap();
    saved["motion"] = serde_json::to_value(BattleMotion::stationary(
        BattleHexCoordinate { x: 0, y: 0 }.center(),
    ))
    .unwrap();
    saved["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
        phase: if falling {
            BattleVtolFlightPhase::Falling
        } else {
            BattleVtolFlightPhase::Airborne
        },
        altitude: 5.0,
        vertical_speed: 0.0,
        fall: falling.then(|| BattleFreeFall::new(5)),
    })
    .unwrap();
    saved["dice"] = serde_json::to_value(BattleDice::seeded([17; 32])).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()] = saved;
    world.btech = serde_json::from_value(state).unwrap();
}

#[tokio::test]
async fn crashes_share_packets_water_reduction_and_saved_replay() {
    let (_dir, config, mut base) = support::isolated_world().await;
    let map = base.create(&config, "Crash field".into(), Kind::Room);
    let id = base.create(&config, "Crash aircraft".into(), Kind::Thing);
    for (terrain, height, divisor) in [(".3", 3.0, 10), ("~3", -3.0, 20)] {
        let mut base = base.clone();
        create_battle_map(
            &mut base,
            map,
            "crash",
            BattleMapAsset::from_cells(&format!("1 1\n{terrain}\n")).unwrap(),
        )
        .unwrap();
        for falling in [false, true] {
            for levels in [0, 1, 3, 300] {
                let mut world = base.clone();
                aircraft(&mut world, id, map, falling);
                let tons = world.btech.vehicles()[&id].definition().tons;
                let mut replay = world.clone();
                let mut rules = BattleMovementRules::STANDARD.fall;
                rules.vehicle_impact.criticals.enabled = false;
                let report = resolve_battle_vtol_crash(&mut world, id, levels, rules).unwrap();
                assert_eq!(report.damage, levels * (u32::from(tons) + 5) / divisor);
                assert_eq!(report.impacts.len(), (report.damage as usize).div_ceil(5));
                assert_eq!(
                    report.avoidance.as_ref().unwrap().situational,
                    levels as i32
                );
                assert_eq!(
                    resolve_battle_vtol_crash(&mut replay, id, levels, rules).unwrap(),
                    report
                );
                assert_eq!(world.btech, replay.btech);
                let unit = &world.btech.vehicles()[&id];
                assert_eq!(
                    unit.vtol_flight().unwrap(),
                    BattleVtolFlight {
                        phase: BattleVtolFlightPhase::Landed,
                        altitude: height,
                        vertical_speed: 0.0,
                        fall: None,
                    }
                );
                assert!(unit.immobilized());
                assert_eq!(unit.maximum_speed(), 0.0);
                assert_eq!(
                    serde_json::from_value::<BattleVehicle>(serde_json::to_value(unit).unwrap())
                        .unwrap(),
                    *unit
                );
                let before = world.btech.clone();
                assert!(resolve_battle_vtol_crash(&mut world, id, levels, rules).is_err());
                assert_eq!(world.btech, before);
            }
        }
    }
}

#[tokio::test]
async fn safe_crash_stops_descent_without_damage_and_invalid_crashes_are_atomic() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Safe field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "safe",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Safe aircraft".into(), Kind::Thing);
    aircraft(&mut world, id, map, true);
    let original = world.btech.clone();
    let mut rules = BattleMovementRules::STANDARD.fall;
    assert!(resolve_battle_vtol_crash(&mut world, id, u32::MAX, rules).is_err());
    assert_eq!(world.btech, original);
    rules.vehicle_impact.criticals.combat_safe = true;
    let report = resolve_battle_vtol_crash(&mut world, id, 3, rules).unwrap();
    assert!(report.avoidance.is_none());
    assert!(report.impacts.is_empty());
    let unit = &world.btech.vehicles()[&id];
    assert!(!unit.immobilized());
    assert_eq!(unit.sections(), original.vehicles()[&id].sections());
    assert_eq!(
        unit.vtol_flight().unwrap().phase,
        BattleVtolFlightPhase::Landed
    );
    let mut dice = BattleDice::seeded([17; 32]);
    dice.d6();
    assert_eq!(
        serde_json::to_value(unit).unwrap()["dice"],
        serde_json::to_value(dice).unwrap()
    );
    // The material adapter must not weaken the whole-world placement checks.
    assert!(
        world
            .validate(&config)
            .unwrap_err()
            .to_string()
            .contains("Placed vehicle location differs")
    );
}

#[tokio::test]
#[cfg_attr(
    not(debug_assertions),
    ignore = "injects a late failure through per-operation validation, which runs only in debug builds"
)]
async fn nested_mine_admission_failure_rolls_back_crash_damage_and_dice() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Mined field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "mined",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    set_minefield(
        &mut world,
        map,
        0,
        Some(BattleMinefield {
            coordinate: BattleHexCoordinate { x: 0, y: 0 },
            kind: BattleMineKind::Command,
            strength: 1,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    let id = world.create(&config, "Mined aircraft".into(), Kind::Thing);
    aircraft(&mut world, id, map, true);
    let before = world.btech.clone();
    let mut rules = BattleMovementRules::STANDARD.fall;
    rules.vehicle_impact.criticals.enabled = false;
    let error = resolve_battle_vtol_crash(&mut world, id, 1, rules).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Placed vehicle location differs"),
        "{error}"
    );
    assert_eq!(world.btech, before);
    // The same rejection at the scheduled impact must retain the due cursor.
    for _ in 0..5 {
        let event = advance_battle_vtol_fall(&mut world, id, rules).unwrap();
        assert!(!matches!(event, BattleVehicleDescentEvent::Impact { .. }));
    }
    let due = world.btech.clone();
    for _ in 0..2 {
        let error = advance_battle_vtol_fall(&mut world, id, rules).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Placed vehicle location differs"),
            "{error}"
        );
        assert_eq!(world.btech, due);
    }
}

#[tokio::test]
async fn descent_commits_crashes_at_shared_clock_boundaries_and_survives_reload() {
    let (_dir, config, mut base) = support::isolated_world().await;
    let map = base.create(&config, "Descent field".into(), Kind::Room);
    let id = base.create(&config, "Falling aircraft".into(), Kind::Thing);
    for (terrain, surface) in [(".0", 0), (".3", 3), ("/3", 3), ("/8", -1), ("~3", -3)] {
        let mut world = base.clone();
        create_battle_map(
            &mut world,
            map,
            "descent",
            BattleMapAsset::from_cells(&format!("1 1\n{terrain}\n")).unwrap(),
        )
        .unwrap();
        aircraft(&mut world, id, map, true);
        let mut clock = BattleFreeFall::new(5);
        let mut rules = BattleMovementRules::STANDARD.fall;
        rules.vehicle_impact.criticals.enabled = false;
        let mut landed = false;
        for _ in 0..20 {
            let before = world.btech.clone();
            let step = clock.advance(surface).unwrap();
            let mut replay = world.clone();
            replay.btech =
                serde_json::from_value(serde_json::to_value(&world.btech).unwrap()).unwrap();
            let event = advance_battle_vtol_fall(&mut world, id, rules).unwrap();
            assert_eq!(
                advance_battle_vtol_fall(&mut replay, id, rules).unwrap(),
                event
            );
            assert_eq!(world.btech, replay.btech);
            match step {
                BattleFreeFallStep::Recovered => panic!("Unpowered clock cannot recover"),
                BattleFreeFallStep::Waiting => {
                    assert_eq!(event, BattleVehicleDescentEvent::Waiting)
                }
                BattleFreeFallStep::Descending => {
                    assert_eq!(event, BattleVehicleDescentEvent::Descending)
                }
                BattleFreeFallStep::Impact { levels } => {
                    let BattleVehicleDescentEvent::Impact {
                        levels: actual,
                        fall,
                    } = event
                    else {
                        panic!("Missing crash")
                    };
                    assert_eq!(actual, levels);
                    let mut expected = world.clone();
                    expected.btech = before;
                    assert_eq!(
                        *fall,
                        resolve_battle_vtol_crash(&mut expected, id, levels, rules).unwrap()
                    );
                    assert_eq!(world.btech, expected.btech);
                    let flight = world.btech.vehicles()[&id].vtol_flight().unwrap();
                    assert_eq!(flight.phase, BattleVtolFlightPhase::Landed);
                    assert_eq!(flight.altitude, f64::from(surface));
                    assert!(flight.fall.is_none());
                    let settled = world.btech.clone();
                    assert!(advance_battle_vtol_fall(&mut world, id, rules).is_err());
                    assert_eq!(world.btech, settled);
                    landed = true;
                    break;
                }
            }
            let unit = &world.btech.vehicles()[&id];
            assert_eq!(unit.vtol_flight().unwrap().fall, Some(clock));
            assert_eq!(
                unit.vtol_flight().unwrap().altitude,
                f64::from(clock.elevation())
            );
            assert_eq!(
                serde_json::to_value(unit).unwrap()["dice"],
                serde_json::to_value(&before.vehicles()[&id]).unwrap()["dice"]
            );
            assert_eq!(unit.sections(), before.vehicles()[&id].sections());
        }
        assert!(landed, "{terrain}");
    }
}

#[tokio::test]
async fn movement_dispatch_advances_falls_and_powered_flight_exactly_once() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Dispatch field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "dispatch",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let falling = world.create(&config, "Unpowered aircraft".into(), Kind::Thing);
    let flying = world.create(&config, "Flying aircraft".into(), Kind::Thing);
    aircraft(&mut world, falling, map, true);
    aircraft(&mut world, flying, map, false);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][falling.0.to_string()]["power"] =
        serde_json::to_value(BattlePower::Off).unwrap();
    state["vehicles"][flying.0.to_string()]["map_slot"] = 1.into();
    state["vehicles"][flying.0.to_string()]["motion"]["speed"] = 20.into();
    state["vehicles"][flying.0.to_string()]["motion"]["desired_speed"] = 20.into();
    world.btech = serde_json::from_value(state).unwrap();
    let airborne = world.btech.vehicles()[&flying].clone();
    let mut rules = BattleMovementRules::STANDARD;
    rules.fall.vehicle_impact.criticals.enabled = false;
    for tick in 1..=6 {
        let mut expected = world.clone();
        let event = advance_battle_vtol_fall(&mut expected, falling, rules.fall).unwrap();
        let mut flyer = expected.btech.vehicles()[&flying].clone();
        let _ = flyer.consume_vtol_fuel(0.0, 5, false, false).unwrap();
        let mut state = serde_json::to_value(&expected.btech).unwrap();
        state["vehicles"][flying.0.to_string()] = serde_json::to_value(flyer).unwrap();
        expected.btech = serde_json::from_value(state).unwrap();
        let _ = advance_battle_vtol_environment(&mut expected, flying, false, rules.fall).unwrap();
        let notices = advance_battle_motion(&mut world, rules).unwrap();
        assert_eq!(world.btech, expected.btech);
        assert_eq!(
            world.btech.vehicles()[&flying].sections(),
            airborne.sections()
        );
        if let BattleVehicleDescentEvent::Impact { fall, .. } = event {
            assert_eq!(tick, 6);
            assert!(
                notices
                    .iter()
                    .any(|notice| notice.unit == falling && notice.text == "You hit the ground!")
            );
            for notice in fall.notices {
                assert!(notices.contains(&notice));
            }
        } else {
            assert!(notices.is_empty());
        }
    }
    let settled = world.btech.vehicles()[&falling].clone();
    assert!(advance_battle_motion(&mut world, rules).unwrap().is_empty());
    assert_eq!(world.btech.vehicles()[&falling], settled);
}

#[tokio::test]
async fn host_movement_uses_character_path_and_rolls_back_invalid_placement() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Host field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "host",
        BattleMapAsset::from_cells("1 1\n.3\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Host aircraft".into(), Kind::Thing);
    aircraft(&mut world, id, map, true);
    for _ in 0..2 {
        let _ =
            advance_battle_vtol_fall(&mut world, id, BattleMovementRules::STANDARD.fall).unwrap();
    }
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
    let before = world.clone();
    assert!(
        advance_battle_motion(&mut world, BattleMovementRules::STANDARD)
            .unwrap_err()
            .to_string()
            .contains("character publication")
    );
    assert_eq!(world.btech, before.btech);
    // The invalid placement below is caught by per-operation validation, which runs only in
    // debug builds; release relies on the commit check.
    if !cfg!(debug_assertions) {
        return;
    }
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let error =
        advance_battle_motion_action(&scripts, &config, BattleMovementRules::STANDARD).unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Placed vehicle location differs"),
        "{error}"
    );
    assert_eq!(scripts.world().btech, before.btech);
    assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(id));
    assert!(scripts.drain_outbox().is_empty());
}

#[tokio::test]
async fn world_contacts_commit_clear_flight_landing_crash_and_water_with_replay() {
    let (_dir, config, mut base) = support::isolated_world().await;
    let map = base.create(&config, "Contact field".into(), Kind::Room);
    let id = base.create(&config, "Contact aircraft".into(), Kind::Thing);
    for (tile, altitude, vertical, expected) in [
        (".1", 5.0, 60.0, "movement"),
        (".1", 1.01, -5.0, "landing"),
        (".1", 1.5, -129.0, "crash"),
        ("~5", -0.5, -129.0, "water"),
    ] {
        let mut world = base.clone();
        create_battle_map(
            &mut world,
            map,
            "contact",
            BattleMapAsset::from_cells(&format!("1 1\n{tile}\n")).unwrap(),
        )
        .unwrap();
        aircraft(&mut world, id, map, false);
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["vehicles"][id.0.to_string()]["vtol_flight"]["altitude"] = altitude.into();
        state["vehicles"][id.0.to_string()]["vtol_flight"]["vertical_speed"] = vertical.into();
        world.btech = serde_json::from_value(state).unwrap();
        let before = world.btech.clone();
        let mut replay = world.clone();
        let mut rules = BattleMovementRules::STANDARD.fall;
        rules.vehicle_impact.criticals.enabled = false;
        let result = advance_battle_vtol_environment(&mut world, id, false, rules).unwrap();
        assert_eq!(
            advance_battle_vtol_environment(&mut replay, id, false, rules).unwrap(),
            result
        );
        assert_eq!(world.btech, replay.btech);
        let unit = &world.btech.vehicles()[&id];
        match (expected, result) {
            (
                "movement",
                BattleVtolEnvironment::Movement {
                    path: BattleVtolPath::Advanced { .. },
                },
            ) => {
                assert_eq!(
                    unit.vtol_flight().unwrap().altitude,
                    altitude + vertical / 129.0
                );
            }
            ("landing", BattleVtolEnvironment::Landed { .. }) => {
                assert_eq!(unit.vtol_flight().unwrap().altitude, 1.0);
                assert_eq!(
                    unit.vtol_flight().unwrap().phase,
                    BattleVtolFlightPhase::Landed
                );
            }
            (
                "crash",
                BattleVtolEnvironment::Crashed {
                    path: BattleVtolPath::Contact { contact, .. },
                    fall,
                },
            ) => {
                assert_eq!(
                    contact,
                    BattleVtolSurfaceContact::Ground { fall_levels: 13 }
                );
                assert_eq!(fall.avoidance.unwrap().situational, 13);
                assert!(!fall.impacts.is_empty());
                assert_eq!(unit.vtol_flight().unwrap().altitude, 1.0);
                assert!(unit.immobilized());
            }
            ("water", BattleVtolEnvironment::Flooded { newly: true, .. }) => {
                assert!(unit.flooded() && unit.is_destroyed());
                assert!(!unit.crew_killed());
                assert_eq!(unit.power(), BattlePower::Off);
                assert_eq!(
                    unit.vtol_flight().unwrap().phase,
                    BattleVtolFlightPhase::Landed
                );
                assert!(unit.vtol_flight().unwrap().fall.is_none());
                assert!(unit.vtol_flight().unwrap().altitude <= -1.0);
            }
            (_, result) => panic!("Unexpected {expected} outcome: {result:?}"),
        }
        if expected != "crash" {
            assert_eq!(unit.sections(), before.vehicles()[&id].sections());
            assert_eq!(
                serde_json::to_value(unit).unwrap()["dice"],
                serde_json::to_value(&before.vehicles()[&id]).unwrap()["dice"]
            );
        }
        assert_eq!(
            serde_json::from_value::<BattleVehicle>(serde_json::to_value(unit).unwrap()).unwrap(),
            *unit
        );
    }
}

#[tokio::test]
#[cfg_attr(
    not(debug_assertions),
    ignore = "injects a late failure through per-operation validation, which runs only in debug builds"
)]
async fn world_contact_failure_restores_precontact_height_position_and_dice() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Rejected contact field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "contact",
        BattleMapAsset::from_cells("1 1\n.1\n").unwrap(),
    )
    .unwrap();
    set_minefield(
        &mut world,
        map,
        0,
        Some(BattleMinefield {
            coordinate: BattleHexCoordinate { x: 0, y: 0 },
            kind: BattleMineKind::Command,
            strength: 1,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    let id = world.create(&config, "Rejected aircraft".into(), Kind::Thing);
    aircraft(&mut world, id, map, false);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["vtol_flight"]["altitude"] = 1.5.into();
    state["vehicles"][id.0.to_string()]["vtol_flight"]["vertical_speed"] = (-129.0).into();
    world.btech = serde_json::from_value(state).unwrap();
    let before = world.btech.clone();
    let mut rules = BattleMovementRules::STANDARD.fall;
    rules.vehicle_impact.criticals.enabled = false;
    for _ in 0..2 {
        let error = advance_battle_vtol_environment(&mut world, id, false, rules).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Placed vehicle location differs"),
            "{error}"
        );
        assert_eq!(world.btech, before);
    }
}

#[tokio::test]
async fn launch_flight_and_fuel_exhaustion_share_one_restartable_tick() {
    let (_dir, config, mut base) = support::isolated_world().await;
    let map = base.create(&config, "Launch field".into(), Kind::Room);
    create_battle_map(
        &mut base,
        map,
        "launch",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = base.create(&config, "Launching aircraft".into(), Kind::Thing);
    for free_fuel in [false, true] {
        let mut world = base.clone();
        aircraft(&mut world, id, map, false);
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["vehicles"][id.0.to_string()]["vtol_flight"] =
            serde_json::to_value(BattleVtolFlight {
                phase: BattleVtolFlightPhase::Launching { remaining: 2 },
                altitude: 0.0,
                vertical_speed: 0.0,
                fall: None,
            })
            .unwrap();
        state["vehicles"][id.0.to_string()]["vtol_fuel"]["remaining"] =
            if free_fuel { 0 } else { 1 }.into();
        if free_fuel {
            state["vehicles"][id.0.to_string()]["definition"]["attributes"]["specials"] =
                "CargoTech".into();
        }
        world.btech = serde_json::from_value(state).unwrap();
        let rules = BattleMovementRules {
            free_fusion_vtol_fuel: free_fuel,
            ..BattleMovementRules::STANDARD
        };
        for tick in 1..=5 {
            let mut replay = world.clone();
            replay.btech =
                serde_json::from_value(serde_json::to_value(&world.btech).unwrap()).unwrap();
            let notices = advance_battle_motion(&mut world, rules).unwrap();
            assert_eq!(advance_battle_motion(&mut replay, rules).unwrap(), notices);
            assert_eq!(world.btech, replay.btech);
            let unit = &world.btech.vehicles()[&id];
            let flight = unit.vtol_flight().unwrap();
            if tick == 1 {
                assert_eq!(
                    flight.phase,
                    BattleVtolFlightPhase::Launching { remaining: 1 }
                );
                assert_eq!(flight.altitude, 0.0);
            } else if tick == 2 {
                assert_eq!(flight.phase, BattleVtolFlightPhase::Airborne);
                assert_eq!(flight.altitude, 0.0);
                assert_eq!(flight.vertical_speed, 60.0);
                assert!(notices.iter().any(|notice| notice.text == "You lift off!"));
            } else if free_fuel || tick == 3 {
                assert_eq!(flight.phase, BattleVtolFlightPhase::Airborne);
                assert!((flight.altitude - f64::from(tick - 2) * 60.0 / 129.0).abs() < 1e-12);
                assert_eq!(unit.vtol_fuel().unwrap().remaining(), 0);
            } else {
                assert_eq!(flight.phase, BattleVtolFlightPhase::Falling);
                assert_eq!(flight.altitude, 60.0 / 129.0);
                assert_eq!(unit.vtol_fuel().unwrap().remaining(), -1);
                let mut cursor = BattleFreeFall::new(0);
                if tick == 5 {
                    let _ = cursor.advance(0).unwrap();
                }
                assert_eq!(flight.fall, Some(cursor));
                assert_eq!(
                    notices
                        .iter()
                        .filter(|notice| notice.text.contains("run out of fuel"))
                        .count(),
                    usize::from(tick == 4)
                );
            }
        }
    }
}

#[tokio::test]
async fn launch_rechecks_ceiling_and_unlinked_boundaries_stop_horizontal_flight() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Ceiling field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "ceiling",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Ceiling aircraft".into(), Kind::Thing);
    aircraft(&mut world, id, map, false);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["flags"] = 16.into();
    state["vehicles"][id.0.to_string()]["vtol_flight"]["phase"] =
        serde_json::json!({"kind":"launching","remaining":1});
    world.btech = serde_json::from_value(state).unwrap();
    let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
    assert!(notices.iter().any(|notice| notice.text.contains("ceiling")));
    assert_eq!(
        world.btech.vehicles()[&id].vtol_flight().unwrap().phase,
        BattleVtolFlightPhase::Landed
    );
    aircraft(&mut world, id, map, false);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["flags"] = 0.into();
    state["maps"][map.0.to_string()]["movement_modifier"] = 64500.into();
    state["vehicles"][id.0.to_string()]["motion"]["desired_speed"] = 100.into();
    world.btech = serde_json::from_value(state).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["vtol_flight"]["vertical_speed"] = 60.into();
    world.btech = serde_json::from_value(state).unwrap();
    let before = world.btech.clone();
    let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
    assert!(
        notices
            .iter()
            .any(|notice| notice.text == "You cannot move off this map!")
    );
    let unit = &world.btech.vehicles()[&id];
    assert_eq!(unit.motion().unwrap().speed, 0.0);
    assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
    assert_eq!(
        unit.vtol_fuel().unwrap().remaining(),
        before.vehicles()[&id].vtol_fuel().unwrap().remaining() - 1
    );
    let flight = unit.vtol_flight().unwrap();
    assert_eq!(flight.phase, BattleVtolFlightPhase::Airborne);
    assert_eq!(flight.vertical_speed, 60.0);
    assert!(flight.altitude > 5.0 && flight.altitude < 5.0 + 60.0 / 129.0);
    assert_eq!(unit.sections(), before.vehicles()[&id].sections());
    assert!(
        advance_battle_motion(&mut world, BattleMovementRules::STANDARD)
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        world.btech.vehicles()[&id].vtol_flight().unwrap().altitude,
        flight.altitude + 60.0 / 129.0
    );
}

#[tokio::test]
async fn shared_control_commands_apply_aircraft_velocity_and_rotor_limits() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Control field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "controls",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Controlled aircraft".into(), Kind::Thing);
    BattleUnitTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml"))
        .unwrap()
        .create(&mut world, id)
        .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    aircraft(&mut world, id, map, false);
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["pilot"] = 2.into();
    state["vehicles"][id.0.to_string()]["vtol_flight"]["vertical_speed"] = 60.into();
    world.btech = serde_json::from_value(state).unwrap();
    let maximum = world.btech.vehicles()[&id]
        .vtol_horizontal_limit(60.0)
        .unwrap();
    let _ = set_battle_speed(&mut world, id, ObjectId(2), 10000.0).unwrap();
    assert_eq!(
        world.btech.vehicles()[&id].motion().unwrap().desired_speed,
        maximum
    );
    let _ = set_battle_speed(&mut world, id, ObjectId(2), -10000.0).unwrap();
    assert_eq!(
        world.btech.vehicles()[&id].motion().unwrap().desired_speed,
        -maximum * 2.0 / 3.0
    );
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["tail_rotor_destroyed"] = true.into();
    world.btech = serde_json::from_value(state).unwrap();
    let before = world.btech.clone();
    assert!(set_battle_speed(&mut world, id, ObjectId(2), maximum).is_err());
    assert_eq!(world.btech, before);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["vtol_flight"]["phase"] =
        serde_json::json!({"kind":"landed"});
    state["vehicles"][id.0.to_string()]["vtol_flight"]["vertical_speed"] = 0.into();
    world.btech = serde_json::from_value(state).unwrap();
    let before = world.btech.clone();
    set_battle_speed(&mut world, id, ObjectId(2), 1.0).unwrap();
    assert_eq!(
        world.btech.vehicles()[&id].motion().unwrap().desired_speed,
        1.0
    );
    assert_eq!(
        world.btech.vehicles()[&id].vtol_flight(),
        before.vehicles()[&id].vtol_flight()
    );
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["vtol_flight"]["phase"] =
        serde_json::json!({"kind":"airborne"});
    world.btech = serde_json::from_value(state).unwrap();
    let _ = stop_battle_unit(
        &mut world,
        id,
        ObjectId(2),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let unit = &world.btech.vehicles()[&id];
    assert_eq!(unit.power(), BattlePower::Off);
    assert_eq!(
        unit.vtol_flight().unwrap().phase,
        BattleVtolFlightPhase::Falling
    );
    assert_eq!(
        unit.vtol_flight().unwrap().fall,
        Some(BattleFreeFall::new(5))
    );
}

#[tokio::test]
async fn boundary_resolution_reuses_the_first_exit_and_preserves_saved_replay() {
    let (_dir, config, mut base) = support::isolated_world().await;
    let map = base.create(&config, "Boundary field".into(), Kind::Room);
    let asset = BattleMapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap();
    create_battle_map(&mut base, map, "boundary", asset.clone()).unwrap();
    let id = base.create(&config, "Boundary aircraft".into(), Kind::Thing);
    for heading in [0.0, 45.0, 90.0, 135.0, 180.0, 225.0, 270.0, 315.0] {
        let mut world = base.clone();
        aircraft(&mut world, id, map, false);
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["maps"][map.0.to_string()]["movement_modifier"] = 10000.into();
        state["vehicles"][id.0.to_string()]["position"]["x"] = 1.into();
        state["vehicles"][id.0.to_string()]["position"]["y"] = 1.into();
        state["vehicles"][id.0.to_string()]["motion"] = serde_json::to_value(BattleMotion {
            point: BattleHexCoordinate { x: 1, y: 1 }.center(),
            heading,
            desired_heading: heading,
            speed: 193.5,
            desired_speed: 193.5,
        })
        .unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        let mut pure = world.btech.vehicles()[&id].clone();
        let path = pure.advance_vtol_clear_path(&asset, 10000).unwrap();
        let BattleVtolPath::MapEdge {
            hex,
            last,
            altitude,
        } = path
        else {
            panic!("Expected an edge")
        };
        assert!(asset.hex(hex.x, hex.y).is_none());
        assert!(asset.hex(last.x, last.y).is_some());
        assert_ne!(last, BattleHexCoordinate { x: 1, y: 1 });
        let before = world.btech.clone();
        assert_eq!(pure, before.vehicles()[&id]);
        let mut replay = world.clone();
        let result = advance_battle_vtol_environment(
            &mut world,
            id,
            false,
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        assert_eq!(result, BattleVtolEnvironment::Boundary { path });
        assert_eq!(
            advance_battle_vtol_environment(
                &mut replay,
                id,
                false,
                BattleMovementRules::STANDARD.fall
            )
            .unwrap(),
            result
        );
        assert_eq!(world.btech, replay.btech);
        let unit = &world.btech.vehicles()[&id];
        assert_eq!(unit.motion().unwrap().point, last.center());
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
        assert_eq!(unit.motion().unwrap().desired_heading, heading);
        assert_eq!(unit.vtol_flight().unwrap().altitude, altitude);
        assert_eq!(unit.sections(), before.vehicles()[&id].sections());
        assert_eq!(
            serde_json::to_value(unit).unwrap()["dice"],
            serde_json::to_value(&before.vehicles()[&id]).unwrap()["dice"]
        );
        assert_eq!(
            serde_json::from_value::<BattleVehicle>(serde_json::to_value(unit).unwrap()).unwrap(),
            *unit
        );
    }
}

#[tokio::test]
async fn an_intermediate_ground_collision_takes_precedence_over_a_map_exit() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Hill at boundary".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "hill",
        BattleMapAsset::from_cells("1 3\n.0\n^9\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Hill aircraft".into(), Kind::Thing);
    aircraft(&mut world, id, map, false);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["movement_modifier"] = 10000.into();
    state["vehicles"][id.0.to_string()]["position"]["y"] = 2.into();
    state["vehicles"][id.0.to_string()]["motion"] = serde_json::to_value(BattleMotion {
        point: BattleHexCoordinate { x: 0, y: 2 }.center(),
        heading: 0.0,
        desired_heading: 0.0,
        speed: 100.0,
        desired_speed: 100.0,
    })
    .unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let mut rules = BattleMovementRules::STANDARD.fall;
    rules.vehicle_impact.criticals.enabled = false;
    let result = advance_battle_vtol_environment(&mut world, id, false, rules).unwrap();
    assert!(matches!(
        result,
        BattleVtolEnvironment::Obstacle {
            path: BattleVtolPath::Contact {
                hex: BattleHexCoordinate { x: 0, y: 1 },
                contact: BattleVtolSurfaceContact::Elevation,
                ..
            },
            fall: None,
            ..
        }
    ));
    assert_eq!(world.btech.vehicles()[&id].position().unwrap().y, 2);
    assert_eq!(
        world.btech.vehicles()[&id].vtol_flight().unwrap().altitude,
        0.0
    );
    assert_eq!(
        world.btech.vehicles()[&id].vtol_flight().unwrap().phase,
        BattleVtolFlightPhase::Landed
    );
    assert_eq!(
        world.btech.vehicles()[&id].motion().unwrap().desired_speed,
        100.0
    );
}

#[tokio::test]
async fn movement_dispatch_recovers_powered_aircraft_without_impact_or_dice() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Recovery field".into(), Kind::Room);
    let id = world.create(&config, "Recovery aircraft".into(), Kind::Thing);
    create_battle_map(
        &mut world,
        map,
        "recovery",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    aircraft(&mut world, id, map, true);
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    let unit = &mut saved["vehicles"][id.0.to_string()];
    unit["power"] = serde_json::to_value(BattlePower::Running).unwrap();
    unit["definition"]["attributes"]["specials"] = "CargoTech".into();
    unit["vtol_fuel"]["remaining"] = 0.into();
    unit["vtol_flight"]["fall"]["speed"] = 0.into();
    unit["vtol_flight"]["fall"]["remaining"] = 1.into();
    world.btech = serde_json::from_value(saved).unwrap();
    let before = serde_json::to_value(&world.btech.vehicles()[&id]).unwrap();
    let mut replay = world.clone();
    let rules = BattleMovementRules {
        free_fusion_vtol_fuel: true,
        ..BattleMovementRules::STANDARD
    };
    let report = advance_battle_motion(&mut world, rules).unwrap();
    let other = advance_battle_motion(&mut replay, rules).unwrap();
    assert_eq!(
        serde_json::to_value(report).unwrap(),
        serde_json::to_value(other).unwrap()
    );
    assert_eq!(world.btech, replay.btech);
    let unit = &world.btech.vehicles()[&id];
    assert_eq!(
        unit.vtol_flight().unwrap().phase,
        BattleVtolFlightPhase::Airborne
    );
    assert!(unit.vtol_flight().unwrap().fall.is_none());
    let after = serde_json::to_value(unit).unwrap();
    for field in ["dice", "sections", "motion", "vtol_fuel"] {
        assert_eq!(before[field], after[field]);
    }
}

/// Hull destruction keeps the existing descent cursor and uses ordinary fall packet resolution.
#[tokio::test]
async fn destroyed_aircraft_descend_and_settle_with_replay_and_no_second_pilot_loss() {
    let (_dir, config, mut base) = support::isolated_world().await;
    let map = base.create(&config, "Wreck field".into(), Kind::Room);
    let id = base.create(&config, "Falling wreck".into(), Kind::Thing);
    for (terrain, surface) in [(".0", 0.0), ("~3", -3.0), ("/3", 3.0)] {
        let mut world = base.clone();
        create_battle_map(
            &mut world,
            map,
            "wreck",
            BattleMapAsset::from_cells(&format!("1 1\n{terrain}\n")).unwrap(),
        )
        .unwrap();
        aircraft(&mut world, id, map, false);
        let mut unit = world.btech.vehicles()[&id].clone();
        let hit = unit
            .damage_phase(
                BattleVehicleSection::Front,
                u16::MAX,
                BattleDamagePhase::Internal,
            )
            .unwrap();
        assert!(hit.unit_destroyed);
        assert!(unit.is_destroyed());
        assert_eq!(
            unit.vtol_flight().unwrap().phase,
            BattleVtolFlightPhase::Falling
        );
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved["vehicles"][id.0.to_string()] = serde_json::to_value(unit).unwrap();
        world.btech = serde_json::from_value(saved).unwrap();
        let mut rules = BattleMovementRules::STANDARD;
        rules.fall.vehicle_impact.criticals.enabled = false;
        let mut impacted = false;
        for _ in 0..30 {
            let mut replay = world.clone();
            replay.btech =
                serde_json::from_value(serde_json::to_value(&world.btech).unwrap()).unwrap();
            let mut dispatched = replay.clone();
            let _ = advance_battle_motion(&mut dispatched, rules).unwrap();
            let event = advance_battle_vtol_fall(&mut world, id, rules.fall).unwrap();
            assert_eq!(
                advance_battle_vtol_fall(&mut replay, id, rules.fall).unwrap(),
                event
            );
            assert_eq!(world.btech, replay.btech);
            assert_eq!(world.btech, dispatched.btech);
            if let BattleVehicleDescentEvent::Impact { fall, .. } = event {
                assert!(!fall.impacts.is_empty());
                assert!(fall.pilot_injury.is_none());
                assert!(fall.character_injury.is_none());
                let avoidance = fall.avoidance.unwrap();
                assert!(!avoidance.success && avoidance.roll.is_none());
                impacted = true;
                break;
            }
        }
        assert!(impacted);
        let unit = &world.btech.vehicles()[&id];
        assert!(unit.is_destroyed());
        let flight = unit.vtol_flight().unwrap();
        assert_eq!(flight.phase, BattleVtolFlightPhase::Landed);
        assert_eq!(flight.altitude, surface);
        assert!(flight.fall.is_none());
        let settled = world.btech.clone();
        assert!(advance_battle_vtol_fall(&mut world, id, rules.fall).is_err());
        assert_eq!(world.btech, settled);
        assert!(resolve_battle_vehicle_fall(&mut world, id, 1, rules.fall).is_err());
        assert_eq!(world.btech, settled);
        let report = advance_battle_motion(&mut world, rules).unwrap();
        assert!(report.is_empty());
        assert_eq!(world.btech, settled);
    }
}
