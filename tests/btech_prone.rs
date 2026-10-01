//! Controlled drops share native/Lua admission, physical consequences and world rollback.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Edit isolated scenario state without introducing production mutation helpers.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    let mut state = serde_json::to_value(&world.btech).unwrap();
    change(&mut state["constructed"][id.0.to_string()]);
    world.btech = serde_json::from_value(state).unwrap();
}

/// A conscious pilot in a running biped or quad on a chosen surface.
async fn fixture(quad: bool, tile: &str) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Drop field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "drop",
        BattleMapAsset::parse(&format!("1 1\n{tile}\n")).unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Controlled drop".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        BattleTemplate::parse(
            "test",
            if quad {
                include_str!("../game/mechs/SCP-1N.toml")
            } else {
                include_str!("../game/mechs/JR7-D.toml")
            },
        )
        .unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id, map)
}

/// Exact speed boundaries, reverse travel and success/failure use the existing replayable checks.
#[tokio::test]
async fn prone_speed_boundaries_and_replay() {
    for quad in [false, true] {
        let (_dir, config, base, id, _) = fixture(quad, ".0").await;
        let threshold =
            f64::from(battle_effective_maximum_speed(&base, id, true).unwrap() as f32 / 3.0);
        for (speed, levels) in [
            (0.0, 0),
            (threshold, 0),
            (threshold + 0.01, 1),
            (2.0 * threshold, 1),
            (2.0 * threshold + 0.01, 2),
        ] {
            for reverse in [false, true] {
                if reverse
                    && speed
                        > base.btech.constructed_units()[&id].mobility().maximum_speed * 2.0 / 3.0
                {
                    continue;
                }
                for success in [false, true] {
                    let seed = (0..=255)
                        .find(|seed| {
                            BattleDice::seeded([*seed; 32]).two_d6() == if success { 12 } else { 2 }
                        })
                        .unwrap();
                    let mut world = base.clone();
                    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
                    world
                        .objects
                        .get_mut(&ObjectId(2))
                        .unwrap()
                        .flags
                        .insert(Flag::Connected);
                    edit(&mut world, id, |unit| {
                        unit["motion"]["speed"] = (if reverse { -speed } else { speed }).into();
                        unit["motion"]["desired_speed"] = unit["motion"]["speed"].clone();
                        unit["motion"]["desired_heading"] = 90.0.into();
                        unit["dice"] =
                            serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
                    });
                    let scripts =
                        Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                    let report = prone_action(&scripts, &config, id, ObjectId(1)).unwrap();
                    assert_eq!(report.check.is_some(), levels > 0);
                    let output = scripts.drain_outbox();
                    for player in [ObjectId(1), ObjectId(2)] {
                        let feedback: Vec<_> = output
                            .iter()
                            .filter(|(who, text)| {
                                *who == player
                                    && (text.source().starts_with("You make a piloting")
                                        || text.source().starts_with("Modified Pilot Skill:"))
                            })
                            .map(|(_, text)| text.source().to_owned())
                            .collect();
                        if player == ObjectId(1) && levels > 0 {
                            let check = report.check.unwrap();
                            let mut expected = check.messages().unwrap().to_vec();
                            if let Some(messages) = report
                                .fall
                                .as_ref()
                                .and_then(|fall| fall.avoidance.as_ref())
                                .and_then(|check| check.messages())
                            {
                                expected.extend(messages);
                            }
                            assert_eq!(feedback, expected);
                            assert_eq!(report.pilot_notices[0].before_notice, 1);
                        } else {
                            assert!(feedback.is_empty());
                        }
                    }
                    assert_eq!(report.fall.is_some(), levels > 0 && !success);
                    if let Some(check) = &report.check {
                        assert_eq!(check.situational, if levels == 2 { 2 } else { 0 });
                    }
                    let after = scripts.world().clone();
                    assert_eq!(
                        after.btech.constructed_units()[&id].posture(),
                        BattlePosture::Prone
                    );
                    assert_eq!(
                        after.btech.constructed_units()[&id].motion().unwrap().speed,
                        0.0
                    );
                    if report.fall.is_none() {
                        assert_eq!(
                            after.btech.constructed_units()[&id].sections(),
                            base.btech.constructed_units()[&id].sections()
                        );
                        assert_eq!(
                            after.btech.constructed_units()[&id]
                                .motion()
                                .unwrap()
                                .desired_heading,
                            90.0
                        );
                    }
                    if levels == 0 {
                        let before = serde_json::to_value(&world.btech).unwrap();
                        let after = serde_json::to_value(&after.btech).unwrap();
                        assert_eq!(
                            after["constructed"][id.0.to_string()]["dice"],
                            before["constructed"][id.0.to_string()]["dice"]
                        );
                    }
                    let replay = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                    assert_eq!(
                        prone_action(&replay, &config, id, ObjectId(1)).unwrap(),
                        report
                    );
                    assert_eq!(replay.world().btech, after.btech);
                }
            }
        }
    }
}

