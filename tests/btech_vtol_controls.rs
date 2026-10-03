//! Live pilot flight commands share domain controls and callback rollback.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Create, place and start an aircraft through the shared vehicle lifecycle.
async fn fixture() -> (tempfile::TempDir, Config, World, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Control field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "controls",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Control aircraft".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().location = Some(map);
    let pilot = world.objects.get_mut(&ObjectId(2)).unwrap();
    pilot.location = Some(id);
    pilot.flags.remove(Flag::Wizard);
    create_battle_vehicle(
        &mut world,
        id,
        BattleVehicleTemplate::parse("Kestrel", include_str!("../game/mechs/Kestrel.toml"))
            .unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    assign_battle_pilot(&mut world, id, ObjectId(2)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(2), false).unwrap();
    for _ in 0..30 {
        advance_battle_units(&mut world, 0);
    }
    world.validate(&config).unwrap();
    (dir, config, world, id)
}

/// A powered aircraft and trained pilot start south of a configurable obstacle.
async fn obstacle_fixture(source: &str) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut base, id) = fixture().await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(base))).unwrap();
    support::run_text(&scripts, &config, ObjectId(2), 1, "shutdown");
    base = scripts.world().clone();
    let map = base.create(&config, "Forest course".into(), Kind::Room);
    create_battle_map(
        &mut base,
        map,
        "forest",
        BattleMapAsset::from_cells(source).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut base, id, map, 0, 2).unwrap();
    base.objects.get_mut(&id).unwrap().location = Some(map);
    assign_battle_pilot(&mut base, id, ObjectId(2)).unwrap();
    start_battle_unit(&mut base, id, ObjectId(2), false).unwrap();
    for _ in 0..30 {
        advance_battle_units(&mut base, 0);
    }
    begin_battle_vtol_takeoff(&mut base, id, ObjectId(2), 0, false).unwrap();
    advance_battle_motion(&mut base, BattleMovementRules::STANDARD).unwrap();
    set_battle_character(
        &mut base,
        ObjectId(2),
        BattleCharacter {
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
    set_battle_character_value(
        &mut base,
        ObjectId(2),
        "Piloting-Aerospace",
        BattleCharacterValue {
            value: 2,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    let passenger = base.objects.get_mut(&ObjectId(1)).unwrap();
    passenger.location = Some(id);
    passenger.flags.insert(Flag::Connected);
    (dir, config, base, id, map)
}

/// Obstacle rolls stay between warning and outcome and belong only to the assigned pilot.
fn assert_obstacle_feedback(
    output: &[(ObjectId, String)],
    roll: Option<u8>,
    warning: &str,
    nested: &[BattlePilotNotice],
) {
    let is_roll = |text: &str| {
        text.starts_with("You make a piloting") || text.starts_with("Modified Pilot Skill:")
    };
    let feedback: Vec<_> = output.iter().filter(|(_, text)| is_roll(text)).collect();
    let initial = usize::from(roll.is_some()) * 2;
    assert_eq!(feedback.len(), initial + nested.len());
    assert_eq!(
        feedback[initial..]
            .iter()
            .map(|(who, text)| (*who, text.as_str()))
            .collect::<Vec<_>>(),
        nested
            .iter()
            .map(|notice| (notice.pilot, notice.text.as_str()))
            .collect::<Vec<_>>()
    );
    assert!(feedback.iter().all(|(who, _)| *who == ObjectId(2)));
    let Some(roll) = roll else {
        return;
    };
    let pilot: Vec<_> = output
        .iter()
        .filter(|(who, _)| *who == ObjectId(2))
        .map(|(_, text)| text.as_str())
        .collect();
    let start = pilot.iter().position(|text| *text == warning).unwrap();
    assert_eq!(pilot[start + 1], "You make a piloting skill roll!");
    assert!(pilot[start + 2].starts_with("Modified Pilot Skill: BTH "));
    assert!(pilot[start + 2].ends_with(&format!("\tRoll: {roll}")));
    assert!(
        output
            .iter()
            .any(|(who, text)| *who == ObjectId(1) && text == warning)
    );
}

/// Elevation collisions retain rollback terrain, pilotless admission and signed crew modifiers.
#[tokio::test]
async fn elevation_collision_rolls_back_and_preserves_signed_crash_severity() {
    // Previous terrain, rolled sum, assigned pilot, safe landing, signed crash severity.
    for (tile, roll, assigned, safe, levels) in [
        (".0", 12, true, true, 1),
        (".0", 2, true, false, 1),
        (".1", 2, true, false, 0),
        (".6", 7, true, false, -5),
        ("%0", 12, true, false, 1),
        ("%0", 2, false, true, 1),
        ("/3", 12, true, false, -2),
        ("~3", 12, true, false, 4),
    ] {
        let (_dir, config, mut world, id, map) =
            obstacle_fixture(&format!("1 3\n.0\n^9\n{tile}\n")).await;
        world
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let previous = world.btech.maps()[&map].base_hex(0, 2).unwrap();
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
            .unwrap();
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut encoded["vehicles"][id.0.to_string()];
        if !assigned {
            unit["pilot"] = serde_json::Value::Null;
        }
        unit["motion"]["speed"] = 129.0.into();
        unit["motion"]["desired_speed"] = 129.0.into();
        unit["vtol_flight"]["altitude"] = f64::from(previous.surface_height().max(0) + 1).into();
        unit["vtol_flight"]["vertical_speed"] = 0.0.into();
        unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        encoded["maps"][map.0.to_string()]["movement_modifier"] = 1000.into();
        world.btech = serde_json::from_value(encoded).unwrap();
        let before = world.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        replay
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let outcome = advance_battle_vtol_environment(
            &mut world,
            id,
            false,
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        assert_eq!(
            advance_battle_vtol_environment(
                &mut replay,
                id,
                false,
                BattleMovementRules::STANDARD.fall
            )
            .unwrap(),
            outcome
        );
        assert_eq!(replay.btech, world.btech);
        let BattleVtolEnvironment::Obstacle {
            path,
            fall,
            notices,
            ..
        } = outcome
        else {
            panic!("Expected elevation obstacle: {tile}");
        };
        assert!(matches!(
            path,
            BattleVtolPath::Contact {
                contact: BattleVtolSurfaceContact::Elevation,
                ..
            }
        ));
        assert_eq!(fall.is_none(), safe, "{tile}");
        let nested = fall
            .as_ref()
            .map(|fall| fall.pilot_notices.clone())
            .unwrap_or_default();
        let unit = &world.btech.vehicles()[&id];
        assert_eq!(unit.position(), before.btech.vehicles()[&id].position());
        assert_eq!(
            unit.vtol_flight().unwrap().phase,
            BattleVtolFlightPhase::Landed
        );
        assert_eq!(
            unit.vtol_flight().unwrap().altitude,
            f64::from(previous.surface_height())
        );
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        assert!(
            notices
                .iter()
                .any(|notice| notice.text == "You attempt to fly over elevation that is too high!")
        );
        if safe {
            assert_eq!(unit.motion().unwrap().desired_speed, 129.0);
            assert_eq!(unit.sections(), before.btech.vehicles()[&id].sections());
            assert!(
                notices
                    .iter()
                    .any(|notice| notice.text == "You land safely.")
            );
            if !assigned {
                assert_eq!(
                    serde_json::to_value(unit).unwrap()["dice"],
                    serde_json::to_value(&before.btech.vehicles()[&id]).unwrap()["dice"]
                );
            }
        } else {
            let fall = fall.unwrap();
            assert_eq!(fall.avoidance.unwrap().situational, levels, "{tile}");
            if levels <= 0 {
                assert_eq!(fall.damage, 0);
            }
            assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
            assert!(
                notices
                    .iter()
                    .any(|notice| notice.text
                        == "You crash into the obstacle and fall from the sky!")
            );
        }
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(before))).unwrap();
        advance_battle_motion_action(&scripts, &config, BattleMovementRules::STANDARD).unwrap();
        let messages: Vec<_> = scripts
            .drain_outbox()
            .into_iter()
            .map(|(who, document)| (who, text::plain(document.source())))
            .collect();
        assert_obstacle_feedback(
            &messages,
            assigned.then_some(roll),
            "You attempt to fly over elevation that is too high!",
            &nested,
        );
        let output = messages
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            output.contains("You attempt to fly over elevation that is too high!"),
            "{tile}: {output}"
        );
        assert!(
            output.contains(if safe {
                "You land safely."
            } else {
                "You crash into the obstacle and fall from the sky!"
            }),
            "{tile}: {output}"
        );
        assert!(!output.contains("CRASH! You smash your toy into the ground!"));
        scripts.world().validate(&config).unwrap();
    }
}

/// Connected-pilot forest avoidance and crashes use deterministic shared checks and persistence.
#[tokio::test]
async fn forest_entry_replays_pilot_avoidance_and_one_level_crashes() {
    let (_dir, config, base, id, map) = obstacle_fixture("1 3\n.0\n`0\n.0\n").await;
    // The connected pilot has target six: the forest's +5 makes a ten fail.
    for (connected, roll, safe) in [
        (true, 12, true),
        (true, 10, false),
        (true, 2, false),
        (false, 12, false),
    ] {
        let mut world = base.clone();
        let flags = &mut world.objects.get_mut(&ObjectId(2)).unwrap().flags;
        if connected {
            flags.insert(Flag::Connected);
        } else {
            flags.remove(Flag::Connected);
        }
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
            .unwrap();
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut encoded["vehicles"][id.0.to_string()];
        unit["motion"]["speed"] = 129.0.into();
        unit["motion"]["desired_speed"] = 129.0.into();
        unit["vtol_flight"]["altitude"] = 1.0.into();
        unit["vtol_flight"]["vertical_speed"] = 0.0.into();
        unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        encoded["maps"][map.0.to_string()]["movement_modifier"] = 1000.into();
        world.btech = serde_json::from_value(encoded).unwrap();
        let before = world.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        // Connections are session state, so restore the same pilot presence for replay.
        if connected {
            replay
                .objects
                .get_mut(&ObjectId(2))
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        let rules = BattleMovementRules::STANDARD.fall;
        let outcome = advance_battle_vtol_environment(&mut world, id, false, rules).unwrap();
        assert_eq!(
            advance_battle_vtol_environment(&mut replay, id, false, rules).unwrap(),
            outcome
        );
        assert_eq!(world.btech, replay.btech);
        let BattleVtolEnvironment::Obstacle { fall, notices, .. } = outcome else {
            panic!("Expected forest collision");
        };
        assert_eq!(fall.is_none(), safe);
        let nested = fall
            .as_ref()
            .map(|fall| fall.pilot_notices.clone())
            .unwrap_or_default();
        assert!(
            notices
                .iter()
                .any(|notice| notice.text == "You go where no flying thing has ever gone before..")
        );
        let unit = &world.btech.vehicles()[&id];
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
        if safe {
            assert!(
                notices
                    .iter()
                    .any(|notice| notice.text == "You stop in time!")
            );
            assert_eq!(unit.position(), before.btech.vehicles()[&id].position());
            assert_eq!(unit.vtol_flight().unwrap().altitude, 0.0);
            assert_eq!(
                unit.vtol_flight().unwrap().phase,
                BattleVtolFlightPhase::Airborne
            );
            assert_eq!(unit.sections(), before.btech.vehicles()[&id].sections());
        } else {
            assert!(
                notices
                    .iter()
                    .any(|notice| notice.text == "Eww.. You've a bad feeling about this.")
            );
            assert_eq!(
                fall.as_ref().unwrap().damage,
                u32::from((unit.definition().tons + 5) / 10)
            );
            assert_eq!(unit.position().unwrap().y, 1);
            assert_eq!(
                unit.vtol_flight().unwrap().phase,
                BattleVtolFlightPhase::Landed
            );
            assert!(unit.immobilized());
            assert!(
                notices
                    .iter()
                    .any(|notice| notice.text == "Your rotor has been destroyed!")
            );
        }
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(before))).unwrap();
        advance_battle_motion_action(&scripts, &config, BattleMovementRules::STANDARD).unwrap();
        let messages: Vec<_> = scripts
            .drain_outbox()
            .into_iter()
            .map(|(who, document)| (who, text::plain(document.source())))
            .collect();
        assert_obstacle_feedback(
            &messages,
            connected.then_some(roll),
            "You go where no flying thing has ever gone before..",
            &nested,
        );
        let output = messages
            .into_iter()
            .map(|(_, text)| text)
            .collect::<Vec<_>>()
            .join("\n");
        if connected {
            assert!(
                output.contains("You go where no flying thing has ever gone before.."),
                "{output}"
            );
            assert!(
                output.contains(if safe {
                    "You stop in time!"
                } else {
                    "Eww.. You've a bad feeling about this."
                }),
                "{output}"
            );
            assert!(!output.contains("CRASH! You smash your toy into the ground!"));
        }
        scripts.world().validate(&config).unwrap();
    }
}

