//! Vehicle RPG crew injuries reuse character health, consciousness and transactional evacuation.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// A running tactical chassis with an in-character non-wizard pilot and mixed occupants.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    fixture_with_template(include_str!("../game/mechs/Demolisher.toml")).await
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
        BattleMapAsset::from_cells(&format!("1 1\n{tile}\n")).unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Character vehicle".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("Demolisher", template).unwrap(),
    )
    .unwrap();
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
        BattleCharacter {
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
        advance_battle_units(&mut world, 0);
    }
    let passenger = world.create(&config, "Passenger".into(), Kind::Thing);
    world.objects.get_mut(&passenger).unwrap().home = Some(ObjectId(config.home()));
    world.objects.get_mut(&passenger).unwrap().location = Some(id);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["recoveries"]["2"]["dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
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
    assert_eq!(unit.power(), BattlePower::Off);
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
    invalid["power"] = serde_json::to_value(BattlePower::Running).unwrap();
    assert!(serde_json::from_value::<BattleVehicle>(invalid).is_err());
    let before = scripts.world().btech.clone();
    assert!(injure_battle_character_pilot_action(&scripts, &config, id, 1, false).is_err());
    assert_eq!(before, scripts.world().btech);
}

/// Set only the vehicle's random stream; character recovery keeps its independent seed.
fn seed_vehicle(world: &mut World, id: ObjectId, seed: u8) {
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
}

