//! Standard vehicle hit tables, turret loss, and armor-dependent critical dice boundaries.
use stompymux_rs::{
    BattleDice, BattleHitArc as Arc, BattleVehicle, BattleVehicleSection as S,
    BattleVehicleTemplate,
};

/// Normalize armor to percentages while preserving valid vehicle anatomy and equipment.
fn vehicle(armor: u16) -> BattleVehicle {
    let vehicle = BattleVehicle::new(
        BattleVehicleTemplate::parse(include_str!("../game/mechs/Demolisher")).unwrap(),
    )
    .unwrap();
    let mut state = serde_json::to_value(vehicle).unwrap();
    for section in ["left", "right", "front", "rear", "turret"] {
        state["definition"]["sections"][section]["armor"] = 100.into();
        state["sections"][section]["armor"] = armor.into();
    }
    serde_json::from_value(state).unwrap()
}

#[test]
fn standard_vehicle_locations_cover_every_arc_roll_and_turret_state() {
    let intact = vehicle(70);
    let mut lost = intact.clone();
    lost.damage_phase(S::Turret, 100, stompymux_rs::BattleDamagePhase::Internal)
        .unwrap();
    let turretless = BattleVehicle::new(
        BattleVehicleTemplate::parse(include_str!("../game/mechs/Flatbed_Truck")).unwrap(),
    )
    .unwrap();
    for (arc, hull, row) in [
        (
            Arc::Front,
            S::Front,
            [
                S::Front,
                S::Front,
                S::Front,
                S::Front,
                S::Front,
                S::Front,
                S::Front,
                S::Front,
                S::Turret,
                S::Turret,
                S::Turret,
            ],
        ),
        (
            Arc::Rear,
            S::Rear,
            [
                S::Rear,
                S::Rear,
                S::Rear,
                S::Rear,
                S::Rear,
                S::Rear,
                S::Rear,
                S::Rear,
                S::Turret,
                S::Turret,
                S::Turret,
            ],
        ),
        (
            Arc::Left,
            S::Left,
            [
                S::Left,
                S::Left,
                S::Left,
                S::Left,
                S::Left,
                S::Left,
                S::Left,
                S::Left,
                S::Turret,
                S::Turret,
                S::Left,
            ],
        ),
        (
            Arc::Right,
            S::Right,
            [
                S::Right,
                S::Right,
                S::Right,
                S::Right,
                S::Right,
                S::Right,
                S::Right,
                S::Right,
                S::Turret,
                S::Turret,
                S::Right,
            ],
        ),
    ] {
        for roll in 2..=12 {
            let mut dice = BattleDice::seeded([3; 32]);
            let before = dice.clone();
            let hit = intact.standard_hit(arc, roll, 2, &mut dice).unwrap();
            assert_eq!(hit.section, row[usize::from(roll - 2)]);
            assert!(!hit.through_armor_critical);
            assert_eq!(dice, before);
            for vehicle in [&lost, &turretless] {
                assert_eq!(
                    vehicle
                        .standard_hit(arc, roll, 1, &mut dice)
                        .unwrap()
                        .section,
                    hull
                );
            }
        }
    }
    let mut dice = BattleDice::seeded([4; 32]);
    let before = dice.clone();
    for roll in [0, 1, 13, 255] {
        assert!(intact.standard_hit(Arc::Front, roll, 2, &mut dice).is_err());
    }
    assert_eq!(dice, before);
}

#[test]
fn critical_thresholds_and_modes_preserve_secondary_dice_order() {
    for (armor, arc, roll, sides) in [
        (39, Arc::Front, 2, Some(12)),
        (40, Arc::Front, 2, None),
        (49, Arc::Front, 11, Some(12)),
        (50, Arc::Front, 11, None),
        (100, Arc::Left, 12, Some(71)),
        (99, Arc::Left, 12, None),
        (0, Arc::Front, 10, None),
        (0, Arc::Rear, 12, Some(12)),
    ] {
        let vehicle = vehicle(armor);
        for seed in 0..=255 {
            let mut dice = BattleDice::seeded([seed; 32]);
            let mut expected = dice.clone();
            let critical = sides.is_some_and(|sides| {
                expected.die(sides).unwrap() == if sides == 12 { 6 } else { 23 }
            });
            assert_eq!(
                vehicle
                    .standard_hit(arc, roll, 2, &mut dice)
                    .unwrap()
                    .through_armor_critical,
                critical
            );
            assert_eq!(dice, expected);
        }
        let mut dice = BattleDice::seeded([3; 32]);
        let before = dice.clone();
        for mode in [-1, 0, 1] {
            assert!(
                !vehicle
                    .standard_hit(arc, roll, mode, &mut dice)
                    .unwrap()
                    .through_armor_critical
            );
        }
        assert_eq!(dice, before);
    }
}

