//! Advanced rotor criticals use the shared critical transaction while live aircraft remain gated.
use crate::support;
use stompymux_rs::*;

#[tokio::test]
async fn advanced_rotor_criticals_commit_saved_dice_and_material_effects_together() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Rotor critical fixture".into(), Kind::Thing);
    // Install material directly for the critical transaction; this does not admit a live VTOL.
    let aircraft = BattleVehicle::new(
        BattleVehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml"))
            .unwrap(),
    )
    .unwrap();
    let rules = BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        table: BattleVehicleCriticalTable::Advanced,
        enabled: true,
        combat_safe: false,
        toughness: false,
    };
    for roll in 2..=12 {
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
            .unwrap();
        let mut saved = serde_json::to_value(&aircraft).unwrap();
        saved["position"] = serde_json::json!({"map":0,"x":0,"y":0});
        saved["map_slot"] = 0.into();
        saved["power"] = serde_json::to_value(BattlePower::Running).unwrap();
        saved["motion"] = serde_json::to_value(BattleMotion::stationary(
            HexCoordinate { x: 0, y: 0 }.center(),
        ))
        .unwrap();
        saved["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
            fall: None,
            phase: BattleVtolFlightPhase::Airborne,
            vertical_speed: 60.0,
            altitude: 10.0,
        })
        .unwrap();

        saved["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["vehicles"][id.0.to_string()] = saved;
        world.btech = serde_json::from_value(state).unwrap();
        let mut replay = world.clone();
        let report =
            resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Rotor, rules)
                .unwrap();
        assert_eq!(report.selection.rolls, [roll]);
        assert_eq!(
            report.selection.effect,
            BattleRotorHit::from_critical_roll(roll)
                .unwrap()
                .map(BattleVehicleCriticalEffect::Rotor)
        );
        assert_eq!(
            resolve_battle_vehicle_critical(&mut replay, id, BattleVehicleSection::Rotor, rules)
                .unwrap(),
            report
        );
        assert_eq!(world.btech, replay.btech);
        let unit = &world.btech.vehicles()[&id];
        assert_eq!(
            unit.maximum_speed(),
            if roll >= 11 {
                0.0
            } else if (6..=8).contains(&roll) {
                182.75
            } else {
                193.5
            }
        );
        assert_eq!(unit.tail_rotor_destroyed(), matches!(roll, 9 | 10));
        assert_eq!(unit.rotor_destroyed(), roll >= 11);
        assert!(!unit.is_destroyed() && !unit.crew_killed());
        assert_eq!(unit.vtol_flight().unwrap().altitude, 10.0);
        assert_eq!(
            unit.vtol_flight().unwrap().phase,
            if roll >= 11 {
                BattleVtolFlightPhase::Falling
            } else {
                BattleVtolFlightPhase::Airborne
            }
        );

        assert_eq!(
            serde_json::from_value::<BattleVehicle>(serde_json::to_value(unit).unwrap()).unwrap(),
            *unit
        );
        let mut dice = BattleDice::seeded([seed; 32]);
        dice.two_d6();
        assert_eq!(
            serde_json::to_value(unit).unwrap()["dice"],
            serde_json::to_value(dice).unwrap()
        );
        if roll >= 11 {
            let before = world.btech.clone();
            assert!(
                resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Rotor, rules)
                    .is_err()
            );
            assert_eq!(world.btech, before);
        }
    }
}

#[tokio::test]
async fn suppressed_rotor_criticals_do_not_draw_dice_or_change_material() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Protected rotor".into(), Kind::Thing);
    for suppression in ["disabled", "safe", "proof"] {
        let source = if suppression == "proof" {
            include_str!("../game/mechs/Kestrel.toml")
                .replace("\"CargoTech\"", "\"CargoTech\", \"CritProof_Tech\"")
        } else {
            include_str!("../game/mechs/Kestrel.toml").into()
        };
        let aircraft =
            BattleVehicle::new(BattleVehicleTemplate::parse("Kestrel", &source).unwrap()).unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["vehicles"][id.0.to_string()] = serde_json::to_value(aircraft).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        let before = world.btech.clone();
        let report = resolve_battle_vehicle_critical(
            &mut world,
            id,
            BattleVehicleSection::Rotor,
            BattleVehicleCriticalRules {
                rotor_damage_divisor: 0,
                extended_piloting: false,
                vtol_table: None,
                table: BattleVehicleCriticalTable::Advanced,
                enabled: suppression != "disabled",
                combat_safe: suppression == "safe",
                toughness: false,
            },
        )
        .unwrap();
        assert!(report.selection.rolls.is_empty());
        assert!(report.selection.effect.is_none());
        assert_eq!(world.btech, before);
    }
}

