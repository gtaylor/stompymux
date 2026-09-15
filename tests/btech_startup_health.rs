//! Startup health projection shares arithmetic across every supported cockpit family.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[allow(dead_code)]
#[path = "support/btech_firing.rs"]
mod firing;
mod support;

/// High health-derived counts survive startup; later tactical and IC injuries follow their own rules.
#[tokio::test]
async fn startup_health_replays_without_premature_crew_death() {
    for source in firing::templates() {
        for character in [false, true] {
            let (_dir, config, mut world, unit, _, _) =
                firing::fixture_with_target(&source, None, &source).await;
            firing::edit(&mut world, unit, |state| {
                state["power"] = serde_json::to_value(BattlePower::Off).unwrap();
                state["target_lock"] = serde_json::Value::Null
            });
            if character {
                world
                    .objects
                    .get_mut(&unit)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
            }
            set_battle_character(
                &mut world,
                ObjectId(1),
                BattleCharacter {
                    build: 1,
                    reflexes: 1,
                    intuition: 1,
                    learn: 1,
                    charisma: 1,
                    bruise: 10,
                    lethal: 2,
                },
            )
            .unwrap();
            let before = world.clone();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            assert!(
                scripts
                    .eval_callback::<()>(&format!("btech.unit.start({},1); error('abort')", unit.0))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, before.btech);
            let output = support::run_text(&scripts, &config, ObjectId(1), 1, "startup");
            assert!(output.contains("Startup Cycle commencing..."), "{output}");
            let mut started = scripts.world().clone();
            let check = |world: &World| {
                if let Some(state) = world.btech.constructed_units().get(&unit) {
                    assert_eq!(state.pilot_injuries(), 6);
                    assert!(!state.is_destroyed());
                    assert_eq!(state.pilot(), Some(ObjectId(1)));
                } else {
                    let state = &world.btech.vehicles()[&unit];
                    assert_eq!(state.pilot_injuries(), 6);
                    assert!(!state.is_destroyed());
                    assert_eq!(state.pilot(), Some(ObjectId(1)));
                }
                assert_eq!(world.btech.characters(), before.btech.characters());
                assert_eq!(world.btech.recoveries(), before.btech.recoveries());
            };
            check(&started);
            started.validate(&config).unwrap();
            persistence::save(&config.database(), &started)
                .await
                .unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            check(&restored);
            assert_eq!(restored.btech, started.btech);
            let lua = Scripts::new(&config, Rc::new(RefCell::new(before))).unwrap();
            lua.eval_callback::<()>(&format!("btech.unit.start({},1)", unit.0))
                .unwrap();
            assert_eq!(lua.world().btech, started.btech);
            if character {
                let report = injure_battle_character_pilot(&mut started, unit, 1, false).unwrap();
                assert!(!report.injury.fatal);
                let status = started
                    .btech
                    .constructed_units()
                    .get(&unit)
                    .and_then(|u| u.character_pilot_status())
                    .or_else(|| {
                        started
                            .btech
                            .vehicles()
                            .get(&unit)
                            .and_then(|u| u.character_pilot_status())
                    })
                    .unwrap();
                assert_eq!(status.injuries, 7);
            } else {
                let report = injure_battle_tactical_pilot(&mut started, unit, 1, false).unwrap();
                assert!(report.killed);
                assert_eq!(report.injuries, 7);
            }
            let inspection = Scripts::new(&config, Rc::new(RefCell::new(started.clone()))).unwrap();
            let report =
                view_battle_unit_fields_action(&inspection, &config, ObjectId(1), unit, "pilotdam")
                    .unwrap();
            assert_eq!(report.fields[0].value.as_deref(), Some("7"));
            let lua_value: String = inspection
                .eval_callback(&format!(
                    "return btech.unit.fields(1,{},'pilotdam').fields[1].value",
                    unit.0
                ))
                .unwrap();
            assert_eq!(lua_value, "7");
            if character {
                let status = text::plain(&battle_unit_status(&started, unit, "").unwrap());
                if started.btech.vehicles().get(&unit).is_some_and(|unit| {
                    unit.definition().movement == BattleVehicleMovement::Stationary
                }) {
                    assert!(!status.contains("Pilot Injury:"));
                } else {
                    assert!(
                        status.lines().any(|line| line.contains("Pilot Injury: 7")),
                        "{status}"
                    );
                }
                let native =
                    support::run_text(&inspection, &config, ObjectId(1), 1, "@viewmech pilotdam");
                assert!(
                    native.contains("pilotdam") && native.contains('7'),
                    "{native}"
                );
            }
            assert_eq!(inspection.world().btech, started.btech);
            if character {
                let mut empty = started.clone();
                release_battle_pilot(&mut empty, unit, ObjectId(1)).unwrap();
                let edits = Scripts::new(&config, Rc::new(RefCell::new(empty))).unwrap();
                set_battle_unit_field_action(&edits, &config, ObjectId(1), unit, "pilotdam", "0")
                    .unwrap();
                let cleared = edits.world();
                let state = cleared
                    .btech
                    .constructed_units()
                    .get(&unit)
                    .map(|unit| (unit.pilot_injuries(), unit.character_pilot_status()))
                    .or_else(|| {
                        cleared
                            .btech
                            .vehicles()
                            .get(&unit)
                            .map(|unit| (unit.pilot_injuries(), unit.character_pilot_status()))
                    })
                    .unwrap();
                assert_eq!(state.0, 0);
                assert_eq!(state.1.unwrap().injuries, 0);
            }
            started.validate(&config).unwrap();
            persistence::save(&config.database(), &started)
                .await
                .unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                started.btech
            );
        }
    }
}