#[tokio::test]
async fn crew_critical_uses_shared_health_and_evacuates_fatal_injury_atomically() {
    let (_dir, config, base, id, passenger) = fixture().await;
    let rules = BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        table: BattleVehicleCriticalTable::Standard,
        enabled: true,
        combat_safe: false,
        toughness: false,
    };
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            dice.die(10).unwrap() > 5 && dice.d6() == 1
        })
        .unwrap();
    for previous_hits in [0, 9] {
        let mut world = base.clone();
        if previous_hits > 0 {
            injure_battle_character_pilot(&mut world, id, previous_hits, false).unwrap();
        }
        seed_vehicle(&mut world, id, seed);
        let mut expected = world.clone();
        let selection =
            roll_battle_vehicle_critical(&mut expected, id, BattleVehicleSection::Front, rules)
                .unwrap();
        let injury = injure_battle_character_pilot(&mut expected, id, 1, false).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let baseline = scripts.world().clone();
        if previous_hits > 0 {
            scripts
                .world_mut()
                .objects
                .remove(&ObjectId(config.battletech.afterlife_dbref));
            let before = scripts.world().clone();
            assert!(
                resolve_battle_vehicle_critical_action(
                    &scripts,
                    &config,
                    id,
                    BattleVehicleSection::Front,
                    rules
                )
                .is_err()
            );
            assert_eq!(scripts.world().btech, before.btech);
            assert_eq!(scripts.world().objects[&passenger].location, Some(id));
            *scripts.world_mut() = baseline;
        }
        let report = resolve_battle_vehicle_critical_action(
            &scripts,
            &config,
            id,
            BattleVehicleSection::Front,
            rules,
        )
        .unwrap();
        assert_eq!(report.selection, selection);
        assert_eq!(report.character_injury, Some(injury));
        assert!(report.pilot_injury.is_none());
        assert_eq!(scripts.world().btech, expected.btech);
        assert_eq!(
            scripts.world().btech.vehicles()[&id].pilot_injuries(),
            if previous_hits == 9 { 9 } else { 1 }
        );
        assert_eq!(
            scripts.world().objects[&passenger].location,
            Some(if previous_hits > 0 {
                ObjectId(config.battletech.afterlife_dbref)
            } else {
                id
            })
        );
        let result = scripts.world().clone();
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
async fn gauss_critical_injures_character_after_surviving_internal_damage() {
    let template = include_str!("../game/mechs/Demolisher.toml").replace(
        "[sections.front_side]\n",
        "[sections.front_side]\nslots = [{ at = 1, item = \"IS.MagshotGaussRifle\" }]\n",
    );
    let (_dir, config, mut world, id, _) = fixture_with_template(&template).await;
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            if dice.two_d6() != 11 {
                return false;
            }
            dice.die(1).unwrap();
            dice.two_d6();
            dice.two_d6() < 8
        })
        .unwrap();
    seed_vehicle(&mut world, id, seed);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report = resolve_battle_vehicle_critical_action(
        &scripts,
        &config,
        id,
        BattleVehicleSection::Front,
        BattleVehicleCriticalRules {
            rotor_damage_divisor: 0,
            extended_piloting: false,
            vtol_table: None,
            table: BattleVehicleCriticalTable::Advanced,
            enabled: true,
            combat_safe: false,
            toughness: false,
        },
    )
    .unwrap();
    assert_eq!(
        report.selection.effect,
        Some(BattleVehicleCriticalEffect::WeaponDestroyed)
    );
    assert!(report.pilot_injury.is_none());
    assert!(!report.character_injury.unwrap().injury.fatal);
    let result = scripts.world().clone();
    let vehicle = &result.btech.vehicles()[&id];
    assert_eq!(vehicle.character_pilot_status().unwrap().injuries, 2);
    assert_eq!(vehicle.pilot_injuries(), 2);
    assert_eq!(vehicle.sections()[&BattleVehicleSection::Front].internal, 5);
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
    for table in [
        BattleVehicleCriticalTable::Standard,
        BattleVehicleCriticalTable::Advanced,
    ] {
        let rules = BattleVehicleCriticalRules {
            rotor_damage_divisor: 0,
            extended_piloting: false,
            vtol_table: None,
            table,
            enabled: true,
            combat_safe: false,
            toughness: false,
        };
        let seed = (0..=255)
            .find(|seed| {
                let mut world = base.clone();
                seed_vehicle(&mut world, id, *seed);
                roll_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Front, rules)
                    .unwrap()
                    .effect
                    == Some(BattleVehicleCriticalEffect::CrewKilled)
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
                    BattleVehicleSection::Front,
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
                BattleVehicleSection::Front,
                rules,
            )
            .unwrap();
            assert_eq!(
                report.selection.effect,
                Some(BattleVehicleCriticalEffect::CrewKilled)
            );
            assert!(report.character_injury.is_none());
            assert!(report.pilot_injury.is_none());
            let result = scripts.world().clone();
            let vehicle = &result.btech.vehicles()[&id];
            assert!(vehicle.crew_killed());
            assert!(vehicle.is_destroyed());
            assert!(!vehicle.flooded());
            assert_eq!(vehicle.pilot_injuries(), 0);
            assert_eq!(vehicle.character_pilot_status(), None);
            assert_eq!(vehicle.pilot(), None);
            assert_eq!(vehicle.power(), BattlePower::Off);
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
            invalid["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            assert!(serde_json::from_value::<BattleVehicle>(invalid).is_err());
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
    let template = include_str!("../game/mechs/Demolisher.toml").replace(
        "[sections.front_side]\n",
        "[sections.front_side]\nslots = [{ at = 1, item = \"IS.MagshotGaussRifle\" }]\n",
    );
    let (_dir, config, mut world, id, passenger) = fixture_with_template(&template).await;
    let seed = (0u32..100_000)
        .find_map(|value| {
            let mut seed = [0; 32];
            seed[..4].copy_from_slice(&value.to_le_bytes());
            let mut dice = BattleDice::seeded(seed);
            if dice.two_d6() != 11 {
                return None;
            }
            dice.die(1).unwrap();
            dice.two_d6();
            if !matches!(dice.two_d6(), 8 | 9) || dice.two_d6() != 12 {
                return None;
            }
            Some(seed)
        })
        .unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded(seed)).unwrap();
    world.btech = serde_json::from_value(saved).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let rules = BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        table: BattleVehicleCriticalTable::Advanced,
        enabled: true,
        combat_safe: false,
        toughness: false,
    };
    let report = resolve_battle_vehicle_critical_action(
        &scripts,
        &config,
        id,
        BattleVehicleSection::Front,
        rules,
    )
    .unwrap();
    assert_eq!(
        report.selection.effect,
        Some(BattleVehicleCriticalEffect::WeaponDestroyed)
    );
    assert_eq!(
        report.internal_damage[0].criticals[0].selection.effect,
        Some(BattleVehicleCriticalEffect::CrewKilled)
    );
    assert!(report.character_injury.is_none());
    let result = scripts.world().clone();
    let vehicle = &result.btech.vehicles()[&id];
    assert!(vehicle.crew_killed());
    assert_eq!(vehicle.sections()[&BattleVehicleSection::Front].internal, 5);
    assert_eq!(vehicle.character_pilot_status(), None);
    assert_eq!(
        result.objects[&passenger].location,
        Some(ObjectId(config.battletech.afterlife_dbref))
    );
    let before = result.btech.clone();
    assert!(
        resolve_battle_vehicle_critical_action(
            &scripts,
            &config,
            id,
            BattleVehicleSection::Front,
            rules
        )
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
    for (case, fuel) in [(false, false), (true, false), (true, true)] {
        let template = if case {
            include_str!("../game/mechs/Demolisher.toml").replace(
                "[sections.aft_side]\n",
                "[sections.aft_side]\nslots = [{ at = 1, item = \"CASE\" }]\n",
            )
        } else {
            include_str!("../game/mechs/Demolisher.toml").to_owned()
        };
        let (_dir, config, mut world, id, passenger) = fixture_with_template(&template).await;
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = BattleDice::seeded([*seed; 32]);
                dice.die(10).unwrap() > 5 && dice.d6() == if fuel { 5 } else { 6 }
            })
            .unwrap();
        seed_vehicle(&mut world, id, seed);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let rules = BattleVehicleCriticalRules {
            rotor_damage_divisor: 0,
            extended_piloting: false,
            vtol_table: None,
            table: BattleVehicleCriticalTable::Standard,
            enabled: true,
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
                BattleVehicleSection::Front,
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
            BattleVehicleSection::Front,
            rules,
        )
        .unwrap();
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
                let mut dice = BattleDice::seeded([*seed; 32]);
                dice.two_d6();
                matches!(dice.two_d6(), 8 | 9) && dice.two_d6() == 12
            })
            .unwrap();
        seed_vehicle(&mut world, id, seed);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let hit = BattleVehicleArmorHit {
            damage_class: BattleDamageClass::Ordinary,
            section: BattleVehicleSection::Front,
            amount: if crew_critical { 1 } else { 48 },
            through_armor_critical: crew_critical,
            armor_piercing: None,
        };
        let report = resolve_battle_vehicle_armor_damage_action(
            &scripts,
            &config,
            id,
            hit,
            BattleVehicleCriticalRules {
                rotor_damage_divisor: 0,
                extended_piloting: false,
                vtol_table: None,
                table: BattleVehicleCriticalTable::Advanced,
                enabled: crew_critical,
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
            result.btech.vehicles()[&id].sections()[&BattleVehicleSection::Front].internal,
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
async fn fire_exposure_evacuates_crew_and_rolls_back_failed_publication() {
    let (_dir, config, mut world, id, passenger) = fixture().await;
    let seed = (0u32..100_000)
        .find_map(|value| {
            let mut seed = [0; 32];
            seed[..4].copy_from_slice(&value.to_le_bytes());
            let mut dice = BattleDice::seeded(seed);
            if dice.two_d6() != 10 {
                return None;
            }
            dice.d6();
            dice.two_d6();
            if !matches!(dice.two_d6(), 8 | 9) || dice.die(10).unwrap() <= 5 || dice.d6() != 4 {
                return None;
            }
            Some(seed)
        })
        .unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded(seed)).unwrap();
    for section in saved["vehicles"][id.0.to_string()]["sections"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        section["armor"] = 0.into();
    }
    world.btech = serde_json::from_value(saved).unwrap();
    let rules = BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        table: BattleVehicleCriticalTable::Standard,
        enabled: true,
        combat_safe: false,
        toughness: false,
    };
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let before = scripts.world().clone();
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    scripts.world_mut().objects.remove(&afterlife);
    assert!(resolve_battle_vehicle_fire_exposure_action(&scripts, &config, id, rules).is_err());
    assert_eq!(scripts.world().btech, before.btech);
    assert_eq!(scripts.world().objects[&passenger].location, Some(id));
    *scripts.world_mut() = before;
    let report = resolve_battle_vehicle_fire_exposure_action(&scripts, &config, id, rules).unwrap();
    assert!(!report.effects.damage.is_empty());
    assert!(scripts.world().btech.vehicles()[&id].crew_killed());
    assert_eq!(
        scripts.world().objects[&passenger].location,
        Some(afterlife)
    );
}

