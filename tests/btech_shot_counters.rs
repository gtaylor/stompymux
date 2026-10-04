//! Unit shot statistics share launch semantics, field controls and transactional persistence.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Read the durable owner directly so detached script tables cannot conceal missed writes.
fn counters(world: &World, id: ObjectId) -> serde_json::Value {
    let store = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    serde_json::to_value(&world.btech).unwrap()[store][id.0.to_string()]["shot_counters"].clone()
}

/// Each direct attack counts once, with shared native/Lua output, abort rollback and restart.
#[tokio::test]
async fn direct_shots_count_the_launch_result_for_every_chassis() {
    let mut outcomes = [false; 2];
    for source in firing::templates() {
        let (_dir, config, world, id, target, index) = firing::fixture_with_target(
            &source,
            Some(BattleWeapon::MediumLaser),
            include_str!("../game/mechs/AS7-D.toml"),
        )
        .await;
        for seed in 0..6 {
            let mut candidate = world.clone();
            firing::edit(&mut candidate, id, |unit| {
                unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
            });
            let before = serde_json::to_value(&candidate.btech).unwrap();
            let native = Scripts::new(&config, Rc::new(RefCell::new(candidate.clone()))).unwrap();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(candidate))).unwrap();
            let command = format!("btech.unit.fire({},1,{index},{})", id.0, target.0);
            assert!(
                scripts
                    .eval_callback::<()>(&format!("{command}; error('abort')"))
                    .is_err()
            );
            assert!(serde_json::to_value(&scripts.world().btech).unwrap() == before);
            assert!(scripts.drain_outbox().is_empty());
            let report: mlua::Table = scripts.eval_callback(&format!("return {command}")).unwrap();
            let report = serde_json::to_value(report).unwrap();
            let launch = report.get("launch").unwrap_or(&report);
            assert_eq!(
                launch
                    .get("launched")
                    .unwrap_or(&launch["expenditure"]["launched"]),
                &serde_json::Value::Bool(true)
            );
            let hit = if let Some(hit) = launch.get("hit") {
                hit.as_bool().unwrap()
            } else {
                report["roll"].as_i64().unwrap()
                    >= (report["target_number"].as_i64().unwrap()
                        - i64::from(config.battletech.glancing_blows == 2))
            };
            outcomes[usize::from(hit)] = true;
            let expected =
                serde_json::json!({"fired":1,"hit":i32::from(hit),"missed":i32::from(!hit)});
            assert_eq!(counters(&scripts.world(), id), expected);
            support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire {index} #{}", target.0),
            );
            assert_eq!(counters(&native.world(), id), expected);
            let report =
                view_battle_unit_fields_action(&scripts, &config, ObjectId(1), id, "shots_")
                    .unwrap();
            assert_eq!(report.fields.len(), 3);
            assert!(report.fields.iter().all(|f| f.value.is_some()));
            let saved = scripts.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                counters(&persistence::load(&config.database()).await.unwrap(), id),
                expected
            );
        }
    }
    assert!(
        outcomes.into_iter().all(|seen| seen),
        "Exercise both hits and misses"
    );
}

/// Signed administrative fields are independently editable, with strict bounds and atomic failures.
#[tokio::test]
async fn shot_counter_fields_preserve_independent_values_and_reject_overflow() {
    for source in firing::templates() {
        let (_dir, config, world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for (field, value) in [
            ("shots_fired", "2147483647"),
            ("shots_hit", "-7"),
            ("shots_missed", "91"),
        ] {
            set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, field, value).unwrap();
        }
        let before = serde_json::to_value(&scripts.world().btech).unwrap();
        for value in ["2147483648", "-2147483649", "no"] {
            assert!(
                set_battle_unit_field_action(
                    &scripts,
                    &config,
                    ObjectId(1),
                    id,
                    "shots_fired",
                    value
                )
                .is_err()
            );
            assert!(serde_json::to_value(&scripts.world().btech).unwrap() == before);
        }
        assert_eq!(
            counters(&scripts.world(), id),
            serde_json::json!({"fired":i32::MAX,"hit":-7,"missed":91})
        );
    }
}

/// Occupied coordinate fire damages its occupant without incrementing direct-target statistics.
#[tokio::test]
async fn coordinate_fire_and_counter_overflow_keep_their_transaction_boundaries() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, target, index) = firing::fixture_with_target(
            &source,
            Some(BattleWeapon::MediumLaser),
            include_str!("../game/mechs/AS7-D.toml"),
        )
        .await;
        let direct = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        set_battle_unit_field_action(
            &direct,
            &config,
            ObjectId(1),
            id,
            "shots_fired",
            "2147483647",
        )
        .unwrap();
        direct.drain_outbox();
        let before = serde_json::to_value(&direct.world().btech).unwrap();
        let error = direct
            .eval_callback::<()>(&format!("btech.unit.fire({},1,{index},{})", id.0, target.0))
            .unwrap_err();
        assert!(
            format!("{error:#}").contains("Shot counter overflow"),
            "{error:#}"
        );
        assert!(serde_json::to_value(&direct.world().btech).unwrap() == before);
        assert!(direct.drain_outbox().is_empty());
        select_battle_hex_target(
            &mut world,
            id,
            ObjectId(1),
            HexCoordinate { x: 0, y: 10 },
            BattleHexTargetMode::UnitAtHex,
        )
        .unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let coordinate: (i32, i32) = scripts
            .eval_callback(&format!(
                "local r=btech.unit.fire({},1,{index}); return r.coordinate.x,r.coordinate.y",
                id.0
            ))
            .unwrap();
        assert_eq!(coordinate, (0, 10));
        assert_eq!(
            counters(&scripts.world(), id),
            serde_json::json!({"fired":0,"hit":0,"missed":0})
        );
    }
}

/// A failed Streak lock spends no shot counter, even though it is a completed firing attempt.
#[tokio::test]
async fn failed_streak_locks_do_not_count_as_misses() {
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    for source in firing::templates() {
        let (_dir, config, mut world, id, target, index) = firing::fixture_with_supply(
            &source,
            Some(BattleWeapon::StreakSrm2),
            include_str!("../game/mechs/AS7-D.toml"),
            false,
            Some(""),
        )
        .await;
        firing::edit(&mut world, id, |unit| {
            unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
        });
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let launched: bool = scripts
            .eval_callback(&format!(
                "local r=btech.unit.fire({},1,{index},{}); return r.launched",
                id.0, target.0
            ))
            .unwrap();
        assert!(!launched);
        assert_eq!(
            counters(&scripts.world(), id),
            serde_json::json!({"fired":0,"hit":0,"missed":0})
        );
    }
}

/// Physically out-of-range direct fire still launches and records one miss for ordinary weapons.
#[tokio::test]
async fn out_of_range_direct_attempts_count_once() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, target, index) = firing::fixture_with_target(
            &source,
            Some(BattleWeapon::MediumLaser),
            include_str!("../game/mechs/AS7-D.toml"),
        )
        .await;
        firing::edit(&mut world, target, |unit| {
            unit["position"]["y"] = 0.into();
            unit["motion"]["point"] =
                serde_json::to_value(HexCoordinate { x: 0, y: 0 }.center()).unwrap();
        });
        world.validate(&config).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let launched: bool = scripts
            .eval_callback(&format!(
                "local r=btech.unit.fire({},1,{index},{}); return r.launched",
                id.0, target.0
            ))
            .unwrap();
        assert!(launched);
        assert_eq!(
            counters(&scripts.world(), id),
            serde_json::json!({"fired":1,"hit":0,"missed":1})
        );
    }
}
