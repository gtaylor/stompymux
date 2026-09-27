//! Markings configuration and contact disclosure share native, Lua and restart behavior.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Every supported source/recipient pair uses the same literal, read-only cockpit report.
#[tokio::test]
async fn markings_share_native_lua_and_restart_across_chassis() {
    for source in firing::templates() {
        for recipient in firing::templates() {
            let (_dir, config, mut world, observer, target, _) =
                firing::fixture_with_target(&source, None, &recipient).await;
            let literal = "A white wolf.\n[fg=red]literal markup[reset] <徽章>";
            set_battle_unit_markings(&mut world, ObjectId(1), target, literal).unwrap();
            let label = visible_battle_contact(&world, observer, target)
                .unwrap()
                .unwrap()
                .label;
            let before = world.btech.clone();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            // Markings do not require a working detailed-scan range.
            set_battle_unit_field_action(
                &scripts,
                &config,
                ObjectId(1),
                observer,
                "scanrange",
                "0",
            )
            .unwrap();
            let checkpoint = scripts.world().btech.clone();
            for target_arg in [None, Some(target)] {
                let report =
                    view_battle_unit_markings(&scripts.world(), observer, ObjectId(1), target_arg)
                        .unwrap();
                assert_eq!(text::plain(&report), literal);
            }
            let lua: String = scripts
                .eval_callback(&format!("return btech.unit.view({},1)", observer.0))
                .unwrap();
            assert_eq!(text::plain(&lua), literal);
            for command in ["view".to_owned(), format!("view {label}")] {
                let output = text::plain(&support::run_text(
                    &scripts,
                    &config,
                    ObjectId(1),
                    1,
                    &command,
                ));
                assert!(output.contains("A white wolf."), "{output}");
                assert!(output.contains("[fg=red]literal markup[reset]"), "{output}");
            }
            assert_eq!(scripts.world().btech, checkpoint);
            let saved = scripts.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(restored.btech, checkpoint);
            assert_eq!(battle_unit_markings(&restored, target).unwrap(), literal);
            assert_ne!(before, checkpoint); // Only the explicit hardware edit changed the fixture.
        }
    }
}

/// Configuration is bounded, administrative, and part of the enclosing Lua transaction.
#[tokio::test]
async fn markings_configuration_validates_and_rolls_back() {
    for template in firing::templates() {
        let (_dir, config, mut world, observer, target, _) =
            firing::fixture_with_target(&template, None, &template).await;
        let unprivileged = world.create(&config, "Unprivileged".into(), Kind::Player);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.set_markings_as(1,{},'temporary'); error('abort')",
                    target.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
        let result: String = scripts
            .eval_callback(&format!(
                "btech.unit.set_markings_as(1,{},'Blue shield'); return btech.unit.markings({})",
                target.0, target.0
            ))
            .unwrap();
        assert_eq!(result, "Blue shield");
        for invalid in ["x".repeat(16384), "bad\0text".into()] {
            let before = scripts.world().btech.clone();
            assert!(
                set_battle_unit_markings(&mut scripts.world_mut(), ObjectId(1), target, &invalid)
                    .is_err()
            );
            assert_eq!(scripts.world().btech, before);
        }
        assert!(
            set_battle_unit_markings(&mut scripts.world_mut(), unprivileged, target, "forbidden")
                .is_err()
        );
        set_battle_unit_markings(&mut scripts.world_mut(), ObjectId(1), target, "").unwrap();
        assert_eq!(
            view_battle_unit_markings(&scripts.world(), observer, ObjectId(1), None).unwrap(),
            "That target has no markings."
        );
    }
}

/// The VIEW command in a map room remains wizard-only for ordinary players.
#[tokio::test]
async fn markings_map_view_is_wizard_only() {
    for source in firing::templates() {
        let (_dir, config, mut world, parent, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let player = world.create(&config, "Player".into(), Kind::Player);
        let map = world.btech.units()[&parent].map.unwrap();
        world.objects.get_mut(&player).unwrap().location = Some(map);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        assert!(
            support::run_text(&scripts, &config, player, 1, "view 0 0")
                .contains("Sorry, that command is restricted!")
        );
    }
}

/// A selected target is not permission to disclose markings after it becomes invisible.
#[tokio::test]
async fn markings_recheck_visibility_and_running_admission_without_side_effects() {
    let (_dir, config, mut world, observer, target, _) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        None,
        include_str!("../game/mechs/Demolisher"),
    )
    .await;
    set_battle_unit_markings(&mut world, ObjectId(1), target, "Secret insignia").unwrap();
    set_battle_visibility(
        &mut world,
        target,
        BattleVisibility {
            invisible: true,
            clairvoyant: false,
        },
    )
    .unwrap();
    let before = world.btech.clone();
    assert_eq!(
        view_battle_unit_markings(&world, observer, ObjectId(1), Some(target))
            .unwrap_err()
            .to_string(),
        "Target is not in line of sight!"
    );
    assert!(
        view_battle_unit_markings(&world, observer, ObjectId(1), None)
            .unwrap_err()
            .to_string()
            .contains("scannfers")
    );
    assert_eq!(world.btech, before);
    select_battle_target(&mut world, observer, ObjectId(1), None).unwrap();
    assert_eq!(
        view_battle_unit_markings(&world, observer, ObjectId(1), None)
            .unwrap_err()
            .to_string(),
        "You do not have a default target set!"
    );
    stop_battle_unit(
        &mut world,
        observer,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert!(view_battle_unit_markings(&world, observer, ObjectId(1), Some(target)).is_err());
    world.validate(&config).unwrap();
}
