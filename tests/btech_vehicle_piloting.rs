//! Vehicle control skill selection, blocked crew, and saved roll replay through the shared API.
use crate::support;
use stompymux_rs::*;

/// A running vehicle with a present pilot, initially disconnected from a session.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId) {
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
        BattleVehicleTemplate::parse("Demolisher",include_str!("../game/mechs/Demolisher.toml")).unwrap(),
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
async fn vehicle_control_uses_configured_skills_cockpit_and_saved_dice() {
    let (_dir, config, mut world, id) = fixture().await;
    assert_eq!(battle_unit_piloting_target(&world, id, true).unwrap(), 6);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assert_eq!(battle_unit_piloting_target(&world, id, true).unwrap(), 18);
    set_battle_character(
        &mut world,
        ObjectId(1),
        BattleCharacter {
            build: 5,
            reflexes: 4,
            intuition: 3,
            learn: 2,
            charisma: 1,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    for (name, value) in [
        ("Drive", 5),
        ("Piloting-Tracked", 4),
        ("Piloting-Wheeled", 3),
        ("Piloting-Hover", 2),
    ] {
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            name,
            BattleCharacterValue {
                value,
                experience: 16_777_216,
                last_used: 0,
            },
        )
        .unwrap();
    }
    let original = serde_json::to_value(&world.btech).unwrap();
    for (movement, expected) in [
        (BattleVehicleMovement::Tracked, 6),
        (BattleVehicleMovement::Wheeled, 7),
        (BattleVehicleMovement::Hover, 8),
        (BattleVehicleMovement::Stationary, 6),
    ] {
        let mut state = original.clone();
        // Keep the synthetic hovercraft inside the engine catalogue after suspension allowance.
        if movement == BattleVehicleMovement::Hover {
            state["vehicles"][id.0.to_string()]["definition"]["max_speed"] = 64.5.into();
        }
        state["vehicles"][id.0.to_string()]["definition"]["movement"] =
            serde_json::to_value(movement).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        assert_eq!(
            battle_unit_piloting_target(&world, id, true).unwrap(),
            expected
        );
        assert_eq!(battle_unit_piloting_target(&world, id, false).unwrap(), 5);
    }
    let mut state = original;
    state["vehicles"][id.0.to_string()]["definition"]["attributes"]["specials"] = "SMCPIT".into();
    state["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([12; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let check = roll_battle_piloting(&mut world, id, -1, true).unwrap();
    assert_eq!(
        (check.skill, check.cockpit, check.damage, check.target),
        (6, 1, 0, 6)
    );
    assert_eq!(check.roll, Some(BattleDice::seeded([12; 32]).two_d6()));
    assert_eq!(check.success, check.roll.unwrap() >= 6);
    assert_eq!(
        check,
        roll_battle_piloting(&mut loaded, id, -1, true).unwrap()
    );
    assert_eq!(loaded.btech, world.btech);
    assert!(check.experience.is_none());
    assert!(
        roll_battle_piloting(&mut world, id, i16::MIN, true)
            .unwrap()
            .success
    );
    assert!(
        !roll_battle_piloting(&mut world, id, i16::MAX, true)
            .unwrap()
            .success
    );
}

#[tokio::test]
async fn stopped_unconscious_and_unavailable_vehicle_checks_do_not_consume_dice() {
    let (_dir, _config, world, id) = fixture().await;
    let mut stopped = world.clone();
    stop_battle_unit(
        &mut stopped,
        id,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let before = stopped.btech.clone();
    let check = roll_battle_piloting(&mut stopped, id, -100, true).unwrap();
    assert!(!check.success && check.roll.is_none());
    assert_eq!(stopped.btech, before);
    let mut world = world;
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["recoveries"]["1"] = serde_json::json!({
        "mode":{"kind":"tactical","injuries":4},"remaining":30,
        "pain_resistance":false,"toughness":false,"dice":BattleDice::seeded([0;32])
    });
    world.btech = serde_json::from_value(state).unwrap();
    let before = world.btech.clone();
    let check = roll_battle_piloting(&mut world, id, -100, true).unwrap();
    assert!(!check.success && check.roll.is_none());
    assert_eq!(world.btech, before);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert!(roll_battle_piloting(&mut world, id, 0, true).is_err());
    assert!(roll_battle_piloting(&mut world, ObjectId(-1), 0, true).is_err());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn absent_character_vehicle_operator_adds_five_without_preventing_roll() {
    let (_dir, _config, mut world, id) = fixture().await;
    release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let check = roll_battle_piloting(&mut world, id, 0, true).unwrap();
    assert_eq!(
        (check.skill, check.absent_character_pilot, check.target),
        (6, 5, 11)
    );
    assert!(check.roll.is_some());
}
