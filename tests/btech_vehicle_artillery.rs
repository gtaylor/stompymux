//! Mixed artillery occupants share blast packets, heat policy, and atomic saved arrivals.
use crate::support;
use stompymux_rs::*;

/// Stable slot order puts a vehicle before a Mech, with a second vehicle in a neighboring cell.
async fn fixture(
    movement: VehicleMovement,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 3]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Artillery field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        MapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for index in 0..3 {
        let id = world.create(&config, "Target".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        if index == 1 {
            create_battle_unit(
                &mut world,
                id,
                MechTemplate::parse("AS7-D", include_str!("../game/units/AS7-D.toml")).unwrap(),
            )
            .unwrap();
        } else {
            let mut template = crate::support::templates::fitted(
                VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
                    .unwrap(),
            );
            template.movement = movement;
            create_battle_vehicle(&mut world, id, template).unwrap();
        }
        place_battle_unit(&mut world, id, map, 1, if index == 2 { 0 } else { 1 }).unwrap();
        ids.push(id);
    }
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    // Face the blast source in packet-arithmetic fixtures; rear selection has separate coverage.
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    for index in [0, 2] {
        saved["vehicles"][ids[index].0.to_string()]["motion"]["heading"] = 180.0.into();
        saved["vehicles"][ids[index].0.to_string()]["motion"]["desired_heading"] = 180.0.into();
    }
    world.btech = serde_json::from_value(saved).unwrap();
    (dir, config, world, map, ids.try_into().unwrap())
}

/// Keep packet tests independent of random critical cascades.
fn rules() -> FallRules {
    MovementRules::STANDARD.fall
}

/// Stop just before arrival so persisted replay includes the map's pattern dice.
fn approaching(world: &mut World, map: ObjectId, mode: ArtilleryMode) -> ArtilleryFlight {
    let center = HexCoordinate { x: 1, y: 1 };
    let mut flight =
        ArtilleryFlight::new(center, center, Weapon::LongTom, mode, ArtilleryOutcome::Hit).unwrap();
    for _ in 0..9 {
        assert!(
            advance_artillery_flight(world, map, &mut flight, rules())
                .unwrap()
                .is_none()
        );
    }
    flight
}