#[test]
fn unarmored_faces_crit_without_dice_unless_stationary_or_critproof() {
    let mut state = serde_json::to_value(vehicle(0)).unwrap();
    state["definition"]["sections"]["front"]["armor"] = 0.into();
    let unarmored: BattleVehicle = serde_json::from_value(state.clone()).unwrap();
    let mut dice = BattleDice::seeded([5; 32]);
    let before = dice.clone();
    assert!(
        unarmored
            .standard_hit(Arc::Front, 2, 0, &mut dice)
            .unwrap()
            .through_armor_critical
    );
    assert_eq!(dice, before);
    state["definition"]["attributes"]["specials"] = "CritProof_Tech".into();
    let proof: BattleVehicle = serde_json::from_value(state.clone()).unwrap();
    assert!(
        !proof
            .standard_hit(Arc::Front, 2, 2, &mut dice)
            .unwrap()
            .through_armor_critical
    );
    state["definition"]["attributes"]["specials"] = "".into();
    state["definition"]["movement"] = "stationary".into();
    let fixed: BattleVehicle = serde_json::from_value(state).unwrap();
    assert!(
        !fixed
            .standard_hit(Arc::Front, 2, 2, &mut dice)
            .unwrap()
            .through_armor_critical
    );
    assert_eq!(dice, before);
}

/// Explicit FASA policy for tests; secondary armor candidates are disabled by default.
fn fasa_rules() -> stompymux_rs::BattleVehicleFasaHitRules {
    stompymux_rs::BattleVehicleFasaHitRules {
        friendly_criticals: false,
        critical_shielding: false,
        critical_mode: 1,
        critical_level: 40,
    }
}

#[test]
fn fasa_vehicle_effects_cover_directions_movement_and_configuration() {
    use stompymux_rs::{BattleVehicleHitCondition as Condition, BattleVehicleMotiveHit as M};
    for hover in [false, true] {
        let mut encoded = serde_json::to_value(vehicle(70)).unwrap();
        if hover {
            encoded["definition"]["movement"] = "hover".into();
            encoded["definition"]["max_speed"] = 64.5.into();
        }
        let target: BattleVehicle = serde_json::from_value(encoded).unwrap();
        for friendly in [false, true] {
            for shielding in [false, true] {
                let mut rules = fasa_rules();
                rules.friendly_criticals = friendly;
                rules.critical_shielding = shielding;
                let severe = Some(if friendly {
                    M::SpeedLoss { movement_points: 2 }
                } else {
                    M::Immobilize
                });
                let minor = Some(M::SpeedLoss { movement_points: 1 });
                for (arc, effects) in [
                    (
                        Arc::Left,
                        [
                            None, severe, minor, minor, None, None, None, None, None, None, None,
                        ],
                    ),
                    (
                        Arc::Right,
                        [
                            None,
                            severe,
                            minor,
                            minor,
                            None,
                            None,
                            None,
                            if hover { minor } else { None },
                            None,
                            None,
                            None,
                        ],
                    ),
                    (
                        Arc::Front,
                        [
                            None,
                            if shielding { severe } else { None },
                            if shielding { minor } else { None },
                            if hover { minor } else { None },
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                        ],
                    ),
                    (
                        Arc::Rear,
                        [
                            None,
                            if shielding { severe } else { None },
                            if shielding { minor } else { None },
                            if hover { minor } else { None },
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                            None,
                        ],
                    ),
                ] {
                    for roll in 2..=12 {
                        let mut dice = BattleDice::seeded([8; 32]);
                        let before = dice.clone();
                        let hit = target
                            .fasa_hit(arc, roll, rules, Condition::default(), &mut dice)
                            .unwrap();
                        let standard = target.standard_hit(arc, roll, 1, &mut dice).unwrap();
                        assert_eq!(hit.section, standard.section);
                        assert_eq!(
                            hit.motive,
                            effects[usize::from(roll - 2)],
                            "{arc:?} {roll} hover={hover}"
                        );
                        assert_eq!(hit.lock_turret, roll == 11);
                        assert_eq!(
                            hit.through_armor_critical,
                            match arc {
                                Arc::Left | Arc::Right => matches!(roll, 2 | 12),
                                Arc::Front | Arc::Rear => matches!(roll, 2 | 11),
                            }
                        );
                        let disabled = target
                            .fasa_hit(
                                arc,
                                roll,
                                rules,
                                Condition {
                                    immobilized: true,
                                    turret_locked: true,
                                },
                                &mut dice,
                            )
                            .unwrap();
                        assert!(disabled.motive.is_none() && !disabled.lock_turret);
                        assert_eq!(disabled.through_armor_critical, hit.through_armor_critical);
                        assert_eq!(dice, before);
                    }
                }
            }
        }
    }
}