/// Native and Lua adapters share authority and save/reload behavior; caller failure restores the drop.
#[tokio::test]
async fn prone_native_lua_and_restart() {
    for quad in [false, true] {
        let (_dir, config, mut world, id, _) = fixture(quad, ".0").await;
        if quad {
            edit(&mut world, id, |unit| {
                unit["hull_down"] = serde_json::to_value(BattleHullDownState {
                    active: true,
                    pending: Some(false),
                    remaining: 3,
                })
                .unwrap();
            });
        } else {
            let _ = flip_battle_arms(&mut world, id, ObjectId(1)).unwrap();
        }
        edit(&mut world, id, |unit| {
            unit["stagger"]["hits"] =
                serde_json::json!([{"damage":40,"remaining":60,"counted":false}]);
        });
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let error = scripts.eval_callback::<()>(&format!(
            "btech.unit.prone({},1); error('abort drop')",
            id.0
        ));
        assert!(error.is_err());
        assert_eq!(scripts.world().btech, world.btech);
        assert!(scripts.outbox().is_empty());
        let report: mlua::Table = scripts
            .eval_callback(&format!("return btech.unit.prone({},1)", id.0))
            .unwrap();
        assert!(
            report
                .get::<Option<mlua::Table>>("check")
                .unwrap()
                .is_none()
        );
        let expected = scripts.world().clone();
        assert!(
            expected.btech.constructed_units()[&id]
                .stagger()
                .hits
                .is_empty()
        );
        assert_eq!(
            expected.btech.constructed_units()[&id].facing(),
            BattleFacing::default()
        );
        assert_eq!(
            expected.btech.constructed_units()[&id].hull_down(),
            BattleHullDownState::default()
        );
        *scripts.world_mut() = world;
        support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            "prone ignored trailing text",
        );
        assert_eq!(scripts.world().btech, expected.btech);
        persistence::save(&config.database(), &expected)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            expected.btech
        );
        assert!(
            prone_action(&scripts, &config, id, ObjectId(1))
                .unwrap_err()
                .to_string()
                .contains("already prone")
        );
    }
}

