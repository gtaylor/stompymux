//! Vehicle RPG crew injuries reuse character health, consciousness and transactional evacuation.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// A running tactical chassis with an in-character non-wizard pilot and mixed occupants.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    fixture_with_template(include_str!("../game/units/Demolisher.toml")).await
}

/// Supply a weapon layout while retaining the same character and occupant setup.
async fn fixture_with_template(
    template: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    fixture_on_surface(template, ".0").await
}

/// Place the crew on an authored surface to exercise nested terrain consequences.
async fn fixture_on_surface(
    template: &str,
    tile: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Crew field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "crew",
        MapAsset::from_cells(&format!("1 1\n{tile}\n")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
    let id = world.create(&config, "Character vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        VehicleTemplate::parse("Demolisher", template).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
    set_battle_character(
        &mut world,
        ObjectId(2),
        Character {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    assign_battle_pilot(&mut world, id, ObjectId(2)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(2), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    let passenger = world.create(&config, "Passenger".into(), Kind::Thing);
    world.objects.get_mut(&passenger).unwrap().home = Some(ObjectId(config.home()));
    world.objects.get_mut(&passenger).unwrap().location = Some(id);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["recoveries"]["2"]["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
    (dir, config, world, id, passenger)
}

#[tokio::test]
async fn vehicle_character_injury_uses_health_and_recovery_without_tactical_hit_limits() {
    let (_dir, config, mut world, id, _passenger) = fixture().await;
    let initial = world.btech.clone();
    injure_battle_character_pilot(&mut world, id, 0, false).unwrap();
    assert_eq!(initial, world.btech);
    let material = world.btech.vehicles()[&id].clone();
    let report = injure_battle_character_pilot(&mut world, id, 6, false).unwrap();
    assert!(!report.injury.fatal);
    assert!(!report.consciousness.unwrap().conscious);
    let unit = &world.btech.vehicles()[&id];
    assert_eq!(unit.character_pilot_status().unwrap().injuries, 6);
    assert_eq!(unit.pilot_injuries(), 6);
    assert!(!unit.is_destroyed());
    assert_eq!(unit.sections(), material.sections());
    assert_eq!(unit.ammunition(), material.ammunition());
    assert!(world.btech.unconscious(ObjectId(2)));
    let recovery = world.btech.recoveries()[&ObjectId(2)].clone();
    let report = injure_battle_character_pilot(&mut world, id, 1, false).unwrap();
    assert!(report.consciousness.is_none());
    assert_eq!(world.btech.recoveries()[&ObjectId(2)], recovery);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let count: u16 = scripts
        .eval_callback(&format!(
            "return btech.unit.state({}).character_pilot.injuries",
            id.0
        ))
        .unwrap();
    assert_eq!(count, 7);
    assert!(set_battle_speed(&mut scripts.world_mut(), id, ObjectId(2), 1.0).is_err());
}

#[tokio::test]
async fn fatal_vehicle_character_injury_evacuates_atomically_and_preserves_material() {
    let (_dir, config, mut world, id, passenger) = fixture().await;
    injure_battle_character_pilot(&mut world, id, 7, false).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let baseline = scripts.world().clone();
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    scripts.world_mut().objects.remove(&afterlife);
    assert!(injure_battle_character_pilot_action(&scripts, &config, id, 3, false).is_err());
    assert_eq!(scripts.world().btech, baseline.btech);
    assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(id));
    assert_eq!(scripts.world().objects[&passenger].location, Some(id));
    *scripts.world_mut() = baseline.clone();
    let report = injure_battle_character_pilot_action(&scripts, &config, id, 3, false).unwrap();
    assert!(report.injury.fatal);
    let result = scripts.world().clone();
    let unit = &result.btech.vehicles()[&id];
    assert!(unit.character_pilot_status().unwrap().killed);
    assert!(unit.is_destroyed());
    assert_eq!(unit.pilot_injuries(), 7);
    assert!(!unit.flooded());
    assert_eq!(unit.pilot(), None);
    assert_eq!(unit.power(), Power::Off);
    assert!(!unit.motion().unwrap().active());
    assert_eq!(unit.sections(), baseline.btech.vehicles()[&id].sections());
    assert_eq!(
        unit.ammunition(),
        baseline.btech.vehicles()[&id].ammunition()
    );
    assert_eq!(result.objects[&ObjectId(2)].location, Some(afterlife));
    assert_eq!(result.objects[&passenger].location, Some(afterlife));
    assert_eq!(result.objects[&ObjectId(1)].location, Some(id));
    assert!(!result.btech.unconscious(ObjectId(2)));
    result.validate(&config).unwrap();
    persistence::save(&config.database(), &result)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        result.btech
    );
    let mut invalid = serde_json::to_value(unit).unwrap();
    invalid["power"] = serde_json::to_value(Power::Running).unwrap();
    assert!(serde_json::from_value::<Vehicle>(invalid).is_err());
    let before = scripts.world().btech.clone();
    assert!(injure_battle_character_pilot_action(&scripts, &config, id, 1, false).is_err());
    assert_eq!(before, scripts.world().btech);
}

/// Set only the vehicle's random stream; character recovery keeps its independent seed.
fn seed_vehicle(world: &mut World, id: ObjectId, seed: u8) {
    world
        .btech
        .set_unit_dice(id, Dice::seeded([seed; 32]))
        .unwrap();
}

#[tokio::test]
async fn gauss_critical_explodes_without_injuring_the_character_crew() {
    let template = include_str!("../game/units/Demolisher.toml").replace(
        "[sections.front_side]\n",
        "[sections.front_side]\nslots = [{ at = 1, item = \"IS.MagshotGaussRifle\" }]\n",
    );
    let (_dir, config, mut world, id, _) = fixture_with_template(&template).await;
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = Dice::seeded([*seed; 32]);
            if dice.two_d6() != 11 {
                return false;
            }
            dice.die(1).unwrap();
            dice.two_d6();
            dice.two_d6() < 6
        })
        .unwrap();
    seed_vehicle(&mut world, id, seed);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report = resolve_battle_vehicle_critical_action(
        &scripts,
        &config,
        id,
        VehicleSection::Front,
        VehicleCriticalRules {
            extended_piloting: false,
            combat_safe: false,
            toughness: false,
        },
    )
    .unwrap();
    assert_eq!(
        report.selection.effect,
        Some(VehicleCriticalEffect::WeaponDestroyed)
    );
    // Vehicle crews are not injured by internal explosions (TW p. 195).
    assert_eq!(report.internal_damage.len(), 1);
    let result = scripts.world().clone();
    let vehicle = &result.btech.vehicles()[&id];
    assert!(
        vehicle
            .character_pilot_status()
            .is_none_or(|status| status.injuries == 0)
    );
    assert_eq!(vehicle.pilot_injuries(), 0);
    assert_eq!(vehicle.sections()[&VehicleSection::Front].internal, 5);
    assert_eq!(vehicle.lost_criticals().len(), 1);
    persistence::save(&config.database(), &result)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        result.btech
    );
}

#[tokio::test]
async fn instant_crew_death_preserves_health_and_evacuates_with_or_without_a_pilot() {
    let (_dir, config, base, id, passenger) = fixture().await;
    {
        let rules = VehicleCriticalRules {
            extended_piloting: false,
            combat_safe: false,
            toughness: false,
        };
        let seed = (0..=255)
            .find(|seed| {
                let mut world = base.clone();
                seed_vehicle(&mut world, id, *seed);
                roll_battle_vehicle_critical(&mut world, id, VehicleSection::Front, rules)
                    .unwrap()
                    .effect
                    == Some(VehicleCriticalEffect::CrewKilled)
            })
            .unwrap();
        for assigned in [false, true] {
            let mut world = base.clone();
            if !assigned {
                release_battle_pilot(&mut world, id, ObjectId(2)).unwrap();
            }
            seed_vehicle(&mut world, id, seed);
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let before = scripts.world().clone();
            let afterlife = ObjectId(config.battletech.afterlife_dbref);
            scripts.world_mut().objects.remove(&afterlife);
            assert!(
                resolve_battle_vehicle_critical_action(
                    &scripts,
                    &config,
                    id,
                    VehicleSection::Front,
                    rules
                )
                .is_err()
            );
            assert_eq!(scripts.world().btech, before.btech);
            assert_eq!(scripts.world().objects[&passenger].location, Some(id));
            *scripts.world_mut() = before.clone();
            let report = resolve_battle_vehicle_critical_action(
                &scripts,
                &config,
                id,
                VehicleSection::Front,
                rules,
            )
            .unwrap();
            assert_eq!(
                report.selection.effect,
                Some(VehicleCriticalEffect::CrewKilled)
            );
            let result = scripts.world().clone();
            let vehicle = &result.btech.vehicles()[&id];
            assert!(vehicle.crew_killed());
            assert!(vehicle.is_destroyed());
            assert!(!vehicle.flooded());
            assert_eq!(vehicle.pilot_injuries(), 0);
            assert_eq!(vehicle.character_pilot_status(), None);
            assert_eq!(vehicle.pilot(), None);
            assert_eq!(vehicle.power(), Power::Off);
            assert_eq!(vehicle.sections(), before.btech.vehicles()[&id].sections());
            assert_eq!(
                vehicle.ammunition(),
                before.btech.vehicles()[&id].ammunition()
            );
            assert_eq!(result.btech.characters(), before.btech.characters());
            assert_eq!(result.objects[&passenger].location, Some(afterlife));
            assert_eq!(result.objects[&ObjectId(2)].location, Some(afterlife));
            assert_eq!(result.objects[&ObjectId(1)].location, Some(id));
            let lua: bool = scripts
                .eval_callback(&format!("return btech.unit.state({}).crew_killed", id.0))
                .unwrap();
            assert!(lua);
            let mut invalid = serde_json::to_value(vehicle).unwrap();
            invalid["power"] = serde_json::to_value(Power::Running).unwrap();
            assert!(serde_json::from_value::<Vehicle>(invalid).is_err());
            persistence::save(&config.database(), &result)
                .await
                .unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                result.btech
            );
        }
    }
}

