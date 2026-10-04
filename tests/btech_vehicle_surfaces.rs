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
                BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                    .unwrap(),
            )
            .unwrap();
        } else {
            create_battle_vehicle(
                &mut world,
                id,
                BattleVehicleTemplate::parse(
                    "test",
                    if index == 1 {
                        include_str!("../game/mechs/Demolisher.toml")
                    } else {
                        include_str!("../game/mechs/Fulcrum.toml")
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
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert_eq!(
        report,
        break_battle_ice(
            &mut restored,
            map,
            HexCoordinate { x: 0, y: 0 },
            Some(ids[1]),
            BattleMovementRules::STANDARD.fall
        )
        .unwrap()
    );
    assert_eq!(world.btech, restored.btech);
    assert_eq!(report.falls.len(), 1);
    assert_eq!(report.vehicle_falls.len(), 1);
    assert_eq!(report.vehicle_falls[0].0, ids[1]);
    assert_eq!(report.vehicle_falls[0].1.damage, 12);
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
        BattleMovementRules::STANDARD.fall,
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
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][ids[2].0.to_string()]["under_bridge"] = true.into();
    world.btech = serde_json::from_value(saved).unwrap();
    let report = break_battle_bridge(
        &mut world,
        map,
        HexCoordinate { x: 0, y: 0 },
        BattleMovementRules::STANDARD.fall,
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
            BattleMovementRules::STANDARD.fall
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
        .find(|seed| BattleDice::seeded([*seed; 32]).d6() == 1)
        .unwrap();
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        })
        .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    let fall =
        resolve_battle_vehicle_fall(&mut world, id, 1, BattleMovementRules::STANDARD.fall).unwrap();
    assert_eq!(
        fall,
        resolve_battle_vehicle_fall(&mut restored, id, 1, BattleMovementRules::STANDARD.fall)
            .unwrap()
    );
    assert_eq!(world.btech, restored.btech);
    let fracture = fall.ice_break.as_ref().unwrap();
    assert_eq!(fracture.vehicle_falls[0].1.damage, 12);
    assert_eq!(fall.damage, 4);
    assert!(world.btech.vehicles()[&id].flooded());
    assert!(!fall.impacts.is_empty());
    world.validate(&config).unwrap();
}

/// A shutdown tumble preserves vehicle and neighboring Mech checks through nested ice breakage.
#[tokio::test]
async fn vehicle_shutdown_ice_cascade_keeps_each_pilots_feedback_private() {
    let (_dir, config, mut world, map, ids) = fixture(Terrain::Ice, 3).await;
    let tank = ids[1];
    for (unit, pilot) in [(tank, ObjectId(1)), (ids[0], ObjectId(2))] {
        world.objects.get_mut(&pilot).unwrap().location = Some(unit);
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        assign_battle_pilot(&mut world, unit, pilot).unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, unit, pilot, true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
    }
    let passenger = world.create(&config, "Crash passenger".into(), Kind::Player);
    world.objects.get_mut(&passenger).unwrap().location = Some(tank);
    world
        .objects
        .get_mut(&passenger)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let dice = (0..=u16::MAX)
        .find_map(|seed| {
            let mut bytes = [0; 32];
            bytes[..2].copy_from_slice(&seed.to_le_bytes());
            let dice = BattleDice::seeded(bytes);
            let mut probe = dice.clone();
            (probe.two_d6() == 12 && probe.d6() == 1).then_some(dice)
        })
        .unwrap();
    world
        .btech
        .rewrite_unit_record(tank, |record| {
            record["dice"] = serde_json::to_value(dice).unwrap();
            record["motion"]["speed"] = 21.5.into();
            record["motion"]["desired_speed"] = 21.5.into();
        })
        .unwrap();
    let before = world.clone();
    let native = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(before.clone())),
    )
    .unwrap();
    let call = format!("btech.unit.stop({},1)", tank.0);
    assert!(
        lua.eval_callback::<()>(&format!("{call}; error('abort cascade')"))
            .is_err()
    );
    assert_eq!(lua.world().btech, before.btech);
    assert!(lua.drain_outbox().is_empty());
    stop_battle_unit_action(&native, &config, tank, ObjectId(1)).unwrap();
    lua.eval_callback::<()>(&call).unwrap();
    assert_eq!(lua.world().btech, native.world().btech);
    assert_eq!(
        native.world().btech.maps()[&map]
            .base_hex(0, 0)
            .unwrap()
            .terrain(),
        Terrain::Water
    );
    let output = native.drain_outbox();
    let replay = lua.drain_outbox();
    assert_eq!(
        output
            .iter()
            .map(|(who, message)| (*who, message.source()))
            .collect::<Vec<_>>(),
        replay
            .iter()
            .map(|(who, message)| (*who, message.source()))
            .collect::<Vec<_>>()
    );
    for pilot in [ObjectId(1), ObjectId(2)] {
        let messages: Vec<_> = output
            .iter()
            .filter(|(who, _)| *who == pilot)
            .map(|(_, message)| message.source())
            .collect();
        let index = messages
            .iter()
            .position(|message| *message == "You make a piloting skill roll!")
            .unwrap();
        assert!(messages[index + 1].starts_with("Modified Pilot Skill: BTH "));
    }
    assert!(output.iter().any(|(who, _)| *who == passenger));
    assert!(!output.iter().any(|(who, message)| *who == passenger
        && (message.source().starts_with("Modified Pilot Skill:")
            || message.source() == "You make a piloting skill roll!")));
}
