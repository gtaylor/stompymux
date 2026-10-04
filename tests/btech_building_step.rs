//! Surface entry notices use one rule for every mobile ground chassis and durable replay.
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Ordinary structures report once per entered hex; concealed and dropship structures stay quiet.
#[tokio::test]
async fn ground_entry_reports_live_cf_once_and_replays_after_restart() {
    for source in firing::templates().into_iter().take(5) {
        for (flags, perception, integrity) in [
            (0, None, 7),
            (0, None, 0),
            (4, None, 7),
            (16, None, 7),
            (16, Some(-10), 7),
            (16, Some(18), 7),
            (4, Some(-10), 7),
        ] {
            let (_dir, config, mut world, id, _, _) =
                firing::fixture_with_target(&source, None, &source).await;
            let map = if let Some(unit) = world.btech.vehicles().get(&id) {
                unit.position().unwrap().map
            } else {
                world.btech.constructed_units()[&id].position().unwrap().map
            };
            let interior = world.create(&config, "Field workshop".into(), Kind::Room);
            create_battle_map(
                &mut world,
                interior,
                "interior",
                BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
            )
            .unwrap();
            crate::support::seed_object_dice(
                &mut world,
                interior,
                crate::support::FIXTURE_DICE_SEED,
            );
            set_building_state(
                &mut world,
                interior,
                BattleBuildingState {
                    integrity,
                    maximum_integrity: 10,
                    flags,
                    regeneration: 1,
                },
            )
            .unwrap();
            // Duplicate entrance records still describe a single building in this hex.
            for ordinal in [0, 1] {
                set_building_entrance(
                    &mut world,
                    map,
                    ordinal,
                    Some(BattleBuildingEntrance {
                        coordinate: BattleHexCoordinate { x: 0, y: 10 },
                        interior,
                        data_char: 0,
                        data_short: 0,
                        data_int: 0,
                    }),
                )
                .unwrap();
            }
            if let Some(target) = perception {
                world
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
                world
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
                set_battle_character(
                    &mut world,
                    ObjectId(1),
                    BattleCharacter {
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
                crate::support::seed_object_dice(
                    &mut world,
                    ObjectId(1),
                    crate::support::FIXTURE_DICE_SEED,
                );
                firing::edit(&mut world, id, |state| {
                    state["scanner_perception"] = target.into()
                });
            }
            set_battle_speed(&mut world, id, ObjectId(1), 10.0).unwrap();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            // Connection presence is a host-session property, not saved character state.
            if perception.is_some() {
                restored
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
            }
            let mut messages = Vec::new();
            for _ in 0..50 {
                let notices =
                    advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
                let replay =
                    advance_battle_motion(&mut restored, BattleMovementRules::STANDARD).unwrap();
                assert_eq!(notices, replay);
                messages.extend(
                    notices
                        .into_iter()
                        .filter(|n| n.text.contains(" has CF of ")),
                );
            }
            if flags == 0 || (flags == 16 && perception == Some(-10)) {
                assert_eq!(messages.len(), 1, "{source}");
                assert_eq!(messages[0].unit, id);
                assert_eq!(
                    messages[0].text,
                    format!("THE FIELD WORKSHOP has CF of {integrity}.")
                );
            } else {
                assert!(messages.is_empty());
            }
            assert_eq!(world.btech.maps()[&interior].building.integrity, integrity);
        }
    }
}

/// Flying above an entrance does not disclose its construction factor.
#[tokio::test]
async fn aircraft_overflight_does_not_report_ground_buildings() {
    let source = include_str!("../game/mechs/Kestrel.toml");
    let (_dir, config, mut world, id, _, _) =
        firing::fixture_with_target(source, None, source).await;
    let map = world.btech.vehicles()[&id].position().unwrap().map;
    let interior = world.create(&config, "Hangar".into(), Kind::Room);
    create_battle_map(
        &mut world,
        interior,
        "inside",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    crate::support::seed_object_dice(&mut world, interior, crate::support::FIXTURE_DICE_SEED);
    set_building_entrance(
        &mut world,
        map,
        0,
        Some(BattleBuildingEntrance {
            coordinate: BattleHexCoordinate { x: 0, y: 10 },
            interior,
            data_char: 0,
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    firing::edit(&mut world, id, |state| {
        state["vtol_flight"]["phase"] = serde_json::json!({"kind": "airborne"});
        state["vtol_flight"]["altitude"] = 5.into();
    });
    set_battle_speed(&mut world, id, ObjectId(1), 10.0).unwrap();
    let mut crossed = false;
    for _ in 0..50 {
        let notices = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        assert!(!notices.iter().any(|n| n.text.contains(" has CF of ")));
        crossed |= world.btech.vehicles()[&id].position().unwrap().y == 10;
    }
    assert!(crossed, "The aircraft must actually cross the entrance hex");
}

/// A cliff stop reveals no unentered building; a forced fall reports the hex actually reached.
#[tokio::test]
async fn interrupted_ground_steps_report_only_accepted_surface_entries() {
    for source in firing::templates().into_iter().take(5) {
        for downhill in [false, true] {
            let (_dir, config, mut world, id, _, _) =
                firing::fixture_with_target(&source, None, &source).await;
            let map = if let Some(unit) = world.btech.vehicles().get(&id) {
                unit.position().unwrap().map
            } else {
                world.btech.constructed_units()[&id].position().unwrap().map
            };
            let interior = world.create(&config, "Cliff shelter".into(), Kind::Room);
            create_battle_map(
                &mut world,
                interior,
                "shelter",
                BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
            )
            .unwrap();
            crate::support::seed_object_dice(
                &mut world,
                interior,
                crate::support::FIXTURE_DICE_SEED,
            );
            set_building_entrance(
                &mut world,
                map,
                0,
                Some(BattleBuildingEntrance {
                    coordinate: BattleHexCoordinate { x: 0, y: 10 },
                    interior,
                    data_char: 0,
                    data_short: 0,
                    data_int: 0,
                }),
            )
            .unwrap();
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            let high = if downhill { 11 } else { 10 };
            crate::support::set_hex_elevation(
                &mut saved["maps"][map.0.to_string()]["terrain"][high],
                3,
            );
            world.btech = serde_json::from_value(saved).unwrap();
            firing::edit(&mut world, id, |state| state["auto_fall"] = true.into());
            set_battle_speed(&mut world, id, ObjectId(1), 10.0).unwrap();
            let mut messages = Vec::new();
            for _ in 0..50 {
                messages.extend(
                    advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap(),
                );
            }
            assert!(
                messages
                    .iter()
                    .any(|n| n.text.contains(if downhill { "drop" } else { "steep" })),
                "Movement must reach the cliff"
            );
            let cf: Vec<_> = messages
                .iter()
                .filter(|n| n.text.contains(" has CF of "))
                .collect();
            assert_eq!(
                cf.len(),
                usize::from(downhill),
                "{source}; downhill={downhill}"
            );
            if downhill {
                assert_eq!(cf[0].text, "THE CLIFF SHELTER has CF of 0.");
            }
        }
    }
}
