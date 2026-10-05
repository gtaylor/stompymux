//! Caseless bin capacities, half-ton interaction, ordinary damage and saved contents.
use stompymux_rs::*;

#[test]
fn caseless_bins_have_double_capacity_and_half_ton_sizing() {
    for weapon in [
        Weapon::Ac2,
        Weapon::Ac5,
        Weapon::Ac10,
        Weapon::Ac20,
        Weapon::LightAc2,
        Weapon::LightAc5,
    ] {
        for half in [false, true] {
            let mut definition =
                MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
                    .unwrap();
            let mut part = definition.sections[&MechSection::LeftArm].criticals[&2].clone();
            part.equipment = weapon.name().into();
            part.modes = vec!["Caseless".into()];
            for slot in 2..2 + weapon.profile().critical_slots {
                definition
                    .sections
                    .get_mut(&MechSection::LeftArm)
                    .unwrap()
                    .criticals
                    .insert(slot, part.clone());
            }
            let bin = definition
                .sections
                .get_mut(&MechSection::RightTorso)
                .unwrap()
                .criticals
                .get_mut(&0)
                .unwrap();
            bin.equipment = format!("Ammo_{}", weapon.name());
            bin.data = weapon.profile().ammunition_per_ton.to_string();
            bin.modes = vec!["Caseless".into()];
            if half {
                bin.modes.push("Halfton".into());
            }
            let unit = Mech::from_template(definition).unwrap();
            let loadout = unit.loadout().unwrap();
            let index = loadout
                .weapons
                .iter()
                .position(|m| m.weapon == weapon)
                .unwrap();
            let capacity = u16::from(if half {
                weapon.profile().ammunition_per_ton / 2
            } else {
                weapon.profile().ammunition_per_ton * 2
            });
            assert_eq!(unit.ammunition(), &[capacity]);
            assert_eq!(loadout.ammunition[0].capacity, capacity);
            assert_eq!(loadout.ammunition[0].half_ton, half);
            assert_eq!(
                unit.ammunition_mode(index).unwrap(),
                AmmunitionMode::Caseless
            );
            assert_eq!(
                unit.mass().unwrap().ammunition,
                u32::from(capacity) * 1024 / u32::from(weapon.profile().ammunition_per_ton)
            );
            assert_eq!(
                weapon
                    .damage_groups_for_ammunition(AmmunitionMode::Caseless, None, 1.0)
                    .unwrap(),
                vec![u16::from(weapon.profile().damage)]
            );
            let mut saved = serde_json::to_value(&unit).unwrap();
            saved["ammunition"][0] = 1.into();
            let restored: Mech = serde_json::from_value(saved).unwrap();
            assert_eq!(restored.ammunition(), &[1]);
            assert_eq!(
                restored.ammunition_mode(index).unwrap(),
                AmmunitionMode::Caseless
            );
        }
    }
    assert!(
        Weapon::UltraAc5
            .damage_groups_for_ammunition(AmmunitionMode::Caseless, None, 1.0)
            .is_err()
    );
}
