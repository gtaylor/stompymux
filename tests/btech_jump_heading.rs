//! Jump facing uses shared rotation without steering the committed flight path.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

#[tokio::test]
async fn jump_heading_rates_preserve_trajectory_and_replay_across_chassis() {
    for quad in [false, true] {
        for gravity in [50, 100, 200] {
            for fasa in [false, true] {
                let (_dir, config, mut world) = support::isolated_world().await;
                let map = world.create(&config, "Jump field".into(), Kind::Room);
                create_battle_map(
                    &mut world,
                    map,
                    "jump",
                    MapAsset::from_cells(&format!(
                        "12 12\n{}",
                        format!("{}\n", ".0".repeat(12)).repeat(12)
                    ))
                    .unwrap(),
                )
                .unwrap();
                support::seed_object_dice(&mut world, map, support::FIXTURE_DICE_SEED);
                let id = world.create(&config, "Jumper".into(), Kind::Thing);
                let mut definition = MechTemplate::parse(
                    "test",
                    if quad {
                        include_str!("../game/units/SCP-1N.toml")
                    } else {
                        include_str!("fixtures/btech/units/JR7-D.toml")
                    },
                )
                .unwrap();
                definition.jump_speed = 53.75;
                create_battle_unit(&mut world, id, definition).unwrap();
                support::seed_object_dice(&mut world, id, support::FIXTURE_DICE_SEED);
                place_battle_unit(&mut world, id, map, 5, 5).unwrap();
                world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
                assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
                support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
                start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
                for _ in 0..5 {
                    advance_battle_units(&mut world, 0);
                }
                let mut saved = serde_json::to_value(&world.btech).unwrap();
                saved["maps"][map.0.to_string()]["gravity"] = serde_json::json!(gravity);
                saved["constructed"][id.0.to_string()]["motion"]["heading"] =
                    serde_json::json!(350.0);
                saved["constructed"][id.0.to_string()]["motion"]["desired_heading"] =
                    serde_json::json!(350.0);
                world.btech = serde_json::from_value(saved).unwrap();
                launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
                let mut straight = world.clone();
                let rate = if fasa {
                    18.0
                } else {
                    3.0 * (500 / gravity) as f64 * if quad { 2.0 } else { 1.0 }
                };
                let desired = (350.0 + rate * 2.0 + 3.0) % 360.0;
                let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                let native = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                let call = format!("btech.unit.heading({},1,{desired})", id.0);
                let before = scripts.world().btech.clone();
                assert!(
                    scripts
                        .eval_callback::<()>(&format!("{call}; error('abort')"))
                        .is_err()
                );
                assert_eq!(scripts.world().btech, before);
                assert!(scripts.drain_outbox().is_empty());
                scripts.eval_callback::<()>(&call).unwrap();
                let text = support::run_text(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("heading {desired}"),
                );
                assert!(text.contains("Desired heading"), "{text}");
                assert_eq!(scripts.world().btech, native.world().btech);
                let fields =
                    view_battle_unit_fields_action(&scripts, &config, ObjectId(1), id, "jump")
                        .unwrap();
                assert_eq!(fields.fields[0].value.as_deref(), Some("0"));
                assert_eq!(fields.fields[1].value.as_deref(), Some("645"));
                let mut turning = scripts.world().clone();
                persistence::save(&config.database(), &turning)
                    .await
                    .unwrap();
                let mut replay = persistence::load(&config.database()).await.unwrap();
                let rules = MovementRules {
                    fasa_turning: fasa,
                    ..MovementRules::STANDARD
                };
                for tick in 1..=4 {
                    let before = turning.btech.constructed_units()[&id].clone();
                    advance_battle_motion(&mut turning, rules).unwrap();
                    advance_battle_motion(&mut replay, rules).unwrap();
                    let unit = &turning.btech.constructed_units()[&id];
                    assert_eq!(unit.flight(), before.flight());
                    assert_eq!(unit.motion().unwrap().point, before.motion().unwrap().point);
                    let expected = (350.0 + (rate * f64::from(tick)).min(rate * 2.0 + 3.0)) % 360.0;
                    assert!((unit.motion().unwrap().heading - expected).abs() < 1e-9);
                    advance_battle_jumps(&mut turning, rules).unwrap();
                    advance_battle_jumps(&mut replay, rules).unwrap();
                    advance_battle_jumps(&mut straight, rules).unwrap();
                    assert_eq!(turning.btech, replay.btech);
                    assert_eq!(
                        turning.btech.constructed_units()[&id].flight(),
                        straight.btech.constructed_units()[&id].flight()
                    );
                    assert_eq!(
                        turning.btech.constructed_units()[&id].position(),
                        straight.btech.constructed_units()[&id].position()
                    );
                }
                if !quad {
                    let mut damaged = turning.clone();
                    let jet = damaged.btech.constructed_units()[&id]
                        .loadout()
                        .unwrap()
                        .systems
                        .iter()
                        .find(|part| part.system == System::JumpJet)
                        .unwrap()
                        .location;
                    destroy_battle_critical(&mut damaged, id, jet).unwrap();
                    let old = damaged.btech.constructed_units()[&id]
                        .motion()
                        .unwrap()
                        .heading;
                    set_battle_heading(&mut damaged, id, ObjectId(1), (old + 90.0) % 360.0)
                        .unwrap();
                    let capacity = damaged.btech.constructed_units()[&id]
                        .jump_capacity(gravity)
                        .unwrap();
                    advance_battle_motion(&mut damaged, rules).unwrap();
                    let rate = if fasa {
                        18.0
                    } else {
                        3.0 * f64::from(capacity.movement_points)
                    };
                    assert!(
                        (damaged.btech.constructed_units()[&id]
                            .motion()
                            .unwrap()
                            .heading
                            - (old + rate) % 360.0)
                            .abs()
                            < 1e-9
                    );
                    damaged.validate(&config).unwrap();
                }
                turning.validate(&config).unwrap();
            }
        }
    }
}
