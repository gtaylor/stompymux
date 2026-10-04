//! Shutdown shares mechanical falls, host casualty publication and restart-safe descent across chassis.
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

/// Positive speed strictly above one movement point triggers a fall; reverse speed does not.
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
                include_str!("../game/mechs/AS7-D.toml"),
            )
            .await;
            speed(&mut world, id, value);
            world.validate(&config).unwrap();
            let before = world.btech.clone();
            persistence::save(&config.database(), &world).await.unwrap();
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let call = format!("btech.unit.stop({},1)", id.0);
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

/// Airborne shutdown schedules the existing descent path instead of applying a ground tumble.
#[tokio::test]
async fn airborne_shutdown_preserves_descent_and_startup_abort_does_not_fall() {
    for chassis in [0, 2, 4, 6] {
        let (_dir, config, mut world, id, _, _) = firing::fixture_with_target(
            &firing::templates()[chassis],
            None,
            include_str!("../game/mechs/AS7-D.toml"),
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
        let key = if chassis == 0 {
            "constructed"
        } else {
            "vehicles"
        };
        let before = serde_json::to_value(original).unwrap();
        let after = serde_json::to_value(&saved.btech).unwrap();
        assert_eq!(
            before[key][id.0.to_string()]["dice"],
            after[key][id.0.to_string()]["dice"]
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
            firing::fixture_with_target(&source, None, include_str!("../game/mechs/AS7-D.toml"))
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

/// A lethal shutdown fall must publish the same crew evacuation as ordinary combat, atomically.
#[tokio::test]
async fn shutdown_fall_casualties_and_failed_evacuation_are_atomic() {
    let (_dir, config, mut world, id, _, _) = firing::fixture_with_target(
        &firing::templates()[0],
        None,
        include_str!("../game/mechs/AS7-D.toml"),
    )
    .await;
    release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let pilot = ObjectId(2);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&pilot).unwrap().location = Some(id);
    set_battle_character(
        &mut world,
        pilot,
        Character {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    assign_battle_pilot(&mut world, id, pilot).unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    apply_damage_phase(
        &mut world,
        id,
        MechSection::Head,
        1000,
        DamagePhase::Armor { rear: false },
    )
    .unwrap();
    let internal = world.btech.constructed_units()[&id].sections()[&MechSection::Head].internal;
    apply_damage_phase(
        &mut world,
        id,
        MechSection::Head,
        internal - 1,
        DamagePhase::Internal,
    )
    .unwrap();
    speed(&mut world, id, 21.5);
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    let seed = (0..=255)
        .find(|seed| {
            let mut candidate = world.clone();
            firing::edit(&mut candidate, id, |state| {
                state["dice"] = serde_json::to_value(Dice::seeded([*seed; 32])).unwrap()
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(candidate))).unwrap();
            stop_battle_unit_action(&scripts, &config, id, pilot).is_ok()
                && scripts.world().objects[&pilot].location == Some(afterlife)
        })
        .unwrap();
    firing::edit(&mut world, id, |state| {
        state["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
    });
    let before = world.clone();
    world.objects.remove(&afterlife);
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    assert!(stop_battle_unit_action(&scripts, &config, id, pilot).is_err());
    assert_eq!(scripts.world().btech, before.btech);
    assert!(scripts.drain_outbox().is_empty());
    *scripts.world_mut() = before;
    stop_battle_unit_action(&scripts, &config, id, pilot).unwrap();
    assert_eq!(scripts.world().objects[&pilot].location, Some(afterlife));
    let saved = scripts.world().clone();
    saved.validate(&config).unwrap();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}
