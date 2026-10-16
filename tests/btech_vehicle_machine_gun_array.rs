//! Machine gun arrays on vehicle and VTOL shooters: firing a linked gun fires every ready
//! linked gun as one attack on the Cluster Hits Table column for the number of guns, all hits
//! in one location (Total Warfare p. 137), and a destroyed array leaves the guns firing alone.
use crate::btech_machine_gun_array::{hex_shot_that_hits, rounds};
use crate::btech_vehicle_apollo::{engagement, rules, seed_roll, shooters};
use stompymux_rs::*;

/// Front slots of three machine guns, the array linking them and their ammunition.
const FRONT: &str = "{ at = \"1-3\", item = \"IS.MachineGun\" },\n    { at = 4, item = \"IS.MGA\" },\n    { at = 5, item = \"Ammo_IS.MachineGun\", rounds = 200 },";

/// The array module's slot in the front.
const ARRAY: VehicleCriticalLocation = VehicleCriticalLocation {
    section: VehicleSection::Front,
    slot: 3,
};

/// Indices of the shooter's front machine guns.
fn guns(world: &World, shooter: ObjectId) -> Vec<usize> {
    world.btech.vehicles()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .enumerate()
        .filter(|(_, mount)| mount.weapon == Weapon::MachineGun)
        .map(|(index, _)| index)
        .collect()
}

#[tokio::test]
async fn a_vehicle_array_fires_its_guns_as_one_cluster_attack() {
    // The Cluster Hits Table's 3 column (Total Warfare p. 114).
    let column = [1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3];
    for source in shooters(FRONT) {
        let (_dir, config, mut world, shooter, target, _) =
            engagement(&source, Weapon::MachineGun).await;
        let guns = guns(&world, shooter);
        assert_eq!(guns.len(), 3);
        let unit = &world.btech.vehicles()[&shooter];
        assert_eq!(unit.array_volley(guns[1]).unwrap(), Some(guns.clone()));
        assert_eq!(
            unit.loadout().unwrap().machine_gun_arrays()[0].location,
            ARRAY
        );
        let before = rounds(&world, shooter);
        seed_roll(&mut world, shooter, 12);
        let report =
            fire_battle_vehicle_shot(&mut world, shooter, ObjectId(1), target, guns[1], rules())
                .unwrap();
        assert!(report.hit);
        let Some(TargetSalvo::Mech(salvo)) = &report.salvo else {
            panic!("a Mech salvo: {:?}", report.salvo);
        };
        let roll = salvo.cluster_roll.unwrap();
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
        assert!(salvo.groups.iter().all(|group| group.damage == 2));
        let unit = &world.btech.vehicles()[&shooter];
        for gun in &guns {
            assert!(unit.weapon_recycle().contains_key(gun), "{gun}");
        }
        assert_eq!(before - rounds(&world, shooter), 3);
        assert!(
            report
                .launch_notices
                .iter()
                .any(|notice| notice.text.contains("fires 3 guns"))
        );
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn a_destroyed_vehicle_array_leaves_the_guns_firing_alone() {
    for source in shooters(FRONT) {
        let (_dir, config, mut world, shooter, target, _) =
            engagement(&source, Weapon::MachineGun).await;
        let guns = guns(&world, shooter);
        destroy_battle_vehicle_critical(&mut world, shooter, ARRAY).unwrap();
        assert_eq!(
            world.btech.vehicles()[&shooter]
                .array_volley(guns[0])
                .unwrap(),
            None
        );
        let before = rounds(&world, shooter);
        seed_roll(&mut world, shooter, 12);
        let report =
            fire_battle_vehicle_shot(&mut world, shooter, ObjectId(1), target, guns[0], rules())
                .unwrap();
        assert!(report.hit);
        let Some(TargetSalvo::Mech(salvo)) = &report.salvo else {
            panic!("a Mech salvo: {:?}", report.salvo);
        };
        assert_eq!(salvo.cluster_roll, None);
        let unit = &world.btech.vehicles()[&shooter];
        assert!(unit.weapon_recycle().contains_key(&guns[0]));
        assert!(!unit.weapon_recycle().contains_key(&guns[1]));
        assert_eq!(before - rounds(&world, shooter), 1);
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn a_short_vehicle_bin_fires_only_the_guns_it_can_feed() {
    for source in shooters(FRONT) {
        for (loaded, fired) in [(1u16, 1usize), (2, 2)] {
            let (_dir, config, mut world, shooter, target, _) =
                engagement(&source, Weapon::MachineGun).await;
            let guns = guns(&world, shooter);
            let bin = world.btech.vehicles()[&shooter]
                .loadout()
                .unwrap()
                .ammunition
                .iter()
                .position(|bin| bin.weapon == Weapon::MachineGun)
                .unwrap();
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
                world.btech.vehicles()[&shooter]
                    .array_volley(guns[1])
                    .unwrap(),
                Some(expected.clone())
            );
            seed_roll(&mut world, shooter, 12);
            let report = fire_battle_vehicle_shot(
                &mut world,
                shooter,
                ObjectId(1),
                target,
                guns[1],
                rules(),
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
            let unit = &world.btech.vehicles()[&shooter];
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
}

#[tokio::test]
async fn a_vehicle_array_fires_its_guns_together_at_a_hex() {
    for source in shooters(FRONT) {
        let (_dir, config, mut world, shooter, target, _) =
            engagement(&source, Weapon::MachineGun).await;
        let guns = guns(&world, shooter);
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
        let unit = &world.btech.vehicles()[&shooter];
        for gun in &guns {
            assert!(unit.weapon_recycle().contains_key(gun), "{gun}");
        }
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn a_destroyed_vehicle_array_fires_one_gun_at_a_hex() {
    for source in shooters(FRONT) {
        let (_dir, config, mut world, shooter, target, _) =
            engagement(&source, Weapon::MachineGun).await;
        let guns = guns(&world, shooter);
        destroy_battle_vehicle_critical(&mut world, shooter, ARRAY).unwrap();
        let before = rounds(&world, shooter);
        let report = hex_shot_that_hits(&mut world, shooter, target, guns[0]);
        assert_eq!(report.cluster_roll, None);
        assert_eq!(before - rounds(&world, shooter), 1);
        let unit = &world.btech.vehicles()[&shooter];
        assert!(unit.weapon_recycle().contains_key(&guns[0]));
        assert!(!unit.weapon_recycle().contains_key(&guns[1]));
        world.validate(&config).unwrap();
    }
}
