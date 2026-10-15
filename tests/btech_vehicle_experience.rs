//! Shared classic gunnery awards and persisted unit policy across mixed construction types.
use crate::support;
use stompymux_rs::*;

/// Construct independent award participants without introducing firing or hit-location dice.
async fn fixture(
    vehicle_attacker: bool,
    vehicle_target: bool,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let source = |vehicle| {
        if vehicle {
            include_str!("../game/units/Demolisher.toml")
        } else {
            include_str!("fixtures/btech/units/JR7-D.toml")
        }
    };
    fixture_sources([source(vehicle_attacker), source(vehicle_target)]).await
}

/// Share character and eligibility setup across all admitted chassis, including rotorcraft.
async fn fixture_sources(
    sources: [&str; 2],
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let mut ids = Vec::new();
    for source in sources {
        let id = world.create(&config, "XP participant".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        crate::support::templates::fitted_unit(UnitTemplate::parse("test", source).unwrap())
            .create(&mut world, id)
            .unwrap();
        ids.push(id);
    }
    let [attacker, target] = [ids[0], ids[1]];
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(attacker);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(&mut world, attacker, ObjectId(1)).unwrap();
    set_battle_character(
        &mut world,
        ObjectId(1),
        Character {
            bruise: 0,
            lethal: 0,
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
        },
    )
    .unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    set_battle_unit_signature(
        &mut world,
        target,
        UnitSignature {
            team: 1,
            ..Default::default()
        },
    )
    .unwrap();
    (dir, config, world, attacker, target)
}

#[tokio::test]
async fn classic_awards_share_the_formula_for_every_attacker_target_pair() {
    for vehicle_attacker in [false, true] {
        for vehicle_target in [false, true] {
            let (_dir, config, mut world, attacker, target) =
                fixture(vehicle_attacker, vehicle_target).await;
            let policy = UnitExperience {
                multiplier: 2.0,
                suppress_gunnery: true,
            };
            set_battle_unit_experience(&mut world, attacker, policy).unwrap();
            set_battle_unit_experience(&mut world, target, policy).unwrap();
            let request = GunneryAwardRequest {
                tsm_tow_bonus: true,

                attacker,
                pilot: ObjectId(1),
                target,
                weapon: Weapon::MediumLaser,
                damage: 20,
                base_to_hit: 7,
                extended_gunnery: true,
                extended_piloting: true,
                use_unit_modifier: true,
                now: 100,
            };
            let expected = GunneryExperienceInput {
                attacker_tons: if vehicle_attacker { 80 } else { 35 },
                target_tons: if vehicle_target { 80 } else { 35 },
                attacker_speed: if vehicle_attacker {
                    world.btech.vehicles()[&attacker].maximum_speed()
                } else {
                    world.btech.constructed_units()[&attacker]
                        .definition()
                        .max_speed
                },
                target_speed: if vehicle_target {
                    world.btech.vehicles()[&target].maximum_speed()
                } else {
                    world.btech.constructed_units()[&target]
                        .definition()
                        .max_speed
                },
                base_to_hit: 7,
                damage: 20,
                unit_modifier: 2.0,
            }
            .classic_chance()
            .unwrap()
            .unwrap();
            let mut replay = world.clone();
            let report = award_battle_classic_gunnery_experience(&mut world, request)
                .unwrap()
                .unwrap();
            assert_eq!(report.chance, expected);
            assert!(report.award.unwrap().accepted);
            assert_eq!(
                award_battle_classic_gunnery_experience(&mut replay, request).unwrap(),
                Some(report)
            );
            assert_eq!(world.btech, replay.btech);
            let repeated = award_battle_classic_gunnery_experience(&mut world, request)
                .unwrap()
                .unwrap();
            // Gunnery skills are continuous and accept another award at the same timestamp.
            assert!(repeated.award.unwrap().accepted);
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
            let before = world.btech.clone();
            for multiplier in [-1.0, f64::INFINITY, f64::NAN] {
                assert!(
                    set_battle_unit_experience(
                        &mut world,
                        attacker,
                        UnitExperience {
                            multiplier,
                            ..Default::default()
                        }
                    )
                    .is_err()
                );
                assert_eq!(world.btech, before);
            }
            if vehicle_attacker {
                assert_eq!(
                    world.btech.vehicles()[&attacker].experience_settings(),
                    policy
                );
                let mut saved = serde_json::to_value(&world.btech).unwrap();
                saved["vehicles"][attacker.0.to_string()]["experience"]["multiplier"] =
                    serde_json::json!(-1.0);
                assert!(serde_json::from_value::<BtechState>(saved).is_err());
            }
            for case in [
                "friendly",
                "tactical",
                "absent",
                "disconnected",
                "sure",
                "self",
            ] {
                let mut rejected = world.clone();
                let mut request = request;
                match case {
                    "friendly" => {
                        set_battle_unit_signature(&mut rejected, target, UnitSignature::default())
                            .unwrap()
                    }
                    "tactical" => {
                        rejected
                            .objects
                            .get_mut(&target)
                            .unwrap()
                            .flags
                            .remove(Flag::InCharacter);
                    }
                    "absent" => {
                        rejected.objects.get_mut(&ObjectId(1)).unwrap().location = Some(target)
                    }
                    "disconnected" => {
                        rejected
                            .objects
                            .get_mut(&ObjectId(1))
                            .unwrap()
                            .flags
                            .remove(Flag::Connected);
                    }
                    "sure" => request.base_to_hit = 2,
                    "self" => request.target = attacker,
                    _ => unreachable!(),
                }
                let before = rejected.btech.clone();
                assert!(
                    award_battle_classic_gunnery_experience(&mut rejected, request)
                        .unwrap()
                        .is_none(),
                    "{case}"
                );
                assert_eq!(rejected.btech, before);
            }
        }
    }
}

#[tokio::test]
async fn classic_vehicle_difficulty_uses_current_motive_damage() {
    let (_dir, _config, mut world, attacker, target) = fixture(true, true).await;
    let request = GunneryAwardRequest {
        tsm_tow_bonus: true,

        attacker,
        pilot: ObjectId(1),
        target,
        weapon: Weapon::MediumLaser,
        damage: 6,
        base_to_hit: 7,
        extended_gunnery: true,
        extended_piloting: true,
        use_unit_modifier: true,
        now: 100,
    };
    let undamaged = award_battle_classic_gunnery_experience(&mut world.clone(), request)
        .unwrap()
        .unwrap();
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["immobilized"] = true.into();
        })
        .unwrap();
    let expected = GunneryExperienceInput {
        attacker_tons: 80,
        target_tons: 80,
        attacker_speed: world.btech.vehicles()[&attacker].maximum_speed(),
        target_speed: 0.0,
        base_to_hit: 7,
        damage: 6,
        unit_modifier: 1.0,
    }
    .classic_chance()
    .unwrap()
    .unwrap();
    let damaged = award_battle_classic_gunnery_experience(&mut world, request)
        .unwrap()
        .unwrap();
    assert_eq!(damaged.chance, expected);
    assert!(damaged.chance.difficulty < undamaged.chance.difficulty);
}

