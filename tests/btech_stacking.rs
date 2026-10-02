//! Crowding thresholds, physical collision inputs and atomic damage/restart contracts.
use crate::support;
use stompymux_rs::*;

/// A supplied team roster shares one hex; the first unit has the active cockpit.
async fn fixture(teams: &[i32]) -> (tempfile::TempDir, Config, World, Vec<ObjectId>) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Crowded field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "crowding.map",
        BattleMapAsset::parse("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for &team in teams {
        let id = world.create(&config, "Jenner".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(
            &mut world,
            id,
            BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml"))
                .unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, 1, 1).unwrap();
        let mut signature = world.btech.constructed_units()[&id].signature();
        signature.team = team;
        set_battle_unit_signature(&mut world, id, signature).unwrap();
        ids.push(id);
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ids[0]);
    assign_battle_pilot(&mut world, ids[0], ObjectId(1)).unwrap();
    start_battle_unit(&mut world, ids[0], ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let running = state["constructed"][ids[0].0.to_string()]["power"].clone();
    for id in &ids {
        state["constructed"][id.0.to_string()]["power"] = running.clone();
    }
    world.btech = serde_json::from_value(state).unwrap();
    world.validate(&config).unwrap();
    (dir, config, world, ids)
}

/// Seed the selection roll and the following piloting roll when requested.
fn seed(world: &mut World, id: ObjectId, count: u16, selection: u16, roll: Option<u8>) {
    let chosen = (0..=255)
        .find(|value| {
            let mut dice = BattleDice::seeded([*value; 32]);
            dice.die(count).unwrap() == selection && roll.is_none_or(|roll| dice.two_d6() == roll)
        })
        .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([chosen; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
}

/// Current mass and thrust are explicit until construction-weight calculation is integrated.
fn input(entry: BattleStackingEntry) -> BattleStackingInput {
    BattleStackingInput {
        entry,
        mass: 35 * 1024,
        jump_movement_points: 5,
    }
}

/// Ordinary tactical damage configuration.
fn fall() -> BattleFallRules {
    BattleMovementRules::STANDARD.fall
}

#[tokio::test]
async fn crowding_thresholds_and_disabled_mode_do_not_consume_dice() {
    for teams in [vec![0, 0], vec![0, 0, 1], vec![0, 0, 1, 1, 1, 1]] {
        let (_dir, _, mut world, ids) = fixture(&teams).await;
        let before = world.btech.clone();
        assert!(
            resolve_battle_stacking(
                &mut world,
                ids[0],
                input(BattleStackingEntry::Jump),
                BattleStackingRules::STANDARD,
                fall()
            )
            .unwrap()
            .is_empty()
        );
        assert_eq!(world.btech, before);
    }
    let (_dir, _, mut world, ids) = fixture(&[0, 0, 0]).await;
    let before = world.btech.clone();
    assert!(
        resolve_battle_stacking(
            &mut world,
            ids[0],
            input(BattleStackingEntry::Jump),
            BattleStackingRules {
                mode: 0,
                ..BattleStackingRules::STANDARD
            },
            fall()
        )
        .unwrap()
        .is_empty()
    );
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn crowding_selection_obeys_team_priority_and_stopped_target_gaps() {
    for (teams, count, target) in [
        (vec![0, 1, 0, 0, 1, 1, 1, 0], 3, 2),
        (vec![0, 1, 1, 1, 1, 1, 1, 1], 7, 1),
    ] {
        let (_dir, config, mut world, ids) = fixture(&teams).await;
        seed(&mut world, ids[0], count, 1, None);
        let events = resolve_battle_stacking(
            &mut world,
            ids[0],
            input(BattleStackingEntry::Jump),
            BattleStackingRules::STANDARD,
            fall(),
        )
        .unwrap();
        assert_eq!(events[0].unit, ids[0]);
        assert_eq!(events[1].unit, ids[target]);
        world.validate(&config).unwrap();
    }
    let (_dir, _, mut world, ids) = fixture(&[0, 0, 0]).await;
    stop_battle_unit(
        &mut world,
        ids[0],
        ObjectId(1),
        stompymux_rs::BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    // Shut-down units count toward the threshold, but cannot be selected as targets.
    seed(&mut world, ids[1], 3, 3, None);
    let before = world.btech.clone();
    assert!(
        resolve_battle_stacking(
            &mut world,
            ids[1],
            input(BattleStackingEntry::Fall),
            BattleStackingRules::STANDARD,
            fall()
        )
        .unwrap()
        .is_empty()
    );
    assert_ne!(world.btech, before); // Selection still consumes one die.
}

#[tokio::test]
async fn crowding_avoidance_falls_and_success_replay_without_target_damage() {
    for entry in [
        BattleStackingEntry::Ground,
        BattleStackingEntry::Jump,
        BattleStackingEntry::Fall,
    ] {
        for success in [false, true] {
            let (_dir, config, mut world, ids) = fixture(if entry == BattleStackingEntry::Jump {
                &[0, 0, 0, 0]
            } else {
                &[0, 0, 0]
            })
            .await;
            let id = ids[0];
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["constructed"][id.0.to_string()]["motion"]["speed"] = 21.5.into();
            state["constructed"][id.0.to_string()]["motion"]["desired_speed"] = 21.5.into();
            world.btech = serde_json::from_value(state).unwrap();
            let target = if entry == BattleStackingEntry::Ground {
                9
            } else {
                11
            };
            seed(&mut world, id, 3, 1, Some(target - u8::from(!success)));
            let other = world.btech.constructed_units()[&ids[1]].clone();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            let rules = BattleStackingRules {
                mode: 1,
                ..BattleStackingRules::STANDARD
            };
            let events =
                resolve_battle_stacking(&mut world, id, input(entry), rules, fall()).unwrap();
            assert_eq!(
                events,
                resolve_battle_stacking(&mut loaded, id, input(entry), rules, fall()).unwrap()
            );
            assert_eq!(world.btech, loaded.btech);
            assert_eq!(world.btech.constructed_units()[&ids[1]], other);
            let unit = &world.btech.constructed_units()[&id];
            assert_eq!(
                unit.posture(),
                if success {
                    BattlePosture::Standing
                } else {
                    BattlePosture::Prone
                }
            );
            assert_eq!(
                unit.motion().unwrap().speed,
                if success && entry != BattleStackingEntry::Ground {
                    21.5
                } else {
                    0.0
                }
            );
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
        }
    }
}

#[tokio::test]
async fn collision_damage_and_second_unit_failure_commit_together() {
    let (_dir, config, mut world, ids) = fixture(&[0, 0, 0, 0]).await;
    seed(&mut world, ids[0], 3, 1, None);
    let mut rejected = world.clone();
    rejected
        .objects
        .get_mut(&ids[0])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let before = rejected.btech.clone();
    assert!(
        resolve_battle_stacking(
            &mut rejected,
            ids[0],
            input(BattleStackingEntry::Jump),
            BattleStackingRules::STANDARD,
            fall()
        )
        .is_err()
    );
    assert_eq!(rejected.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let before = world.btech.clone();
    let events = resolve_battle_stacking(
        &mut world,
        ids[0],
        input(BattleStackingEntry::Jump),
        BattleStackingRules::STANDARD,
        fall(),
    )
    .unwrap();
    assert_eq!(
        events,
        resolve_battle_stacking(
            &mut loaded,
            ids[0],
            input(BattleStackingEntry::Jump),
            BattleStackingRules::STANDARD,
            fall()
        )
        .unwrap()
    );
    assert_eq!(world.btech, loaded.btech);
    for id in &ids[..2] {
        assert_ne!(
            world.btech.constructed_units()[id].sections(),
            before.constructed_units()[id].sections()
        );
    }
    assert_eq!(
        world.btech.constructed_units()[&ids[2]],
        before.constructed_units()[&ids[2]]
    );
    world.validate(&config).unwrap();
}

#[tokio::test]
async fn collision_tables_use_punch_kick_and_prone_normal_locations() {
    for entry in [
        BattleStackingEntry::Ground,
        BattleStackingEntry::Jump,
        BattleStackingEntry::Fall,
    ] {
        for prone in [false, true] {
            let (_dir, config, mut world, ids) = fixture(if entry == BattleStackingEntry::Jump {
                &[0, 0, 0, 0]
            } else {
                &[0, 0, 0]
            })
            .await;
            seed(&mut world, ids[0], 3, 1, None);
            let target_seed = (0..=255)
                .find(|value| {
                    let mut dice = BattleDice::seeded([*value; 32]);
                    dice.d6() == 3 && dice.d6() == 5
                })
                .unwrap();
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["constructed"][ids[1].0.to_string()]["dice"] =
                serde_json::to_value(BattleDice::seeded([target_seed; 32])).unwrap();
            state["constructed"][ids[1].0.to_string()]["posture"] =
                serde_json::to_value(if prone {
                    BattlePosture::Prone
                } else {
                    BattlePosture::Standing
                })
                .unwrap();
            state["constructed"][ids[0].0.to_string()]["motion"]["speed"] = 21.5.into();
            world.btech = serde_json::from_value(state).unwrap();
            let before = world.btech.constructed_units()[&ids[1]].sections().clone();
            let physical = BattleStackingInput {
                entry,
                mass: if entry == BattleStackingEntry::Ground {
                    35 * 1024
                } else {
                    20 * 1024
                },
                jump_movement_points: 2,
            };
            let events = resolve_battle_stacking(
                &mut world,
                ids[0],
                physical,
                BattleStackingRules::STANDARD,
                fall(),
            )
            .unwrap();
            let location = if prone || entry == BattleStackingEntry::Ground {
                BattleSection::LeftTorso
            } else {
                BattleSection::CenterTorso
            };
            let warning = before[&location].armor - 5 < before[&location].armor / 2;
            assert_eq!(events.len(), 2 + usize::from(warning));
            if warning {
                assert_eq!(events[2].unit, ids[1]);
                assert!(events[2].text.contains("WARNING:"));
                assert!(events[2].text.contains("Armor low."));
            }
            let after = world.btech.constructed_units()[&ids[1]].sections();
            for (&section, original) in &before {
                assert_eq!(
                    after[&section].armor,
                    original.armor - if section == location { 5 } else { 0 }
                );
                assert_eq!(after[&section].internal, original.internal);
                assert_eq!(after[&section].rear, original.rear);
            }
            world.validate(&config).unwrap();
        }
    }
}

#[tokio::test]
async fn ground_entry_collisions_use_current_mass_and_replay_the_whole_tick() {
    for mode in [0, 1, 2] {
        let (_dir, config, mut world, ids) = fixture(&[0, 0, 0]).await;
        let id = ids[0];
        seed(&mut world, id, 3, 1, Some(9));
        let mut state = serde_json::to_value(&world.btech).unwrap();
        let moving = &mut state["constructed"][id.0.to_string()];
        moving["position"]["y"] = 0.into();
        moving["motion"]["point"] = serde_json::to_value(BattlePoint {
            y: 0.49,
            ..BattleHexCoordinate { x: 1, y: 0 }.center()
        })
        .unwrap();
        moving["motion"]["heading"] = 180.0.into();
        moving["motion"]["desired_heading"] = 180.0.into();
        moving["motion"]["speed"] = 21.5.into();
        moving["motion"]["desired_speed"] = 21.5.into();
        world.btech = serde_json::from_value(state).unwrap();
        world.validate(&config).unwrap();
        let movement = BattleMovementRules {
            fall: BattleFallRules {
                stacking: BattleStackingRules {
                    mode,
                    ..BattleStackingRules::STANDARD
                },
                ..fall()
            },
            ..BattleMovementRules::STANDARD
        };
        if mode == 2 {
            let mut rejected = world.clone();
            rejected
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            let before = rejected.btech.clone();
            assert!(advance_battle_motion(&mut rejected, movement).is_err());
            assert_eq!(rejected.btech, before);
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_motion(&mut world, movement).unwrap();
        assert_eq!(
            notices,
            advance_battle_motion(&mut loaded, movement).unwrap()
        );
        assert_eq!(world.btech, loaded.btech);
        assert_eq!(
            notices.iter().any(|notice| notice.text.contains("bump")),
            mode != 0
        );
        assert_eq!(
            world.btech.constructed_units()[&id].position().unwrap().y,
            1
        );
        assert_eq!(
            world.btech.constructed_units()[&id].motion().unwrap().speed,
            if mode == 1 { 0.0 } else { 21.5 }
        );
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let mass:i64 = scripts.eval_callback(&format!("local s=btech.unit.state({}); local n=s.mass.total; s.mass.total=0; assert(btech.unit.state({}).mass.total==n); return n",id.0,id.0)).unwrap();
        assert_eq!(
            mass,
            i64::from(world.btech.constructed_units()[&id].mass().unwrap().total)
        );
        let text = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech inspect #{}", id.0),
        );
        assert!(text.contains("Current mass:"), "{text}");
        persistence::save(&config.database(), &world).await.unwrap();
        let reloaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            world.btech.constructed_units()[&id].mass().unwrap(),
            reloaded.btech.constructed_units()[&id].mass().unwrap()
        );
    }
}

/// An airborne Jenner is one tick away from landing beside the supplied number of bipeds.
async fn landing_fixture(neighbors: usize) -> (tempfile::TempDir, Config, World, Vec<ObjectId>) {
    let (dir, config, mut world, ids) = fixture(&vec![0; neighbors + 1]).await;
    let map = world.btech.constructed_units()[&ids[0]]
        .position()
        .unwrap()
        .map;
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["movement_modifier"] = 800.into();
    for id in &ids[1..] {
        state["constructed"][id.0.to_string()]["position"]["y"] = 2.into();
        state["constructed"][id.0.to_string()]["motion"]["point"] =
            serde_json::to_value(BattleHexCoordinate { x: 1, y: 2 }.center()).unwrap();
    }
    world.btech = serde_json::from_value(state).unwrap();
    launch_battle_jump(&mut world, ids[0], ObjectId(1), 180, 1.0).unwrap();
    advance_battle_jumps(
        &mut world,
        stompymux_rs::BattleMovementRules {
            fall: BattleFallRules {
                stacking: BattleStackingRules::STANDARD,
                ..fall()
            },
            ..stompymux_rs::BattleMovementRules::STANDARD
        },
    )
    .unwrap();
    assert!(world.btech.constructed_units()[&ids[0]].flight().is_some());
    world.validate(&config).unwrap();
    (dir, config, world, ids)
}

#[tokio::test]
async fn completed_landings_exclude_the_arriving_unit_and_replay_crowding() {
    for neighbors in [2, 3] {
        for (mode, success) in [(0, true), (1, true), (1, false), (2, true)] {
            let (_dir, config, mut world, ids) = landing_fixture(neighbors).await;
            let id = ids[0];
            refresh_battle_contacts(&mut world, &[ids[1]]).unwrap();
            assert!(
                visible_battle_contact(&world, ids[1], id)
                    .unwrap()
                    .is_some()
            );
            seed(&mut world, id, 3, 1, Some(if success { 11 } else { 10 }));
            let before = world.btech.clone();
            let rules = BattleFallRules {
                stacking: BattleStackingRules {
                    mode,
                    ..BattleStackingRules::STANDARD
                },
                ..fall()
            };
            if mode == 2 && neighbors == 3 {
                let mut rejected = world.clone();
                rejected
                    .objects
                    .get_mut(&ids[1])
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
                let original = rejected.btech.clone();
                assert!(
                    advance_battle_jumps(
                        &mut rejected,
                        stompymux_rs::BattleMovementRules {
                            fall: rules,
                            ..stompymux_rs::BattleMovementRules::STANDARD
                        }
                    )
                    .is_err()
                );
                assert_eq!(rejected.btech, original);
            }
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            let events = advance_battle_jumps(
                &mut world,
                stompymux_rs::BattleMovementRules {
                    fall: rules,
                    ..stompymux_rs::BattleMovementRules::STANDARD
                },
            )
            .unwrap();
            assert_eq!(
                events,
                advance_battle_jumps(
                    &mut loaded,
                    stompymux_rs::BattleMovementRules {
                        fall: rules,
                        ..stompymux_rs::BattleMovementRules::STANDARD
                    }
                )
                .unwrap()
            );
            assert_eq!(world.btech, loaded.btech);
            let collides = neighbors == 3 && mode != 0;
            assert_eq!(
                events
                    .iter()
                    .filter(|notice| notice.unit == ids[1]
                        && notice.text.ends_with("lands gracefully."))
                    .count(),
                usize::from(!collides)
            );
            assert_eq!(
                events
                    .iter()
                    .any(|notice| notice.text.contains("another unit")),
                collides
            );
            let unit = &world.btech.constructed_units()[&id];
            assert!(unit.flight().is_none());
            assert_eq!(unit.jump_stabilization(), 12);
            if mode != 2 {
                assert_eq!(
                    unit.posture(),
                    if collides && !success {
                        BattlePosture::Prone
                    } else {
                        BattlePosture::Standing
                    }
                );
            }
            if !collides {
                assert_eq!(
                    serde_json::to_value(unit).unwrap()["dice"],
                    serde_json::to_value(&before.constructed_units()[&id]).unwrap()["dice"]
                );
            }
            for target in &ids[1..] {
                if mode != 2 || !collides {
                    assert_eq!(
                        world.btech.constructed_units()[target],
                        before.constructed_units()[target]
                    );
                }
            }
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
        }
    }
}

#[tokio::test]
async fn early_landing_uses_configured_crowding_in_native_and_lua_transactions() {
    for mode in [0, 1, 2] {
        let (dir, _config, mut world, ids) = landing_fixture(3).await;
        let path = dir.path().join("stompymux.toml");
        let mut document: toml::Value =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        document["battletech"]
            .as_table_mut()
            .unwrap()
            .insert("stacking".into(), mode.into());
        document["battletech"]
            .as_table_mut()
            .unwrap()
            .insert("stackdamage".into(), 50.into());
        std::fs::write(&path, toml::to_string(&document).unwrap()).unwrap();
        let config = Config::load(dir.path()).unwrap();
        let id = ids[0];
        let chosen = (0..=255)
            .find(|value| {
                let mut dice = BattleDice::seeded([*value; 32]);
                dice.two_d6() >= 6 && dice.die(3).unwrap() == 1
            })
            .unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([chosen; 32])).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        refresh_battle_contacts(&mut world, &[ids[2]]).unwrap();
        assert!(
            visible_battle_contact(&world, ids[2], id)
                .unwrap()
                .is_some()
        );
        assert!(
            visible_battle_contact(&world, ids[2], ids[1])
                .unwrap()
                .is_some()
        );
        let witness = world.create(&config, "Collision witness".into(), Kind::Player);
        world.objects.get_mut(&witness).unwrap().location = Some(ids[2]);
        world
            .objects
            .get_mut(&witness)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let messages = |scripts: &Scripts| {
            scripts
                .drain_outbox()
                .into_iter()
                .map(|(id, text)| (id, text.source().to_owned()))
                .collect::<Vec<_>>()
        };
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.land({},1); error('abort crowded landing')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        assert!(messages(&lua).is_empty());
        commands::run(&native, &config, ObjectId(1), 1, "land").unwrap();
        lua.eval_callback::<()>(&format!("btech.unit.land({},1)", id.0))
            .unwrap();
        let output = messages(&native);
        assert_eq!(messages(&lua), output);
        let observed: Vec<_> = output
            .iter()
            .filter(|(recipient, _)| *recipient == witness)
            .map(|(_, text)| text.as_str())
            .collect();
        let action = match mode {
            0 => "lands gracefully.".to_owned(),
            1 => format!("nearly lands on #{} Jenner!", ids[1].0),
            _ => format!("lands on #{} Jenner!", ids[1].0),
        };
        let expected = format!("#{} Jenner {action}", id.0);
        assert_eq!(observed.iter().filter(|text| **text == expected).count(), 1);
        assert_eq!(native.world().btech, lua.world().btech);
        assert!(
            native.world().btech.constructed_units()[&id]
                .flight()
                .is_none()
        );
        let saved = native.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

#[tokio::test]
async fn landing_collision_thrust_uses_gravity_only_with_special_conditions() {
    for special in [false, true] {
        let (_dir, config, mut world, ids) = landing_fixture(3).await;
        let id = ids[0];
        let map = world.btech.constructed_units()[&id].position().unwrap().map;
        let mover_seed = (0..=255)
            .find(|value| {
                let mut dice = BattleDice::seeded([*value; 32]);
                dice.two_d6() >= 6 && dice.die(3).unwrap() == 1
            })
            .unwrap();
        let target_seed = (0..=255)
            .find(|value| {
                let mut dice = BattleDice::seeded([*value; 32]);
                let first = dice.d6();
                dice.two_d6(); // First damage packet enters material resolution.
                first == 3 && dice.d6() == 3
            })
            .unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["maps"][map.0.to_string()]["gravity"] = 200.into();
        state["maps"][map.0.to_string()]["flags"] = if special { 2 } else { 0 }.into();
        for (unit, value) in [(id, mover_seed), (ids[1], target_seed)] {
            state["constructed"][unit.0.to_string()]["dice"] =
                serde_json::to_value(BattleDice::seeded([value; 32])).unwrap();
        }
        world.btech = serde_json::from_value(state).unwrap();
        let before =
            world.btech.constructed_units()[&ids[1]].sections()[&BattleSection::CenterTorso].armor;
        let rules = BattleFallRules {
            stacking: BattleStackingRules {
                damage_percent: 50,
                ..BattleStackingRules::STANDARD
            },
            ..fall()
        };
        land_battle_jump(
            &mut world,
            id,
            ObjectId(1),
            stompymux_rs::BattleMovementRules {
                fall: rules,
                ..stompymux_rs::BattleMovementRules::STANDARD
            },
        )
        .unwrap();
        let after =
            world.btech.constructed_units()[&ids[1]].sections()[&BattleSection::CenterTorso].armor;
        assert_eq!(before - after, if special { 4 } else { 6 });
        world.validate(&config).unwrap();
    }
}

#[tokio::test]
async fn jump_obstacle_falls_resolve_crowding_after_fall_damage() {
    let (_dir, config, mut world, ids) = fixture(&[0, 0, 0]).await;
    let id = ids[0];
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let chosen = (0..=255)
        .find(|value| {
            let mut dice = BattleDice::seeded([*value; 32]);
            let safe = dice.two_d6() >= 7;
            dice.d6();
            safe && dice.two_d6() == 7 && dice.die(3).unwrap() == 1
        })
        .unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][map.0.to_string()]["movement_modifier"] = 800.into();
    for index in 3..6 {
        state["maps"][map.0.to_string()]["terrain"][index] =
            serde_json::to_value(BattleHex::new(Terrain::Wall, 9)).unwrap();
    }
    for unit in &ids {
        state["constructed"][unit.0.to_string()]["position"]["y"] = 0.into();
        state["constructed"][unit.0.to_string()]["motion"]["point"] =
            serde_json::to_value(BattleHexCoordinate { x: 1, y: 0 }.center()).unwrap();
    }
    state["constructed"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([chosen; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 180, 2.0).unwrap();
    let rules = BattleFallRules {
        stacking: BattleStackingRules::STANDARD,
        ..fall()
    };
    let mut rejected = world.clone();
    rejected
        .objects
        .get_mut(&ids[1])
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let before = rejected.btech.clone();
    assert!(
        advance_battle_jumps(
            &mut rejected,
            stompymux_rs::BattleMovementRules {
                fall: rules,
                ..stompymux_rs::BattleMovementRules::STANDARD
            }
        )
        .is_err()
    );
    assert_eq!(rejected.btech, before);
    let mut expected = world.clone();
    let mut expected_notices = advance_battle_jumps(
        &mut expected,
        stompymux_rs::BattleMovementRules {
            fall: BattleFallRules {
                stacking: BattleStackingRules {
                    mode: 0,
                    ..rules.stacking
                },
                ..rules
            },
            ..stompymux_rs::BattleMovementRules::STANDARD
        },
    )
    .unwrap();
    let physical = BattleStackingInput {
        entry: BattleStackingEntry::Fall,
        mass: expected.btech.constructed_units()[&id]
            .mass()
            .unwrap()
            .total,
        jump_movement_points: 5,
    };
    expected_notices.extend(
        resolve_battle_stacking(&mut expected, id, physical, rules.stacking, fall()).unwrap(),
    );
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let events = advance_battle_jumps(
        &mut world,
        stompymux_rs::BattleMovementRules {
            fall: rules,
            ..stompymux_rs::BattleMovementRules::STANDARD
        },
    )
    .unwrap();
    assert_eq!(events, expected_notices);
    assert_eq!(world.btech, expected.btech);
    assert_eq!(
        events,
        advance_battle_jumps(
            &mut loaded,
            stompymux_rs::BattleMovementRules {
                fall: rules,
                ..stompymux_rs::BattleMovementRules::STANDARD
            }
        )
        .unwrap()
    );
    assert_eq!(world.btech, loaded.btech);
    assert!(
        events
            .iter()
            .any(|notice| notice.text.contains("land on another"))
    );
    world.validate(&config).unwrap();
}

/// Avoid unrelated pilot injuries and distribute a five-level fall over four intact sections.
fn thermal_fall_dice() -> BattleDice {
    (0u32..100_000)
        .find_map(|value| {
            let mut bytes = [0; 32];
            bytes[..4].copy_from_slice(&value.to_le_bytes());
            let original = BattleDice::seeded(bytes);
            let mut dice = original.clone();
            for _ in 0..3 {
                dice.d6();
            }
            if dice.two_d6() < 11 {
                return None;
            }
            dice.d6();
            let mut locations = Vec::new();
            for _ in 0..4 {
                locations.push(dice.two_d6());
                dice.two_d6(); // Each fall packet enters material resolution.
            }
            locations.sort();
            (locations == [5, 6, 8, 9] && dice.die(3).unwrap() == 1).then_some(original)
        })
        .unwrap()
}

#[tokio::test]
async fn airborne_shutdown_scales_fall_and_commits_crowding_before_power_down() {
    for mode in [0, 2] {
        let (_dir, config, mut world, ids) = landing_fixture(2).await;
        let id = ids[0];
        let mut state = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut state["constructed"][id.0.to_string()];
        unit["dice"] = serde_json::to_value(thermal_fall_dice()).unwrap();
        unit["heat"] = serde_json::json!({"stored":24.0,"excess":14.0});
        unit["overheat_clock"] = serde_json::json!({"elapsed":30,"phase":0,"injury_due":false});
        world.btech = serde_json::from_value(state).unwrap();
        let rules = BattleOverheatRules {
            vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
            hit: fall().hit,
            extended_piloting: true,
            stagger: BattleStaggerMode::Retain,
            stacking: BattleStackingRules {
                mode,
                damage_percent: 50,
                ..BattleStackingRules::STANDARD
            },
        };
        if mode == 2 {
            let mut rejected = world.clone();
            rejected
                .objects
                .get_mut(&ids[1])
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            let before = rejected.btech.clone();
            assert!(advance_battle_overheat(&mut rejected, rules).is_err());
            assert_eq!(rejected.btech, before);
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        let reports = advance_battle_overheat(&mut world, rules).unwrap();
        assert_eq!(
            reports,
            advance_battle_overheat(&mut loaded, rules).unwrap()
        );
        assert_eq!(world.btech, loaded.btech);
        assert_eq!(reports[0].fall.as_ref().unwrap().damage, 20);
        assert_eq!(
            reports[0]
                .notices
                .iter()
                .any(|notice| notice.text.contains("land on another")),
            mode == 2
        );
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.power(), BattlePower::Off);
        assert!(unit.pilot().is_none());
        assert!(unit.flight().is_none());
        assert_eq!(unit.jump_stabilization(), 0);
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

#[tokio::test]
async fn airborne_critical_falls_apply_configured_collisions_inside_the_impact() {
    for jet in [false, true] {
        for mode in [0, 2] {
            let (_dir, config, mut world, ids) = landing_fixture(2).await;
            let id = ids[0];
            let location = if jet {
                let jets: Vec<_> = world.btech.constructed_units()[&id]
                    .loadout()
                    .unwrap()
                    .systems
                    .into_iter()
                    .filter(|part| part.system == BattleSystem::JumpJet)
                    .map(|part| part.location)
                    .collect();
                for &location in &jets[..jets.len() - 1] {
                    destroy_battle_critical(&mut world, id, location).unwrap();
                }
                *jets.last().unwrap()
            } else {
                CriticalLocation {
                    section: BattleSection::CenterTorso,
                    slot: 3,
                }
            };
            let candidates =
                world.btech.constructed_units()[&id].critical_candidates(location.section);
            let selected = candidates
                .iter()
                .position(|&slot| slot == location)
                .unwrap() as u16
                + 1;
            let chosen = (0..=255)
                .find(|value| {
                    let mut dice = BattleDice::seeded([*value; 32]);
                    dice.two_d6(); // Material entry precedes the selected critical.
                    matches!(dice.two_d6(), 8 | 9)
                        && dice.die(candidates.len() as u16).unwrap() == selected
                        && (jet || dice.two_d6() < 9)
                })
                .unwrap();
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["constructed"][id.0.to_string()]["dice"] =
                serde_json::to_value(BattleDice::seeded([chosen; 32])).unwrap();
            world.btech = serde_json::from_value(state).unwrap();
            let hit = BattleHit {
                section: location.section,
                rear_armor: false,
                through_armor_critical: true,
                crew_stun: false,
            };
            let rules = BattleFallRules {
                stacking: BattleStackingRules {
                    mode,
                    ..BattleStackingRules::STANDARD
                },
                ..fall()
            };
            if !jet && mode == 2 {
                let mut rejected = world.clone();
                for target in &ids[1..] {
                    rejected
                        .objects
                        .get_mut(target)
                        .unwrap()
                        .flags
                        .insert(Flag::InCharacter);
                }
                let before = rejected.btech.clone();
                assert!(resolve_battle_tactical_impact(&mut rejected, id, hit, 1, rules).is_err());
                assert_eq!(rejected.btech, before);
            }
            let neighbors: Vec<_> = ids[1..]
                .iter()
                .map(|target| world.btech.constructed_units()[target].clone())
                .collect();
            persistence::save(&config.database(), &world).await.unwrap();
            let mut loaded = persistence::load(&config.database()).await.unwrap();
            let report = resolve_battle_tactical_impact(&mut world, id, hit, 1, rules).unwrap();
            assert_eq!(
                report,
                resolve_battle_tactical_impact(&mut loaded, id, hit, 1, rules).unwrap()
            );
            assert_eq!(world.btech, loaded.btech);
            assert!(report.balance[0].fall.is_some());
            assert!(world.btech.constructed_units()[&id].flight().is_none());
            assert_eq!(
                report
                    .notices
                    .iter()
                    .any(|notice| notice.text.contains("land on another")),
                !jet && mode == 2
            );
            if jet || mode == 0 {
                for (target, old) in ids[1..].iter().zip(neighbors) {
                    assert_eq!(world.btech.constructed_units()[target], old);
                }
            }
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
        }
    }
}

#[tokio::test]
async fn ground_shutdown_speed_boundary_and_facing_match_native_lua() {
    for speed in [-53.75, 0.0, 10.75, 10.76, 21.5] {
        let (_dir, config, mut world, ids) = fixture(&[0, 0, 0]).await;
        let id = ids[0];
        let chosen = (0..=255)
            .find(|value| {
                let mut dice = BattleDice::seeded([*value; 32]);
                let protected = dice.two_d6() >= 7;
                dice.d6();
                protected && dice.two_d6() == 7 && dice.die(3).unwrap() == 1
            })
            .unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut state["constructed"][id.0.to_string()];
        unit["motion"]["speed"] = speed.into();
        unit["motion"]["desired_speed"] = speed.into();
        unit["facing"] = serde_json::to_value(BattleFacing {
            torso: BattleTorso::Left,
            arms_flipped: true,
        })
        .unwrap();
        unit["dice"] = serde_json::to_value(BattleDice::seeded([chosen; 32])).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        world.validate(&config).unwrap();
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.stop({},1); error('abort moving shutdown')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        let text = support::run_text(&native, &config, ObjectId(1), 1, "shutdown");
        lua.eval_callback::<()>(&format!("btech.unit.stop({},1)", id.0))
            .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        assert!(text.contains("All systems shut down"), "{text}");
        let saved = native.world().clone();
        let unit = &saved.btech.constructed_units()[&id];
        assert_eq!(unit.power(), BattlePower::Off);
        assert_eq!(
            unit.posture(),
            if speed > 10.75 {
                BattlePosture::Prone
            } else {
                BattlePosture::Standing
            }
        );
        assert_eq!(unit.facing().torso, BattleTorso::Center);
        assert_eq!(unit.facing().arms_flipped, speed <= 10.75);
        assert!(unit.pilot().is_none());
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        if speed <= 10.75 {
            assert_eq!(
                serde_json::to_value(unit).unwrap()["dice"],
                serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"]
            );
        }
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}

#[tokio::test]
async fn moving_shutdown_neighbor_failure_rolls_back_power_fall_and_facing() {
    let (_dir, config, mut world, ids) = fixture(&[0, 0, 0]).await;
    let id = ids[0];
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["motion"]["speed"] = 21.5.into();
    state["constructed"][id.0.to_string()]["motion"]["desired_speed"] = 21.5.into();
    state["constructed"][id.0.to_string()]["facing"]["torso"] = "right".into();
    let chosen = (0..=255)
        .find(|value| {
            let mut dice = BattleDice::seeded([*value; 32]);
            let protected = dice.two_d6() >= 7;
            dice.d6();
            protected && dice.two_d6() == 7
        })
        .unwrap();
    state["constructed"][id.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([chosen; 32])).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    for target in &ids[1..] {
        world
            .objects
            .get_mut(target)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
    }
    let before = world.btech.clone();
    assert!(stop_battle_unit(&mut world, id, ObjectId(1), fall()).is_err());
    assert_eq!(world.btech, before);
    let disabled = BattleFallRules {
        stacking: BattleStackingRules {
            mode: 0,
            ..BattleStackingRules::STANDARD
        },
        ..fall()
    };
    stop_battle_unit(&mut world, id, ObjectId(1), disabled).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id].power(),
        BattlePower::Off
    );
    for target in &ids[1..] {
        assert_eq!(
            world.btech.constructed_units()[target],
            before.constructed_units()[target]
        );
    }
    world.validate(&config).unwrap();
}

/// Collision observers independently identify participants and replay damage/avoidance feedback.
#[tokio::test]
async fn collision_observers_filter_each_participant_and_replay_all_entry_modes() {
    let (_dir, config, mut base, ids) = fixture(&[0, 0, 0, 0, 1]).await;
    let actor = ids[0];
    let target = ids[1];
    let observer = ids[4];
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][observer.0.to_string()]["position"]["x"] = 2.into();
    state["constructed"][observer.0.to_string()]["position"]["y"] = 2.into();
    state["constructed"][observer.0.to_string()]["motion"]["point"] =
        serde_json::to_value(BattleHexCoordinate { x: 2, y: 2 }.center()).unwrap();
    base.btech = serde_json::from_value(state).unwrap();
    base.validate(&config).unwrap();
    for _ in 0..32 {
        refresh_battle_contacts(&mut base, &[observer]).unwrap();
        if base.btech.constructed_units()[&observer]
            .contacts()
            .contains_key(&actor)
            && base.btech.constructed_units()[&observer]
                .contacts()
                .contains_key(&target)
        {
            break;
        }
    }
    assert!(
        visible_battle_contact(&base, observer, actor)
            .unwrap()
            .is_some()
    );
    assert!(
        visible_battle_contact(&base, observer, target)
            .unwrap()
            .is_some()
    );
    // Keep this routing matrix on armor-only torso hits; random target criticals have their own notice tests.
    let target_seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            let mut locations = [dice.d6(), dice.d6(), dice.d6()];
            locations.sort();
            locations == [2, 3, 4]
        })
        .unwrap();
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][target.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([target_seed; 32])).unwrap();
    base.btech = serde_json::from_value(state).unwrap();
    for entry in [
        BattleStackingEntry::Ground,
        BattleStackingEntry::Jump,
        BattleStackingEntry::Fall,
    ] {
        for (mode, success) in [(1, false), (1, true), (2, true)] {
            for visible in 0..4 {
                for powered in [false, true] {
                    let mut world = base.clone();
                    let mut state = serde_json::to_value(&world.btech).unwrap();
                    let mover = &mut state["constructed"][actor.0.to_string()];
                    mover["motion"]["speed"] = 21.5.into();
                    mover["motion"]["desired_speed"] = 21.5.into();
                    let viewer = &mut state["constructed"][observer.0.to_string()];
                    for (bit, id) in [(1, actor), (2, target)] {
                        if visible & bit == 0 {
                            viewer["contacts"]
                                .as_object_mut()
                                .unwrap()
                                .remove(&id.0.to_string());
                        }
                    }
                    if !powered {
                        viewer["power"] = serde_json::json!({"state":"off"});
                    }
                    world.btech = serde_json::from_value(state).unwrap();
                    let count = if entry == BattleStackingEntry::Jump {
                        3
                    } else {
                        4
                    };
                    seed(
                        &mut world,
                        actor,
                        count,
                        1,
                        Some(if success { 12 } else { 2 }),
                    );
                    let observer_before = world.btech.constructed_units()[&observer].clone();
                    let rules = BattleStackingRules {
                        mode,
                        ..BattleStackingRules::STANDARD
                    };
                    let original = world.btech.clone();
                    let mut rejected = world.clone();
                    rejected
                        .objects
                        .get_mut(&target)
                        .unwrap()
                        .flags
                        .insert(Flag::InCharacter);
                    if mode == 2 {
                        assert!(
                            resolve_battle_stacking(
                                &mut rejected,
                                actor,
                                input(entry),
                                rules,
                                fall()
                            )
                            .is_err()
                        );
                        assert_eq!(rejected.btech, original);
                    }
                    persistence::save(&config.database(), &world).await.unwrap();
                    let mut restored = persistence::load(&config.database()).await.unwrap();
                    let notices =
                        resolve_battle_stacking(&mut world, actor, input(entry), rules, fall())
                            .unwrap();
                    assert_eq!(
                        resolve_battle_stacking(&mut restored, actor, input(entry), rules, fall())
                            .unwrap(),
                        notices
                    );
                    assert_eq!(world.btech, restored.btech);
                    assert_eq!(world.btech.constructed_units()[&observer], observer_before);
                    assert_eq!(notices[0].unit, actor);
                    assert_eq!(notices[1].unit, target);
                    let observed: Vec<_> = notices
                        .iter()
                        .filter(|notice| notice.unit == observer)
                        .map(|notice| notice.text.clone())
                        .collect();
                    let mut expected = Vec::new();
                    if powered && visible != 0 {
                        let identity = |id| format!("#{} Jenner", id);
                        let actor_name = if visible & 1 != 0 {
                            identity(actor.0)
                        } else {
                            "Someone".to_owned()
                        };
                        let target_name = if visible & 2 != 0 {
                            identity(target.0)
                        } else {
                            "someone".to_owned()
                        };
                        let action = match (entry == BattleStackingEntry::Ground, mode) {
                            (true, 2) => "bumps into",
                            (true, _) => "nearly bumps into",
                            (false, 2) => "lands on",
                            (false, _) => "nearly lands on",
                        };
                        expected.push(format!("{actor_name} {action} {target_name}!"));
                        if mode == 1
                            && !success
                            && entry != BattleStackingEntry::Ground
                            && visible & 1 != 0
                        {
                            expected.push(format!("{} falls down!", identity(actor.0)));
                        }
                    }
                    assert_eq!(
                        observed, expected,
                        "{entry:?}, mode {mode}, success {success}, mask {visible}, powered {powered}"
                    );
                }
            }
        }
    }
}

/// Character collision hits apply health at impact and roll back both units on failed evacuation.
#[tokio::test]
async fn character_collision_action_replays_injury_and_evacuation() {
    for fatal in [false, true] {
        let (_dir, config, mut world, ids) = fixture(&[0, 0, 0, 0]).await;
        let target = ids[1];
        let pilot = ObjectId(2);
        seed(&mut world, ids[0], 3, 1, None);
        world
            .objects
            .get_mut(&target)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        world.objects.get_mut(&pilot).unwrap().location = Some(target);
        assign_battle_pilot(&mut world, target, pilot).unwrap();
        set_battle_character(
            &mut world,
            pilot,
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: if fatal { 50 } else { 0 },
                lethal: if fatal { 40 } else { 0 },
            },
        )
        .unwrap();
        let chosen = (0..=255)
            .find(|byte| BattleDice::seeded([*byte; 32]).d6() == 6)
            .unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][target.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([chosen; 32])).unwrap();
        world.btech = serde_json::from_value(state).unwrap();
        let rules = BattleStackingRules {
            damage_percent: 1,
            ..BattleStackingRules::STANDARD
        };
        let baseline = world.clone();
        assert!(
            resolve_battle_stacking(
                &mut world,
                ids[0],
                input(BattleStackingEntry::Jump),
                rules,
                fall()
            )
            .is_err()
        );
        assert_eq!(world.btech, baseline.btech);
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        if fatal {
            scripts.world_mut().objects.remove(&afterlife);
            assert!(
                resolve_battle_stacking_action(
                    &scripts,
                    &config,
                    ids[0],
                    input(BattleStackingEntry::Jump),
                    rules,
                    fall()
                )
                .is_err()
            );
            assert_eq!(scripts.world().btech, baseline.btech);
            assert_eq!(scripts.world().objects[&pilot].location, Some(target));
            assert!(scripts.drain_outbox().is_empty());
            *scripts.world_mut() = baseline.clone();
        }
        persistence::save(&config.database(), &baseline)
            .await
            .unwrap();
        let replay = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(
                persistence::load(&config.database()).await.unwrap(),
            )),
        )
        .unwrap();
        let notices = resolve_battle_stacking_action(
            &scripts,
            &config,
            ids[0],
            input(BattleStackingEntry::Jump),
            rules,
            fall(),
        )
        .unwrap();
        assert_eq!(
            notices,
            resolve_battle_stacking_action(
                &replay,
                &config,
                ids[0],
                input(BattleStackingEntry::Jump),
                rules,
                fall()
            )
            .unwrap()
        );
        assert_eq!(scripts.world().btech, replay.world().btech);
        let candidate = scripts.world().clone();
        let status = candidate.btech.constructed_units()[&target]
            .character_pilot_status()
            .unwrap();
        assert_eq!(status.killed, fatal);
        assert_eq!(status.injuries, if fatal { 0 } else { 1 });
        assert_eq!(
            candidate.objects[&pilot].location,
            Some(if fatal { afterlife } else { target })
        );
        assert!(
            candidate.btech.constructed_units()[&target].sections()[&BattleSection::Head].internal
                > 0
        );
        assert_ne!(
            candidate.btech.constructed_units()[&ids[0]].sections(),
            baseline.btech.constructed_units()[&ids[0]].sections()
        );
        candidate.validate(&config).unwrap();
        persistence::save(&config.database(), &candidate)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            candidate.btech
        );
    }
}

