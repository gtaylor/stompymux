//! Restored action-time staggering participates in landing without replacing rolling damage history.
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Advance to settlement while verifying every committed flight sample against a saved replay.
fn land(
    world: &mut World,
    replay: &mut World,
    unit: ObjectId,
    rules: BattleMovementRules,
) -> Vec<String> {
    let mut messages = Vec::new();
    for _ in 0..60 {
        if world.btech.constructed_units()[&unit].flight().is_none() {
            break;
        }
        let notices = advance_battle_jumps(world, rules).unwrap();
        assert_eq!(notices, advance_battle_jumps(replay, rules).unwrap());
        assert_eq!(world.btech, replay.btech);
        messages.extend(
            notices
                .into_iter()
                .filter(|n| n.unit == unit)
                .map(|n| n.text),
        );
    }
    assert!(world.btech.constructed_units()[&unit].flight().is_none());
    messages
}

/// Every weight class uses its action modifier, including the quad anatomy and zero-scalar bypass.
async fn restored_landing_scalar_matrix(template: &str, weight_modifier: i32) {
    let (_dir, config, base, unit, _, _) =
        firing::fixture_with_target(template, None, template).await;
    let mut probe = base.clone();
    let base_target = roll_battle_piloting(&mut probe, unit, 0, false)
        .unwrap()
        .target;
    for scalar in [0, 19, 20, 40] {
        for roll in [2, 5, 12] {
            let mut world = base.clone();
            launch_battle_jump(&mut world, unit, ObjectId(1), 0, 2.0).unwrap();
            let seed = (0..=255)
                .find(|&seed| BattleDice::seeded([seed; 32]).two_d6() == roll)
                .unwrap();
            firing::edit(&mut world, unit, |s| {
                s["stagger"]["action_damage"] = scalar.into();
                s["stagger"]["hits"] =
                    serde_json::json!([{"damage":60,"remaining":60,"counted":false}]);
                s["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
            });
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            let messages = land(&mut world, &mut replay, unit, BattleMovementRules::STANDARD);
            let checked = scalar >= 20;
            let failed = checked && i32::from(roll) < base_target + scalar / 20 + weight_modifier;
            assert_eq!(
                messages
                    .iter()
                    .any(|m| m == "The damage you've taken makes the landing a bit harder..."),
                checked
            );
            assert_eq!(
                messages
                    .iter()
                    .any(|m| m == "... something you apparently can't handle!"),
                failed
            );
            let mech = &world.btech.constructed_units()[&unit];
            assert_eq!(mech.posture() == BattlePosture::Prone, failed);
            assert_eq!(mech.jump_stabilization(), 12);
            if !failed {
                let mut expected = BattleDice::seeded([seed; 32]);
                if checked {
                    expected.two_d6();
                }
                let saved = serde_json::to_value(&world.btech).unwrap();
                assert_eq!(
                    saved["constructed"][unit.0.to_string()]["dice"],
                    serde_json::to_value(expected).unwrap()
                );
            }
            if checked {
                let finish = messages
                    .iter()
                    .position(|m| m == "You finish your jump.")
                    .unwrap();
                let warning = messages
                    .iter()
                    .position(|m| m == "The damage you've taken makes the landing a bit harder...")
                    .unwrap();
                assert!(finish < warning);
            }
        }
    }
}

#[tokio::test]
async fn restored_landing_scalar_uses_weight_class_and_preserves_replay_jr7d() {
    restored_landing_scalar_matrix(include_str!("../game/mechs/JR7-D.toml"), 1).await;
}

#[tokio::test]
async fn restored_landing_scalar_uses_weight_class_and_preserves_replay_spider() {
    restored_landing_scalar_matrix(include_str!("../game/mechs/StalkingSpider-1.toml"), 0).await;
}

#[tokio::test]
async fn restored_landing_scalar_uses_weight_class_and_preserves_replay_tdr() {
    restored_landing_scalar_matrix(include_str!("../game/mechs/TDR-5SE.toml"), -1).await;
}

#[tokio::test]
async fn restored_landing_scalar_uses_weight_class_and_preserves_replay_vtr() {
    restored_landing_scalar_matrix(include_str!("../game/mechs/VTR-9B.toml"), -2).await;
}

/// Failure stops before the gyro check; success continues to that independent check.
#[tokio::test]
async fn stagger_landing_roll_precedes_damaged_gyro_roll() {
    let template = include_str!("../game/mechs/JR7-D.toml");
    let (_dir, _config, base, unit, _, _) =
        firing::fixture_with_target(template, None, template).await;
    for stagger_passes in [false, true] {
        let mut world = base.clone();
        launch_battle_jump(&mut world, unit, ObjectId(1), 0, 2.0).unwrap();
        let gyro = world.btech.constructed_units()[&unit]
            .loadout()
            .unwrap()
            .systems
            .iter()
            .find(|p| p.system == BattleSystem::Gyro)
            .unwrap()
            .location;
        destroy_battle_critical(&mut world, unit, gyro).unwrap();
        let seed = (0..=255)
            .find(|&seed| {
                let mut dice = BattleDice::seeded([seed; 32]);
                dice.two_d6() == if stagger_passes { 12 } else { 2 }
                    && (!stagger_passes || dice.two_d6() <= 4)
            })
            .unwrap();
        firing::edit(&mut world, unit, |s| {
            s["stagger"]["action_damage"] = 20.into();
            s["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        });
        let mut replay = world.clone();
        let messages = land(&mut world, &mut replay, unit, BattleMovementRules::STANDARD);
        assert_eq!(
            messages
                .iter()
                .any(|m| m == "Your damaged gyro makes it harder to land"),
            stagger_passes
        );
        assert_eq!(
            messages
                .iter()
                .any(|m| m == "Your damaged gyro has caused you to fall upon landing!"),
            stagger_passes
        );
        assert_eq!(
            world.btech.constructed_units()[&unit].posture(),
            BattlePosture::Prone
        );
    }
}

/// Traditional completion resets only the action scalar; early stagger falls retain it in every mode.
async fn landing_scalar_reset_matrix(template: &str, mode: BattleStaggerMode) {
    let (_dir, config, base, unit, _, _) =
        firing::fixture_with_target(template, None, template).await;
    let mut probed_fidelity = [false; 2];
    for scalar in [-10, 19, 20] {
        for success in [false, true] {
            let mut world = base.clone();
            launch_battle_jump(&mut world, unit, ObjectId(1), 0, 2.0).unwrap();
            let seed = (0..=255)
                .find(|&seed| {
                    BattleDice::seeded([seed; 32]).two_d6() == if success { 12 } else { 2 }
                })
                .unwrap();
            firing::edit(&mut world, unit, |s| {
                s["stagger"]["action_damage"] = scalar.into();
                s["stagger"]["hits"] =
                    serde_json::json!([{"damage":40,"remaining":60,"counted":false}]);
                s["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
            });
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            let mut rules = BattleMovementRules::STANDARD;
            rules.fall.stagger = mode;
            land(&mut world, &mut replay, unit, rules);
            let failed = scalar >= 20 && !success;
            let state = world.btech.constructed_units()[&unit].stagger();
            assert_eq!(
                state.action_damage,
                if mode == BattleStaggerMode::Traditional && !failed {
                    0
                } else {
                    scalar
                }
            );
            assert_eq!(
                world.btech.constructed_units()[&unit].posture() == BattlePosture::Prone,
                failed
            );
            // Successful completion does not erase the independent damage window.
            if !failed {
                assert_eq!(state.hits.len(), 1);
            }
            // Fidelity probe once per outcome shape per shard; the lockstep replay
            // twin above stays per scenario.
            let shape = usize::from(failed);
            if !probed_fidelity[shape] {
                probed_fidelity[shape] = true;
                persistence::save(&config.database(), &world).await.unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    world.btech
                );
            }
        }
    }
}

#[tokio::test]
async fn landing_scalar_reset_distinguishes_completion_from_stagger_failure_jr7d_traditional() {
    landing_scalar_reset_matrix(
        include_str!("../game/mechs/JR7-D.toml"),
        BattleStaggerMode::Traditional,
    )
    .await;
}

#[tokio::test]
async fn landing_scalar_reset_distinguishes_completion_from_stagger_failure_jr7d_retain() {
    landing_scalar_reset_matrix(
        include_str!("../game/mechs/JR7-D.toml"),
        BattleStaggerMode::Retain,
    )
    .await;
}

#[tokio::test]
async fn landing_scalar_reset_distinguishes_completion_from_stagger_failure_jr7d_consume() {
    landing_scalar_reset_matrix(
        include_str!("../game/mechs/JR7-D.toml"),
        BattleStaggerMode::Consume,
    )
    .await;
}

#[tokio::test]
async fn landing_scalar_reset_distinguishes_completion_from_stagger_failure_spider_traditional() {
    landing_scalar_reset_matrix(
        include_str!("../game/mechs/StalkingSpider-1.toml"),
        BattleStaggerMode::Traditional,
    )
    .await;
}

#[tokio::test]
async fn landing_scalar_reset_distinguishes_completion_from_stagger_failure_spider_retain() {
    landing_scalar_reset_matrix(
        include_str!("../game/mechs/StalkingSpider-1.toml"),
        BattleStaggerMode::Retain,
    )
    .await;
}

#[tokio::test]
async fn landing_scalar_reset_distinguishes_completion_from_stagger_failure_spider_consume() {
    landing_scalar_reset_matrix(
        include_str!("../game/mechs/StalkingSpider-1.toml"),
        BattleStaggerMode::Consume,
    )
    .await;
}

/// Reference-style map reassignment preserves action stagger; material destruction resets it.
#[tokio::test]
async fn map_reassignment_preserves_scalar_and_core_destruction_clears_it() {
    for template in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/StalkingSpider-1.toml"),
    ] {
        let (_dir, config, base, unit, _, _) =
            firing::fixture_with_target(template, None, template).await;
        for height in [2, 12] {
            let mut world = base.clone();
            firing::edit(&mut world, unit, |s| {
                s["stagger"]["action_damage"] = 60.into();
                s["stagger"]["hits"] =
                    serde_json::json!([{"damage":40,"remaining":60,"counted":false}]);
            });
            let before = world.btech.constructed_units()[&unit].stagger().clone();
            let map = world.create(&config, "New field".into(), Kind::Room);
            create_battle_map(
                &mut world,
                map,
                "new",
                BattleMapAsset::from_cells(&format!("1 {height}\n{}", ".0\n".repeat(height)))
                    .unwrap(),
            )
            .unwrap();
            let moved = reassign_battle_map(&mut world, unit, map, None).unwrap();
            assert_eq!(moved.reset_origin, height == 2);
            assert_eq!(world.btech.constructed_units()[&unit].stagger(), &before);
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
            let internal = world.btech.constructed_units()[&unit].sections()
                [&BattleSection::CenterTorso]
                .internal;
            apply_damage_phase(
                &mut world,
                unit,
                BattleSection::CenterTorso,
                internal,
                BattleDamagePhase::Internal,
            )
            .unwrap();
            assert!(world.btech.constructed_units()[&unit].is_destroyed());
            assert_eq!(
                world.btech.constructed_units()[&unit]
                    .stagger()
                    .action_damage,
                0
            );
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
        }
    }
}
