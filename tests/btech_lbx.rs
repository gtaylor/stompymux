//! LB-X catalogs, pellet distributions and explicit template ammunition selection.
use stompymux_rs::*;

/// Build a catalog-valid torso mount with one bin of each ammunition type.
fn definition(weapon: BattleWeapon, cluster: bool) -> BattleTemplate {
    let mut template =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let mut part = template.sections[&BattleSection::LeftArm].criticals[&2].clone();
    part.equipment = weapon.name().into();
    if cluster {
        part.modes.push("LBX/Cluster".into());
    }
    for slot in 0..weapon.profile().critical_slots {
        template
            .sections
            .get_mut(&BattleSection::LeftTorso)
            .unwrap()
            .criticals
            .insert(slot, part.clone());
    }
    let torso = template
        .sections
        .get_mut(&BattleSection::RightTorso)
        .unwrap();
    let mut bin = torso.criticals[&0].clone();
    bin.equipment = format!("Ammo_{}", weapon.name());
    bin.data = weapon.profile().ammunition_per_ton.to_string();
    torso.criticals.insert(0, bin.clone());
    bin.modes.push("LBX/Cluster".into());
    torso.criticals.insert(3, bin);
    template
}

/// Slug profiles and cluster probabilities remain distinct even though they share one weapon.
#[test]
fn lbx_catalog_cluster_distribution_and_template_modes() {
    for (weapon, heat, damage, slots, ammo, mass, recycle, minimum, ranges, total) in [
        (
            BattleWeapon::ClanLbx2,
            1,
            2,
            3,
            45,
            5120,
            15,
            4,
            [10, 20, 30],
            51,
        ),
        (
            BattleWeapon::ClanLbx5,
            1,
            5,
            4,
            20,
            7168,
            20,
            3,
            [8, 15, 24],
            114,
        ),
        (
            BattleWeapon::ClanLbx10,
            2,
            10,
            5,
            10,
            10240,
            25,
            0,
            [6, 12, 18],
            227,
        ),
        (
            BattleWeapon::ClanLbx20,
            6,
            20,
            9,
            5,
            12288,
            30,
            0,
            [4, 8, 12],
            457,
        ),
        (
            BattleWeapon::Lbx2,
            1,
            2,
            4,
            45,
            6144,
            15,
            4,
            [9, 18, 27],
            51,
        ),
        (
            BattleWeapon::Lbx5,
            1,
            5,
            5,
            20,
            8192,
            20,
            3,
            [7, 14, 21],
            114,
        ),
        (
            BattleWeapon::Lbx10,
            2,
            10,
            6,
            10,
            11264,
            25,
            0,
            [6, 12, 18],
            227,
        ),
        (
            BattleWeapon::Lbx20,
            6,
            20,
            11,
            5,
            14336,
            30,
            0,
            [4, 8, 12],
            457,
        ),
    ] {
        let profile = weapon.profile();
        assert_eq!(BattleWeapon::parse(weapon.name()).unwrap(), weapon);
        assert_eq!(
            (
                profile.heat,
                profile.damage,
                profile.critical_slots,
                profile.ammunition_per_ton,
                profile.recycle_seconds
            ),
            (heat, damage, slots, ammo, recycle)
        );
        assert_eq!(
            (
                profile.minimum_range,
                profile.short_range,
                profile.medium_range,
                profile.long_range
            ),
            (minimum, ranges[0], ranges[1], ranges[2])
        );
        assert_eq!(weapon.mass(), mass);
        assert_eq!(weapon.gunnery_skill(true), "Gunnery-Ballistic");
        assert_eq!(weapon.damage_groups(None).unwrap(), [u16::from(damage)]);
        let mut sum = 0;
        for a in 1..=6 {
            for b in 1..=6 {
                let groups = weapon
                    .damage_groups_for_ammunition(BattleAmmunitionMode::Cluster, Some(a + b), 1.0)
                    .unwrap();
                assert!(groups.iter().all(|&damage| damage == 1));
                assert_eq!(
                    groups.len(),
                    usize::from(weapon.missile_hits(a + b).unwrap())
                );
                sum += groups.len();
            }
        }
        assert_eq!(sum, total);
        assert!(
            weapon
                .damage_groups_for_ammunition(BattleAmmunitionMode::Cluster, None, 1.0)
                .is_err()
        );
        assert!(
            weapon
                .damage_groups_for_ammunition(BattleAmmunitionMode::Normal, Some(7), 1.0)
                .is_err()
        );
        for cluster in [false, true] {
            let unit = BattleUnit::from_template(definition(weapon, cluster)).unwrap();
            let loadout = unit.loadout().unwrap();
            let index = loadout
                .weapons
                .iter()
                .position(|mount| mount.weapon == weapon)
                .unwrap();
            let mode = if cluster {
                BattleAmmunitionMode::Cluster
            } else {
                BattleAmmunitionMode::Normal
            };
            assert_eq!(unit.ammunition_mode(index).unwrap(), mode);
            assert_eq!(loadout.ammunition[0].mode, BattleAmmunitionMode::Normal);
            assert_eq!(loadout.ammunition[1].mode, BattleAmmunitionMode::Cluster);
            assert_eq!(
                unit.weapon_readiness(index).unwrap().ammunition,
                u32::from(ammo)
            );
            assert_eq!(
                unit.ammunition_hazard_maximum().unwrap().unwrap().damage,
                u32::from(ammo) * u32::from(damage)
            );
        }
    }
}

/// The existing UrbanMech carries a supported slug-fed LB-X autocannon without asset changes.
#[test]
fn existing_lbx_urbanmech_constructs() {
    let unit = BattleUnit::from_template(
        BattleTemplate::parse("UM-R63", include_str!("../game/mechs/UM-R63.toml")).unwrap(),
    )
    .unwrap();
    assert!(
        unit.loadout()
            .unwrap()
            .weapons
            .iter()
            .any(|mount| mount.weapon == BattleWeapon::Lbx10)
    );
}
