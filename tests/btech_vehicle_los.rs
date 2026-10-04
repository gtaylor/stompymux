//! Mixed-unit terrain sight lines, perception, illumination and hidden-unit searches use live vehicle height.
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
        MapAsset::from_cells(&format!("1 5\n{tiles}")).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for index in 0..4 {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if index < 2 {
            let mut definition =
                BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                    .unwrap();
            definition
                .attributes
                .insert("specials".into(), "FlipArms Searchlight".into());
            create_battle_unit(&mut world, id, definition).unwrap();
        } else {
            create_battle_vehicle(
                &mut world,
                id,
                BattleVehicleTemplate::parse("test", vehicle).unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut world, id, map, 0, if index % 2 == 0 { 4 } else { 0 }).unwrap();
        ids.push(id);
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, map, ids.try_into().unwrap())
}

/// Mark units running through their saved state so contact updates may run without crews.
fn running(world: &mut World, ids: &[ObjectId]) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    for id in ids {
        let key = if world.btech.vehicles().contains_key(id) {
            "vehicles"
        } else {
            "constructed"
        };
        state[key][id.0.to_string()]["power"] = serde_json::to_value(BattlePower::Running).unwrap();
    }
    world.btech = serde_json::from_value(state).unwrap();
}

/// Low vehicles lose sight over a one-level ridge that Mechs and tall installations see across.
#[tokio::test]
async fn vehicle_eye_height_changes_ridge_visibility_and_stationary_units_remain_tall() {
    for (template, tall) in [
        (include_str!("../game/mechs/Demolisher.toml"), false),
        (include_str!("../game/mechs/RadioTower.toml"), true),
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

/// Submerged vehicles are hidden from Mechs, hovercraft on the surface are not.
#[tokio::test]
async fn vehicle_water_height_and_perception_use_actual_surface_position() {
    for (template, hover) in [
        (include_str!("../game/mechs/Demolisher.toml"), false),
        (include_str!("../game/mechs/Fulcrum.toml"), true),
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
        assert!(
            battle_perceive(&world, vehicle_a, vehicle_b)
                .unwrap()
                .is_some()
        );
        assert_eq!(
            battle_perceive(&world, mech_a, vehicle_b)
                .unwrap()
                .is_some(),
            hover
        );
        assert_eq!(world.btech, before);
    }
}

/// Beyond the sensor band, darkness costs +1 unless a Mech searchlight lights the vehicle target.
#[tokio::test]
async fn mech_searchlights_illuminate_vehicle_targets_and_replay_perception() {
    let (_dir, config, mut world, map, [lamp, _, observer, target]) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/mechs/Demolisher.toml"),
    )
    .await;
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["light"] = 0.into();
    world.btech = serde_json::from_value(state).unwrap();
    // Without the all-conditions band, only sight reaches the target four hexes away.
    set_battle_map_perception(&mut world, map, BattleMapPerceptionFlag::Sensors, false).unwrap();
    let before = world.btech.clone();
    let sight = |world: &World| {
        battle_perceive(world, observer, target)
            .unwrap()
            .map(|perception| (perception.channel, perception.aim_modifier))
    };
    assert_eq!(sight(&world), Some((BattleDetectionChannel::Sight, 1)));
    let mut short = world.clone();
    set_battle_map_visibility(&mut short, map, BattleLight::Night, 2).unwrap();
    assert_eq!(sight(&short), None);
    assert_eq!(world.btech, before);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(lamp);
    assign_battle_pilot(&mut world, lamp, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, lamp, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    toggle_battle_searchlight(&mut world, lamp, ObjectId(1)).unwrap();
    for _ in 0..5 {
        advance_battle_searchlights(&mut world);
    }
    assert!(battle_unit_illuminated(&world, target));
    let lit = sight(&world);
    assert_eq!(lit, Some((BattleDetectionChannel::Sight, 0)));
    // A lit target stays visible out to three times the night visibility.
    let mut short = world.clone();
    set_battle_map_visibility(&mut short, map, BattleLight::Night, 2).unwrap();
    assert_eq!(sight(&short), lit);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert!(battle_unit_illuminated(&restored, target));
    assert_eq!(sight(&restored), lit);
    let other = world.create(&config, "Other map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        other,
        "other",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, other, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, target, other, 0, 0).unwrap();
    assert!(battle_unit_terrain_los(&world, observer, target).is_err());
    assert!(!battle_unit_illuminated(&world, target));
}

/// Hovercraft under a bridge lose the sight line, and the posture survives restart.
#[tokio::test]
async fn hovercraft_sight_lines_retain_under_bridge_height_after_restart() {
    let (_dir, config, mut world, _map, [_, _, observer, target]) = fixture(
        "/4\n.1\n.1\n.1\n/4\n",
        include_str!("../game/mechs/Fulcrum.toml"),
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

/// A hidden hostile target searched for by a pilot with perception skill seven.
const HIDDEN: BattleContactRules = BattleContactRules {
    hostile: true,
    hidden: true,
    perception: 7,
    acquire: true,
};

/// Hidden-unit searches weight the vehicle hull arc, add the turret bonus and save exact dice.
#[tokio::test]
async fn vehicle_acquisition_uses_hull_and_turret_weights_and_saves_exact_dice() {
    let (_dir, config, mut world, map, [_mech_a, mech_b, vehicle_a, vehicle_b]) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/mechs/Demolisher.toml"),
    )
    .await;
    set_battle_map_visibility(&mut world, map, BattleLight::Day, 30).unwrap();
    running(&mut world, &[vehicle_a]);
    assert_eq!(battle_perception_factor(HIDDEN.perception), 79);
    for target in [mech_b, vehicle_b] {
        // Four hexes lies between the automatic and maximum search ranges: trunc(100 - 4/3) = 98.
        let distance = battle_perceive(&world, vehicle_a, target)
            .unwrap()
            .unwrap()
            .range
            .spatial;
        assert!(
            (AUTOMATIC_DETECTION_RANGE..=HIDDEN_DETECTION_RANGE).contains(&distance),
            "{distance}"
        );
        assert_eq!((100.0 - distance / 3.0) as u16, 98);
        // Threshold = hull arc (+15 turret) * 79 / 100 / 4 * 98.
        for (heading, offset, threshold, arc) in [
            (0.0, 0.0, 2156, BattleSensorArc::Front),
            (0.0, 90.0, 1862, BattleSensorArc::Front),
            (90.0, 0.0, 1470, BattleSensorArc::Side),
            (90.0, 270.0, 1764, BattleSensorArc::Side),
            (180.0, 0.0, 882, BattleSensorArc::Rear),
            (180.0, 180.0, 1176, BattleSensorArc::Rear),
        ] {
            world
                .btech
                .rewrite_unit_record(vehicle_a, |record| {
                    let vehicle = record;
                    vehicle["motion"]["heading"] = serde_json::json!(heading);
                    vehicle["motion"]["desired_heading"] = serde_json::json!(heading);
                    vehicle["turret_offset"] = serde_json::json!(offset);
                    vehicle["dice"] = serde_json::to_value(BattleDice::seeded([43; 32])).unwrap();
                    vehicle["contacts"] = serde_json::json!({});
                })
                .unwrap();
            assert_eq!(
                BattleSensorArc::from_bearing(0.0, heading, BattleFacing::default()).unwrap(),
                arc
            );
            let before = world.clone();
            let mut expected = BattleDice::seeded([43; 32]);
            let roll = expected.die(10_000).unwrap();
            let detection = update_battle_contact(&mut world, vehicle_a, target, HIDDEN)
                .unwrap()
                .detection
                .unwrap();
            assert_eq!(
                detection,
                BattleDetection {
                    detected: roll < threshold,
                    threshold,
                    roll: Some(roll)
                }
            );
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

/// Only hidden hostile targets between the automatic and maximum search ranges consume dice.
#[tokio::test]
async fn vehicle_acquisition_rolls_only_for_hidden_hostiles_beyond_automatic_range() {
    let (_dir, _config, mut world, map, [_mech_a, _mech_b, vehicle_a, vehicle_b]) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/mechs/Demolisher.toml"),
    )
    .await;
    set_battle_map_visibility(&mut world, map, BattleLight::Night, 30).unwrap();
    running(&mut world, &[vehicle_a]);
    world
        .btech
        .set_unit_dice(vehicle_a, BattleDice::seeded([77; 32]))
        .unwrap();
    let before = world.btech.clone();
    for (hostile, hidden) in [(false, false), (true, false), (false, true)] {
        let mut trial = world.clone();
        let update = update_battle_contact(
            &mut trial,
            vehicle_a,
            vehicle_b,
            BattleContactRules {
                hostile,
                hidden,
                ..HIDDEN
            },
        )
        .unwrap();
        assert_eq!(update.transition, BattleContactTransition::Acquired);
        assert_eq!(
            update.detection,
            Some(BattleDetection {
                detected: true,
                threshold: 0,
                roll: None
            })
        );
        assert_eq!(
            roll_unit_dice(&mut trial, vehicle_a, 1).unwrap(),
            vec![BattleDice::seeded([77; 32]).d6()]
        );
    }
    let mut searched = world.clone();
    let mut expected = BattleDice::seeded([77; 32]);
    let update = update_battle_contact(&mut searched, vehicle_a, vehicle_b, HIDDEN).unwrap();
    assert_eq!(
        update.detection.unwrap().roll,
        Some(expected.die(10_000).unwrap())
    );
    assert_eq!(
        roll_unit_dice(&mut searched, vehicle_a, 1).unwrap(),
        vec![expected.d6()]
    );
    assert_eq!(world.btech, before);

    place_battle_unit(&mut world, vehicle_b, map, 0, 3).unwrap();
    let update = update_battle_contact(&mut world, vehicle_a, vehicle_b, HIDDEN).unwrap();
    let detection = update.detection.unwrap();
    assert!(detection.detected);
    assert!(detection.threshold > 0);
    assert_eq!(detection.roll, None);
    assert_eq!(update.transition, BattleContactTransition::Acquired);
    assert_eq!(
        roll_unit_dice(&mut world, vehicle_a, 1).unwrap(),
        vec![BattleDice::seeded([77; 32]).d6()]
    );
}

/// Burrowing lowers a moving chassis enough for a shallow intervening ridge to hide it.
#[tokio::test]
async fn dug_in_eye_height_changes_live_los_and_survives_restart() {
    let (_dir, config, mut world, _map, [observer, _, _, target]) = fixture(
        ".0\n.1\n.0\n.0\n.2\n",
        include_str!("../game/mechs/Demolisher.toml"),
    )
    .await;
    assert!(
        !battle_unit_terrain_los(&world, observer, target)
            .unwrap()
            .blocked
    );
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["dig"] = serde_json::to_value(BattleDigState::covered()).unwrap();
        })
        .unwrap();
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
