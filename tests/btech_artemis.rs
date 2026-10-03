//! Artemis controller links, mass and damage availability without duplicated persisted state.
use stompymux_rs::*;

/// Install a controller with an explicit reference to the Jenner's center-torso missile mount.
fn definition(section: BattleSection, link: &str) -> BattleTemplate {
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    template
        .sections
        .get_mut(&section)
        .unwrap()
        .criticals
        .insert(
            if section == BattleSection::Head {
                3
            } else {
                11
            },
            CriticalDefinition {
                equipment: "ArtemisIV".into(),
                data: link.into(),
                modes: vec![],
            },
        );
    if section == BattleSection::CenterTorso {
        // Retain the displaced jump jet in a free torso slot.
        template
            .sections
            .get_mut(&BattleSection::RightTorso)
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
    template
}

/// Local and head-to-torso links resolve; loss disables assistance but preserves installed mass.
#[test]
fn artemis_links_damage_and_restoration() {
    for section in [BattleSection::CenterTorso, BattleSection::Head] {
        let unit = BattleUnit::from_template(definition(section, "11")).unwrap();
        let controllers = unit.artemis_controllers().unwrap();
        assert_eq!(controllers.len(), 1);
        let controller = &controllers[0];
        assert_eq!(controller.weapon_indices.len(), 1);
        let index = controller.weapon_indices[0];
        assert_eq!(
            unit.loadout().unwrap().weapons[index].weapon,
            BattleWeapon::Srm4
        );
        assert!(unit.artemis_operational(index).unwrap());
        assert!(
            unit.critical_candidates(section)
                .contains(&controller.location)
        );
        let mut damaged = unit.clone();
        assert_eq!(
            damaged.destroy_critical(controller.location).unwrap(),
            Some(BattleCriticalLoss::System {
                system: BattleSystem::ArtemisIv
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
        let restored: BattleUnit =
            serde_json::from_value(serde_json::to_value(&damaged).unwrap()).unwrap();
        assert_eq!(
            restored.artemis_controllers().unwrap(),
            damaged.artemis_controllers().unwrap()
        );
        let mut state = serde_json::to_value(&unit).unwrap();
        state["flooded_sections"] = serde_json::json!([section]);
        let flooded: BattleUnit = serde_json::from_value(state).unwrap();
        assert!(!flooded.artemis_operational(index).unwrap());
        assert!(unit.artemis_operational(999).is_err());
    }
}

/// Empty, zero and dangling links remain visible and cannot provide guidance.
#[test]
fn artemis_unassigned_links_and_mass() {
    let standard = BattleUnit::from_template(
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    for link in ["-", "0", "1", "255"] {
        let unit = BattleUnit::from_template(definition(BattleSection::CenterTorso, link)).unwrap();
        assert_eq!(
            unit.mass().unwrap().equipment,
            standard.mass().unwrap().equipment + 1024
        );
        let controller = unit.artemis_controllers().unwrap().remove(0);
        assert!(controller.operational);
        assert!(controller.weapon_indices.is_empty());
    }
    for link in ["bad", "-1", "256"] {
        assert!(BattleUnit::from_template(definition(BattleSection::CenterTorso, link)).is_err());
    }
}

/// An Artemis V controller weighs a ton and a half, half a ton more than Artemis IV.
#[test]
fn artemis_v_controller_mass() {
    let standard = BattleUnit::from_template(definition(BattleSection::CenterTorso, "11")).unwrap();
    for flag in ["ArtemisV_Tech", "AV"] {
        let mut template = definition(BattleSection::CenterTorso, "11");
        let value = template.attributes.entry("specials".into()).or_default();
        value.push(' ');
        value.push_str(flag);
        let unit = BattleUnit::from_template(template).unwrap();
        assert_eq!(
            unit.mass().unwrap().equipment,
            standard.mass().unwrap().equipment + 512,
            "{flag}"
        );
    }
}

/// Head lookup does not hide a same-numbered center-torso launcher.
#[test]
fn head_controller_reports_both_matching_mounts() {
    let mut template = definition(BattleSection::Head, "1");
    let launcher = CriticalDefinition {
        equipment: "IS.SRM-2".into(),
        data: "-".into(),
        modes: vec![],
    };
    let life_support = template
        .sections
        .get_mut(&BattleSection::Head)
        .unwrap()
        .criticals
        .insert(0, launcher.clone())
        .unwrap();
    template
        .sections
        .get_mut(&BattleSection::LeftTorso)
        .unwrap()
        .criticals
        .insert(8, life_support);
    let center = template
        .sections
        .get_mut(&BattleSection::CenterTorso)
        .unwrap();
    let engine = center.criticals.insert(0, launcher).unwrap();
    let jet = center.criticals.insert(11, engine).unwrap();
    template
        .sections
        .get_mut(&BattleSection::RightTorso)
        .unwrap()
        .criticals
        .insert(6, jet);
    let unit = BattleUnit::from_template(template).unwrap();
    let controller = unit.artemis_controllers().unwrap().remove(0);
    assert_eq!(controller.weapon_indices.len(), 2);
    for index in controller.weapon_indices {
        assert!(unit.artemis_operational(index).unwrap());
    }
}

/// The ammunition bonus shifts the missile table by two, capped at twelve, with ordinary packet grouping.
#[test]
fn artemis_missile_tables_and_existing_archer() {
    for &weapon in BattleWeapon::ALL
        .iter()
        .filter(|weapon| weapon.profile().missiles > 0)
    {
        for roll in 2..=12 {
            assert_eq!(
                weapon
                    .damage_groups_for_ammunition(BattleAmmunitionMode::Artemis, Some(roll), 8.0)
                    .unwrap(),
                weapon
                    .damage_groups_at_range(Some((roll + 2).min(12)), 8.0)
                    .unwrap()
            );
        }
    }
    assert!(
        BattleWeapon::MediumLaser
            .damage_groups_for_ammunition(BattleAmmunitionMode::Artemis, None, 1.0)
            .is_err()
    );
    let source =
        std::fs::read_to_string(crate::support::repository_root().join("game/mechs/ARC-5R.toml"))
            .unwrap();
    let unit = BattleUnit::from_template(BattleTemplate::parse("test", &source).unwrap()).unwrap();
    for (index, mount) in unit.loadout().unwrap().weapons.iter().enumerate() {
        if mount.weapon == BattleWeapon::Lrm15 {
            assert_eq!(
                unit.ammunition_mode(index).unwrap(),
                BattleAmmunitionMode::Artemis
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
        BattleAmmunitionMode::Normal
    );
    assert_eq!(damaged.weapon_readiness(index).unwrap().ammunition, 0);
}
