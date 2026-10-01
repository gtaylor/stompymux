//! Tactical vehicle catastrophes preserve containment policy and avoid unrelated battlefield damage.
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

/// Seed only the selected vehicle's dice, keeping all material and equipment state unchanged.
fn seed(world: &mut World, id: ObjectId, value: u8) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
}

/// Select a deterministic critical stream without consuming the live victim's dice.
fn rules(table: BattleVehicleCriticalTable) -> BattleVehicleCriticalRules {
    BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        table,
        enabled: true,
        combat_safe: false,
        toughness: false,
    }
}

#[tokio::test]
async fn ground_powerplant_explosions_ignore_case_and_persist() {
    use BattleVehicleCriticalTable as T;
    for case in [false, true] {
        let text = if case {
            include_str!("../game/mechs/Demolisher.toml")
                .replace(
                    "[sections.aft_side]\n",
                    "[sections.aft_side]\nslots = [{ at = 1, item = \"CASE\" }]\n",
                )
        } else {
            include_str!("../game/mechs/Demolisher.toml").into()
        };
        let (_dir, config, mut world, id) = fixture(&text).await;
        let value = (0..=255)
            .find(|value| {
                let mut dice = BattleDice::seeded([*value; 32]);
                if dice.die(3).unwrap() == 2 {
                    return false;
                }
                dice.d6() == 6
            })
            .unwrap();
        seed(&mut world, id, value);
        let mut dice = BattleDice::seeded([value; 32]);
        dice.die(3).unwrap();
        dice.d6();
        let result = resolve_battle_vehicle_critical(
            &mut world,
            id,
            BattleVehicleSection::Turret,
            rules(T::Standard),
        )
        .unwrap();
        let explosion = result.explosion.unwrap();
        assert!(!explosion.contained);
        assert_eq!(explosion.destroyed_sections.len(), 5);
        let vehicle = &world.btech.vehicles()[&id];
        assert!(vehicle.is_destroyed());
        assert_eq!(vehicle.power(), BattlePower::Off);
        assert!(vehicle.pilot().is_none());
        for state in vehicle.sections().values() {
            assert_eq!(state.internal, 0);
            assert_eq!(state.armor, 0);
        }
        assert_eq!(world.objects[&ObjectId(1)].location, Some(id));
        assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), vec![dice.d6()]);
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

#[tokio::test]
async fn fuel_explosions_ignore_case_and_do_not_damage_nearby_units() {
    let text = include_str!("../game/mechs/Demolisher.toml")
        .replace(
                    "[sections.aft_side]\n",
                    "[sections.aft_side]\nslots = [{ at = 1, item = \"CASE\" }]\n",
                );
    let (_dir, config, mut world, id) = fixture(&text).await;
    let map = world.btech.vehicles()[&id].position().unwrap().map;
    let neighbor = world.create(&config, "Neighbor".into(), Kind::Thing);
    world.objects.get_mut(&neighbor).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        neighbor,
        BattleVehicleTemplate::parse("Demolisher",include_str!("../game/mechs/Demolisher.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, neighbor, map, 0, 0).unwrap();
    let before = world.btech.vehicles()[&neighbor].clone();
    let value = (0..=255)
        .find(|value| {
            let mut dice = BattleDice::seeded([*value; 32]);
            dice.die(10).unwrap() > 5 && dice.d6() == 5
        })
        .unwrap();
    seed(&mut world, id, value);
    let result = resolve_battle_vehicle_critical(
        &mut world,
        id,
        BattleVehicleSection::Front,
        rules(BattleVehicleCriticalTable::Standard),
    )
    .unwrap();
    assert!(!result.explosion.unwrap().contained);
    assert!(
        world.btech.vehicles()[&id]
            .ammunition()
            .iter()
            .all(|rounds| *rounds == 0)
    );
    assert_eq!(world.btech.vehicles()[&neighbor], before);
}

#[tokio::test]
async fn case_requires_installed_equipment_and_character_explosions_record_crew_loss() {
    for extra in ["", "Clan"] {
        let text = include_str!("../game/mechs/Demolisher.toml")
            .replace(
                "\"ICEEngine_Tech\"",
                &if extra.is_empty() {
                    "\"ICEEngine_Tech\"".to_owned()
                } else {
                    format!("\"ICEEngine_Tech\", \"{extra}\"")
                },
            )
            .replace(
                "[sections.aft_side]\n",
                "[sections.aft_side]\nconfig = \"CASE\"\n",
            );
        let (_dir, _config, world, id) = fixture(&text).await;
        assert!(!world.btech.vehicles()[&id].has_powerplant_containment());
    }
    let (_dir, _config, mut world, id) = fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    let value = (0..=255)
        .find(|value| {
            let mut dice = BattleDice::seeded([*value; 32]);
            dice.die(10).unwrap() > 5 && dice.d6() == 6
        })
        .unwrap();
    seed(&mut world, id, value);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let report = resolve_battle_vehicle_critical(
        &mut world,
        id,
        BattleVehicleSection::Front,
        rules(BattleVehicleCriticalTable::Standard),
    )
    .unwrap();
    assert!(!report.explosion.unwrap().contained);
    assert!(world.btech.vehicles()[&id].crew_killed());
    assert!(
        world.btech.vehicles()[&id]
            .sections()
            .values()
            .all(|state| state.internal == 0)
    );
}

#[tokio::test]
async fn carried_battlesuits_require_casualty_handling_before_explosion_commit() {
    let (_dir, config, mut world, id) = fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    let child = world.create(&config, "Carried squad".into(), Kind::Thing);
    let object = world.objects.get_mut(&child).unwrap();
    object.home = Some(ObjectId(config.home()));
    object.location = Some(id);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["units"][child.0.to_string()] = serde_json::to_value(StoredBattleUnit {
        name: "Squad".into(),
        template: "squad".into(),
        class_code: 8,
        movement_code: 0,
        tons: 1,
        map: None,
    })
    .unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let value = (0..=255)
        .find(|value| {
            let mut dice = BattleDice::seeded([*value; 32]);
            dice.die(10).unwrap() > 5 && dice.d6() == 5
        })
        .unwrap();
    seed(&mut world, id, value);
    let before = world.btech.clone();
    let error = resolve_battle_vehicle_critical(
        &mut world,
        id,
        BattleVehicleSection::Front,
        rules(BattleVehicleCriticalTable::Standard),
    )
    .unwrap_err();
    assert!(error.to_string().contains("battlesuit"));
    assert_eq!(world.btech, before);
    assert_eq!(world.objects[&child].location, Some(id));
}
