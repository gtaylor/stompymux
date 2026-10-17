//! Mixed-unit terrain sight lines, perception and illumination use live vehicle height.
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
                MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
                    .unwrap();
            definition
                .attributes
                .insert("specials".into(), "FlipArms Searchlight".into());
            create_battle_unit(&mut world, id, definition).unwrap();
        } else {
            create_battle_vehicle(
                &mut world,
                id,
                VehicleTemplate::parse("test", vehicle).unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut world, id, map, 0, if index % 2 == 0 { 4 } else { 0 }).unwrap();
        ids.push(id);
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, map, ids.try_into().unwrap())
}

/// Low vehicles lose sight of each other over a one-level ridge midway that Mechs and tall
/// installations see across; a Mech also sees a vehicle over that ridge, because the ridge is
/// below the Mech and next to neither unit (Total Warfare p. 100).
#[tokio::test]
async fn vehicle_eye_height_changes_ridge_visibility_and_stationary_units_remain_tall() {
    for (template, tall) in [
        (include_str!("../game/units/Demolisher.toml"), false),
        (include_str!("../game/units/RadioTower.toml"), true),
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
            assert!(!battle_unit_terrain_los(&world, a, b).unwrap().blocked);
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
        (include_str!("../game/units/Demolisher.toml"), false),
        (include_str!("../game/units/Fulcrum.toml"), true),
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

/// Pitch black hides a vehicle four hexes away unless a Mech searchlight lights it, and the
/// lit target is seen out to the lit-target reach.
#[tokio::test]
async fn mech_searchlights_illuminate_vehicle_targets_and_replay_perception() {
    let (_dir, config, mut world, map, [lamp, _, observer, target]) = fixture(
        ".0\n.0\n.0\n.0\n.0\n",
        include_str!("../game/units/Demolisher.toml"),
    )
    .await;
    set_battle_map_conditions(&mut world, map, Light::Day, Weather::Clear).unwrap();
    // A sweep of 12 finds nothing, so only sight reaches the target four hexes away.
    world
        .btech
        .set_unit_sensor_sweep(observer, Some(12))
        .unwrap();
    let before = world.btech.clone();
    let sight = |world: &World| {
        battle_perceive(world, observer, target)
            .unwrap()
            .map(|perception| (perception.channel, perception.aim_modifier))
    };
    assert_eq!(sight(&world), Some((DetectionChannel::Sight, 0)));
    // Pitch black limits sight to three hexes.
    let mut short = world.clone();
    set_battle_map_conditions(&mut short, map, Light::PitchBlack, Weather::Clear).unwrap();
    assert_eq!(sight(&short), None);
    assert_eq!(world.btech, before);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(lamp);
    assign_battle_pilot(&mut world, lamp, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, lamp, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    toggle_battle_searchlight(&mut world, lamp, ObjectId(1)).unwrap();
    for _ in 0..5 {
        advance_battle_searchlights(&mut world);
    }
    assert!(battle_unit_illuminated(&world, target));
    let lit = sight(&world);
    assert_eq!(lit, Some((DetectionChannel::Sight, 0)));
    // A lit target stays visible at night out to the lit-target reach.
    let mut short = world.clone();
    set_battle_map_conditions(&mut short, map, Light::PitchBlack, Weather::Clear).unwrap();
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
        include_str!("../game/units/Fulcrum.toml"),
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