#[tokio::test]
async fn stationary_observation_aircraft_still_use_the_rotor_critical_table() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Observation rotor".into(), Kind::Thing);
    let unit = BattleVehicle::new(
        BattleVehicleTemplate::parse(
            "ObservationVTOL",
            include_str!("../game/mechs/ObservationVTOL.toml"),
        )
        .unwrap(),
    )
    .unwrap();
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 9)
        .unwrap();
    let mut saved = serde_json::to_value(unit).unwrap();
    saved["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()] = saved;
    world.btech = serde_json::from_value(state).unwrap();
    let report = resolve_battle_vehicle_critical(
        &mut world,
        id,
        BattleVehicleSection::Rotor,
        BattleVehicleCriticalRules {
            rotor_damage_divisor: 0,
            extended_piloting: false,
            vtol_table: None,
            table: BattleVehicleCriticalTable::Advanced,
            enabled: true,
            combat_safe: false,
            toughness: false,
        },
    )
    .unwrap();
    assert_eq!(
        report.selection.effect,
        Some(BattleVehicleCriticalEffect::Rotor(
            BattleRotorHit::TailRotor
        ))
    );
    assert!(world.btech.vehicles()[&id].tail_rotor_destroyed());
}

#[tokio::test]
async fn advanced_aircraft_hull_rows_share_selection_dice_and_control_effects() {
    use BattleVehicleCriticalEffect as E;
    use BattleVehicleSection as S;
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Hull critical fixture".into(), Kind::Thing);
    let rules = BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        table: BattleVehicleCriticalTable::Advanced,
        enabled: true,
        combat_safe: false,
        toughness: false,
    };
    let rows = [
        (
            S::Front,
            [
                E::VtolCopilot,
                E::WeaponJam,
                E::Stabilizer,
                E::Sensors,
                E::VtolPilot,
                E::WeaponDestroyed,
                E::CrewKilled,
            ],
        ),
        (
            S::Left,
            [
                E::WeaponJam,
                E::Cargo,
                E::Stabilizer,
                E::WeaponDestroyed,
                E::Engine,
                E::Ammunition,
                E::FuelTank,
            ],
        ),
        (
            S::Right,
            [
                E::WeaponJam,
                E::Cargo,
                E::Stabilizer,
                E::WeaponDestroyed,
                E::Engine,
                E::Ammunition,
                E::FuelTank,
            ],
        ),
        (
            S::Rear,
            [
                E::Cargo,
                E::WeaponJam,
                E::Stabilizer,
                E::WeaponDestroyed,
                E::Sensors,
                E::Engine,
                E::FuelTank,
            ],
        ),
    ];
    for source in [
        include_str!("../game/mechs/Kestrel.toml"),
        include_str!("../game/mechs/ObservationVTOL.toml"),
    ] {
        let unit =
            BattleVehicle::new(BattleVehicleTemplate::parse("test", source).unwrap()).unwrap();
        for (section, row) in rows {
            for roll in 2..=12 {
                let seed = (0..=255)
                    .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
                    .unwrap();
                let mut saved = serde_json::to_value(&unit).unwrap();
                saved["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["vehicles"][id.0.to_string()] = saved;
                world.btech = serde_json::from_value(state).unwrap();
                let base = world.clone();
                let report = roll_battle_vehicle_critical(&mut world, id, section, rules).unwrap();
                assert_eq!(report.rolls, [roll]);
                assert_eq!(
                    report.effect,
                    if roll < 6 {
                        None
                    } else {
                        Some(row[usize::from(roll - 6)])
                    }
                );
                if matches!(report.effect, Some(E::VtolPilot | E::VtolCopilot)) {
                    let mut resolved = base.clone();
                    let mut replay = base;
                    let result =
                        resolve_battle_vehicle_critical(&mut resolved, id, section, rules).unwrap();
                    assert_eq!(result.selection, report);
                    assert_eq!(
                        resolve_battle_vehicle_critical(&mut replay, id, section, rules).unwrap(),
                        result
                    );
                    assert_eq!(replay.btech, resolved.btech);
                    let changed = &resolved.btech.vehicles()[&id];
                    assert_eq!(changed.piloting_damage(), if roll == 10 { 2 } else { 0 });
                    assert_eq!(changed.gunnery_damage(), if roll == 6 { 1 } else { 0 });
                    assert_eq!(changed.pilot_injuries(), 0);
                    assert_eq!(changed.crew_stun_remaining(), 0);
                    assert_eq!(
                        serde_json::from_value::<BattleVehicle>(
                            serde_json::to_value(changed).unwrap()
                        )
                        .unwrap(),
                        *changed
                    );
                }
            }
        }
    }
}

