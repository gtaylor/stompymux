//! Vehicle turn preferences share admission, saved fields and throttle rules with Mechs.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
use crate::support;

/// Cockpit, Lua and administrative settings select the same durable movement preference.
#[tokio::test]
async fn vehicle_turnmode_is_guarded_durable_and_changes_only_slowdown_two() {
    for source in firing::templates().into_iter().skip(2) {
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        set_battle_character(
            &mut world,
            ObjectId(1),
            BattleCharacter {
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
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        assert!(battle_turnmode(&scripts, id, ObjectId(1), "tight").is_err());
        set_battle_character_value(
            &mut scripts.world_mut(),
            ObjectId(1),
            "Maneuvering_Ace",
            BattleCharacterValue {
                value: 1,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(battle_turnmode(&scripts, id, ObjectId(2), "tight").is_err());
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.turnmode({},1,'tight'); error('abort')",
                    id.0
                ))
                .is_err()
        );
        assert!(!scripts.world().btech.vehicles()[&id].tight_turn_mode());
        assert!(
            support::run_text(&scripts, &config, ObjectId(1), 1, "turnmode tight")
                .contains("tighter turns")
        );
        let enabled: bool = scripts
            .eval_callback(&format!(
                "return btech.unit.state({}).tight_turn_mode",
                id.0
            ))
            .unwrap();
        assert!(enabled);
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, saved.btech);
        if loaded.btech.vehicles()[&id].definition().movement != BattleVehicleMovement::Stationary {
            let maximum = loaded.btech.vehicles()[&id].maximum_speed();
            for slowdown in [0, 1, 2] {
                let unit = &loaded.btech.vehicles()[&id];
                let mut motion = unit.motion().unwrap();
                motion.heading = 0.0;
                motion.desired_heading = 120.0;
                motion.desired_speed = maximum;
                motion.speed = maximum * 0.5;
                let rules = BattleVehicleMotionRules {
                    slowdown,
                    ..BattleVehicleMotionRules::STANDARD
                };
                let tight = unit
                    .ground_motion_step(motion, Terrain::Grassland, rules)
                    .unwrap();
                firing::edit(&mut loaded, id, |unit| {
                    unit["tight_turn_mode"] = serde_json::json!(false)
                });
                let normal = loaded.btech.vehicles()[&id]
                    .ground_motion_step(motion, Terrain::Grassland, rules)
                    .unwrap();
                if slowdown == 2 {
                    assert!(tight.speed < normal.speed, "{tight:?} {normal:?}");
                } else {
                    assert_eq!(tight, normal);
                }
                firing::edit(&mut loaded, id, |unit| {
                    unit["tight_turn_mode"] = serde_json::json!(true)
                });
            }
        }
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "MechPrefs", "0").unwrap();
        assert!(!scripts.world().btech.vehicles()[&id].tight_turn_mode());
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "MechPrefs", "i").unwrap();
        assert!(scripts.world().btech.vehicles()[&id].tight_turn_mode());
    }
}