#[tokio::test]
async fn scheduled_fires_publish_character_injuries_and_fatal_evacuation_with_replay() {
    let (_dir, config, base, id, passenger) = fixture().await;
    for fatal in [false, true] {
        let mut initial = base.clone();
        if fatal {
            injure_battle_character_pilot(&mut initial, id, 9, false).unwrap();
        }
        let mut saved = serde_json::to_value(&initial.btech).unwrap();
        saved["vehicles"][id.0.to_string()]["burning_sections"] = serde_json::json!({"front":1});
        saved["vehicles"][id.0.to_string()]["sections"]["front"]["armor"] = 0.into();
        initial.btech = serde_json::from_value(saved).unwrap();
        let (before, expected, expected_report) = (0..=255)
            .find_map(|seed| {
                let mut world = initial.clone();
                seed_vehicle(&mut world, id, seed);
                let before = world.clone();
                let report = advance_battle_vehicle_fires(&mut world, &config).ok()?;
                (report.character_injuries.len() == 1
                    && report.character_injuries[0].injury.fatal == fatal
                    && world.btech.vehicles()[&id].is_destroyed() == fatal)
                    .then_some((before, world, report))
            })
            .unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(before.clone()))).unwrap();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        if fatal {
            scripts.world_mut().objects.remove(&afterlife);
            assert!(advance_battle_vehicle_fires_action(&scripts, &config).is_err());
            assert_eq!(scripts.world().btech, before.btech);
            assert_eq!(scripts.world().objects[&passenger].location, Some(id));
            *scripts.world_mut() = before;
        }
        let report = advance_battle_vehicle_fires_action(&scripts, &config).unwrap();
        assert_eq!(report, expected_report);
        let mut expected_state = serde_json::to_value(&expected.btech).unwrap();
        if expected.btech.vehicles()[&id]
            .sections()
            .values()
            .all(|section| section.internal == 0)
        {
            // The host action also registers a fully destroyed IC wreck for retirement.
            expected_state["wrecks"][id.0.to_string()] = 10.into();
        }
        assert_eq!(
            serde_json::to_value(&scripts.world().btech).unwrap(),
            expected_state
        );
        assert_eq!(
            scripts.world().objects[&passenger].location,
            Some(if fatal { afterlife } else { id })
        );
        let result = scripts.world().clone();
        persistence::save(&config.database(), &result)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, result.btech);
        let mut original = result;
        assert_eq!(
            advance_battle_vehicle_fires(&mut original, &config).unwrap(),
            advance_battle_vehicle_fires(&mut restored, &config).unwrap()
        );
        assert_eq!(original.btech, restored.btech);
    }
}

