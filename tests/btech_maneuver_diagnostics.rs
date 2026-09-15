//! Maneuver diagnostics preserve their reference label, ordering, subscriber audience and rollback.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[allow(dead_code)]
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// All three host actions expose the initial check and use one diagnostic publication path.
fn attempt(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    action: u8,
) -> anyhow::Result<Option<BattlePilotingCheck>> {
    match action {
        0 => Ok(Some(
            battle_bootlegger(scripts, config, unit, ObjectId(1), "right")?.check,
        )),
        1 => Ok(prone_action(scripts, config, unit, ObjectId(1))?.check),
        _ => Ok(Some(
            begin_battle_stand_action(
                scripts,
                config,
                unit,
                ObjectId(1),
                BattleStandMode::Anyway,
                false,
                BattleFallRules::configured(config),
            )?
            .check,
        )),
    }
}

/// Capture both rolling and automatic paths and retry after a diagnostic-channel publication failure.
#[tokio::test]
async fn maneuver_checks_publish_diagnostics_at_the_roll_boundary() {
    for action in 0..3 {
        for automatic in [false, true] {
            if automatic && action == 0 {
                continue;
            }
            let template = &firing::templates()[usize::from(automatic && action == 2)];
            let (_dir, config, mut world, unit, _, _) =
                firing::fixture_with_target(template, None, template).await;
            firing::edit(&mut world, unit, |state| {
                state["motion"]["speed"] = if action == 0 {
                    50.0
                } else if action == 1 && !automatic {
                    100.0
                } else {
                    0.0
                }
                .into();
                state["motion"]["desired_speed"] = state["motion"]["speed"].clone();
                if action == 2 {
                    state["posture"] = serde_json::to_value(BattlePosture::Prone).unwrap();
                }
            });
            world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(unit);
            for player in [ObjectId(1), ObjectId(2)] {
                world
                    .objects
                    .get_mut(&player)
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
            }
            let mut channel = Channel::new("MechDebugInfo".into());
            channel.users.push(communication::Membership {
                who: ObjectId(2),
                listening: true,
            });
            world.channels.insert("MechDebugInfo".into(), channel);
            let before = world.clone();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let check = attempt(&scripts, &config, unit, action).unwrap();
            let output = scripts.drain_outbox();
            let diagnostics: Vec<_> = output
                .iter()
                .filter(|(_, text)| text.source().contains("Attempting to make pilot"))
                .collect();
            if automatic {
                assert!(check.is_none_or(|check| check.roll.is_none()));
                assert!(diagnostics.is_empty());
                assert_eq!(scripts.world().channels["MechDebugInfo"].messages, 0);
                continue;
            }
            let check = check.unwrap();
            assert!(check.roll.is_some());
            assert_eq!(diagnostics.len(), 1);
            assert_eq!(diagnostics[0].0, ObjectId(2));
            let label = if action == 2 { "" } else { " (noxp)" };
            let expected = format!(
                "Attempting to make pilot{label} skill roll. SPilot: {}, mods: {}, MechPilot: {}, BTH: {}",
                check.skill, check.situational, check.damage, check.target
            );
            assert!(diagnostics[0].1.source().contains(&expected));
            let diagnostic_position = output
                .iter()
                .position(|(_, text)| text.source().contains(&expected))
                .unwrap();
            let roll_position = output
                .iter()
                .position(|(who, text)| {
                    *who == ObjectId(1) && text.source() == "You make a piloting skill roll!"
                })
                .unwrap();
            assert!(diagnostic_position < roll_position);
            if action == 1 {
                let warning = output
                    .iter()
                    .position(|(_, text)| {
                        text.source().starts_with("You attempt a controlled drop")
                    })
                    .unwrap();
                assert!(warning < diagnostic_position);
            }
            assert!(!output.iter().any(|(who, text)| *who == ObjectId(2)
                && text.source() == "You make a piloting skill roll!"));
            let after = scripts.world().btech.clone();
            persistence::save(&config.database(), &scripts.world())
                .await
                .unwrap();
            let loaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(loaded.btech, after);
            assert_eq!(loaded.channels["MechDebugInfo"].messages, 1);
            *scripts.world_mut() = before.clone();
            scripts
                .world_mut()
                .channels
                .get_mut("MechDebugInfo")
                .unwrap()
                .messages = i64::MAX;
            let error = attempt(&scripts, &config, unit, action).unwrap_err();
            assert!(format!("{error:#}").contains("channel message counter overflow"));
            assert_eq!(scripts.world().btech, before.btech);
            assert_eq!(scripts.world().channels["MechDebugInfo"].messages, i64::MAX);
            assert!(scripts.drain_outbox().is_empty());
            *scripts.world_mut() = before.clone();
            let command = match action {
                0 => "bootlegger right",
                1 => "prone",
                _ => "stand anyway",
            };
            support::run_text(&scripts, &config, ObjectId(1), 1, command);
            assert_eq!(scripts.world().btech, after);
            assert_eq!(scripts.world().channels["MechDebugInfo"].messages, 1);
            *scripts.world_mut() = before;
            let lua = match action {
                0 => format!("btech.unit.bootlegger({},1,'right')", unit.0),
                1 => format!("btech.unit.prone({},1)", unit.0),
                _ => format!("btech.unit.stand({},1,'anyway')", unit.0),
            };
            scripts
                .eval_callback::<()>(&format!("{lua}; error('undo diagnostic')"))
                .unwrap_err();
            assert_eq!(scripts.world().channels["MechDebugInfo"].messages, 0);
            assert!(scripts.drain_outbox().is_empty());
            scripts.eval_callback::<()>(&lua).unwrap();
            assert_eq!(scripts.world().btech, after);
            assert_eq!(scripts.world().channels["MechDebugInfo"].messages, 1);
        }
    }
}
