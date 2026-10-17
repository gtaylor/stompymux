//! Shutdown across chassis: a Mech shuts down only once stopped on the ground, while vehicles
//! still tumble from speed and airborne units keep their restart-safe descent.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Set a legal actual speed while leaving the requested throttle at rest.
fn speed(world: &mut World, id: ObjectId, value: f64) {
    firing::edit(world, id, |state| {
        state["motion"]["speed"] = value.into();
        state["motion"]["desired_speed"] = 0.into();
    });
}

/// Whether the chassis at this index of [`firing::templates`] is a Mech.
fn mech(chassis: usize) -> bool {
    chassis < 2
}

/// A Mech shuts down voluntarily only in the End Phase, after movement (Total Warfare p. 160):
/// the command and Lua refuse it at any forward or reverse speed and change nothing, and a
/// stopped Mech shuts down with no roll. A vehicle above one movement point still tumbles;
/// reverse or slower motion does not.
#[tokio::test]
async fn shutdown_speed_boundaries_share_native_lua_and_restart() {
    for (chassis, source) in firing::templates().into_iter().enumerate() {
        for value in [0.0, 10.75, 10.7501, 21.5, -21.5] {
            if chassis == 5 && value != 0.0 {
                continue;
            }
            let (_dir, config, mut world, id, _, _) = firing::fixture_with_target(
                &source,
                None,
                include_str!("../game/units/AS7-D.toml"),
            )
            .await;
            speed(&mut world, id, value);
            world.validate(&config).unwrap();
            let before = world.btech.clone();
            persistence::save(&config.database(), &world).await.unwrap();
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let call = format!("btech.unit.stop({},1)", id.0);
            if mech(chassis) && value != 0.0 {
                let output = support::run_text(&native, &config, ObjectId(1), 1, "shutdown");
                assert!(
                    output.contains("You must come to a stop before you can shut down."),
                    "{chassis} {value}: {output}"
                );
                assert!(!output.contains("All systems shut down."), "{output}");
                assert_eq!(native.world().btech, before);
                let error = lua
                    .eval_callback::<bool>(&format!("return {call}"))
                    .unwrap_err();
                assert!(format!("{error:#}").contains("come to a stop"), "{error:#}");
                assert_eq!(lua.world().btech, before);
                continue;
            }
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort shutdown')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            assert!(
                lua.eval_callback::<bool>(&format!("return {call}"))
                    .unwrap()
            );
            let output = support::run_text(&native, &config, ObjectId(1), 1, "shutdown");
            assert!(
                output.contains("All systems shut down."),
                "{chassis} {value}: {output}"
            );
            assert_eq!(
                output.contains("mid-motion"),
                value > 10.75,
                "{chassis} {value}: {output}"
            );
            assert_eq!(native.world().btech, lua.world().btech);
            let key = if before.vehicles().contains_key(&id) {
                "vehicles"
            } else {
                "constructed"
            };
            let old = serde_json::to_value(&before).unwrap();
            let saved = native.world().clone();
            let new = serde_json::to_value(&saved.btech).unwrap();
            assert_eq!(new[key][id.0.to_string()]["power"]["state"], "off");
            assert!(new[key][id.0.to_string()]["pilot"].is_null());
            if value <= 10.75 {
                assert_eq!(
                    new[key][id.0.to_string()]["dice"],
                    old[key][id.0.to_string()]["dice"]
                );
            } else {
                assert_ne!(
                    new[key][id.0.to_string()]["dice"],
                    old[key][id.0.to_string()]["dice"]
                );
            }
            saved.validate(&config).unwrap();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
    }
}

