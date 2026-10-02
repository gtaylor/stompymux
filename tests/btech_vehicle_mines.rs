//! Vehicle mine selection shares ordered fields and live-mass thresholds with Mechs.
use crate::support;
use stompymux_rs::*;

#[tokio::test]
async fn vehicle_mine_queries_use_live_mass_and_preserve_saved_state() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Mine field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Demolisher".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
            .unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 1, 1).unwrap();
    for (ordinal, kind, extra) in [
        (9, BattleMineKind::Standard, 0),
        (3, BattleMineKind::Vibra, 80),
        (1, BattleMineKind::Trigger, 0),
        (5, BattleMineKind::Command, 42),
    ] {
        set_minefield(
            &mut world,
            map,
            ordinal,
            Some(BattleMinefield {
                coordinate: BattleHexCoordinate { x: 1, y: 1 },
                kind,
                strength: 80,
                extra,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
    }
    let original = world.btech.clone();
    let selected = mine_activations(&world, id, BattleMineTriggerReason::Step).unwrap();
    assert_eq!(
        selected
            .iter()
            .map(|field| field.ordinal)
            .collect::<Vec<_>>(),
        [1, 3, 5, 9]
    );
    assert_eq!(
        mine_activations(&world, id, BattleMineTriggerReason::Fall)
            .unwrap()
            .iter()
            .map(|field| field.ordinal)
            .collect::<Vec<_>>(),
        [3, 5, 9]
    );
    assert_eq!(world.btech, original);
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["vehicles"][id.0.to_string()]["ammunition"][0] = 4.into();
    world.btech = serde_json::from_value(encoded).unwrap();
    let mass = world.btech.vehicles()[&id].mass().unwrap();
    assert_eq!(mass.total / 1024, 79);
    let before = world.btech.clone();
    let selected = mine_activations(&world, id, BattleMineTriggerReason::Step).unwrap();
    assert_eq!(
        selected
            .iter()
            .map(|field| field.ordinal)
            .collect::<Vec<_>>(),
        [5, 9]
    );
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech.vehicles()[&id].mass().unwrap(), mass);
    assert_eq!(
        mine_activations(&loaded, id, BattleMineTriggerReason::Step).unwrap(),
        selected
    );
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(loaded))).unwrap();
    let total: u32 = scripts
        .eval_callback(&format!(
            "local s=btech.unit.state({}); s.mass.total=0; return btech.unit.state({}).mass.total",
            id.0, id.0
        ))
        .unwrap();
    assert_eq!(total, mass.total);
    assert_eq!(scripts.world().btech, before);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert!(mine_activations(&world, id, BattleMineTriggerReason::Step).is_err());
}

