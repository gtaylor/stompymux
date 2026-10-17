//! Electronic equipment construction, slot accounting, capability and defensive value.
use stompymux_rs::*;

/// Install electronic slots in otherwise empty torso positions.
fn installed(name: &str, slots: u8, clan: bool) -> Mech {
    Mech::from_template(installation(name, slots, clan)).unwrap()
}

/// The legal design [`installed`] constructs.
fn installation(name: &str, slots: u8, clan: bool) -> MechTemplate {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    if clan {
        template.attributes.insert("specials".into(), "Clan".into());
        template.heat_sinks = 10;
        for section in template.sections.values_mut() {
            section
                .criticals
                .retain(|_, part| part.equipment != "HeatSink");
        }
    }
    let torso = template.sections.get_mut(&MechSection::LeftTorso).unwrap();
    for slot in 2..2 + slots {
        torso.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: name.into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    }
    crate::support::templates::legal(template)
}

/// Guardian mass differs by technology; Angel mass is one ton per slot. Suites add their
/// defensive BV (Guardian 61, Angel 100) times the Jenner's 1.4 Defensive Factor. A suite fills
/// its whole slot count, so a lone Guardian or Angel slot does not construct (TechManual
/// pp. 45, 342; Tactical Operations: Advanced Units & Equipment p. 217).
#[test]
fn electronics_construction_mass_value_and_damage() {
    use ElectronicSuite::{Angel, Guardian};
    for name in ["Ecm", "AngelEcm"] {
        let refused = Mech::from_template(installation(name, 1, false))
            .unwrap_err()
            .to_string();
        assert!(
            refused.contains(&format!(
                "{name} fills 2 contiguous critical slots per installation"
            )),
            "{refused}"
        );
    }
    for (name, slots, clan, suite, available, mass, bv) in [
        ("Ecm", 2, false, Guardian, true, 1536, 85.4),
        ("Ecm", 1, true, Guardian, true, 1024, 85.4),
        ("AngelEcm", 2, false, Angel, true, 2048, 140.0),
        ("AngelEcm", 2, true, Angel, true, 2048, 140.0),
    ] {
        let mut unit = installed(name, slots, clan);
        // The same design without the suite, keeping the armor the suite displaced, so the
        // difference is the suite's alone.
        let mut bare = unit.definition().clone();
        bare.sections
            .get_mut(&MechSection::LeftTorso)
            .unwrap()
            .criticals
            .retain(|_, part| part.equipment != name);
        let base = Mech::from_template(crate::support::templates::legal(bare)).unwrap();
        assert_eq!(unit.electronics(), Electronics::default());
        assert_eq!(unit.electronic_suite_available(suite).unwrap(), available);
        assert_eq!(
            unit.mass().unwrap().equipment - base.mass().unwrap().equipment,
            mass
        );
        assert!(
            (unit.battle_value().unwrap().defensive - base.battle_value().unwrap().defensive - bv)
                .abs()
                < 0.001
        );
        let location = CriticalLocation {
            section: MechSection::LeftTorso,
            slot: 2,
        };
        unit.destroy_critical(location).unwrap();
        assert!(!unit.electronic_suite_available(suite).unwrap());
        assert_eq!(
            unit.mass().unwrap().equipment - base.mass().unwrap().equipment,
            mass
        );
        let restored: Mech = serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored, unit);
    }
}