#[tokio::test]
async fn inferno_and_heat_actions_share_explosions_and_fatal_rollback() {
    let (_dir, config, base, id, passenger) = fixture().await;
    for inferno in [false, true] {
        for roll in [8, 9] {
            let mut world = base.clone();
            let seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
                .unwrap();
            seed_vehicle(&mut world, id, seed);
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let apply = || -> anyhow::Result<bool> {
                if inferno {
                    return Ok(resolve_battle_vehicle_inferno_hit_action(
                        &scripts,
                        &config,
                        id,
                        3,
                        BattleVehicleImpactRules::STANDARD,
                    )?
                    .explosion
                    .is_some());
                }
                Ok(resolve_battle_vehicle_heat_exposure_action(
                    &scripts,
                    &config,
                    id,
                    0,
                    BattleVehicleImpactRules::STANDARD,
                )?
                .explosion
                .is_some())
            };
            let before = scripts.world().clone();
            let afterlife = ObjectId(config.battletech.afterlife_dbref);
            if roll == 9 {
                scripts.world_mut().objects.remove(&afterlife);
                assert!(apply().is_err());
                assert_eq!(scripts.world().btech, before.btech);
                assert_eq!(scripts.world().objects[&passenger].location, Some(id));
                *scripts.world_mut() = before.clone();
            }
            assert_eq!(apply().unwrap(), roll == 9);
            let result = scripts.world().clone();
            assert_eq!(result.btech.vehicles()[&id].crew_killed(), roll == 9);
            assert_eq!(result.btech.characters(), before.btech.characters());
            assert_eq!(
                result.objects[&passenger].location,
                Some(if roll == 9 { afterlife } else { id })
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
}

#[tokio::test]
async fn character_mine_heat_evacuates_after_packets_and_rolls_back_the_field() {
    let (_dir, config, mut world, id, passenger) = fixture().await;
    let map = world.btech.vehicles()[&id].position().unwrap().map;
    set_minefield(
        &mut world,
        map,
        0,
        Some(BattleMinefield {
            coordinate: BattleHexCoordinate { x: 0, y: 0 },
            kind: BattleMineKind::Standard,
            strength: 5,
            extra: 0,
            owner: ObjectId(1),
        }),
    )
    .unwrap();
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["vehicles"][id.0.to_string()]["motion"]["heading"] = 180.0.into();
    encoded["vehicles"][id.0.to_string()]["motion"]["desired_heading"] = 180.0.into();
    world.btech = serde_json::from_value(encoded).unwrap();
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            dice.two_d6();
            dice.two_d6();
            dice.two_d6() == 9
        })
        .unwrap();
    seed_vehicle(&mut world, id, seed);
    let mut rules = BattleMovementRules::STANDARD.fall;
    rules.vehicle_impact.criticals.enabled = false;
    rules.vehicle_impact.hit.critical_mode = 0;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let before = scripts.world().clone();
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    scripts.world_mut().objects.remove(&afterlife);
    assert!(resolve_mine_blast_action(&scripts, &config, map, 0, rules).is_err());
    assert_eq!(scripts.world().btech, before.btech);
    *scripts.world_mut() = before;
    let report = resolve_mine_blast_action(&scripts, &config, map, 0, rules).unwrap();
    assert_eq!(report.hits[0].impacts.len(), 1);
    assert_eq!(
        report.hits[0].vehicle_heat.as_ref().unwrap().explosion_roll,
        Some(9)
    );
    assert!(scripts.world().btech.vehicles()[&id].crew_killed());
    assert_eq!(
        scripts.world().objects[&passenger].location,
        Some(afterlife)
    );
    let result = scripts.world().clone();
    persistence::save(&config.database(), &result)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        result.btech
    );
}