/// Native and Lua controls honor engine fuel policy in flight and on the ground.
#[tokio::test]
async fn horizontal_fuel_gate_shares_policy_without_blocking_heading_or_readouts() {
    let (_dir, config, world, id) = fixture().await;
    for phase in ["landed", "launching", "airborne"] {
        for (ice, free, fuel, accepted) in [
            (true, false, 0, false),
            (true, true, 0, false),
            (false, false, 0, false),
            (false, true, 0, true),
            (true, false, 1, true),
        ] {
            let path = config.root.join("stompymux.toml");
            let mut settings: toml::Value =
                toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            settings
                .as_table_mut()
                .unwrap()
                .entry("battletech")
                .or_insert_with(|| toml::Value::Table(Default::default()))
                .as_table_mut()
                .unwrap()
                .insert(
                    "nofusionvtolfuel".into(),
                    toml::Value::Integer(i64::from(free)),
                );
            std::fs::write(&path, toml::to_string(&settings).unwrap()).unwrap();
            let config = Config::load(&config.root).unwrap();
            let mut world = world.clone();
            if phase != "landed" {
                begin_battle_vtol_takeoff(&mut world, id, ObjectId(2), 0, false).unwrap();
            }
            if phase == "airborne" {
                advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
            }
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    let unit = record;
                    unit["vtol_fuel"]["remaining"] = fuel.into();
                    unit["definition"]["attributes"]["specials"] =
                        if ice { "ICEEngine_Tech" } else { "" }.into();
                })
                .unwrap();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let output = support::run_text(&scripts, &config, ObjectId(2), 1, "speed 20");
            assert_eq!(
                output.contains("Desired speed changed to 20 KPH."),
                accepted,
                "{output}"
            );
            if !accepted {
                assert!(output.contains("You're out of fuel!"), "{output}");
                assert_eq!(scripts.world().btech, world.btech);
                let output = support::run_text(&scripts, &config, ObjectId(2), 1, "speed stop");
                assert!(output.contains("You're out of fuel!"), "{output}");
                assert_eq!(scripts.world().btech, world.btech);
            }
            let native = scripts.world().btech.clone();
            *scripts.world_mut() = world.clone();
            let lua = scripts.eval_callback::<()>(&format!("btech.unit.speed({},2,20)", id.0));
            assert_eq!(lua.is_ok(), accepted);
            assert_eq!(scripts.world().btech, native);
            scripts.drain_outbox();
            let output = support::run_text(&scripts, &config, ObjectId(2), 1, "heading 90");
            assert!(
                output.contains("Desired heading: 90.0 degrees."),
                "{output}"
            );
            let heading = scripts.world().btech.clone();
            *scripts.world_mut() = world.clone();
            if accepted {
                scripts
                    .eval_callback::<()>(&format!("btech.unit.speed({},2,20)", id.0))
                    .unwrap();
            }
            scripts
                .eval_callback::<()>(&format!("btech.unit.heading({},2,90)", id.0))
                .unwrap();
            assert_eq!(scripts.world().btech, heading);
            scripts.drain_outbox();
            assert!(
                scripts
                    .eval_callback::<()>(&format!(
                        "btech.unit.heading({},2,180); error('abort')",
                        id.0
                    ))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, heading);
            assert!(scripts.drain_outbox().is_empty());
            let readout = scripts
                .eval_callback::<f64>(&format!("return btech.unit.speed({},2)", id.0))
                .unwrap();
            assert_eq!(readout, world.btech.vehicles()[&id].motion().unwrap().speed);
            let saved = scripts.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(restored.btech, saved.btech);
        }
    }
}

