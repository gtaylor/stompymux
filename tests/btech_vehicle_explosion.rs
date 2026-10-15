//! Fuel-tank explosions destroy internal-combustion vehicles regardless of CASE and persist.
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
        crate::support::templates::fitted(VehicleTemplate::parse("Demolisher", template).unwrap()),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
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

fn rules() -> VehicleCriticalRules {
    VehicleCriticalRules {
        extended_piloting: false,
        combat_safe: false,
        toughness: false,
    }
}

/// A seed whose first 2d6 is twelve: Fuel Tank in the side and rear columns.
fn twelve() -> u8 {
    (0..=255)
        .find(|value| Dice::seeded([*value; 32]).two_d6() == 12)
        .unwrap()
}

#[tokio::test]
async fn fuel_tank_explosions_ignore_case_and_persist() {
    for case in [false, true] {
        let text = if case {
            include_str!("../game/units/Demolisher.toml").replace(
                "[sections.aft_side]\n",
                "[sections.aft_side]\nslots = [{ at = 1, item = \"CASE\" }]\n",
            )
        } else {
            include_str!("../game/units/Demolisher.toml").into()
        };
        let (_dir, config, mut world, id) = fixture(&text).await;
        assert_eq!(
            world.btech.vehicles()[&id].has_powerplant_containment(),
            case
        );
        seed(&mut world, id, twelve());
        let result =
            resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Rear, rules()).unwrap();
        assert_eq!(
            result.selection.effect,
            Some(VehicleCriticalEffect::FuelTank)
        );
        let explosion = result.explosion.unwrap();
        assert!(!explosion.contained);
        assert_eq!(explosion.destroyed_sections.len(), 5);
        let vehicle = &world.btech.vehicles()[&id];
        assert!(vehicle.is_destroyed());
        assert_eq!(vehicle.power(), Power::Off);
        assert!(vehicle.pilot().is_none());
        for state in vehicle.sections().values() {
            assert_eq!(state.internal, 0);
            assert_eq!(state.armor, 0);
        }
        assert_eq!(world.objects[&ObjectId(1)].location, Some(id));
        let mut dice = Dice::seeded([twelve(); 32]);
        dice.two_d6();
        assert_eq!(roll_unit_dice(&mut world, id, 1).unwrap(), vec![dice.d6()]);
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

#[tokio::test]
async fn fuel_explosions_do_not_damage_nearby_units() {
    let (_dir, config, mut world, id) =
        fixture(include_str!("../game/units/Demolisher.toml")).await;
    let map = world.btech.vehicles()[&id].position().unwrap().map;
    let neighbor = world.create(&config, "Neighbor".into(), Kind::Thing);
    world.objects.get_mut(&neighbor).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        neighbor,
        crate::support::templates::fitted(
            VehicleTemplate::parse("Demolisher", include_str!("../game/units/Demolisher.toml"))
                .unwrap(),
        ),
    )
    .unwrap();
    support::seed_object_dice(&mut world, neighbor, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, neighbor, map, 0, 0).unwrap();
    let before = world.btech.vehicles()[&neighbor].clone();
    seed(&mut world, id, twelve());
    let result =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Left, rules()).unwrap();
    assert!(result.explosion.is_some());
    assert!(world.btech.vehicles()[&id].is_destroyed());
    assert_eq!(world.btech.vehicles()[&neighbor], before);
}

#[tokio::test]
async fn case_requires_installed_equipment_and_character_explosions_record_crew_loss() {
    for extra in ["", "Clan"] {
        let text =
            support::templates::with_flags(include_str!("../game/units/Demolisher.toml"), &[extra])
                .replace(
                    "[sections.aft_side]\n",
                    "[sections.aft_side]\nconfig = \"CASE\"\n",
                );
        let (_dir, _config, world, id) = fixture(&text).await;
        assert!(!world.btech.vehicles()[&id].has_powerplant_containment());
    }
    let (_dir, _config, mut world, id) =
        fixture(include_str!("../game/units/Demolisher.toml")).await;
    seed(&mut world, id, twelve());
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let report =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Left, rules()).unwrap();
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
async fn fusion_vehicles_take_an_engine_hit_instead_of_exploding() {
    let fusion = support::templates::without_flags(
        include_str!("../game/units/Demolisher.toml"),
        &["ICEEngine_Tech"],
    );
    let (_dir, _config, mut world, id) = fixture(&fusion).await;
    seed(&mut world, id, twelve());
    let report =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Left, rules()).unwrap();
    assert_eq!(report.selection.effect, Some(VehicleCriticalEffect::Engine));
    assert!(report.explosion.is_none());
    let vehicle = &world.btech.vehicles()[&id];
    assert!(!vehicle.is_destroyed());
    assert!(vehicle.engine_destroyed() && vehicle.immobilized());
}

#[tokio::test]
async fn carried_battlesuits_require_casualty_handling_before_explosion_commit() {
    let (_dir, config, mut world, id) =
        fixture(include_str!("../game/units/Demolisher.toml")).await;
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
    seed(&mut world, id, twelve());
    let before = world.btech.clone();
    let error =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Left, rules()).unwrap_err();
    assert!(error.to_string().contains("battlesuit"));
    assert_eq!(world.btech, before);
    assert_eq!(world.objects[&child].location, Some(id));
}