#[tokio::test]
async fn battle_value_awards_share_mixed_participants_and_preserve_dice() {
    for vehicle_attacker in [false, true] {
        for vehicle_target in [false, true] {
            let (_dir, config, mut world, attacker, target) =
                fixture(vehicle_attacker, vehicle_target).await;
            for skill in [
                "Gunnery-Laser",
                "Piloting-Biped",
                "Piloting-Battlemech",
                "Piloting-Tracked",
                "Drive",
            ] {
                set_battle_character_value(
                    &mut world,
                    ObjectId(1),
                    skill,
                    CharacterValue {
                        value: 5,
                        ..Default::default()
                    },
                )
                .unwrap();
            }
            let request = GunneryAwardRequest {
                tsm_tow_bonus: true,

                attacker,
                pilot: ObjectId(1),
                target,
                weapon: Weapon::MediumLaser,
                damage: 5,
                base_to_hit: 7,
                extended_gunnery: true,
                extended_piloting: true,
                use_unit_modifier: true,
                now: 100,
            };
            let xp = config::XpConfig {
                oldxpsystem: 0,
                ..Default::default()
            };
            let before = world.clone();
            let report = award_battle_gunnery_experience(&mut world, request, &xp)
                .unwrap()
                .unwrap();
            let ShotExperienceAward::BattleValue(report) = report else {
                panic!("wrong formula")
            };
            assert!(report.award.accepted);
            assert!(report.amount > 0);
            assert!(report.calculation.difficulty.is_some());
            let mut replay = before.clone();
            assert_eq!(
                award_battle_value_gunnery_experience(&mut replay, request, &xp).unwrap(),
                Some(report)
            );
            assert_eq!(replay.btech, world.btech);
            for (id, vehicle) in [(attacker, vehicle_attacker), (target, vehicle_target)] {
                let class = if vehicle { "vehicles" } else { "constructed" };
                assert_eq!(
                    serde_json::to_value(&world.btech).unwrap()[class][id.0.to_string()]["dice"],
                    serde_json::to_value(&before.btech).unwrap()[class][id.0.to_string()]["dice"]
                );
            }
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
            set_battle_unit_experience(
                &mut world,
                target,
                UnitExperience {
                    suppress_gunnery: true,
                    ..Default::default()
                },
            )
            .unwrap();
            let before = world.btech.clone();
            assert!(
                award_battle_gunnery_experience(&mut world, request, &xp)
                    .unwrap()
                    .is_none()
            );
            assert_eq!(world.btech, before);
        }
    }
}