/// Grounded throttle changes share command transactions without creating taxi movement.
#[tokio::test]
async fn grounded_throttle_is_shared_saved_and_cleared_at_liftoff() {
    let (_dir, config, mut world, id) = fixture().await;
    for launching in [false, true] {
        if launching {
            world
                .objects
                .get_mut(&ObjectId(2))
                .unwrap()
                .flags
                .insert(Flag::Wizard);
            begin_battle_vtol_takeoff(&mut world, id, ObjectId(2), 3, false).unwrap();
            world
                .objects
                .get_mut(&ObjectId(2))
                .unwrap()
                .flags
                .remove(Flag::Wizard);
        }
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let output = support::run_text(&scripts, &config, ObjectId(2), 1, "speed 20");
        assert!(
            output.contains("Desired speed changed to 20 KPH."),
            "{output}"
        );
        let output = support::run_text(&scripts, &config, ObjectId(2), 1, "heading 90");
        assert!(
            output.contains("Desired heading: 90.0 degrees."),
            "{output}"
        );
        let native = scripts.world().btech.clone();
        *scripts.world_mut() = world.clone();
        scripts
            .eval_callback::<()>(&format!("btech.unit.speed({},2,20)", id.0))
            .unwrap();
        scripts
            .eval_callback::<()>(&format!("btech.unit.heading({},2,90)", id.0))
            .unwrap();
        assert_eq!(scripts.world().btech, native);
        scripts.drain_outbox();
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.unit.speed({},2,30); error('abort')", id.0))
                .is_err()
        );
        assert_eq!(scripts.world().btech, native);
        assert!(scripts.drain_outbox().is_empty());
        let saved = scripts.world().clone();
        persistence::save(&config.database(), &saved).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, native);
        let before = restored.btech.vehicles()[&id].clone();
        advance_battle_motion(&mut restored, BattleMovementRules::STANDARD).unwrap();
        let unit = &restored.btech.vehicles()[&id];
        assert_eq!(unit.position(), before.position());
        assert_eq!(unit.motion(), before.motion());
        assert_eq!(unit.motion().unwrap().desired_speed, 20.0);
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        assert_eq!(unit.motion().unwrap().heading, 0.0);
        assert_eq!(unit.motion().unwrap().desired_heading, 90.0);
        if launching {
            for _ in 0..3 {
                advance_battle_motion(&mut restored, BattleMovementRules::STANDARD).unwrap();
            }
            let unit = &restored.btech.vehicles()[&id];
            assert_eq!(
                unit.vtol_flight().unwrap().phase,
                BattleVtolFlightPhase::Airborne
            );
            assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
            assert_eq!(unit.motion().unwrap().speed, 0.0);
            assert_eq!(unit.motion().unwrap().desired_heading, 90.0);
            assert_eq!(unit.vtol_flight().unwrap().vertical_speed, 60.0);
        }
    }
}

