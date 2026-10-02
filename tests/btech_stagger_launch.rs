//! Pre-launch failures share native/Lua consequences and preserve the enclosing transaction boundary.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Projected and DFA attempts use the same scalar, messages, replay and fall publication.
#[tokio::test]
async fn prelaunch_stagger_shares_native_lua_and_saved_attempts() {
    for template in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/StalkingSpider-1.toml"),
    ] {
        let (_dir, config, base, unit, target, _) =
            firing::fixture_with_target(template, None, template).await;
        for dfa in [false, true] {
            for scalar in [0, 19, 20, 40] {
                for success in [false, true] {
                    let mut world = base.clone();
                    let seed = (0..=255)
                        .find(|&seed| {
                            BattleDice::seeded([seed; 32]).two_d6() == if success { 12 } else { 2 }
                        })
                        .unwrap();
                    firing::edit(&mut world, unit, |s| {
                        s["stagger"]["action_damage"] = scalar.into();
                        s["stagger"]["hits"] =
                            serde_json::json!([{"damage":60,"remaining":60,"counted":false}]);
                        s["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
                    });
                    persistence::save(&config.database(), &world).await.unwrap();
                    let restored = persistence::load(&config.database()).await.unwrap();
                    let native =
                        Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                    let lua = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
                    let expression = if dfa {
                        format!("btech.unit.dfa({},1,{})", unit.0, target.0)
                    } else {
                        format!("btech.unit.jump({},1,0,2)", unit.0)
                    };
                    assert!(
                        lua.eval_callback::<()>(&format!(
                            "{expression}; error('abort jump attempt')"
                        ))
                        .is_err()
                    );
                    assert_eq!(lua.world().btech, world.btech);
                    assert!(lua.outbox().is_empty());
                    assert!(
                        lua.eval_callback::<bool>(&format!("return {expression}"))
                            .unwrap()
                    );
                    let lua_text = text::plain(
                        &lua.drain_outbox()
                            .into_iter()
                            .map(|(_, d)| d.source().to_owned())
                            .collect::<Vec<_>>()
                            .join("\n"),
                    );
                    let command = if dfa {
                        format!("jump #{}", target.0)
                    } else {
                        "jump 0 2".into()
                    };
                    let native_text = text::plain(&support::run_text(
                        &native,
                        &config,
                        ObjectId(1),
                        1,
                        &command,
                    ));
                    assert_eq!(native_text, lua_text);
                    assert_eq!(native.world().btech, lua.world().btech);
                    let checked = scalar >= 20;
                    let lines: Vec<_> = native_text.lines().collect();
                    let roll = lines
                        .iter()
                        .position(|line| *line == "You make a piloting skill roll!");
                    assert_eq!(roll.is_some(), checked);
                    if let Some(index) = roll {
                        assert_eq!(lines[index - 1], "The damage inhibits your coordination...");
                        assert!(lines[index + 1].starts_with("Modified Pilot Skill: BTH "));
                        assert!(lines[index + 1].ends_with(if success {
                            "\tRoll: 12"
                        } else {
                            "\tRoll: 2"
                        }));
                    }
                    let failed = checked && !success;
                    assert_eq!(
                        native_text.contains("The damage inhibits your coordination..."),
                        checked
                    );
                    assert_eq!(
                        native_text.contains("... something you apparently can't handle!"),
                        failed
                    );
                    let state = native.world();
                    let mech = &state.btech.constructed_units()[&unit];
                    assert_eq!(mech.flight().is_some(), !failed);
                    assert_eq!(mech.posture() == BattlePosture::Prone, failed);
                    assert_eq!(mech.jump_stabilization(), 0);
                    if failed {
                        assert!(!native_text.contains("You engage your jump jets"));
                    } else {
                        let mut dice = BattleDice::seeded([seed; 32]);
                        if checked {
                            dice.two_d6();
                        }
                        assert_eq!(
                            serde_json::to_value(&state.btech).unwrap()["constructed"]
                                [unit.0.to_string()]["dice"],
                            serde_json::to_value(dice).unwrap()
                        );
                    }
                }
            }
        }
    }
}