/// Grounded drops flood exposed compartments, extinguish infernos and publish stepping mine callbacks atomically.
#[tokio::test]
async fn prone_water_and_mine_callback_rollback() {
    for quad in [false, true] {
        let (_dir, config, mut world, id, _map) = fixture(quad, "~1").await;
        edit(&mut world, id, |unit| {
            unit["sections"]["LeftArm"]["armor"] = 0.into();
            unit["inferno_remaining"] = 60.into();
        });
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let report = prone_action(&scripts, &config, id, ObjectId(1)).unwrap();
        assert!(
            report
                .flooding
                .iter()
                .any(|flood| flood.section == BattleSection::LeftArm)
        );
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].inferno_remaining(),
            0
        );
        assert_eq!(report.mines.reason, BattleMineTriggerReason::Step);
        let (_dir, config, mut world, id, map) = fixture(quad, ".0").await;
        set_minefield(
            &mut world,
            map,
            1,
            Some(BattleMinefield {
                coordinate: BattleHexCoordinate { x: 0, y: 0 },
                kind: BattleMineKind::Trigger,
                strength: 1,
                extra: 0,
                owner: ObjectId(1),
            }),
        )
        .unwrap();
        let before = world.clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let parents: mlua::Table = scripts
            .inspect_lua()
            .named_registry_value("mux.parents")
            .unwrap();
        let parent: mlua::Table = parents
            .get(before.objects[&id].lua_parent.as_str())
            .unwrap();
        let previous: mlua::Value = parent.get("events").unwrap();
        let events = scripts.inspect_lua().create_table().unwrap();
        events
            .set(
                "on_mech_mine_trigger",
                scripts
                    .inspect_lua()
                    .create_function(|_, _: mlua::Value| -> mlua::Result<()> {
                        Err(mlua::Error::external(
                            "controlled drop mine callback failed",
                        ))
                    })
                    .unwrap(),
            )
            .unwrap();
        parent.set("events", events).unwrap();
        assert!(prone_action(&scripts, &config, id, ObjectId(1)).is_err());
        assert_eq!(scripts.world().btech, before.btech);
        assert!(scripts.outbox().is_empty());
        parent.set("events", previous).unwrap();
        assert_eq!(
            prone_action(&scripts, &config, id, ObjectId(1))
                .unwrap()
                .mines
                .triggers,
            1
        );
    }
}

/// Admission never consumes dice for absent pilots, shutdown, airborne motion or pending standing.
#[tokio::test]
async fn prone_admission_rejects_without_mutation() {
    let (_dir, config, base, id, _) = fixture(false, ".0").await;
    for case in ["off", "standing", "falling", "pilot"] {
        let mut world = base.clone();
        edit(&mut world, id, |unit| match case {
            "off" => unit["power"] = serde_json::to_value(BattlePower::Off).unwrap(),
            "standing" => {
                unit["stand_timer"] =
                    serde_json::to_value(BattleStandTimer::Rising { remaining: 5 }).unwrap()
            }
            "falling" => unit["free_fall"] = serde_json::to_value(BattleFreeFall::new(10)).unwrap(),
            _ => unit["pilot"] = serde_json::Value::Null,
        });
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        assert!(
            prone_action(&scripts, &config, id, ObjectId(1)).is_err(),
            "{case}"
        );
        assert_eq!(scripts.world().btech, world.btech);
    }
}

/// A prone water breach uses ordinary in-character evacuation while wizard contents remain aboard.
#[tokio::test]
async fn prone_flooded_cockpit_evacuates_contents() {
    let (_dir, config, mut world, id, _) = fixture(false, "~1").await;
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
    edit(&mut world, id, |unit| {
        unit["sections"]["Head"]["armor"] = 0.into();
    });
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let report = prone_action(&scripts, &config, id, ObjectId(1)).unwrap();
    assert!(
        report
            .flooding
            .iter()
            .any(|flood| flood.section == BattleSection::Head)
    );
    assert_eq!(
        scripts.world().objects[&ObjectId(2)].location,
        Some(ObjectId(config.battletech.afterlife_dbref))
    );
    assert_eq!(scripts.world().objects[&ObjectId(1)].location, Some(id));
    let saved = scripts.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}