/// Avoidance collisions pass character falls through the same casualty publication boundary.
#[tokio::test]
async fn character_collision_avoidance_falls_and_replays() {
    let (_dir, config, mut world, ids) = fixture(&[0, 0, 0, 0]).await;
    let mover = ids[0];
    world
        .objects
        .get_mut(&mover)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    set_battle_character(
        &mut world,
        ObjectId(1),
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
    seed(&mut world, mover, 3, 1, Some(2));
    let rules = BattleStackingRules {
        mode: 1,
        ..BattleStackingRules::STANDARD
    };
    let baseline = world.clone();
    assert!(
        resolve_battle_stacking(
            &mut world,
            mover,
            input(BattleStackingEntry::Jump),
            rules,
            fall()
        )
        .is_err()
    );
    assert_eq!(world.btech, baseline.btech);
    persistence::save(&config.database(), &baseline)
        .await
        .unwrap();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let replay = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(
            persistence::load(&config.database()).await.unwrap(),
        )),
    )
    .unwrap();
    let notices = resolve_battle_stacking_action(
        &scripts,
        &config,
        mover,
        input(BattleStackingEntry::Jump),
        rules,
        fall(),
    )
    .unwrap();
    assert_eq!(
        notices,
        resolve_battle_stacking_action(
            &replay,
            &config,
            mover,
            input(BattleStackingEntry::Jump),
            rules,
            fall()
        )
        .unwrap()
    );
    assert_eq!(scripts.world().btech, replay.world().btech);
    assert_eq!(
        scripts.world().btech.constructed_units()[&mover].posture(),
        BattlePosture::Prone
    );
    assert_ne!(
        scripts.world().btech.constructed_units()[&mover].sections(),
        baseline.btech.constructed_units()[&mover].sections()
    );
    scripts.world().validate(&config).unwrap();
}