#[tokio::test]
async fn advanced_thermal_actions_publish_nested_character_feedback_once() {
    let (_dir, config, mut base, id, _) = fixture().await;
    let mut saved = serde_json::to_value(&base.btech).unwrap();
    for section in saved["vehicles"][id.0.to_string()]["sections"]
        .as_object_mut()
        .unwrap()
        .values_mut()
    {
        section["armor"] = 0.into();
    }
    base.btech = serde_json::from_value(saved).unwrap();
    let mut rules = BattleVehicleImpactRules::STANDARD;
    rules.advanced_fire = true;
    rules.criticals.table = BattleVehicleCriticalTable::Standard;
    for inferno in [false, true] {
        let (before, expected) = (0..=255)
            .find_map(|seed| {
                let mut world = base.clone();
                seed_vehicle(&mut world, id, seed);
                let before = world.clone();
                if inferno {
                    let _ = resolve_battle_vehicle_inferno_hit(&mut world, id, 1, rules).ok()?;
                } else {
                    let _ = resolve_battle_vehicle_heat_exposure(&mut world, id, 1, rules).ok()?;
                }
                world.btech.vehicles()[&id]
                    .character_pilot_status()
                    .filter(|status| status.injuries > 0 && !status.killed)?;
                Some((before, world))
            })
            .unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(before))).unwrap();
        if inferno {
            let _ =
                resolve_battle_vehicle_inferno_hit_action(&scripts, &config, id, 1, rules).unwrap();
        } else {
            let _ = resolve_battle_vehicle_heat_exposure_action(&scripts, &config, id, 1, rules)
                .unwrap();
        }
        let actual = scripts.world().clone();
        let mut expected_state = serde_json::to_value(&expected.btech).unwrap();
        if expected.btech.vehicles()[&id]
            .sections()
            .values()
            .all(|section| section.internal == 0)
        {
            // The host action additionally admits the fully destroyed IC wreck for retirement.
            expected_state["wrecks"][id.0.to_string()] = 10.into();
            assert_eq!(
                actual.objects[&ObjectId(2)].location,
                Some(ObjectId(config.battletech.afterlife_dbref))
            );
        }
        assert_eq!(serde_json::to_value(&actual.btech).unwrap(), expected_state);
        let checks = scripts
            .drain_outbox()
            .iter()
            .filter(|(recipient, text)| {
                *recipient == ObjectId(2) && text.to_string().contains("Retain Conciousness")
            })
            .count();
        assert_eq!(checks, 1);
        persistence::save(&config.database(), &actual)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            actual.btech
        );
    }
}

