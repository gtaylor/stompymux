//! Main-weapon jams rank all intact mounts and preserve the reference recycle-dependent recovery.
use crate::support;
use stompymux_rs::*;

/// A running vehicle with a present pilot, initially disconnected from a session.
async fn fixture(template: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Test field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        VehicleTemplate::parse("test", template).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

/// Seed only the selected vehicle's dice, keeping all material and equipment state unchanged.
fn seed(world: &mut World, id: ObjectId, value: u8) {
    world
        .btech
        .set_unit_dice(id, Dice::seeded([value; 32]))
        .unwrap();
}

#[tokio::test]
async fn main_jam_ranks_intact_mounts_and_persists_without_a_recycle_timer() {
    let text = include_str!("../game/units/Demolisher.toml").replace(
        "[sections.front_side]\n",
        "[sections.front_side]\nslots = [{ at = 1, item = \"IS.MediumLaser\" }]\n",
    );
    let (_dir, config, mut world, id) = fixture(&text).await;
    seed(&mut world, id, 31);
    let loadout = world.btech.vehicles()[&id].loadout().unwrap();
    let mut dice = Dice::seeded([31; 32]);
    let mut expected = None;
    let mut highest = 0;
    for index in 0..loadout.weapons.len() {
        let rank = dice.rank_i31();
        if rank > highest {
            highest = rank;
            expected = Some(index);
        }
    }
    let ammo = world.btech.vehicles()[&id].ammunition().to_vec();
    let jam = jam_battle_vehicle_main_weapon(&mut world, id)
        .unwrap()
        .unwrap();
    assert_eq!(Some(jam.index), expected);
    assert_eq!(
        world.btech.vehicles()[&id].weapon_failures()[&jam.index],
        EquipmentFailure::Disabled
    );
    assert!(world.btech.vehicles()[&id].weapon_recycle().is_empty());
    assert_eq!(world.btech.vehicles()[&id].ammunition(), ammo);
    assert_eq!(
        roll_unit_dice(&mut world, id, 2).unwrap(),
        vec![dice.d6(), dice.d6()]
    );
    assert!(
        reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), jam.index, true)
            .unwrap_err()
            .to_string()
            .contains("destroyed")
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    for _ in 0..120 {
        assert!(advance_battle_recycle(&mut loaded).is_empty());
    }
    assert_eq!(loaded.btech, world.btech);
    seed(&mut loaded, id, 31);
    assert_eq!(
        jam_battle_vehicle_main_weapon(&mut loaded, id)
            .unwrap()
            .unwrap(),
        jam
    );
    for mount in &loadout.weapons {
        destroy_battle_vehicle_critical(&mut world, id, mount.criticals[0]).unwrap();
    }
    let before = world.btech.clone();
    assert!(
        jam_battle_vehicle_main_weapon(&mut world, id)
            .unwrap()
            .is_none()
    );
    assert_eq!(world.btech, before);
    let mut bad = serde_json::to_value(&world.btech.vehicles()[&id]).unwrap();
    bad["weapon_failures"] = serde_json::json!({"0":"disabled"});
    assert!(serde_json::from_value::<Vehicle>(bad).is_err());
}

#[tokio::test]
async fn main_jam_recovers_on_next_powered_update_only_when_already_recycling() {
    for temporary_failure in [false, true] {
        let (_dir, config, mut world, id) =
            fixture(include_str!("../game/units/Demolisher.toml")).await;
        if temporary_failure {
            assert!(
                jam_battle_vehicle_weapon(&mut world, id, VehicleSection::Turret)
                    .unwrap()
                    .is_some()
            );
            assert!(
                jam_battle_vehicle_weapon(&mut world, id, VehicleSection::Turret)
                    .unwrap()
                    .is_some()
            );
        } else {
            for index in 0..2 {
                let shot = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), index, true)
                    .unwrap();
                assert!(shot.launched);
            }
        }
        let timers = world.btech.vehicles()[&id].weapon_recycle().clone();
        let ammo = world.btech.vehicles()[&id].ammunition().to_vec();
        let jam = jam_battle_vehicle_main_weapon(&mut world, id)
            .unwrap()
            .unwrap();
        assert_eq!(world.btech.vehicles()[&id].weapon_recycle(), &timers);
        assert_eq!(world.btech.vehicles()[&id].ammunition(), ammo);
        stop_battle_unit(&mut world, id, ObjectId(1), MovementRules::STANDARD.fall).unwrap();
        for _ in 0..120 {
            assert!(advance_battle_recycle(&mut world).is_empty());
        }
        assert_eq!(world.btech.vehicles()[&id].weapon_recycle(), &timers);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        assign_battle_pilot(&mut loaded, id, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut loaded, ObjectId(1), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut loaded, id, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut loaded, 0);
        }
        let notices = advance_battle_recycle(&mut loaded);
        assert_eq!(notices.len(), 1);
        assert!(notices[0].text.contains("is operational again"));
        let vehicle = &loaded.btech.vehicles()[&id];
        assert!(!vehicle.weapon_failures().contains_key(&jam.index));
        assert!(vehicle.weapon_readiness(jam.index).unwrap().ready);
        assert_eq!(
            vehicle.weapon_recycle()[&(1 - jam.index)],
            timers[&(1 - jam.index)] - 1
        );
    }
}
