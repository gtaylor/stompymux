//! Reference sensor text agrees between native and Lua inspection without mutating combat state.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[allow(dead_code)]
#[path = "support/btech_firing.rs"]
mod firing;
use crate::support;

/// Compact, verbose and wanted blocks share one read-only renderer on every supported chassis.
#[tokio::test]
async fn native_lua_sensor_layout_and_arcs_match_after_restart() {
    for (index, template) in firing::templates().iter().enumerate() {
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(template, None, template).await;
        let map = world.btech.units()[&unit].map.unwrap();
        set_battle_map_visibility(&mut world, map, BattleLight::Night, 30).unwrap();
        assert_eq!(
            battle_sensor_report(&world, unit, false).unwrap(),
            "Sensors: Vislight in 360 degree scanning mode (R:Visual)"
        );
        firing::edit(&mut world, unit, |s| {
            s["sensor_selection"] = serde_json::to_value(BattleSensorSelection {
                active: BattleSensorPair {
                    primary: BattleSensorMode::Visual,
                    secondary: BattleSensorMode::Infrared,
                },
                pending: Some(BattleSensorChange {
                    wanted: BattleSensorPair {
                        primary: BattleSensorMode::LightAmplification,
                        secondary: BattleSensorMode::LightAmplification,
                    },
                    remaining: 7,
                }),
            })
            .unwrap()
        });
        let arc = if index == 5 {
            "360 degree scanning mode"
        } else {
            "120 degree scanning mode (Forward arc)"
        };
        let wanted = "Wanted: Light-amplification in 360 degree scanning mode (R:Visual (Dawn/Dusk), 2x Visual (Night))";
        let compact = format!(
            "Sensors\r\n-------\r\nPrimary:   Vislight in {arc} (R:Visual)\r\nSecondary: Infrared (R:15)\r\n{wanted}"
        );
        let verbose = format!(
            "Sensors\r\n-------\r\nPrimary:   Vislight in {arc} \r\n\tRange:      Visual\r\n\tBlocked by: Fire/Smoke/Obstacles, 3 pt woods, 5 underwater hexes\r\n\tNotes:      Bad in night-fighting (BTH)\r\nSecondary: Infrared \r\n\tRange:      15\r\n\tBlocked by: Fire/Obstacles, 6 pt woods\r\n\tNotes:      Easy to hit 'hot' targets, not very efficient in forests (BTH)\r\n{wanted}"
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let restored = persistence::load(&config.database()).await.unwrap();
        for state in [world, restored] {
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(state))).unwrap();
            let before = scripts.world().btech.clone();
            for (argument, expected) in [
                ("", &compact),
                ("verbose", &verbose),
                ("anything", &verbose),
                ("V", &verbose),
            ] {
                let report =
                    battle_sensor_report(&scripts.world(), unit, !argument.is_empty()).unwrap();
                assert_eq!(&report, expected);
                let lua: String = scripts
                    .eval_callback(&format!(
                        "return btech.unit.sensor_report({}, {})",
                        unit.0,
                        !argument.is_empty()
                    ))
                    .unwrap();
                assert_eq!(lua, report);
                let output = support::run_text(
                    &scripts,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("sensor {argument}"),
                );
                assert_eq!(output.trim_end(), expected.trim_end());
            }
            assert_eq!(scripts.world().btech, before);
            assert_eq!(
                support::run_text(&scripts, &config, ObjectId(1), 1, "sensor V L I").trim(),
                "Invalid number of arguments!"
            );
        }
    }
}
