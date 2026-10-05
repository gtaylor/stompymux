//! Stinger ammunition construction, anti-air reach and ordinary missile damage.
use stompymux_rs::*;

#[test]
fn stinger_templates_and_capacity() {
    for &weapon in Weapon::ALL
        .iter()
        .filter(|weapon| weapon.supports_semiguided())
    {
        let mut definition =
            MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
        let mut part = definition.sections[&MechSection::LeftArm].criticals[&2].clone();
        part.equipment = weapon.name().into();
        part.modes = vec!["Stinger".into()];
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
        bin.modes = vec!["Stinger".into()];
        let unit = Mech::from_template(definition).unwrap();
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
        assert_eq!(loadout.ammunition[0].mode, AmmunitionMode::Stinger);
        assert_eq!(
            unit.ammunition_mode(index).unwrap(),
            AmmunitionMode::Stinger
        );
        let restored: Mech = serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
        assert_eq!(restored, unit);
        for roll in [2, 7, 12] {
            assert_eq!(
                weapon
                    .damage_groups_for_ammunition(AmmunitionMode::Stinger, Some(roll), 10.0)
                    .unwrap(),
                weapon
                    .damage_groups_for_ammunition(AmmunitionMode::Normal, Some(roll), 10.0)
                    .unwrap()
            );
        }
    }
    for weapon in [
        Weapon::Srm4,
        Weapon::MediumLaser,
        Weapon::NarcBeacon,
        Weapon::ClanStreakSrm2,
    ] {
        assert!(!weapon.supports_semiguided());
    }
}

#[test]
fn stinger_reach_extends_only_maximum_range() {
    let weapon = Weapon::Lrm5;
    for extended in [false, true] {
        let maximum = if extended { 35.0 } else { 28.0 };
        for distance in [
            0.0, 6.0, 7.0, 14.0, 21.0, 21.001, 21.051, 28.0, 28.001, 35.0, 35.001,
        ] {
            let range = weapon
                .range_modifier_for_ammunition(
                    distance,
                    extended,
                    FireMode::Normal,
                    false,
                    AmmunitionMode::Stinger,
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
                    WeaponRange {
                        bracket: RangeBracket::Long,
                        modifier: 4
                    }
                );
            } else {
                assert_eq!(
                    range.unwrap(),
                    WeaponRange {
                        bracket: RangeBracket::Extreme,
                        modifier: 8
                    }
                );
            }
        }
    }
    for weapon in [
        Weapon::MediumLaser,
        Weapon::Srm4,
        Weapon::StreakSrm4,
        Weapon::Thumper,
    ] {
        assert!(
            weapon
                .range_modifier_for_ammunition(
                    2.0,
                    false,
                    FireMode::Normal,
                    false,
                    AmmunitionMode::Stinger
                )
                .is_err()
        );
    }
    assert_eq!(
        weapon
            .range_modifier_for_ammunition(
                1.0,
                false,
                FireMode::Hotload,
                false,
                AmmunitionMode::Stinger
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
        VehicleTemplate::parse("RadioTower", include_str!("../game/units/RadioTower.toml"))
            .unwrap();
    let mass = definition.mass().unwrap();
    assert_eq!(mass.engine, 0);
    let loadout = VehicleLoadout::resolve(&definition).unwrap();
    assert_eq!(
        loadout
            .weapons
            .iter()
            .filter(|mount| mount.initial_ammunition_mode == AmmunitionMode::Stinger)
            .count(),
        1
    );
    assert_eq!(
        loadout
            .ammunition
            .iter()
            .filter(|bin| bin.mode == AmmunitionMode::Stinger)
            .count(),
        1
    );
}
