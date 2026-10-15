//! Machine gun arrays: firing a linked gun fires every ready linked gun as one attack on the
//! Cluster Hits Table column for the number of guns, all hits in one location (Total Warfare
//! p. 137), and a destroyed array leaves the guns firing alone.
use crate::support;
use stompymux_rs::*;

/// Direct-fire rules for these shots.
fn shot_rules() -> ShotRules {
    ShotRules {
        tsm_tow_bonus: true,
        vehicle_impact: VehicleImpactRules::STANDARD,
        aim: AimRules::STANDARD,
        extended_gunnery: false,
        extended_piloting: false,
        target_toughness: false,
    }
}

/// A Locust LCT-5T, whose arms each hold three light machine guns and an array, a hex from a
/// Jenner, with dice that land the first shot.
async fn engagement() -> (
    tempfile::TempDir,
    Config,
    World,
    ObjectId,
    ObjectId,
    Vec<usize>,
) {
    let (dir, config, world) = support::isolated_world().await;
    let (mut world, shooter, target, _) = support::btech_firing::supply_fixture_on(
        world,
        &config,
        include_str!("fixtures/btech/units/LCT-5T.toml"),
        None,
        include_str!("fixtures/btech/units/JR7-D.toml"),
        false,
        None,
    );
    let unit = &world.btech.constructed_units()[&shooter];
    let loadout = unit.loadout().unwrap();
    let guns: Vec<_> = loadout
        .weapons
        .iter()
        .enumerate()
        .filter(|(_, mount)| {
            mount.weapon == Weapon::LightMachineGun
                && mount.criticals[0].section == MechSection::LeftArm
        })
        .map(|(index, _)| index)
        .collect();
    assert_eq!(guns.len(), 3);
    let value = (0..=255)
        .find(|value| Dice::seeded([*value; 32]).two_d6() == 12)
        .unwrap();
    world
        .btech
        .set_unit_dice(shooter, Dice::seeded([value; 32]))
        .unwrap();
    (dir, config, world, shooter, target, guns)
}

#[tokio::test]
async fn an_array_fires_its_guns_as_one_cluster_attack() {
    let (_dir, config, mut world, shooter, target, guns) = engagement().await;
    let unit = &world.btech.constructed_units()[&shooter];
    assert_eq!(unit.array_volley(guns[1]).unwrap(), Some(guns.clone()));
    let ammunition: u32 = unit
        .ammunition()
        .iter()
        .map(|&rounds| u32::from(rounds))
        .sum();
    let report = resolve_battle_shot(
        &mut world,
        shooter,
        ObjectId(1),
        target,
        guns[1],
        shot_rules(),
    )
    .unwrap();
    assert!(report.hit);
    let Some(TargetSalvo::Mech(salvo)) = &report.salvo else {
        panic!("a Mech salvo: {:?}", report.salvo);
    };
    let roll = salvo.cluster_roll.unwrap();
    // The Cluster Hits Table's 3 column (Total Warfare p. 114).
    let column = [1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3];
    assert_eq!(
        salvo.groups.len(),
        column[usize::from(roll) - 2],
        "roll {roll}"
    );
    let section = salvo.groups[0].hit.section;
    assert!(
        salvo
            .groups
            .iter()
            .all(|group| group.hit.section == section)
    );
    assert!(salvo.groups.iter().all(|group| group.damage == 1));
    let unit = &world.btech.constructed_units()[&shooter];
    for gun in &guns {
        assert!(unit.weapon_recycle().contains_key(gun), "{gun}");
    }
    let spent: u32 = unit
        .ammunition()
        .iter()
        .map(|&rounds| u32::from(rounds))
        .sum();
    assert_eq!(ammunition - spent, 3);
    assert!(
        report
            .launch_notices
            .iter()
            .any(|notice| notice.text.contains("fires 3 guns"))
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn a_destroyed_array_leaves_the_guns_firing_alone() {
    let (_dir, config, mut world, shooter, target, guns) = engagement().await;
    let array = world.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .machine_gun_arrays()
        .into_iter()
        .find(|array| array.weapon_indices.contains(&guns[0]))
        .unwrap();
    destroy_battle_critical(&mut world, shooter, array.location).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&shooter]
            .array_volley(guns[0])
            .unwrap(),
        None
    );
    let report = resolve_battle_shot(
        &mut world,
        shooter,
        ObjectId(1),
        target,
        guns[0],
        shot_rules(),
    )
    .unwrap();
    assert!(report.hit);
    let unit = &world.btech.constructed_units()[&shooter];
    assert!(unit.weapon_recycle().contains_key(&guns[0]));
    assert!(!unit.weapon_recycle().contains_key(&guns[1]));
    world.validate(&config).unwrap();
}
