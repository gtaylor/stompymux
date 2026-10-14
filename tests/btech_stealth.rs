//! Stealth construction and concealment range rules, independent of the host.
use stompymux_rs::*;

/// Armor uses two passive slots in each limb and side torso, plus a Guardian suite.
fn template() -> MechTemplate {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    for section in MechSection::ALL {
        if matches!(section, MechSection::Head | MechSection::CenterTorso) {
            continue;
        }
        for slot in [4, 5] {
            template
                .sections
                .get_mut(&section)
                .unwrap()
                .criticals
                .insert(
                    slot,
                    CriticalDefinition {
                        equipment: "StealthArmor".into(),
                        data: "-".into(),
                        modes: vec![],
                    },
                );
        }
    }
    for slot in [6, 7] {
        template
            .sections
            .get_mut(&MechSection::LeftTorso)
            .unwrap()
            .criticals
            .insert(
                slot,
                CriticalDefinition {
                    equipment: "Ecm".into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
    }
    template
}

/// Capability comes from the installed layout; passive slots add no mass and cannot receive random criticals.
#[test]
fn stealth_equipment_layout_and_accounting() {
    let complete = template();
    let unit = Mech::from_template(complete.clone()).unwrap();
    assert!(unit.has_stealth_armor().unwrap());
    assert_eq!(unit.stealth(), SignatureState::default());
    let mut stripped = complete.clone();
    for section in stripped.sections.values_mut() {
        section
            .criticals
            .retain(|_, part| part.equipment != "StealthArmor");
    }
    let base = Mech::from_template(stripped).unwrap();
    assert_eq!(unit.mass().unwrap(), base.mass().unwrap());
    // Stealth armor adds its +2 long-range modifier to the Defensive Factor (1.4 to 1.6) and
    // ten points of movement heat, which halves more weapons (TechManual p. 302).
    let stealthy = unit.battle_value().unwrap();
    let plain = base.battle_value().unwrap();
    assert!((stealthy.defensive - plain.defensive / 1.4 * 1.6).abs() < 1e-9);
    assert!(stealthy.offensive < plain.offensive);
    for section in MechSection::ALL {
        assert!(unit.critical_candidates(section).iter().all(|location| {
            unit.definition().sections[&section].criticals[&location.slot].equipment
                != "StealthArmor"
        }));
        if matches!(section, MechSection::Head | MechSection::CenterTorso) {
            continue;
        }
        let mut incomplete = complete.clone();
        incomplete
            .sections
            .get_mut(&section)
            .unwrap()
            .criticals
            .remove(&5);
        assert!(
            !Mech::from_template(incomplete)
                .unwrap()
                .has_stealth_armor()
                .unwrap()
        );
    }
    let mut angel_only = complete;
    for part in angel_only
        .sections
        .get_mut(&MechSection::LeftTorso)
        .unwrap()
        .criticals
        .values_mut()
    {
        if part.equipment == "Ecm" {
            part.equipment = "AngelEcm".into();
        }
    }
    // Any ECM suite powers stealth armor, the Angel as well as the Guardian.
    assert!(
        Mech::from_template(angel_only)
            .unwrap()
            .has_stealth_armor()
            .unwrap()
    );
}

/// Stealth changes accuracy at the selected bracket and never extends weapon reach or changes minimum penalties.
#[test]
fn stealth_range_brackets_and_limits() {
    for (distance, plain, hidden) in [(0.0, 0, 0), (3.0, 0, 0), (3.1, 2, 3), (6.1, 4, 6)] {
        let range = Weapon::MediumLaser
            .range_modifier(distance)
            .unwrap()
            .unwrap();
        assert_eq!(range.modifier, plain);
        assert_eq!(range.against_stealth(true).modifier, hidden);
        assert_eq!(range.against_stealth(false), range);
    }
    assert!(Weapon::MediumLaser.range_modifier(12.1).unwrap().is_none());
    let minimum = Weapon::Ppc.range_modifier(1.0).unwrap().unwrap();
    assert_eq!(minimum.against_stealth(true), minimum);
}
