//! Wizard roll reports preserve access policy, committed checks and read-only command behavior.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Empty reports need no cockpit; argument text is ignored and switches are rejected after permission.
#[tokio::test]
async fn rolls_command_admission_and_empty_report() {
    let (_dir, config, scripts) = support::isolated_scripts().await;
    scripts
        .world_mut()
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    for command in ["+rolls", "+RoLlS ignored text", "+rolls [setq(0,changed)]"] {
        assert_eq!(
            support::run_text(&scripts, &config, ObjectId(1), 1, command),
            "No rolls to show statistics for!"
        );
    }
    assert_eq!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "+rolls/reset"),
        "Command +rolls does not take switches."
    );
    for command in ["+rolls", "+rolls/reset"] {
        assert!(
            support::run_text(&scripts, &config, ObjectId(2), 2, command)
                .contains("Permission denied")
        );
    }
    assert_eq!(scripts.world().battle_roll_statistics().unwrap().total(), 0);
}

/// Actual piloting checks appear once; direct dice and repeated report reads cannot change the histogram.
#[tokio::test]
async fn rolls_command_reports_live_checks_and_resets_on_restart() {
    for source in [
        include_str!("../game/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
    ] {
        let (_dir, config, mut world, unit, target, index) =
            firing::fixture_with_target(source, None, source).await;
        let mut expected = world.battle_roll_statistics().unwrap();
        for _ in 0..16 {
            let check = roll_battle_piloting(&mut world, unit, 0, false).unwrap();
            expected
                .record(check.roll.expect("running unit makes a check"))
                .unwrap();
        }
        roll_unit_dice(&mut world, unit, 20).unwrap();
        assert_eq!(world.battle_roll_statistics().unwrap(), expected);
        let before_preview = serde_json::to_value(&world).unwrap();
        battle_pilot_aim_modifiers(
            &world,
            unit,
            target,
            index,
            false,
            BattleAimRules {
                woods_damage: false,
                dig_bonus: 2,
                dig_only_front: true,
                hit_arc_mode: 0,
                fasa_turning: false,
                extended_movement: false,
                extended_ranges: false,
                hotload_half_minimum: false,
                override_weapon_arcs: false,
            },
        )
        .unwrap();
        assert_eq!(world.battle_roll_statistics().unwrap(), expected);
        assert_eq!(serde_json::to_value(&world).unwrap(), before_preview);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = serde_json::to_value(&*scripts.world()).unwrap();
        for _ in 0..3 {
            let action = commands::run(&scripts, &config, ObjectId(1), 1, "+rolls").unwrap();
            let CommandAction::Report(CommandReport::Literal(report)) = action else {
                panic!("statistics must be a read-only literal report")
            };
            assert_eq!(report, expected.render());
            assert_eq!(report.lines().count(), 13);
            assert_eq!(scripts.world().battle_roll_statistics().unwrap(), expected);
            assert_eq!(serde_json::to_value(&*scripts.world()).unwrap(), before);
            assert!(scripts.outbox().is_empty());
        }
        let world_snapshot = scripts.world().clone();
        persistence::save(&config.database(), &world_snapshot)
            .await
            .unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.battle_roll_statistics().unwrap().total(), 0);
        let restarted = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
        assert_eq!(
            support::run_text(&restarted, &config, ObjectId(1), 1, "+rolls"),
            "No rolls to show statistics for!"
        );
    }
}