#[tokio::test]
async fn vehicle_battle_value_uses_live_protection_and_installed_weapons() {
    let (_dir, _config, mut world, attacker, _target) = fixture(true, true).await;
    let value = world.btech.vehicles()[&attacker].battle_value().unwrap();
    // 160 armor and 40 structure, the tracked 0.9, then the five-MP +2 Defensive Factor.
    assert!((value.defensive - 496.8).abs() < 1e-9, "{value:?}");
    // Vehicles never halve for heat: both AC/20s, four tons of ammunition and half the
    // tonnage, at a five-MP Speed Factor of 1.00 (TechManual p. 306).
    let ac20 = f64::from(Weapon::parse("IS.AC/20").unwrap().battle_value());
    assert!(
        (value.offensive - (ac20 * 2.0 + 88.0 + 40.0)).abs() < 1e-9,
        "{value:?}"
    );
    world
        .btech
        .rewrite_unit_record(attacker, |record| {
            record["ammunition"] = serde_json::json!([0, 0, 0, 0]);
            record["sections"]["front"]["armor"] = 0.into();
        })
        .unwrap();
    let damaged = world.btech.vehicles()[&attacker].battle_value().unwrap();
    assert_eq!(damaged.offensive, value.offensive);
    assert!((damaged.defensive - 388.8).abs() < 1e-9, "{damaged:?}");
}

#[test]
fn vehicle_battle_value_applies_each_ground_movement_discount() {
    for (movement, defensive) in [
        (VehicleMovement::Tracked, 496.8),
        (VehicleMovement::Wheeled, 441.6),
        // The hovercraft is the 50-ton variant, with less structure than the 80-ton tank.
        (VehicleMovement::Hover, 367.5),
        (VehicleMovement::Stationary, 460.0),
    ] {
        let definition = crate::support::templates::demolisher(movement);
        let unit = Vehicle::new(definition).unwrap();
        let value = unit.battle_value().unwrap();
        assert!(
            (value.defensive - defensive).abs() < 1e-9,
            "{movement:?} {value:?}"
        );
    }
}

#[test]
fn shipped_vehicles_and_vtols_have_finite_battle_values() {
    let mut count = 0;
    let mut rotorcraft = 0;
    for entry in std::fs::read_dir(support::repository_root().join("game/units")).unwrap() {
        let path = entry.unwrap().path();
        if !path.is_file() {
            continue;
        }
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let Ok(definition) = VehicleTemplate::parse("test", &text) else {
            continue;
        };
        rotorcraft += usize::from(definition.is_vtol());
        let unit =
            Vehicle::new(definition).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        let value = unit
            .battle_value()
            .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
        assert!(value.total > 0, "{}", path.display());
        assert!(value.defensive >= 0.0 && value.offensive >= 0.0);
        count += 1;
    }
    assert!(count >= 235, "only {count} ground vehicles checked");
    assert_eq!(rotorcraft, 31);
}

