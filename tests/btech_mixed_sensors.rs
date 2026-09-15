//! Physical sensors share mixed-unit facts, emission lifetime and independent signal replay.
use stompymux_rs::*;
mod support;

/// Change saved scenario inputs for either chassis without duplicating test logic.
fn fact(world: &mut World, id: ObjectId, edit: impl FnOnce(&mut serde_json::Value)) {
    let key = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    edit(&mut saved[key][id.0.to_string()]);
    world.btech = serde_json::from_value(saved).unwrap();
}

#[tokio::test]
async fn physical_sensors_share_mixed_queries_emissions_and_signal_replay() {
    let templates = [
        include_str!("fixtures/btech/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
        include_str!("../game/mechs/Kestrel"),
    ];
    for source in templates {
        for target_source in templates {
            let (_dir, config, mut world) = support::isolated_world().await;
            let map = world.create(&config, "Sensor field".into(), Kind::Room);
            create_battle_map(
                &mut world,
                map,
                "sensors",
                BattleMapAsset::parse("1 4\n.0\n.0\n.0\n.0\n").unwrap(),
            )
            .unwrap();
            let pilot = world.create(&config, "Other pilot".into(), Kind::Player);
            let mut ids = Vec::new();
            for (template, pilot, y) in [(source, ObjectId(1), 0), (target_source, pilot, 2)] {
                let id = world.create(&config, "Sensor unit".into(), Kind::Thing);
                BattleUnitTemplate::parse(template)
                    .unwrap()
                    .create(&mut world, id)
                    .unwrap();
                place_battle_unit(&mut world, id, map, 0, y).unwrap();
                world.objects.get_mut(&pilot).unwrap().location = Some(id);
                assign_battle_pilot(&mut world, id, pilot).unwrap();
                start_battle_unit(&mut world, id, pilot, true).unwrap();
                ids.push(id);
            }
            for _ in 0..5 {
                advance_battle_units(&mut world, 0);
            }
            let (observer, target) = (ids[0], ids[1]);
            for &id in &ids {
                fact(&mut world, id, |state| {
                    state["sensor_signal"] =
                        serde_json::to_value(BattleSensorSignal::seeded(100, [27; 32]).unwrap())
                            .unwrap()
                });
            }
            configure_battle_sensor_policy(&mut world, true);
            let em = BattleElectromagneticRules {
                signal_strength: 100,
                aim_adjustment: 0,
            };
            let seismic = BattleSeismicRules {
                detect_stopped: true,
                signal_strength: 100,
                aim_adjustment: 0,
            };
            let before = world.btech.clone();
            let baseline = battle_electromagnetic_contact(&world, observer, target, em).unwrap();
            assert!(baseline.eligible);
            assert!(
                battle_seismic_contact(&world, observer, target, seismic)
                    .unwrap()
                    .eligible
            );
            assert!(
                !battle_seismic_contact(
                    &world,
                    observer,
                    target,
                    BattleSeismicRules {
                        detect_stopped: false,
                        ..seismic
                    }
                )
                .unwrap()
                .eligible
            );
            assert_eq!(world.btech, before);
            for sensor in [
                BattleSensorMode::Electromagnetic,
                BattleSensorMode::Seismic,
                BattleSensorMode::Infrared,
            ] {
                select_battle_optical_sensors(
                    &mut world,
                    observer,
                    ObjectId(1),
                    BattleSensorPair {
                        primary: sensor,
                        secondary: sensor,
                    },
                )
                .unwrap();
                for _ in 0..10 {
                    advance_battle_sensor_selection(&mut world);
                }
                let report =
                    battle_map_optical_contact(&world, observer, target, sensor, false, false)
                        .unwrap();
                assert!(report.eligible);
                assert!(
                    !battle_map_optical_contact(&world, observer, target, sensor, false, true)
                        .unwrap()
                        .eligible
                );
                assert_eq!(
                    battle_hex_sensor_visibility(
                        &world,
                        observer,
                        BattleHexCoordinate { x: 0, y: 1 }
                    )
                    .unwrap()
                    .primary,
                    sensor != BattleSensorMode::Seismic
                );
                refresh_optical_scanners(&mut world, &[observer]).unwrap();
            }
            // Actual expenditure, rather than aim or inspection, creates the emission.
            if world.btech.vehicles().contains_key(&target) {
                let _ = reserve_battle_vehicle_weapon(&mut world, target, pilot, 0, true).unwrap();
            } else {
                let _ = spend_battle_weapon(&mut world, target, pilot, 0).unwrap();
            }
            let fired = battle_electromagnetic_contact(&world, observer, target, em).unwrap();
            assert_eq!(fired.aim_modifier, baseline.aim_modifier - 1);
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            configure_battle_sensor_policy(&mut replay, true);
            assert_eq!(
                battle_electromagnetic_contact(&replay, observer, target, em).unwrap(),
                fired
            );
            let mut next_attack = world.clone();
            let expected = roll_unit_dice(&mut next_attack, target, 5).unwrap();
            for _ in 0..20 {
                advance_battle_sensor_signals(&mut world);
                advance_battle_sensor_signals(&mut replay);
            }
            assert_eq!(world.btech, replay.btech);
            assert_eq!(roll_unit_dice(&mut world, target, 5).unwrap(), expected);
            clear_battle_recent_fire(&mut world);
            assert_eq!(
                battle_electromagnetic_contact(&world, observer, target, em)
                    .unwrap()
                    .aim_modifier,
                baseline.aim_modifier
            );
            for &id in &ids {
                if world
                    .btech
                    .vehicles()
                    .get(&id)
                    .is_some_and(|unit| unit.definition().is_vtol())
                {
                    let mut airborne = world.clone();
                    fact(&mut airborne, id, |state| {
                        state["vtol_flight"]["phase"] =
                            serde_json::to_value(BattleVtolFlightPhase::Airborne).unwrap();
                        state["vtol_flight"]["altitude"] = 5.25.into();
                    });
                    assert!(
                        !battle_seismic_contact(&airborne, observer, target, seismic)
                            .unwrap()
                            .eligible
                    );
                    assert!(
                        battle_electromagnetic_contact(&airborne, observer, target, em)
                            .unwrap()
                            .eligible
                    );
                }
            }
            let scripts = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
            )
            .unwrap();
            let state: (u8, bool) = scripts
                .eval_callback(&format!(
                    "local s=btech.unit.state({}); return s.sensor_signal,s.fired_recently",
                    target.0
                ))
                .unwrap();
            assert!(state.0 <= 100 && !state.1);
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn seismic_target_exclusions_and_fixed_sensor_ranges_follow_chassis_rules() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Long sensor lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "lane",
        BattleMapAsset::parse(&format!("1 31\n{}", ".0\n".repeat(31))).unwrap(),
    )
    .unwrap();
    let fixed = world.create(&config, "Fixed scanner".into(), Kind::Thing);
    let mut definition =
        BattleVehicleTemplate::parse(include_str!("../game/mechs/Demolisher")).unwrap();
    definition.movement = BattleVehicleMovement::Stationary;
    create_battle_vehicle(&mut world, fixed, definition).unwrap();
    let hover = world.create(&config, "Hovercraft".into(), Kind::Thing);
    create_battle_vehicle(
        &mut world,
        hover,
        BattleVehicleTemplate::parse(include_str!("../game/mechs/Savannah_Master")).unwrap(),
    )
    .unwrap();
    let mech = world.create(&config, "Ground target".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        mech,
        BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap(),
    )
    .unwrap();
    for (id, y) in [(fixed, 0), (hover, 2), (mech, 3)] {
        place_battle_unit(&mut world, id, map, 0, y).unwrap();
        fact(&mut world, id, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            state["sensor_signal"] =
                serde_json::to_value(BattleSensorSignal::seeded(100, [15; 32]).unwrap()).unwrap();
        });
    }
    let seismic = BattleSeismicRules {
        detect_stopped: true,
        signal_strength: 100,
        aim_adjustment: 0,
    };
    assert!(
        !battle_seismic_contact(&world, mech, fixed, seismic)
            .unwrap()
            .eligible
    );
    assert!(
        !battle_seismic_contact(&world, mech, hover, seismic)
            .unwrap()
            .eligible
    );
    // Hover and stationary observers can still sense a qualifying ground target.
    assert!(
        battle_seismic_contact(&world, fixed, mech, seismic)
            .unwrap()
            .eligible
    );
    assert!(
        battle_seismic_contact(&world, hover, mech, seismic)
            .unwrap()
            .eligible
    );
    for (distance, expected) in [(7, true), (10, true), (11, true), (12, false)] {
        let mut candidate = world.clone();
        fact(&mut candidate, mech, |state| {
            state["power"] = serde_json::to_value(BattlePower::Off).unwrap()
        });
        place_battle_unit(&mut candidate, mech, map, 0, distance).unwrap();
        fact(&mut candidate, mech, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap()
        });
        assert_eq!(
            battle_seismic_contact(&candidate, fixed, mech, seismic)
                .unwrap()
                .eligible,
            expected
        );
        assert_eq!(
            battle_seismic_contact(
                &candidate,
                fixed,
                mech,
                BattleSeismicRules {
                    signal_strength: 0,
                    ..seismic
                }
            )
            .unwrap()
            .eligible,
            distance <= 7
        );
    }
    for distance in [25, 26, 29, 30] {
        let mut candidate = world.clone();
        fact(&mut candidate, mech, |state| {
            state["power"] = serde_json::to_value(BattlePower::Off).unwrap()
        });
        place_battle_unit(&mut candidate, mech, map, 0, distance).unwrap();
        fact(&mut candidate, mech, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap()
        });
        fact(&mut candidate, fixed, |state| {
            state["sensor_selection"]["active"] = serde_json::to_value(BattleSensorPair {
                primary: BattleSensorMode::Electromagnetic,
                secondary: BattleSensorMode::Electromagnetic,
            })
            .unwrap()
        });
        let em = BattleElectromagneticRules {
            signal_strength: 100,
            aim_adjustment: 0,
        };
        assert_eq!(
            battle_electromagnetic_contact(&candidate, fixed, mech, em)
                .unwrap()
                .eligible,
            distance <= 29
        );
        assert_eq!(
            battle_electromagnetic_contact(
                &candidate,
                fixed,
                mech,
                BattleElectromagneticRules {
                    signal_strength: 0,
                    ..em
                }
            )
            .unwrap()
            .eligible,
            distance <= 25
        );
        assert_eq!(
            battle_hex_sensor_visibility(
                &candidate,
                fixed,
                BattleHexCoordinate {
                    x: 0,
                    y: distance as i32
                }
            )
            .unwrap()
            .primary,
            distance <= 29
        );
    }
    for distance in [15, 16, 21, 22] {
        let mut candidate = world.clone();
        fact(&mut candidate, mech, |state| {
            state["power"] = serde_json::to_value(BattlePower::Off).unwrap()
        });
        place_battle_unit(&mut candidate, mech, map, 0, distance).unwrap();
        for observer in [fixed, mech] {
            fact(&mut candidate, observer, |state| {
                state["sensor_selection"]["active"] = serde_json::to_value(BattleSensorPair {
                    primary: BattleSensorMode::Infrared,
                    secondary: BattleSensorMode::Infrared,
                })
                .unwrap()
            });
        }
        let conditions = BattleSensorConditions {
            light: BattleLight::Day,
            visibility: 60,
            target_lit: false,
            disabled: false,
        };
        for (observer, target, maximum) in [(fixed, mech, 21), (mech, fixed, 15)] {
            let report = battle_optical_contact(
                &candidate,
                observer,
                target,
                BattleSensorMode::Infrared,
                conditions,
            )
            .unwrap();
            assert_eq!(report.eligible, distance <= maximum);
            assert_eq!(
                battle_map_optical_contact(
                    &candidate,
                    observer,
                    target,
                    BattleSensorMode::Infrared,
                    false,
                    false
                )
                .unwrap()
                .eligible,
                report.eligible
            );
        }
        assert_eq!(
            battle_hex_sensor_visibility(
                &candidate,
                fixed,
                BattleHexCoordinate {
                    x: 0,
                    y: distance as i32
                }
            )
            .unwrap()
            .primary,
            distance <= 21
        );
    }
}