/// Native syntax and destination rejections happen after the roll and remain private to the pilot.
#[tokio::test]
async fn rejected_native_requests_preserve_roll_and_private_rejection() {
    for template in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/StalkingSpider-1.toml"),
    ] {
        let (_dir, config, mut base, unit, _, _) =
            firing::fixture_with_target(template, None, template).await;
        select_battle_target(&mut base, unit, ObjectId(1), None).unwrap();
        base.objects.get_mut(&ObjectId(2)).unwrap().location = Some(unit);
        for (command, rejection) in [
            ("jump extra invalid arguments", "Usage: jump"),
            ("jump bad 2", "Invalid jump bearing"),
            ("jump 0 999", "That target is out of range!"),
            ("jump #999999", "Invalid Target!"),
            ("jump", "Invalid Target!"),
        ] {
            for success in [false, true] {
                let mut world = base.clone();
                let seed = (0..=255)
                    .find(|&seed| {
                        BattleDice::seeded([seed; 32]).two_d6() == if success { 12 } else { 2 }
                    })
                    .unwrap();
                firing::edit(&mut world, unit, |s| {
                    s["stagger"]["action_damage"] = 20.into();
                    s["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
                });
                let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                assert!(matches!(
                    commands::run(&scripts, &config, ObjectId(1), 1, command).unwrap(),
                    CommandAction::Continue
                ));
                let messages = scripts.drain_outbox();
                let for_player = |id| {
                    messages
                        .iter()
                        .filter(|(p, _)| *p == ObjectId(id))
                        .map(|(_, d)| text::plain(d.source()))
                        .collect::<Vec<_>>()
                        .join("\n")
                };
                let pilot = for_player(1);
                let passenger = for_player(2);
                assert!(pilot.contains("The damage inhibits your coordination..."));
                assert!(passenger.contains("The damage inhibits your coordination..."));
                let lines: Vec<_> = pilot.lines().collect();
                assert_eq!(lines[1], "You make a piloting skill roll!");
                assert!(lines[2].starts_with("Modified Pilot Skill: BTH "));
                assert!(lines[2].ends_with(if success { "\tRoll: 12" } else { "\tRoll: 2" }));
                assert!(!passenger.contains("You make a piloting skill roll!"));
                assert!(!passenger.contains("Modified Pilot Skill:"));
                assert_eq!(pilot.contains(rejection), success, "{pilot}");
                assert!(!passenger.contains(rejection), "{passenger}");
                let after = scripts.world();
                assert!(after.btech.constructed_units()[&unit].flight().is_none());
                assert_eq!(
                    after.btech.constructed_units()[&unit].posture() == BattlePosture::Prone,
                    !success
                );
                if success {
                    let mut dice = BattleDice::seeded([seed; 32]);
                    dice.two_d6();
                    firing::edit(&mut world, unit, |s| {
                        s["dice"] = serde_json::to_value(dice).unwrap()
                    });
                    assert_eq!(after.btech, world.btech);
                }
            }
        }
    }
}

/// Typed Lua requests retain a successful roll on rejection but revert it with an enclosing failure.
#[tokio::test]
async fn rejected_lua_requests_and_early_admission_keep_transaction_boundaries() {
    let template = include_str!("../game/mechs/JR7-D.toml");
    let (_dir, config, mut base, unit, _, _) =
        firing::fixture_with_target(template, None, template).await;
    select_battle_target(&mut base, unit, ObjectId(1), None).unwrap();
    let seed = (0..=255)
        .find(|&seed| BattleDice::seeded([seed; 32]).two_d6() == 12)
        .unwrap();
    firing::edit(&mut base, unit, |s| {
        s["stagger"]["action_damage"] = 20.into();
        s["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    });
    for expression in [
        format!("btech.unit.jump({},1,0,999)", unit.0),
        format!("btech.unit.dfa({},1,999999)", unit.0),
        format!("btech.unit.dfa({},1)", unit.0),
    ] {
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
        assert!(
            scripts
                .eval_callback::<()>(&format!("{expression}; error('abort rejected attempt')"))
                .is_err()
        );
        assert_eq!(scripts.world().btech, base.btech);
        assert!(scripts.outbox().is_empty());
        assert!(
            scripts
                .eval_callback::<bool>(&format!("return {expression}"))
                .unwrap()
        );
        let mut expected = base.clone();
        let mut dice = BattleDice::seeded([seed; 32]);
        dice.two_d6();
        firing::edit(&mut expected, unit, |s| {
            s["dice"] = serde_json::to_value(dice).unwrap()
        });
        assert_eq!(scripts.world().btech, expected.btech);
    }
    let map = base.btech.units()[&unit].map.unwrap();
    let mut saved = serde_json::to_value(&base.btech).unwrap();
    saved["maps"][map.0.to_string()]["flags"] = 16.into();
    base.btech = serde_json::from_value(saved).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(base.clone()))).unwrap();
    let response = support::run_text(
        &scripts,
        &config,
        ObjectId(1),
        1,
        "jump malformed arguments extra",
    );
    assert!(response.contains("The underground ceiling prevents jumping"));
    assert!(!response.contains("coordination"));
    assert_eq!(scripts.world().btech, base.btech);
}
