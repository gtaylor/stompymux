//! Exhaustive rotorcraft routing checks, independent of flight lifecycle and launcher resolution.
use stompymux_rs::*;

#[test]
fn vtol_tables_cover_every_arc_roll_and_critical_proof_override() {
    let base = BattleVehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml"))
        .unwrap();
    for proof in [false, true] {
        let mut template = base.clone();
        if proof {
            template
                .attributes
                .insert("specials".into(), "CritProof_Tech".into());
        }

        let layout = "RRRHHHHHRRR";
        for (arc, hull) in [
            (BattleHitArc::Front, BattleVehicleSection::Front),
            (BattleHitArc::Rear, BattleVehicleSection::Rear),
            (BattleHitArc::Left, BattleVehicleSection::Left),
            (BattleHitArc::Right, BattleVehicleSection::Right),
        ] {
            for roll in 2..=12 {
                let before = template.clone();
                let report = template.vtol_hit(arc, roll).unwrap();
                let rotor = layout.as_bytes()[usize::from(roll - 2)] == b'R';
                let expected = if rotor {
                    BattleVehicleSection::Rotor
                } else {
                    hull
                };
                assert_eq!(report.hit.section, expected, "{arc:?}/{roll}/{proof}");
                assert_eq!(
                    report.hit.through_armor_critical,
                    !proof && [2, 12].contains(&roll)
                );
                assert_eq!(
                    report.rotor,
                    if proof || !rotor {
                        None
                    } else if roll == 2 {
                        Some(BattleRotorHit::Destroy)
                    } else {
                        Some(BattleRotorHit::Damage)
                    }
                );
                assert_eq!(report.hit.motive, None);
                assert_eq!(template, before);
            }
        }
    }
}

#[test]
fn rotor_critical_ranges_and_invalid_requests_are_explicit() {
    let expected = [
        None,
        None,
        None,
        None,
        Some(BattleRotorHit::Damage),
        Some(BattleRotorHit::Damage),
        Some(BattleRotorHit::Damage),
        Some(BattleRotorHit::TailRotor),
        Some(BattleRotorHit::TailRotor),
        Some(BattleRotorHit::Destroy),
        Some(BattleRotorHit::Destroy),
    ];
    for (index, effect) in expected.into_iter().enumerate() {
        assert_eq!(
            BattleRotorHit::from_critical_roll(index as u8 + 2).unwrap(),
            effect
        );
    }
    let template =
        BattleVehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml"))
            .unwrap();
    for roll in [0, 1, 13, 255] {
        assert!(BattleRotorHit::from_critical_roll(roll).is_err());
        assert!(template.vtol_hit(BattleHitArc::Front, roll).is_err());
    }
    let ground =
        BattleVehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
            .unwrap();
    assert!(ground.vtol_hit(BattleHitArc::Front, 7).is_err());
}

#[test]
fn advanced_aircraft_locations_share_armor_gate_draws_without_direct_rotor_effects() {
    use BattleVehicleSection as S;
    for proof in [false, true] {
        let source = if proof {
            include_str!("../game/mechs/Kestrel.toml").replacen(
                "\"CargoTech\"",
                "\"CargoTech\", \"CritProof_Tech\"",
                1,
            )
        } else {
            include_str!("../game/mechs/Kestrel.toml").into()
        };
        let unit =
            BattleVehicle::new(BattleVehicleTemplate::parse("Kestrel", &source).unwrap()).unwrap();
        for (arc, row) in [
            (BattleHitArc::Front, "FFORFFFLOOO"),
            (BattleHitArc::Rear, "BBOLBBBROOO"),
            (BattleHitArc::Left, "LLOFLLLBOOO"),
            (BattleHitArc::Right, "RROFRRRBOOO"),
        ] {
            for (index, location) in row.chars().enumerate() {
                let roll = index as u8 + 2;
                for lucky in [false, true] {
                    let seed = (0..=255)
                        .find(|seed| {
                            (BattleDice::seeded([*seed; 32]).die(71).unwrap() == 23) == lucky
                        })
                        .unwrap();
                    let mut dice = BattleDice::seeded([seed; 32]);
                    let mut expected = dice.clone();
                    if !proof {
                        expected.die(71).unwrap();
                    }
                    let hit = unit.advanced_vtol_hit(arc, roll, 2, 60, &mut dice).unwrap();
                    assert_eq!(
                        hit.hit.section,
                        match location {
                            'F' => S::Front,
                            'B' => S::Rear,
                            'L' => S::Left,
                            'R' => S::Right,
                            _ => S::Rotor,
                        }
                    );
                    assert_eq!(
                        hit.hit.through_armor_critical,
                        !proof
                            && lucky
                            && (matches!(roll, 2 | 12)
                                || (matches!(arc, BattleHitArc::Left | BattleHitArc::Right)
                                    && roll == 8))
                    );
                    assert!(hit.rotor.is_none());
                    assert_eq!(
                        serde_json::to_value(dice).unwrap(),
                        serde_json::to_value(expected).unwrap()
                    );
                }
            }
        }
    }
}

#[test]
fn advanced_aircraft_turret_fallback_and_damaged_armor_gate_use_material_state() {
    use BattleVehicleSection as S;
    let mut template =
        BattleVehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml"))
            .unwrap();
    template
        .sections
        .insert(S::Turret, template.sections[&S::Front].clone());
    let mut unit = BattleVehicle::new(template).unwrap();
    let mut dice = BattleDice::seeded([0; 32]);
    assert_eq!(
        unit.advanced_vtol_hit(BattleHitArc::Left, 4, 1, 60, &mut dice)
            .unwrap()
            .hit
            .section,
        S::Turret
    );
    unit.damage_phase(S::Turret, u16::MAX, BattleDamagePhase::Internal)
        .unwrap();
    assert_eq!(
        unit.advanced_vtol_hit(BattleHitArc::Left, 4, 1, 60, &mut dice)
            .unwrap()
            .hit
            .section,
        S::Rotor
    );
    let mut saved = serde_json::to_value(&unit).unwrap();
    saved["sections"]["front"]["armor"] = 4.into();
    let unit: BattleVehicle = serde_json::from_value(saved).unwrap();
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).die(12).unwrap() == 6)
        .unwrap();
    let mut dice = BattleDice::seeded([seed; 32]);
    let hit = unit
        .advanced_vtol_hit(BattleHitArc::Front, 2, 2, 60, &mut dice)
        .unwrap();
    assert!(hit.hit.through_armor_critical);
    let mut expected = BattleDice::seeded([seed; 32]);
    expected.die(12).unwrap();
    assert_eq!(
        serde_json::to_value(&dice).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    let before = serde_json::to_value(&dice).unwrap();
    assert!(
        !unit
            .advanced_vtol_hit(BattleHitArc::Front, 2, 1, 60, &mut dice)
            .unwrap()
            .hit
            .through_armor_critical
    );
    assert!(
        unit.advanced_vtol_hit(BattleHitArc::Front, 13, 2, 60, &mut dice)
            .is_err()
    );
    assert_eq!(serde_json::to_value(dice).unwrap(), before);
}
