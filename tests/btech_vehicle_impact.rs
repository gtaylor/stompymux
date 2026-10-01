//! Vehicle hit routing, motive effects and protection changes commit as one replayable impact.
use crate::support;
use stompymux_rs::*;

/// A running vehicle with a present pilot, initially disconnected from a session.
async fn fixture(template: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Test field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("test", template).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

/// Find a deterministic stream for a specific multi-stage explosion path.
fn matching_seed(predicate: impl Fn(&mut BattleDice) -> bool) -> [u8; 32] {
    for value in 0u32..100000 {
        let mut seed = [0; 32];
        seed[..4].copy_from_slice(&value.to_le_bytes());
        if predicate(&mut BattleDice::seeded(seed)) {
            return seed;
        }
    }
    panic!("No deterministic stream matched");
}

/// Change only the victim's random stream.
fn set_seed(world: &mut World, id: ObjectId, seed: [u8; 32]) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded(seed)).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
}

/// Explicit policy for the hit table and critical consequences.
fn rules(table: BattleVehicleCriticalTable) -> BattleVehicleImpactRules {
    BattleVehicleImpactRules {
        advanced_fire: false,
        criticals: BattleVehicleCriticalRules {
            rotor_damage_divisor: 0,
            extended_piloting: false,
            vtol_table: None,
            table,
            enabled: false,
            combat_safe: false,
            toughness: false,
        },
        hit: BattleVehicleHitRules {
            critical_mode: 1,
            critical_level: 40,
        },
    }
}

#[test]
fn advanced_locations_cover_all_arcs_and_turretless_fallbacks() {
    use BattleVehicleSection as S;
    let intact = BattleVehicle::new(
        BattleVehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
            .unwrap(),
    )
    .unwrap();
    let mut lost = intact.clone();
    lost.damage_phase(S::Turret, 8, BattleDamagePhase::Internal)
        .unwrap();
    for (arc, row) in [
        (
            BattleHitArc::Front,
            [
                S::Front,
                S::Front,
                S::Front,
                S::Right,
                S::Front,
                S::Front,
                S::Front,
                S::Left,
                S::Left,
                S::Left,
                S::Left,
            ],
        ),
        (
            BattleHitArc::Rear,
            [
                S::Rear,
                S::Rear,
                S::Rear,
                S::Left,
                S::Rear,
                S::Rear,
                S::Rear,
                S::Right,
                S::Right,
                S::Right,
                S::Right,
            ],
        ),
        (
            BattleHitArc::Left,
            [
                S::Left,
                S::Left,
                S::Left,
                S::Front,
                S::Left,
                S::Left,
                S::Left,
                S::Rear,
                S::Left,
                S::Left,
                S::Left,
            ],
        ),
        (
            BattleHitArc::Right,
            [
                S::Right,
                S::Right,
                S::Right,
                S::Front,
                S::Right,
                S::Right,
                S::Right,
                S::Rear,
                S::Right,
                S::Right,
                S::Right,
            ],
        ),
    ] {
        for roll in 2..=12 {
            for turret in [false, true] {
                let vehicle = if turret { &intact } else { &lost };
                let mut dice = BattleDice::seeded([9; 32]);
                let hit = vehicle.advanced_hit(arc, roll, 1, 40, &mut dice).unwrap();
                assert_eq!(
                    hit.section,
                    if turret && roll >= 10 {
                        S::Turret
                    } else {
                        row[usize::from(roll - 2)]
                    }
                );
                assert_eq!(
                    hit.through_armor_critical,
                    matches!(roll, 2 | 12)
                        || (roll == 8 && matches!(arc, BattleHitArc::Left | BattleHitArc::Right))
                );
                assert_eq!(hit.motive_roll, None);
                assert_eq!(dice.d6(), BattleDice::seeded([9; 32]).d6());
            }
        }
        assert_eq!(intact.critical_proof_hit(arc, 12).unwrap().section, row[0]);
    }
    assert!(
        intact
            .advanced_hit(
                BattleHitArc::Front,
                1,
                1,
                40,
                &mut BattleDice::seeded([0; 32])
            )
            .is_err()
    );
}

