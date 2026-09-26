//! Station equipment inspection shares parent reports without granting pilot authority.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Every supported chassis exposes identical reports to its pilot and registered gunner.
#[tokio::test]
async fn stations_share_equipment_reports_and_preserve_control_boundaries() {
    for (index, template) in firing::templates().iter().enumerate() {
        let (_dir, config, mut world, parent, _, _) =
            firing::fixture_with_target(template, Some(BattleWeapon::MediumLaser), template).await;
        let station = world.create(&config, "Station".into(), Kind::Thing);
        let gunner = world.create(&config, "Gunner".into(), Kind::Player);
        register_gunner_station(&mut world, ObjectId(1), station, parent, 5).unwrap();
        world.objects.get_mut(&gunner).unwrap().location = Some(station);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let criticals = if index < 2 {
            "critstatus h"
        } else {
            "critstatus f"
        };
        let commands = ["weaponspecs", "weaponstatus", criticals];
        for command in commands {
            let text = support::run_text(&scripts, &config, gunner, 1, command);
            assert!(
                text.contains("hasn't been initialized"),
                "{command}: {text}"
            );
        }
        gunner_station_action(&scripts, station, gunner, true).unwrap();
        scripts.drain_outbox();
        let before = serde_json::to_value(&scripts.world().btech).unwrap();
        for command in commands {
            let expected = support::run_text(&scripts, &config, ObjectId(1), 1, command);
            let actual = support::run_text(&scripts, &config, gunner, 1, command);
            assert_eq!(actual, expected, "chassis {index}, {command}");
            assert!(!actual.contains("Invalid section"), "{actual}");
        }
        assert_eq!(
            serde_json::to_value(&scripts.world().btech).unwrap(),
            before
        );
        // Releasing controls must revoke report access, even while remaining inside.
        gunner_station_action(&scripts, station, gunner, false).unwrap();
        for command in commands {
            assert!(
                support::run_text(&scripts, &config, gunner, 1, command)
                    .contains("hasn't been initialized")
            );
        }
        // An ordinary passenger retains specifications but does not gain critical controls.
        scripts
            .world_mut()
            .objects
            .get_mut(&gunner)
            .unwrap()
            .location = Some(parent);
        assert_eq!(
            support::run_text(&scripts, &config, gunner, 1, "weaponspecs"),
            support::run_text(&scripts, &config, ObjectId(1), 1, "weaponspecs")
        );
        let denied = support::run_text(&scripts, &config, gunner, 1, criticals);
        assert!(denied.contains("pilot"), "{denied}");
    }
}

/// Conscious reports inspect the gunner's health and the parent's sensors, not station fields.
#[tokio::test]
async fn station_observation_guards_use_physical_parent() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, _, _) =
            firing::fixture_with_target(&template, Some(BattleWeapon::MediumLaser), &template)
                .await;
        let station = world.create(&config, "Station".into(), Kind::Thing);
        let gunner = world.create(&config, "Gunner".into(), Kind::Player);
        register_gunner_station(&mut world, ObjectId(1), station, parent, 0).unwrap();
        world.objects.get_mut(&gunner).unwrap().location = Some(station);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        gunner_station_action(&scripts, station, gunner, true).unwrap();
        let original = scripts.world().btech.clone();
        let mut state = serde_json::to_value(&original).unwrap();
        state["recoveries"][gunner.0.to_string()] = serde_json::json!({
            "remaining": 1, "pain_resistance": false, "toughness": false,
            "dice": BattleDice::seeded([34;32])
        });
        scripts.world_mut().btech = serde_json::from_value(state).unwrap();
        let before = scripts.world().btech.clone();
        for command in ["weaponstatus", "critstatus invalid", "status", "contacts"] {
            let text = support::run_text(&scripts, &config, gunner, 1, command);
            assert!(text.contains("unconscious"), "{text}");
        }
        assert!(
            support::run_text(&scripts, &config, gunner, 1, "weaponspecs")
                .contains("Weapons statistics for")
        );
        assert_eq!(scripts.world().btech, before);
    }
}