#[tokio::test]
async fn nested_crew_death_finishes_weapon_damage_before_single_evacuation() {
    let template = include_str!("../game/units/Demolisher.toml").replace(
        "[sections.front_side]\n",
        "[sections.front_side]\nslots = [{ at = 1, item = \"IS.MagshotGaussRifle\" }]\n",
    );
    let (_dir, config, mut world, id, passenger) = fixture_with_template(&template).await;
    let seed = (0u32..100_000)
        .find_map(|value| {
            let mut seed = [0; 32];
            seed[..4].copy_from_slice(&value.to_le_bytes());
            let mut dice = Dice::seeded(seed);
            if dice.two_d6() != 11 {
                return None;
            }
            dice.die(1).unwrap();
            dice.two_d6();
            if dice.two_d6() != 12 {
                return None;
            }
            Some(seed)
        })
        .unwrap();
    world.btech.set_unit_dice(id, Dice::seeded(seed)).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let rules = VehicleCriticalRules {
        extended_piloting: false,
        combat_safe: false,
        toughness: false,
    };
    let report =
        resolve_battle_vehicle_critical_action(&scripts, &config, id, VehicleSection::Front, rules)
            .unwrap();
    assert_eq!(
        report.selection.effect,
        Some(VehicleCriticalEffect::WeaponDestroyed)
    );
    assert_eq!(
        report.internal_damage[0].criticals[0].selection.effect,
        Some(VehicleCriticalEffect::CrewKilled)
    );
    let result = scripts.world().clone();
    let vehicle = &result.btech.vehicles()[&id];
    assert!(vehicle.crew_killed());
    assert_eq!(vehicle.sections()[&VehicleSection::Front].internal, 5);
    assert_eq!(vehicle.character_pilot_status(), None);
    assert_eq!(
        result.objects[&passenger].location,
        Some(ObjectId(config.battletech.afterlife_dbref))
    );
    let before = result.btech.clone();
    assert!(
        resolve_battle_vehicle_critical_action(&scripts, &config, id, VehicleSection::Front, rules)
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    persistence::save(&config.database(), &result)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        result.btech
    );
}