#[test]
fn fasa_turret_loss_critical_proof_and_invalid_rolls_preserve_semantics() {
    use stompymux_rs::BattleVehicleHitCondition as Condition;
    let mut target = vehicle(70);
    target
        .damage_phase(S::Turret, 100, stompymux_rs::BattleDamagePhase::Internal)
        .unwrap();
    let mut dice = BattleDice::seeded([1; 32]);
    let before = dice.clone();
    for (arc, section, critical) in [
        (Arc::Front, S::Front, true),
        (Arc::Rear, S::Rear, true),
        (Arc::Left, S::Left, false),
        (Arc::Right, S::Right, false),
    ] {
        let hit = target
            .fasa_hit(arc, 11, fasa_rules(), Condition::default(), &mut dice)
            .unwrap();
        assert_eq!(hit.section, section);
        assert_eq!(hit.through_armor_critical, critical);
        assert!(!hit.lock_turret);
    }
    let mut encoded = serde_json::to_value(target).unwrap();
    encoded["definition"]["attributes"]["specials"] = "CritProof_Tech".into();
    let target: BattleVehicle = serde_json::from_value(encoded).unwrap();
    for roll in 2..=12 {
        let hit = target
            .fasa_hit(
                Arc::Left,
                roll,
                fasa_rules(),
                Condition::default(),
                &mut dice,
            )
            .unwrap();
        assert!(!hit.through_armor_critical && hit.motive.is_none() && !hit.lock_turret);
    }
    for roll in [0, 1, 13, 255] {
        assert!(
            target
                .fasa_hit(
                    Arc::Front,
                    roll,
                    fasa_rules(),
                    Condition::default(),
                    &mut dice
                )
                .is_err()
        );
    }
    assert_eq!(dice, before);
}

#[test]
fn fasa_configured_critical_threshold_preserves_conditional_roll_sequence() {
    use stompymux_rs::BattleVehicleHitCondition as Condition;
    for (armor, level) in [(70, 70), (70, 71), (100, 40), (100, 101)] {
        let target = vehicle(armor);
        let rules = stompymux_rs::BattleVehicleFasaHitRules {
            critical_mode: 2,
            critical_level: level,
            ..fasa_rules()
        };
        for seed in 0..=255 {
            let mut dice = BattleDice::seeded([seed; 32]);
            let mut expected = dice.clone();
            let mut critical = i64::from(armor) < level && expected.die(12).unwrap() == 6;
            if !critical && armor == 100 {
                critical = expected.die(71).unwrap() == 23;
            }
            let hit = target
                .fasa_hit(Arc::Rear, 12, rules, Condition::default(), &mut dice)
                .unwrap();
            assert_eq!(hit.through_armor_critical, critical);
            assert_eq!(dice, expected);
        }
    }
}
