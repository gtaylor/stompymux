//! ArmorPiercing bin capacities, half-ton interaction, ordinary damage and saved contents.
use stompymux_rs::*;

#[test]
fn armor_piercing_bins_have_half_capacity_without_double_halving() {
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
                BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
            let mut part = definition.sections[&BattleSection::LeftArm].criticals[&2].clone();
            part.equipment = weapon.name().into();
            part.modes = vec!["AP".into()];
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
            bin.modes = vec!["AP".into()];
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
                BattleAmmunitionMode::ArmorPiercing
            );
            assert_eq!(
                unit.mass().unwrap().ammunition,
                u32::from(capacity) * 1024 / u32::from(weapon.profile().ammunition_per_ton)
            );
            assert_eq!(
                weapon
                    .damage_groups_for_ammunition(BattleAmmunitionMode::ArmorPiercing, None, 1.0)
                    .unwrap(),
                vec![u16::from(weapon.profile().damage)]
            );
            let mut saved = serde_json::to_value(&unit).unwrap();
            saved["ammunition"][0] = 1.into();
            let restored: BattleUnit = serde_json::from_value(saved).unwrap();
            assert_eq!(restored.ammunition(), &[1]);
            assert_eq!(
                restored.ammunition_mode(index).unwrap(),
                BattleAmmunitionMode::ArmorPiercing
            );
        }
    }
    assert!(
        BattleWeapon::UltraAc5
            .damage_groups_for_ammunition(BattleAmmunitionMode::ArmorPiercing, None, 1.0)
            .is_err()
    );
}

use crate::support;
use crate::support::btech_firing as firing;

/// The reference AP spelling shares native admission, mode changes and switch policy on all chassis.
#[tokio::test]
async fn ap_alias_matches_armor_piercing_controls_across_chassis() {
    use std::{cell::RefCell, rc::Rc};
    for source in firing::templates() {
        let (_dir, config, world, _id, _, weapon) =
            firing::fixture_with_target(&source, Some(BattleWeapon::Ac5), &source).await;
        let canonical = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let alias = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for selection in [
            weapon.to_string(),
            format!("{weapon}-{weapon}"),
            "95".into(),
        ] {
            let expected = support::run_text(
                &canonical,
                &config,
                ObjectId(1),
                1,
                &format!("armorpiercing {selection}"),
            );
            let actual =
                support::run_text(&alias, &config, ObjectId(1), 1, &format!("AP {selection}"));
            if selection == weapon.to_string() {
                assert!(
                    actual.contains(&format!("Weapon {weapon} has been set to fire AP rounds")),
                    "{actual}"
                );
            }
            assert_eq!(actual, expected);
            assert_eq!(alias.world().btech, canonical.world().btech);
        }
        let before = alias.world().btech.clone();
        let expected = support::run_text(
            &canonical,
            &config,
            ObjectId(1),
            1,
            &format!("armorpiercing/bad {weapon}"),
        );
        let actual =
            support::run_text(&alias, &config, ObjectId(1), 1, &format!("ap/bad {weapon}"));
        assert_eq!(actual, expected);
        assert_eq!(alias.world().btech, before);
    }
}