#[tokio::test]
async fn ground_explosions_evacuate_crew_and_restore_failed_actions() {
    for case in [false, true] {
        let template = if case {
            include_str!("../game/units/Demolisher.toml").replace(
                "[sections.aft_side]\n",
                "[sections.aft_side]\nslots = [{ at = 1, item = \"CASE\" }]\n",
            )
        } else {
            include_str!("../game/units/Demolisher.toml").to_owned()
        };
        let (_dir, config, mut world, id, passenger) = fixture_with_template(&template).await;
        // A side 12 on an internal-combustion vehicle is Fuel Tank: the vehicle explodes.
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        seed_vehicle(&mut world, id, seed);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let rules = VehicleCriticalRules {
            extended_piloting: false,
            combat_safe: false,
            toughness: false,
        };
        let before = scripts.world().clone();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        scripts.world_mut().objects.remove(&afterlife);
        assert!(
            resolve_battle_vehicle_critical_action(
                &scripts,
                &config,
                id,
                VehicleSection::Left,
                rules
            )
            .is_err()
        );
        assert_eq!(scripts.world().btech, before.btech);
        assert_eq!(scripts.world().objects[&passenger].location, Some(id));
        *scripts.world_mut() = before.clone();
        let report = resolve_battle_vehicle_critical_action(
            &scripts,
            &config,
            id,
            VehicleSection::Left,
            rules,
        )
        .unwrap();
        assert_eq!(
            report.selection.effect,
            Some(VehicleCriticalEffect::FuelTank)
        );
        assert!(!report.explosion.unwrap().contained);
        let result = scripts.world().clone();
        let vehicle = &result.btech.vehicles()[&id];
        assert!(vehicle.is_destroyed());
        assert!(vehicle.crew_killed());
        assert_eq!(vehicle.pilot_injuries(), 0);
        assert_eq!(result.btech.characters(), before.btech.characters());
        assert_eq!(result.objects[&passenger].location, Some(afterlife));
        assert_eq!(result.objects[&ObjectId(2)].location, Some(afterlife));
        assert_eq!(result.objects[&ObjectId(1)].location, Some(id));
        assert!(vehicle.sections().values().all(|state| state.internal == 0));
        persistence::save(&config.database(), &result)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            result.btech
        );
    }
}