#[test]
fn advanced_motive_rolls_apply_class_modifiers_and_critical_immunity() {
    for (movement, modifier) in [("track", 0), ("wheel", 2), ("hover", 4)] {
        let text = include_str!("../game/mechs/Demolisher.toml")
            .replace(
                "movement = \"track\"",
                &format!("movement = \"{movement}\""),
            )
            .replace("armor = 40", "armor = 0")
            .replace("max_speed = 53.75", "max_speed = 86.0");
        let vehicle =
            BattleVehicle::new(BattleVehicleTemplate::parse("test", &text).unwrap()).unwrap();
        for roll in 2..=12 {
            let stream = matching_seed(|dice| dice.two_d6() == roll);
            let mut dice = BattleDice::seeded(stream);
            let hit = vehicle
                .advanced_hit(BattleHitArc::Front, 3, 1, 40, &mut dice)
                .unwrap();
            assert_eq!(hit.motive_roll, Some(roll));
            let adjusted = roll + modifier;
            assert_eq!(
                hit.piloting_penalty,
                match adjusted {
                    8 | 9 => 1,
                    10 | 11 => 2,
                    _ => 0,
                }
            );
            assert_eq!(
                hit.motive,
                match adjusted {
                    10 | 11 => Some(BattleVehicleMotiveHit::SpeedLoss { movement_points: 1 }),
                    12.. => Some(BattleVehicleMotiveHit::Immobilize),
                    _ => None,
                }
            );
            let mut expected = BattleDice::seeded(stream);
            expected.two_d6();
            assert_eq!(dice.d6(), expected.d6());
        }
        let text = text.replace(
            "\"ICEEngine_Tech\"",
            "\"ICEEngine_Tech\", \"CritProof_Tech\"",
        );
        let immune =
            BattleVehicle::new(BattleVehicleTemplate::parse("test", &text).unwrap()).unwrap();
        let mut dice = BattleDice::seeded([9; 32]);
        assert_eq!(
            immune
                .advanced_hit(BattleHitArc::Front, 3, 2, 100, &mut dice)
                .unwrap()
                .motive_roll,
            None
        );
        assert_eq!(dice.d6(), BattleDice::seeded([9; 32]).d6());
    }
}

#[tokio::test]
async fn complete_impacts_apply_hit_effects_and_damage_with_saved_replay() {
    use BattleVehicleCriticalTable as T;
    for (table, arc, roll, section, loss, locked) in [
        (
            T::Standard,
            BattleHitArc::Left,
            7,
            BattleVehicleSection::Left,
            0.0,
            false,
        ),
        (
            T::Advanced,
            BattleHitArc::Front,
            5,
            BattleVehicleSection::Right,
            0.0,
            false,
        ),
    ] {
        let (_dir, config, mut world, id) =
            fixture(include_str!("../game/mechs/Demolisher.toml")).await;
        let stream = matching_seed(|dice| {
            let first = dice.two_d6();
            let selected = if table == T::Standard {
                first
            } else {
                dice.two_d6()
            };
            selected == roll
        });
        set_seed(&mut world, id, stream);
        let before = world.btech.vehicles()[&id].clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let report =
            resolve_battle_vehicle_impact(&mut world, id, arc, 1, None, rules(table)).unwrap();
        assert_eq!(
            report,
            resolve_battle_vehicle_impact(&mut restored, id, arc, 1, None, rules(table)).unwrap()
        );
        assert_eq!(world.btech, restored.btech);
        assert_eq!(report.hit.unwrap().section, section);
        assert_eq!(
            world.btech.vehicles()[&id].sections()[&section].armor,
            before.sections()[&section].armor - 1
        );
        assert_eq!(
            world.btech.vehicles()[&id].maximum_speed(),
            before.maximum_speed() - loss
        );
        assert_eq!(world.btech.vehicles()[&id].turret_locked(), locked);
        let mut dice = BattleDice::seeded(stream);
        dice.two_d6();
        if table != T::Standard {
            dice.two_d6();
        }
        dice.two_d6();
        if locked {
            dice.two_d6();
        }
        assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            world.btech,
            persistence::load(&config.database()).await.unwrap().btech
        );
    }
}

