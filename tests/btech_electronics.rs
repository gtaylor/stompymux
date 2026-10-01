//! Electronic equipment construction, slot accounting, capability and defensive value.
use stompymux_rs::*;

/// Install electronic slots in otherwise empty torso positions.
fn installed(name: &str, slots: u8, clan: bool) -> BattleUnit {
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    if clan {
        template.attributes.insert("specials".into(), "Clan".into());
        template.heat_sinks = 20;
        for section in template.sections.values_mut() {
            section
                .criticals
                .retain(|_, part| part.equipment != "HeatSink");
        }
    }
    let torso = template
        .sections
        .get_mut(&BattleSection::LeftTorso)
        .unwrap();
    for slot in 2..2 + slots {
        torso.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: name.into(),
                data: "-".into(),
                modes: vec![],
                brand: None,
            },
        );
    }
    BattleUnit::from_template(template).unwrap()
}

/// Guardian mass differs by technology; Angel mass is one ton per slot and has no intrinsic defensive BV.
#[test]
fn electronics_construction_mass_value_and_damage() {
    use BattleElectronicSuite::{Angel, Guardian};
    for (name, slots, clan, suite, available, mass, bv) in [
        ("Ecm", 1, false, Guardian, true, 768, 0.0),
        ("Ecm", 2, false, Guardian, true, 1536, 85.4),
        ("Ecm", 1, true, Guardian, true, 1024, 85.4),
        ("AngelEcm", 1, false, Angel, false, 1024, 0.0),
        ("AngelEcm", 2, false, Angel, true, 2048, 0.0),
        ("AngelEcm", 2, true, Angel, true, 2048, 0.0),
    ] {
        let base = installed(name, 0, clan);
        let mut unit = installed(name, slots, clan);
        assert_eq!(unit.electronics(), BattleElectronics::default());
        assert_eq!(unit.electronic_suite_available(suite).unwrap(), available);
        assert_eq!(
            unit.mass().unwrap().equipment - base.mass().unwrap().equipment,
            mass
        );
        assert!(
            (unit.battle_value(None).unwrap().defensive
                - base.battle_value(None).unwrap().defensive
                - bv)
                .abs()
                < 0.001
        );
        let location = CriticalLocation {
            section: BattleSection::LeftTorso,
            slot: 2,
        };
        unit.destroy_critical(location).unwrap();
        assert!(!unit.electronic_suite_available(suite).unwrap());
        assert_eq!(
            unit.mass().unwrap().equipment - base.mass().unwrap().equipment,
            mass
        );
        let restored: BattleUnit =
            serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored, unit);
    }
}