#[tokio::test]
async fn ordinary_hull_loss_leaves_occupants_alive_and_armor_criticals_publish_casualties() {
    let (_dir, config, base, id, passenger) = fixture().await;
    for crew_critical in [false, true] {
        let mut world = base.clone();
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = Dice::seeded([*seed; 32]);
                dice.two_d6();
                dice.two_d6() == 12
            })
            .unwrap();
        seed_vehicle(&mut world, id, seed);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let hit = VehicleArmorHit {
            damage_class: DamageClass::Ordinary,
            section: VehicleSection::Front,
            amount: if crew_critical { 1 } else { 48 },
            through_armor_critical: crew_critical,
            armor_piercing: None,
        };
        let report = resolve_battle_vehicle_armor_damage_action(
            &scripts,
            &config,
            id,
            hit,
            VehicleCriticalRules {
                extended_piloting: false,
                combat_safe: false,
                toughness: false,
            },
        )
        .unwrap();
        assert!(report.unit_destroyed);
        let result = scripts.world().clone();
        assert_eq!(result.btech.vehicles()[&id].crew_killed(), crew_critical);
        assert_eq!(result.btech.characters(), base.btech.characters());
        assert_eq!(
            result.objects[&passenger].location,
            Some(if crew_critical {
                ObjectId(config.battletech.afterlife_dbref)
            } else {
                id
            })
        );
        assert_eq!(
            result.btech.vehicles()[&id].sections()[&VehicleSection::Front].internal,
            if crew_critical { 8 } else { 0 }
        );
        persistence::save(&config.database(), &result)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            result.btech
        );
    }
}