#[tokio::test]
async fn native_and_lua_flight_controls_share_state_permissions_and_rollback() {
    let (_dir, config, world, id) = fixture().await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let output = support::run_text(&scripts, &config, ObjectId(2), 1, "takeoff 3");
    assert!(output.contains("Insufficient access"), "{output}");
    assert_eq!(scripts.world().btech, world.btech);
    let output = support::run_text(&scripts, &config, ObjectId(2), 1, "takeoff");
    assert!(output.contains("takeoff sequence"), "{output}");
    let native = scripts.world().btech.clone();
    *scripts.world_mut() = world;
    assert!(
        scripts
            .eval_callback::<bool>(&format!("return btech.unit.takeoff({},2)", id.0))
            .unwrap()
    );
    assert_eq!(scripts.world().btech, native);
    scripts.drain_outbox();
    let _ = advance_battle_motion(&mut scripts.world_mut(), BattleMovementRules::STANDARD).unwrap();
    assert_eq!(
        scripts.world().btech.vehicles()[&id]
            .vtol_flight()
            .unwrap()
            .phase,
        BattleVtolFlightPhase::Airborne
    );
    assert!(
        scripts
            .eval_callback::<bool>(&format!("return btech.unit.vertical({},2,12)", id.0))
            .unwrap()
    );
    scripts.drain_outbox();
    let output = support::run_text(&scripts, &config, ObjectId(2), 1, "vertical");
    assert!(output.contains("12.00 KPH"), "{output}");
    assert_eq!(
        scripts
            .eval_callback::<f64>(&format!("return btech.unit.vertical({},2)", id.0))
            .unwrap(),
        12.0
    );
    let before = scripts.world().btech.clone();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.vertical({},2,15); error('abort')",
                id.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, before);
    assert!(scripts.drain_outbox().is_empty());
    let output = support::run_text(&scripts, &config, ObjectId(2), 1, "vertical 10000");
    assert!(output.contains("velocity budget"), "{output}");
    assert_eq!(scripts.world().btech, before);
    assert!(
        battle_vtol_vertical_readout(&scripts.world(), ObjectId(0), ObjectId(2), false).is_err()
    );
}

