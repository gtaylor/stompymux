//! Stagger diagnostics follow severity warnings and precede private or cockpit roll feedback.
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// All stagger policies retain the same diagnostic order, including unassigned cockpit fallback.
#[tokio::test]
async fn stagger_diagnostic_order_audience_and_rollback() {
    for template in firing::templates().into_iter().take(2) {
        for mode in [
            StaggerMode::Traditional,
            StaggerMode::Retain,
            StaggerMode::Consume,
        ] {
            for assigned in [false, true] {
                let (_dir, config, mut world, unit, _, _) =
                    firing::fixture_with_target(&template, None, &template).await;
                for player in [ObjectId(1), ObjectId(2)] {
                    world.objects.get_mut(&player).unwrap().location = Some(unit);
                    world
                        .objects
                        .get_mut(&player)
                        .unwrap()
                        .flags
                        .insert(Flag::Connected);
                }
                if !assigned {
                    release_battle_pilot(&mut world, unit, ObjectId(1)).unwrap();
                }
                firing::edit(&mut world, unit, |state| {
                    state["stagger"]["turn_damage"] = 40.into();
                    state["stagger"]["hits"] =
                        serde_json::json!([{"damage":40,"remaining":60,"counted":false}]);
                });
                let mut channel = Channel::new("MechDebugInfo".into());
                channel.users.push(communication::Membership {
                    who: ObjectId(2),
                    listening: true,
                });
                world.channels.insert("MechDebugInfo".into(), channel);
                let before = world.clone();
                let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                let rules = StaggerRules {
                    vehicle_impact: MovementRules::STANDARD.fall.vehicle_impact,
                    mode,
                    interval: 1,
                    tonnage: true,
                    hit: MovementRules::STANDARD.fall.hit,
                    extended_piloting: false,
                };
                let reports = advance_battle_stagger_action(&scripts, &config, rules).unwrap();
                assert_eq!(reports.len(), 1);
                let report = &reports[0];
                assert!(report.check.roll.is_some());
                let output = scripts.drain_outbox();
                let diagnostics: Vec<_> = output
                    .iter()
                    .enumerate()
                    .filter(|(_, (_, text))| text.source().contains("Attempting to make pilot"))
                    .collect();
                assert_eq!(diagnostics.len(), 1);
                let (index, (recipient, text)) = diagnostics[0];
                assert_eq!(*recipient, ObjectId(2));
                let check = report.check;
                assert!(text.source().contains(&format!("Attempting to make pilot (noxp) skill roll. SPilot: {}, mods: {}, MechPilot: {}, BTH: {}", check.skill, check.situational, check.damage, check.target)));
                let warning = output
                    .iter()
                    .position(|(_, text)| text.source() == "You stagger from the damage!")
                    .unwrap();
                let feedback = output
                    .iter()
                    .position(|(_, text)| text.source() == "You make a piloting skill roll!")
                    .unwrap();
                assert!(warning < index && index < feedback);
                assert_eq!(
                    report.notices[report.check_notice_index - 1].text,
                    "You stagger from the damage!"
                );
                assert_eq!(
                    output.iter().any(|(who, text)| *who == ObjectId(2)
                        && text.source() == "You make a piloting skill roll!"),
                    !assigned
                );
                let world_snapshot = scripts.world().clone();
                persistence::save(&config.database(), &world_snapshot)
                    .await
                    .unwrap();
                let loaded = persistence::load(&config.database()).await.unwrap();
                assert_eq!(loaded.btech, scripts.world().btech);
                assert_eq!(loaded.channels["MechDebugInfo"].messages, 1);
                *scripts.world_mut() = before.clone();
                scripts
                    .world_mut()
                    .channels
                    .get_mut("MechDebugInfo")
                    .unwrap()
                    .messages = i64::MAX;
                let error = advance_battle_stagger_action(&scripts, &config, rules).unwrap_err();
                assert!(format!("{error:#}").contains("channel message counter overflow"));
                assert_eq!(scripts.world().btech, before.btech);
                assert!(scripts.drain_outbox().is_empty());
            }
        }
    }
}
