//! Mixed surface failures share terrain transactions while retaining chassis consequences.
use crate::support;
use stompymux_rs::*;

/// One Mech and two vehicle movement types occupy the same breakable hex.
async fn fixture(
    terrain: Terrain,
    depth: u8,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 3]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Surface".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "surface",
        MapAsset::from_cells(&format!("1 1\n{}{depth}\n", terrain.symbol())).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for index in 0..3 {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if index == 0 {
            create_battle_unit(
                &mut world,
                id,
                MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
                    .unwrap(),
            )
            .unwrap();
        } else {
            create_battle_vehicle(
                &mut world,
                id,
                VehicleTemplate::parse(
                    "test",
                    if index == 1 {
                        include_str!("../game/units/Demolisher.toml")
                    } else {
                        include_str!("../game/units/Fulcrum.toml")
                    },
                )
                .unwrap(),
            )
            .unwrap();
        }
        place_battle_unit(&mut world, id, map, 0, 0).unwrap();
        ids.push(id);
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    (dir, config, world, map, [ids[0], ids[1], ids[2]])
}

#[tokio::test]
async fn ice_falls_damage_then_flood_ground_vehicles_and_leave_hovercraft_unchanged() {
    let (_dir, config, mut world, map, ids) = fixture(Terrain::Ice, 3).await;
    let hover = world.btech.vehicles()[&ids[2]].clone();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    let report = break_battle_ice(
        &mut world,
        map,
        HexCoordinate { x: 0, y: 0 },
        Some(ids[1]),
        MovementRules::STANDARD.fall,
    )
    .unwrap();
    assert_eq!(
        report,
        break_battle_ice(
            &mut restored,
            map,
            HexCoordinate { x: 0, y: 0 },
            Some(ids[1]),
            MovementRules::STANDARD.fall
        )
        .unwrap()
    );
    assert_eq!(world.btech, restored.btech);
    assert_eq!(report.falls.len(), 1);
    assert_eq!(report.vehicle_falls.len(), 1);
    assert_eq!(report.vehicle_falls[0].0, ids[1]);
    // Three levels down through the ice: the levels fallen plus one (Total Warfare p. 68).
    assert_eq!(report.vehicle_falls[0].1.damage, 16);
    assert_eq!(report.flooded_vehicles, [ids[1]]);
    assert!(world.btech.vehicles()[&ids[1]].flooded());
    assert_eq!(world.btech.vehicles()[&ids[2]], hover);
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn shallow_ice_changes_terrain_without_falls_or_flooding() {
    let (_dir, config, mut world, map, ids) = fixture(Terrain::Ice, 0).await;
    let before = world.btech.vehicles().clone();
    let report = break_battle_ice(
        &mut world,
        map,
        HexCoordinate { x: 0, y: 0 },
        Some(ids[1]),
        MovementRules::STANDARD.fall,
    )
    .unwrap();
    assert!(
        report.falls.is_empty()
            && report.vehicle_falls.is_empty()
            && report.flooded_vehicles.is_empty()
    );
    assert_eq!(world.btech.vehicles(), &before);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn bridge_collapse_drops_deck_vehicles_and_clears_under_span_state() {
    let (_dir, config, mut world, map, ids) = fixture(Terrain::Bridge, 3).await;
    world
        .btech
        .rewrite_unit_record(ids[2], |record| {
            record["under_bridge"] = true.into();
        })
        .unwrap();
    let report = break_battle_bridge(
        &mut world,
        map,
        HexCoordinate { x: 0, y: 0 },
        MovementRules::STANDARD.fall,
    )
    .unwrap();
    // The vehicle on the deck falls past it into the water; the hovercraft beneath stays put.
    assert_eq!(
        report
            .vehicle_falls
            .iter()
            .map(|(id, _)| *id)
            .collect::<Vec<_>>(),
        [ids[1]]
    );
    let tile = world.btech.maps()[&map].hex(0, 0).unwrap();
    assert_eq!(world.btech.vehicles()[&ids[1]].elevation_level(tile), -1);
    assert_eq!(world.btech.vehicles()[&ids[2]].elevation_level(tile), 0);
    assert!(!world.btech.vehicles()[&ids[2]].under_bridge());
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

#[tokio::test]
async fn unsupported_character_fall_rolls_back_prior_neighbor_damage_and_terrain() {
    let (_dir, _config, mut world, map, ids) = fixture(Terrain::Ice, 3).await;
    world
        .objects
        .get_mut(&ids[1])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let before = world.btech.clone();
    assert!(
        break_battle_ice(
            &mut world,
            map,
            HexCoordinate { x: 0, y: 0 },
            Some(ids[1]),
            MovementRules::STANDARD.fall
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn vehicle_fall_fractures_ice_before_outer_damage_and_replays_nested_falls() {
    let (_dir, config, mut world, _map, ids) = fixture(Terrain::Ice, 3).await;
    let id = ids[1];
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).d6() == 1)
        .unwrap();
    world
        .btech
        .set_unit_dice(id, Dice::seeded([seed; 32]))
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    let fall =
        resolve_battle_vehicle_fall(&mut world, id, 1, MovementRules::STANDARD.fall).unwrap();
    assert_eq!(
        fall,
        resolve_battle_vehicle_fall(&mut restored, id, 1, MovementRules::STANDARD.fall).unwrap()
    );
    assert_eq!(world.btech, restored.btech);
    let fracture = fall.ice_break.as_ref().unwrap();
    // Three levels down through the ice: the levels fallen plus one (Total Warfare p. 68).
    assert_eq!(fracture.vehicle_falls[0].1.damage, 16);
    assert_eq!(fall.damage, 4);
    assert!(world.btech.vehicles()[&id].flooded());
    assert!(!fall.groups.is_empty());
    world.validate(&config).unwrap();
}

/// A moving tank on ice cannot be shut down by its pilot, so shutting down never tumbles it
/// through the ice: the command and Lua refuse and change nothing, and the ice holds.
#[tokio::test]
async fn moving_vehicle_shutdown_on_ice_is_refused_without_breaking_it() {
    let (_dir, config, mut world, map, ids) = fixture(Terrain::Ice, 3).await;
    let tank = ids[1];
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(tank);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(&mut world, tank, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, tank, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    world
        .btech
        .rewrite_unit_record(tank, |record| {
            record["motion"]["speed"] = 21.5.into();
            record["motion"]["desired_speed"] = 21.5.into();
        })
        .unwrap();
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let error = stop_battle_unit_action(&scripts, &config, tank, ObjectId(1)).unwrap_err();
    assert!(
        format!("{error:#}").contains("You must come to a stop before you can shut down."),
        "{error:#}"
    );
    let call = format!("return btech.unit.stop({},1)", tank.0);
    let error = scripts.eval_callback::<bool>(&call).unwrap_err();
    assert!(format!("{error:#}").contains("come to a stop"), "{error:#}");
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    assert_eq!(
        scripts.world().btech.maps()[&map]
            .base_hex(0, 0)
            .unwrap()
            .terrain(),
        Terrain::Ice
    );
}
