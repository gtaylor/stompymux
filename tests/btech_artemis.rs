//! Artemis controller links, mass and damage availability without duplicated persisted state.
use stompymux_rs::*;

/// Install a controller with an explicit reference to the Jenner's center-torso missile mount.
fn definition(section: MechSection, link: &str) -> MechTemplate {
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    template
        .sections
        .get_mut(&section)
        .unwrap()
        .criticals
        .insert(
            if section == MechSection::Head { 3 } else { 11 },
            CriticalDefinition {
                equipment: "ArtemisIV".into(),
                data: link.into(),
                modes: vec![],
            },
        );
    if section == MechSection::CenterTorso {
        // Retain the displaced jump jet in a free torso slot.
        template
            .sections
            .get_mut(&MechSection::RightTorso)
            .unwrap()
            .criticals
            .insert(
                6,
                CriticalDefinition {
                    equipment: "JumpJet".into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
    }
    crate::support::templates::legal(template)
}

/// Local links resolve; loss disables assistance but preserves installed mass. Artemis IV
/// sits in its launcher's location (TechManual p. 206), so a head controller cannot serve the
/// center torso launcher.
#[test]
fn artemis_links_damage_and_restoration() {
    let head = MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
        .map(|mut template| {
            template
                .sections
                .get_mut(&MechSection::Head)
                .unwrap()
                .criticals
                .insert(
                    3,
                    CriticalDefinition {
                        equipment: "ArtemisIV".into(),
                        data: "11".into(),
                        modes: vec![],
                    },
                );
            template
        })
        .unwrap();
    assert!(
        Mech::from_template(crate::support::templates::legal(head))
            .unwrap_err()
            .to_string()
            .contains("Artemis IV must serve every LRM, SRM and MML launcher")
    );
    {
        let section = MechSection::CenterTorso;
        let unit = Mech::from_template(definition(section, "11")).unwrap();
        let controllers = unit.artemis_controllers().unwrap();
        assert_eq!(controllers.len(), 1);
        let controller = &controllers[0];
        assert_eq!(controller.weapon_indices.len(), 1);
        let index = controller.weapon_indices[0];
        assert_eq!(unit.loadout().unwrap().weapons[index].weapon, Weapon::Srm4);
        assert!(unit.artemis_operational(index).unwrap());
        assert!(
            unit.critical_candidates(section)
                .contains(&controller.location)
        );
        let mut damaged = unit.clone();
        assert_eq!(
            damaged.destroy_critical(controller.location).unwrap(),
            Some(CriticalLoss::System {
                system: System::ArtemisIv
            })
        );
        assert!(!damaged.artemis_operational(index).unwrap());
        assert_eq!(damaged.mass().unwrap(), unit.mass().unwrap());
        assert!(
            damaged
                .destroy_critical(controller.location)
                .unwrap()
                .is_none()
        );
        let restored: Mech =
            serde_json::from_value(serde_json::to_value(&damaged).unwrap()).unwrap();
        assert_eq!(
            restored.artemis_controllers().unwrap(),
            damaged.artemis_controllers().unwrap()
        );
        let mut state = serde_json::to_value(&unit).unwrap();
        state["flooded_sections"] = serde_json::json!([section]);
        let flooded: Mech = serde_json::from_value(state).unwrap();
        assert!(!flooded.artemis_operational(index).unwrap());
        assert!(unit.artemis_operational(999).is_err());
    }
}

/// Empty, zero and dangling links remain visible and cannot provide guidance.
#[test]
fn artemis_unassigned_links_and_mass() {
    let standard = Mech::from_template(
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    for link in ["-", "0", "1", "255"] {
        let unit = Mech::from_template(definition(MechSection::CenterTorso, link)).unwrap();
        assert_eq!(
            unit.mass().unwrap().equipment,
            standard.mass().unwrap().equipment + 1024
        );
        let controller = unit.artemis_controllers().unwrap().remove(0);
        assert!(controller.operational);
        assert!(controller.weapon_indices.is_empty());
    }
    for link in ["bad", "-1", "256"] {
        assert!(Mech::from_template(definition(MechSection::CenterTorso, link)).is_err());
    }
}

/// An Artemis V controller weighs a ton and a half across two slots, half a ton more than
/// Artemis IV (Tactical Operations: Advanced Units & Equipment p. 217).
#[test]
fn artemis_v_controller_mass() {
    let standard = Mech::from_template(definition(MechSection::CenterTorso, "11")).unwrap();
    let mut template = definition(MechSection::CenterTorso, "11");
    let torso = template
        .sections
        .get_mut(&MechSection::CenterTorso)
        .unwrap();
    torso.criticals.get_mut(&11).unwrap().equipment = "ArtemisV".into();
    template
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap()
        .criticals
        .insert(
            7,
            CriticalDefinition {
                equipment: "ArtemisV".into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    let unit = Mech::from_template(template).unwrap();
    assert_eq!(
        unit.mass().unwrap().equipment,
        standard.mass().unwrap().equipment + 512
    );
}

/// The ammunition bonus shifts the missile table by two, capped at twelve, with ordinary packet
/// grouping. Only standard LRM, SRM, MML and torpedo launchers take Artemis rounds (Total
/// Warfare p. 131); every other missile launcher refuses them.
#[test]
fn artemis_missile_tables_and_existing_archer() {
    for &weapon in Weapon::ALL
        .iter()
        .filter(|weapon| weapon.profile().missiles > 0 && !weapon.accepts_artemis())
    {
        assert!(
            weapon
                .damage_groups_for_ammunition(AmmunitionMode::Artemis, Some(7), 8.0)
                .is_err(),
            "{weapon:?}"
        );
    }
    for &weapon in Weapon::ALL.iter().filter(|weapon| weapon.accepts_artemis()) {
        assert!(weapon.profile().missiles > 0);
        for roll in 2..=12 {
            assert_eq!(
                weapon
                    .damage_groups_for_ammunition(AmmunitionMode::Artemis, Some(roll), 8.0)
                    .unwrap(),
                weapon
                    .damage_groups_at_range(Some((roll + 2).min(12)), 8.0)
                    .unwrap()
            );
        }
    }
    assert!(
        Weapon::MediumLaser
            .damage_groups_for_ammunition(AmmunitionMode::Artemis, None, 1.0)
            .is_err()
    );
    let source =
        std::fs::read_to_string(crate::support::repository_root().join("game/units/ARC-5R.toml"))
            .unwrap();
    let unit = Mech::from_template(MechTemplate::parse("test", &source).unwrap()).unwrap();
    for (index, mount) in unit.loadout().unwrap().weapons.iter().enumerate() {
        if mount.weapon == Weapon::Lrm15 {
            assert_eq!(
                unit.ammunition_mode(index).unwrap(),
                AmmunitionMode::Artemis
            );
            assert!(unit.weapon_readiness(index).unwrap().ammunition > 0);
        }
    }
    // ARC-5R controllers are unassigned, but their raw zero clears primary slot zero on a critical hit.
    let controller = unit.artemis_controllers().unwrap().remove(0);
    assert_eq!(controller.link, 0);
    let index = unit
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.criticals[0].section == controller.location.section)
        .unwrap();
    let mut damaged = unit.clone();
    damaged.destroy_critical(controller.location).unwrap();
    assert_eq!(
        damaged.ammunition_mode(index).unwrap(),
        AmmunitionMode::Normal
    );
    assert_eq!(damaged.weapon_readiness(index).unwrap().ammunition, 0);
}
