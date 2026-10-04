//! Landing uses tactical crew consciousness independently of cockpit assignment.
use crate::support::btech_firing as firing;
use stompymux_rs::*;

/// Departure never makes a conscious crew fall; actual crew injuries still do, including after load.
#[tokio::test]
async fn landing_distinguishes_empty_cockpits_from_unconscious_crews() {
    for template in [
        include_str!("../game/mechs/JR7-D.toml"),
        include_str!("../game/mechs/StalkingSpider-1.toml"),
    ] {
        let (_dir, config, base, unit, _, _) =
            firing::fixture_with_target(template, None, template).await;
        for assigned in [false, true] {
            for unconscious in [false, true] {
                let mut world = base.clone();
                launch_battle_jump(&mut world, unit, ObjectId(1), 0, 2.0).unwrap();
                if !assigned {
                    release_battle_pilot(&mut world, unit, ObjectId(1)).unwrap();
                }
                if unconscious {
                    let seed = (0..=255)
                        .find(|&seed| Dice::seeded([seed; 32]).two_d6() < 7)
                        .unwrap();
                    let mut saved = serde_json::to_value(&world.btech).unwrap();
                    let dice = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
                    saved["constructed"][unit.0.to_string()]["crew_recovery"]["dice"] =
                        dice.clone();
                    saved["recoveries"]["1"]["dice"] = dice;
                    world.btech = serde_json::from_value(saved).unwrap();
                    assert!(
                        !injure_battle_tactical_pilot(&mut world, unit, 3, false)
                            .unwrap()
                            .consciousness
                            .unwrap()
                            .conscious
                    );
                }
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                let mut messages = Vec::new();
                for _ in 0..60 {
                    if world.btech.constructed_units()[&unit].flight().is_none() {
                        break;
                    }
                    let notices =
                        advance_battle_jumps(&mut world, MovementRules::STANDARD).unwrap();
                    let replay =
                        advance_battle_jumps(&mut restored, MovementRules::STANDARD).unwrap();
                    assert_eq!(notices, replay);
                    assert_eq!(world.btech, restored.btech);
                    messages.extend(
                        notices
                            .into_iter()
                            .filter(|n| n.unit == unit)
                            .map(|n| n.text),
                    );
                }
                let mech = &world.btech.constructed_units()[&unit];
                assert!(mech.flight().is_none());
                assert_eq!(mech.posture() == Posture::Prone, unconscious);
                assert_eq!(messages.iter().any(|m| m == "Your lack of conciousness makes you fall to the ground. Not like you can read this anyway."), unconscious);
                assert_eq!(
                    messages.iter().any(|m| m == "You finish your jump."),
                    !unconscious
                );
            }
        }
    }
}
