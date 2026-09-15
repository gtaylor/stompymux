//! Clan missile cluster tables, grouped payloads and supported launcher construction.
use stompymux_rs::*;

/// The Clan LRM-10/15 tables differ from IS tables; SRMs and Streaks retain individual packets.
#[test]
fn clan_missile_tables_groups_and_mounts() {
    for (weapon, hits) in [
        (BattleWeapon::ClanLrm5, [1, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5]),
        (BattleWeapon::ClanLrm10, [3, 3, 4, 6, 6, 6, 6, 8, 8, 10, 10]),
        (
            BattleWeapon::ClanLrm15,
            [5, 5, 6, 9, 9, 9, 9, 12, 12, 15, 15],
        ),
        (
            BattleWeapon::ClanLrm20,
            [6, 6, 9, 12, 12, 12, 12, 16, 16, 20, 20],
        ),
        (BattleWeapon::ClanSrm2, [1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2]),
        (BattleWeapon::ClanSrm4, [1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4]),
        (BattleWeapon::ClanSrm6, [2, 2, 3, 3, 4, 4, 4, 5, 5, 6, 6]),
        (BattleWeapon::ClanStreakSrm2, [2; 11]),
        (BattleWeapon::ClanStreakSrm4, [4; 11]),
        (BattleWeapon::ClanStreakSrm6, [6; 11]),
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
            BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
        let arm = template.sections.get_mut(&BattleSection::LeftArm).unwrap();
        let mut part = arm.criticals[&2].clone();
        part.equipment = weapon.name().into();
        for slot in 2..2 + p.critical_slots {
            arm.criticals.insert(slot, part.clone());
        }
        let bin = template
            .sections
            .get_mut(&BattleSection::RightTorso)
            .unwrap()
            .criticals
            .get_mut(&0)
            .unwrap();
        bin.equipment = format!("Ammo_{}", weapon.name());
        bin.data = p.ammunition_per_ton.to_string();
        let unit = BattleUnit::from_template(template).unwrap();
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
                Some(BattleCriticalLoss::Weapon {
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
            Some(BattleCriticalLoss::Ammunition {
                index: 0,
                rounds: u16::from(p.ammunition_per_ton),
                explosion_damage: u32::from(p.ammunition_per_ton)
                    * u32::from(p.missiles)
                    * u32::from(p.damage)
            })
        );
    }
}