#[tokio::test]
async fn shared_landing_cancels_launch_and_resolves_touchdown_without_jump_checks() {
    let (_dir, config, mut world, id) = fixture().await;
    let _ = begin_battle_vtol_takeoff(&mut world, id, ObjectId(2), 0, false).unwrap();
    let notices =
        land_battle_jump(&mut world, id, ObjectId(2), BattleMovementRules::STANDARD).unwrap();
    assert!(notices.iter().any(|notice| notice.text
        == format!(
            "Launch aborted by {}.",
            text::escape(&world.objects[&ObjectId(2)].name)
        )));
    assert_eq!(
        world.btech.vehicles()[&id].vtol_flight().unwrap().phase,
        BattleVtolFlightPhase::Landed
    );
    let _ = begin_battle_vtol_takeoff(&mut world, id, ObjectId(2), 0, false).unwrap();
    let _ = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
    let _ = set_battle_vtol_vertical_speed(&mut world, id, ObjectId(2), 0.0, false).unwrap();
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let output = support::run_text(&scripts, &config, ObjectId(2), 1, "land");
    assert!(
        output.contains("You bring your VTOL to a safe landing."),
        "{output}"
    );
    scripts.world().validate(&config).unwrap();
    let notices =
        land_battle_jump(&mut world, id, ObjectId(2), BattleMovementRules::STANDARD).unwrap();
    assert!(
        notices
            .iter()
            .any(|notice| notice.text == "You bring your VTOL to a safe landing.")
    );
    let unit = &world.btech.vehicles()[&id];
    assert_eq!(
        unit.vtol_flight().unwrap().phase,
        BattleVtolFlightPhase::Landed
    );
    assert_eq!(unit.sections(), before.vehicles()[&id].sections());
    assert_eq!(
        serde_json::to_value(unit).unwrap()["dice"],
        serde_json::to_value(&before.vehicles()[&id]).unwrap()["dice"]
    );
}

