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

/// Index of the shooter's light machine gun ammunition bin.
fn gun_bin(world: &World, shooter: ObjectId) -> usize {
    world.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .ammunition
        .iter()
        .position(|bin| bin.weapon == Weapon::LightMachineGun)
        .unwrap()
}

/// Rounds left in every bin of the shooter, on either chassis.
pub(crate) fn rounds(world: &World, shooter: ObjectId) -> u32 {
    world
        .btech
        .unit(shooter)
        .unwrap()
        .ammunition()
        .iter()
        .map(|&rounds| u32::from(rounds))
        .sum()
}

/// Select the empty hex just beyond `target`, which stands one hex north of `shooter` on the
/// one-hex-wide firing map, and fire the gun at `index` at it with the first dice seed that
/// hits.
pub(crate) fn hex_shot_that_hits(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    index: usize,
) -> HexShotReport {
    let rules = ShotRules {
        aim: AimRules {
            override_weapon_arcs: true,
        },
        ..shot_rules()
    };
    let position = world.btech.unit(target).unwrap().position().unwrap();
    let coordinate = HexCoordinate {
        x: i32::from(position.x),
        y: i32::from(position.y) - 1,
    };
    select_battle_hex_target(world, shooter, ObjectId(1), coordinate, HexTargetMode::Hex).unwrap();
    for seed in 0..=255 {
        let mut trial = world.clone();
        trial
            .btech
            .set_unit_dice(shooter, Dice::seeded([seed; 32]))
            .unwrap();
        let report =
            resolve_battle_hex_shot(&mut trial, shooter, ObjectId(1), coordinate, index, rules)
                .unwrap();
        if report.hit {
            *world = trial;
            return report;
        }
    }
    panic!("no seed hit the hex");
}

#[tokio::test]
async fn a_short_bin_fires_only_the_guns_it_can_feed() {
    for (loaded, fired) in [(1u16, 1usize), (2, 2)] {
        let (_dir, config, mut world, shooter, target, guns) = engagement().await;
        let bin = gun_bin(&world, shooter);
        world
            .btech
            .set_unit_ammunition_bin(shooter, bin, loaded)
            .unwrap();
        // The selected gun draws first, then the others in weapon order.
        let mut expected = vec![guns[1]];
        expected.extend(guns.iter().copied().filter(|gun| *gun != guns[1]));
        expected.truncate(fired);
        expected.sort_unstable();
        assert_eq!(
            world.btech.constructed_units()[&shooter]
                .array_volley(guns[1])
                .unwrap(),
            Some(expected.clone())
        );
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
        let notice = if fired == 1 {
            assert_eq!(salvo.cluster_roll, None);
            "ammunition for only one gun".to_owned()
        } else {
            let roll = salvo.cluster_roll.unwrap();
            // The Cluster Hits Table's 2 column (Total Warfare p. 114).
            let column = [1, 1, 1, 1, 1, 1, 2, 2, 2, 2, 2];
            assert_eq!(salvo.groups.len(), column[usize::from(roll) - 2]);
            format!("fires {fired} guns")
        };
        assert!(
            report
                .launch_notices
                .iter()
                .any(|line| line.text.contains(&notice)),
            "{:?}",
            report.launch_notices
        );
        let unit = &world.btech.constructed_units()[&shooter];
        assert_eq!(unit.ammunition()[bin], 0);
        for gun in &guns {
            assert_eq!(
                unit.weapon_recycle().contains_key(gun),
                expected.contains(gun),
                "{gun}"
            );
        }
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn an_array_fires_its_guns_together_at_a_hex() {
    let (_dir, config, mut world, shooter, target, guns) = engagement().await;
    let before = rounds(&world, shooter);
    let report = hex_shot_that_hits(&mut world, shooter, target, guns[1]);
    assert!(report.cluster_roll.is_some());
    assert!(
        report
            .launch_notices
            .iter()
            .any(|notice| notice.text.contains("fires 3 guns"))
    );
    assert_eq!(before - rounds(&world, shooter), 3);
    let unit = &world.btech.constructed_units()[&shooter];
    for gun in &guns {
        assert!(unit.weapon_recycle().contains_key(gun), "{gun}");
    }
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn a_destroyed_array_fires_one_gun_at_a_hex() {
    let (_dir, config, mut world, shooter, target, guns) = engagement().await;
    let array = world.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .machine_gun_arrays()
        .into_iter()
        .find(|array| array.weapon_indices.contains(&guns[0]))
        .unwrap();
    destroy_battle_critical(&mut world, shooter, array.location).unwrap();
    let before = rounds(&world, shooter);
    let report = hex_shot_that_hits(&mut world, shooter, target, guns[0]);
    assert_eq!(report.cluster_roll, None);
    assert_eq!(before - rounds(&world, shooter), 1);
    let unit = &world.btech.constructed_units()[&shooter];
    assert!(unit.weapon_recycle().contains_key(&guns[0]));
    assert!(!unit.weapon_recycle().contains_key(&guns[1]));
    world.validate(&config).unwrap();
}
