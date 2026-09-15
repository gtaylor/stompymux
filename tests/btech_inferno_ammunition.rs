//! Inferno construction flags and woodland ignition share the live ammunition identity.
use stompymux_rs::*;

#[test]
fn inferno_ammunition_templates_preserve_capacity_modes_and_mass() {
    for weapon in [
        BattleWeapon::Srm2,
        BattleWeapon::Srm4,
        BattleWeapon::Srm6,
        BattleWeapon::StreakSrm2,
        BattleWeapon::StreakSrm4,
        BattleWeapon::StreakSrm6,
        BattleWeapon::ClanSrm4,
        BattleWeapon::ClanStreakSrm6,
        BattleWeapon::Lrm20,
        BattleWeapon::NarcBeacon,
    ] {
        for half in [false, true] {
            let mut definition =
                BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
            let arm = definition
                .sections
                .get_mut(&BattleSection::LeftArm)
                .unwrap();
            let mut part = arm.criticals[&2].clone();
            part.equipment = weapon.name().into();
            part.modes = vec!["Inferno".into()];
            arm.criticals.retain(|slot, _| *slot < 2);
            for slot in 2..2 + weapon.profile().critical_slots {
                arm.criticals.insert(slot, part.clone());
            }
            let bin = definition
                .sections
                .get_mut(&BattleSection::RightTorso)
                .unwrap()
                .criticals
                .get_mut(&0)
                .unwrap();
            bin.equipment = format!("Ammo_{}", weapon.name());
            bin.data = weapon.profile().ammunition_per_ton.to_string();
            bin.modes = vec!["Inferno".into()];
            if half {
                bin.modes.push("Halfton".into());
            }
            let unit = BattleUnit::from_template(definition).unwrap();
            let loadout = unit.loadout().unwrap();
            let index = loadout
                .weapons
                .iter()
                .position(|m| m.weapon == weapon)
                .unwrap();
            assert_eq!(
                unit.ammunition_mode(index).unwrap(),
                BattleAmmunitionMode::Inferno
            );
            let capacity = weapon.profile().ammunition_per_ton / if half { 2 } else { 1 };
            assert_eq!(loadout.ammunition[0].mode, BattleAmmunitionMode::Inferno);
            assert_eq!(unit.ammunition(), &[u16::from(capacity)]);
            assert_eq!(
                unit.mass().unwrap().ammunition,
                u32::from(capacity) * 1024 / u32::from(weapon.profile().ammunition_per_ton)
            );
            assert_eq!(
                serde_json::from_value::<BattleUnit>(serde_json::to_value(&unit).unwrap()).unwrap(),
                unit
            );
        }
    }
}

#[test]
fn inferno_ammunition_ignites_small_missiles_without_offering_armor_packets() {
    assert_eq!(
        BattleWeapon::Srm2.terrain_ignition_target(BattleAmmunitionMode::Normal),
        None
    );
    assert_eq!(
        BattleWeapon::Srm2.terrain_ignition_target(BattleAmmunitionMode::Inferno),
        Some(5)
    );
    assert!(
        BattleWeapon::Srm4
            .damage_groups_for_ammunition(BattleAmmunitionMode::Inferno, Some(7), 1.0)
            .is_err()
    );
    let mut ignited = false;
    for seed in 0..32 {
        let mut dice = BattleDice::seeded([seed; 32]);
        let effect = resolve_woodland_effect(
            Terrain::HeavyForest,
            BattleWeapon::Srm2,
            BattleAmmunitionMode::Inferno,
            0,
            BattleWoodlandIntent::Ignite,
            &mut dice,
        );
        ignited |= matches!(effect, BattleWoodlandEffect::Ignite { .. });
    }
    assert!(ignited);
}
