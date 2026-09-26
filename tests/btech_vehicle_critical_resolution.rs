//! Critical resolution commits selected effects and rolls together, with explicit unsupported outcomes.
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
        BattleVehicleTemplate::parse(template).unwrap(),
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

#[tokio::test]
async fn critical_resolution_applies_all_tables_for_tactical_crews() {
    use BattleVehicleCriticalEffect as E;
    use BattleVehicleCriticalTable as T;
    use BattleVehicleSection as S;
    let (_dir, config, base, id) =
        fixture(&include_str!("../game/mechs/Demolisher").replace("ICEEngine_Tech", "")).await;
    let mut seen = std::collections::BTreeSet::new();
    let mut saved = None;
    for table in [T::Standard, T::Advanced] {
        let rules = BattleVehicleCriticalRules {
            rotor_damage_divisor: 0,
            extended_piloting: false,
            vtol_table: None,
            toughness: false,
            table,
            enabled: true,
            combat_safe: false,
        };
        for section in [S::Front, S::Left, S::Right, S::Rear, S::Turret] {
            for value in 0..128 {
                let mut world = base.clone();
                seed(&mut world, id, value);
                let mut selected = world.clone();
                let expected =
                    roll_battle_vehicle_critical(&mut selected, id, section, rules).unwrap();
                let result = resolve_battle_vehicle_critical(&mut world, id, section, rules);
                let result = result.unwrap();
                assert_eq!(result.selection, expected);
                let Some(effect) = expected.effect else {
                    assert!(result.notices.is_empty());
                    assert_eq!(world.btech, selected.btech);
                    continue;
                };
                seen.insert(format!("{effect:?}"));
                assert!(result.notices[0].text.contains("CRITICAL HIT"));
                let vehicle = &world.btech.vehicles()[&id];
                match effect {
                    E::Rotor(_) | E::VtolPilot | E::VtolCopilot => {
                        panic!("Ground vehicle selected an aircraft critical")
                    }
                    E::CrewHit => {
                        assert_eq!(vehicle.pilot_injuries(), 1);
                        assert_eq!(result.pilot_injury.as_ref().unwrap().injuries, 1);
                    }
                    E::CrewKilled => {
                        assert!(vehicle.is_destroyed());
                        assert_eq!(vehicle.pilot_injuries(), 0);
                        assert!(vehicle.crew_killed());
                        assert_eq!(vehicle.power(), BattlePower::Off);
                        assert!(result.pilot_injury.is_none());
                    }
                    E::PowerPlant => {
                        assert!(vehicle.is_destroyed());
                        assert!(!result.explosion.as_ref().unwrap().contained);
                        assert!(vehicle.sections().values().all(|state| state.internal == 0));
                    }
                    E::WeaponDestroyed => assert_eq!(
                        vehicle.lost_criticals().len(),
                        usize::from(section == S::Turret)
                    ),
                    E::TurretBlownOff => assert_eq!(vehicle.sections()[&S::Turret].internal, 0),
                    E::Ammunition => {
                        assert!(vehicle.ammunition().iter().all(|rounds| *rounds == 0));
                        assert_eq!(result.ammunition_cascade.as_ref().unwrap().damage, 400);
                        assert_eq!(result.internal_damage.len(), 1);
                    }
                    E::Driver => assert_eq!(vehicle.piloting_damage(), 2),
                    E::Sensors => assert_eq!(vehicle.gunnery_damage(), 1),
                    E::Commander => {
                        assert_eq!(vehicle.piloting_damage(), 1);
                        assert_eq!(vehicle.gunnery_damage(), 1);
                        assert_eq!(vehicle.crew_stun_remaining(), 60);
                        assert_eq!(result.notices.len(), 3);
                    }
                    E::CrewStunned => assert_eq!(vehicle.crew_stun_remaining(), 60),
                    E::Cargo => assert_eq!(world.btech, selected.btech),
                    E::Stabilizer => assert!(vehicle.lost_stabilizers().contains(&section)),
                    E::TurretJam => assert!(vehicle.turret_jammed()),
                    E::TurretLock => assert!(vehicle.turret_locked()),
                    E::MotiveSpeedLoss => assert_eq!(vehicle.motive_speed_loss(), 10.75),
                    E::Immobilize => assert!(vehicle.immobilized()),
                    E::Engine | E::FuelTank => {
                        assert_eq!(vehicle.maximum_speed(), 0.0);
                        assert_eq!(vehicle.immobilized(), table == T::Advanced);
                        assert_eq!(
                            vehicle.definition(),
                            base.btech.vehicles()[&id].definition()
                        );
                        saved = Some(world.clone());
                    }
                    E::MainWeaponJam => {
                        assert_eq!(vehicle.weapon_failures().len(), 1);
                        assert!(
                            vehicle
                                .weapon_failures()
                                .values()
                                .all(|failure| *failure == BattleEquipmentFailure::Disabled)
                        );
                        assert!(vehicle.weapon_recycle().is_empty());
                    }
                    E::WeaponJam => {
                        assert_eq!(
                            vehicle.weapon_failures().len(),
                            usize::from(section == S::Turret)
                        );
                        if section == S::Turret {
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
        }
    }
    assert_eq!(seen.len(), 20, "{seen:?}");
    let world = saved.unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(world.btech, loaded.btech);
}

#[tokio::test]
async fn critical_resolution_suppression_and_replay_preserve_world_boundaries() {
    let (_dir, config, mut world, id) = fixture(include_str!("../game/mechs/Demolisher")).await;
    let mut rules = BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        toughness: false,
        table: BattleVehicleCriticalTable::Advanced,
        enabled: false,
        combat_safe: false,
    };
    let before = world.btech.clone();
    let result =
        resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Front, rules)
            .unwrap();
    assert!(result.notices.is_empty());
    assert_eq!(world.btech, before);
    rules.enabled = true;
    rules.combat_safe = true;
    assert!(
        resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Turret, rules)
            .unwrap()
            .selection
            .effect
            .is_none()
    );
    assert_eq!(world.btech, before);
    rules.combat_safe = false;
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    for _ in 0..30 {
        let result =
            resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Turret, rules);
        let replay =
            resolve_battle_vehicle_critical(&mut loaded, id, BattleVehicleSection::Turret, rules);
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
        resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Turret, rules)
            .is_err()
    );
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn combustion_fuel_critical_destroys_the_vehicle_instead_of_only_its_engine() {
    let (_dir, _config, mut world, id) = fixture(include_str!("../game/mechs/Demolisher")).await;
    let value = (0..=255)
        .find(|value| BattleDice::seeded([*value; 32]).two_d6() == 12)
        .unwrap();
    seed(&mut world, id, value);
    let before = world.btech.clone();
    let rules = BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        toughness: false,
        table: BattleVehicleCriticalTable::Advanced,
        enabled: true,
        combat_safe: false,
    };
    let result =
        resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Left, rules).unwrap();
    assert!(result.explosion.is_some());
    assert!(world.btech.vehicles()[&id].is_destroyed());
    assert_ne!(world.btech, before);
}
