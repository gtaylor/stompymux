//! Exhaustive rotorcraft routing checks, independent of flight lifecycle and launcher resolution.
use stompymux_rs::*;

#[test]
fn vtol_tables_cover_every_arc_roll_and_critical_proof_override() {
    let base =
        VehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml")).unwrap();
    for proof in [false, true] {
        let mut template = base.clone();
        if proof {
            template
                .attributes
                .insert("specials".into(), "CritProof_Tech".into());
        }

        let layout = "RRRHHHHHRRR";
        for (arc, hull) in [
            (HitArc::Front, VehicleSection::Front),
            (HitArc::Rear, VehicleSection::Rear),
            (HitArc::Left, VehicleSection::Left),
            (HitArc::Right, VehicleSection::Right),
        ] {
            for roll in 2..=12 {
                let before = template.clone();
                let report = template.vtol_hit(arc, roll).unwrap();
                let rotor = layout.as_bytes()[usize::from(roll - 2)] == b'R';
                let expected = if rotor { VehicleSection::Rotor } else { hull };
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
                        Some(RotorHit::Destroy)
                    } else {
                        Some(RotorHit::Damage)
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
        Some(RotorHit::Damage),
        Some(RotorHit::Damage),
        Some(RotorHit::Damage),
        Some(RotorHit::TailRotor),
        Some(RotorHit::TailRotor),
        Some(RotorHit::Destroy),
        Some(RotorHit::Destroy),
    ];
    for (index, effect) in expected.into_iter().enumerate() {
        assert_eq!(
            RotorHit::from_critical_roll(index as u8 + 2).unwrap(),
            effect
        );
    }
    let template =
        VehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml")).unwrap();
    for roll in [0, 1, 13, 255] {
        assert!(RotorHit::from_critical_roll(roll).is_err());
        assert!(template.vtol_hit(HitArc::Front, roll).is_err());
    }
    let ground =
        VehicleTemplate::parse("Demolisher", include_str!("../game/mechs/Demolisher.toml"))
            .unwrap();
    assert!(ground.vtol_hit(HitArc::Front, 7).is_err());
}

#[test]
fn advanced_aircraft_locations_share_armor_gate_draws_without_direct_rotor_effects() {
    use VehicleSection as S;
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
        let unit = Vehicle::new(VehicleTemplate::parse("Kestrel", &source).unwrap()).unwrap();
        for (arc, row) in [
            (HitArc::Front, "FFORFFFLOOO"),
            (HitArc::Rear, "BBOLBBBROOO"),
            (HitArc::Left, "LLOFLLLBOOO"),
            (HitArc::Right, "RROFRRRBOOO"),
        ] {
            for (index, location) in row.chars().enumerate() {
                let roll = index as u8 + 2;
                for lucky in [false, true] {
                    let seed = (0..=255)
                        .find(|seed| (Dice::seeded([*seed; 32]).die(71).unwrap() == 23) == lucky)
                        .unwrap();
                    let mut dice = Dice::seeded([seed; 32]);
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
                                || (matches!(arc, HitArc::Left | HitArc::Right) && roll == 8))
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
    use VehicleSection as S;
    let mut template =
        VehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml")).unwrap();
    template
        .sections
        .insert(S::Turret, template.sections[&S::Front].clone());
    let mut unit = Vehicle::new(template).unwrap();
    let mut dice = Dice::seeded([0; 32]);
    assert_eq!(
        unit.advanced_vtol_hit(HitArc::Left, 4, 1, 60, &mut dice)
            .unwrap()
            .hit
            .section,
        S::Turret
    );
    unit.damage_phase(S::Turret, u16::MAX, DamagePhase::Internal)
        .unwrap();
    assert_eq!(
        unit.advanced_vtol_hit(HitArc::Left, 4, 1, 60, &mut dice)
            .unwrap()
            .hit
            .section,
        S::Rotor
    );
    let mut saved = serde_json::to_value(&unit).unwrap();
    saved["sections"]["front"]["armor"] = 4.into();
    let unit: Vehicle = serde_json::from_value(saved).unwrap();
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).die(12).unwrap() == 6)
        .unwrap();
    let mut dice = Dice::seeded([seed; 32]);
    let hit = unit
        .advanced_vtol_hit(HitArc::Front, 2, 2, 60, &mut dice)
        .unwrap();
    assert!(hit.hit.through_armor_critical);
    let mut expected = Dice::seeded([seed; 32]);
    expected.die(12).unwrap();
    assert_eq!(
        serde_json::to_value(&dice).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    let before = serde_json::to_value(&dice).unwrap();
    assert!(
        !unit
            .advanced_vtol_hit(HitArc::Front, 2, 1, 60, &mut dice)
            .unwrap()
            .hit
            .through_armor_critical
    );
    assert!(
        unit.advanced_vtol_hit(HitArc::Front, 13, 2, 60, &mut dice)
            .is_err()
    );
    assert_eq!(serde_json::to_value(dice).unwrap(), before);
}
