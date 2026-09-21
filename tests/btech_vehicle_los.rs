//! Mixed-unit terrain sight lines, optical queries and external illumination use live vehicle height.
use crate::support;
use stompymux_rs::*;

/// Two Mechs and two vehicles at opposite ends of a north/south lane.
async fn fixture(
    tiles: &str,
    vehicle: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 4]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Sight lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "sight",
        BattleMapAsset::parse(&format!("1 5\n{tiles}")).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for index in 0..4 {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if index < 2 {
            let mut definition =
                BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
            definition
                .attributes
                .insert("specials".into(), "FlipArms Searchlight".into());
            create_battle_unit(&mut world, id, definition).unwrap();
        } else {
            create_battle_vehicle(
                &mut world,
                id,
                BattleVehicleTemplate::parse(vehicle).unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut world, id, map, 0, if index % 2 == 0 { 4 } else { 0 }).unwrap();
        ids.push(id);
    }
    (dir, config, world, map, ids.try_into().unwrap())
}

#[tokio::test]
async fn vehicle_eye_height_changes_ridge_visibility_and_stationary_units_remain_tall() {
    for (template, tall) in [
        (include_str!("../game/mechs/Demolisher"), false),
        (include_str!("../game/mechs/RadioTower"), true),
    ] {
        let (_dir, config, world, _map, [mech_a, mech_b, vehicle_a, vehicle_b]) =
            fixture(".0\n.0\n.1\n.0\n.0\n", template).await;
        let before = world.btech.clone();
        assert!(
            !battle_unit_terrain_los(&world, mech_a, mech_b)
                .unwrap()
                .blocked
        );
        assert_eq!(
            battle_unit_terrain_los(&world, vehicle_a, vehicle_b)
                .unwrap()
                .blocked,
            !tall
        );
        for (a, b) in [
            (mech_a, vehicle_b),
            (vehicle_b, mech_a),
            (vehicle_a, mech_b),
        ] {
            assert_eq!(
                battle_unit_terrain_los(&world, a, b).unwrap().blocked,
                !tall
            );
        }
        assert_eq!(world.btech, before);
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            battle_unit_terrain_los(&world, vehicle_a, vehicle_b).unwrap(),
            battle_unit_terrain_los(&restored, vehicle_a, vehicle_b).unwrap()
        );
    }
}

#[tokio::test]
async fn vehicle_water_height_and_visual_queries_use_actual_surface_position() {
    for (template, hover) in [
        (include_str!("../game/mechs/Demolisher"), false),
        (include_str!("../game/mechs/Fulcrum"), true),
    ] {
        let (_dir, _config, world, _map, [mech_a, mech_b, vehicle_a, vehicle_b]) =
            fixture("~1\n~1\n~1\n~1\n~1\n", template).await;
        let before = world.btech.clone();
        assert!(
            !battle_unit_terrain_los(&world, vehicle_a, vehicle_b)
                .unwrap()
                .blocked
        );
        assert_eq!(
            battle_unit_terrain_los(&world, mech_a, vehicle_b)
                .unwrap()
                .blocked,
            !hover
        );
        assert_eq!(
            battle_unit_terrain_los(&world, vehicle_a, mech_b)
                .unwrap()
                .blocked,
            !hover
        );
        assert_eq!(
            battle_unit_elevation(&world, vehicle_a).unwrap(),
            Some(if hover { 0 } else { -1 })
        );
        let conditions = BattleSensorConditions {
            light: BattleLight::Day,
            visibility: 60,
            target_lit: false,
            disabled: false,
        };
        assert!(
            battle_optical_contact(
                &world,
                vehicle_a,
                vehicle_b,
                BattleSensorMode::Visual,
                conditions
            )
            .unwrap()
            .eligible
        );
        assert_eq!(
            battle_optical_contact(
                &world,
                mech_a,
                vehicle_b,
                BattleSensorMode::Visual,
                conditions
            )
            .unwrap()
            .eligible,
            hover
        );
        assert!(
            battle_optical_contact(
                &world,
                vehicle_a,
                vehicle_b,
                BattleSensorMode::Infrared,
                conditions
            )
            .unwrap()
            .eligible
        );
        assert_eq!(world.btech, before);
    }
}

