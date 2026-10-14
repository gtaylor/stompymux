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
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
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
        VehicleTemplate::parse("Kestrel", include_str!("../game/units/Kestrel.toml")).unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    assign_battle_pilot(&mut world, id, ObjectId(2)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(2), false).unwrap();
    for _ in 0..30 {
        advance_battle_units(&mut world, 0, true);
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
        MapAsset::from_cells(source).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut base, map, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut base, id, map, 0, 2).unwrap();
    base.objects.get_mut(&id).unwrap().location = Some(map);
    assign_battle_pilot(&mut base, id, ObjectId(2)).unwrap();
    support::seed_object_dice(&mut base, ObjectId(2), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut base, id, ObjectId(2), false).unwrap();
    for _ in 0..30 {
        advance_battle_units(&mut base, 0, true);
    }
    begin_battle_vtol_takeoff(&mut base, id, ObjectId(2), 0).unwrap();
    advance_battle_motion(&mut base, MovementRules::STANDARD).unwrap();
    set_battle_character(
        &mut base,
        ObjectId(2),
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
    support::seed_object_dice(&mut base, ObjectId(2), support::FIXTURE_DICE_SEED);
    set_battle_character_value(
        &mut base,
        ObjectId(2),
        "Piloting-Aerospace",
        CharacterValue {
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

/// Flying into terrain crashes the VTOL in the hex it entered (Total Warfare p. 54), with or
/// without a pilot and with no roll to avoid it; the crash damage follows TW p. 68, and the
/// VTOL has landed on the hilltop when it comes through still able to fly.
#[tokio::test]
async fn flying_into_terrain_crashes_in_the_hex_entered_without_a_roll() {
    for (tile, assigned) in [(".0", true), (".0", false), ("~3", true), ("%0", true)] {
        let (_dir, config, mut world, id, map) =
            obstacle_fixture(&format!("1 3\n.0\n^9\n{tile}\n")).await;
        world
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let previous = world.btech.maps()[&map].base_hex(0, 2).unwrap();
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut encoded["vehicles"][id.0.to_string()];
        if !assigned {
            unit["pilot"] = serde_json::Value::Null;
        }
        unit["motion"]["speed"] = 129.0.into();
        unit["motion"]["desired_speed"] = 129.0.into();
        unit["vtol_flight"]["altitude"] = f64::from(previous.surface_height().max(0) + 1).into();
        unit["vtol_flight"]["vertical_speed"] = 0.0.into();
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
        let outcome =
            advance_battle_vtol_environment(&mut world, id, MovementRules::STANDARD.fall).unwrap();
        assert_eq!(
            advance_battle_vtol_environment(&mut replay, id, MovementRules::STANDARD.fall).unwrap(),
            outcome
        );
        assert_eq!(replay.btech, world.btech);
        let VtolEnvironment::Crashed { path, fall } = outcome else {
            panic!("Expected a crash into the hill: {tile}");
        };
        assert!(matches!(
            path,
            VtolPath::Contact {
                hex: HexCoordinate { x: 0, y: 1 },
                contact: VtolSurfaceContact::Elevation,
                ..
            }
        ));
        let unit = &world.btech.vehicles()[&id];
        // Twelve hexes of speed times 25 tons over ten (TW p. 68); the crew makes no roll.
        assert_eq!(fall.damage, 36, "{tile}");
        assert!(fall.avoidance.is_none(), "{tile}");
        assert_eq!(unit.position().unwrap().y, 1);
        assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Landed);
        assert_eq!(unit.vtol_flight().unwrap().altitude, 9.0);
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        let wrecked = fall
            .feedback
            .notices
            .iter()
            .any(|notice| notice.text == "[fg=red bold]The VTOL is wrecked.[reset]");
        assert_eq!(unit.is_destroyed(), wrecked, "{tile}");
        if !wrecked {
            assert!(!unit.rotor_destroyed() && !unit.immobilized());
            assert!(
                fall.feedback
                    .notices
                    .iter()
                    .any(|notice| notice.text.contains("still flyable"))
            );
        }
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(before))).unwrap();
        advance_battle_motion_action(&scripts, &config, MovementRules::STANDARD).unwrap();
        let output = scripts
            .drain_outbox()
            .into_iter()
            .map(|(_, document)| text::plain(document.source()))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            output.contains("CRASH! You fly straight into the terrain!"),
            "{tile}: {output}"
        );
        assert!(!output.contains("piloting skill roll"), "{tile}: {output}");
        scripts.world().validate(&config).unwrap();
    }
}