/// Station status reports parent facts with independent targeting through native and Lua access.
#[tokio::test]
async fn station_status_uses_own_selection_and_parent_state() {
    for template in firing::templates() {
        let (_dir, config, mut world, parent, _target, _) =
            firing::fixture_with_target(&template, Some(BattleWeapon::MediumLaser), &template)
                .await;
        let station = world.create(&config, "Station".into(), Kind::Thing);
        let gunner = world.create(&config, "Gunner".into(), Kind::Player);
        world.objects.get_mut(&gunner).unwrap().location = Some(station);
        register_gunner_station(&mut world, ObjectId(1), station, parent, 0).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        gunner_station_action(&scripts, station, gunner, true).unwrap();
        scripts.drain_outbox();
        let parent_status = battle_unit_status(&scripts.world(), parent, "info").unwrap();
        assert!(parent_status.contains("Target:") && parent_status.contains("Range:"));
        let station_status = support::run_text(&scripts, &config, gunner, 1, "status info");
        assert!(!station_status.contains("Target:"), "{station_status}");
        select_battle_hex_target(
            &mut scripts.world_mut(),
            station,
            gunner,
            BattleHexCoordinate { x: 0, y: 9 },
            BattleHexTargetMode::Hex,
        )
        .unwrap();
        let before = scripts.world().btech.clone();
        for options in [
            "", "info", "armor", "weapons", "heat", "short", "AIWH", "N", "NW",
        ] {
            let native =
                support::run_text(&scripts, &config, gunner, 1, &format!("status {options}"));
            let lua: String = scripts
                .eval_callback(&format!(
                    "return btech.gunner.status({}, {}, '{options}')",
                    station.0, gunner.0
                ))
                .unwrap();
            assert_eq!(native.trim(), lua.trim(), "{template}: {options}");
            if options == "info" {
                assert!(lua.contains("Target: Hex 0 9"), "{lua}");
                assert!(!lua.contains("Target in "), "{lua}");
            }
            if matches!(options, "armor" | "weapons" | "heat" | "N" | "NW") {
                assert_eq!(
                    lua,
                    battle_unit_status(&scripts.world(), parent, options).unwrap()
                );
            }
        }
        assert_eq!(scripts.world().btech, before);
        assert_eq!(
            battle_unit_status(&scripts.world(), parent, "info").unwrap(),
            parent_status
        );
        assert!(
            scripts
                .eval_callback::<String>(&format!(
                    "return btech.gunner.status({}, {}, 'info')",
                    parent.0, gunner.0
                ))
                .is_err()
        );
        // Persist the active station selection before exercising the real shutdown transition.
        let active: String = scripts
            .eval_callback(&format!(
                "return btech.gunner.status({}, {}, 'info')",
                station.0, gunner.0
            ))
            .unwrap();
        assert!(active.contains("Target: Hex 0 9"));
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        let restarted = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
        let reloaded: String = restarted
            .eval_callback(&format!(
                "return btech.gunner.status({}, {}, 'info')",
                station.0, gunner.0
            ))
            .unwrap();
        assert_eq!(reloaded, active);
        support::run_text(&scripts, &config, ObjectId(1), 1, "shutdown");
        let stopped: String = scripts
            .eval_callback(&format!(
                "return btech.gunner.status({}, {}, 'info')",
                station.0, gunner.0
            ))
            .unwrap();
        assert!(stopped.contains("SHUTDOWN"), "{stopped}");
        gunner_station_action(&scripts, station, gunner, false).unwrap();
        scripts.drain_outbox();
        assert!(
            support::run_text(&scripts, &config, gunner, 1, "status")
                .contains("hasn't been initialized")
        );
        scripts
            .world_mut()
            .objects
            .get_mut(&gunner)
            .unwrap()
            .location = Some(parent);
        assert_eq!(
            support::run_text(&scripts, &config, gunner, 1, "status"),
            support::run_text(&scripts, &config, ObjectId(1), 1, "status")
        );
    }
}