#[tokio::test]
async fn airborne_engine_critical_requires_emergency_resolution_without_partial_changes() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Airborne engine".into(), Kind::Thing);
    let unit = BattleVehicle::new(
        BattleVehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml"))
            .unwrap(),
    )
    .unwrap();
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 10)
        .unwrap();
    let mut saved = serde_json::to_value(unit).unwrap();
    saved["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    saved["position"] = serde_json::json!({"map":0,"x":0,"y":0});
    saved["map_slot"] = 0.into();
    saved["power"] = serde_json::to_value(BattlePower::Running).unwrap();
    saved["motion"] = serde_json::to_value(BattleMotion::stationary(
        HexCoordinate { x: 0, y: 0 }.center(),
    ))
    .unwrap();
    saved["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
        fall: None,
        phase: BattleVtolFlightPhase::Airborne,
        altitude: 5.0,
        vertical_speed: 0.0,
    })
    .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()] = saved;
    world.btech = serde_json::from_value(state).unwrap();
    let before = world.btech.clone();
    let error = resolve_battle_vehicle_critical(
        &mut world,
        id,
        BattleVehicleSection::Left,
        BattleVehicleCriticalRules {
            rotor_damage_divisor: 0,
            extended_piloting: false,
            vtol_table: None,
            table: BattleVehicleCriticalTable::Advanced,
            enabled: true,
            combat_safe: false,
            toughness: false,
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("emergency landing"));
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn standard_vtol_criticals_share_common_effects_without_ground_preliminary_rolls() {
    use BattleVehicleCriticalEffect as E;
    let (_dir, config, mut world) = support::isolated_world().await;
    let id = world.create(&config, "Standard aircraft criticals".into(), Kind::Thing);
    for source in [
        include_str!("../game/mechs/Kestrel.toml"),
        include_str!("../game/mechs/ObservationVTOL.toml"),
    ] {
        let aircraft =
            BattleVehicle::new(BattleVehicleTemplate::parse("test", source).unwrap()).unwrap();
        for table in [BattleVehicleCriticalTable::Standard] {
            for (index, effect) in [
                E::CrewKilled,
                E::MainWeaponJam,
                E::Engine,
                E::CrewKilled,
                E::FuelTank,
                E::PowerPlant,
            ]
            .into_iter()
            .enumerate()
            {
                let roll = index as u8 + 1;
                let seed = (0..=255)
                    .find(|seed| BattleDice::seeded([*seed; 32]).d6() == roll)
                    .unwrap();
                let mut saved = serde_json::to_value(&aircraft).unwrap();
                saved["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["vehicles"][id.0.to_string()] = saved;
                world.btech = serde_json::from_value(state).unwrap();
                let mut replay = world.clone();
                let rules = BattleVehicleCriticalRules {
                    rotor_damage_divisor: 0,
                    extended_piloting: false,
                    vtol_table: None,
                    table,
                    enabled: true,
                    combat_safe: false,
                    toughness: false,
                };
                let report = resolve_battle_vehicle_critical(
                    &mut world,
                    id,
                    BattleVehicleSection::Front,
                    rules,
                )
                .unwrap();
                assert_eq!(report.selection.rolls, [roll]);
                assert_eq!(report.selection.effect, Some(effect));
                assert_eq!(report.selection.table, BattleVehicleCriticalTable::Standard);
                assert_eq!(
                    resolve_battle_vehicle_critical(
                        &mut replay,
                        id,
                        BattleVehicleSection::Front,
                        rules
                    )
                    .unwrap(),
                    report
                );
                assert_eq!(world.btech, replay.btech);
                let unit = &world.btech.vehicles()[&id];
                assert_eq!(unit.crew_killed(), matches!(roll, 1 | 4 | 5 | 6));
                if roll == 3 {
                    assert_eq!(unit.maximum_speed(), 0.0);
                }
                if matches!(roll, 5 | 6) {
                    assert!(report.explosion.is_some());
                }
                assert_eq!(
                    serde_json::from_value::<BattleVehicle>(serde_json::to_value(unit).unwrap())
                        .unwrap(),
                    *unit
                );
            }
        }
    }
}

#[tokio::test]
async fn aircraft_explosions_settle_at_surface_and_share_case_containment_atomically() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Explosion field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "explosion",
        MapAsset::from_cells("1 1\n.3\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, "Exploding aircraft".into(), Kind::Thing);
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).d6() == 6)
        .unwrap();
    let rules = BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        table: BattleVehicleCriticalTable::Standard,
        enabled: true,
        combat_safe: false,
        toughness: false,
    };
    for case in [false, true] {
        let source = if case {
            include_str!("../game/mechs/Kestrel.toml").replace(
                "[sections.aft_side]\n",
                "[sections.aft_side]\nslots = [{ at = 1, item = \"CASE\" }]\n",
            )
        } else {
            include_str!("../game/mechs/Kestrel.toml").into()
        };
        let unit =
            BattleVehicle::new(BattleVehicleTemplate::parse("Kestrel", &source).unwrap()).unwrap();
        assert_eq!(unit.has_powerplant_containment(), case);
        let mut saved = serde_json::to_value(unit).unwrap();
        saved["position"] = serde_json::json!({"map":map.0,"x":0,"y":0});
        saved["map_slot"] = 0.into();
        saved["power"] = serde_json::to_value(BattlePower::Running).unwrap();
        saved["motion"] = serde_json::to_value(BattleMotion::stationary(
            HexCoordinate { x: 0, y: 0 }.center(),
        ))
        .unwrap();
        saved["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
            fall: None,
            phase: BattleVtolFlightPhase::Airborne,
            altitude: 20.0,
            vertical_speed: 60.0,
        })
        .unwrap();
        saved["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["vehicles"][id.0.to_string()] = saved.clone();
        world.btech = serde_json::from_value(state.clone()).unwrap();
        let mut replay = world.clone();
        let report =
            resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Front, rules)
                .unwrap();
        assert_eq!(report.explosion.as_ref().unwrap().contained, case);
        if case {
            assert_eq!(
                report.explosion.as_ref().unwrap().destroyed_sections,
                [BattleVehicleSection::Rear]
            );
        }
        let unit = &world.btech.vehicles()[&id];
        assert_eq!(unit.vtol_flight().unwrap().altitude, 3.0);
        assert_eq!(unit.vtol_flight().unwrap().vertical_speed, 0.0);
        assert_eq!(
            resolve_battle_vehicle_critical(&mut replay, id, BattleVehicleSection::Front, rules)
                .unwrap(),
            report
        );
        assert_eq!(replay.btech, world.btech);
        assert_eq!(
            serde_json::from_value::<BattleVehicle>(serde_json::to_value(unit).unwrap()).unwrap(),
            *unit
        );
        saved["position"]["map"] = 999999.into();
        state["vehicles"][id.0.to_string()] = saved;
        world.btech = serde_json::from_value(state).unwrap();
        let before = world.btech.clone();
        assert!(
            resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Front, rules)
                .is_err()
        );
        assert_eq!(world.btech, before);
    }
}

