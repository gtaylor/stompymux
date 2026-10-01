//! Precision bin capacities, half-ton interaction, ordinary damage and saved contents.
use stompymux_rs::*;

#[test]
fn precision_bins_have_half_capacity_without_double_halving() {
    for weapon in [
        BattleWeapon::Ac2,
        BattleWeapon::Ac5,
        BattleWeapon::Ac10,
        BattleWeapon::Ac20,
        BattleWeapon::LightAc2,
        BattleWeapon::LightAc5,
    ] {
        for half in [false, true] {
            let mut definition =
                BattleTemplate::parse("JR7-D",include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
            let mut part = definition.sections[&BattleSection::LeftArm].criticals[&2].clone();
            part.equipment = weapon.name().into();
            part.modes = vec!["Precision".into()];
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
            bin.modes = vec!["Precision".into()];
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
            let capacity = u16::from(weapon.profile().ammunition_per_ton / 2);
            assert_eq!(unit.ammunition(), &[capacity]);
            assert_eq!(loadout.ammunition[0].capacity, capacity);
            assert_eq!(loadout.ammunition[0].half_ton, half);
            assert_eq!(
                unit.ammunition_mode(index).unwrap(),
                BattleAmmunitionMode::Precision
            );
            assert_eq!(
                unit.mass().unwrap().ammunition,
                u32::from(capacity) * 1024 / u32::from(weapon.profile().ammunition_per_ton)
            );
            assert_eq!(
                weapon
                    .damage_groups_for_ammunition(BattleAmmunitionMode::Precision, None, 1.0)
                    .unwrap(),
                vec![u16::from(weapon.profile().damage)]
            );
            let mut saved = serde_json::to_value(&unit).unwrap();
            saved["ammunition"][0] = 1.into();
            let restored: BattleUnit = serde_json::from_value(saved).unwrap();
            assert_eq!(restored.ammunition(), &[1]);
            assert_eq!(
                restored.ammunition_mode(index).unwrap(),
                BattleAmmunitionMode::Precision
            );
        }
    }
    assert!(
        BattleWeapon::UltraAc5
            .damage_groups_for_ammunition(BattleAmmunitionMode::Precision, None, 1.0)
            .is_err()
    );
}