#[tokio::test]
async fn character_vehicle_falls_share_personal_injury_and_atomic_evacuation() {
    for initial in [0, 9] {
        let (_dir, config, mut world, id, passenger) = fixture().await;
        if initial > 0 {
            injure_battle_character_pilot(&mut world, id, initial, false).unwrap();
        }
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
            .unwrap();
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved["vehicles"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        world.btech = serde_json::from_value(saved).unwrap();
        let rules = BattleMovementRules::STANDARD.fall;
        let before = world.btech.clone();
        assert!(resolve_battle_vehicle_fall(&mut world, id, 0, rules).is_err());
        assert_eq!(world.btech, before);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        if initial == 9 {
            scripts.world_mut().objects.remove(&afterlife);
            assert!(resolve_battle_vehicle_fall_action(&scripts, &config, id, 0, rules).is_err());
            assert_eq!(scripts.world().btech, before);
            assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(id));
            *scripts.world_mut() = world.clone();
        }
        let report = resolve_battle_vehicle_fall_action(&scripts, &config, id, 0, rules).unwrap();
        assert!(report.pilot_injury.is_none());
        assert_eq!(
            report.character_injury.as_ref().unwrap().injury.fatal,
            initial == 9
        );
        assert_eq!(report.damage, 0);
        assert!(report.impacts.is_empty());
        let result = scripts.world().clone();
        assert_eq!(
            result.btech.vehicles()[&id].pilot_injuries(),
            if initial == 9 { 9 } else { 1 }
        );
        assert_eq!(
            result.btech.vehicles()[&id]
                .character_pilot_status()
                .unwrap()
                .injuries,
            if initial == 9 { 9 } else { 1 }
        );
        let destination = if initial == 9 { afterlife } else { id };
        assert_eq!(result.objects[&ObjectId(2)].location, Some(destination));
        assert_eq!(result.objects[&passenger].location, Some(destination));
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
async fn character_vehicle_fall_protection_reuses_control_experience() {
    let (_dir, config, mut world, id, _) = fixture().await;
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    saved["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    saved["vehicles"][id.0.to_string()]["definition"]["attributes"]["specials"] =
        "ICEEngine_Tech CritProof_Tech".into();
    world.btech = serde_json::from_value(saved).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report = resolve_battle_vehicle_fall_action(
        &scripts,
        &config,
        id,
        1,
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert!(report.avoidance.as_ref().unwrap().success);
    assert!(
        report
            .avoidance
            .as_ref()
            .unwrap()
            .experience
            .unwrap()
            .accepted
    );
    assert_eq!(report.experience_messages.len(), 1);
    assert!(report.character_injury.is_none());
    assert!(report.damage > 0);
    assert!(!report.impacts.is_empty());
}

#[tokio::test]
async fn character_surface_fractures_publish_vehicle_injuries_atomically() {
    for terrain in [Terrain::Ice, Terrain::Bridge] {
        for initial in [0, 9] {
            let tile = format!("{}1", terrain.symbol());
            let (_dir, config, mut world, id, passenger) =
                fixture_on_surface(include_str!("../game/mechs/Demolisher.toml"), &tile).await;
            if initial > 0 {
                injure_battle_character_pilot(&mut world, id, initial, false).unwrap();
            }
            let seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
                .unwrap();
            let mut saved = serde_json::to_value(&world.btech).unwrap();
            saved["vehicles"][id.0.to_string()]["dice"] =
                serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
            saved["vehicles"][id.0.to_string()]["definition"]["attributes"]["specials"] =
                "ICEEngine_Tech CritProof_Tech".into();
            world.btech = serde_json::from_value(saved).unwrap();
            let map = world.btech.vehicles()[&id].position().unwrap().map;
            let coordinate = BattleHexCoordinate { x: 0, y: 0 };
            let rules = BattleMovementRules::STANDARD.fall;
            let before = world.btech.clone();
            let raw = if terrain == Terrain::Ice {
                break_battle_ice(&mut world, map, coordinate, None, rules)
            } else {
                break_battle_bridge(&mut world, map, coordinate, rules)
            };
            assert!(raw.is_err());
            assert_eq!(world.btech, before);
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let afterlife = ObjectId(config.battletech.afterlife_dbref);
            if initial == 9 {
                scripts.world_mut().objects.remove(&afterlife);
                assert!(
                    break_battle_surface_action(&scripts, &config, map, coordinate, terrain, rules)
                        .is_err()
                );
                assert_eq!(scripts.world().btech, before);
                assert_eq!(scripts.world().objects[&ObjectId(2)].location, Some(id));
                *scripts.world_mut() = world.clone();
            }
            let report =
                break_battle_surface_action(&scripts, &config, map, coordinate, terrain, rules)
                    .unwrap();
            assert_eq!(report.vehicle_falls.len(), 1);
            assert_eq!(
                report.vehicle_falls[0]
                    .1
                    .character_injury
                    .as_ref()
                    .unwrap()
                    .injury
                    .fatal,
                initial == 9
            );
            let result = scripts.world().clone();
            assert_eq!(
                result.btech.vehicles()[&id].pilot_injuries(),
                if initial == 9 { 9 } else { 1 }
            );
            let destination = if initial == 9 { afterlife } else { id };
            assert_eq!(result.objects[&ObjectId(2)].location, Some(destination));
            assert_eq!(result.objects[&passenger].location, Some(destination));
            assert_eq!(result.btech.vehicles()[&id].flooded(), initial == 0);
            *scripts.world_mut() = world;
            assert_eq!(
                break_battle_surface_action(&scripts, &config, map, coordinate, terrain, rules)
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
                            BattleVehicleTemplate::parse(
                                "Demolisher",
                                include_str!("../game/mechs/Demolisher.toml"),
                            )
                            .unwrap(),
                        )
                        .unwrap();
                    } else {
                        create_battle_unit(
                            &mut world,
                            id,
                            BattleTemplate::parse(
                                "JR7-D",
                                include_str!("fixtures/btech/mechs/JR7-D.toml"),
                            )
                            .unwrap(),
                        )
                        .unwrap();
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
                    let mut saved = serde_json::to_value(&world.btech).unwrap();
                    let collection = if vehicle_first {
                        "vehicles"
                    } else {
                        "constructed"
                    };
                    saved[collection][cargo[0].0.to_string()]["transport_destroyed"] = true.into();
                    world.btech = serde_json::from_value(saved).unwrap();
                }
                world.validate(&config).unwrap();
                let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                let destroy = |scripts: &Scripts| {
                    resolve_battle_vehicle_armor_damage_action(
                        scripts,
                        &config,
                        carrier,
                        BattleVehicleArmorHit {
                            damage_class: BattleDamageClass::Ordinary,
                            section: BattleVehicleSection::Front,
                            amount: 48,
                            through_armor_critical: false,
                            armor_piercing: None,
                        },
                        BattleVehicleCriticalRules {
                            rotor_damage_divisor: 0,
                            extended_piloting: false,
                            vtol_table: None,
                            table: BattleVehicleCriticalTable::Standard,
                            enabled: false,
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
