//! Command computer installation, independent master groups, and durable damage.
use stompymux_rs::*;

/// Replace arm equipment with explicitly positioned computer slots.
fn design(parts: &[(MechSection, u8, &str)]) -> Mech {
    let mut template =
        MechTemplate::parse("AS7-D", include_str!("fixtures/btech/mechs/AS7-D.toml")).unwrap();
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
            .clear();
    }
    for &(section, slot, equipment) in parts {
        template
            .sections
            .get_mut(&section)
            .unwrap()
            .criticals
            .insert(
                slot,
                CriticalDefinition {
                    equipment: equipment.into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
    }
    Mech::from_template(template).unwrap()
}

#[test]
fn masters_group_within_sections_and_damage_does_not_cross_groups() {
    let left = MechSection::LeftArm;
    let right = MechSection::RightArm;
    let mut parts: Vec<_> = (0..10).map(|slot| (left, slot, "C3Master")).collect();
    parts.extend((0..4).map(|slot| (right, slot, "C3Master")));
    let mut unit = design(&parts);
    assert_eq!(unit.c3_hardware().unwrap().masters, 2);
    assert_eq!(unit.c3_hardware().unwrap().working_masters, 2);
    let mass = unit.mass().unwrap();
    assert_eq!(
        mass.equipment - design(&[]).mass().unwrap().equipment,
        14 * 1024
    );
    unit.destroy_critical(CriticalLocation {
        section: left,
        slot: 2,
    })
    .unwrap();
    assert_eq!(unit.c3_hardware().unwrap().working_masters, 1);
    unit.destroy_critical(CriticalLocation {
        section: left,
        slot: 7,
    })
    .unwrap();
    assert_eq!(unit.c3_hardware().unwrap().working_masters, 0);
    assert_eq!(unit.c3_hardware().unwrap().masters, 2);
    assert_eq!(unit.mass().unwrap(), mass);
    let loaded: Mech = serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
    assert_eq!(loaded.c3_hardware().unwrap(), unit.c3_hardware().unwrap());
    // Five master slots may have intervening empty slots, but cannot span sections.
    let spaced: Vec<_> = [0, 2, 4, 6, 8]
        .into_iter()
        .map(|slot| (left, slot, "C3Master"))
        .collect();
    assert_eq!(design(&spaced).c3_hardware().unwrap().working_masters, 1);
    assert_eq!(
        design(&[
            (left, 0, "C3Master"),
            (left, 1, "C3Master"),
            (left, 2, "C3Master"),
            (right, 0, "C3Master"),
            (right, 1, "C3Master")
        ])
        .c3_hardware()
        .unwrap()
        .masters,
        0
    );
}

#[test]
fn slaves_and_c3i_use_live_slot_thresholds_without_trusting_flags() {
    let left = MechSection::LeftArm;
    let right = MechSection::RightArm;
    assert_eq!(design(&[]).c3_hardware().unwrap(), C3Hardware::default());
    let mut unit = design(&[
        (left, 0, "C3Slave"),
        (left, 1, "C3i"),
        (right, 0, "C3i"),
        (right, 1, "C3i"),
    ]);
    assert_eq!(
        unit.mass().unwrap().equipment - design(&[]).mass().unwrap().equipment,
        1024 + 3 * 1280
    );
    assert!(unit.c3_hardware().unwrap().slave_operational);
    assert!(unit.c3_hardware().unwrap().c3i_operational);
    unit.destroy_critical(CriticalLocation {
        section: right,
        slot: 0,
    })
    .unwrap();
    assert!(unit.c3_hardware().unwrap().c3i_operational);
    unit.destroy_critical(CriticalLocation {
        section: left,
        slot: 1,
    })
    .unwrap();
    unit.destroy_critical(CriticalLocation {
        section: left,
        slot: 0,
    })
    .unwrap();
    let hardware = unit.c3_hardware().unwrap();
    assert!(hardware.c3i_installed && hardware.slave_installed);
    assert!(!hardware.c3i_operational && !hardware.slave_operational);
}