#[tokio::test]
async fn mech_searchlights_illuminate_vehicle_targets_and_replay_optical_queries() {
    let (_dir, config, mut world, map, [lamp, _, observer, target]) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/mechs/Demolisher"),
    )
    .await;
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["light"] = 0.into();
    world.btech = serde_json::from_value(state).unwrap();
    let before = world.btech.clone();
    let dark = battle_map_optical_contact(
        &world,
        observer,
        target,
        BattleSensorMode::Visual,
        false,
        false,
    )
    .unwrap();
    assert!(dark.eligible);
    assert_eq!(dark.aim_modifier, 2);
    assert!(
        battle_map_optical_contact(
            &world,
            observer,
            target,
            BattleSensorMode::LightAmplification,
            false,
            false
        )
        .unwrap()
        .eligible
    );
    assert_eq!(world.btech, before);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(lamp);
    assign_battle_pilot(&mut world, lamp, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, lamp, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    toggle_battle_searchlight(&mut world, lamp, ObjectId(1)).unwrap();
    for _ in 0..5 {
        advance_battle_searchlights(&mut world);
    }
    assert!(battle_unit_illuminated(&world, target));
    let lit = battle_map_optical_contact(
        &world,
        observer,
        target,
        BattleSensorMode::Visual,
        false,
        false,
    )
    .unwrap();
    assert_eq!(lit.aim_modifier, 0);
    assert!(
        !battle_map_optical_contact(
            &world,
            observer,
            target,
            BattleSensorMode::LightAmplification,
            false,
            false
        )
        .unwrap()
        .eligible
    );
    assert!(
        !battle_map_optical_contact(
            &world,
            observer,
            target,
            BattleSensorMode::Visual,
            false,
            true
        )
        .unwrap()
        .eligible
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(battle_unit_illuminated(&restored, target));
    assert_eq!(
        lit,
        battle_map_optical_contact(
            &restored,
            observer,
            target,
            BattleSensorMode::Visual,
            false,
            false
        )
        .unwrap()
    );
    let other = world.create(&config, "Other map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        other,
        "other",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, target, other, 0, 0).unwrap();
    assert!(battle_unit_terrain_los(&world, observer, target).is_err());
    assert!(!battle_unit_illuminated(&world, target));
}

#[tokio::test]
async fn hovercraft_sight_lines_retain_under_bridge_height_after_restart() {
    let (_dir, config, mut world, _map, [_, _, observer, target]) = fixture(
        "/4\n.1\n.1\n.1\n/4\n",
        include_str!("../game/mechs/Fulcrum"),
    )
    .await;
    assert!(
        !battle_unit_terrain_los(&world, observer, target)
            .unwrap()
            .blocked
    );
    let mut state = serde_json::to_value(&world.btech).unwrap();
    for id in [observer, target] {
        state["vehicles"][id.0.to_string()]["under_bridge"] = true.into();
    }
    world.btech = serde_json::from_value(state).unwrap();
    assert_eq!(battle_unit_elevation(&world, observer).unwrap(), Some(0));
    let blocked = battle_unit_terrain_los(&world, observer, target).unwrap();
    assert!(blocked.blocked);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        blocked,
        battle_unit_terrain_los(&restored, observer, target).unwrap()
    );
}

/// Scenario scan inputs stay explicit until vehicle teams and contact cadence are owned.
fn vehicle_scan() -> BattleSensorScan {
    BattleSensorScan {
        primary: BattleSensorMode::Visual,
        secondary: BattleSensorMode::Visual,
        visual_disabled: false,
        amplification_disabled: false,
        perception: 7,
        target: BattleScanTarget {
            lit: false,
            hostile: false,
            hidden: false,
        },
    }
}

#[tokio::test]
async fn vehicle_acquisition_uses_hull_and_turret_weights_and_saves_exact_dice() {
    let (_dir, config, mut world, map, [_mech_a, mech_b, vehicle_a, vehicle_b]) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/mechs/Demolisher"),
    )
    .await;
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 30).unwrap();
    for target in [mech_b, vehicle_b] {
        for (heading, offset, base, arc) in [
            (0.0, 0.0, 115, BattleSensorArc::Front),
            (0.0, 90.0, 100, BattleSensorArc::Front),
            (90.0, 0.0, 80, BattleSensorArc::Side),
            (90.0, 270.0, 95, BattleSensorArc::Side),
            (180.0, 0.0, 50, BattleSensorArc::Rear),
            (180.0, 180.0, 65, BattleSensorArc::Rear),
        ] {
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            let vehicle = &mut saved["vehicles"][vehicle_a.0.to_string()];
            vehicle["motion"]["heading"] = serde_json::json!(heading);
            vehicle["motion"]["desired_heading"] = serde_json::json!(heading);
            vehicle["turret_offset"] = serde_json::json!(offset);
            vehicle["dice"] = serde_json::to_value(BattleDice::seeded([43; 32])).unwrap();
            world.btech = serde_json::from_value(saved).unwrap();
            let before = world.clone();
            let factor = battle_map_optical_contact(
                &world,
                vehicle_a,
                target,
                BattleSensorMode::Visual,
                false,
                false,
            )
            .unwrap()
            .acquisition_factor;
            let mut expected = BattleDice::seeded([43; 32]);
            let roll = expected.die(10_000).unwrap();
            let report =
                scan_battle_optical_target(&mut world, vehicle_a, target, vehicle_scan()).unwrap();
            assert_eq!(report.primary.threshold, base * u16::from(factor));
            assert_eq!(report.primary.roll, Some(roll));
            assert_eq!(report.primary.detected, roll < report.primary.threshold);
            assert_eq!(report.secondary, None);
            let mut single = before.clone();
            assert_eq!(
                roll_battle_optical_detection(
                    &mut single,
                    vehicle_a,
                    target,
                    BattleSensorAttempt {
                        sensor: BattleSensorMode::Visual,
                        target_lit: false,
                        disabled: false,
                        rules: BattleDetectionRules {
                            arc,
                            perception: 7,
                            hostile: false,
                            hidden: false,
                            secondary: false
                        },
                    }
                )
                .unwrap(),
                report.primary
            );
            assert_eq!(single.btech, world.btech);
            persistence::save(&config.database(), &world).await.unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                roll_unit_dice(&mut restored, vehicle_a, 1).unwrap(),
                vec![expected.d6()]
            );
            assert_eq!(
                roll_unit_dice(&mut restored, target, 1).unwrap(),
                roll_unit_dice(&mut before.clone(), target, 1).unwrap()
            );
        }
    }
}

