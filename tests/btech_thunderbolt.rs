//! Single-missile Thunderbolt payloads, live supply and hotloaded critical hazards.
use stompymux_rs::*;

/// Build a launcher in an arm with a matching torso magazine.
fn definition(weapon: BattleWeapon, hotload: bool) -> BattleTemplate {
    let mut template = BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
    let arm = template.sections.get_mut(&BattleSection::LeftArm).unwrap();
    let mut part = arm.criticals[&2].clone();
    part.equipment = weapon.name().into();
    if hotload {
        part.modes.push("Hotload".into());
    }
    for slot in 2..2 + weapon.profile().critical_slots {
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
    bin.data = weapon.profile().ammunition_per_ton.to_string();
    template
}

/// Every cluster result retains one whole missile, including Artemis-adjusted results.
#[test]
fn thunderbolt_clusters_supply_and_mount_losses() {
    for weapon in [
        BattleWeapon::Thunderbolt5,
        BattleWeapon::Thunderbolt10,
        BattleWeapon::Thunderbolt15,
        BattleWeapon::Thunderbolt20,
    ] {
        let p = weapon.profile();
        assert!(weapon.is_thunderbolt());
        assert!(!weapon.is_streak());
        assert!(!weapon.supports_targeting_computer());
        assert!(weapon.supports_hotload());
        assert_eq!(weapon.gunnery_skill(true), "Gunnery-Missile");
        for roll in 2..=12 {
            assert_eq!(weapon.missile_hits(roll).unwrap(), 1);
            for mode in [BattleAmmunitionMode::Normal, BattleAmmunitionMode::Artemis] {
                assert_eq!(
                    weapon
                        .damage_groups_for_ammunition(mode, Some(roll), 1.0)
                        .unwrap(),
                    [u16::from(p.damage)]
                );
            }
        }
        assert!(weapon.damage_groups(None).is_err());
        for hotload in [false, true] {
            let unit = BattleUnit::from_template(definition(weapon, hotload)).unwrap();
            let loadout = unit.loadout().unwrap();
            let (index, mount) = loadout
                .weapons
                .iter()
                .enumerate()
                .find(|(_, m)| m.weapon == weapon)
                .unwrap();
            assert_eq!(mount.criticals.len(), usize::from(p.critical_slots));
            for rounds in [0, 1, p.ammunition_per_ton] {
                let mut state = serde_json::to_value(&unit).unwrap();
                state["ammunition"][0] = rounds.into();
                let supplied: BattleUnit = serde_json::from_value(state).unwrap();
                assert_eq!(
                    supplied.weapon_readiness(index).unwrap().ammunition,
                    u32::from(rounds)
                );
                for location in &mount.criticals {
                    let mut damaged = supplied.clone();
                    assert_eq!(
                        damaged.destroy_critical(*location).unwrap(),
                        Some(BattleCriticalLoss::Weapon {
                            index,
                            explosion_damage: if hotload && rounds > 0 { p.damage } else { 0 }
                        })
                    );
                    assert!(!damaged.weapon_readiness(index).unwrap().intact);
                }
                let mut damaged = supplied;
                assert_eq!(
                    damaged
                        .destroy_critical(loadout.ammunition[0].location)
                        .unwrap(),
                    Some(BattleCriticalLoss::Ammunition {
                        index: 0,
                        rounds: u16::from(rounds),
                        explosion_damage: u32::from(rounds) * u32::from(p.damage)
                    })
                );
            }
        }
    }
}
