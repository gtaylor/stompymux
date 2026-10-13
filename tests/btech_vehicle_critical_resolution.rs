//! Critical resolution commits the Total Warfare effects and rolls together, with explicit
//! unsupported outcomes.
use crate::support;
use stompymux_rs::*;

/// A running vehicle built from a Demolisher variant with a present pilot, initially disconnected from a session.
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
        VehicleTemplate::parse("Demolisher", template).unwrap(),
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
        toughness: false,
        combat_safe: false,
    }
}

/// A seed whose first 2d6 is `roll`.
fn rolling(roll: u8) -> u8 {
    (0..=255)
        .find(|value| Dice::seeded([*value; 32]).two_d6() == roll)
        .unwrap()
}

#[tokio::test]
async fn critical_resolution_applies_every_ground_effect() {
    use VehicleCriticalEffect as E;
    use VehicleSection as S;
    let fusion = support::templates::without_flags(
        include_str!("../game/units/Demolisher.toml"),
        &["ICEEngine_Tech"],
    );
    assert!(!fusion.contains("engine = \"ice\""));
    let (_dir, config, base, id) = fixture(&fusion).await;
    let mut seen = std::collections::BTreeSet::new();
    let mut saved = None;
    for section in [S::Front, S::Left, S::Right, S::Rear, S::Turret] {
        for value in 0..128 {
            let mut world = base.clone();
            seed(&mut world, id, value);
            let mut selected = world.clone();
            let expected =
                roll_battle_vehicle_critical(&mut selected, id, section, rules()).unwrap();
            let result = resolve_battle_vehicle_critical(&mut world, id, section, rules()).unwrap();
            assert_eq!(result.selection, expected);
            let Some(effect) = expected.effect else {
                assert!(result.notices.is_empty());
                assert_eq!(world.btech, selected.btech);
                continue;
            };
            seen.insert(format!("{effect:?}"));
            assert!(result.notices[0].text.contains("CRITICAL HIT"));
            let vehicle = &world.btech.vehicles()[&id];
            assert_eq!(result.unit_destroyed, vehicle.is_destroyed());
            match effect {
                E::Rotor(_) | E::VtolPilot | E::VtolCopilot | E::Cargo | E::FuelTank => {
                    panic!("Fusion ground vehicle selected {effect:?}")
                }
                E::CrewKilled => {
                    assert!(vehicle.is_destroyed());
                    assert!(vehicle.crew_killed());
                    assert_eq!(vehicle.power(), Power::Off);
                }
                E::WeaponDestroyed => {
                    assert_eq!(section, S::Turret);
                    assert_eq!(vehicle.lost_criticals().len(), 1);
                }
                E::TurretBlownOff => {
                    assert_eq!(vehicle.sections()[&S::Turret].internal, 0);
                    assert!(
                        vehicle.is_destroyed(),
                        "Turret Blown Off destroys the vehicle"
                    );
                }
                E::Ammunition => {
                    assert!(vehicle.ammunition().iter().all(|rounds| *rounds == 0));
                    assert_eq!(result.ammunition_cascade.as_ref().unwrap().damage, 400);
                    assert_eq!(result.internal_damage.len(), 1);
                    assert!(vehicle.is_destroyed());
                }
                E::Driver => {
                    assert_eq!(vehicle.piloting_damage(), 2);
                    assert!(vehicle.driver_hit());
                    assert_eq!(vehicle.crew_stun_remaining(), 0);
                }
                E::Sensors => {
                    assert_eq!(vehicle.gunnery_damage(), 1);
                    assert_eq!(vehicle.sensor_hits(), 1);
                }
                E::Commander => {
                    assert_eq!(vehicle.piloting_damage(), 1);
                    assert_eq!(vehicle.gunnery_damage(), 1);
                    assert!(vehicle.commander_hit());
                    assert_eq!(vehicle.crew_stun_remaining(), 30);
                }
                E::CrewStunned => assert_eq!(vehicle.crew_stun_remaining(), 30),
                E::Stabilizer => assert!(vehicle.lost_stabilizers().contains(&section)),
                E::TurretJam => assert!(vehicle.turret_jammed()),
                E::TurretLock => assert!(vehicle.turret_locked()),
                E::Engine => {
                    assert_eq!(vehicle.maximum_speed(), 0.0);
                    assert!(vehicle.immobilized());
                    assert!(vehicle.engine_destroyed());
                    assert!(vehicle.turret_locked(), "an Engine Hit locks the turret");
                    assert_eq!(
                        vehicle.definition(),
                        base.btech.vehicles()[&id].definition()
                    );
                    saved = Some(world.clone());
                }
                E::WeaponJam => {
                    assert_eq!(section, S::Turret);
                    assert_eq!(vehicle.weapon_failures().len(), 1);
                    assert!(
                        vehicle
                            .weapon_recycle()
                            .values()
                            .all(|seconds| (60..=120).contains(seconds))
                    );
                }
            }
        }
    }
    assert_eq!(seen.len(), 13, "{seen:?}");
    let world = saved.unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech, loaded.btech);
}