#[tokio::test]
async fn hull_impacts_preserve_occupants_and_combat_safety_preserves_material() {
    let (_dir, _config, mut world, id) =
        fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    let stream = matching_seed(|dice| dice.two_d6() == 3);
    set_seed(&mut world, id, stream);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let before = world.btech.clone();
    let report = resolve_battle_vehicle_impact(
        &mut world,
        id,
        BattleHitArc::Left,
        40,
        None,
        rules(BattleVehicleCriticalTable::Standard),
    )
    .unwrap();
    assert!(report.damage.unwrap().unit_destroyed);
    assert!(!world.btech.vehicles()[&id].crew_killed());
    assert_eq!(world.objects[&ObjectId(1)].location, Some(id));
    world.btech = before.clone();
    assert!(
        resolve_battle_vehicle_impact(
            &mut world,
            id,
            BattleHitArc::Left,
            0,
            None,
            rules(BattleVehicleCriticalTable::Standard)
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
    let sections = world.btech.vehicles()[&id].sections().clone();
    let mut policy = rules(BattleVehicleCriticalTable::Standard);
    policy.criticals.combat_safe = true;
    let report =
        resolve_battle_vehicle_impact(&mut world, id, BattleHitArc::Left, 40, None, policy)
            .unwrap();
    assert!(report.hit.is_none());
    assert!(report.damage.is_none());
    assert!(report.notices.is_empty());
    assert_eq!(world.btech.vehicles()[&id].sections(), &sections);
    assert_eq!(world.btech.vehicles()[&id].motive_speed_loss(), 0.0);
    let mut dice = BattleDice::seeded(stream);
    dice.two_d6();
    dice.two_d6();
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
}

#[tokio::test]
async fn advanced_motive_impacts_commit_steering_speed_and_penetration_together() {
    let text = include_str!("../game/mechs/Demolisher.toml").replace("armor = 40", "armor = 0");
    let (_dir, config, mut world, id) = fixture(&text).await;
    let stream = matching_seed(|dice| {
        dice.two_d6();
        if dice.two_d6() != 3 || dice.two_d6() != 11 {
            return false;
        }
        dice.two_d6();
        dice.two_d6() < 8
    });
    set_seed(&mut world, id, stream);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["motion"]["speed"] = 10.into();
    state["vehicles"][id.0.to_string()]["motion"]["desired_speed"] = 10.into();
    world.btech = serde_json::from_value(state).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    let policy = rules(BattleVehicleCriticalTable::Advanced);
    let report =
        resolve_battle_vehicle_impact(&mut world, id, BattleHitArc::Front, 1, None, policy)
            .unwrap();
    assert_eq!(
        report,
        resolve_battle_vehicle_impact(&mut restored, id, BattleHitArc::Front, 1, None, policy)
            .unwrap()
    );
    assert_eq!(world.btech, restored.btech);
    assert_eq!(report.hit.unwrap().motive_roll, Some(11));
    assert_eq!(world.btech.vehicles()[&id].piloting_damage(), 2);
    assert_eq!(world.btech.vehicles()[&id].maximum_speed(), 43.0);
    assert_eq!(
        world.btech.vehicles()[&id].sections()[&BattleVehicleSection::Front].internal,
        7
    );
    assert!(
        report
            .broadcasts
            .iter()
            .any(|notice| notice.text == "wobbles violently.")
    );
    let mut dice = BattleDice::seeded(stream);
    for _ in 0..5 {
        dice.two_d6();
    }
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), [dice.d6()]);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        world.btech,
        persistence::load(&config.database()).await.unwrap().btech
    );
}
