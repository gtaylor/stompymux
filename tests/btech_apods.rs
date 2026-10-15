//! A-Pod and B-Pod construction, zero-damage profiles, installed Battle Value contributions
//! and B-Pod critical explosions.
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
            restored.battle_value().unwrap().total,
            unit.battle_value().unwrap().total
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
    let baseline = Mech::from_template(crate::support::templates::legal(template.clone())).unwrap();
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
        let unit = Mech::from_template(crate::support::templates::legal(template.clone())).unwrap();
        assert_eq!(
            unit.mass().unwrap().equipment,
            baseline.mass().unwrap().equipment + 512
        );
        let before = baseline.battle_value().unwrap();
        let after = unit.battle_value().unwrap();
        // A-Pods are defensive equipment (TechManual p. 317): worth 1 before the
        // Jenner's 1.4 Defensive Factor and nothing offensively.
        assert_eq!(after.offensive, before.offensive);
        assert!((after.defensive - before.defensive - 1.4).abs() < 0.001);
    }
}

/// A B-Pod weighs a ton in one slot, counts 2 defensive BV and, like a Gauss weapon, one
/// exposed slot against the Defensive Battle Rating (TechManual pp. 204, 317), and a critical
/// hit on the unexpended pod releases 2 points of damage (Total Warfare p. 130). Without
/// infantry nothing triggers it.
#[test]
fn bpod_mass_battle_value_and_explosion() {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    for section in template.sections.values_mut() {
        section.criticals.retain(|_, part| {
            Weapon::parse(&part.equipment).is_err() && !part.equipment.starts_with("Ammo_")
        });
    }
    let baseline = Mech::from_template(crate::support::templates::legal(template.clone())).unwrap();
    for weapon in [Weapon::BPod, Weapon::ClanBPod] {
        let mut pod = template.clone();
        pod.sections
            .get_mut(&MechSection::LeftTorso)
            .unwrap()
            .criticals
            .insert(
                5,
                CriticalDefinition {
                    equipment: weapon.name().into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
        let mut unit = Mech::from_template(crate::support::templates::legal(pod)).unwrap();
        assert_eq!(
            unit.mass().unwrap().equipment,
            baseline.mass().unwrap().equipment + 1024
        );
        let before = baseline.battle_value().unwrap();
        let after = unit.battle_value().unwrap();
        assert_eq!(after.offensive, before.offensive);
        // Two defensive points less the one exposed Gauss-like slot, at the Jenner's 1.4
        // Defensive Factor.
        assert!(
            (after.defensive - before.defensive - 1.4).abs() < 0.001,
            "{} {}",
            after.defensive,
            before.defensive
        );
        let (index, mount) = unit
            .loadout()
            .unwrap()
            .weapons
            .into_iter()
            .enumerate()
            .find(|(_, mount)| mount.weapon == weapon)
            .unwrap();
        assert_eq!(mount.weapon.damage_groups(None).unwrap(), [0]);
        assert_eq!(
            unit.destroy_critical(mount.criticals[0]).unwrap(),
            Some(CriticalLoss::Weapon {
                index,
                explosion_damage: 2
            })
        );
    }
}