#[tokio::test]
async fn character_vehicle_falls_injure_no_crew() {
    // Total Warfare p. 152: a vehicle fall damages the vehicle; its crew makes no avoidance
    // roll and takes no injury, however badly hurt it already is.
    for initial in [0, 9] {
        let (_dir, config, mut world, id, passenger) = fixture().await;
        if initial > 0 {
            injure_battle_character_pilot(&mut world, id, initial, false).unwrap();
        }
        let rules = MovementRules::STANDARD.fall;
        let before = world.btech.clone();
        assert!(resolve_battle_vehicle_fall(&mut world, id, 0, rules).is_err());
        assert_eq!(world.btech, before);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let report = resolve_battle_vehicle_fall_action(&scripts, &config, id, 0, rules).unwrap();
        assert!(report.avoidance.is_none());
        assert!(report.pilot_injury.is_none());
        assert!(report.character_injury.is_none());
        assert!(report.experience_messages.is_empty());
        assert_eq!(report.damage, 0);
        assert!(report.groups.is_empty());
        let result = scripts.world().clone();
        assert_eq!(result.btech.vehicles()[&id].pilot_injuries(), initial);
        assert_eq!(result.objects[&ObjectId(2)].location, Some(id));
        assert_eq!(result.objects[&passenger].location, Some(id));
        *scripts.world_mut() = world;
        assert_eq!(
            resolve_battle_vehicle_fall_action(&scripts, &config, id, 0, rules).unwrap(),
            report
        );
        assert_eq!(scripts.world().btech, result.btech);
        persistence::save(&config.database(), &result)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            result.btech
        );
    }
}

#[tokio::test]
async fn character_surface_fractures_drop_vehicles_without_crew_injury() {
    for terrain in [Terrain::Ice, Terrain::Bridge] {
        for initial in [0, 9] {
            let tile = format!("{}1", terrain.symbol());
            let (_dir, config, mut world, id, passenger) =
                fixture_on_surface(include_str!("../game/units/Demolisher.toml"), &tile).await;
            if initial > 0 {
                injure_battle_character_pilot(&mut world, id, initial, false).unwrap();
            }
            let seed = (0..=255)
                .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
                .unwrap();
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    record["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
                    record["definition"]["attributes"]["specials"] =
                        "ICEEngine_Tech CritProof_Tech".into();
                })
                .unwrap();
            let map = world.btech.vehicles()[&id].position().unwrap().map;
            let coordinate = HexCoordinate { x: 0, y: 0 };
            let rules = MovementRules::STANDARD.fall;
            let before = world.btech.clone();
            let raw = if terrain == Terrain::Ice {
                break_battle_ice(&mut world, map, coordinate, None, rules)
            } else {
                break_battle_bridge(&mut world, map, coordinate, rules)
            };
            assert!(raw.is_err());
            assert_eq!(world.btech, before);
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let report = break_battle_surface_action(
                &scripts,
                &config,
                map,
                coordinate,
                Surface::of(Hex::new(terrain, 1)).unwrap(),
                rules,
            )
            .unwrap();
            // The fall through the surface damages the vehicle only (Total Warfare p. 152).
            assert_eq!(report.vehicle_falls.len(), 1);
            let fall = &report.vehicle_falls[0].1;
            assert!(fall.avoidance.is_none());
            assert!(fall.character_injury.is_none());
            let result = scripts.world().clone();
            assert_eq!(result.btech.vehicles()[&id].pilot_injuries(), initial);
            assert_eq!(result.objects[&ObjectId(2)].location, Some(id));
            assert_eq!(result.objects[&passenger].location, Some(id));
            assert!(result.btech.vehicles()[&id].flooded());
            *scripts.world_mut() = world;
            assert_eq!(
                break_battle_surface_action(
                    &scripts,
                    &config,
                    map,
                    coordinate,
                    Surface::of(Hex::new(terrain, 1)).unwrap(),
                    rules
                )
                .unwrap(),
                report
            );
            assert_eq!(scripts.world().btech, result.btech);
            persistence::save(&config.database(), &result)
                .await
                .unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                result.btech
            );
        }
    }
}

