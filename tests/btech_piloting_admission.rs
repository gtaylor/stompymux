//! Shared control checks fail without dice for blinded or unconscious crews, including empty cockpits.
use stompymux_rs::*;
#[allow(dead_code)]
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// A real tactical injury starts the unit-owned recovery clock without a fictitious pilot.
fn unconscious_crew(world: &mut World, unit: ObjectId) {
    let seed = (0..=255)
        .find(|&seed| {
            let mut dice = BattleDice::seeded([seed; 32]);
            dice.two_d6() < 7 && dice.two_d6() >= 10
        })
        .unwrap();
    firing::edit(world, unit, |s| {
        s["crew_recovery"]["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
    });
    let injury = injure_battle_tactical_pilot(world, unit, 3, false).unwrap();
    assert!(!injury.consciousness.unwrap().conscious);
}

/// Both blindness and empty-crew recovery block rolls independently and survive restart on all chassis.
#[tokio::test]
async fn control_gates_share_no_roll_policy_and_resume_after_recovery() {
    for template in firing::templates() {
        let (_dir, config, mut base, unit, _, _) =
            firing::fixture_with_target(&template, None, &template).await;
        release_battle_pilot(&mut base, unit, ObjectId(1)).unwrap();
        for blind in [false, true] {
            for unconscious in [false, true] {
                let mut world = base.clone();
                if unconscious {
                    unconscious_crew(&mut world, unit);
                }
                firing::edit(&mut world, unit, |s| {
                    s["blinded_remaining"] = if blind { 4 } else { 0 }.into()
                });
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                let before = world.btech.clone();
                let check = roll_battle_piloting(&mut world, unit, -100, false).unwrap();
                assert_eq!(
                    check,
                    roll_battle_piloting(&mut restored, unit, -100, false).unwrap()
                );
                if blind || unconscious {
                    assert!(!check.success);
                    assert_eq!(check.roll, None);
                    assert_eq!(world.btech, before);
                } else {
                    assert!(check.success);
                    assert!(check.roll.is_some());
                }
                for _ in 0..30 {
                    advance_battle_units(&mut world, 0);
                    advance_battle_sensor_flashes(&mut world);
                }
                assert!(!battle_unit_blinded(&world, unit));
                let recovered = roll_battle_piloting(&mut world, unit, -100, false).unwrap();
                assert!(recovered.success);
                assert!(recovered.roll.is_some());
            }
        }
    }
}

/// The reference's already-fallen exception precedes blocked-crew checks for Mechs.
#[tokio::test]
async fn prone_mech_success_precedes_blindness_and_unconsciousness() {
    for template in firing::templates().into_iter().take(2) {
        let (_dir, _config, mut world, unit, _, _) =
            firing::fixture_with_target(&template, None, &template).await;
        release_battle_pilot(&mut world, unit, ObjectId(1)).unwrap();
        unconscious_crew(&mut world, unit);
        firing::edit(&mut world, unit, |s| {
            s["posture"] = serde_json::to_value(BattlePosture::Prone).unwrap();
            s["blinded_remaining"] = 4.into();
        });
        let before = world.btech.clone();
        let check = roll_battle_piloting(&mut world, unit, 100, false).unwrap();
        assert!(check.success);
        assert_eq!(check.roll, None);
        assert_eq!(world.btech, before);
    }
}