/// Avoidance XP applies to ground, jump and fall crowding and rolls back alongside motion and dice.
#[tokio::test]
async fn character_crowding_experience_and_delivery_rollback() {
    for extended in [false, true] {
        for entry in [
            BattleStackingEntry::Ground,
            BattleStackingEntry::Jump,
            BattleStackingEntry::Fall,
        ] {
            let (_dir, config, mut world, ids) = fixture(if entry == BattleStackingEntry::Jump {
                &[0, 0, 0, 0]
            } else {
                &[0, 0, 0]
            })
            .await;
            let id = ids[0];
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            world
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .insert(Flag::Connected);
            set_battle_character(
                &mut world,
                ObjectId(1),
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
            let skill = if extended {
                "Piloting-Biped"
            } else {
                "Piloting-Battlemech"
            };
            set_battle_character_value(
                &mut world,
                ObjectId(1),
                skill,
                BattleCharacterValue {
                    value: 3,
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["constructed"][id.0.to_string()]["motion"]["speed"] = 21.5.into();
            state["constructed"][id.0.to_string()]["motion"]["desired_speed"] = 21.5.into();
            world.btech = serde_json::from_value(state).unwrap();
            seed(&mut world, id, 3, 1, Some(12));
            let mut channel = Channel::new("MechPilotXP".into());
            channel.users.push(communication::Membership {
                who: ObjectId(1),
                listening: true,
            });
            world.channels.insert("MechPilotXP".into(), channel);
            // A connected passenger hears the collision warning, but not the pilot's roll.
            world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
            world
                .objects
                .get_mut(&ObjectId(2))
                .unwrap()
                .flags
                .insert(Flag::Connected);
            let before = world.clone();
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let rules = BattleStackingRules {
                mode: 1,
                ..BattleStackingRules::STANDARD
            };
            let fall = BattleFallRules {
                extended_piloting: extended,
                ..fall()
            };
            scripts
                .world_mut()
                .channels
                .get_mut("MechPilotXP")
                .unwrap()
                .messages = i64::MAX;
            assert!(
                resolve_battle_stacking_action(&scripts, &config, id, input(entry), rules, fall)
                    .is_err()
            );
            assert_eq!(scripts.world().btech, before.btech);
            assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
            assert!(scripts.drain_outbox().is_empty());
            *scripts.world_mut() = before.clone();
            let _ =
                resolve_battle_stacking_action(&scripts, &config, id, input(entry), rules, fall)
                    .unwrap();
            let output = scripts.drain_outbox();
            let pilot: Vec<_> = output
                .iter()
                .filter(|(recipient, _)| *recipient == ObjectId(1))
                .map(|(_, message)| message.source())
                .collect();
            let roll_index = pilot
                .iter()
                .position(|message| *message == "You make a piloting skill roll!")
                .expect("the actual avoidance roll must be displayed");
            assert!(roll_index > 0, "collision warning precedes the roll");
            assert!(pilot[roll_index + 1].starts_with("Modified Pilot Skill: BTH "));
            assert!(pilot[roll_index + 1].ends_with("\tRoll: 12"));
            assert_eq!(
                output
                    .iter()
                    .filter(|(_, message)| message.source() == "You make a piloting skill roll!")
                    .count(),
                1
            );
            assert!(
                !output
                    .iter()
                    .any(|(recipient, message)| *recipient != ObjectId(1)
                        && message.source().starts_with("Modified Pilot Skill:"))
            );
            assert!(
                output
                    .iter()
                    .any(|(recipient, message)| *recipient == ObjectId(2)
                        && message.source().contains("nearly"))
            );
            let candidate = scripts.world().clone();
            let amount = if entry == BattleStackingEntry::Ground {
                1
            } else {
                3
            };
            assert_eq!(
                candidate.btech.character_values()[&ObjectId(1)][skill].experience_balance(),
                amount
            );
            assert_eq!(
                candidate.btech.constructed_units()[&id].posture(),
                BattlePosture::Standing
            );
            assert_eq!(
                candidate.btech.constructed_units()[&ids[1]],
                before.btech.constructed_units()[&ids[1]]
            );
            assert_eq!(candidate.channels["MechPilotXP"].history.len(), 1);
            assert_eq!(
                text::plain_with(
                    scripts.palette(),
                    &candidate.channels["MechPilotXP"].history[0].message
                ),
                format!("[MechPilotXP] GOD gained {amount} {skill} XP")
            );
            persistence::save(&config.database(), &candidate)
                .await
                .unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                candidate.btech
            );
            *scripts.world_mut() = before;
            scripts
                .world_mut()
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .remove(Flag::Connected);
            let _ =
                resolve_battle_stacking_action(&scripts, &config, id, input(entry), rules, fall)
                    .unwrap();
            assert_eq!(
                scripts.world().btech.character_values()[&ObjectId(1)][skill].experience_balance(),
                0
            );
            assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
        }
    }
}

/// Domino target packets warn a held mover; reciprocal self damage stays silent and deterministic.
#[tokio::test]
async fn weapons_hold_collision_feedback_preserves_damage_and_replay() {
    for held_unit in [0, 1] {
        let (_dir, config, mut base, ids) = fixture(&[0, 0, 0, 0]).await;
        seed(&mut base, ids[0], 3, 1, None);
        let mut ordinary = base.clone();
        let expected = resolve_battle_stacking(
            &mut ordinary,
            ids[0],
            input(BattleStackingEntry::Jump),
            BattleStackingRules::STANDARD,
            fall(),
        )
        .unwrap();
        set_battle_weapons_hold(&mut base, ids[held_unit], true).unwrap();
        persistence::save(&config.database(), &base).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        let events = resolve_battle_stacking(
            &mut base,
            ids[0],
            input(BattleStackingEntry::Jump),
            BattleStackingRules::STANDARD,
            fall(),
        )
        .unwrap();
        let warnings: Vec<_> = events
            .iter()
            .filter(|n| n.text == "You are currently in weapons hold!")
            .collect();
        assert_eq!(!warnings.is_empty(), held_unit == 0);
        assert!(warnings.iter().all(|n| n.unit == ids[0]));
        assert_eq!(
            events
                .iter()
                .filter(|n| n.text != "You are currently in weapons hold!")
                .cloned()
                .collect::<Vec<_>>(),
            expected
        );
        assert_eq!(
            resolve_battle_stacking(
                &mut replay,
                ids[0],
                input(BattleStackingEntry::Jump),
                BattleStackingRules::STANDARD,
                fall()
            )
            .unwrap(),
            events
        );
        assert_eq!(base.btech, replay.btech);
        set_battle_weapons_hold(&mut base, ids[held_unit], false).unwrap();
        assert_eq!(base.btech, ordinary.btech);
        base.validate(&config).unwrap();
    }
}

/// Nested collision criticals retain the damaged unit's private balance feedback.
#[tokio::test]
async fn collision_damage_preserves_target_balance_feedback() {
    let (_dir, config, mut base, ids) = fixture(&[0, 0, 0]).await;
    let mover = ids[0];
    let target = ids[1];
    base.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
    assign_battle_pilot(&mut base, target, ObjectId(2)).unwrap();
    for pilot in [ObjectId(1), ObjectId(2)] {
        base.objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .insert(Flag::Connected);
    }
    for section in BattleSection::ALL {
        let armor = base.btech.constructed_units()[&target].sections()[&section].armor;
        apply_damage_phase(
            &mut base,
            target,
            section,
            armor,
            BattleDamagePhase::Armor { rear: false },
        )
        .unwrap();
    }
    let mut encoded = serde_json::to_value(&base.btech).unwrap();
    encoded["constructed"][mover.0.to_string()]["motion"]["speed"] = 43.0.into();
    encoded["constructed"][mover.0.to_string()]["motion"]["desired_speed"] = 43.0.into();
    base.btech = serde_json::from_value(encoded).unwrap();
    seed(&mut base, mover, 3, 1, None);
    for byte in 0..=255 {
        let mut world = base.clone();
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["constructed"][target.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([byte; 32])).unwrap();
        world.btech = serde_json::from_value(encoded).unwrap();
        let before = world.clone();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        resolve_battle_stacking_action(
            &scripts,
            &config,
            mover,
            input(BattleStackingEntry::Ground),
            BattleStackingRules::STANDARD,
            fall(),
        )
        .unwrap();
        let output = scripts.drain_outbox();
        let pilot: Vec<_> = output
            .iter()
            .filter(|(who, _)| *who == ObjectId(2))
            .map(|(_, message)| message.source())
            .collect();
        let Some(index) = pilot
            .iter()
            .position(|message| *message == "You make a piloting skill roll!")
        else {
            continue;
        };
        assert!(index > 0);
        assert!(pilot[index + 1].starts_with("Modified Pilot Skill: BTH "));
        assert!(!output.iter().any(|(who, message)| *who != ObjectId(2)
            && message.source().starts_with("Modified Pilot Skill:")));
        let state = scripts.world().btech.clone();
        *scripts.world_mut() = before;
        resolve_battle_stacking_action(
            &scripts,
            &config,
            mover,
            input(BattleStackingEntry::Ground),
            BattleStackingRules::STANDARD,
            fall(),
        )
        .unwrap();
        assert_eq!(scripts.world().btech, state);
        let replay = scripts.drain_outbox();
        assert_eq!(
            replay
                .iter()
                .map(|(who, message)| (*who, message.source()))
                .collect::<Vec<_>>(),
            output
                .iter()
                .map(|(who, message)| (*who, message.source()))
                .collect::<Vec<_>>()
        );
        return;
    }
    panic!("collision seed matrix did not exercise target balance feedback");
}

/// A host salvo retains secondary character injuries after an airborne critical fall.
#[tokio::test]
async fn airborne_critical_collision_publishes_secondary_character_effects() {
    let (_dir, config, mut base, ids) = landing_fixture(2).await;
    let airborne = ids[0];
    let neighbor = ids[1];
    let pilot = ObjectId(2);
    base.objects.get_mut(&pilot).unwrap().location = Some(neighbor);
    base.objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    base.objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    base.objects
        .get_mut(&neighbor)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    assign_battle_pilot(&mut base, neighbor, pilot).unwrap();
    set_battle_character(
        &mut base,
        pilot,
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
    destroy_battle_critical(
        &mut base,
        airborne,
        CriticalLocation {
            section: BattleSection::CenterTorso,
            slot: 3,
        },
    )
    .unwrap();
    let armor =
        base.btech.constructed_units()[&airborne].sections()[&BattleSection::CenterTorso].armor;
    apply_damage_phase(
        &mut base,
        airborne,
        BattleSection::CenterTorso,
        armor,
        BattleDamagePhase::Armor { rear: false },
    )
    .unwrap();
    let head_seed = (0..=255)
        .find(|byte| BattleDice::seeded([*byte; 32]).d6() == 6)
        .unwrap();
    let mut encoded = serde_json::to_value(&base.btech).unwrap();
    encoded["constructed"][neighbor.0.to_string()]["dice"] =
        serde_json::to_value(BattleDice::seeded([head_seed; 32])).unwrap();
    base.btech = serde_json::from_value(encoded).unwrap();
    for byte in 0..=255 {
        let mut world = base.clone();
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        encoded["constructed"][airborne.0.to_string()]["dice"] =
            serde_json::to_value(BattleDice::seeded([byte; 32])).unwrap();
        world.btech = serde_json::from_value(encoded).unwrap();
        let before = world.clone();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let report = resolve_battle_salvo_action(
            &scripts,
            &config,
            airborne,
            BattleWeapon::MediumLaser,
            BattleHitArc::Front,
            fall(),
        )
        .unwrap();
        let injuries: Vec<_> = report
            .groups
            .iter()
            .flat_map(|group| &group.balance)
            .flat_map(|balance| &balance.collision_impacts)
            .flat_map(|impact| &impact.impact.character_injuries)
            .filter(|injury| injury.player == pilot && injury.consciousness.is_some())
            .collect();
        if injuries.is_empty() {
            continue;
        }
        assert!(
            scripts.world().btech.constructed_units()[&airborne]
                .flight()
                .is_none()
        );
        let output = scripts.drain_outbox();
        assert_eq!(
            output
                .iter()
                .filter(|(who, message)| *who == pilot
                    && message.source() == "You attempt to keep consciousness!")
                .count(),
            injuries.len()
        );
        assert!(scripts.world().btech.characters()[&pilot].bruise > 0);
        let state = scripts.world().btech.clone();
        *scripts.world_mut() = before.clone();
        let replay = resolve_battle_salvo_action(
            &scripts,
            &config,
            airborne,
            BattleWeapon::MediumLaser,
            BattleHitArc::Front,
            fall(),
        )
        .unwrap();
        assert_eq!(replay, report);
        assert_eq!(scripts.world().btech, state);
        let replay_output = scripts.drain_outbox();
        assert_eq!(
            output
                .iter()
                .map(|(who, message)| (*who, message.source()))
                .collect::<Vec<_>>(),
            replay_output
                .iter()
                .map(|(who, message)| (*who, message.source()))
                .collect::<Vec<_>>()
        );
        persistence::save(&config.database(), &before)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            before.btech
        );
        // A failed secondary evacuation restores the original hit, fall, collision and output.
        *scripts.world_mut() = before.clone();
        set_battle_character(
            &mut scripts.world_mut(),
            pilot,
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: 50,
                lethal: 40,
            },
        )
        .unwrap();
        let fatal_before = scripts.world().clone();
        scripts
            .world_mut()
            .objects
            .remove(&ObjectId(config.battletech.afterlife_dbref));
        assert!(
            resolve_battle_salvo_action(
                &scripts,
                &config,
                airborne,
                BattleWeapon::MediumLaser,
                BattleHitArc::Front,
                fall()
            )
            .is_err()
        );
        assert_eq!(scripts.world().btech, fatal_before.btech);
        assert_eq!(scripts.world().objects[&pilot].location, Some(neighbor));
        assert!(scripts.drain_outbox().is_empty());
        *scripts.world_mut() = fatal_before;
        resolve_battle_salvo_action(
            &scripts,
            &config,
            airborne,
            BattleWeapon::MediumLaser,
            BattleHitArc::Front,
            fall(),
        )
        .unwrap();
        assert_eq!(
            scripts.world().objects[&pilot].location,
            Some(ObjectId(config.battletech.afterlife_dbref))
        );
        return;
    }
    panic!("seed matrix did not exercise secondary character collision injury");
}