#[tokio::test]
async fn mixed_artillery_packets_use_vehicle_tables_and_replay_arrival() {
    let (_dir, config, base, map, ids) = fixture(VehicleMovement::Stationary).await;
    for mode in [ArtilleryMode::Standard, ArtilleryMode::Cluster] {
        let mut world = base.clone();
        let mut flight = approaching(&mut world, map, mode);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let mut replay = flight.clone();
        let report = advance_artillery_flight(&mut world, map, &mut flight, rules())
            .unwrap()
            .unwrap();
        assert_eq!(
            advance_artillery_flight(&mut restored, map, &mut replay, rules()).unwrap(),
            Some(report.clone())
        );
        assert_eq!(world.btech, restored.btech);
        assert_eq!(flight, replay);
        if mode == ArtilleryMode::Standard {
            assert_eq!(
                report.hits.iter().map(|hit| hit.unit).collect::<Vec<_>>(),
                ids
            );
            assert_eq!(
                report
                    .hits
                    .iter()
                    .map(|hit| hit.impacts.len())
                    .collect::<Vec<_>>(),
                [5, 5, 3]
            );
        }
        assert!(!report.hits.is_empty());
        for hit in &report.hits {
            let cell = report
                .pattern
                .cells
                .iter()
                .find(|cell| cell.position == hit.coordinate)
                .unwrap();
            let ArtilleryEffect::Damage {
                total, packet_size, ..
            } = cell.effect
            else {
                panic!("Damage cell expected")
            };
            assert_eq!(
                hit.impacts.len(),
                usize::from(total).div_ceil(usize::from(packet_size))
            );
            for impact in &hit.impacts {
                if hit.unit == ids[1] {
                    assert!(matches!(impact, BlastImpact::Mech(_)));
                } else {
                    let BlastImpact::Vehicle(impact) = impact else {
                        panic!("Vehicle packet expected")
                    };
                    assert_eq!(
                        impact.damage.as_ref().unwrap().incoming,
                        u32::from(packet_size)
                    );
                    // Cluster's punch table is Mech-only; vehicles still draw a normal 2d6 location.
                    assert!(!impact.rolls.is_empty());
                    assert!((2..=12).contains(&impact.rolls[0]));
                }
            }
            if hit.unit != ids[1] {
                assert!(hit.vehicle_heat.is_some());
            }
        }
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn later_vehicle_character_guard_restores_artillery_world_and_cursor() {
    let (_dir, config, mut world, map, ids) = fixture(VehicleMovement::Stationary).await;
    world
        .objects
        .get_mut(&ids[2])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let mut flight = approaching(&mut world, map, ArtilleryMode::Standard);
    let before = world.clone();
    let cursor = flight.clone();
    assert!(advance_artillery_flight(&mut world, map, &mut flight, rules()).is_err());
    assert_eq!(world.btech, before.btech);
    assert_eq!(
        serde_json::to_value(&world.objects).unwrap(),
        serde_json::to_value(&before.objects).unwrap()
    );
    assert_eq!(flight, cursor);
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn vehicle_observers_receive_visible_artillery_arrival_notices() {
    let (_dir, config, mut world, map, ids) = fixture(VehicleMovement::Stationary).await;
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ids[0]);
    assign_battle_pilot(&mut world, ids[0], ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, ids[0], ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    let mut flight = approaching(&mut world, map, ArtilleryMode::Standard);
    let report = advance_artillery_flight(&mut world, map, &mut flight, rules())
        .unwrap()
        .unwrap();
    assert!(report.notices.iter().any(|notice| notice.unit == ids[0]
        && notice.text.contains("LongTom fire hits")
        && notice.text.contains("YOUR HEX")));
    assert!(
        !report
            .notices
            .iter()
            .any(|notice| notice.unit == ids[2] && notice.text.contains("LongTom fire hits"))
    );
    world.validate(&config).unwrap();
}

/// Artillery loses ten points per level below the impact surface (TO:AR p. 183), so a hull on a
/// Depth 3 bed is beyond a Sniper's reach while a hovercraft on the surface is not.
#[tokio::test]
async fn artillery_water_depth_excludes_submerged_hulls_but_not_hovercraft() {
    for depth in [3, 4] {
        let (_dir, config, mut world, _map, ids) = fixture(VehicleMovement::Tracked).await;
        let map = world.create(&config, "Water".into(), Kind::Room);
        let row = format!("~{depth}~{depth}~{depth}\n");
        create_battle_map(
            &mut world,
            map,
            "water",
            MapAsset::from_cells(&format!("3 3\n{}", row.repeat(3))).unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
        let hover = world.create(&config, "Hover target".into(), Kind::Thing);
        world.objects.get_mut(&hover).unwrap().home = Some(ObjectId(config.home()));
        let template = crate::support::templates::demolisher(VehicleMovement::Hover);
        create_battle_vehicle(&mut world, hover, template).unwrap();
        support::seed_object_dice(&mut world, hover, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, ids[0], map, 1, 1).unwrap();
        place_battle_unit(&mut world, hover, map, 1, 1).unwrap();
        let original = world.btech.vehicles()[&ids[0]].clone();
        let mut flight = approaching(&mut world, map, ArtilleryMode::Standard);
        let report = advance_artillery_flight(&mut world, map, &mut flight, rules())
            .unwrap()
            .unwrap();
        let targets: Vec<_> = report.hits.iter().map(|hit| hit.unit).collect();
        assert_eq!(targets, vec![hover]);
        assert_eq!(world.btech.vehicles()[&ids[0]], original);
        world.validate(&config).unwrap();
    }
}
