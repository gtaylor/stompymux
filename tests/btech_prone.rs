//! Controlled drops share native/Lua admission, physical consequences and world rollback.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Edit isolated scenario state without introducing production mutation helpers.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    world.btech.rewrite_unit_record(id, change).unwrap();
}

/// A conscious pilot in a running biped or quad on a chosen surface.
async fn fixture(quad: bool, tile: &str) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Drop field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "drop",
        MapAsset::from_cells(&format!("1 1\n{tile}\n")).unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Controlled drop".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        id,
        MechTemplate::parse(
            "test",
            if quad {
                include_str!("../game/units/SCP-1N.toml")
            } else {
                include_str!("../game/units/JR7-D.toml")
            },
        )
        .unwrap(),
    )
    .unwrap();
    place_battle_unit(&mut world, id, map, 0, 0).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    (dir, config, world, id, map)
}

/// Dropping to the ground is safe at any speed, forward or backward (Total Warfare p. 49): no
/// control roll, no damage, no dice, the facing kept, and the same result on replay.
#[tokio::test]
async fn prone_drops_at_any_speed_without_a_roll() {
    for quad in [false, true] {
        let (_dir, config, base, id, _) = fixture(quad, ".0").await;
        let walking = base.btech.constructed_units()[&id].walking_speed();
        let running = base.btech.constructed_units()[&id].movement_maximum_speed();
        for speed in [0.0, walking, -walking, running] {
            let mut world = base.clone();
            world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
            world
                .objects
                .get_mut(&ObjectId(2))
                .unwrap()
                .flags
                .insert(Flag::Connected);
            edit(&mut world, id, |unit| {
                unit["motion"]["speed"] = speed.into();
                unit["motion"]["desired_speed"] = speed.into();
                unit["motion"]["desired_heading"] = 90.0.into();
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let report = prone_action(&scripts, &config, id, ObjectId(1)).unwrap();
            assert_eq!(report.notices[0].text, "You drop to the ground prone!");
            assert!(report.pilot_notices.is_empty());
            let output = scripts.drain_outbox();
            assert!(output.iter().any(|(who, text)| *who == ObjectId(1)
                && text.source() == "You drop to the ground prone!"));
            assert!(!output.iter().any(|(_, text)| {
                text.source().starts_with("You make a piloting")
                    || text.source().starts_with("Modified Pilot Skill:")
            }));
            let after = scripts.world().clone();
            let unit = &after.btech.constructed_units()[&id];
            assert_eq!(unit.posture(), Posture::Prone);
            assert_eq!(unit.motion().unwrap().speed, 0.0);
            assert_eq!(unit.motion().unwrap().desired_speed, 0.0);
            assert_eq!(unit.motion().unwrap().desired_heading, 90.0);
            assert_eq!(
                unit.sections(),
                base.btech.constructed_units()[&id].sections()
            );
            assert_eq!(unit.heat(), base.btech.constructed_units()[&id].heat());
            let before = serde_json::to_value(&world.btech).unwrap();
            let saved = serde_json::to_value(&after.btech).unwrap();
            assert_eq!(
                saved["constructed"][id.0.to_string()]["dice"],
                before["constructed"][id.0.to_string()]["dice"]
            );
            let replay = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            assert_eq!(
                prone_action(&replay, &config, id, ObjectId(1)).unwrap(),
                report
            );
            assert_eq!(replay.world().btech, after.btech);
        }
    }
}

/// A Mech may not drop to the ground in the turn it jumped (Total Warfare p. 49): thirty
/// seconds must pass after a launch, and the refusal changes nothing.
#[tokio::test]
async fn prone_is_refused_for_thirty_seconds_after_a_jump() {
    let (_dir, config, base, id, _) = fixture(false, ".0").await;
    let now = base.btech.simulation_time();
    for (launched, allowed) in [(now, false), (now - 29, false), (now - 30, true)] {
        let mut world = base.clone();
        edit(&mut world, id, |unit| {
            unit["last_jump_launch"] = launched.into();
        });
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let result = prone_action(&scripts, &config, id, ObjectId(1));
        assert_eq!(result.is_ok(), allowed, "{launched}");
        if !allowed {
            assert!(
                result
                    .unwrap_err()
                    .to_string()
                    .contains("so soon after jumping")
            );
            assert_eq!(scripts.world().btech, world.btech);
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
                unit["hull_down"] = serde_json::to_value(HullDownState {
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
            unit["stagger"]["turn_damage"] = 40.into();
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
        // A deliberate drop leaves the turn's ordinary damage accounting alone.
        assert_eq!(
            expected.btech.constructed_units()[&id]
                .stagger()
                .turn_damage,
            40
        );
        assert_eq!(
            expected.btech.constructed_units()[&id].facing(),
            Facing::default()
        );
        assert_eq!(
            expected.btech.constructed_units()[&id].hull_down(),
            HullDownState::default()
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

/// Firing when down (Total Warfare p. 113; Tactical Operations p. 83): a prone biped fires
/// at +2, or +3 once only one arm is left to prop it; a prone quad with four legs and no hip
/// hit fires as though standing, and a hip hit puts it back on the biped rule.
#[tokio::test]
async fn prone_fire_modifier_by_chassis_arms_and_hips() {
    for quad in [false, true] {
        let (_dir, config, world, id, _) = fixture(quad, ".0").await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        prone_action(&scripts, &config, id, ObjectId(1)).unwrap();
        let mut world = scripts.world().clone();
        assert_eq!(
            world.btech.constructed_units()[&id].posture(),
            Posture::Prone
        );
        let modifier =
            |world: &World| world.btech.constructed_units()[&id].attacker_movement_modifier();
        assert_eq!(modifier(&world), if quad { 0 } else { 2 });
        if quad {
            destroy_battle_critical(
                &mut world,
                id,
                CriticalLocation {
                    section: MechSection::LeftLeg,
                    slot: 0,
                },
            )
            .unwrap();
            assert_eq!(modifier(&world), 2);
        } else {
            edit(&mut world, id, |unit| {
                unit["sections"]["LeftArm"]["internal"] = 0.into();
                unit["sections"]["LeftArm"]["armor"] = 0.into();
            });
            assert_eq!(modifier(&world), 3);
            edit(&mut world, id, |unit| {
                unit["sections"]["RightArm"]["internal"] = 0.into();
                unit["sections"]["RightArm"]["armor"] = 0.into();
            });
            assert_eq!(modifier(&world), 2);
        }
        world.validate(&config).unwrap();
    }
}

/// Grounded drops flood exposed compartments and publish stepping mine callbacks atomically.
#[tokio::test]
async fn prone_water_and_mine_callback_rollback() {
    for quad in [false, true] {
        let (_dir, config, mut world, id, _map) = fixture(quad, "~1").await;
        edit(&mut world, id, |unit| {
            unit["sections"]["LeftArm"]["armor"] = 0.into();
        });
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let report = prone_action(&scripts, &config, id, ObjectId(1)).unwrap();
        assert!(
            report
                .flooding
                .iter()
                .any(|flood| flood.section == MechSection::LeftArm)
        );
        assert_eq!(report.mines.reason, MineTriggerReason::Step);
        let (_dir, config, mut world, id, map) = fixture(quad, ".0").await;
        set_minefield(
            &mut world,
            map,
            1,
            Some(Minefield {
                coordinate: HexCoordinate { x: 0, y: 0 },
                kind: MineKind::Trigger,
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
            "off" => unit["power"] = serde_json::to_value(Power::Off).unwrap(),
            "standing" => {
                unit["stand_timer"] =
                    serde_json::to_value(StandTimer::Rising { remaining: 5 }).unwrap()
            }
            "falling" => unit["free_fall"] = serde_json::to_value(FreeFall::new(10)).unwrap(),
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
            .any(|flood| flood.section == MechSection::Head)
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
            include_str!("../game/units/Kestrel.toml").to_owned()
        } else {
            crate::support::templates::with_movement(
                include_str!("../game/units/Demolisher.toml"),
                movement,
            )
        };
        let text = if movement == "none" {
            text.replace("walk_mp = 3", "walk_mp = 0")
        } else {
            text
        };
        create_battle_vehicle(
            &mut world,
            id,
            crate::support::templates::fitted(VehicleTemplate::parse("test", &text).unwrap()),
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

/// Restored action damage survives replay and is shown by status and the field views, and a
/// staggering Mech still drops to the ground without a roll.
#[tokio::test]
async fn restored_stagger_survives_replay_and_never_adds_a_drop_roll() {
    for quad in [false, true] {
        let (_dir, config, base, id, _) = fixture(quad, ".0").await;
        let walking = base.btech.constructed_units()[&id].walking_speed();
        for scalar in [-10, 0, 19, 20, 40, 60] {
            for speed in [0.0, walking + 0.01] {
                {
                    let mut world = base.clone();
                    edit(&mut world, id, |u| {
                        u["stagger"]["action_damage"] = scalar.into();
                        // Ordinary turn damage must not itself activate action-time staggering.
                        u["stagger"]["turn_damage"] = 60.into();
                        u["motion"]["speed"] = speed.into();
                        u["motion"]["desired_speed"] = speed.into();
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
                    assert!(report.pilot_notices.is_empty());
                    assert_eq!(report.notices[0].text, "You drop to the ground prone!");
                    let after = scripts.world();
                    let state = after.btech.constructed_units()[&id].stagger();
                    assert_eq!(state.action_damage, scalar);
                    // A deliberate drop is not a fall, so the turn's damage accounting goes on.
                    assert_eq!(state.turn_damage, 60);
                }
            }
        }
    }
}
