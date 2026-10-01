//! Temporary vehicle critical failures preserve dice, ammunition and powered recovery across restart.
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
        BattleMapAsset::parse("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("test",template).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id)
}

#[tokio::test]
async fn critical_jams_select_unaffected_weapons_and_replay_powered_recovery() {
    let (_dir, config, mut world, id) = fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([31; 32])).unwrap();
    world.btech = serde_json::from_value(encoded).unwrap();
    let checkpoint = world.btech.clone();
    assert!(
        jam_battle_vehicle_weapon(&mut world, id, BattleVehicleSection::Front)
            .unwrap()
            .is_none()
    );
    assert_eq!(world.btech, checkpoint);
    let mut dice = BattleDice::seeded([31; 32]);
    let index = usize::from(dice.die(2).unwrap() - 1);
    let seconds = dice.die(61).unwrap() + 59;
    let ammunition = world.btech.vehicles()[&id].ammunition().to_vec();
    let cycle = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), index, true).unwrap();
    assert!(cycle.launched);
    let after_shot = world.btech.vehicles()[&id].ammunition().to_vec();
    assert_ne!(ammunition, after_shot);
    let jam = jam_battle_vehicle_weapon(&mut world, id, BattleVehicleSection::Turret)
        .unwrap()
        .unwrap();
    assert_eq!(jam.index, index);
    assert_eq!(jam.seconds, seconds);
    assert_eq!(jam.failure, BattleEquipmentFailure::Jammed);
    assert!(jam.notice(id).text.contains("temporarily jams"));
    assert_eq!(world.btech.vehicles()[&id].ammunition(), after_shot);
    let ready = world.btech.vehicles()[&id].weapon_readiness(index).unwrap();
    assert!(ready.intact && ready.jammed && !ready.ready);
    assert_eq!(ready.recycle_remaining, seconds);
    assert!(reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), index, true).is_err());
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let second = jam_battle_vehicle_weapon(&mut world, id, BattleVehicleSection::Turret)
        .unwrap()
        .unwrap();
    assert_eq!(
        second,
        jam_battle_vehicle_weapon(&mut loaded, id, BattleVehicleSection::Turret)
            .unwrap()
            .unwrap()
    );
    assert_ne!(second.index, jam.index);
    dice.die(1).unwrap();
    assert_eq!(second.seconds, dice.die(61).unwrap() + 59);
    assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), vec![dice.d6()]);
    roll_unit_dice(&mut loaded, id, 1).unwrap();
    let checkpoint = world.btech.clone();
    assert!(
        jam_battle_vehicle_weapon(&mut world, id, BattleVehicleSection::Turret)
            .unwrap()
            .is_none()
    );
    assert_eq!(world.btech, checkpoint);
    assert_eq!(world.btech, loaded.btech);
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let checkpoint = world.btech.clone();
    for _ in 0..120 {
        assert!(advance_battle_recycle(&mut world).is_empty());
    }
    assert_eq!(world.btech, checkpoint);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    let mut notices = Vec::new();
    for _ in 0..120 {
        notices.extend(advance_battle_recycle(&mut world));
    }
    assert_eq!(notices.len(), 2);
    assert!(
        notices
            .iter()
            .all(|notice| notice.text.contains("is operational again"))
    );
    assert!(world.btech.vehicles()[&id].weapon_failures().is_empty());
    assert!(
        world.btech.vehicles()[&id]
            .weapon_readiness(index)
            .unwrap()
            .ready
    );
}

#[tokio::test]
async fn critical_shorts_validate_snapshots_and_disappear_with_destroyed_mounts() {
    let text = include_str!("../game/mechs/Demolisher.toml").replace(
        "Front_Side\n",
        "Front_Side\n    CRIT_1 { IS.MediumLaser - - }\n",
    );
    let (_dir, _config, mut world, id) = fixture(&text).await;
    let jam = jam_battle_vehicle_weapon(&mut world, id, BattleVehicleSection::Front)
        .unwrap()
        .unwrap();
    assert_eq!(jam.failure, BattleEquipmentFailure::Shorted);
    assert!(jam.notice(id).text.contains("short out"));
    let original = serde_json::to_value(&world.btech.vehicles()[&id]).unwrap();
    for (field, value) in [
        ("weapon_failures", serde_json::json!({"99":"shorted"})),
        (
            "weapon_recycle",
            serde_json::json!({(jam.index.to_string()):121}),
        ),
    ] {
        let mut bad = original.clone();
        bad[field] = value;
        assert!(serde_json::from_value::<BattleVehicle>(bad).is_err());
    }
    let mount = world.btech.vehicles()[&id].loadout().unwrap().weapons[jam.index].criticals[0];
    assert!(destroy_battle_vehicle_critical(&mut world, id, mount).unwrap());
    assert!(world.btech.vehicles()[&id].weapon_failures().is_empty());
    assert!(world.btech.vehicles()[&id].weapon_recycle().is_empty());
    let checkpoint = world.btech.clone();
    assert!(
        jam_battle_vehicle_weapon(&mut world, id, BattleVehicleSection::Front)
            .unwrap()
            .is_none()
    );
    assert_eq!(world.btech, checkpoint);
    let jam = jam_battle_vehicle_weapon(&mut world, id, BattleVehicleSection::Turret)
        .unwrap()
        .unwrap();
    damage_battle_vehicle_phase(
        &mut world,
        id,
        BattleVehicleSection::Turret,
        100,
        BattleDamagePhase::Internal,
    )
    .unwrap();
    assert!(
        !world.btech.vehicles()[&id]
            .weapon_failures()
            .contains_key(&jam.index)
    );
    assert!(advance_battle_recycle(&mut world).is_empty());
}
