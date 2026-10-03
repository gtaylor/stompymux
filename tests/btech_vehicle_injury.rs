//! Tactical vehicle injuries share pilot recovery and commit scenario loss without damaging construction.
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
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("test", template).unwrap(),
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
async fn vehicle_injury_recovery_and_scenario_death_replay_without_material_damage() {
    let (_dir, config, mut world, id) =
        fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let value = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() < 11)
        .unwrap();
    state["recoveries"]["1"]["dice"] =
        serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let definition = world.btech.vehicles()[&id].definition().clone();
    let sections = world.btech.vehicles()[&id].sections().clone();
    let injury = injure_battle_tactical_pilot(&mut world, id, 5, false).unwrap();
    assert_eq!(injury.injuries, 5);
    assert!(!injury.killed);
    assert!(world.btech.unconscious(ObjectId(1)));
    assert!(set_battle_speed(&mut world, id, ObjectId(1), 10.0).is_err());
    assert!(reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).is_err());
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    for _ in 0..90 {
        assert_eq!(
            advance_battle_recovery(&mut world),
            advance_battle_recovery(&mut loaded)
        );
    }
    assert_eq!(world.btech, loaded.btech);
    let injury = injure_battle_tactical_pilot(&mut world, id, 1, false).unwrap();
    assert!(injury.killed && injury.consciousness.is_none());
    let vehicle = &world.btech.vehicles()[&id];
    assert!(vehicle.is_destroyed());
    assert_eq!(vehicle.power(), BattlePower::Off);
    assert!(vehicle.pilot().is_none());
    assert_eq!(vehicle.sections(), &sections);
    assert_eq!(vehicle.definition(), &definition);
    assert_eq!(world.objects[&ObjectId(1)].location, Some(id));
    assert!(!world.btech.unconscious(ObjectId(1)));
    assert!(assign_battle_pilot(&mut world, id, ObjectId(1)).is_err());
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let mut bad = serde_json::to_value(&world.btech.vehicles()[&id]).unwrap();
    bad["pilot_injuries"] = 128.into();
    assert!(serde_json::from_value::<BattleVehicle>(bad).is_err());
}

#[tokio::test]
async fn vehicle_injury_guards_and_critical_casualties_are_atomic() {
    let (_dir, _config, base, id) = fixture(include_str!("../game/mechs/Demolisher.toml")).await;
    let rules = BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        table: BattleVehicleCriticalTable::Standard,
        enabled: true,
        combat_safe: false,
        toughness: true,
    };
    let seed = (0..=255)
        .find(|value| {
            let mut dice = BattleDice::seeded([*value; 32]);
            dice.die(10).unwrap() > 5 && dice.d6() == 1
        })
        .unwrap();
    for absent in [false, true] {
        let mut world = base.clone();
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
            })
            .unwrap();
        if absent {
            release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        } else {
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
        }
        let before = world.btech.clone();
        let result =
            resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Front, rules);
        if absent {
            let report = result.unwrap();
            assert_eq!(report.pilot_injury.unwrap().injuries, 1);
            assert_eq!(world.btech.vehicles()[&id].pilot_injuries(), 1);
            assert_eq!(world.btech.recoveries(), before.recoveries());
        } else {
            assert!(result.is_err());
            assert_eq!(world.btech, before);
        }
    }
    let mut world = base;
    damage_battle_vehicle_controls(&mut world, id, BattleVehicleControlHit::CrewStun).unwrap();
    jam_battle_vehicle_turret(&mut world, id).unwrap();
    begin_battle_turret_repair(&mut world, id, ObjectId(1)).unwrap();
    let injury = injure_battle_tactical_pilot(&mut world, id, 6, true).unwrap();
    assert!(injury.killed);
    assert_eq!(world.btech.vehicles()[&id].crew_stun_remaining(), 0);
    assert!(world.btech.vehicles()[&id].turret_repairs().is_empty());
}