#[tokio::test]
async fn engine_emergency_landings_use_shared_checks_and_commit_failed_attempts_as_falls() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Emergency field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "emergency",
        MapAsset::from_cells("1 1\n.3\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, "Emergency aircraft".into(), Kind::Thing);
    let unit = BattleVehicle::new(
        BattleVehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml"))
            .unwrap(),
    )
    .unwrap();
    for advanced in [false, true] {
        for success in [false, true] {
            let seed = (0..=255)
                .find(|seed| {
                    let mut dice = BattleDice::seeded([*seed; 32]);
                    let engine = if advanced {
                        dice.two_d6() == 10
                    } else {
                        dice.d6() == 3
                    };
                    engine && (dice.two_d6() >= 8) == success
                })
                .unwrap();
            let mut saved = serde_json::to_value(&unit).unwrap();
            saved["position"] = serde_json::json!({"map":map.0,"x":0,"y":0});
            saved["map_slot"] = 0.into();
            saved["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            saved["motion"] = serde_json::to_value(BattleMotion::stationary(
                HexCoordinate { x: 0, y: 0 }.center(),
            ))
            .unwrap();
            saved["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
                fall: None,
                phase: BattleVtolFlightPhase::Airborne,
                altitude: 5.0,
                vertical_speed: 60.0,
            })
            .unwrap();
            saved["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["vehicles"][id.0.to_string()] = saved;
            world.btech = serde_json::from_value(state).unwrap();
            let before = world.btech.clone();
            let mut replay = world.clone();
            let rules = BattleVehicleCriticalRules {
                rotor_damage_divisor: 0,
                extended_piloting: true,
                vtol_table: None,
                table: if advanced {
                    BattleVehicleCriticalTable::Advanced
                } else {
                    BattleVehicleCriticalTable::Standard
                },
                enabled: true,
                combat_safe: false,
                toughness: false,
            };
            let report =
                resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Left, rules);
            if !success {
                let report = report.unwrap();
                assert!(!report.emergency_landing.as_ref().unwrap().success);
                assert_ne!(world.btech, before);
                assert_eq!(
                    resolve_battle_vehicle_critical(
                        &mut replay,
                        id,
                        BattleVehicleSection::Left,
                        rules
                    )
                    .unwrap(),
                    report
                );
                assert_eq!(replay.btech, world.btech);
                let changed = &world.btech.vehicles()[&id];
                assert_eq!(changed.maximum_speed(), 0.0);
                assert_eq!(
                    changed.vtol_flight().unwrap().phase,
                    BattleVtolFlightPhase::Falling
                );
                assert_eq!(changed.vtol_flight().unwrap().altitude, 5.0);
                assert!(changed.vtol_flight().unwrap().fall.is_some());
                assert_eq!(
                    serde_json::from_value::<BattleVehicle>(serde_json::to_value(changed).unwrap())
                        .unwrap(),
                    *changed
                );
                world
                    .btech
                    .set_unit_dice(id, BattleDice::seeded([seed; 32]))
                    .unwrap();
                let cursor = world.btech.vehicles()[&id].vtol_flight().unwrap().fall;
                let repeated = resolve_battle_vehicle_critical(
                    &mut world,
                    id,
                    BattleVehicleSection::Left,
                    rules,
                )
                .unwrap();
                assert!(repeated.emergency_landing.is_none());
                assert_eq!(
                    world.btech.vehicles()[&id].vtol_flight().unwrap().fall,
                    cursor
                );
                continue;
            }
            let report = report.unwrap();
            let check = report.emergency_landing.as_ref().unwrap();
            assert!(check.success);
            assert_eq!(check.skill, 6);
            assert_eq!(check.situational, 2);
            assert_eq!(check.target, 8);
            assert_eq!(
                resolve_battle_vehicle_critical(&mut replay, id, BattleVehicleSection::Left, rules)
                    .unwrap(),
                report
            );
            assert_eq!(replay.btech, world.btech);
            let changed = &world.btech.vehicles()[&id];
            assert_eq!(changed.maximum_speed(), 0.0);
            assert_eq!(
                changed.vtol_flight().unwrap(),
                BattleVtolFlight {
                    fall: None,
                    phase: BattleVtolFlightPhase::Landed,
                    altitude: 3.0,
                    vertical_speed: 0.0
                }
            );
            assert_eq!(changed.sections(), unit.sections());
            assert!(!changed.is_destroyed());
            assert_eq!(
                serde_json::from_value::<BattleVehicle>(serde_json::to_value(changed).unwrap())
                    .unwrap(),
                *changed
            );
        }
    }
}