/// Launch cancellation names the acting pilot literally through both host adapters.
#[tokio::test]
async fn launch_cancellation_preserves_literal_pilot_name_and_callback_rollback() {
    let (_dir, config, mut world, id) = fixture().await;
    let name = "[fg=red]Pilot[reset]";
    world.objects.get_mut(&ObjectId(2)).unwrap().name = name.into();
    begin_battle_vtol_takeoff(&mut world, id, ObjectId(2), 0, false).unwrap();
    let before = world.btech.clone();
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let call = format!("btech.unit.land({},2)", id.0);
    assert!(
        lua.eval_callback::<()>(&format!("{call}; error('abort')"))
            .is_err()
    );
    assert_eq!(lua.world().btech, before);
    assert!(lua.drain_outbox().is_empty());
    lua.eval_callback::<()>(&call).unwrap();
    let rows: Vec<_> = lua
        .drain_outbox()
        .into_iter()
        .map(|(_, document)| text::plain(document.source()))
        .collect();
    let expected = format!("Launch aborted by {name}.");
    assert!(rows.iter().any(|row| row.contains(&expected)), "{rows:?}");
    let reply = text::plain(&support::run_text(&native, &config, ObjectId(2), 1, "land"));
    assert!(reply.contains(&expected), "{reply}");
    assert_eq!(native.world().btech, lua.world().btech);
    let saved = native.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}

