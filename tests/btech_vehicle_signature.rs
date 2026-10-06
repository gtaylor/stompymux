//! Vehicle scenario signatures and startup-captured perception survive transactional replay.
use crate::support;
use stompymux_rs::*;

/// A running vehicle with a present pilot at the end of a night lane, initially disconnected
/// from a session.
async fn fixture(template: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Test field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "test",
        MapAsset::from_cells(&format!("1 20\n{}", ".0\n".repeat(20))).unwrap(),
    )
    .unwrap();
    set_battle_map_visibility(&mut world, map, Light::MoonlessNight, 30).unwrap();
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

#[tokio::test]
async fn vehicle_perception_captures_completion_and_replays_until_next_startup() {
    let (_dir, config, mut world, id) =
        fixture(include_str!("../game/units/Demolisher.toml")).await;
    assert_eq!(world.btech.vehicles()[&id].scanner_perception(), 18);
    stop_battle_unit(&mut world, id, ObjectId(1), MovementRules::STANDARD.fall).unwrap();
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..4 {
        advance_battle_units(&mut world, 0);
    }
    set_battle_character(
        &mut world,
        ObjectId(1),
        Character {
            bruise: 0,
            lethal: 0,
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
        },
    )
    .unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    let expected = battle_perception_target(&world, ObjectId(1)).unwrap();
    assert_eq!(expected, 8);
    assert_eq!(world.btech.vehicles()[&id].scanner_perception(), 18);
    advance_battle_units(&mut world, 0);
    assert_eq!(world.btech.vehicles()[&id].scanner_perception(), expected);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Perception",
        CharacterValue {
            value: 2,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(battle_perception_target(&world, ObjectId(1)).unwrap(), 6);
    advance_battle_units(&mut world, 0);
    assert_eq!(world.btech.vehicles()[&id].scanner_perception(), expected);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech, restored.btech);
    stop_battle_unit(&mut restored, id, ObjectId(1), MovementRules::STANDARD.fall).unwrap();
    assign_battle_pilot(&mut restored, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut restored, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut restored, id, ObjectId(1), true).unwrap();
    advance_battle_units(&mut restored, 0);
    stop_battle_unit(&mut restored, id, ObjectId(1), MovementRules::STANDARD.fall).unwrap();
    assert_eq!(
        restored.btech.vehicles()[&id].scanner_perception(),
        expected
    );
    assign_battle_pilot(&mut restored, id, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut restored, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut restored, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut restored, 0);
    }
    assert_eq!(restored.btech.vehicles()[&id].scanner_perception(), 6);
}

/// Scenario illumination removes the night sight penalty beyond the sensor band and survives
/// replay and detached Lua state.
#[tokio::test]
async fn vehicle_signature_drives_scenario_lighting_and_detached_lua_state() {
    let (_dir, config, mut world, observer) =
        fixture(include_str!("../game/units/Demolisher.toml")).await;
    let map = world.btech.vehicles()[&observer].position().unwrap().map;
    let target = world.create(&config, "Target".into(), Kind::Thing);
    world.objects.get_mut(&target).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        target,
        VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
            .unwrap(),
    )
    .unwrap();
    // Beyond the fifteen-hex sensor band, only night sight reaches the target.
    place_battle_unit(&mut world, target, map, 0, 17).unwrap();
    assert_eq!(
        world.btech.vehicles()[&target].signature(),
        UnitSignature::default()
    );
    assert!(!battle_unit_illuminated(&world, target));
    let unlit = battle_perceive(&world, observer, target).unwrap().unwrap();
    assert_eq!(
        (unlit.channel, unlit.aim_modifier),
        (DetectionChannel::Sight, 0)
    );
    let signature = UnitSignature {
        team: -17,
        hidden: true,
        illuminated: true,
    };
    set_battle_unit_signature(&mut world, target, signature).unwrap();
    assert!(battle_unit_illuminated(&world, target));
    let lit = battle_perceive(&world, observer, target).unwrap().unwrap();
    assert_eq!(
        (lit.channel, lit.aim_modifier),
        (DetectionChannel::Sight, 0)
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    assert!(battle_unit_illuminated(&restored, target));
    let scripts =
        Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
    let (team, hidden, illuminated, perception): (i32, bool, bool, i16) = scripts.eval_callback(&format!(
        "local s=btech.unit.state({}); local team=s.signature.team; s.signature.team=99; return team,s.signature.hidden,s.signature.illuminated,s.scanner_perception", target.0)).unwrap();
    assert_eq!(
        (team, hidden, illuminated, perception),
        (-17, true, true, 6)
    );
    assert_eq!(
        scripts.world().btech.vehicles()[&target].signature(),
        signature
    );
    let before = world.btech.clone();
    assert!(set_battle_unit_signature(&mut world, ObjectId(1), signature).is_err());
    assert_eq!(world.btech, before);
    world
        .objects
        .get_mut(&target)
        .unwrap()
        .flags
        .insert(Flag::Going);
    assert!(set_battle_unit_signature(&mut world, target, UnitSignature::default()).is_err());
    assert_eq!(world.btech, before);
    assert!(!battle_unit_illuminated(&world, target));
}
