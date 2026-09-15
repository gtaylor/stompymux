//! Facing controls share ordinary turning rates while forced descent retains its own cursor.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
mod support;

#[tokio::test]
async fn falling_units_turn_without_translation_and_replay_shared_chassis_rates() {
    for chassis in ["biped", "quad", "tracked", "wheeled", "hover", "vtol"] {
        for fasa in [false, true] {
            let (_dir, config, mut world) = support::isolated_world().await;
            let map = world.create(&config, "Facing field".into(), Kind::Room);
            create_battle_map(
                &mut world,
                map,
                "facing",
                BattleMapAsset::parse("3 3\n.0.0.0\n.0.0.0\n.0.0.0\n").unwrap(),
            )
            .unwrap();
            let id = world.create(&config, "Falling unit".into(), Kind::Thing);
            world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
            let mech = matches!(chassis, "biped" | "quad");
            if mech {
                create_battle_unit(
                    &mut world,
                    id,
                    BattleTemplate::parse(if chassis == "quad" {
                        include_str!("../game/mechs/SCP-1N")
                    } else {
                        include_str!("../game/mechs/JR7-D")
                    })
                    .unwrap(),
                )
                .unwrap();
            } else {
                let text =
                    match chassis {
                        "vtol" => include_str!("../game/mechs/Kestrel").to_owned(),
                        "wheeled" => include_str!("../game/mechs/Demolisher")
                            .replace("{ Track }", "{ Wheel }"),
                        "hover" => include_str!("../game/mechs/Demolisher")
                            .replace("{ Track }", "{ Hover }"),
                        _ => include_str!("../game/mechs/Demolisher").to_owned(),
                    };
                create_battle_vehicle(&mut world, id, BattleVehicleTemplate::parse(&text).unwrap())
                    .unwrap();
            }
            place_battle_unit(&mut world, id, map, 1, 1).unwrap();
            world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
            assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
            start_battle_unit(&mut world, id, ObjectId(1), false).unwrap();
            for _ in 0..30 {
                advance_battle_units(&mut world, 0);
            }
            let maximum = if mech {
                world.btech.constructed_units()[&id]
                    .mobility()
                    .maximum_speed
            } else {
                world.btech.vehicles()[&id].maximum_speed()
            };
            let rate = 1.5 * maximum / 10.75 * if chassis == "quad" { 2.0 } else { 1.0 };
            let desired = (350.0 + 2.0 * rate + 1.0) % 360.0;
            let key = if mech { "constructed" } else { "vehicles" };
            let mut state = serde_json::to_value(&world.btech).unwrap();
            let unit = &mut state[key][id.0.to_string()];
            unit["motion"]["heading"] = 350.0.into();
            unit["motion"]["desired_heading"] = 350.0.into();
            let ground = state.clone();
            let unit = &mut state[key][id.0.to_string()];
            let fall = serde_json::to_value(BattleFreeFall::new(50)).unwrap();
            if chassis == "vtol" {
                unit["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
                    phase: BattleVtolFlightPhase::Falling,
                    altitude: 50.0,
                    vertical_speed: 0.0,
                    fall: Some(BattleFreeFall::new(50)),
                })
                .unwrap();
            } else {
                unit["free_fall"] = fall;
                unit["ground_elevation"] = serde_json::Value::Null;
            }
            world.btech = serde_json::from_value(state).unwrap();
            world.validate(&config).unwrap();
            let rules = BattleMovementRules {
                fasa_turning: fasa,
                ..BattleMovementRules::STANDARD
            };
            // The same zero-speed ground turn includes the quad multiplier.
            if chassis != "vtol" {
                let mut trial = world.clone();
                trial.btech = serde_json::from_value(ground).unwrap();
                set_battle_heading(&mut trial, id, ObjectId(1), desired).unwrap();
                advance_battle_motion(&mut trial, rules).unwrap();
                let heading = serde_json::to_value(&trial.btech).unwrap()[key][id.0.to_string()]["motion"]["heading"].as_f64().unwrap();
                assert!(
                    (heading - (350.0 + rate) % 360.0).abs() < 1e-8,
                    "{chassis} {fasa} {heading} {rate}"
                );
            }
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            assert!(
                scripts
                    .eval_callback::<()>(&format!(
                        "btech.unit.heading({},1,{desired}); error('abort')",
                        id.0
                    ))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, world.btech);
            assert!(scripts.drain_outbox().is_empty());
            assert!(set_battle_heading(&mut world, id, ObjectId(2), desired).is_err());
            set_battle_heading(&mut world, id, ObjectId(1), desired).unwrap();
            scripts
                .eval_callback::<()>(&format!("btech.unit.heading({},1,{desired})", id.0))
                .unwrap();
            assert_eq!(scripts.world().btech, world.btech);
            world.validate(&config).unwrap();
            if chassis == "vtol" {
                let seed = (0..100)
                    .find(|seed| BattleDice::seeded([*seed; 32]).die(2).unwrap() == 2)
                    .unwrap();
                let mut recovered = world.clone();
                let mut state = serde_json::to_value(&recovered.btech).unwrap();
                state[key][id.0.to_string()]["vtol_flight"]["fall"]["remaining"] = 1.into();
                state[key][id.0.to_string()]["vtol_flight"]["fall"]["speed"] = 0.into();
                recovered.btech = serde_json::from_value(state).unwrap();
                advance_battle_motion(&mut recovered, rules).unwrap();
                assert_eq!(
                    recovered.btech.vehicles()[&id].vtol_flight().unwrap().phase,
                    BattleVtolFlightPhase::Airborne
                );
                assert!(
                    (recovered.btech.vehicles()[&id].motion().unwrap().heading
                        - (350.0 + rate) % 360.0)
                        .abs()
                        < 1e-8
                );
                for remaining in [0, -1] {
                    let mut empty = world.clone();
                    let mut state = serde_json::to_value(&empty.btech).unwrap();
                    state[key][id.0.to_string()]["vtol_fuel"]["remaining"] = remaining.into();
                    state[key][id.0.to_string()]["dice"] =
                        serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
                    empty.btech = serde_json::from_value(state).unwrap();
                    let before = empty.clone();
                    let notices = advance_battle_motion(&mut empty, rules).unwrap();
                    assert_eq!(empty.btech.vehicles()[&id].motion().unwrap().heading, 350.0);
                    assert_eq!(
                        notices
                            .iter()
                            .filter(|notice| notice.text.contains("run out of fuel"))
                            .count(),
                        usize::from(remaining == 0)
                    );
                    let mut replay = before;
                    assert_eq!(advance_battle_motion(&mut replay, rules).unwrap(), notices);
                    assert_eq!(replay.btech, empty.btech);
                }
            }
            persistence::save(&config.database(), &world).await.unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            for tick in 1..=3 {
                let mut straight = world.clone();
                let mut state = serde_json::to_value(&straight.btech).unwrap();
                let motion = &mut state[key][id.0.to_string()]["motion"];
                motion["desired_heading"] = motion["heading"].clone();
                straight.btech = serde_json::from_value(state).unwrap();
                advance_battle_motion(&mut straight, rules).unwrap();
                let mut expected = serde_json::to_value(&straight.btech).unwrap();
                if chassis == "vtol" {
                    let mut aircraft = straight.btech.vehicles()[&id].clone();
                    let flight = aircraft.vtol_flight().unwrap();
                    let fuel = aircraft
                        .consume_vtol_fuel(
                            flight.vertical_speed,
                            flight.altitude as i32,
                            false,
                            rules.free_fusion_vtol_fuel,
                        )
                        .unwrap();
                    assert!(!matches!(fuel, BattleVtolFuelUse::Exhausted { .. }));
                    expected[key][id.0.to_string()] = serde_json::to_value(aircraft).unwrap();
                }
                expected[key][id.0.to_string()]["motion"]["desired_heading"] = desired.into();
                expected[key][id.0.to_string()]["motion"]["heading"] =
                    ((350.0 + (rate * f64::from(tick)).min(2.0 * rate + 1.0)) % 360.0).into();
                advance_battle_motion(&mut world, rules).unwrap();
                advance_battle_motion(&mut restored, rules).unwrap();
                assert_eq!(world.btech, restored.btech);
                let actual = serde_json::to_value(&world.btech).unwrap();
                assert_eq!(actual, expected, "{chassis} {fasa} tick {tick}");
                world.validate(&config).unwrap();
            }
        }
    }
}