#[tokio::test]
async fn engine_loss_over_water_starts_falling_without_a_landing_roll() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Water emergency".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "water",
        MapAsset::from_cells("1 1\n~3\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, "Aircraft".into(), Kind::Thing);
    let unit = BattleVehicle::new(
        BattleVehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml"))
            .unwrap(),
    )
    .unwrap();
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).d6() == 3)
        .unwrap();
    let mut saved = serde_json::to_value(unit).unwrap();
    saved["position"] = serde_json::json!({"map":map.0,"x":0,"y":0});
    saved["map_slot"] = 0.into();
    saved["power"] = serde_json::to_value(BattlePower::Running).unwrap();
    saved["motion"] = serde_json::to_value(BattleMotion::stationary(
        HexCoordinate { x: 0, y: 0 }.center(),
    ))
    .unwrap();
    saved["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
        fall: None,
        phase: BattleVtolFlightPhase::Airborne,
        altitude: 5.0,
        vertical_speed: 60.0,
    })
    .unwrap();
    saved["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()] = saved;
    world.btech = serde_json::from_value(state).unwrap();
    let report = resolve_battle_vehicle_critical(
        &mut world,
        id,
        BattleVehicleSection::Front,
        BattleVehicleCriticalRules {
            rotor_damage_divisor: 0,
            extended_piloting: false,
            vtol_table: None,
            table: BattleVehicleCriticalTable::Standard,
            enabled: true,
            combat_safe: false,
            toughness: false,
        },
    )
    .unwrap();
    assert!(report.emergency_landing.is_none());
    let unit = &world.btech.vehicles()[&id];
    assert_eq!(
        unit.vtol_flight().unwrap().phase,
        BattleVtolFlightPhase::Falling
    );
    let mut dice = BattleDice::seeded([seed; 32]);
    dice.d6();
    assert_eq!(
        serde_json::to_value(unit).unwrap()["dice"],
        serde_json::to_value(dice).unwrap()
    );
}

