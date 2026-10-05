//! Tracking modes share combat modifiers, administrative admission and saved state across chassis.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Ordinary direct-fire rules keep the selected-target settling contribution visible.
fn rules() -> AimRules {
    AimRules {
        woods_damage: false,
        dig_bonus: 3,
        dig_only_front: false,
        hit_arc_mode: 0,
        fasa_turning: false,
        extended_movement: false,
        extended_ranges: false,
        hotload_half_minimum: false,
        override_weapon_arcs: false,
    }
}

#[tokio::test]
async fn tracking_modes_change_shared_aim_and_persist_without_changing_equipment() {
    for source in firing::templates() {
        let (_dir, config, world, id, target, weapon) =
            firing::fixture_with_target(&source, None, &source).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let baseline =
            battle_aim_modifiers(&scripts.world(), id, target, weapon, 4, rules()).unwrap();
        for (mode, expected) in [(1, -1), (2, 1), (3, 0), (4, 1), (0, 0)] {
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.set_field(1,{},'targcomp','{}')",
                    id.0, mode
                ))
                .unwrap();
            let aim =
                battle_aim_modifiers(&scripts.world(), id, target, weapon, 4, rules()).unwrap();
            assert_eq!(aim.targeting_mode, expected);
            assert_eq!(aim.targeting_computer, baseline.targeting_computer);
            let hex = battle_hex_aim_modifiers(
                &scripts.world(),
                id,
                HexCoordinate { x: 0, y: 9 },
                weapon,
                4,
                rules(),
            )
            .unwrap();
            assert_eq!(
                hex.modifiers.targeting_mode,
                if mode == 4 { 0 } else { expected }
            );
            assert_eq!(
                aim.subtotal(),
                baseline.subtotal().map(|value| value
                    + i32::from(expected)
                    + i32::from(aim.target_lock)
                    - i32::from(baseline.target_lock))
            );
        }
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "targcomp", "2").unwrap();
        let before = scripts.world().btech.clone();
        for value in ["-1", "5", "256", "nan"] {
            assert!(
                set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "targcomp", value)
                    .is_err()
            );
            assert_eq!(scripts.world().btech, before);
        }
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, before);
        let aim = battle_aim_modifiers(&restored, id, target, weapon, 4, rules()).unwrap();
        assert_eq!(aim.targeting_mode, 1);
    }
}

#[tokio::test]
async fn tracking_suppresses_lock_settling_and_anti_air_distinguishes_flight() {
    let sources = firing::templates();
    for source in &sources {
        let (_dir, config, mut world, id, target, weapon) =
            firing::fixture_with_target(source, None, &sources[6]).await;
        firing::edit(&mut world, id, |unit| {
            unit["target_lock"]["remaining"] = 3.into()
        });
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let initial =
            battle_aim_modifiers(&scripts.world(), id, target, weapon, 4, rules()).unwrap();
        assert!(initial.target_lock > 0);
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "targcomp", "3").unwrap();
        let tracked =
            battle_aim_modifiers(&scripts.world(), id, target, weapon, 4, rules()).unwrap();
        assert_eq!(tracked.target_lock, 0);
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "targcomp", "4").unwrap();
        let landed =
            battle_aim_modifiers(&scripts.world(), id, target, weapon, 4, rules()).unwrap();
        assert_eq!(landed.targeting_mode, 1);
        firing::edit(&mut scripts.world_mut(), target, |unit| {
            unit["vtol_flight"]["phase"] = serde_json::json!({"kind":"airborne"});
            unit["vtol_flight"]["altitude"] = 1.0.into();
        });
        let airborne =
            battle_aim_modifiers(&scripts.world(), id, target, weapon, 4, rules()).unwrap();
        assert_eq!(airborne.targeting_mode, -2);
    }
}

#[tokio::test]
async fn anti_air_tracks_shared_orbital_drops_and_installed_equipment() {
    for source in firing::templates().into_iter().take(6) {
        let (_dir, config, mut world, id, target, weapon) =
            firing::fixture_with_target(&source, None, &source).await;
        firing::edit(&mut world, target, |state| {
            state["orbital_drop"] =
                serde_json::to_value(OrbitalDrop::new(100 * 1024, 2).unwrap()).unwrap();
            state["ground_elevation"] = serde_json::Value::Null;
        });
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "targcomp", "4").unwrap();
        let aim = battle_aim_modifiers(&scripts.world(), id, target, weapon, 4, rules()).unwrap();
        assert_eq!(aim.targeting_mode, -2);
        assert_eq!(aim.orbital_drop, -2);
        firing::edit(&mut scripts.world_mut(), id, |state| {
            let attributes = &mut state["definition"]["attributes"];
            let specials = attributes["specials"]
                .as_str()
                .unwrap_or("-")
                .replace('-', "");
            attributes["specials"] = format!("{} AntiAircraft", specials.trim()).trim().into();
        });
        let equipped =
            battle_aim_modifiers(&scripts.world(), id, target, weapon, 4, rules()).unwrap();
        assert_eq!(equipped.targeting_mode, -3);
        assert_eq!(equipped.orbital_drop, -2);
        assert_eq!(equipped.subtotal(), aim.subtotal().map(|value| value - 1));
        let before = scripts.world().btech.clone();
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, before);
        assert_eq!(
            battle_aim_modifiers(&restored, id, target, weapon, 4, rules()).unwrap(),
            equipped
        );
    }
}

#[tokio::test]
async fn multi_target_side_arc_penalty_is_independent_of_lock_and_arc_override() {
    for source in firing::templates() {
        let (_dir, config, mut world, id, target, weapon) =
            firing::fixture_with_target(&source, None, &source).await;
        firing::edit(&mut world, id, |state| {
            state["motion"]["heading"] = 180.0.into();
            state["motion"]["desired_heading"] = 180.0.into();
            state["target_lock"]["remaining"] = 0.into();
        });
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let output = support::run_text(&scripts, &config, ObjectId(1), 1, "@setunit targcomp 3");
        assert!(output.is_empty(), "{output}");
        for override_weapon_arcs in [false, true] {
            let mut selected_rules = rules();
            selected_rules.override_weapon_arcs = override_weapon_arcs;
            let aim = battle_aim_modifiers(&scripts.world(), id, target, weapon, 4, selected_rules)
                .unwrap();
            assert_eq!(aim.target_lock, 0);
            assert_eq!(aim.targeting_mode, 1);
        }
    }
}