/// Place XP participants and a shutdown tow load without firing or moving either carrier.
fn tow_fixture(
    world: &mut World,
    config: &Config,
    attacker: ObjectId,
    target: ObjectId,
    carrier: ObjectId,
) {
    let map = world.create(config, "Experience yard".into(), Kind::Room);
    create_battle_map(
        world,
        map,
        "yard",
        MapAsset::from_cells("2 2\n.0.0\n.0.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(world, map, support::FIXTURE_DICE_SEED);
    for id in [attacker, target] {
        place_battle_unit(world, id, map, 0, 0).unwrap();
    }
    assign_battle_pilot(world, attacker, ObjectId(1)).unwrap();
    support::seed_object_dice(world, ObjectId(1), support::FIXTURE_DICE_SEED);
    let load = world.create(config, "Tow load".into(), Kind::Thing);
    create_battle_vehicle(
        world,
        load,
        crate::support::templates::fitted(
            VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
                .unwrap(),
        ),
    )
    .unwrap();
    support::seed_object_dice(world, load, support::FIXTURE_DICE_SEED);
    place_battle_unit(world, load, map, 0, 0).unwrap();
    set_battle_tow(world, carrier, Some(load)).unwrap();
}

#[tokio::test]
async fn experience_and_battle_value_use_live_load_for_either_participant() {
    for vehicle_attacker in [false, true] {
        for vehicle_target in [false, true] {
            for load_attacker in [false, true] {
                let (_dir, config, mut world, attacker, target) =
                    fixture(vehicle_attacker, vehicle_target).await;
                let request = GunneryAwardRequest {
                    tsm_tow_bonus: true,

                    attacker,
                    pilot: ObjectId(1),
                    target,
                    weapon: Weapon::MediumLaser,
                    damage: 6,
                    base_to_hit: 7,
                    extended_gunnery: true,
                    extended_piloting: true,
                    use_unit_modifier: false,
                    now: 100,
                };
                let xp = config::XpConfig {
                    oldxpsystem: 0,
                    ..Default::default()
                };
                let classic_before =
                    award_battle_classic_gunnery_experience(&mut world.clone(), request)
                        .unwrap()
                        .unwrap();
                let bv_before =
                    award_battle_value_gunnery_experience(&mut world.clone(), request, &xp)
                        .unwrap()
                        .unwrap();
                let carrier = if load_attacker { attacker } else { target };
                let value_before = battle_unit_value(&world, carrier).unwrap();
                tow_fixture(&mut world, &config, attacker, target, carrier);
                let unchanged = world.btech.clone();
                let value = battle_unit_value(&world, carrier).unwrap();
                // Battle Value is a design value: carried load does not change it.
                assert_eq!(value, value_before);
                assert_eq!(world.btech, unchanged);
                let classic = award_battle_classic_gunnery_experience(&mut world.clone(), request)
                    .unwrap()
                    .unwrap();
                let bv = award_battle_value_gunnery_experience(&mut world.clone(), request, &xp)
                    .unwrap()
                    .unwrap();
                if load_attacker {
                    assert!(classic.chance.difficulty > classic_before.chance.difficulty);
                } else {
                    assert!(classic.chance.difficulty < classic_before.chance.difficulty);
                }
                assert_eq!(bv.calculation, bv_before.calculation);
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                // Connection state is transient; reconnect the pilot before comparing award eligibility.
                restored
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
                for classic_mode in [false, true] {
                    let settings = config::XpConfig {
                        oldxpsystem: i64::from(classic_mode),
                        ..xp.clone()
                    };
                    let mut live = world.clone();
                    let mut replay = restored.clone();
                    assert_eq!(
                        award_battle_gunnery_experience(&mut live, request, &settings).unwrap(),
                        award_battle_gunnery_experience(&mut replay, request, &settings).unwrap()
                    );
                    assert_eq!(live.btech, replay.btech);
                    live.validate(&config).unwrap();
                }
            }
        }
    }
}

#[tokio::test]
async fn experience_load_queries_honor_hot_myomer_configuration() {
    let (_dir, config, mut world, attacker, target) = fixture(false, true).await;
    let mut definition = world.btech.constructed_units()[&attacker]
        .definition()
        .clone();
    let mut remaining = 6;
    for section in [MechSection::LeftTorso, MechSection::RightTorso] {
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
                },
            );
            remaining -= 1;
        }
    }
    assert_eq!(remaining, 0);
    world
        .btech
        .rewrite_unit_record(attacker, |record| {
            crate::support::templates::set_record_definition(record, definition);
            record["heat"]["excess"] = 9.0.into();
        })
        .unwrap();
    tow_fixture(&mut world, &config, attacker, target, attacker);
    // Use a Mech-sized load so enabled assistance crosses an XP movement band.
    set_battle_tow(&mut world, attacker, None).unwrap();
    let map = world.btech.constructed_units()[&attacker]
        .position()
        .unwrap()
        .map;
    let lighter = world.create(&config, "Myomer load".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        lighter,
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, lighter, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, lighter, map, 0, 0).unwrap();
    set_battle_tow(&mut world, attacker, Some(lighter)).unwrap();
    let request = GunneryAwardRequest {
        tsm_tow_bonus: false,

        attacker,
        pilot: ObjectId(1),
        target,
        weapon: Weapon::MediumLaser,
        damage: 6,
        base_to_hit: 7,
        extended_gunnery: true,
        extended_piloting: true,
        use_unit_modifier: false,
        now: 100,
    };
    let disabled = award_battle_classic_gunnery_experience(&mut world.clone(), request)
        .unwrap()
        .unwrap();
    let enabled = award_battle_classic_gunnery_experience(
        &mut world.clone(),
        GunneryAwardRequest {
            tsm_tow_bonus: true,

            ..request
        },
    )
    .unwrap()
    .unwrap();
    assert!(enabled.chance.difficulty < disabled.chance.difficulty);
    let xp = config::XpConfig {
        oldxpsystem: 0,
        ..Default::default()
    };
    let disabled = award_battle_value_gunnery_experience(&mut world.clone(), request, &xp)
        .unwrap()
        .unwrap();
    let enabled = award_battle_value_gunnery_experience(
        &mut world.clone(),
        GunneryAwardRequest {
            tsm_tow_bonus: true,

            ..request
        },
        &xp,
    )
    .unwrap()
    .unwrap();
    // The towing assistance setting changes the classic award's speed but not Battle Value.
    assert_eq!(enabled.calculation, disabled.calculation);
}

