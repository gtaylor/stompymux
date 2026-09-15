//! Biped location probabilities, arc boundaries, critical gating and head-hit variants.
use stompymux_rs::{
    BattleDice, BattleHitArc as Arc, BattleHitRules, BattleHitTable as Table,
    BattleSection as Section, BattleTemplate, BattleUnit,
};

/// Build an undamaged conventional biped; owned JSON permits isolated damaged-state fixtures.
fn unit(armor: u16) -> BattleUnit {
    let unit = BattleUnit::from_template(
        BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap(),
    )
    .unwrap();
    let mut state = serde_json::to_value(unit).unwrap();
    state["sections"]["CenterTorso"]["armor"] = armor.into();
    serde_json::from_value(state).unwrap()
}

#[test]
fn biped_arc_boundaries_wrap_and_follow_all_three_configured_modes() {
    for mode in [0, 2] {
        for (angle, expected) in [
            (0.0, Arc::Front),
            (90.0, Arc::Front),
            (90.01, Arc::Right),
            (149.99, Arc::Right),
            (150.0, Arc::Rear),
            (210.0, Arc::Rear),
            (210.01, Arc::Left),
            (269.99, Arc::Left),
            (270.0, Arc::Front),
            (360.0, Arc::Front),
        ] {
            assert_eq!(Arc::from_bearing(angle, 0.0, mode).unwrap(), expected);
            assert_eq!(
                Arc::from_bearing(angle + 721.0, 721.0, mode).unwrap(),
                expected
            );
        }
    }
    for (angle, expected) in [
        (45.0, Arc::Front),
        (45.01, Arc::Right),
        (135.0, Arc::Rear),
        (225.0, Arc::Rear),
        (225.01, Arc::Left),
        (315.0, Arc::Front),
    ] {
        assert_eq!(Arc::from_bearing(angle, 0.0, 1).unwrap(), expected);
    }
    assert!(Arc::from_bearing(f64::NAN, 0.0, 0).is_err());
    assert!(Arc::from_bearing(0.0, 0.0, 3).is_err());
}

#[test]
fn weapon_distribution_and_physical_tables_preserve_location_and_transfer_rules() {
    use Section::*;
    // Counts across the 36 equally likely die pairs, in BattleSection::ALL order.
    for (arc, expected) in [
        (Arc::Front, [5, 5, 5, 5, 7, 4, 4, 1]),
        (Arc::Rear, [5, 5, 5, 5, 7, 4, 4, 1]),
        (Arc::Left, [7, 3, 7, 4, 5, 7, 2, 1]),
        (Arc::Right, [3, 7, 4, 7, 5, 2, 7, 1]),
    ] {
        let mut counts = [0; 8];
        for a in 1..=6 {
            for b in 1..=6 {
                let section = Table::Weapon
                    .location(stompymux_rs::BattleMechChassis::Biped, arc, a + b)
                    .unwrap();
                counts[Section::ALL.iter().position(|s| *s == section).unwrap()] += 1;
            }
        }
        assert_eq!(counts, expected);
        for roll in 1..=6 {
            assert_ne!(
                Table::Punch
                    .location(stompymux_rs::BattleMechChassis::Biped, arc, roll)
                    .unwrap(),
                LeftLeg
            );
            assert_ne!(
                Table::Punch
                    .location(stompymux_rs::BattleMechChassis::Biped, arc, roll)
                    .unwrap(),
                RightLeg
            );
            assert!(matches!(
                Table::Kick
                    .location(stompymux_rs::BattleMechChassis::Biped, arc, roll)
                    .unwrap(),
                LeftLeg | RightLeg
            ));
        }
    }
    for (section, next) in [
        (LeftArm, Some(LeftTorso)),
        (LeftLeg, Some(LeftTorso)),
        (RightArm, Some(RightTorso)),
        (RightLeg, Some(RightTorso)),
        (LeftTorso, Some(CenterTorso)),
        (RightTorso, Some(CenterTorso)),
        (CenterTorso, None),
        (Head, None),
    ] {
        assert_eq!(section.damage_transfer(), next);
    }
    assert_eq!(
        Table::Punch
            .location(stompymux_rs::BattleMechChassis::Biped, Arc::Rear, 1)
            .unwrap(),
        LeftArm
    );
    assert_eq!(
        Table::Kick
            .location(stompymux_rs::BattleMechChassis::Biped, Arc::Front, 1)
            .unwrap(),
        RightLeg
    );
    assert!(
        Table::Weapon
            .location(stompymux_rs::BattleMechChassis::Biped, Arc::Front, 1)
            .is_err()
    );
    assert!(
        Table::Punch
            .location(stompymux_rs::BattleMechChassis::Biped, Arc::Front, 7)
            .is_err()
    );
}