#[tokio::test]
async fn repeated_crew_hits_escalate_to_stun_and_then_death() {
    use VehicleSection as S;
    let (_dir, _config, base, id) = fixture(include_str!("../game/units/Demolisher.toml")).await;
    // Front 6 is Driver Hit, front 10 is Commander Hit; a side 8 is Crew Stunned.
    let mut world = base.clone();
    seed(&mut world, id, rolling(6));
    let _ = resolve_battle_vehicle_critical(&mut world, id, S::Front, rules()).unwrap();
    assert_eq!(world.btech.vehicles()[&id].piloting_damage(), 2);
    seed(&mut world, id, rolling(6));
    let repeat = resolve_battle_vehicle_critical(&mut world, id, S::Front, rules()).unwrap();
    assert_eq!(world.btech.vehicles()[&id].piloting_damage(), 2);
    assert_eq!(world.btech.vehicles()[&id].crew_stun_remaining(), 30);
    assert!(
        repeat
            .notices
            .iter()
            .any(|notice| notice.text.contains("stunning"))
    );
    seed(&mut world, id, rolling(10));
    let _ = resolve_battle_vehicle_critical(&mut world, id, S::Front, rules()).unwrap();
    let vehicle = &world.btech.vehicles()[&id];
    assert_eq!(vehicle.piloting_damage(), 3);
    assert_eq!(vehicle.gunnery_damage(), 1);
    assert!(vehicle.driver_hit() && vehicle.commander_hit());
    assert!(!vehicle.is_destroyed());
    // Driver Hit + Commander Hit + Crew Stunned kills the crew.
    seed(&mut world, id, rolling(8));
    let fatal = resolve_battle_vehicle_critical(&mut world, id, S::Left, rules()).unwrap();
    assert!(fatal.unit_destroyed);
    assert!(world.btech.vehicles()[&id].crew_killed());
    // A second Commander Hit is only a stun.
    let mut world = base.clone();
    seed(&mut world, id, rolling(10));
    let _ = resolve_battle_vehicle_critical(&mut world, id, S::Front, rules()).unwrap();
    advance_battle_units(&mut world, 0, true);
    seed(&mut world, id, rolling(10));
    let _ = resolve_battle_vehicle_critical(&mut world, id, S::Front, rules()).unwrap();
    let vehicle = &world.btech.vehicles()[&id];
    assert_eq!(vehicle.piloting_damage(), 1);
    assert_eq!(vehicle.gunnery_damage(), 1);
    assert_eq!(vehicle.crew_stun_remaining(), 30);
}

