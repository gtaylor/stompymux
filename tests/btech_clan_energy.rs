//! Clan energy mounts on supported chassis retain distinct mass, accuracy and grouped losses.
use stompymux_rs::*;

/// Construct each supported Clan energy identity without requiring unrelated Clan chassis flags.
#[test]
fn clan_energy_mounts_accuracy_and_grouped_critical_losses() {
    for &weapon in BattleWeapon::ALL
        .iter()
        .filter(|w| w.name().starts_with("CL.") && w.gunnery_skill(true) == "Gunnery-Laser")
    {
        assert_eq!(weapon.gunnery_skill(true), "Gunnery-Laser");
        assert_eq!(weapon.profile().ammunition_per_ton, 0);
        assert_eq!(weapon.profile().missiles, 0);
        assert_eq!(
            weapon.damage_groups(None).unwrap(),
            [u16::from(weapon.profile().damage)]
        );
        let mut template =
            BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                .unwrap();
        let arm = template.sections.get_mut(&BattleSection::LeftArm).unwrap();
        let mut part = arm.criticals[&2].clone();
        part.equipment = weapon.name().into();
        for slot in 2..2 + weapon.profile().critical_slots {
            arm.criticals.insert(slot, part.clone());
        }
        let unit = BattleUnit::from_template(template.clone()).unwrap();
        let loadout = unit.loadout().unwrap();
        let (index, mount) = loadout
            .weapons
            .iter()
            .enumerate()
            .find(|(_, m)| m.weapon == weapon)
            .unwrap();
        assert_eq!(
            mount.criticals.len(),
            usize::from(weapon.profile().critical_slots)
        );
        assert_eq!(
            weapon.supports_targeting_computer(),
            weapon != BattleWeapon::ClanFlamer
        );
        assert_eq!(
            weapon.supports_heat_mode(),
            weapon == BattleWeapon::ClanFlamer
        );
        for location in &mount.criticals {
            let mut damaged = unit.clone();
            assert_eq!(
                damaged.destroy_critical(*location).unwrap(),
                Some(BattleCriticalLoss::Weapon {
                    index,
                    explosion_damage: 0
                })
            );
            assert!(!damaged.weapon_readiness(index).unwrap().intact);
            assert_eq!(damaged.mass().unwrap(), unit.mass().unwrap());
        }
        if weapon.profile().critical_slots > 1 {
            template
                .sections
                .get_mut(&BattleSection::LeftArm)
                .unwrap()
                .criticals
                .remove(&2);
            assert!(BattleUnit::from_template(template).is_err());
        }
        let accuracy = if weapon.name().contains("ERLargePulse")
            || weapon.name().contains("ERMediumPulse")
            || weapon.name().contains("ERSmallPulse")
        {
            -1
        } else if weapon.name().contains("Pulse") {
            -2
        } else if weapon.name().contains("Heavy") {
            1
        } else {
            0
        };
        assert_eq!(weapon.accuracy_modifier(), accuracy);
    }
    assert_eq!(
        BattleWeapon::ClanFlamer.mass(),
        BattleWeapon::Flamer.mass() / 2
    );
    assert!(!BattleWeapon::ClanPlasmaRifle.is_flamer());
    assert_eq!(BattleWeapon::ClanPlasmaRifle.weapon_explosion_damage(), 0);
}
