//! Mixed artillery occupants share blast packets, heat policy, and atomic saved arrivals.
use crate::support;
use stompymux_rs::*;

/// Stable slot order puts a vehicle before a Mech, with a second vehicle in a neighboring cell.
async fn fixture(
    movement: BattleVehicleMovement,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 3]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Artillery field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::parse("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
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
                BattleTemplate::parse(include_str!("../game/mechs/AS7-D")).unwrap(),
            )
            .unwrap();
        } else {
            let mut template =
                BattleVehicleTemplate::parse(include_str!("../game/mechs/Demolisher")).unwrap();
            template.movement = movement;
            create_battle_vehicle(&mut world, id, template).unwrap();
        }
        place_battle_unit(&mut world, id, map, 1, if index == 2 { 0 } else { 1 }).unwrap();
        ids.push(id);
    }
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
fn rules() -> BattleFallRules {
    let mut rules = BattleMovementRules::STANDARD.fall;
    rules.vehicle_impact.criticals.enabled = false;
    rules.vehicle_impact.fasa.critical_mode = 0;
    rules
}

/// Stop just before arrival so persisted replay includes the map's pattern dice.
fn approaching(
    world: &mut World,
    map: ObjectId,
    mode: BattleArtilleryMode,
) -> BattleArtilleryFlight {
    let center = BattleHexCoordinate { x: 1, y: 1 };
    let mut flight =
        BattleArtilleryFlight::new(center, center, BattleWeapon::LongTom, mode, true).unwrap();
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
    let (_dir, config, base, map, ids) = fixture(BattleVehicleMovement::Stationary).await;
    for mode in [BattleArtilleryMode::Standard, BattleArtilleryMode::Cluster] {
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
        if mode == BattleArtilleryMode::Standard {
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
                [4, 4, 2]
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
            let BattleArtilleryEffect::Damage {
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
                    assert!(matches!(impact, BattleBlastImpact::Mech(_)));
                } else {
                    let BattleBlastImpact::Vehicle(impact) = impact else {
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
                assert_eq!(hit.vehicle_heat.as_ref().unwrap().explosion_roll, None);
            }
        }
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn artillery_zero_heat_uses_mobile_explosions_after_all_packets() {
    let (_dir, config, mut world, map, ids) = fixture(BattleVehicleMovement::Tracked).await;
    let initial = (0u32..100000)
        .find_map(|number| {
            let mut bytes = [0; 32];
            bytes[..4].copy_from_slice(&number.to_le_bytes());
            let initial = BattleDice::seeded(bytes);
            let mut dice = initial.clone();
            for _ in 0..4 {
                dice.two_d6();
                dice.two_d6();
            }
            (dice.two_d6() == 9).then_some(initial)
        })
        .unwrap();
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["vehicles"][ids[0].0.to_string()]["dice"] = serde_json::to_value(initial).unwrap();
    world.btech = serde_json::from_value(encoded).unwrap();
    let mut flight = approaching(&mut world, map, BattleArtilleryMode::Standard);
    let report = advance_artillery_flight(&mut world, map, &mut flight, rules())
        .unwrap()
        .unwrap();
    let hit = &report.hits[0];
    assert_eq!(hit.impacts.len(), 4);
    let heat = hit.vehicle_heat.as_ref().unwrap();
    assert_eq!(heat.explosion_roll, Some(9));
    assert!(heat.explosion.is_some());
    assert!(
        world.btech.vehicles()[&ids[0]]
            .sections()
            .values()
            .all(|section| section.internal == 0)
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn later_vehicle_character_guard_restores_artillery_world_and_cursor() {
    let (_dir, config, mut world, map, ids) = fixture(BattleVehicleMovement::Stationary).await;
    world
        .objects
        .get_mut(&ids[2])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let mut flight = approaching(&mut world, map, BattleArtilleryMode::Standard);
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
    let (_dir, config, mut world, map, ids) = fixture(BattleVehicleMovement::Stationary).await;
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ids[0]);
    assign_battle_pilot(&mut world, ids[0], ObjectId(1)).unwrap();
    start_battle_unit(&mut world, ids[0], ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    let mut flight = approaching(&mut world, map, BattleArtilleryMode::Standard);
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

/// Artillery's below-surface cutoff is exclusive and measured from the water surface.
#[tokio::test]
async fn artillery_water_depth_excludes_submerged_hulls_but_not_hovercraft() {
    for depth in [3, 4] {
        let (_dir, config, mut world, _map, ids) = fixture(BattleVehicleMovement::Tracked).await;
        let map = world.create(&config, "Water".into(), Kind::Room);
        let row = format!("~{depth}~{depth}~{depth}\n");
        create_battle_map(
            &mut world,
            map,
            "water",
            BattleMapAsset::parse(&format!("3 3\n{}", row.repeat(3))).unwrap(),
        )
        .unwrap();
        let hover = world.create(&config, "Hover target".into(), Kind::Thing);
        world.objects.get_mut(&hover).unwrap().home = Some(ObjectId(config.home()));
        let mut template =
            BattleVehicleTemplate::parse(include_str!("../game/mechs/Demolisher")).unwrap();
        template.movement = BattleVehicleMovement::Hover;
        create_battle_vehicle(&mut world, hover, template).unwrap();
        place_battle_unit(&mut world, ids[0], map, 1, 1).unwrap();
        place_battle_unit(&mut world, hover, map, 1, 1).unwrap();
        let original = world.btech.vehicles()[&ids[0]].clone();
        let mut flight = approaching(&mut world, map, BattleArtilleryMode::Standard);
        let report = advance_artillery_flight(&mut world, map, &mut flight, rules())
            .unwrap()
            .unwrap();
        let targets: Vec<_> = report.hits.iter().map(|hit| hit.unit).collect();
        assert_eq!(
            targets,
            if depth == 3 {
                vec![ids[0], hover]
            } else {
                vec![hover]
            }
        );
        if depth == 4 {
            assert_eq!(world.btech.vehicles()[&ids[0]], original);
        }
        world.validate(&config).unwrap();
    }
}

/// Rear selection persists across a front-facing Mech to a later vehicle in the same blast cell.
#[tokio::test]
async fn blast_rear_selection_redirects_later_vehicle_faces_and_preserves_dice() {
    let (_dir, config, base, map, ids) = fixture(BattleVehicleMovement::Stationary).await;
    for artillery in [false, true] {
        for salvage in [false, true] {
            for table in [
                BattleVehicleCriticalTable::Standard,
                BattleVehicleCriticalTable::Fasa,
                BattleVehicleCriticalTable::Advanced,
            ] {
                let mut world = base.clone();
                place_battle_unit(&mut world, ids[2], map, 1, 1).unwrap();
                let packets = if artillery { 4 } else { 1 };
                let (initial, mut expected) = (0u32..100000)
                    .find_map(|number| {
                        let mut bytes = [0; 32];
                        bytes[..4].copy_from_slice(&number.to_le_bytes());
                        let initial = BattleDice::seeded(bytes);
                        let mut dice = initial.clone();
                        for _ in 0..packets {
                            if table != BattleVehicleCriticalTable::Standard {
                                dice.two_d6();
                            }
                            if dice.two_d6() != 7 {
                                return None;
                            }
                            dice.two_d6();
                            if !salvage {
                                dice.two_d6();
                            }
                        }
                        Some((initial, dice))
                    })
                    .unwrap();
                let mut saved = serde_json::to_value(&world.btech).unwrap();
                saved["vehicles"][ids[0].0.to_string()]["motion"]["heading"] = 0.0.into();
                saved["vehicles"][ids[0].0.to_string()]["motion"]["desired_heading"] = 0.0.into();
                saved["constructed"][ids[1].0.to_string()]["motion"]["heading"] = 180.0.into();
                saved["vehicles"][ids[2].0.to_string()]["motion"]["heading"] = 180.0.into();
                saved["vehicles"][ids[2].0.to_string()]["motion"]["desired_heading"] = 180.0.into();
                saved["vehicles"][ids[2].0.to_string()]["dice"] =
                    serde_json::to_value(initial).unwrap();
                if salvage {
                    saved["vehicles"][ids[2].0.to_string()]["definition"]["attributes"]["specials"] =
                        "SalvageTech".into();
                }
                world.btech = serde_json::from_value(saved).unwrap();
                let original = world.btech.vehicles()[&ids[2]].clone();
                let mut policy = rules();
                policy.vehicle_impact.criticals.table = table;
                let impacts = if artillery {
                    let mut flight = approaching(&mut world, map, BattleArtilleryMode::Standard);
                    let report = advance_artillery_flight(&mut world, map, &mut flight, policy)
                        .unwrap()
                        .unwrap();
                    assert_eq!(
                        report.hits.iter().map(|hit| hit.unit).collect::<Vec<_>>(),
                        ids
                    );
                    report.hits[2].impacts.clone()
                } else {
                    set_minefield(
                        &mut world,
                        map,
                        0,
                        Some(BattleMinefield {
                            coordinate: BattleHexCoordinate { x: 1, y: 1 },
                            kind: BattleMineKind::Standard,
                            strength: 1,
                            extra: 0,
                            owner: ObjectId(1),
                        }),
                    )
                    .unwrap();
                    resolve_mine_blast(&mut world, map, 0, policy).unwrap().hits[2]
                        .impacts
                        .clone()
                };
                assert_eq!(impacts.len(), packets);
                for impact in impacts {
                    let BattleBlastImpact::Vehicle(impact) = impact else {
                        panic!("Vehicle packet expected")
                    };
                    assert_eq!(impact.hit.unwrap().section, BattleVehicleSection::Front);
                    let damage = impact.damage.unwrap();
                    assert_eq!(damage.section, BattleVehicleSection::Rear);
                    assert_eq!(damage.rolls.len(), if salvage { 1 } else { 2 });
                }
                let unit = &world.btech.vehicles()[&ids[2]];
                assert_eq!(
                    unit.sections()[&BattleVehicleSection::Front],
                    original.sections()[&BattleVehicleSection::Front]
                );
                assert_eq!(
                    original.sections()[&BattleVehicleSection::Rear].armor
                        - unit.sections()[&BattleVehicleSection::Rear].armor,
                    if artillery { 20 } else { 1 }
                );
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                let next = expected.d6();
                assert_eq!(roll_unit_dice(&mut world, ids[2], 1).unwrap(), [next]);
                assert_eq!(roll_unit_dice(&mut restored, ids[2], 1).unwrap(), [next]);
                assert_eq!(world.btech, restored.btech);
                world.validate(&config).unwrap();
            }
        }
    }
}

#[tokio::test]
async fn character_artillery_heat_evacuates_atomically_with_the_flight_cursor() {
    use std::{cell::RefCell, rc::Rc};
    let (_dir, config, mut world, map, ids) = fixture(BattleVehicleMovement::Tracked).await;
    let id = ids[0];
    let seed = (0u32..100_000)
        .find_map(|value| {
            let mut seed = [0; 32];
            seed[..4].copy_from_slice(&value.to_le_bytes());
            let mut dice = BattleDice::seeded(seed);
            for _ in 0..4 {
                dice.two_d6();
                dice.two_d6();
            }
            (dice.two_d6() == 9).then_some(seed)
        })
        .unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded(seed)).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
    let mut flight = approaching(&mut world, map, BattleArtilleryMode::Standard);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let before = scripts.world().clone();
    let cursor = flight.clone();
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    scripts.world_mut().objects.remove(&afterlife);
    assert!(advance_artillery_flight_action(&scripts, &config, map, &mut flight, rules()).is_err());
    assert_eq!(flight, cursor);
    assert_eq!(scripts.world().btech, before.btech);
    *scripts.world_mut() = before;
    let report = advance_artillery_flight_action(&scripts, &config, map, &mut flight, rules())
        .unwrap()
        .unwrap();
    assert_eq!(report.hits[0].impacts.len(), 4);
    assert_eq!(
        report.hits[0].vehicle_heat.as_ref().unwrap().explosion_roll,
        Some(9)
    );
    assert!(scripts.world().btech.vehicles()[&id].crew_killed());
    assert_eq!(
        scripts.world().objects[&ObjectId(2)].location,
        Some(afterlife)
    );
    let result = scripts.world().clone();
    persistence::save(&config.database(), &result)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        result.btech
    );
}
