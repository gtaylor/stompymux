//! Rotorcraft pod removal uses landed-state admission before crew busy checks.
use crate::support;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Native and Lua requests distinguish launch preparation from flight without altering rejected state.
#[tokio::test]
async fn vtol_pod_removal_requires_landing_and_preserves_rejection_order() {
    let (_dir, config, mut base, id, _, _) = firing::fixture_with_target(
        include_str!("../game/mechs/Kestrel.toml"),
        None,
        include_str!("../game/mechs/AS7-D.toml"),
    )
    .await;
    firing::edit(&mut base, id, |unit| {
        unit["beacons"] = serde_json::json!({"front":["narc","homing"]});
        unit["motion"]["speed"] = 0.0.into();
        unit["motion"]["desired_speed"] = 0.0.into();
    });
    for phase in ["landed", "launching", "airborne", "falling"] {
        let mut world = base.clone();
        firing::edit(&mut world, id, |unit| {
            unit["vtol_flight"]["phase"] = if phase == "launching" {
                serde_json::json!({"kind":"launching","remaining":2})
            } else {
                serde_json::json!({"kind":if phase == "falling" { "airborne" } else { phase }})
            };
            unit["vtol_flight"]["altitude"] = if matches!(phase, "airborne" | "falling") {
                5.0
            } else {
                0.0
            }
            .into();
        });
        if phase == "falling" {
            begin_battle_vehicle_descent(&mut world, id).unwrap();
        }
        world.validate(&config).unwrap();
        let accepted = matches!(phase, "landed" | "launching");
        let before = world.btech.clone();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let call = format!("btech.unit.removepods({},1)", id.0);
        let reply = support::run_text(&native, &config, ObjectId(1), 1, "removepods");
        let result = lua.eval_callback::<()>(&call);
        if accepted {
            assert!(
                reply.contains("You begin to systematically remove all the iNarc pods"),
                "{phase}: {reply}"
            );
            result.unwrap();
            assert_eq!(native.world().btech.vehicles()[&id].pod_removal(), Some(60));
            let aborted = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            assert!(
                aborted
                    .eval_callback::<()>(&format!("{call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(aborted.world().btech, before);
            assert!(aborted.drain_outbox().is_empty());
        } else {
            let message = "You must land before attempting to remove iNarc pods!";
            assert!(reply.contains(message), "{phase}: {reply}");
            assert!(format!("{:#}", result.unwrap_err()).contains(message));
            assert_eq!(native.world().btech, before);
            assert_eq!(lua.world().btech, before);
            // Motion rejects first; a crew busy condition must not obscure the landing refusal.
            for moving in [false, true] {
                let mut rejected = world.clone();
                firing::edit(&mut rejected, id, |unit| {
                    unit["motion"]["desired_speed"] = if moving { 1.0 } else { 0.0 }.into();
                    unit["crew_stun_remaining"] = 10.into();
                });
                let snapshot = rejected.btech.clone();
                let error = begin_battle_pod_removal(&mut rejected, id, ObjectId(1)).unwrap_err();
                assert_eq!(
                    error.to_string(),
                    if moving {
                        "You can not be moving when attempting to remove iNarc pods!"
                    } else {
                        message
                    }
                );
                assert_eq!(rejected.btech, snapshot);
            }
        }
        assert_eq!(native.world().btech, lua.world().btech);
        let saved = native.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            saved.btech
        );
    }
}