/// Every supported vehicle movement family rejects the Mech-only posture command.
#[tokio::test]
async fn prone_rejects_vehicle_chassis() {
    let (_dir, config, base, _, _) = fixture(false, ".0").await;
    for movement in ["track", "wheel", "hover", "none", "vtol"] {
        let mut world = base.clone();
        let id = world.create(&config, "Vehicle".into(), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        let text = if movement == "vtol" {
            include_str!("../game/mechs/Kestrel.toml").to_owned()
        } else {
            include_str!("../game/mechs/Demolisher.toml").replace(
                "movement = \"track\"",
                &format!("movement = \"{movement}\""),
            )
        };
        let text = if movement == "none" {
            text.replace("max_speed = 53.75", "max_speed = 0.0")
        } else {
            text
        };
        create_battle_vehicle(
            &mut world,
            id,
            BattleVehicleTemplate::parse("test", &text).unwrap(),
        )
        .unwrap();
        let before = world.btech.clone();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        assert!(
            prone_action(&scripts, &config, id, ObjectId(1))
                .unwrap_err()
                .to_string()
                .contains("can't prone")
        );
        assert_eq!(scripts.world().btech, before);
    }
}

/// Restored action damage is independent of periodic history and applies before the shared drop roll.
#[tokio::test]
async fn restored_stagger_controls_drop_levels_and_survives_replay() {
    for quad in [false, true] {
        let (_dir, config, base, id, _) = fixture(quad, ".0").await;
        let threshold =
            f64::from(battle_effective_maximum_speed(&base, id, true).unwrap() as f32 / 3.0);
        for scalar in [-10, 0, 19, 20, 40, 60] {
            for (speed, speed_levels) in
                [(0.0, 0), (threshold + 0.01, 1), (2.0 * threshold + 0.01, 2)]
            {
                for success in [false, true] {
                    let mut world = base.clone();
                    let seed = (0..=255)
                        .find(|&seed| {
                            BattleDice::seeded([seed; 32]).two_d6() == if success { 12 } else { 2 }
                        })
                        .unwrap();
                    edit(&mut world, id, |u| {
                        u["stagger"]["action_damage"] = scalar.into();
                        // Positive ordinary damage must not itself activate action-time staggering.
                        u["stagger"]["hits"] =
                            serde_json::json!([{"damage":60,"remaining":60,"counted":false}]);
                        u["motion"]["speed"] = speed.into();
                        u["motion"]["desired_speed"] = speed.into();
                        u["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
                    });
                    persistence::save(&config.database(), &world).await.unwrap();
                    let restored = persistence::load(&config.database()).await.unwrap();
                    assert_eq!(
                        restored.btech.constructed_units()[&id]
                            .stagger()
                            .action_damage,
                        scalar
                    );
                    let scripts =
                        Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                    let replay = Scripts::new(&config, Rc::new(RefCell::new(restored))).unwrap();
                    let status = battle_unit_status(&world, id, "I").unwrap();
                    let level = (scalar / 20).max(0);
                    assert_eq!(status.contains("STAGGERING"), level > 0);
                    let fields =
                        view_battle_unit_fields_action(&scripts, &config, ObjectId(1), id, "")
                            .unwrap();
                    assert_eq!(
                        fields
                            .fields
                            .iter()
                            .find(|f| f.name == "StaggerDamage")
                            .unwrap()
                            .value,
                        Some(scalar.to_string())
                    );
                    assert!(
                        set_battle_unit_field_action(
                            &scripts,
                            &config,
                            ObjectId(1),
                            id,
                            "StaggerDamage",
                            "20"
                        )
                        .is_err()
                    );
                    scripts.drain_outbox();
                    let before = scripts.world().btech.clone();
                    assert!(
                        scripts
                            .eval_callback::<()>(&format!(
                                "btech.unit.prone({},1); error('abort staggered drop')",
                                id.0
                            ))
                            .is_err()
                    );
                    assert_eq!(scripts.world().btech, before);
                    assert!(scripts.outbox().is_empty());
                    let report = prone_action(&scripts, &config, id, ObjectId(1)).unwrap();
                    assert_eq!(
                        report,
                        prone_action(&replay, &config, id, ObjectId(1)).unwrap()
                    );
                    assert_eq!(scripts.world().btech, replay.world().btech);
                    let needs_roll = speed_levels > 0 || level > 0;
                    assert_eq!(report.check.is_some(), needs_roll);
                    assert_eq!(report.fall.is_some(), needs_roll && !success);
                    if let Some(check) = report.check {
                        assert_eq!(
                            check.situational,
                            level + if speed_levels == 2 { 2 } else { 0 }
                        );
                    }
                    assert_eq!(
                        report.notices.iter().any(
                            |n| n.text == "Still staggering, you try not to fall on your face."
                        ),
                        level > 0
                    );
                    let after = scripts.world();
                    let state = after.btech.constructed_units()[&id].stagger();
                    assert_eq!(state.action_damage, scalar);
                    assert!(state.hits.is_empty());
                }
            }
        }
    }
}
