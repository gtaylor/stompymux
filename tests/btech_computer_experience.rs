//! Successful thermal overrides award Computer XP through the ordinary transactional skill service.
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Thermal admission uses the standard impact and movement policy in every scenario.
fn rules() -> OverheatRules {
    let fall = MovementRules::STANDARD.fall;
    OverheatRules {
        vehicle_impact: fall.vehicle_impact,
        stacking: fall.stacking,
        hit: fall.hit,
        extended_piloting: fall.extended_piloting,
        stagger: fall.stagger,
    }
}

/// Force a due override without adding injury, ammunition or movement checks.
fn due(world: &mut World, id: ObjectId, heat: f64, roll: u8) {
    let seed = (0..=255)
        .find(|&s| Dice::seeded([s; 32]).two_d6() == roll)
        .unwrap();
    firing::edit(world, id, |state| {
        state["heat"] = serde_json::json!({"stored":heat + 10.0,"excess":heat});
        state["overheat_clock"] = serde_json::json!({"elapsed":30,"phase":0,"injury_due":false});
        state["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
    });
}

/// Train Computer to target six, preserving enough threshold for the first point to remain a balance.
async fn fixture(source: &str) -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world, id, _, _) =
        firing::fixture_with_target(source, None, source).await;
    set_battle_character(
        &mut world,
        ObjectId(1),
        Character {
            bruise: 0,
            lethal: 0,
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
        },
    )
    .unwrap();
    crate::support::seed_object_dice(&mut world, ObjectId(1), crate::support::FIXTURE_DICE_SEED);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Computer",
        CharacterValue {
            value: 2,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    (dir, config, world, id)
}

/// IC, success and actual Computer rolls gate awards; being disconnected does not change eligibility.
#[tokio::test]
async fn computer_override_xp_obeys_character_success_and_heat_gates() {
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/GOL-1H.toml"),
    ] {
        let (_dir, config, base, id) = fixture(source).await;
        for ic in [false, true] {
            for connected in [false, true] {
                for (heat, roll) in [(10.0, 12), (14.0, 2), (14.0, 12)] {
                    let mut world = base.clone();
                    for (object, flag, enabled) in [
                        (id, Flag::InCharacter, ic),
                        (ObjectId(1), Flag::Connected, connected),
                    ] {
                        let flags = &mut world.objects.get_mut(&object).unwrap().flags;
                        if enabled {
                            flags.insert(flag);
                        } else {
                            flags.remove(flag);
                        }
                    }
                    due(&mut world, id, heat, roll);
                    let mut expected_dice = world.clone();
                    roll_unit_dice(&mut expected_dice, id, 2).unwrap();
                    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                    let reports =
                        advance_battle_overheat_action(&scripts, &config, rules()).unwrap();
                    if heat < 14.0 {
                        assert!(reports.is_empty());
                        assert_eq!(
                            scripts.world().btech.character_values()[&ObjectId(1)]["Computer"]
                                .experience_balance(),
                            0
                        );
                        continue;
                    }
                    let report = reports.iter().find(|r| r.unit == id).unwrap();
                    let eligible = ic && heat >= 14.0 && roll == 12;
                    assert_eq!(report.computer_experience.is_some(), eligible);
                    assert_eq!(report.experience_messages.len(), usize::from(eligible));
                    let traces = crate::support::drain_traces(
                        &scripts,
                        stompymux_rs::logging::TraceTopic::Experience,
                    );
                    assert_eq!(traces.len(), usize::from(eligible));
                    assert_eq!(
                        scripts.world().btech.character_values()[&ObjectId(1)]["Computer"]
                            .experience_balance(),
                        u32::from(eligible)
                    );
                    if eligible {
                        assert_eq!(
                            serde_json::to_value(&scripts.world().btech.constructed_units()[&id])
                                .unwrap()["dice"],
                            serde_json::to_value(&expected_dice.btech.constructed_units()[&id])
                                .unwrap()["dice"],
                        );
                        assert!(report.computer_experience.unwrap().accepted);
                        assert_eq!(
                            report.experience_messages[0].topic,
                            DiagnosticTopic::Experience
                        );
                        assert_eq!(
                            report.experience_messages[0].text,
                            format!("GOD gained 1 computer XP (mech #{})", id.0)
                        );
                        assert_eq!(traces[0], report.experience_messages[0].text);
                        assert!(!report.shutdown);
                    }
                }
            }
        }
    }
}

/// Restored cooldowns suppress duplicate awards without suppressing the successful override itself.
#[tokio::test]
async fn computer_override_xp_interval_survives_restart() {
    let (_dir, config, mut world, id) = fixture(include_str!("../game/units/JR7-D.toml")).await;
    due(&mut world, id, 14.0, 12);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report = advance_battle_overheat_action(&scripts, &config, rules())
        .unwrap()
        .remove(0);
    assert!(report.computer_experience.unwrap().accepted);
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, saved.btech);
    for mut world in [saved, restored] {
        due(&mut world, id, 14.0, 12);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let report = advance_battle_overheat_action(&scripts, &config, rules())
            .unwrap()
            .remove(0);
        assert!(report.shutdown_check.unwrap().success);
        assert!(!report.computer_experience.unwrap().accepted);
        assert!(report.experience_messages.is_empty());
        assert_eq!(
            scripts.world().btech.character_values()[&ObjectId(1)]["Computer"].experience_balance(),
            1
        );
    }
}