#[test]
fn conditional_critical_rolls_and_head_grazes_consume_only_required_dice() {
    let standard = BattleHitRules {
        fasa_criticals: false,
        inferno_penalty: false,
        exile_stun_mode: 0,
    };
    let mut dice = BattleDice::seeded([42; 32]);
    let before = dice.clone();
    assert!(dice.die(0).is_err());
    assert!(
        standard
            .resolve(&unit(10), Arc::Front, 1, &mut dice)
            .is_err()
    );
    assert_eq!(dice, before);
    for armor in [6, 9] {
        assert!(
            !standard
                .resolve(&unit(armor), Arc::Front, 2, &mut dice)
                .unwrap()
                .through_armor_critical
        );
        assert_eq!(dice, before);
    }
    // At 50% armor and at full armor, compare conditional rolls to independently consumed dice.
    for (armor, sides, success) in [(5, 12, 6), (10, 71, 23)] {
        let target = unit(armor);
        let mut expected = dice.clone();
        for _ in 0..512 {
            let critical = expected.die(sides).unwrap() == success;
            assert_eq!(
                standard
                    .resolve(&target, Arc::Front, 2, &mut dice)
                    .unwrap()
                    .through_armor_critical,
                critical
            );
            assert_eq!(dice, expected);
        }
    }
    let fasa = BattleHitRules {
        fasa_criticals: true,
        inferno_penalty: false,
        ..standard
    };
    let mut expected = dice.clone();
    let selected = expected.two_d6();
    let hit = fasa.resolve(&unit(10), Arc::Rear, 2, &mut dice).unwrap();
    assert_eq!(
        hit.section,
        Table::Weapon
            .location(stompymux_rs::BattleMechChassis::Biped, Arc::Rear, selected)
            .unwrap()
    );
    assert_eq!(hit.through_armor_critical, selected == 2);
    assert_eq!(dice, expected);
    for mode in [0, 1, 2] {
        let rules = BattleHitRules {
            exile_stun_mode: mode,
            ..standard
        };
        let target = unit(10);
        let mut expected = dice.clone();
        for _ in 0..50 {
            let section = if mode == 0 {
                Section::Head
            } else {
                Table::Punch
                    .location(
                        stompymux_rs::BattleMechChassis::Biped,
                        Arc::Rear,
                        expected.d6(),
                    )
                    .unwrap()
            };
            let hit = rules.resolve(&target, Arc::Rear, 12, &mut dice).unwrap();
            assert_eq!(hit.section, section);
            assert_eq!(hit.crew_stun, mode == 1 && section != Section::Head);
            assert_eq!(
                hit.rear_armor,
                matches!(
                    section,
                    Section::LeftTorso | Section::RightTorso | Section::CenterTorso
                )
            );
            assert_eq!(dice, expected);
        }
    }
}

/// Quad physical distributions distinguish the front and rear pairs in every direction.
#[test]
fn quad_tables_cover_all_rolls_and_preserve_shared_weapon_distribution() {
    use Section::*;
    use stompymux_rs::BattleMechChassis::{Biped, Quad};
    for (arc, punch, kick) in [
        (
            Arc::Front,
            [LeftArm, LeftTorso, CenterTorso, RightTorso, RightArm, Head],
            [RightArm, RightArm, RightArm, LeftArm, LeftArm, LeftArm],
        ),
        (
            Arc::Rear,
            [LeftLeg, LeftTorso, CenterTorso, RightTorso, RightLeg, Head],
            [RightLeg, RightLeg, RightLeg, LeftLeg, LeftLeg, LeftLeg],
        ),
        (
            Arc::Left,
            [LeftTorso, LeftTorso, CenterTorso, LeftArm, LeftLeg, Head],
            [LeftArm, LeftArm, LeftArm, LeftLeg, LeftLeg, LeftLeg],
        ),
        (
            Arc::Right,
            [
                RightTorso,
                RightTorso,
                CenterTorso,
                RightArm,
                RightLeg,
                Head,
            ],
            [RightArm, RightArm, RightArm, RightLeg, RightLeg, RightLeg],
        ),
    ] {
        for roll in 1..=6 {
            assert_eq!(
                Table::Punch.location(Quad, arc, roll).unwrap(),
                punch[usize::from(roll - 1)]
            );
            assert_eq!(
                Table::Kick.location(Quad, arc, roll).unwrap(),
                kick[usize::from(roll - 1)]
            );
        }
        for roll in 2..=12 {
            assert_eq!(
                Table::Weapon.location(Quad, arc, roll).unwrap(),
                Table::Weapon.location(Biped, arc, roll).unwrap()
            );
        }
        for table in [Table::Punch, Table::Kick] {
            for invalid in [0, 7, 255] {
                assert!(table.location(Quad, arc, invalid).is_err());
            }
        }
    }
}

