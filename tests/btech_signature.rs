//! Null signature equipment completeness, critical exposure and durable state validation.
use stompymux_rs::*;

/// Use one slot per non-head section; retain all existing weapons and relocate a jump jet.
fn equipped() -> BattleTemplate {
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let jet = template
        .sections
        .get_mut(&BattleSection::CenterTorso)
        .unwrap()
        .criticals
        .remove(&11)
        .unwrap();
    template
        .sections
        .get_mut(&BattleSection::LeftTorso)
        .unwrap()
        .criticals
        .insert(8, jet);
    for section in BattleSection::ALL
        .into_iter()
        .filter(|section| *section != BattleSection::Head)
    {
        let slot = if section == BattleSection::CenterTorso {
            11
        } else {
            4
        };
        template
            .sections
            .get_mut(&section)
            .unwrap()
            .criticals
            .insert(
                slot,
                CriticalDefinition {
                    equipment: "NullSig_Device".into(),
                    data: "-".into(),
                    modes: vec![],
                    brand: None,
                },
            );
    }
    template
}

/// A flag cannot substitute for a missing device; every device remains vulnerable to random criticals.
#[test]
fn nss_construction_completeness_and_critical_loss() {
    let definition = equipped();
    for section in BattleSection::ALL
        .into_iter()
        .filter(|section| *section != BattleSection::Head)
    {
        let slot = if section == BattleSection::CenterTorso {
            11
        } else {
            4
        };
        let mut unit = BattleUnit::from_template(definition.clone()).unwrap();
        assert!(unit.null_signature_available().unwrap());
        let location = CriticalLocation { section, slot };
        assert!(unit.critical_candidates(section).contains(&location));
        unit.destroy_critical(location).unwrap();
        assert!(unit.has_null_signature().unwrap());
        assert!(!unit.null_signature_available().unwrap());
        let mut incomplete = definition.clone();
        incomplete
            .attributes
            .insert("specials".into(), "NullSigSys_Tech".into());
        incomplete
            .sections
            .get_mut(&section)
            .unwrap()
            .criticals
            .remove(&slot);
        assert!(
            !BattleUnit::from_template(incomplete)
                .unwrap()
                .has_null_signature()
                .unwrap()
        );
    }
}
