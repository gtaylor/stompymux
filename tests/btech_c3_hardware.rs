//! Command computer installation, independent master groups, and durable damage.
use stompymux_rs::*;

/// Replace arm equipment with explicitly positioned computer slots, numbered from the first
/// slot after the shoulder and upper arm actuators every arm keeps (TechManual p. 45).
fn design(parts: &[(MechSection, u8, &str)]) -> Mech {
    Mech::from_template(crate::support::templates::legal(template(parts))).unwrap()
}

/// The template [`design`] constructs.
fn template(parts: &[(MechSection, u8, &str)]) -> MechTemplate {
    let mut template =
        MechTemplate::parse("AS7-D", include_str!("fixtures/btech/units/AS7-D.toml")).unwrap();
    template.attributes.insert(
        "specials".into(),
        "C3MasterTech C3SlaveTech C3I_Tech".into(),
    );
    for section in [MechSection::LeftArm, MechSection::RightArm] {
        template
            .sections
            .get_mut(&section)
            .unwrap()
            .criticals
            .retain(|&slot, _| slot < 2);
    }
    for &(section, slot, equipment) in parts {
        template
            .sections
            .get_mut(&section)
            .unwrap()
            .criticals
            .insert(
                slot + 2,
                CriticalDefinition {
                    equipment: equipment.into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
    }
    template
}

#[test]
fn masters_group_within_sections_and_damage_does_not_cross_groups() {
    let left = MechSection::LeftArm;
    let right = MechSection::RightArm;
    let parts: Vec<_> = (0..10).map(|slot| (left, slot, "C3Master")).collect();
    let mut unit = design(&parts);
    assert_eq!(unit.c3_hardware().unwrap().masters, 2);
    assert_eq!(unit.c3_hardware().unwrap().working_masters, 2);
    let mass = unit.mass().unwrap();
    assert_eq!(
        mass.equipment - design(&[]).mass().unwrap().equipment,
        10 * 1024
    );
    unit.destroy_critical(CriticalLocation {
        section: left,
        slot: 4,
    })
    .unwrap();
    assert_eq!(unit.c3_hardware().unwrap().working_masters, 1);
    unit.destroy_critical(CriticalLocation {
        section: left,
        slot: 9,
    })
    .unwrap();
    assert_eq!(unit.c3_hardware().unwrap().working_masters, 0);
    assert_eq!(unit.c3_hardware().unwrap().masters, 2);
    assert_eq!(unit.mass().unwrap(), mass);
    let loaded: Mech = serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
    assert_eq!(loaded.c3_hardware().unwrap(), unit.c3_hardware().unwrap());
    // A master fills five contiguous slots of one location (TechManual p. 45), so spaced
    // slots, a group split between sections and a partial group do not construct.
    let spaced: Vec<_> = [0, 2, 4, 6, 8]
        .into_iter()
        .map(|slot| (left, slot, "C3Master"))
        .collect();
    let split = [
        (left, 0, "C3Master"),
        (left, 1, "C3Master"),
        (left, 2, "C3Master"),
        (right, 0, "C3Master"),
        (right, 1, "C3Master"),
    ];
    let partial: Vec<_> = (0..4).map(|slot| (right, slot, "C3Master")).collect();
    for parts in [&spaced[..], &split, &partial] {
        let refused = Mech::from_template(crate::support::templates::legal(template(parts)))
            .unwrap_err()
            .to_string();
        assert!(
            refused.contains("C3Master fills 5 contiguous critical slots per installation"),
            "{refused}"
        );
    }
}

#[test]
fn slaves_and_c3i_use_live_slot_thresholds_without_trusting_flags() {
    let left = MechSection::LeftArm;
    let right = MechSection::RightArm;
    assert_eq!(design(&[]).c3_hardware().unwrap(), C3Hardware::default());
    // A Mech's C3i fills two slots (TechManual p. 342), and losing either stops it.
    let mut unit = design(&[(left, 0, "C3Slave"), (right, 0, "C3i"), (right, 1, "C3i")]);
    assert_eq!(
        unit.mass().unwrap().equipment - design(&[]).mass().unwrap().equipment,
        1024 + 2 * 1280
    );
    assert!(unit.c3_hardware().unwrap().slave_operational);
    assert!(unit.c3_hardware().unwrap().c3i_operational);
    unit.destroy_critical(CriticalLocation {
        section: right,
        slot: 2,
    })
    .unwrap();
    assert!(!unit.c3_hardware().unwrap().c3i_operational);
    unit.destroy_critical(CriticalLocation {
        section: left,
        slot: 2,
    })
    .unwrap();
    let hardware = unit.c3_hardware().unwrap();
    assert!(hardware.c3i_installed && hardware.slave_installed);
    assert!(!hardware.c3i_operational && !hardware.slave_operational);
}