/// Head grazes use the target's anatomy and consume exactly one secondary die after restart.
#[test]
fn quad_head_rerolls_follow_chassis_and_preserve_dice_replay() {
    use stompymux_rs::BattleMechChassis::Quad;
    let mut encoded = serde_json::to_value(unit(10)).unwrap();
    // Component fixture isolates hit-table behavior from world registration.
    encoded["definition"]["attributes"]["move_type"] = "Quad".into();
    let target: BattleUnit = serde_json::from_value(encoded).unwrap();
    for arc in [Arc::Front, Arc::Rear, Arc::Left, Arc::Right] {
        for mode in [0, 1, 2] {
            for seed in 0..32 {
                let rules = BattleHitRules {
                    fasa_criticals: false,
                    inferno_penalty: false,
                    exile_stun_mode: mode,
                };
                let mut dice = BattleDice::seeded([seed; 32]);
                let mut expected = dice.clone();
                let section = if mode == 0 {
                    Section::Head
                } else {
                    Table::Punch.location(Quad, arc, expected.d6()).unwrap()
                };
                let mut restored: BattleDice =
                    serde_json::from_value(serde_json::to_value(&dice).unwrap()).unwrap();
                let hit = rules.resolve(&target, arc, 12, &mut dice).unwrap();
                assert_eq!(hit.section, section);
                assert_eq!(hit.crew_stun, mode == 1 && section != Section::Head);
                assert_eq!(hit, rules.resolve(&target, arc, 12, &mut restored).unwrap());
                assert_eq!(dice, expected);
                assert_eq!(dice, restored);
            }
        }
    }
}

/// Seed a delegated table roll without consuming the caller's entry roll.
fn seed_for_location(roll: u8) -> BattleDice {
    (0..=255)
        .map(|seed| BattleDice::seeded([seed; 32]))
        .find(|dice| dice.clone().two_d6() == roll)
        .expect("seed for each 2d6 total")
}

/// Standard, FASA and critical-proof routing share rows but preserve distinct roll and TAC contracts.
#[test]
fn delegated_mech_tables_cover_anatomy_precedence_immunity_and_replay() {
    use stompymux_rs::BattleMechChassis;
    for chassis in [BattleMechChassis::Biped, BattleMechChassis::Quad] {
        for proof in [false, true] {
            for safe in [false, true] {
                let mut state = serde_json::to_value(unit(10)).unwrap();
                state["definition"]["attributes"]["move_type"] = match chassis {
                    BattleMechChassis::Biped => "Biped",
                    BattleMechChassis::Quad => "Quad",
                }
                .into();
                if proof {
                    state["definition"]["attributes"]["specials"] =
                        "FlipArms CritProof_Tech".into();
                }
                state["combat_safe"] = safe.into();
                for section in state["sections"].as_object_mut().unwrap().values_mut() {
                    section["armor"] = (section["armor"].as_u64().unwrap() * 3 / 4).into();
                }
                let target: BattleUnit = serde_json::from_value(state).unwrap();
                for fasa in [false, true] {
                    for mode in [0, 1, 2] {
                        let rules = BattleHitRules {
                            fasa_criticals: fasa,
                            inferno_penalty: false,
                            exile_stun_mode: mode,
                        };
                        for arc in [Arc::Front, Arc::Rear, Arc::Left, Arc::Right] {
                            for selected in 2..=12 {
                                let mut dice = seed_for_location(selected);
                                let mut expected = dice.clone();
                                let entry = if proof || fasa {
                                    assert_eq!(expected.two_d6(), selected);
                                    if selected == 3 { 9 } else { 3 }
                                } else {
                                    selected
                                };
                                let expected_section = if safe {
                                    Section::LeftArm
                                } else if selected == 12 && mode != 0 {
                                    Table::Punch.location(chassis, arc, expected.d6()).unwrap()
                                } else {
                                    Table::Weapon.location(chassis, arc, selected).unwrap()
                                };
                                let mut replay: BattleDice =
                                    serde_json::from_value(serde_json::to_value(&dice).unwrap())
                                        .unwrap();
                                let hit = rules.resolve(&target, arc, entry, &mut dice).unwrap();
                                assert_eq!(hit.section, expected_section);
                                assert_eq!(
                                    hit.through_armor_critical,
                                    !safe && !proof && fasa && selected == 2
                                );
                                assert_eq!(
                                    hit.crew_stun,
                                    !safe
                                        && mode == 1
                                        && selected == 12
                                        && expected_section != Section::Head
                                );
                                assert_eq!(
                                    hit.rear_armor,
                                    !safe
                                        && arc == Arc::Rear
                                        && matches!(
                                            expected_section,
                                            Section::LeftTorso
                                                | Section::RightTorso
                                                | Section::CenterTorso
                                        )
                                );
                                assert_eq!(dice, expected);
                                assert_eq!(
                                    rules.resolve(&target, arc, entry, &mut replay).unwrap(),
                                    hit
                                );
                                assert_eq!(dice, replay);
                            }
                            for invalid in [0, 1, 13, 255] {
                                let mut dice = seed_for_location(12);
                                let original = dice.clone();
                                assert!(rules.resolve(&target, arc, invalid, &mut dice).is_err());
                                assert_eq!(dice, original);
                            }
                        }
                    }
                }
            }
        }
    }
}
