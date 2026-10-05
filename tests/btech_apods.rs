//! A-Pod construction, zero-damage profiles and installed Battle Value contributions.
use stompymux_rs::*;

/// Both technology bases use the same reusable, ammunition-free catalogue behavior.
#[test]
fn apod_assets_construct_and_round_trip() {
    for asset in ["FireScorpion-1", "FireScorpion-2", "SRC-3C", "SRC-5C"] {
        let template =
            read_battle_template(&crate::support::repository_root().join("game/units"), asset)
                .unwrap();
        let unit = Mech::from_template(template).unwrap();
        let loadout = unit.loadout().unwrap();
        let pods: Vec<_> = loadout
            .weapons
            .iter()
            .filter(|mount| matches!(mount.weapon, Weapon::APod | Weapon::ClanAPod))
            .collect();
        assert!(!pods.is_empty(), "{asset}");
        for pod in pods {
            assert_eq!(pod.weapon.damage_groups(None).unwrap(), [0]);
            assert_eq!(pod.weapon.profile().ammunition_per_ton, 0);
            assert_eq!(pod.weapon.profile().heat, 0);
            assert_eq!(pod.weapon.profile().recycle_seconds, 30);
        }
        let restored: Mech = serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored, unit);
        assert_eq!(restored.mass().unwrap(), unit.mass().unwrap());
        assert_eq!(
            restored.battle_value(None).unwrap().total,
            unit.battle_value(None).unwrap().total
        );
    }
}

/// Pods add defensive value while retaining their reference offensive catalogue contribution.
#[test]
fn apod_mass_and_battle_value() {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    for section in template.sections.values_mut() {
        section.criticals.retain(|_, part| {
            Weapon::parse(&part.equipment).is_err() && !part.equipment.starts_with("Ammo_")
        });
    }
    let baseline = Mech::from_template(template.clone()).unwrap();
    for weapon in [Weapon::APod, Weapon::ClanAPod] {
        template
            .sections
            .get_mut(&MechSection::LeftArm)
            .unwrap()
            .criticals
            .insert(
                2,
                CriticalDefinition {
                    equipment: weapon.name().into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
        let unit = Mech::from_template(template.clone()).unwrap();
        assert_eq!(
            unit.mass().unwrap().equipment,
            baseline.mass().unwrap().equipment + 512
        );
        let before = baseline.battle_value(None).unwrap();
        let after = unit.battle_value(None).unwrap();
        assert_eq!(after.offensive - before.offensive, 1.0);
        assert!((after.defensive - before.defensive - 1.4).abs() < 0.001);
    }
}
