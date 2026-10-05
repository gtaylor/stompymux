//! Maneuver roll diagnostics preserve their reference label as debug traces, and roll back with the action.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// All three host actions expose the initial check and use one diagnostic publication path.
fn attempt(
    scripts: &Scripts,
    config: &Config,
    unit: ObjectId,
    action: u8,
) -> anyhow::Result<Option<PilotingCheck>> {
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
                StandMode::Anyway,
                false,
                FallRules::configured(config),
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
                    state["posture"] = serde_json::to_value(Posture::Prone).unwrap();
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
            let before = world.clone();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let check = attempt(&scripts, &config, unit, action).unwrap();
            let output = scripts.drain_outbox();
            let traces = support::drain_traces(&scripts, logging::TraceTopic::PilotingRolls);
            assert!(
                !output
                    .iter()
                    .any(|(_, text)| text.source().contains("Attempting to make pilot"))
            );
            if automatic {
                assert!(check.is_none_or(|check| check.roll.is_none()));
                assert!(traces.is_empty());
                continue;
            }
            let check = check.unwrap();
            assert!(check.roll.is_some());
            let label = if action == 2 { "" } else { " (noxp)" };
            let expected = format!(
                "Attempting to make pilot{label} skill roll. SPilot: {}, mods: {}, Damage: {}, BTH: {}",
                check.skill, check.situational, check.damage, check.target
            );
            assert_eq!(traces, [expected]);
            let roll_position = output
                .iter()
                .position(|(who, text)| {
                    *who == ObjectId(1) && text.source() == "You make a piloting skill roll!"
                })
                .unwrap();
            if action == 1 {
                let warning = output
                    .iter()
                    .position(|(_, text)| {
                        text.source().starts_with("You attempt a controlled drop")
                    })
                    .unwrap();
                assert!(warning < roll_position);
            }
            assert!(!output.iter().any(|(who, text)| *who == ObjectId(2)
                && text.source() == "You make a piloting skill roll!"));
            let after = scripts.world().btech.clone();
            let world_snapshot = scripts.world().clone();
            persistence::save(&config.database(), &world_snapshot)
                .await
                .unwrap();
            let loaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(loaded.btech, after);
            *scripts.world_mut() = before.clone();
            let command = match action {
                0 => "bootlegger right",
                1 => "prone",
                _ => "stand anyway",
            };
            support::run_text(&scripts, &config, ObjectId(1), 1, command);
            assert_eq!(scripts.world().btech, after);
            assert_eq!(
                support::drain_traces(&scripts, logging::TraceTopic::PilotingRolls).len(),
                1
            );
            *scripts.world_mut() = before;
            let lua = match action {
                0 => format!("btech.unit.bootlegger({},1,'right')", unit.0),
                1 => format!("btech.unit.prone({},1)", unit.0),
                _ => format!("btech.unit.stand({},1,'anyway')", unit.0),
            };
            scripts
                .eval_callback::<()>(&format!("{lua}; error('undo diagnostic')"))
                .unwrap_err();
            assert!(support::drain_traces(&scripts, logging::TraceTopic::PilotingRolls).is_empty());
            assert!(scripts.drain_outbox().is_empty());
            scripts.eval_callback::<()>(&lua).unwrap();
            assert_eq!(scripts.world().btech, after);
            assert_eq!(
                support::drain_traces(&scripts, logging::TraceTopic::PilotingRolls).len(),
                1
            );
        }
    }
}
