//! Rocket catalogs and self-contained launcher construction, including spent template state.
use stompymux_rs::*;

/// Put one launcher in a conventional torso while retaining the fixture's external ammunition.
fn definition(weapon: BattleWeapon, modes: &[&str]) -> BattleTemplate {
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let mut part = template.sections[&BattleSection::LeftArm].criticals[&2].clone();
    part.equipment = weapon.name().into();
    part.modes = modes.iter().map(|s| s.to_string()).collect();
    for slot in 0..weapon.profile().critical_slots {
        template
            .sections
            .get_mut(&BattleSection::LeftTorso)
            .unwrap()
            .criticals
            .insert(slot, part.clone());
    }
    template
}

/// Rocket clusters use LRM distributions but their own ranges, mass and accuracy.
#[test]
fn rocket_catalog_clusters_and_one_shot_templates() {
    for (weapon, counterpart, heat, slots, ranges) in [
        (
            BattleWeapon::Rocket10,
            BattleWeapon::Lrm10,
            3,
            1,
            [5, 11, 18],
        ),
        (
            BattleWeapon::Rocket15,
            BattleWeapon::Lrm15,
            4,
            2,
            [4, 9, 15],
        ),
        (
            BattleWeapon::Rocket20,
            BattleWeapon::Lrm20,
            5,
            3,
            [3, 7, 12],
        ),
    ] {
        assert_eq!(BattleWeapon::parse(weapon.name()).unwrap(), weapon);
        let p = weapon.profile();
        assert_eq!(
            (
                p.heat,
                p.damage,
                p.critical_slots,
                p.recycle_seconds,
                p.ammunition_per_ton
            ),
            (heat, 1, slots, 30, 0)
        );
        assert_eq!([p.short_range, p.medium_range, p.long_range], ranges);
        assert_eq!(p.minimum_range, 0);
        assert_eq!(weapon.mass(), u32::from(slots) * 512);
        assert_eq!(weapon.accuracy_modifier(), 1);
        assert_eq!(weapon.gunnery_skill(true), "Gunnery-Missile");
        for a in 1..=6 {
            for b in 1..=6 {
                assert_eq!(
                    weapon.damage_groups(Some(a + b)).unwrap(),
                    counterpart.damage_groups(Some(a + b)).unwrap()
                );
            }
        }
        assert!(BattleLoadout::resolve(&definition(weapon, &[])).is_err());
        for spent in [false, true] {
            let modes = if spent {
                vec!["OneShot", "OneShot_Used"]
            } else {
                vec!["OneShot"]
            };
            let template = definition(weapon, &modes);
            let unit = BattleUnit::from_template(template).unwrap();
            let loadout = unit.loadout().unwrap();
            let (index, mount) = loadout
                .weapons
                .iter()
                .enumerate()
                .find(|(_, m)| m.weapon == weapon)
                .unwrap();
            assert!(mount.one_shot);
            assert_eq!(mount.initially_spent, spent);
            let ready = unit.weapon_readiness(index).unwrap();
            assert_eq!(ready.spent, spent);
            assert_eq!(ready.ammunition, u32::from(!spent));
        }
    }
    for (weapon, modes) in [
        (BattleWeapon::Srm4, vec!["OneShot_Used"]),
        (BattleWeapon::Srm4, vec!["OneShot", "OneShot"]),
        (BattleWeapon::MediumLaser, vec!["OneShot"]),
        (BattleWeapon::Ac2, vec!["OneShot"]),
    ] {
        assert!(BattleLoadout::resolve(&definition(weapon, &modes)).is_err());
    }
}

/// The existing Commando supplies six independent rocket salvos without external bins.
#[test]
fn rocket_commando_constructs_unchanged() {
    let source = std::fs::read_to_string("game/mechs/COM-4H.toml").unwrap();
    let unit = BattleUnit::from_template(BattleTemplate::parse("test", &source).unwrap()).unwrap();
    let loadout = unit.loadout().unwrap();
    assert!(loadout.ammunition.is_empty());
    assert_eq!(
        loadout
            .weapons
            .iter()
            .filter(|m| m.weapon == BattleWeapon::Rocket15 && m.one_shot)
            .count(),
        6
    );
}