#[tokio::test]
async fn vehicle_acquisition_secondary_failures_and_close_contacts_preserve_roll_order() {
    let (_dir, _config, mut world, map, [_mech_a, _mech_b, vehicle_a, vehicle_b]) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/mechs/Demolisher"),
    )
    .await;
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 30).unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][vehicle_a.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([77; 32])).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
    let before = world.btech.clone();
    let scan = BattleSensorScan {
        secondary: BattleSensorMode::Infrared,
        visual_disabled: true,
        ..vehicle_scan()
    };
    let mut infrared = world.clone();
    let report = scan_battle_optical_target(&mut infrared, vehicle_a, vehicle_b, scan).unwrap();
    let mut expected = BattleDice::seeded([77; 32]);
    assert_eq!(report.primary.roll, None);
    assert_eq!(
        report.secondary.unwrap().roll,
        Some(expected.die(10_000).unwrap())
    );
    assert_eq!(
        roll_unit_dice(&mut infrared, vehicle_a, 1).unwrap(),
        vec![expected.d6()]
    );
    assert_eq!(world.btech, before);
    let scan = BattleSensorScan {
        secondary: BattleSensorMode::LightAmplification,
        visual_disabled: true,
        ..vehicle_scan()
    };
    let report = scan_battle_optical_target(&mut world, vehicle_a, vehicle_b, scan).unwrap();
    assert_eq!(report.primary.roll, None);
    let mut expected = BattleDice::seeded([77; 32]);
    assert_eq!(
        report.secondary.unwrap().roll,
        Some(expected.die(10_000).unwrap())
    );
    let factor = battle_map_optical_contact(
        &world,
        vehicle_a,
        vehicle_b,
        BattleSensorMode::LightAmplification,
        false,
        false,
    )
    .unwrap()
    .acquisition_factor;
    assert_eq!(
        report.secondary.unwrap().threshold,
        115 * u16::from(factor) / 2
    );
    assert_eq!(
        roll_unit_dice(&mut world.clone(), vehicle_a, 1).unwrap(),
        vec![expected.d6()]
    );

    place_battle_unit(&mut world, vehicle_b, map, 0, 3).unwrap();
    let before = world.btech.clone();
    let report =
        scan_battle_optical_target(&mut world, vehicle_a, vehicle_b, vehicle_scan()).unwrap();
    assert!(report.primary.detected);
    assert_eq!(report.primary.roll, None);
    assert_eq!(world.btech, before);
    let disabled = BattleSensorScan {
        visual_disabled: true,
        ..vehicle_scan()
    };
    assert!(
        scan_battle_optical_target(&mut world, vehicle_a, vehicle_b, disabled)
            .unwrap()
            .detected_by
            .is_none()
    );
    assert_eq!(world.btech, before);
}

/// Burrowing lowers a moving chassis enough for a shallow intervening ridge to hide it.
#[tokio::test]
async fn dug_in_eye_height_changes_live_los_and_survives_restart() {
    let (_dir, config, mut world, _map, [observer, _, _, target]) = fixture(
        ".0\n.1\n.0\n.0\n.2\n",
        include_str!("../game/mechs/Demolisher"),
    )
    .await;
    assert!(
        !battle_unit_terrain_los(&world, observer, target)
            .unwrap()
            .blocked
    );
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][target.0.to_string()]["dig"] =
        serde_json::to_value(BattleDigState::covered()).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
    let before = world.btech.clone();
    for (a, b) in [(observer, target), (target, observer)] {
        assert!(battle_unit_terrain_los(&world, a, b).unwrap().blocked);
    }
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert!(
        battle_unit_terrain_los(&loaded, observer, target)
            .unwrap()
            .blocked
    );
}
