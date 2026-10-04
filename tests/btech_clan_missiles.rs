//! Clan missile cluster tables, grouped payloads and supported launcher construction.
use stompymux_rs::*;

/// The Clan LRM-10/15 tables differ from IS tables; SRMs and Streaks retain individual packets.
#[test]
fn clan_missile_tables_groups_and_mounts() {
    for (weapon, hits) in [
        (Weapon::ClanLrm5, [1, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5]),
        (Weapon::ClanLrm10, [3, 3, 4, 6, 6, 6, 6, 8, 8, 10, 10]),
        (Weapon::ClanLrm15, [5, 5, 6, 9, 9, 9, 9, 12, 12, 15, 15]),
        (Weapon::ClanLrm20, [6, 6, 9, 12, 12, 12, 12, 16, 16, 20, 20]),
        (Weapon::ClanSrm2, [1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2]),
        (Weapon::ClanSrm4, [1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4]),
        (Weapon::ClanSrm6, [2, 2, 3, 3, 4, 4, 4, 5, 5, 6, 6]),
        (Weapon::ClanStreakSrm2, [2; 11]),
        (Weapon::ClanStreakSrm4, [4; 11]),
        (Weapon::ClanStreakSrm6, [6; 11]),
    ] {
        let p = weapon.profile();
        assert_eq!(p.minimum_range, 0);
        assert!(!weapon.supports_targeting_computer());
        assert_eq!(weapon.gunnery_skill(true), "Gunnery-Missile");
        for (index, count) in hits.into_iter().enumerate() {
            let roll = index as u8 + 2;
            assert_eq!(weapon.missile_hits(roll).unwrap(), count);
            let groups = weapon.damage_groups(Some(roll)).unwrap();
            assert_eq!(
                groups.iter().sum::<u16>(),
                u16::from(count) * u16::from(p.damage)
            );
            if weapon.supports_hotload() {
                assert!(groups.iter().all(|&g| (1..=5).contains(&g)));
                assert_eq!(groups.len(), usize::from(count).div_ceil(5));
            } else {
                assert_eq!(groups, vec![2; usize::from(count)]);
            }
        }
        let mut template =
            MechTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
        let arm = template.sections.get_mut(&MechSection::LeftArm).unwrap();
        let mut part = arm.criticals[&2].clone();
        part.equipment = weapon.name().into();
        for slot in 2..2 + p.critical_slots {
            arm.criticals.insert(slot, part.clone());
        }
        let bin = template
            .sections
            .get_mut(&MechSection::RightTorso)
            .unwrap()
            .criticals
            .get_mut(&0)
            .unwrap();
        bin.equipment = format!("Ammo_{}", weapon.name());
        bin.data = p.ammunition_per_ton.to_string();
        let unit = Mech::from_template(template).unwrap();
        let loadout = unit.loadout().unwrap();
        let (index, mount) = loadout
            .weapons
            .iter()
            .enumerate()
            .find(|(_, m)| m.weapon == weapon)
            .unwrap();
        for location in &mount.criticals {
            let mut damaged = unit.clone();
            assert_eq!(
                damaged.destroy_critical(*location).unwrap(),
                Some(CriticalLoss::Weapon {
                    index,
                    explosion_damage: 0
                })
            );
            assert!(!damaged.weapon_readiness(index).unwrap().intact);
        }
        let mut damaged = unit;
        assert_eq!(
            damaged
                .destroy_critical(loadout.ammunition[0].location)
                .unwrap(),
            Some(CriticalLoss::Ammunition {
                index: 0,
                rounds: u16::from(p.ammunition_per_ton),
                explosion_damage: u32::from(p.ammunition_per_ton)
                    * u32::from(p.missiles)
                    * u32::from(p.damage)
            })
        );
    }
}
