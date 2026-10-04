//! Gauss catalog facts, inert ammunition and atomic weapon-explosion cascades.
use crate::support;
use stompymux_rs::*;

/// Install one supported Gauss mount and matching inert ammunition in an isolated biped template.
fn definition(weapon: BattleWeapon, case: bool) -> BattleTemplate {
    let mut definition =
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap();
    let (section, first) = if weapon == BattleWeapon::HeavyGaussRifle {
        (BattleSection::LeftTorso, 0)
    } else {
        (BattleSection::LeftArm, 2)
    };
    let arm = definition.sections.get_mut(&section).unwrap();
    let mut part = arm.criticals[&first].clone();
    part.equipment = weapon.name().into();
    arm.criticals.retain(|&slot, _| slot < first);
    for slot in first..first + weapon.profile().critical_slots {
        arm.criticals.insert(slot, part.clone());
    }
    if case {
        part.equipment = "CASE".into();
        arm.criticals.insert(11, part);
    }
    let bin = definition
        .sections
        .values_mut()
        .flat_map(|s| s.criticals.values_mut())
        .find(|part| part.equipment.starts_with("Ammo_"))
        .unwrap();
    bin.equipment = format!("Ammo_{}", weapon.name());
    bin.data = weapon.profile().ammunition_per_ton.to_string();
    definition
}