#[tokio::test]
async fn live_flight_persists_and_host_shutdown_descent_settles_after_restart() {
    let (_dir, config, world, id) = fixture().await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let output = support::run_text(&scripts, &config, ObjectId(2), 1, "takeoff");
    assert!(output.contains("takeoff sequence"));
    for _ in 0..10 {
        advance_battle_motion_action(&scripts, &config, BattleMovementRules::STANDARD).unwrap();
        scripts.world().validate(&config).unwrap();
    }
    assert!(
        scripts.world().btech.vehicles()[&id]
            .vtol_flight()
            .unwrap()
            .altitude
            > 4.0
    );
    let output = support::run_text(&scripts, &config, ObjectId(2), 1, "shutdown");
    assert!(output.contains("shut down"), "{output}");
    assert_eq!(
        scripts.world().btech.vehicles()[&id]
            .vtol_flight()
            .unwrap()
            .phase,
        BattleVtolFlightPhase::Falling
    );
    let snapshot = scripts.world().clone();
    persistence::save(&config.database(), &snapshot)
        .await
        .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    loaded.validate(&config).unwrap();
    assert_eq!(loaded.btech, snapshot.btech);
    let replay = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
    scripts.drain_outbox();
    for _ in 0..15 {
        advance_battle_motion_action(&scripts, &config, BattleMovementRules::STANDARD).unwrap();
        advance_battle_motion_action(&replay, &config, BattleMovementRules::STANDARD).unwrap();
        assert_eq!(scripts.world().btech, replay.world().btech);
        scripts.world().validate(&config).unwrap();
    }
    let flight = scripts.world().btech.vehicles()[&id].vtol_flight().unwrap();
    assert_eq!(flight.phase, BattleVtolFlightPhase::Landed);
    assert_eq!(flight.altitude, 0.0);
    assert!(flight.fall.is_none());
}

#[tokio::test]
async fn live_validation_rejects_impossible_flight_lifecycle_combinations() {
    let (_dir, config, mut world, id) = fixture().await;
    let _ = begin_battle_vtol_takeoff(&mut world, id, ObjectId(2), 0, false).unwrap();
    let _ = advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
    world.validate(&config).unwrap();
    let base = serde_json::to_value(&world.btech).unwrap();
    let mut waiting = world.clone();
    let mut saved = base.clone();
    saved["vehicles"][id.0.to_string()]["power"] = serde_json::to_value(BattlePower::Off).unwrap();
    saved["vehicles"][id.0.to_string()]["vtol_flight"]["vertical_speed"] = 0.0.into();
    saved["vehicles"][id.0.to_string()]["motion"]["speed"] = 0.0.into();
    waiting.btech = serde_json::from_value(saved).unwrap();
    waiting.validate(&config).unwrap();
    let before = waiting.btech.clone();
    advance_battle_motion(&mut waiting, BattleMovementRules::STANDARD).unwrap();
    assert_eq!(waiting.btech, before);
    assert!(waiting.btech.vehicles()[&id].vtol_motion_step(0).is_err());
    // Powered vertical travel cannot remain active after power is removed.
    let mut powered_vertical = base.clone();
    powered_vertical["vehicles"][id.0.to_string()]["power"] =
        serde_json::to_value(BattlePower::Off).unwrap();
    let mut invalid = world.clone();
    invalid.btech = serde_json::from_value(powered_vertical).unwrap();
    assert!(
        invalid
            .validate(&config)
            .unwrap_err()
            .to_string()
            .contains("requires power")
    );
    let mut saved = base.clone();
    saved["vehicles"][id.0.to_string()]["ground_elevation"] = serde_json::json!(0);
    let mut invalid = world.clone();
    invalid.btech = serde_json::from_value(saved).unwrap();
    let error = invalid.validate(&config).unwrap_err();
    assert!(
        error.to_string().contains("ground-vehicle elevation"),
        "{error}"
    );
}