/// Hovercraft float above submerged mines; surface ice and raised dry ground retain their own heights.
#[tokio::test]
async fn mine_queries_share_surface_gates_across_ground_vehicle_types() {
    let (_dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Mine heights".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::from_cells("3 1\n.2~2-2\n").unwrap(),
    )
    .unwrap();
    for x in 0..3 {
        set_minefield(
            &mut world,
            map,
            x as u32,
            Some(BattleMinefield {
                coordinate: BattleHexCoordinate { x, y: 0 },
                kind: BattleMineKind::Standard,
                strength: 5,
                extra: 0,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
    }
    for movement in [
        BattleVehicleMovement::Tracked,
        BattleVehicleMovement::Wheeled,
        BattleVehicleMovement::Hover,
    ] {
        let id = world.create(&config, "Vehicle".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        let mut template = BattleVehicleTemplate::parse(
            "Demolisher",
            include_str!("../game/mechs/Demolisher.toml"),
        )
        .unwrap();
        template.movement = movement;
        create_battle_vehicle(&mut world, id, template).unwrap();
        for x in 0..3 {
            place_battle_unit(&mut world, id, map, x, 0).unwrap();
            let before = world.btech.clone();
            let fields = mine_activations(&world, id, BattleMineTriggerReason::Step).unwrap();
            let above_mines = movement == BattleVehicleMovement::Hover && x == 1;
            assert_eq!(fields.is_empty(), above_mines, "{movement:?} at {x}");
            assert_eq!(world.btech, before);
        }
    }
    world.validate(&config).unwrap();
}

/// Place a vehicle, a Mech and a second vehicle in stable mixed slot order.
async fn blast_fixture(
    movement: BattleVehicleMovement,
) -> (tempfile::TempDir, Config, World, ObjectId, [ObjectId; 3]) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Blast field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::from_cells("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
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
                BattleTemplate::parse("AS7-D", include_str!("../game/mechs/AS7-D.toml")).unwrap(),
            )
            .unwrap();
        } else {
            let mut template = BattleVehicleTemplate::parse(
                "Demolisher",
                include_str!("../game/mechs/Demolisher.toml"),
            )
            .unwrap();
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

/// Keep hit-location diagnostics but exclude critical cascades from packet arithmetic assertions.
fn blast_rules() -> BattleFallRules {
    let mut rules = BattleMovementRules::STANDARD.fall;
    rules.vehicle_impact.criticals.enabled = false;
    rules.vehicle_impact.hit.critical_mode = 0;
    rules
}

#[tokio::test]
async fn mixed_mine_blasts_preserve_packets_slots_fields_and_restart() {
    let (_dir, config, base, map, ids) = blast_fixture(BattleVehicleMovement::Stationary).await;
    for kind in [
        BattleMineKind::Standard,
        BattleMineKind::Command,
        BattleMineKind::Vibra,
    ] {
        for strength in [4, 11] {
            let mut world = base.clone();
            let mine = BattleMinefield {
                coordinate: BattleHexCoordinate { x: 1, y: 1 },
                kind,
                strength,
                extra: 0,
                owner: ObjectId(1),
            };
            set_minefield(&mut world, map, 0, Some(mine)).unwrap();
            set_minefield(
                &mut world,
                map,
                1,
                Some(BattleMinefield {
                    kind: BattleMineKind::Trigger,
                    ..mine
                }),
            )
            .unwrap();
            let before = world.clone();
            let original_armor: u16 = world.btech.vehicles()[&ids[0]]
                .sections()
                .values()
                .map(|section| section.armor)
                .sum();
            let report = resolve_mine_blast(&mut world, map, 0, blast_rules()).unwrap();
            let area = kind != BattleMineKind::Standard;
            assert_eq!(
                report.hits.iter().map(|hit| hit.unit).collect::<Vec<_>>(),
                if area {
                    ids.to_vec()
                } else {
                    ids[..2].to_vec()
                }
            );
            let hit = &report.hits[0];
            assert_eq!(hit.damage, strength as u16);
            assert_eq!(hit.impacts.len(), (strength as usize).div_ceil(5));
            assert!(
                hit.impacts
                    .iter()
                    .all(|impact| matches!(impact, BattleBlastImpact::Vehicle(_)))
            );
            assert!(
                report.hits[1]
                    .impacts
                    .iter()
                    .all(|impact| matches!(impact, BattleBlastImpact::Mech(_)))
            );
            let armor: u16 = world.btech.vehicles()[&ids[0]]
                .sections()
                .values()
                .map(|section| section.armor)
                .sum();
            assert_eq!(original_armor - armor, strength as u16);
            assert_eq!(
                report.removed,
                if area || strength < 5 {
                    vec![0, 1]
                } else {
                    vec![]
                }
            );
            assert!(report.notices.iter().any(|notice| notice.unit == ids[0] && notice.text.contains("points of damage")));
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                resolve_mine_blast(&mut replay, map, 0, blast_rules()).unwrap(),
                report
            );
            assert_eq!(replay.btech, world.btech);
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn inferno_mine_heat_uses_blast_duration_and_vehicle_fire_policy() {
    let (_dir, config, base, map, ids) = blast_fixture(BattleVehicleMovement::Tracked).await;
    for stationary in [false, true] {
        for advanced in [false, true] {
            for roll in [8, 9] {
                let mut world = base.clone();
                // Keep this heat-only blast isolated from the Mech; strength two supplies no shrapnel.
                place_battle_unit(&mut world, ids[1], map, 2, 2).unwrap();
                let seed = (0u32..)
                    .find_map(|seed| {
                        let mut bytes = [0; 32];
                        bytes[..4].copy_from_slice(&seed.to_le_bytes());
                        let dice = BattleDice::seeded(bytes);
                        (dice.clone().two_d6() == roll).then_some(dice)
                    })
                    .unwrap();
                let mut saved = serde_json::to_value(&world.btech).unwrap();
                saved["vehicles"][ids[0].0.to_string()]["dice"] =
                    serde_json::to_value(&seed).unwrap();
                if stationary {
                    saved["units"][ids[0].0.to_string()]["movement_code"] = 10.into();
                    saved["vehicles"][ids[0].0.to_string()]["definition"]["movement"] =
                        "stationary".into();
                }
                world.btech = serde_json::from_value(saved).unwrap();
                set_minefield(
                    &mut world,
                    map,
                    0,
                    Some(BattleMinefield {
                        coordinate: BattleHexCoordinate { x: 1, y: 1 },
                        kind: BattleMineKind::Inferno,
                        strength: 2,
                        extra: 0,
                        owner: ObjectId(1),
                    }),
                )
                .unwrap();
                let before = world.clone();
                let mut rules = blast_rules();
                rules.vehicle_impact.advanced_fire = advanced;
                if stationary {
                    let mut overflow = world.clone();
                    let mut encoded = serde_json::to_value(&overflow.btech).unwrap();
                    encoded["vehicles"][ids[0].0.to_string()]["inferno_remaining"] =
                        i32::MAX.into();
                    overflow.btech = serde_json::from_value(encoded).unwrap();
                    let checkpoint = overflow.btech.clone();
                    assert!(resolve_mine_blast(&mut overflow, map, 0, rules).is_err());
                    assert_eq!(overflow.btech, checkpoint);
                }
                let report = resolve_mine_blast(&mut world, map, 0, rules).unwrap();
                let hit = &report.hits[0];
                assert_eq!(hit.unit, ids[0]);
                assert!(hit.impacts.is_empty());
                let heat = hit.vehicle_heat.as_ref().unwrap();
                let mut dice = seed.clone();
                if stationary {
                    assert_eq!(hit.burn_seconds, 12);
                    assert_eq!(world.btech.vehicles()[&ids[0]].inferno_remaining(), 12);
                    assert!(heat.explosion_roll.is_none() && heat.fire.is_none());
                } else if advanced {
                    assert_eq!(heat.fire.as_ref().unwrap().roll, roll);
                    assert!(heat.explosion_roll.is_none() && heat.explosion.is_none());
                    assert!(
                        world.btech.vehicles()[&ids[0]]
                            .burning_sections()
                            .is_empty()
                    );
                    // Tracked raw eight/nine performs a second motive roll, without forced inferno ignition.
                    dice.two_d6();
                    dice.two_d6();
                } else {
                    assert_eq!(heat.explosion_roll, Some(roll));
                    assert_eq!(heat.explosion.is_some(), roll == 9);
                    assert_eq!(world.btech.vehicles()[&ids[0]].is_destroyed(), roll == 9);
                    dice.two_d6();
                }
                let mut replay = before;
                assert_eq!(
                    resolve_mine_blast(&mut replay, map, 0, rules).unwrap(),
                    report
                );
                assert_eq!(replay.btech, world.btech);
                if stationary || advanced || roll == 8 {
                    assert_eq!(roll_unit_dice(&mut world, ids[0], 1).unwrap(), [dice.d6()]);
                }
                world.validate(&config).unwrap();
            }
        }
    }
}

#[tokio::test]
async fn vehicle_mine_events_and_command_detonation_are_atomic() {
    let (_dir, config, mut world, map, ids) =
        blast_fixture(BattleVehicleMovement::Stationary).await;
    let mine = BattleMinefield {
        coordinate: BattleHexCoordinate { x: 1, y: 1 },
        kind: BattleMineKind::Command,
        strength: 4,
        extra: 42,
        owner: ObjectId(1),
    };
    set_minefield(&mut world, map, 0, Some(mine)).unwrap();
    set_minefield(
        &mut world,
        map,
        1,
        Some(BattleMinefield {
            kind: BattleMineKind::Trigger,
            strength: 0,
            extra: 0,
            ..mine
        }),
    )
    .unwrap();
    let before = world.btech.clone();
    let event = activate_mines(
        &mut world,
        ids[0],
        BattleMineTriggerReason::Step,
        blast_rules(),
    )
    .unwrap();
    assert_eq!(event.triggers, 1);
    assert!(event.blasts.is_empty());
    assert_eq!(event.notices.len(), 1);
    assert_eq!(world.btech, before);
    // The last occupant rejects unsupported character consequences after earlier candidates have taken damage.
    world
        .objects
        .get_mut(&ids[2])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    assert!(detonate_command_mines(&mut world, ids[0], 42, blast_rules()).is_err());
    assert_eq!(world.btech, before);
    world
        .objects
        .get_mut(&ids[2])
        .unwrap()
        .flags
        .remove(Flag::InCharacter);
    let report = detonate_command_mines(&mut world, ids[0], 42, blast_rules()).unwrap();
    assert_eq!(report.blasts.len(), 1);
    assert_eq!(report.blasts[0].hits.len(), 3);
    assert_eq!(report.blasts[0].removed, [0, 1]);
    world.validate(&config).unwrap();
}

/// Combat-safe blasts preserve materials while consuming the selected hit table's diagnostic stream.
#[tokio::test]
async fn mine_vehicle_hit_policy_controls_location_dice_and_safe_damage() {
    let (_dir, config, base, map, ids) = blast_fixture(BattleVehicleMovement::Stationary).await;
    for table in [
        BattleVehicleCriticalTable::Standard,
        BattleVehicleCriticalTable::Advanced,
    ] {
        let mut world = base.clone();
        place_battle_unit(&mut world, ids[1], map, 2, 2).unwrap();
        set_minefield(
            &mut world,
            map,
            0,
            Some(BattleMinefield {
                coordinate: BattleHexCoordinate { x: 1, y: 1 },
                kind: BattleMineKind::Standard,
                strength: 6,
                extra: 0,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
        let seed = BattleDice::seeded([71; 32]);
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["vehicles"][ids[0].0.to_string()]["dice"] = serde_json::to_value(&seed).unwrap();
        world.btech = serde_json::from_value(encoded).unwrap();
        let protection = world.btech.vehicles()[&ids[0]].sections().clone();
        let mut rules = blast_rules();
        rules.vehicle_impact.criticals.table = table;
        rules.vehicle_impact.criticals.combat_safe = true;
        let report = resolve_mine_blast(&mut world, map, 0, rules).unwrap();
        assert_eq!(report.hits.len(), 1);
        assert_eq!(report.hits[0].impacts.len(), 2);
        let mut dice = seed;
        for impact in &report.hits[0].impacts {
            let BattleBlastImpact::Vehicle(impact) = impact else {
                panic!("Wrong target anatomy");
            };
            let mut rolls = vec![dice.two_d6()];
            if table != BattleVehicleCriticalTable::Standard {
                rolls.push(dice.two_d6());
            }
            assert_eq!(impact.rolls, rolls);
            assert!(impact.hit.is_none() && impact.damage.is_none());
            dice.two_d6();
        }
        assert_eq!(world.btech.vehicles()[&ids[0]].sections(), &protection);
        assert_eq!(roll_unit_dice(&mut world, ids[0], 1).unwrap(), [dice.d6()]);
        world.validate(&config).unwrap();
    }
}

/// A fatal hull packet is followed by a wasted hull hit and a hit on the surviving turret.
#[tokio::test]
async fn mine_packets_finish_after_hull_loss_and_retain_wreck_material_damage() {
    let (_dir, config, base, map, ids) = blast_fixture(BattleVehicleMovement::Stationary).await;
    for internal in [false, true] {
        let mut world = base.clone();
        place_battle_unit(&mut world, ids[1], map, 2, 2).unwrap();
        let seed = (0u32..100000)
            .find_map(|number| {
                let mut bytes = [0; 32];
                bytes[..4].copy_from_slice(&number.to_le_bytes());
                let initial = BattleDice::seeded(bytes);
                let mut dice = initial.clone();
                if dice.two_d6() != 7 {
                    return None;
                }
                dice.two_d6();
                if dice.two_d6() > 7 {
                    return None;
                }
                if dice.two_d6() > 9 {
                    return None;
                }
                dice.two_d6();
                matches!(dice.two_d6(), 10 | 11).then_some(initial)
            })
            .unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut saved["vehicles"][ids[0].0.to_string()];
        unit["sections"]["front"]["armor"] = 0.into();
        unit["sections"]["front"]["internal"] = 1.into();
        if internal {
            unit["sections"]["turret"]["armor"] = 0.into();
        }
        unit["motion"]["heading"] = 180.0.into();
        unit["motion"]["desired_heading"] = 180.0.into();
        unit["dice"] = serde_json::to_value(&seed).unwrap();
        world.btech = serde_json::from_value(saved).unwrap();
        set_minefield(
            &mut world,
            map,
            0,
            Some(BattleMinefield {
                coordinate: BattleHexCoordinate { x: 1, y: 1 },
                kind: BattleMineKind::Standard,
                strength: 11,
                extra: 0,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        let report = resolve_mine_blast(&mut world, map, 0, blast_rules()).unwrap();
        assert_eq!(
            resolve_mine_blast(&mut replay, map, 0, blast_rules()).unwrap(),
            report
        );
        assert_eq!(replay.btech, world.btech);
        assert_eq!(report.hits.len(), 1);
        let packets = &report.hits[0].impacts;
        assert_eq!(packets.len(), 3);
        let BattleBlastImpact::Vehicle(first) = &packets[0] else {
            panic!("Wrong anatomy");
        };
        assert!(first.damage.as_ref().unwrap().unit_destroyed);
        let BattleBlastImpact::Vehicle(wasted) = &packets[1] else {
            panic!("Wrong anatomy");
        };
        let wasted = wasted.damage.as_ref().unwrap();
        assert_eq!((wasted.absorbed, wasted.overflow), (0, 5));
        assert_eq!(wasted.rolls.len(), 1);
        assert!(
            wasted.internal.is_none() && wasted.notices.is_empty() && wasted.criticals.is_empty()
        );
        let BattleBlastImpact::Vehicle(last) = &packets[2] else {
            panic!("Wrong anatomy");
        };
        let last = last.damage.as_ref().unwrap();
        assert_eq!(last.section, BattleVehicleSection::Turret);
        if internal {
            assert_eq!(last.internal.as_ref().unwrap().absorbed, 1);
            assert_eq!(
                world.btech.vehicles()[&ids[0]].sections()[&BattleVehicleSection::Turret].internal,
                7
            );
        } else {
            assert_eq!(last.absorbed, 1);
            assert_eq!(
                world.btech.vehicles()[&ids[0]].sections()[&BattleVehicleSection::Turret].armor,
                39
            );
        }
        let mut dice = seed;
        for _ in 0..if internal { 8 } else { 7 } {
            dice.two_d6();
        }
        let mut probe = world.clone();
        assert_eq!(
            roll_unit_dice(&mut probe, ids[0], 1).unwrap(),
            [dice.clone().d6()]
        );
        // Wrecks remain in the mine footprint; ordinary new attacks still reject them without mutation.
        let checkpoint = world.btech.clone();
        assert!(
            resolve_battle_vehicle_impact(
                &mut world,
                ids[0],
                BattleHitArc::Front,
                1,
                None,
                blast_rules().vehicle_impact
            )
            .is_err()
        );
        assert_eq!(world.btech, checkpoint);
        let again = resolve_mine_blast(&mut world, map, 0, blast_rules()).unwrap();
        assert_eq!(again.hits.len(), 1);
        assert_eq!(again.hits[0].impacts.len(), 3);
        if !internal {
            for _ in 0..6 {
                dice.two_d6();
            }
            assert_eq!(roll_unit_dice(&mut world, ids[0], 1).unwrap(), [dice.d6()]);
        }
        world.validate(&config).unwrap();
    }
}

/// Every hit table re-evaluates the remaining anatomy between packets, including after destruction.
#[tokio::test]
async fn mine_followups_reselect_locations_after_turret_and_hull_loss() {
    let (_dir, config, base, map, ids) = blast_fixture(BattleVehicleMovement::Stationary).await;
    for table in [
        BattleVehicleCriticalTable::Standard,
        BattleVehicleCriticalTable::Advanced,
    ] {
        let mut world = base.clone();
        place_battle_unit(&mut world, ids[1], map, 2, 2).unwrap();
        let seed = (0u32..1000000)
            .find_map(|number| {
                let mut bytes = [0; 32];
                bytes[..4].copy_from_slice(&number.to_le_bytes());
                let initial = BattleDice::seeded(bytes);
                let mut dice = initial.clone();
                for (index, wanted) in [10, 7, 7, 7].into_iter().enumerate() {
                    if table != BattleVehicleCriticalTable::Standard {
                        dice.two_d6();
                    }
                    if dice.two_d6() != wanted {
                        return None;
                    }
                    dice.two_d6();
                    if index < 2 && dice.two_d6() > 7 {
                        return None;
                    }
                }
                Some(initial)
            })
            .unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut saved["vehicles"][ids[0].0.to_string()];
        for section in ["turret", "front"] {
            unit["sections"][section]["armor"] = 0.into();
            unit["sections"][section]["internal"] = 1.into();
        }
        unit["motion"]["heading"] = 180.0.into();
        unit["motion"]["desired_heading"] = 180.0.into();
        unit["dice"] = serde_json::to_value(&seed).unwrap();
        world.btech = serde_json::from_value(saved).unwrap();
        set_minefield(
            &mut world,
            map,
            0,
            Some(BattleMinefield {
                coordinate: BattleHexCoordinate { x: 1, y: 1 },
                kind: BattleMineKind::Standard,
                strength: 16,
                extra: 0,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
        let mut policy = blast_rules();
        policy.vehicle_impact.criticals.table = table;
        let report = resolve_mine_blast(&mut world, map, 0, policy).unwrap();
        let packets: Vec<_> = report.hits[0]
            .impacts
            .iter()
            .map(|impact| {
                let BattleBlastImpact::Vehicle(impact) = impact else {
                    panic!("Wrong anatomy");
                };
                impact
            })
            .collect();
        assert_eq!(packets.len(), 4);
        assert_eq!(
            packets
                .iter()
                .map(|packet| packet.hit.unwrap().section)
                .collect::<Vec<_>>(),
            [
                BattleVehicleSection::Turret,
                BattleVehicleSection::Front,
                BattleVehicleSection::Front,
                BattleVehicleSection::Front
            ]
        );
        assert!(!packets[0].damage.as_ref().unwrap().unit_destroyed);
        assert!(packets[1].damage.as_ref().unwrap().unit_destroyed);
        for packet in &packets[2..] {
            assert_eq!(packet.damage.as_ref().unwrap().absorbed, 0);
            assert!(packet.damage.as_ref().unwrap().internal.is_none());
        }
        assert!(
            world.btech.vehicles()[&ids[0]]
                .ammunition()
                .iter()
                .all(|rounds| *rounds == 0)
        );
        let mut dice = seed;
        for _ in 0..if table == BattleVehicleCriticalTable::Standard {
            10
        } else {
            14
        } {
            dice.two_d6();
        }
        assert_eq!(roll_unit_dice(&mut world, ids[0], 1).unwrap(), [dice.d6()]);
        world.validate(&config).unwrap();
    }
}

/// Both ordinary and Inferno blasts run their heat response after material damage on wrecks.
#[tokio::test]
async fn mine_heat_explodes_remaining_wreck_sections() {
    let (_dir, config, base, map, ids) = blast_fixture(BattleVehicleMovement::Tracked).await;
    for inferno in [false, true] {
        let mut world = base.clone();
        place_battle_unit(&mut world, ids[1], map, 2, 2).unwrap();
        let _ = damage_battle_vehicle_phase(
            &mut world,
            ids[0],
            BattleVehicleSection::Front,
            100,
            BattleDamagePhase::Internal,
        )
        .unwrap();
        let dice = (0u32..100000)
            .find_map(|number| {
                let mut bytes = [0; 32];
                bytes[..4].copy_from_slice(&number.to_le_bytes());
                let initial = BattleDice::seeded(bytes);
                let mut dice = initial.clone();
                if !inferno {
                    dice.two_d6();
                    dice.two_d6();
                }
                (dice.two_d6() == 9).then_some(initial)
            })
            .unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved["vehicles"][ids[0].0.to_string()]["dice"] = serde_json::to_value(dice).unwrap();
        world.btech = serde_json::from_value(saved).unwrap();
        set_minefield(
            &mut world,
            map,
            0,
            Some(BattleMinefield {
                coordinate: BattleHexCoordinate { x: 1, y: 1 },
                kind: if inferno {
                    BattleMineKind::Inferno
                } else {
                    BattleMineKind::Standard
                },
                strength: if inferno { 2 } else { 1 },
                extra: 0,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let report = resolve_mine_blast(&mut world, map, 0, blast_rules()).unwrap();
        assert_eq!(
            resolve_mine_blast(&mut restored, map, 0, blast_rules()).unwrap(),
            report
        );
        assert_eq!(world.btech, restored.btech);
        let hit = &report.hits[0];
        assert_eq!(hit.unit, ids[0]);
        assert_eq!(hit.impacts.len(), usize::from(!inferno));
        let heat = hit.vehicle_heat.as_ref().unwrap();
        assert_eq!(heat.explosion_roll, Some(9));
        assert_eq!(heat.explosion.as_ref().unwrap().destroyed_sections.len(), 4);
        assert!(
            world.btech.vehicles()[&ids[0]]
                .sections()
                .values()
                .all(|section| section.internal == 0)
        );
        world.validate(&config).unwrap();
    }
}

/// Mixed-unit command blasts preserve packet feedback without disclosing pilot rolls to passengers.
#[tokio::test]
async fn command_blast_feedback_is_private_ordered_and_replayable() {
    let (_dir, config, mut world, map, ids) = blast_fixture(BattleVehicleMovement::Tracked).await;
    let pilot = ObjectId(1);
    world.objects.get_mut(&pilot).unwrap().location = Some(ids[1]);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(&mut world, ids[1], pilot).unwrap();
    start_battle_unit(&mut world, ids[1], pilot, true).unwrap();
    for _ in 0..30 {
        advance_battle_units(&mut world, 0);
    }
    let passenger = world.create(&config, "Passenger".into(), Kind::Player);
    world.objects.get_mut(&passenger).unwrap().location = Some(ids[1]);
    world
        .objects
        .get_mut(&passenger)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    set_minefield(
        &mut world,
        map,
        0,
        Some(BattleMinefield {
            coordinate: BattleHexCoordinate { x: 1, y: 1 },
            kind: BattleMineKind::Command,
            strength: 150,
            extra: 42,
            owner: pilot,
        }),
    )
    .unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    for id in [pilot, passenger] {
        restored
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::Connected);
    }
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let replay =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    let report =
        detonate_command_mines_action(&scripts, &config, ids[0], 42, blast_rules()).unwrap();
    let repeated =
        detonate_command_mines_action(&replay, &config, ids[0], 42, blast_rules()).unwrap();
    assert_eq!(report, repeated);
    assert_eq!(scripts.world().btech, replay.world().btech);
    let output = scripts.drain_outbox();
    assert_eq!(output, replay.drain_outbox());
    assert!(
        !report.pilot_notices.is_empty(),
        "the blast must cause an actual pilot check"
    );
    let actual: Vec<_> = output
        .iter()
        .filter(|(_, message)| {
            message.source() == "You make a piloting skill roll!"
                || message.source().starts_with("Modified Pilot Skill:")
        })
        .map(|(who, message)| (*who, message.source().to_owned()))
        .collect();
    assert_eq!(
        actual,
        report
            .pilot_notices
            .iter()
            .map(|notice| (notice.pilot, notice.text.clone()))
            .collect::<Vec<_>>()
    );
    assert!(actual.iter().all(|(who, _)| *who == pilot));
    assert!(output.iter().any(|(who, _)| *who == passenger));
    assert!(
        report
            .pilot_notices
            .iter()
            .all(|notice| notice.before_notice > 0)
    );
    let direct = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let blast = resolve_mine_blast_action(&direct, &config, map, 0, blast_rules()).unwrap();
    assert_eq!(blast, report.blasts[0]);
    let direct_output = direct.drain_outbox();
    let direct_checks: Vec<_> = direct_output
        .iter()
        .filter(|(_, message)| {
            message.source() == "You make a piloting skill roll!"
                || message.source().starts_with("Modified Pilot Skill:")
        })
        .map(|(who, message)| (*who, message.source().to_owned()))
        .collect();
    assert_eq!(actual, direct_checks);
}