#[tokio::test]
async fn vtol_value_shares_vehicle_accounting_and_has_its_class_movement_bonus() {
    let (_dir, config, mut world, id, _) =
        fixture_sources([include_str!("../game/units/Kestrel.toml"); 2]).await;
    let before = world.btech.clone();
    let initial = battle_unit_value(&world, id).unwrap();
    // 24 armor and 15 structure: 82.5, times the VTOL 0.7, then 18 MP (+5) plus VTOL (+1).
    assert!((initial.defensive - 92.4).abs() < 1e-9, "{initial:?}");
    // Two machine guns, a half-ton lot of ammunition and half of 25 tons at 18 MP's 2.72
    // Speed Factor.
    let machine_guns = 2.0 * f64::from(Weapon::MachineGun.battle_value());
    assert!(
        (initial.offensive - (machine_guns + 0.5 + 12.5) * 2.72).abs() < 1e-9,
        "{initial:?}"
    );
    assert_eq!(world.btech, before);
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ammunition"] = serde_json::json!([0]);
        })
        .unwrap();
    assert_eq!(battle_unit_value(&world, id).unwrap(), initial);
    // A surviving but unarmored rotor changes only protection, not installed weapons or movement.
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["sections"]["rotor"]["armor"] = 0.into();
        })
        .unwrap();
    assert!((battle_unit_value(&world, id).unwrap().defensive - 86.8).abs() < 1e-9);
    // Destroying the rotor removes three internal points; movement keeps its design value.
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["sections"]["rotor"]["internal"] = 0.into();
        })
        .unwrap();
    let stopped = battle_unit_value(&world, id).unwrap();
    assert_eq!(stopped.offensive, initial.offensive);
    assert!((stopped.defensive - 81.76).abs() < 1e-9, "{stopped:?}");
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(battle_unit_value(&restored, id).unwrap(), stopped);
}

#[tokio::test]
async fn vtol_battle_value_experience_supports_mixed_pairs_load_and_replay() {
    let chassis = [
        include_str!("fixtures/btech/units/JR7-D.toml"),
        include_str!("../game/units/Demolisher.toml"),
        include_str!("../game/units/Kestrel.toml"),
    ];
    for (a, attacker_source) in chassis.iter().enumerate() {
        for (t, target_source) in chassis.iter().enumerate() {
            if a != 2 && t != 2 {
                continue;
            }
            let (_dir, config, mut world, attacker, target) =
                fixture_sources([attacker_source, target_source]).await;
            let request = GunneryAwardRequest {
                tsm_tow_bonus: true,

                attacker,
                pilot: ObjectId(1),
                target,
                weapon: Weapon::MachineGun,
                damage: 2,
                base_to_hit: 7,
                extended_gunnery: true,
                extended_piloting: true,
                use_unit_modifier: false,
                now: 100,
            };
            let xp = config::XpConfig {
                oldxpsystem: 0,
                ..Default::default()
            };
            let unloaded = award_battle_gunnery_experience(&mut world.clone(), request, &xp)
                .unwrap()
                .unwrap();
            assert!(matches!(unloaded, ShotExperienceAward::BattleValue(_)));

            let carrier = if a == 2 { attacker } else { target };
            let unloaded_value = battle_unit_value(&world, carrier).unwrap();
            tow_fixture(&mut world, &config, attacker, target, carrier);
            // Battle Value is a design value, so towing changes neither it nor the award.
            assert_eq!(battle_unit_value(&world, carrier).unwrap(), unloaded_value);
            let live = award_battle_gunnery_experience(&mut world.clone(), request, &xp)
                .unwrap()
                .unwrap();
            assert_eq!(live, unloaded);
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            replay
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .insert(Flag::Connected);
            assert_eq!(
                award_battle_gunnery_experience(&mut replay, request, &xp).unwrap(),
                Some(live)
            );
            replay.validate(&config).unwrap();
        }
    }
}