#[tokio::test]
async fn live_character_crash_publishes_shared_crew_injury() {
    let (_dir, config, mut world, id) = fixture().await;
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    set_battle_character(
        &mut world,
        ObjectId(2),
        BattleCharacter {
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
    let _ = begin_battle_vtol_takeoff(&mut world, id, ObjectId(2), 0, false).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for _ in 0..12 {
        advance_battle_motion_action(&scripts, &config, BattleMovementRules::STANDARD).unwrap();
    }
    let mut unit = scripts.world().btech.vehicles()[&id].clone();
    let _ = unit
        .apply_rotor_hit(BattleRotorHit::Destroy, false)
        .unwrap();
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    let mut unit = serde_json::to_value(unit).unwrap();
    unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
    let mut saved = serde_json::to_value(&scripts.world().btech).unwrap();
    saved["vehicles"][id.0.to_string()] = unit;
    scripts.world_mut().btech = serde_json::from_value(saved).unwrap();
    scripts.world().validate(&config).unwrap();
    let mut rules = BattleMovementRules::STANDARD;
    rules.fall.vehicle_impact.criticals.enabled = false;
    for _ in 0..15 {
        advance_battle_motion_action(&scripts, &config, rules).unwrap();
        scripts.world().validate(&config).unwrap();
    }
    let world = scripts.world();
    let unit = &world.btech.vehicles()[&id];
    assert_eq!(
        unit.vtol_flight().unwrap().phase,
        BattleVtolFlightPhase::Landed
    );
    assert_eq!(
        u16::from(unit.pilot_injuries()),
        unit.character_pilot_status().unwrap().injuries
    );
    assert!(unit.character_pilot_status().unwrap().injuries > 0);
}

#[tokio::test]
async fn native_and_lua_asset_loading_admit_flying_and_stationary_aircraft() {
    let (dir, config, mut world) = support::isolated_world().await;
    std::fs::create_dir_all(dir.path().join("mechs")).unwrap();
    let mut ids = Vec::new();
    for name in ["Kestrel", "ObservationVTOL"] {
        std::fs::copy(
            format!("game/mechs/{name}.toml"),
            dir.path().join("mechs").join(format!("{name}.toml")),
        )
        .unwrap();
        let native = world.create(&config, format!("Native {name}"), Kind::Thing);
        let lua = world.create(&config, format!("Lua {name}"), Kind::Thing);
        ids.push((name, native, lua));
    }
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (name, native, lua) in ids {
        let output = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("@btech unit-create #{}={name}", native.0),
        );
        assert!(output.contains("constructed"), "{output}");
        scripts
            .eval_callback::<()>(&format!("btech.unit.create({}, '{name}')", lua.0))
            .unwrap();
        let world = scripts.world();
        world.validate(&config).unwrap();
        assert_eq!(
            world.btech.vehicles()[&native].definition(),
            world.btech.vehicles()[&lua].definition()
        );
        for id in [native, lua] {
            let unit = &world.btech.vehicles()[&id];
            assert!(unit.definition().is_vtol());
            assert_eq!(
                unit.vtol_flight().unwrap().phase,
                BattleVtolFlightPhase::Landed
            );
            assert!(unit.position().is_none());
        }
    }
}
