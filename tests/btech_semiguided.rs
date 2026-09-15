//! Semi-guided profile compatibility, template ammunition, capacity and persisted mode selection.
use stompymux_rs::*;

/// Every supported indirect missile family accepts a full-size semi-guided supply without changing its capacity.
#[test]
fn semiguided_templates_and_capacity() {
    for &weapon in BattleWeapon::ALL
        .iter()
        .filter(|weapon| weapon.supports_semiguided())
    {
        let mut definition =
            BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
        let mut part = definition.sections[&BattleSection::LeftArm].criticals[&2].clone();
        part.equipment = weapon.name().into();
        part.modes = vec!["Sguided".into()];
        for slot in 2..2 + weapon.profile().critical_slots {
            definition
                .sections
                .get_mut(&BattleSection::LeftArm)
                .unwrap()
                .criticals
                .insert(slot, part.clone());
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
        bin.modes = vec!["Sguided".into()];
        let unit = BattleUnit::from_template(definition).unwrap();
        let loadout = unit.loadout().unwrap();
        let index = loadout
            .weapons
            .iter()
            .position(|mount| mount.weapon == weapon)
            .unwrap();
        assert_eq!(
            loadout.ammunition[0].capacity,
            u16::from(weapon.profile().ammunition_per_ton)
        );
        assert_eq!(loadout.ammunition[0].mode, BattleAmmunitionMode::SemiGuided);
        assert_eq!(
            unit.ammunition_mode(index).unwrap(),
            BattleAmmunitionMode::SemiGuided
        );
        let restored: BattleUnit =
            serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored, unit);
        for roll in [2, 7, 12] {
            assert_eq!(
                weapon
                    .damage_groups_for_ammunition(
                        BattleAmmunitionMode::SemiGuided,
                        Some(roll),
                        10.0
                    )
                    .unwrap(),
                weapon
                    .damage_groups_for_ammunition(BattleAmmunitionMode::Normal, Some(roll), 10.0)
                    .unwrap()
            );
        }
    }
    for weapon in [
        BattleWeapon::Srm4,
        BattleWeapon::MediumLaser,
        BattleWeapon::NarcBeacon,
        BattleWeapon::ClanStreakSrm2,
    ] {
        assert!(!weapon.supports_semiguided());
    }
}