/// Entering a woods hex at or below the treetops crashes the VTOL there, and the trees are no
/// place to land, so the wreck is a write-off (Total Warfare pp. 54 and 68).
#[tokio::test]
async fn flying_into_the_treetops_crashes_in_the_woods() {
    let (_dir, config, base, id, map) = obstacle_fixture("1 3\n.0\n`0\n.0\n").await;
    for (altitude, crashes) in [(1.0, true), (2.0, true), (3.0, false)] {
        let mut world = base.clone();
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut encoded["vehicles"][id.0.to_string()];
        unit["motion"]["speed"] = 129.0.into();
        unit["motion"]["desired_speed"] = 129.0.into();
        unit["vtol_flight"]["altitude"] = altitude.into();
        unit["vtol_flight"]["vertical_speed"] = 0.0.into();
        encoded["maps"][map.0.to_string()]["movement_modifier"] = 1000.into();
        world.btech = serde_json::from_value(encoded).unwrap();
        let before = world.clone();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        let rules = MovementRules::STANDARD.fall;
        let outcome = advance_battle_vtol_environment(&mut world, id, rules).unwrap();
        assert_eq!(
            advance_battle_vtol_environment(&mut replay, id, rules).unwrap(),
            outcome
        );
        assert_eq!(world.btech, replay.btech);
        let unit = &world.btech.vehicles()[&id];
        if !crashes {
            assert!(
                matches!(
                    outcome,
                    VtolEnvironment::Movement { .. } | VtolEnvironment::Boundary { .. }
                ),
                "{altitude}: {outcome:?}"
            );
            assert_eq!(unit.position().unwrap().y, 0);
            assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Airborne);
            continue;
        }
        let VtolEnvironment::Crashed { path, fall } = outcome else {
            panic!("Expected a crash into the trees at {altitude}");
        };
        assert!(matches!(
            path,
            VtolPath::Contact {
                hex: HexCoordinate { x: 0, y: 1 },
                contact: VtolSurfaceContact::Forest,
                ..
            }
        ));
        // Crash damage is the hexes covered times tonnage/10 (TW p. 68): 129 kph is 12 MP.
        assert_eq!(
            fall.damage,
            12 * u32::from(unit.definition().tons).div_ceil(10)
        );
        assert!(fall.avoidance.is_none());
        assert_eq!(unit.position().unwrap().y, 1);
        assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Landed);
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
        assert!(unit.immobilized() && unit.rotor_destroyed() && unit.is_destroyed());
        assert!(
            fall.feedback
                .notices
                .iter()
                .any(|notice| notice.text == "[fg=red bold]The VTOL is wrecked.[reset]")
        );
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(before))).unwrap();
        advance_battle_motion_action(&scripts, &config, MovementRules::STANDARD).unwrap();
        let output = scripts
            .drain_outbox()
            .into_iter()
            .map(|(_, document)| text::plain(document.source()))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            output.contains("CRASH! You fly into the trees!"),
            "{altitude}: {output}"
        );
        assert!(
            output.contains("The VTOL is wrecked."),
            "{altitude}: {output}"
        );
        scripts.world().validate(&config).unwrap();
    }
}

