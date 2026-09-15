//! Saved sprint mode shares administrative edits and admission across supported chassis.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[allow(dead_code)]
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// Read the administrative projection without depending on unit-store layout.
fn status(scripts: &Scripts, config: &Config, id: ObjectId) -> String {
    view_battle_unit_fields_action(scripts, config, ObjectId(1), id, "status2")
        .unwrap()
        .fields[0]
        .value
        .clone()
        .unwrap()
}

/// Mode edits replay, failed callbacks restore them, and only admitted orbital insertion clears them.
#[tokio::test]
async fn sprint_status_admission_and_orbital_insertion_replay() {
    for (index, source) in firing::templates().into_iter().enumerate() {
        let (_dir, config, world, id, target, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        let set = format!("btech.unit.set_field(1,{},'status2','q')", id.0);
        assert!(
            scripts
                .eval_callback::<()>(&format!("{set};error('abort')"))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        support::run_text(&scripts, &config, ObjectId(1), 1, "@setmech STATUS2 q");
        assert_eq!(status(&scripts, &config, id), "q");
        let active = scripts.world().clone();
        persistence::save(&config.database(), &active)
            .await
            .unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, active.btech);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
        if index != 5 {
            let output = support::run_text(&scripts, &config, ObjectId(1), 1, "speed back");
            assert!(
                output.contains("You can not backup while sprinting!"),
                "{index}: {output}"
            );
            assert_eq!(scripts.world().btech, active.btech);
        }
        if index < 2 {
            let mut candidate = scripts.world().clone();
            assert_eq!(
                select_battle_charge(
                    &mut candidate,
                    id,
                    ObjectId(1),
                    BattleChargeSelection::Target(target)
                )
                .unwrap_err()
                .to_string(),
                "You cannot charge while in a special movement mode!"
            );
            assert_eq!(candidate.btech, active.btech);
        }
        let request = BattleScenarioPosition {
            coordinate: BattleHexCoordinate { x: 0, y: 11 },
            elevation: Some(100),
        };
        assert!(
            initiate_battle_orbital_drop_action(
                &scripts,
                &config,
                ObjectId(1),
                id,
                BattleScenarioPosition {
                    coordinate: BattleHexCoordinate { x: -1, y: 11 },
                    ..request
                }
            )
            .is_err()
        );
        assert_eq!(scripts.world().btech, active.btech);
        initiate_battle_orbital_drop_action(&scripts, &config, ObjectId(1), id, request).unwrap();
        assert_eq!(status(&scripts, &config, id), "-");
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

/// World-aware transfer/XP speed reads current sprint and exact-one pilot advantages across chassis.
#[tokio::test]
async fn effective_speed_uses_shared_sprint_and_live_pilot_advantage() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        set_battle_character(
            &mut world,
            ObjectId(1),
            BattleCharacter {
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
        let ordinary = battle_effective_maximum_speed(&world, id, true).unwrap() as f32;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "status2", "q").unwrap();
        world = scripts.world().clone();
        for value in [0, 1, 2] {
            set_battle_character_value(
                &mut world,
                ObjectId(1),
                "Speed_Demon",
                BattleCharacterValue {
                    value,
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
            let before = world.btech.clone();
            let expected = (2.0 * ordinary / 3.0) * 2.0 + if value == 1 { 10.75 } else { 0.0 };
            assert_eq!(
                battle_effective_maximum_speed(&world, id, true).unwrap(),
                f64::from(expected)
            );
            assert_eq!(world.btech, before);
            persistence::save(&config.database(), &world).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                battle_effective_maximum_speed(&restored, id, true).unwrap(),
                f64::from(expected)
            );
            assert_eq!(restored.btech, before);
        }
    }
}

/// Throttle, cockpit/Lua ceilings and live acceleration share sprint, including saved replay.
#[tokio::test]
async fn sprint_throttle_and_ground_movement_agree_after_restart() {
    for source in firing::templates().into_iter().take(5) {
        let (_dir, config, world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let ordinary = battle_throttle_maximum(&world, id, true).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "status2", "q").unwrap();
        let maximum = battle_throttle_maximum(&scripts.world(), id, true).unwrap();
        assert!(maximum > ordinary);
        let before = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.speed({},1,'run');error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        let lua: f64 = scripts
            .eval_callback(&format!(
                "local s=btech.unit.state({}); return s.movement_maximum_speed or s.maximum_speed",
                id.0
            ))
            .unwrap();
        assert_eq!(lua, maximum);
        let output = support::run_text(&scripts, &config, ObjectId(1), 1, "speed run");
        assert!(
            output.contains(&format!("Desired speed changed to {} KPH.", maximum as i32)),
            "{output}"
        );
        let mut world = scripts.world().clone();
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for _ in 0..20 {
            let live = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
            let replay =
                advance_battle_motion(&mut restored, BattleMovementRules::STANDARD).unwrap();
            assert_eq!(live, replay);
            assert_eq!(world.btech, restored.btech);
            world.validate(&config).unwrap();
        }
        let motion = world
            .btech
            .constructed_units()
            .get(&id)
            .and_then(|unit| unit.motion())
            .or_else(|| {
                world
                    .btech
                    .vehicles()
                    .get(&id)
                    .and_then(|unit| unit.motion())
            })
            .unwrap();
        assert_eq!(motion.desired_speed, maximum);
        assert!(motion.speed > ordinary, "{} <= {}", motion.speed, ordinary);
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

/// Airborne sprint shares its velocity budget with vertical controls and survives flight replay.
#[tokio::test]
async fn airborne_sprint_preserves_vertical_budget_and_replays() {
    let source = include_str!("../game/mechs/Kestrel");
    let (_dir, config, mut world, id, _, _) =
        firing::fixture_with_target(source, None, source).await;
    firing::edit(&mut world, id, |unit| {
        unit["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
        unit["vtol_flight"]["altitude"] = 10.into();
    });
    let ordinary = battle_throttle_maximum(&world, id, true).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "status2", "q").unwrap();
    let maximum = battle_throttle_maximum(&scripts.world(), id, true).unwrap();
    let mut world = scripts.world().clone();
    set_battle_vtol_vertical_speed(&mut world, id, ObjectId(1), 5.0, false).unwrap();
    set_battle_speed(&mut world, id, ObjectId(1), maximum).unwrap();
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    for _ in 0..20 {
        assert_eq!(
            advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap(),
            advance_battle_motion(&mut restored, BattleMovementRules::STANDARD).unwrap()
        );
        assert_eq!(world.btech, restored.btech);
        world.validate(&config).unwrap();
    }
    let vehicle = &world.btech.vehicles()[&id];
    assert!(vehicle.motion().unwrap().speed > ordinary);
    assert_eq!(vehicle.vtol_flight().unwrap().vertical_speed, 5.0);
    assert!(vehicle.vtol_flight().unwrap().altitude > 10.0);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

/// Clearing an administrative mode preserves requested and actual speed until ordinary controls change them.
#[tokio::test]
async fn clearing_sprint_at_speed_retains_controls_and_saved_replay() {
    for (index, source) in firing::templates().into_iter().enumerate() {
        if index == 5 {
            continue; // Fixed emplacements cannot acquire movement controls.
        }
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        if index == 6 {
            firing::edit(&mut world, id, |unit| {
                unit["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
                unit["vtol_flight"]["altitude"] = 10.into();
            });
        }
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "status2", "q").unwrap();
        let maximum = battle_throttle_maximum(&scripts.world(), id, true).unwrap();
        let mut world = scripts.world().clone();
        set_battle_speed(&mut world, id, ObjectId(1), maximum).unwrap();
        firing::edit(&mut world, id, |unit| {
            unit["motion"]["speed"] = maximum.into()
        });
        let key = if world.btech.vehicles().contains_key(&id) {
            "vehicles"
        } else {
            "constructed"
        };
        let before =
            serde_json::to_value(&world.btech).unwrap()[key][id.0.to_string()]["motion"].clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "status2", "-").unwrap();
        let mut world = scripts.world().clone();
        assert_eq!(
            serde_json::to_value(&world.btech).unwrap()[key][id.0.to_string()]["motion"],
            before
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap(),
            advance_battle_motion(&mut restored, BattleMovementRules::STANDARD).unwrap()
        );
        assert_eq!(world.btech, restored.btech);
        let after =
            serde_json::to_value(&world.btech).unwrap()[key][id.0.to_string()]["motion"].clone();
        assert_eq!(after["speed"], before["speed"]);
        assert_eq!(after["desired_speed"], before["desired_speed"]);
        // Saved controls retain their old ceiling, but fresh commands use the current mode.
        let ordinary = battle_throttle_maximum(&world, id, true).unwrap();
        assert!(ordinary < maximum);
        let unchanged = world.btech.clone();
        if index != 6 {
            assert!(set_battle_speed(&mut world, id, ObjectId(1), maximum).is_err());
            assert_eq!(world.btech, unchanged);
        } else {
            // Rotorcraft commands clamp to their remaining vector budget.
            set_battle_speed(&mut world, id, ObjectId(1), maximum).unwrap();
            assert_eq!(
                world.btech.vehicles()[&id].motion().unwrap().desired_speed,
                ordinary
            );
        }
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let report = support::run_text(&scripts, &config, ObjectId(1), 1, "speed run");
        assert!(text::plain(&report).contains(&format!(
            "Desired speed changed to {} KPH.",
            ordinary as i32
        )));
        let mut world = scripts.world().clone();
        let requested = serde_json::to_value(&world.btech).unwrap()[key][id.0.to_string()]["motion"]["desired_speed"]
            .as_f64().unwrap();
        assert_eq!(requested, ordinary);
        set_battle_speed(&mut world, id, ObjectId(1), 0.0).unwrap();
        for _ in 0..40 {
            advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        }
        world.validate(&config).unwrap();
        let stopped =
            serde_json::to_value(&world.btech).unwrap()[key][id.0.to_string()]["motion"].clone();
        assert_eq!(stopped["speed"].as_f64().unwrap(), 0.0);
    }
}

/// Loaded movement applies environmental gravity once in both modes, queries, controls and live acceleration.
#[tokio::test]
async fn loaded_movement_gravity_agrees_across_mobile_chassis_and_replay() {
    for (index, source, sprinting) in
        firing::templates()
            .into_iter()
            .enumerate()
            .flat_map(|(index, source)| {
                [false, true].map(|sprinting| (index, source.clone(), sprinting))
            })
    {
        if index == 5 {
            continue;
        }
        let (_dir, config, mut base, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let tons = if let Some(unit) = base.btech.constructed_units().get(&id) {
            unit.definition().tons
        } else {
            base.btech.vehicles()[&id].definition().tons
        };
        firing::edit(&mut base, id, |unit| {
            unit["sprinting"] = sprinting.into();
            unit["live_mass"] = (u32::from(tons) * 1024).into();
            if index == 6 {
                unit["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
                unit["vtol_flight"]["altitude"] = 20.0.into();
            }
        });
        let map = if let Some(unit) = base.btech.constructed_units().get(&id) {
            unit.position().unwrap().map
        } else {
            base.btech.vehicles()[&id].position().unwrap().map
        };
        for quantity in [0, 7] {
            let mut loaded = base.clone();
            set_battle_inventory_named(&mut loaded, ObjectId(1), id, "Cockpit", 0, quantity)
                .unwrap();
            let standard = battle_throttle_maximum(&loaded, id, true).unwrap();
            if quantity != 0 {
                assert!(standard < battle_throttle_maximum(&base, id, true).unwrap());
            }
            for (special, gravity) in [(false, 50), (true, 0), (true, 50), (true, 100), (true, 200)]
            {
                let mut world = loaded.clone();
                let mut encoded = serde_json::to_value(&world.btech).unwrap();
                let saved_map = &mut encoded["maps"][map.0.to_string()];
                let flags = saved_map["flags"].as_i64().unwrap();
                saved_map["flags"] = (if special { flags | 2 } else { flags & !2 }).into();
                saved_map["gravity"] = gravity.into();
                world.btech = serde_json::from_value(encoded).unwrap();
                let expected = if special && gravity != 100 {
                    (standard as f32 * 100.0 / gravity.max(50) as f32) as f64
                } else {
                    standard
                };
                let before = world.btech.clone();
                assert_eq!(
                    battle_effective_maximum_speed(&world, id, true).unwrap(),
                    expected,
                    "chassis {index}, cargo {quantity}, special {special}, gravity {gravity}"
                );
                assert_eq!(battle_throttle_maximum(&world, id, true).unwrap(), expected);
                assert_eq!(world.btech, before);
                set_battle_speed(&mut world, id, ObjectId(1), expected).unwrap();
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                for tick in 1..=5 {
                    assert_eq!(
                        advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap(),
                        advance_battle_motion(&mut restored, BattleMovementRules::STANDARD)
                            .unwrap()
                    );
                    assert_eq!(world.btech, restored.btech);
                    world.validate(&config).unwrap();
                    let motion = world
                        .btech
                        .constructed_units()
                        .get(&id)
                        .and_then(|unit| unit.motion())
                        .or_else(|| {
                            world
                                .btech
                                .vehicles()
                                .get(&id)
                                .and_then(|unit| unit.motion())
                        })
                        .unwrap();
                    let speed = expected / if index == 1 { 10.0 } else { 20.0 } * f64::from(tick);
                    assert!(
                        (motion.speed - speed).abs() < 1e-8,
                        "chassis {index}, gravity {gravity}, tick {tick}: {} != {speed}",
                        motion.speed
                    );
                }
            }
        }
    }
}

/// All booster combinations retain their distinct throttle and update conversions with load and gravity.
#[tokio::test]
async fn simultaneous_boosters_sprint_load_and_gravity_replay_live_motion() {
    let source = include_str!("../game/mechs/JR7-D");
    let (_dir, config, mut base, id, _, _) =
        firing::fixture_with_target(source, None, source).await;
    let mut definition = base.btech.constructed_units()[&id].definition().clone();
    definition.max_speed = 64.5;
    definition
        .attributes
        .insert("specials".into(), "FlipArms SuperCharger_Tech".into());
    let torso = definition
        .sections
        .get_mut(&BattleSection::LeftTorso)
        .unwrap();
    let slot = (0..12)
        .find(|slot| !torso.criticals.contains_key(slot))
        .unwrap();
    torso.criticals.insert(
        slot,
        CriticalDefinition {
            equipment: "Masc".into(),
            data: "-".into(),
            modes: vec![],
            brand: None,
        },
    );
    firing::edit(&mut base, id, |unit| {
        unit["definition"] = serde_json::to_value(&definition).unwrap();
        unit["live_mass"] = (35 * 1024).into();
    });
    let map = base.btech.constructed_units()[&id].position().unwrap().map;
    for mask in 0_u8..8 {
        for quantity in [0, 7] {
            for gravity in [50, 100, 200] {
                let mut world = base.clone();
                set_battle_inventory_named(&mut world, ObjectId(1), id, "Cockpit", 0, quantity)
                    .unwrap();
                if mask & 1 != 0 {
                    toggle_battle_masc(&mut world, id, ObjectId(1)).unwrap();
                }
                if mask & 2 != 0 {
                    toggle_battle_supercharger(&mut world, id, ObjectId(1)).unwrap();
                }
                firing::edit(&mut world, id, |unit| {
                    unit["sprinting"] = (mask & 4 != 0).into()
                });
                let mut encoded = serde_json::to_value(&world.btech).unwrap();
                let saved_map = &mut encoded["maps"][map.0.to_string()];
                saved_map["flags"] = (saved_map["flags"].as_i64().unwrap() | 2).into();
                saved_map["gravity"] = gravity.into();
                world.btech = serde_json::from_value(encoded).unwrap();
                // Seven half-ton crates cost seven effective tons without CargoTech.
                let loaded: f32 = if quantity == 0 { 64.5 } else { 53.75 };
                let count = mask.count_ones();
                let effective = if count == 0 {
                    loaded
                } else {
                    (2.0 * loaded / 3.0) * (1.5 + 0.5 * count as f32)
                };
                let effective = if gravity == 100 {
                    effective
                } else {
                    effective * 100.0 / gravity as f32
                };
                let maximum = battle_throttle_maximum(&world, id, true).unwrap();
                // Ordinary controls retain f64 precision; reference arithmetic rounds each f32 operation.
                let tolerance = 4.0 * f64::from(f32::EPSILON) * f64::from(effective).max(1.0);
                assert!(
                    (maximum - f64::from(effective)).abs() <= tolerance,
                    "mask {mask}, cargo {quantity}, gravity {gravity}: {maximum} != {effective}"
                );
                assert!(
                    (battle_effective_maximum_speed(&world, id, true).unwrap()
                        - f64::from(effective))
                    .abs()
                        <= tolerance
                );
                let update = match mask & 3 {
                    3 => ((effective / 1.5).round_ties_even() / 10.75 * 2.5).ceil() * 10.75,
                    1 | 2 => effective * (4.0 / 3.0),
                    _ => effective,
                };
                set_battle_speed(&mut world, id, ObjectId(1), maximum).unwrap();
                world.validate(&config).unwrap();
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                for tick in 1..=5 {
                    assert_eq!(
                        advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap(),
                        advance_battle_motion(&mut restored, BattleMovementRules::STANDARD)
                            .unwrap()
                    );
                    assert_eq!(world.btech, restored.btech);
                    world.validate(&config).unwrap();
                    let motion = world.btech.constructed_units()[&id].motion().unwrap();
                    let expected = f64::from(update) / 20.0 * f64::from(tick);
                    assert!(
                        (motion.speed - expected).abs() < 1e-5,
                        "mask {mask}, cargo {quantity}, gravity {gravity}, tick {tick}: {} != {expected}",
                        motion.speed
                    );
                    assert_eq!(motion.desired_speed, maximum);
                }
            }
        }
    }
}

/// Changing the host myomer policy at speed preserves controls and changes subsequent acceleration.
#[tokio::test]
async fn hot_myomer_policy_changes_at_speed_preserve_controls_and_replay() {
    let source = include_str!("../game/mechs/BJ-TSM");
    let (_dir, config, mut world, id, _, _) =
        firing::fixture_with_target(source, None, source).await;
    firing::edit(&mut world, id, |unit| {
        unit["live_mass"] = (45 * 1024).into();
        unit["sprinting"] = true.into();
        unit["heat"]["excess"] = 9.0.into();
    });
    set_battle_speed(&mut world, id, ObjectId(1), 96.75).unwrap();
    let mut expected_speed = 0.0;
    for (enabled, throttle, update) in [
        (true, 96.75, 118.25),
        (false, 86.0, 96.75),
        (true, 96.75, 118.25),
    ] {
        let path = config.root.join("stompymux.toml");
        let mut settings: toml::Value =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        settings
            .as_table_mut()
            .unwrap()
            .entry("battletech")
            .or_insert_with(|| toml::Value::Table(Default::default()))
            .as_table_mut()
            .unwrap()
            .insert("tsm_sprint_bonus".into(), i64::from(enabled).into());
        std::fs::write(&path, toml::to_string(&settings).unwrap()).unwrap();
        let configured = Config::load(&config.root).unwrap();
        let scripts = Scripts::new(&configured, Rc::new(RefCell::new(world.clone()))).unwrap();
        let maximum: f64 = scripts
            .eval_callback(&format!(
                "return btech.unit.state({}).movement_maximum_speed",
                id.0
            ))
            .unwrap();
        assert_eq!(maximum, throttle);
        assert_eq!(scripts.world().btech, world.btech);
        world.validate(&configured).unwrap();
        persistence::save(&configured.database(), &world)
            .await
            .unwrap();
        let mut restored = persistence::load(&configured.database()).await.unwrap();
        let rules = BattleMovementRules {
            tsm_sprint_bonus: enabled,
            ..BattleMovementRules::STANDARD
        };
        for _ in 0..5 {
            assert_eq!(
                advance_battle_motion(&mut world, rules).unwrap(),
                advance_battle_motion(&mut restored, rules).unwrap()
            );
            assert_eq!(world.btech, restored.btech);
            expected_speed += update / 20.0;
            let motion = world.btech.constructed_units()[&id].motion().unwrap();
            assert!((motion.speed - expected_speed).abs() < 1e-10);
            assert_eq!(motion.desired_speed, 96.75);
            world.validate(&configured).unwrap();
        }
    }
}

/// First tail-rotor damage uses loaded sprint/gravity speed; repeats preserve the existing controls.
#[tokio::test]
async fn tail_rotor_damage_uses_world_velocity_budget_and_replays() {
    let source = include_str!("../game/mechs/Kestrel");
    let (_dir, config, mut world, id, _, _) =
        firing::fixture_with_target(source, None, source).await;
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 9)
        .unwrap();
    firing::edit(&mut world, id, |unit| {
        unit["sprinting"] = true.into();
        unit["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
        unit["vtol_flight"]["altitude"] = 20.0.into();
        unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    });
    set_battle_inventory_named(&mut world, ObjectId(1), id, "Cockpit", 0, 7).unwrap();
    let map = world.btech.vehicles()[&id].position().unwrap().map;
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    let state = &mut encoded["maps"][map.0.to_string()];
    state["flags"] = (state["flags"].as_i64().unwrap() | 2).into();
    state["gravity"] = 50.into();
    world.btech = serde_json::from_value(encoded).unwrap();
    let maximum = battle_throttle_maximum(&world, id, true).unwrap();
    set_battle_vtol_vertical_speed(&mut world, id, ObjectId(1), 5.0, false).unwrap();
    set_battle_speed(&mut world, id, ObjectId(1), maximum).unwrap();
    let speed = world.btech.vehicles()[&id].motion().unwrap().desired_speed;
    firing::edit(&mut world, id, |unit| {
        unit["motion"]["speed"] = speed.into()
    });
    let cruise = (maximum * maximum - 25.0).sqrt() * 2.0 / 3.0;
    let rules = BattleVehicleCriticalRules {
        rotor_damage_divisor: 0,
        extended_piloting: false,
        vtol_table: None,
        table: BattleVehicleCriticalTable::Advanced,
        enabled: true,
        combat_safe: false,
        toughness: false,
    };
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    let report =
        resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Rotor, rules)
            .unwrap();
    assert_eq!(
        report.selection.effect,
        Some(BattleVehicleCriticalEffect::Rotor(
            BattleRotorHit::TailRotor
        ))
    );
    assert!(report.notices.iter().any(|notice| {
        notice
            .text
            .contains("Your tail rotor is damaged, slowing you down!")
    }));
    assert_eq!(
        report,
        resolve_battle_vehicle_critical(&mut restored, id, BattleVehicleSection::Rotor, rules)
            .unwrap()
    );
    assert_eq!(world.btech, restored.btech);
    let motion = world.btech.vehicles()[&id].motion().unwrap();
    assert_eq!(motion.speed, speed);
    assert!(
        (motion.desired_speed - (cruise - 0.1)).abs() < 1e-9,
        "{} != {}",
        motion.desired_speed,
        cruise - 0.1
    );
    assert!(set_battle_speed(&mut world, id, ObjectId(1), maximum).is_err());
    set_battle_speed(&mut world, id, ObjectId(1), cruise).unwrap();
    // A later environmental change does not turn repeated damage into a new speed-limiting event.
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    encoded["maps"][map.0.to_string()]["gravity"] = 200.into();
    encoded["vehicles"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    world.btech = serde_json::from_value(encoded).unwrap();
    let before = world.btech.vehicles()[&id].motion().unwrap();
    let repeated =
        resolve_battle_vehicle_critical(&mut world, id, BattleVehicleSection::Rotor, rules)
            .unwrap();
    assert!(
        repeated
            .notices
            .iter()
            .any(|notice| notice.text == "Your damaged tail rotor suffers more damage!")
    );
    assert_eq!(world.btech.vehicles()[&id].motion().unwrap(), before);
    world.validate(&config).unwrap();
}
