//! Weapon mode metadata may be carried by the primary slot alone or repeated through the mount.
use stompymux_rs::*;

/// Build one multi-slot installation without conflating repeated critical records with weapons.
fn template(weapon: Weapon, modes: &[&str]) -> MechTemplate {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    let jets = template.sections[&MechSection::LeftTorso].criticals.clone();
    for (slot, part) in jets {
        template
            .sections
            .get_mut(&MechSection::RightTorso)
            .unwrap()
            .criticals
            .insert(slot + 6, part);
    }
    if weapon == Weapon::Lbx10 {
        let bin = template
            .sections
            .get_mut(&MechSection::RightTorso)
            .unwrap()
            .criticals
            .get_mut(&0)
            .unwrap();
        bin.equipment = "Ammo_IS.LB10-XAC".into();
        bin.data = "10".into();
        bin.modes = vec!["LBX/Cluster".into()];
    }
    let section = template.sections.get_mut(&MechSection::LeftTorso).unwrap();
    section.criticals.clear();
    for slot in 0..weapon.profile().critical_slots {
        section.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: weapon.name().into(),
                data: "-".into(),
                modes: if slot == 0 {
                    modes.iter().map(|s| (*s).into()).collect()
                } else {
                    Vec::new()
                },
            },
        );
    }
    template
}

/// Modes belong to the mount primary; empty continuation metadata does not erase them.
#[test]
fn primary_only_modes_resolve_like_repeated_modes() {
    for (weapon, modes) in [
        (Weapon::Lbx10, vec!["LBX/Cluster", "RearMount"]),
        (Weapon::Rocket20, vec!["OneShot", "OneShot_Used"]),
        (Weapon::Lrm20, vec!["OneShot"]),
        (Weapon::LargeLaser, vec!["OnTC"]),
    ] {
        let source = template(weapon, &modes);
        let loadout = MechLoadout::resolve(&source).unwrap();
        let mount = loadout.weapons.iter().find(|m| m.weapon == weapon).unwrap();
        assert_eq!(
            mount.criticals.len(),
            usize::from(weapon.profile().critical_slots)
        );
        let unit = Mech::from_template(source.clone()).unwrap();
        let index = loadout
            .weapons
            .iter()
            .position(|m| m.weapon == weapon)
            .unwrap();
        let readiness = unit.weapon_readiness(index).unwrap();
        let restored: Mech = serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored.weapon_readiness(index).unwrap(), readiness);
        if weapon == Weapon::Lbx10 {
            assert_eq!(readiness.ammunition, 10);
            assert_eq!(
                restored.ammunition_mode(index).unwrap(),
                AmmunitionMode::Cluster
            );
        }
        if weapon == Weapon::Rocket20 {
            assert!(readiness.spent);
            assert_eq!(readiness.ammunition, 0);
        }
        let mut repeated = source.clone();
        for part in repeated
            .sections
            .get_mut(&MechSection::LeftTorso)
            .unwrap()
            .criticals
            .values_mut()
        {
            part.modes = modes.iter().map(|s| (*s).into()).collect();
        }
        assert_eq!(loadout, MechLoadout::resolve(&repeated).unwrap());
        if weapon == Weapon::Lbx10 {
            assert_eq!(mount.initial_ammunition_mode, AmmunitionMode::Cluster);
            assert!(mount.rear_mount);
        }
        if weapon == Weapon::Rocket20 {
            assert!(mount.one_shot && mount.initially_spent);
        }
        // Continuations still cannot change the primary's identity, data or modes.
        for change in 0..3 {
            let mut invalid = source.clone();
            let part = invalid
                .sections
                .get_mut(&MechSection::LeftTorso)
                .unwrap()
                .criticals
                .get_mut(&1)
                .unwrap();
            match change {
                0 => part.equipment = "IS.MediumLaser".into(),
                1 => part.data = "1".into(),
                _ => part.modes = vec!["Heat".into()],
            }
            assert!(MechLoadout::resolve(&invalid).is_err());
        }
    }
}
