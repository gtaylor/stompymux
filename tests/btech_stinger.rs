//! Stinger ammunition construction, anti-air reach and ordinary missile damage.
use stompymux_rs::*;

#[test]
fn stinger_templates_and_capacity() {
    for &weapon in BattleWeapon::ALL
        .iter()
        .filter(|weapon| weapon.supports_semiguided())
    {
        let mut definition =
            BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                .unwrap();
        let mut part = definition.sections[&BattleSection::LeftArm].criticals[&2].clone();
        part.equipment = weapon.name().into();
        part.modes = vec!["Stinger".into()];
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
        bin.modes = vec!["Stinger".into()];
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
        assert_eq!(loadout.ammunition[0].mode, BattleAmmunitionMode::Stinger);
        assert_eq!(
            unit.ammunition_mode(index).unwrap(),
            BattleAmmunitionMode::Stinger
        );
        let restored: BattleUnit =
            serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored, unit);
        for roll in [2, 7, 12] {
            assert_eq!(
                weapon
                    .damage_groups_for_ammunition(BattleAmmunitionMode::Stinger, Some(roll), 10.0)
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

#[test]
fn stinger_reach_extends_only_maximum_range() {
    let weapon = BattleWeapon::Lrm5;
    for extended in [false, true] {
        let maximum = if extended { 35.0 } else { 28.0 };
        for distance in [
            0.0, 6.0, 7.0, 14.0, 21.0, 21.001, 21.051, 28.0, 28.001, 35.0, 35.001,
        ] {
            let range = weapon
                .range_modifier_for_ammunition(
                    distance,
                    extended,
                    BattleFireMode::Normal,
                    false,
                    BattleAmmunitionMode::Stinger,
                )
                .unwrap();
            if distance > maximum {
                assert!(range.is_none());
                continue;
            }
            if distance <= 21.0 {
                assert_eq!(range, weapon.range_modifier(distance, extended).unwrap());
            } else if distance < 21.05 {
                assert_eq!(
                    range.unwrap(),
                    BattleWeaponRange {
                        bracket: BattleRangeBracket::Long,
                        modifier: 4
                    }
                );
            } else {
                assert_eq!(
                    range.unwrap(),
                    BattleWeaponRange {
                        bracket: BattleRangeBracket::Extreme,
                        modifier: 8
                    }
                );
            }
        }
    }
    for weapon in [
        BattleWeapon::MediumLaser,
        BattleWeapon::Srm4,
        BattleWeapon::StreakSrm4,
        BattleWeapon::Thumper,
    ] {
        assert!(
            weapon
                .range_modifier_for_ammunition(
                    2.0,
                    false,
                    BattleFireMode::Normal,
                    false,
                    BattleAmmunitionMode::Stinger
                )
                .is_err()
        );
    }
    assert_eq!(
        weapon
            .range_modifier_for_ammunition(
                1.0,
                false,
                BattleFireMode::Hotload,
                false,
                BattleAmmunitionMode::Stinger
            )
            .unwrap()
            .unwrap()
            .modifier,
        0
    );
}

#[test]
fn radio_tower_resolves_original_stinger_bins_and_launchers() {
    let definition =
        BattleVehicleTemplate::parse("RadioTower", include_str!("../game/mechs/RadioTower.toml"))
            .unwrap();
    let mass = definition.mass().unwrap();
    assert_eq!(mass.engine, 0);
    let loadout = BattleVehicleLoadout::resolve(&definition).unwrap();
    assert_eq!(
        loadout
            .weapons
            .iter()
            .filter(|mount| mount.initial_ammunition_mode == BattleAmmunitionMode::Stinger)
            .count(),
        1
    );
    assert_eq!(
        loadout
            .ammunition
            .iter()
            .filter(|bin| bin.mode == BattleAmmunitionMode::Stinger)
            .count(),
        1
    );
}