/// Native and Lua controls honor engine fuel policy in flight and on the ground.
#[tokio::test]
async fn horizontal_fuel_gate_shares_policy_without_blocking_heading_or_readouts() {
    let (_dir, config, world, id) = fixture().await;
    for phase in ["landed", "launching", "airborne"] {
        // Only an internal-combustion VTOL burns fuel; a fusion VTOL never runs dry.
        for (ice, fuel, accepted) in [(true, 0, false), (false, 0, true), (true, 1, true)] {
            let mut world = world.clone();
            if phase != "landed" {
                begin_battle_vtol_takeoff(&mut world, id, ObjectId(2), 0).unwrap();
            }
            if phase == "airborne" {
                advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
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
            begin_battle_vtol_takeoff(&mut world, id, ObjectId(2), 3).unwrap();
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
        advance_battle_motion(&mut restored, MovementRules::STANDARD).unwrap();
        let unit = &restored.btech.vehicles()[&id];
        assert_eq!(unit.position(), before.position());
        assert_eq!(unit.motion(), before.motion());
        assert_eq!(unit.motion().unwrap().desired_speed, 20.0);
        assert_eq!(unit.motion().unwrap().speed, 0.0);
        assert_eq!(unit.motion().unwrap().heading, 0.0);
        assert_eq!(unit.motion().unwrap().desired_heading, 90.0);
        if launching {
            for _ in 0..3 {
                advance_battle_motion(&mut restored, MovementRules::STANDARD).unwrap();
            }
            let unit = &restored.btech.vehicles()[&id];
            assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Airborne);
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
    let _ = advance_battle_motion(&mut scripts.world_mut(), MovementRules::STANDARD).unwrap();
    assert_eq!(
        scripts.world().btech.vehicles()[&id]
            .vtol_flight()
            .unwrap()
            .phase,
        VtolFlightPhase::Airborne
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
    assert!(output.contains("movement budget"), "{output}");
    assert_eq!(scripts.world().btech, before);
    assert!(battle_vtol_vertical_readout(&scripts.world(), ObjectId(0), ObjectId(2)).is_err());
}

#[tokio::test]
async fn shared_landing_cancels_launch_and_resolves_touchdown_without_jump_checks() {
    let (_dir, config, mut world, id) = fixture().await;
    let _ = begin_battle_vtol_takeoff(&mut world, id, ObjectId(2), 0).unwrap();
    let notices = land_battle_jump(&mut world, id, ObjectId(2), MovementRules::STANDARD).unwrap();
    assert!(notices.iter().any(|notice| notice.text
        == format!(
            "Launch aborted by {}.",
            text::escape(&world.objects[&ObjectId(2)].name)
        )));
    assert_eq!(
        world.btech.vehicles()[&id].vtol_flight().unwrap().phase,
        VtolFlightPhase::Landed
    );
    let _ = begin_battle_vtol_takeoff(&mut world, id, ObjectId(2), 0).unwrap();
    let _ = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
    let _ = set_battle_vtol_vertical_speed(&mut world, id, ObjectId(2), 0.0).unwrap();
    let before = world.btech.clone();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let output = support::run_text(&scripts, &config, ObjectId(2), 1, "land");
    assert!(
        output.contains("You bring your VTOL to a safe landing."),
        "{output}"
    );
    scripts.world().validate(&config).unwrap();
    let notices = land_battle_jump(&mut world, id, ObjectId(2), MovementRules::STANDARD).unwrap();
    assert!(
        notices
            .iter()
            .any(|notice| notice.text == "You bring your VTOL to a safe landing.")
    );
    let unit = &world.btech.vehicles()[&id];
    assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Landed);
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
    begin_battle_vtol_takeoff(&mut world, id, ObjectId(2), 0).unwrap();
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
    // Liftoff climbs at 60 kph, a level every eleven seconds or so.
    for _ in 0..60 {
        advance_battle_motion_action(&scripts, &config, MovementRules::STANDARD).unwrap();
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
        VtolFlightPhase::Falling
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
        advance_battle_motion_action(&scripts, &config, MovementRules::STANDARD).unwrap();
        advance_battle_motion_action(&replay, &config, MovementRules::STANDARD).unwrap();
        assert_eq!(scripts.world().btech, replay.world().btech);
        scripts.world().validate(&config).unwrap();
    }
    let flight = scripts.world().btech.vehicles()[&id].vtol_flight().unwrap();
    assert_eq!(flight.phase, VtolFlightPhase::Landed);
    assert_eq!(flight.altitude, 0.0);
    assert!(flight.fall.is_none());
}

#[tokio::test]
async fn live_validation_rejects_impossible_flight_lifecycle_combinations() {
    let (_dir, config, mut world, id) = fixture().await;
    let _ = begin_battle_vtol_takeoff(&mut world, id, ObjectId(2), 0).unwrap();
    let _ = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
    world.validate(&config).unwrap();
    let base = serde_json::to_value(&world.btech).unwrap();
    let mut waiting = world.clone();
    let mut saved = base.clone();
    saved["vehicles"][id.0.to_string()]["power"] = serde_json::to_value(Power::Off).unwrap();
    saved["vehicles"][id.0.to_string()]["vtol_flight"]["vertical_speed"] = 0.0.into();
    saved["vehicles"][id.0.to_string()]["motion"]["speed"] = 0.0.into();
    waiting.btech = serde_json::from_value(saved).unwrap();
    waiting.validate(&config).unwrap();
    let before = waiting.btech.clone();
    advance_battle_motion(&mut waiting, MovementRules::STANDARD).unwrap();
    assert_eq!(waiting.btech, before);
    assert!(waiting.btech.vehicles()[&id].vtol_motion_step(0).is_err());
    // Powered vertical travel cannot remain active after power is removed; such a
    // vehicle is refused as it loads.
    let mut powered_vertical = base.clone();
    powered_vertical["vehicles"][id.0.to_string()]["power"] =
        serde_json::to_value(Power::Off).unwrap();
    let error = serde_json::from_value::<BtechState>(powered_vertical).unwrap_err();
    assert!(error.to_string().contains("requires power"), "{error}");
    let mut saved = base.clone();
    saved["vehicles"][id.0.to_string()]["ground_elevation"] = serde_json::json!(0);
    let error = serde_json::from_value::<BtechState>(saved).unwrap_err();
    assert!(
        error.to_string().contains("ground-vehicle elevation"),
        "{error}"
    );
}

#[tokio::test]
async fn live_character_crash_wrecks_the_vtol_without_crew_injury() {
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
    support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
    let _ = begin_battle_vtol_takeoff(&mut world, id, ObjectId(2), 0).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for _ in 0..12 {
        advance_battle_motion_action(&scripts, &config, MovementRules::STANDARD).unwrap();
    }
    let mut unit = scripts.world().btech.vehicles()[&id].clone();
    let _ = unit.apply_rotor_hit(RotorHit::Destroy, false).unwrap();
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    let mut unit = serde_json::to_value(unit).unwrap();
    unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
    let mut saved = serde_json::to_value(&scripts.world().btech).unwrap();
    saved["vehicles"][id.0.to_string()] = unit;
    scripts.world_mut().btech = serde_json::from_value(saved).unwrap();
    scripts.world().validate(&config).unwrap();
    let rules = MovementRules::STANDARD;
    for _ in 0..15 {
        advance_battle_motion_action(&scripts, &config, rules).unwrap();
        scripts.world().validate(&config).unwrap();
    }
    let world = scripts.world();
    let unit = &world.btech.vehicles()[&id];
    assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Landed);
    // Total Warfare gives vehicle crews no injury roll for a crash; the wreck is the outcome.
    assert!(unit.is_destroyed());
    assert_eq!(unit.pilot_injuries(), 0);
    assert!(
        unit.character_pilot_status()
            .is_none_or(|status| status.injuries == 0)
    );
}

#[tokio::test]
async fn native_and_lua_asset_loading_admit_flying_and_stationary_aircraft() {
    let (dir, config, mut world) = support::isolated_world().await;
    std::fs::create_dir_all(dir.path().join("units")).unwrap();
    let mut ids = Vec::new();
    for name in ["Kestrel", "ObservationVTOL"] {
        std::fs::copy(
            support::repository_root().join(format!("game/units/{name}.toml")),
            dir.path().join("units").join(format!("{name}.toml")),
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
            assert_eq!(unit.vtol_flight().unwrap().phase, VtolFlightPhase::Landed);
            assert!(unit.position().is_none());
        }
    }
}

/// A VTOL at flank speed that changes facing and goes on into a new hex rolls to avoid a
/// sideslip (Total Warfare pp. 54, 67): a failure drifts it along its old course, at its
/// elevation, without damage over open ground; at cruising speed there is no roll.
#[tokio::test]
async fn a_flanking_vtol_rolls_against_sideslipping_after_a_turn() {
    let field = format!("6 6\n{}", format!("{}\n", ".0".repeat(6)).repeat(6));
    for (roll, flank) in [(2, true), (12, true), (12, false)] {
        let (_dir, config, mut world, id, map) = obstacle_fixture(&field).await;
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == roll)
            .unwrap();
        let speed = if flank { 193.5 } else { 129.0 };
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        let unit = &mut encoded["vehicles"][id.0.to_string()];
        unit["motion"]["heading"] = 90.0.into();
        unit["motion"]["desired_heading"] = 150.0.into();
        unit["motion"]["speed"] = speed.into();
        unit["motion"]["desired_speed"] = speed.into();
        unit["vtol_flight"]["altitude"] = 3.0.into();
        unit["vtol_flight"]["vertical_speed"] = 0.0.into();
        unit["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap();
        encoded["maps"][map.0.to_string()]["movement_modifier"] = 200.into();
        world.btech = serde_json::from_value(encoded).unwrap();
        let mut checked = false;
        for _ in 0..12 {
            let before = world.clone();
            let notices = advance_battle_motion(&mut world, MovementRules::STANDARD).unwrap();
            if !notices.iter().any(|n| n.text.contains("hold your line")) {
                continue;
            }
            checked = true;
            let unit = &world.btech.vehicles()[&id];
            let flight = unit.vtol_flight().unwrap();
            assert_eq!(flight.phase, VtolFlightPhase::Airborne);
            assert_eq!(flight.altitude, 3.0);
            assert_eq!(unit.sections(), before.btech.vehicles()[&id].sections());
            if roll == 12 {
                assert!(notices.iter().any(|n| n.text == "You hold your course."));
            } else {
                assert!(
                    notices
                        .iter()
                        .any(|n| n.text == "You lose your line and sideslip!")
                );
                assert!(
                    notices
                        .iter()
                        .any(|n| n.text.starts_with("You sideslip into"))
                );
                assert!(
                    unit.position().unwrap().x > before.btech.vehicles()[&id].position().unwrap().x
                );
            }
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(
                notices,
                advance_battle_motion(&mut restored, MovementRules::STANDARD).unwrap()
            );
            assert_eq!(world.btech, restored.btech);
            break;
        }
        assert_eq!(checked, flank, "roll {roll} flank {flank}");
        world.validate(&config).unwrap();
    }
}