#[tokio::test]
async fn four_sensor_hits_stop_the_vehicle_firing() {
    let (_dir, _config, mut world, id) =
        fixture(include_str!("../game/units/Demolisher.toml")).await;
    for hits in 1..=4u8 {
        seed(&mut world, id, rolling(9));
        let _ = resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Front, rules())
            .unwrap();
        let vehicle = &world.btech.vehicles()[&id];
        assert_eq!(vehicle.sensor_hits(), hits);
        assert_eq!(vehicle.gunnery_damage(), hits);
        assert_eq!(vehicle.sensors_blind(), hits == 4);
    }
    let error = reserve_battle_vehicle_weapon(&mut world, id, ObjectId(1), 0, true).unwrap_err();
    assert!(error.to_string().contains("sensors"), "{error}");
    // A fifth sensor hit no longer applies and moves down to Commander Hit.
    seed(&mut world, id, rolling(9));
    let report =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Front, rules()).unwrap();
    assert_eq!(
        report.selection.effect,
        Some(VehicleCriticalEffect::Commander)
    );
}

#[tokio::test]
async fn case_vents_ammunition_explosions_out_the_rear_and_stuns_the_crew() {
    let cased = include_str!("../game/units/Demolisher.toml").replace(
        "[sections.aft_side]\n",
        "[sections.aft_side]\nslots = [{ at = 1, item = \"CASE\" }]\n",
    );
    let (_dir, _config, mut world, id) = fixture(&cased).await;
    assert!(world.btech.vehicles()[&id].has_powerplant_containment());
    seed(&mut world, id, rolling(11));
    let report =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Turret, rules()).unwrap();
    assert_eq!(
        report.selection.effect,
        Some(VehicleCriticalEffect::Ammunition)
    );
    assert_eq!(report.ammunition_cascade.as_ref().unwrap().damage, 400);
    assert!(report.internal_damage.is_empty());
    let vehicle = &world.btech.vehicles()[&id];
    assert!(!vehicle.is_destroyed());
    assert_eq!(vehicle.sections()[&VehicleSection::Rear].armor, 0);
    assert_eq!(vehicle.sections()[&VehicleSection::Rear].internal, 8);
    assert_eq!(vehicle.sections()[&VehicleSection::Turret].internal, 8);
    assert!(vehicle.ammunition().iter().all(|rounds| *rounds == 0));
    assert_eq!(vehicle.crew_stun_remaining(), 30);
}

#[tokio::test]
async fn critical_resolution_suppression_and_replay_preserve_world_boundaries() {
    let (_dir, config, mut world, id) =
        fixture(include_str!("../game/units/Demolisher.toml")).await;
    let mut rules = rules();
    rules.combat_safe = true;
    let before = world.btech.clone();
    let result =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Turret, rules).unwrap();
    assert!(result.selection.effect.is_none() && result.notices.is_empty());
    assert_eq!(world.btech, before);
    rules.combat_safe = false;
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    for _ in 0..30 {
        let result = resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Turret, rules);
        let replay =
            resolve_battle_vehicle_critical(&mut loaded, id, VehicleSection::Turret, rules);
        match (result, replay) {
            (Ok(a), Ok(b)) => assert_eq!(a, b),
            (Err(a), Err(b)) => {
                assert_eq!(a.to_string(), b.to_string());
                break;
            }
            other => panic!("Replay diverged: {other:?}"),
        }
        assert_eq!(world.btech, loaded.btech);
    }
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    let before = world.btech.clone();
    assert!(
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Turret, rules).is_err()
    );
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn combustion_fuel_critical_destroys_the_vehicle_instead_of_only_its_engine() {
    let (_dir, _config, mut world, id) =
        fixture(include_str!("../game/units/Demolisher.toml")).await;
    seed(&mut world, id, rolling(12));
    let before = world.btech.clone();
    let result =
        resolve_battle_vehicle_critical(&mut world, id, VehicleSection::Left, rules()).unwrap();
    assert_eq!(
        result.selection.effect,
        Some(VehicleCriticalEffect::FuelTank)
    );
    assert!(result.explosion.is_some());
    assert!(world.btech.vehicles()[&id].is_destroyed());
    assert_ne!(world.btech, before);
}