/// Every critical on an intact mount produces one explosion; all its slots become unavailable together.
#[test]
fn gauss_catalog_inert_bins_and_mount_destruction() {
    for (weapon, heat, damage, slots, capacity, mass, recycle, minimum, ranges, explosion) in [
        (
            BattleWeapon::ClanGaussRifle,
            1,
            15,
            6,
            8,
            12288,
            30,
            2,
            [7, 15, 22],
            20,
        ),
        (
            BattleWeapon::GaussRifle,
            1,
            15,
            7,
            8,
            15360,
            30,
            2,
            [7, 15, 22],
            20,
        ),
        (
            BattleWeapon::LightGaussRifle,
            1,
            8,
            5,
            16,
            12288,
            20,
            3,
            [8, 17, 25],
            16,
        ),
        (
            BattleWeapon::MagshotGaussRifle,
            1,
            2,
            2,
            50,
            512,
            12,
            0,
            [3, 6, 9],
            3,
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
            (heat, damage, slots, capacity, recycle)
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
        assert_eq!(weapon.weapon_explosion_damage(), explosion);
        assert_eq!(weapon.ammunition_explosion_damage(u16::MAX), 0);
        let intact = BattleUnit::from_template(definition(weapon, false)).unwrap();
        assert!(intact.ammunition_hazard_maximum().unwrap().is_none());
        let loadout = intact.loadout().unwrap();
        let bin = loadout.ammunition[0].location;
        let mut empty = intact.clone();
        assert_eq!(
            empty.destroy_critical(bin).unwrap(),
            Some(BattleCriticalLoss::Ammunition {
                index: 0,
                rounds: u16::from(capacity),
                explosion_damage: 0,
            })
        );
        assert_eq!(empty.ammunition(), [0]);
        let (index, mount) = loadout
            .weapons
            .iter()
            .enumerate()
            .find(|(_, mount)| mount.weapon == weapon)
            .unwrap();
        for location in &mount.criticals {
            let mut unit = intact.clone();
            assert_eq!(
                unit.destroy_critical(*location).unwrap(),
                Some(BattleCriticalLoss::Weapon {
                    index,
                    explosion_damage: explosion
                })
            );
            for slot in &mount.criticals {
                assert!(unit.lost_criticals().contains(slot));
                assert_eq!(unit.destroy_critical(*slot).unwrap(), None);
            }
            assert!(!unit.weapon_intact(index).unwrap());
            assert_eq!(unit.ammunition(), intact.ammunition());
        }
    }
    let highlander = BattleUnit::from_template(
        BattleTemplate::parse("HGN-732", include_str!("../game/mechs/HGN-732.toml")).unwrap(),
    )
    .unwrap();
    assert!(
        highlander
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .any(|mount| mount.weapon == BattleWeapon::GaussRifle)
    );
}

/// Internal Gauss explosions bypass armor, honor CASE and replay exactly through persistence.
#[tokio::test]
async fn gauss_explosion_cascades_case_and_restart() {
    for weapon in [
        BattleWeapon::GaussRifle,
        BattleWeapon::LightGaussRifle,
        BattleWeapon::MagshotGaussRifle,
    ] {
        for case in [false, true] {
            let (_dir, config, mut base) = support::isolated_world().await;
            let id = base.create(&config, "Gauss target".into(), Kind::Thing);
            base.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
            create_battle_unit(&mut base, id, definition(weapon, case)).unwrap();
            support::seed_object_dice(&mut base, id, support::FIXTURE_DICE_SEED);
            let hit = BattleHit {
                section: BattleSection::LeftArm,
                rear_armor: false,
                through_armor_critical: true,
                crew_stun: false,
            };
            let mut found = false;
            for seed in 0..=255 {
                base.btech
                    .set_unit_dice(id, BattleDice::seeded([seed; 32]))
                    .unwrap();
                let mut fired = base.clone();
                let report = resolve_battle_impact(&mut fired, id, hit, 1).unwrap();
                if !report.criticals.iter().any(|(_, loss)| matches!(loss, BattleCriticalLoss::Weapon { explosion_damage, .. } if *explosion_damage > 0)) { continue; }
                assert_eq!(
                    report
                        .pending_effects
                        .iter()
                        .filter(|effect| **effect == BattleImpactEffect::ExplosionInjury)
                        .count(),
                    1
                );
                let before = &base.btech.constructed_units()[&id];
                let after = &fired.btech.constructed_units()[&id];
                let internal_loss: u16 = before
                    .sections()
                    .iter()
                    .map(|(section, state)| state.internal - after.sections()[section].internal)
                    .sum();
                assert_eq!(
                    internal_loss,
                    u16::from(if case {
                        weapon.weapon_explosion_damage().min(6)
                    } else {
                        weapon.weapon_explosion_damage()
                    })
                );
                assert_eq!(
                    after.sections()[&BattleSection::CenterTorso].armor,
                    before.sections()[&BattleSection::CenterTorso].armor
                );
                if case {
                    assert_eq!(
                        after.sections()[&BattleSection::LeftTorso],
                        before.sections()[&BattleSection::LeftTorso]
                    );
                }
                persistence::save(&config.database(), &base).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                assert_eq!(
                    resolve_battle_impact(&mut restored, id, hit, 1).unwrap(),
                    report
                );
                assert_eq!(restored.btech, fired.btech);
                let repeated = resolve_battle_impact(&mut fired, id, hit, 1).unwrap();
                assert!(
                    !repeated
                        .pending_effects
                        .contains(&BattleImpactEffect::ExplosionInjury)
                );
                found = true;
                break;
            }
            assert!(found, "No seed triggered {weapon:?}, CASE={case}");
        }
    }
}

/// The Heavy Gauss catalog uses an eleven-slot torso mount and a larger weapon explosion.
#[test]
fn heavy_gauss_catalog_and_mount_destruction() {
    let weapon = BattleWeapon::HeavyGaussRifle;
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
        (2, 25, 11, 4, 30)
    );
    assert_eq!(
        (
            profile.minimum_range,
            profile.short_range,
            profile.medium_range,
            profile.long_range
        ),
        (4, 6, 13, 20)
    );
    assert_eq!(weapon.mass(), 18432);
    assert_eq!(weapon.ammunition_explosion_damage(4), 0);
    let original = BattleUnit::from_template(definition(weapon, false)).unwrap();
    let loadout = original.loadout().unwrap();
    let (index, mount) = loadout
        .weapons
        .iter()
        .enumerate()
        .find(|(_, mount)| mount.weapon == weapon)
        .unwrap();
    for location in &mount.criticals {
        let mut unit = original.clone();
        assert_eq!(
            unit.destroy_critical(*location).unwrap(),
            Some(BattleCriticalLoss::Weapon {
                index,
                explosion_damage: 25
            })
        );
        assert!(
            mount
                .criticals
                .iter()
                .all(|location| unit.lost_criticals().contains(location))
        );
    }
}