#[tokio::test]
async fn transport_loss_shares_nested_chassis_destruction_and_transactional_disembarkation() {
    for enabled in [false, true] {
        for vehicle_first in [false, true] {
            for wreck_first in [false, true] {
                let (dir, _config, mut world, carrier, passenger) = fixture().await;
                let path = dir.path().join("stompymux.toml");
                let source = std::fs::read_to_string(&path).unwrap();
                let source = source.replace(
                    "[battletech]",
                    &format!(
                        "[battletech]\ntransported_unit_death = {}",
                        i32::from(enabled)
                    ),
                );
                std::fs::write(&path, source).unwrap();
                let config = Config::load(dir.path()).unwrap();
                assert_eq!(config.battletech.transported_unit_death != 0, enabled);
                let map = world.btech.vehicles()[&carrier].position().unwrap().map;
                let mut parent = carrier;
                let mut cargo = Vec::new();
                for vehicle in [vehicle_first, !vehicle_first] {
                    let id = world.create(&config, "Carried unit".into(), Kind::Thing);
                    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
                    if vehicle {
                        create_battle_vehicle(
                            &mut world,
                            id,
                            VehicleTemplate::parse(
                                "Demolisher",
                                include_str!("../game/units/Demolisher.toml"),
                            )
                            .unwrap(),
                        )
                        .unwrap();
                        support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
                    } else {
                        create_battle_unit(
                            &mut world,
                            id,
                            MechTemplate::parse(
                                "JR7-D",
                                include_str!("fixtures/btech/units/JR7-D.toml"),
                            )
                            .unwrap(),
                        )
                        .unwrap();
                        support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
                    }
                    world
                        .objects
                        .get_mut(&id)
                        .unwrap()
                        .flags
                        .insert(Flag::InCharacter);
                    remove_battle_unit(&mut world, id, parent).unwrap();
                    cargo.push(id);
                    parent = id;
                }
                if wreck_first {
                    world
                        .btech
                        .rewrite_unit_record(cargo[0], |record| {
                            record["transport_destroyed"] = true.into();
                        })
                        .unwrap();
                }
                world.validate(&config).unwrap();
                let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                let destroy = |scripts: &Scripts| {
                    resolve_battle_vehicle_armor_damage_action(
                        scripts,
                        &config,
                        carrier,
                        VehicleArmorHit {
                            damage_class: DamageClass::Ordinary,
                            section: VehicleSection::Front,
                            amount: 48,
                            through_armor_critical: false,
                            armor_piercing: None,
                        },
                        VehicleCriticalRules {
                            extended_piloting: false,
                            combat_safe: false,
                            toughness: false,
                        },
                    )
                };
                if enabled {
                    scripts.world_mut().objects.remove(&map);
                    assert!(destroy(&scripts).is_err());
                    assert_eq!(scripts.world().btech, world.btech);
                    for id in &cargo {
                        assert_eq!(
                            scripts.world().objects[id].location,
                            world.objects[id].location
                        );
                    }
                    *scripts.world_mut() = world.clone();
                }
                assert!(destroy(&scripts).unwrap().unit_destroyed);
                let result = scripts.world().clone();
                for (index, id) in cargo.iter().enumerate() {
                    let disembarked = enabled && (index == 0 || !wreck_first);
                    let destroyed = disembarked || (index == 0 && wreck_first);
                    if let Some(unit) = result.btech.constructed_units().get(id) {
                        assert_eq!(unit.is_destroyed(), destroyed);
                        assert_eq!(
                            unit.sections(),
                            world.btech.constructed_units()[id].sections()
                        );
                    } else {
                        let unit = &result.btech.vehicles()[id];
                        assert_eq!(unit.is_destroyed(), destroyed);
                        assert!(!unit.crew_killed());
                        assert!(!unit.flooded());
                        assert_eq!(unit.sections(), world.btech.vehicles()[id].sections());
                    }
                    assert_eq!(
                        result.objects[id].location,
                        if disembarked {
                            Some(map)
                        } else {
                            world.objects[id].location
                        }
                    );
                }
                assert_eq!(result.objects[&ObjectId(2)].location, Some(carrier));
                assert_eq!(result.objects[&passenger].location, Some(carrier));
                assert_eq!(result.btech.characters(), world.btech.characters());
                *scripts.world_mut() = world;
                assert!(destroy(&scripts).unwrap().unit_destroyed);
                assert_eq!(scripts.world().btech, result.btech);
                persistence::save(&config.database(), &result)
                    .await
                    .unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    result.btech
                );
            }
        }
    }
}