/// Emergency landing rolls reach only the assigned pilot before the landing outcome.
#[tokio::test]
async fn emergency_landing_feedback_is_private_and_replayable() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Emergency landing".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, "Aircraft".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml"))
            .unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    let passenger = world.create(&config, "Passenger".into(), Kind::Player);
    world.objects.get_mut(&passenger).unwrap().location = Some(id);
    world
        .objects
        .get_mut(&passenger)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    set_battle_character(
        &mut world,
        ObjectId(1),
        BattleCharacter {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Piloting-Aerospace",
        BattleCharacterValue {
            value: 2,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    let target = battle_unit_piloting_target(&world, id, true).unwrap() + 2;
    for success in [false, true] {
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = BattleDice::seeded([*seed; 32]);
                dice.d6() == 3 && (i16::from(dice.two_d6()) >= target) == success
            })
            .unwrap();
        let mut candidate = world.clone();
        candidate
            .btech
            .rewrite_unit_record(id, |record| {
                let aircraft = record;
                aircraft["power"] = serde_json::to_value(BattlePower::Running).unwrap();
                aircraft["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
                    fall: None,
                    phase: BattleVtolFlightPhase::Airborne,
                    altitude: 2.0,
                    vertical_speed: 0.0,
                })
                .unwrap();
                aircraft["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
            })
            .unwrap();
        persistence::save(&config.database(), &candidate)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for who in [ObjectId(1), passenger] {
            restored
                .objects
                .get_mut(&who)
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(candidate)),
        )
        .unwrap();
        let replay =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        let rules = BattleVehicleCriticalRules {
            extended_piloting: true,
            table: BattleVehicleCriticalTable::Standard,
            rotor_damage_divisor: 0,
            vtol_table: None,
            enabled: true,
            combat_safe: false,
            toughness: false,
        };
        let report = resolve_battle_vehicle_critical_action(
            &scripts,
            &config,
            id,
            BattleVehicleSection::Left,
            rules,
        )
        .unwrap();
        assert_eq!(report.emergency_landing.as_ref().unwrap().success, success);
        assert_eq!(report.pilot_notices.len(), 2);
        let output = scripts.drain_outbox();
        let messages: Vec<_> = output
            .iter()
            .filter(|(who, _)| *who == ObjectId(1))
            .map(|(_, message)| message.source())
            .collect();
        let index = messages
            .iter()
            .position(|message| *message == "You make a piloting skill roll!")
            .unwrap();
        assert_eq!(messages[index - 1], "Your engine takes a direct hit!");
        assert!(messages[index + 1].starts_with("Modified Pilot Skill:"));
        assert_eq!(
            messages[index + 2],
            if success {
                "You land safely!"
            } else {
                "You lose lift and start to fall!"
            }
        );
        assert!(output.iter().any(|(who, _)| *who == passenger));
        assert!(!output.iter().any(|(who, message)| *who == passenger
            && (message.source().starts_with("Modified Pilot Skill:")
                || message.source() == "You make a piloting skill roll!")));
        let repeated = resolve_battle_vehicle_critical_action(
            &replay,
            &config,
            id,
            BattleVehicleSection::Left,
            rules,
        )
        .unwrap();
        assert_eq!(report, repeated);
        assert_eq!(scripts.world().btech, replay.world().btech);
        assert_eq!(output, replay.drain_outbox());
    }
}
