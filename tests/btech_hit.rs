//! Biped location probabilities, attack-direction diagrams, through-armor criticals and head hits.
use stompymux_rs::{
    Dice, HitArc as Arc, HitTable as Table, Mech, MechSection as Section, MechTemplate,
    resolve_battle_hit,
};

/// Build an undamaged conventional biped; owned JSON permits isolated damaged-state fixtures.
fn unit(armor: u16) -> Mech {
    let unit = Mech::from_template(
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    let mut state = serde_json::to_value(unit).unwrap();
    state["sections"]["CenterTorso"]["armor"] = armor.into();
    serde_json::from_value(state).unwrap()
}

/// The 'Mech Attack Direction Diagram: a 180-degree front centred on the heading, 60-degree
/// sides and a 60-degree rear, with angles normalized before classification.
#[test]
fn mech_arc_boundaries_follow_the_total_warfare_diagram_and_wrap() {
    for (angle, expected) in [
        (0.0, Arc::Front),
        (90.0, Arc::Front),
        (90.01, Arc::Right),
        (149.99, Arc::Right),
        (150.0, Arc::Rear),
        (180.0, Arc::Rear),
        (210.0, Arc::Rear),
        (210.01, Arc::Left),
        (269.99, Arc::Left),
        (270.0, Arc::Front),
        (360.0, Arc::Front),
    ] {
        assert_eq!(Arc::from_bearing(angle, 0.0).unwrap(), expected);
        assert_eq!(Arc::from_bearing(angle + 721.0, 721.0).unwrap(), expected);
        assert_eq!(Arc::from_bearing(angle + 60.0, 60.0).unwrap(), expected);
        assert_eq!(Arc::from_bearing(angle - 400.0, -400.0).unwrap(), expected);
    }
    assert!(Arc::from_bearing(f64::NAN, 0.0).is_err());
    assert!(Arc::from_bearing(0.0, f64::INFINITY).is_err());
}

/// The Vehicle Attack Direction Diagram: one hexside each for the front and rear, two hexsides
/// for each side, with the same boundary inclusion as the 'Mech diagram.
#[test]
fn vehicle_arc_boundaries_follow_the_total_warfare_diagram_and_wrap() {
    for (angle, expected) in [
        (0.0, Arc::Front),
        (30.0, Arc::Front),
        (30.01, Arc::Right),
        (90.0, Arc::Right),
        (149.99, Arc::Right),
        (150.0, Arc::Rear),
        (180.0, Arc::Rear),
        (210.0, Arc::Rear),
        (210.01, Arc::Left),
        (270.0, Arc::Left),
        (329.99, Arc::Left),
        (330.0, Arc::Front),
        (360.0, Arc::Front),
    ] {
        assert_eq!(Arc::from_vehicle_bearing(angle, 0.0).unwrap(), expected);
        assert_eq!(
            Arc::from_vehicle_bearing(angle + 721.0, 721.0).unwrap(),
            expected
        );
        assert_eq!(
            Arc::from_vehicle_bearing(angle + 120.0, 120.0).unwrap(),
            expected
        );
    }
    assert!(Arc::from_vehicle_bearing(f64::NAN, 0.0).is_err());
    assert!(Arc::from_vehicle_bearing(0.0, f64::NEG_INFINITY).is_err());
}

#[test]
fn weapon_distribution_and_physical_tables_preserve_location_and_transfer_rules() {
    use Section::*;
    // Counts across the 36 equally likely die pairs, in MechSection::ALL order.
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
                    .location(stompymux_rs::MechChassis::Biped, arc, a + b)
                    .unwrap();
                counts[Section::ALL.iter().position(|s| *s == section).unwrap()] += 1;
            }
        }
        assert_eq!(counts, expected);
        for roll in 1..=6 {
            assert_ne!(
                Table::Punch
                    .location(stompymux_rs::MechChassis::Biped, arc, roll)
                    .unwrap(),
                LeftLeg
            );
            assert_ne!(
                Table::Punch
                    .location(stompymux_rs::MechChassis::Biped, arc, roll)
                    .unwrap(),
                RightLeg
            );
            assert!(matches!(
                Table::Kick
                    .location(stompymux_rs::MechChassis::Biped, arc, roll)
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
            .location(stompymux_rs::MechChassis::Biped, Arc::Rear, 1)
            .unwrap(),
        LeftArm
    );
    assert_eq!(
        Table::Kick
            .location(stompymux_rs::MechChassis::Biped, Arc::Front, 1)
            .unwrap(),
        RightLeg
    );
    assert!(
        Table::Weapon
            .location(stompymux_rs::MechChassis::Biped, Arc::Front, 1)
            .is_err()
    );
    assert!(
        Table::Punch
            .location(stompymux_rs::MechChassis::Biped, Arc::Front, 7)
            .is_err()
    );
}

/// A location roll of 2 is always a through-armor critical chance regardless of remaining
/// armor, no roll other than 2 ever is, and ordinary resolution never consumes dice.
#[test]
fn through_armor_criticals_are_unconditional_on_a_roll_of_two_without_dice() {
    let mut dice = Dice::seeded([42; 32]);
    let before = dice.clone();
    assert!(dice.die(0).is_err());
    assert!(resolve_battle_hit(&unit(10), Arc::Front, 1, &mut dice).is_err());
    assert_eq!(dice, before);
    for armor in [0, 1, 5, 6, 9, 10] {
        for arc in [Arc::Front, Arc::Rear, Arc::Left, Arc::Right] {
            let hit = resolve_battle_hit(&unit(armor), arc, 2, &mut dice).unwrap();
            assert!(hit.through_armor_critical);
            assert_eq!(
                hit.section,
                match arc {
                    Arc::Front | Arc::Rear => Section::CenterTorso,
                    Arc::Left => Section::LeftTorso,
                    Arc::Right => Section::RightTorso,
                }
            );
            assert_eq!(hit.rear_armor, arc == Arc::Rear);
            for roll in 3..=12 {
                let hit = resolve_battle_hit(&unit(armor), arc, roll, &mut dice).unwrap();
                assert!(!hit.through_armor_critical);
                assert_eq!(
                    hit.section,
                    Table::Weapon
                        .location(stompymux_rs::MechChassis::Biped, arc, roll)
                        .unwrap()
                );
            }
        }
    }
    assert_eq!(dice, before);
    // A roll of 12 is a head hit in every direction; rear head hits are not rear armor.
    for arc in [Arc::Front, Arc::Rear, Arc::Left, Arc::Right] {
        let hit = resolve_battle_hit(&unit(10), arc, 12, &mut dice).unwrap();
        assert_eq!(hit.section, Section::Head);
        assert!(!hit.rear_armor);
        assert!(!hit.through_armor_critical);
    }
    assert_eq!(dice, before);
}

/// Quad physical distributions distinguish the front and rear pairs in every direction.
#[test]
fn quad_tables_cover_all_rolls_and_preserve_shared_weapon_distribution() {
    use Section::*;
    use stompymux_rs::MechChassis::{Biped, Quad};
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

/// Quad head hits use the shared weapon table and never consume secondary dice.
#[test]
fn quad_head_hits_follow_chassis_and_preserve_dice_replay() {
    let mut encoded = serde_json::to_value(unit(10)).unwrap();
    // Component fixture isolates hit-table behavior from world registration.
    encoded["definition"]["attributes"]["move_type"] = "Quad".into();
    let target: Mech = serde_json::from_value(encoded).unwrap();
    for arc in [Arc::Front, Arc::Rear, Arc::Left, Arc::Right] {
        for seed in 0..32 {
            let mut dice = Dice::seeded([seed; 32]);
            let expected = dice.clone();
            let mut restored: Dice =
                serde_json::from_value(serde_json::to_value(&dice).unwrap()).unwrap();
            let hit = resolve_battle_hit(&target, arc, 12, &mut dice).unwrap();
            assert_eq!(hit.section, Section::Head);
            assert!(!hit.rear_armor);
            assert!(!hit.through_armor_critical);
            assert_eq!(
                hit,
                resolve_battle_hit(&target, arc, 12, &mut restored).unwrap()
            );
            assert_eq!(dice, expected);
            assert_eq!(dice, restored);
        }
    }
}

/// Seed a delegated table roll without consuming the caller's entry roll.
fn seed_for_location(roll: u8) -> Dice {
    (0..=255)
        .map(|seed| Dice::seeded([seed; 32]))
        .find(|dice| dice.clone().two_d6() == roll)
        .expect("seed for each 2d6 total")
}

/// Standard and critical-proof routing share rows but preserve distinct roll and TAC contracts.
#[test]
fn delegated_mech_tables_cover_anatomy_precedence_immunity_and_replay() {
    use stompymux_rs::MechChassis;
    for chassis in [MechChassis::Biped, MechChassis::Quad] {
        for proof in [false, true] {
            for safe in [false, true] {
                let mut state = serde_json::to_value(unit(10)).unwrap();
                state["definition"]["attributes"]["move_type"] = match chassis {
                    MechChassis::Biped => "Biped",
                    MechChassis::Quad => "Quad",
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
                let target: Mech = serde_json::from_value(state).unwrap();

                for arc in [Arc::Front, Arc::Rear, Arc::Left, Arc::Right] {
                    for selected in 2..=12 {
                        let mut dice = seed_for_location(selected);
                        let mut expected = dice.clone();
                        // Critical-proof targets draw their own location roll; the caller's
                        // entry roll is ignored, so a different entry must not change the result.
                        let entry = if proof {
                            assert_eq!(expected.two_d6(), selected);
                            if selected == 3 { 9 } else { 3 }
                        } else {
                            selected
                        };
                        let expected_section = if safe {
                            Section::LeftArm
                        } else {
                            Table::Weapon.location(chassis, arc, selected).unwrap()
                        };
                        let mut replay: Dice =
                            serde_json::from_value(serde_json::to_value(&dice).unwrap()).unwrap();
                        let hit = resolve_battle_hit(&target, arc, entry, &mut dice).unwrap();
                        assert_eq!(hit.section, expected_section);
                        assert_eq!(hit.through_armor_critical, !safe && !proof && selected == 2);
                        assert_eq!(
                            hit.rear_armor,
                            !safe
                                && arc == Arc::Rear
                                && matches!(
                                    expected_section,
                                    Section::LeftTorso | Section::RightTorso | Section::CenterTorso
                                )
                        );
                        assert_eq!(dice, expected);
                        assert_eq!(
                            resolve_battle_hit(&target, arc, entry, &mut replay).unwrap(),
                            hit
                        );
                        assert_eq!(dice, replay);
                    }
                    for invalid in [0, 1, 13, 255] {
                        let mut dice = seed_for_location(12);
                        let original = dice.clone();
                        assert!(resolve_battle_hit(&target, arc, invalid, &mut dice).is_err());
                        assert_eq!(dice, original);
                    }
                }
            }
        }
    }
}