/// A jumping Mech must land before it shuts down, and the refusal changes nothing; airborne
/// vehicles still shut down into the existing descent path rather than a ground tumble, and
/// aborting startup never falls.
#[tokio::test]
async fn airborne_shutdown_preserves_descent_and_startup_abort_does_not_fall() {
    for chassis in [0, 2, 4, 6] {
        let (_dir, config, mut world, id, _, _) = firing::fixture_with_target(
            &firing::templates()[chassis],
            None,
            include_str!("../game/units/AS7-D.toml"),
        )
        .await;
        if chassis == 0 {
            launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
        } else {
            firing::edit(&mut world, id, |state| {
                if chassis == 6 {
                    state["vtol_flight"]["phase"] = serde_json::json!({"kind": "airborne"});
                    state["vtol_flight"]["altitude"] = 3.into();
                } else {
                    state["ground_elevation"] = 3.into();
                }
            });
            speed(&mut world, id, 21.5);
        }
        let original = world.btech.clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        if mech(chassis) {
            let error = stop_battle_unit_action(&scripts, &config, id, ObjectId(1)).unwrap_err();
            assert!(
                format!("{error:#}").contains("You must land before you can shut down."),
                "{error:#}"
            );
            assert_eq!(scripts.world().btech, original);
            assert!(scripts.drain_outbox().is_empty());
            let lua = format!("return btech.unit.stop({},1)", id.0);
            let error = scripts.eval_callback::<bool>(&lua).unwrap_err();
            assert!(format!("{error:#}").contains("land before"), "{error:#}");
            assert_eq!(scripts.world().btech, original);
            continue;
        }
        let notices = stop_battle_unit_action(&scripts, &config, id, ObjectId(1)).unwrap();
        assert!(
            notices
                .iter()
                .any(|notice| notice.text.contains("free-fall"))
        );
        assert!(
            !notices
                .iter()
                .any(|notice| notice.text.contains("mid-motion"))
        );
        let saved = scripts.world().clone();
        let before = serde_json::to_value(original).unwrap();
        let after = serde_json::to_value(&saved.btech).unwrap();
        assert_eq!(
            before["vehicles"][id.0.to_string()]["dice"],
            after["vehicles"][id.0.to_string()]["dice"]
        );
        saved.validate(&config).unwrap();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
    for source in firing::templates() {
        let (_dir, config, mut world, id, _, _) =
            firing::fixture_with_target(&source, None, include_str!("../game/units/AS7-D.toml"))
                .await;
        firing::edit(&mut world, id, |state| {
            state["target_lock"] = serde_json::Value::Null;
            state["power"] = serde_json::to_value(Power::Starting { remaining: 10 }).unwrap();
        });
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let notices = stop_battle_unit_action(&scripts, &config, id, ObjectId(1)).unwrap();
        assert!(notices.iter().any(|notice| notice.text.contains("aborted")));
        assert!(
            !notices
                .iter()
                .any(|notice| notice.text.contains("fall") || notice.text.contains("mid-motion"))
        );
        scripts.world().validate(&config).unwrap();
    }
}

/// A Mech that jumped shuts down once it has landed and stopped, with no fall and no roll
/// (Total Warfare p. 160).
#[tokio::test]
async fn landed_and_stopped_mech_shuts_down_without_a_roll() {
    let (_dir, config, mut world, id, _, _) = firing::fixture_with_target(
        &firing::templates()[0],
        None,
        include_str!("../game/units/AS7-D.toml"),
    )
    .await;
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    let mut jumping = world.clone();
    let error = stop_battle_unit(&mut jumping, id, ObjectId(1), FallRules::STANDARD).unwrap_err();
    assert!(format!("{error:#}").contains("land before"), "{error:#}");
    assert_eq!(jumping.btech, world.btech);
    while world.btech.constructed_units()[&id].flight().is_some() {
        advance_battle_jumps(&mut world, MovementRules::STANDARD).unwrap();
    }
    speed(&mut world, id, 0.0);
    let before = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap();
    let notices = stop_battle_unit(&mut world, id, ObjectId(1), FallRules::STANDARD).unwrap();
    assert_eq!(
        notices
            .iter()
            .map(|notice| notice.text.as_str())
            .collect::<Vec<_>>(),
        ["All systems shut down."]
    );
    let after = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap();
    assert_eq!(after["power"]["state"], "off");
    assert_eq!(after["dice"], before["dice"]);
    assert_eq!(after["posture"], before["posture"]);
    assert_eq!(after["sections"], before["sections"]);
    world.validate(&config).unwrap();
}
