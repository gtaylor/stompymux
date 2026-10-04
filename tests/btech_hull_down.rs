//! Quad hull-down timing, action rollback, movement admission and shared perception cover.
use crate::support;
use stompymux_rs::*;
const QUAD: &str = include_str!("../game/mechs/SCP-1N.toml");
const MECH: &str = include_str!("fixtures/btech/mechs/JR7-D.toml");
const VEHICLE: &str = include_str!("../game/mechs/Demolisher.toml");

/// A quad pilot and a separate observer looking across a one-level ridge.
async fn fixture(
    source: &str,
    observer: &str,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Ridge".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "ridge",
        MapAsset::from_cells("3 3\n.0.1.0\n.0.1.0\n.0.1.0\n").unwrap(),
    )
    .unwrap();
    let id = world.create(&config, "Quad".into(), Kind::Thing);
    let shooter = world.create(&config, "Observer".into(), Kind::Thing);
    for (id, source, x) in [(id, source, 2), (shooter, observer, 0)] {
        BattleUnitTemplate::parse("test", source)
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        place_battle_unit(&mut world, id, map, x, 1).unwrap();
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    support::seed_world_dice(&mut world, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    (dir, config, world, id, shooter)
}

/// Lower the quad and wait for the posture change to complete.
fn lower(world: &mut World, id: ObjectId) {
    set_battle_hull_down(world, id, ObjectId(1), "").unwrap();
    for _ in 0..3 {
        advance_battle_units(world, 0);
    }
    assert!(world.btech.constructed_units()[&id].hull_down().active);
}

/// Native and Lua posture changes agree, roll back on error and resume after restart.
#[tokio::test]
async fn native_lua_transitions_cancel_rollback_and_resume_after_restart() {
    let (_dir, config, world, id, _) = fixture(QUAD, MECH).await;
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert!(
        support::run_text(&native, &config, ObjectId(1), 1, "hulldown").contains("start to lower")
    );
    lua.eval_callback::<()>(&format!("btech.unit.hulldown({},1)", id.0))
        .unwrap();
    assert_eq!(native.world().btech, lua.world().btech);
    assert_eq!(
        lua.world().btech.constructed_units()[&id]
            .hull_down()
            .remaining,
        3
    );
    for command in ["speed 1", "heading 90", "stand", "jump 1 0"] {
        let before = lua.world().btech.clone();
        support::run_text(&lua, &config, ObjectId(1), 1, command);
        assert_eq!(lua.world().btech, before, "{command}");
    }
    lua.drain_outbox();
    let before = lua.world().btech.clone();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.hulldown({},1,'stop'); error('abort')",
            id.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, before);
    assert!(lua.drain_outbox().is_empty());
    advance_battle_units(&mut lua.world_mut(), 0);
    let saved = lua.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let mut replay = persistence::load(&config.database()).await.unwrap();
    for _ in 0..2 {
        assert_eq!(
            advance_battle_units(&mut lua.world_mut(), 0),
            advance_battle_units(&mut replay, 0)
        );
    }
    assert_eq!(lua.world().btech, replay.btech);
    assert!(
        lua.world().btech.constructed_units()[&id]
            .hull_down()
            .active
    );
    assert!(
        battle_unit_status(&lua.world(), id, "info")
            .unwrap()
            .contains("HULLDOWN")
    );
    assert!(
        lua.eval_callback::<bool>(&format!(
            "return btech.unit.state({}).hull_down.active",
            id.0
        ))
        .unwrap()
    );
    let before = lua.world().btech.clone();
    assert!(set_battle_speed(&mut lua.world_mut(), id, ObjectId(1), 1.0).is_err());
    assert!(set_battle_heading(&mut lua.world_mut(), id, ObjectId(1), 90.0).is_err());
    assert_eq!(lua.world().btech, before);
    lua.eval_callback::<()>(&format!(
        "btech.unit.hulldown({},1,'-'); btech.unit.hulldown({},1,'stop')",
        id.0, id.0
    ))
    .unwrap();
    assert!(
        lua.world().btech.constructed_units()[&id]
            .hull_down()
            .active
    );
    lua.eval_callback::<()>(&format!("btech.unit.hulldown({},1,'-')", id.0))
        .unwrap();
    for _ in 0..3 {
        advance_battle_units(&mut lua.world_mut(), 0);
    }
    assert!(
        !lua.world().btech.constructed_units()[&id]
            .hull_down()
            .active
    );
    set_battle_speed(&mut lua.world_mut(), id, ObjectId(1), 1.0).unwrap();
    lua.world().validate(&config).unwrap();
}

/// Shutdown cancels a pending change, keeps a completed posture, and a fall clears it.
#[tokio::test]
async fn shutdown_cancels_changes_preserves_completed_posture_and_falls_clear_it() {
    for complete in [false, true] {
        let (_dir, config, mut world, id, _) = fixture(QUAD, MECH).await;
        set_battle_hull_down(&mut world, id, ObjectId(1), "").unwrap();
        if complete {
            for _ in 0..3 {
                advance_battle_units(&mut world, 0);
            }
        }
        stop_battle_unit(
            &mut world,
            id,
            ObjectId(1),
            BattleMovementRules::STANDARD.fall,
        )
        .unwrap();
        let state = world.btech.constructed_units()[&id].hull_down();
        assert_eq!(
            state,
            BattleHullDownState {
                active: complete,
                pending: None,
                remaining: 0
            }
        );
        world.validate(&config).unwrap();
        if complete {
            let _ =
                resolve_battle_fall(&mut world, id, 1, BattleMovementRules::STANDARD.fall).unwrap();
            assert_eq!(
                world.btech.constructed_units()[&id].hull_down(),
                BattleHullDownState::default()
            );
            world.validate(&config).unwrap();
        }
    }
}

/// Only quads may lower; speed sets the countdown and invalid saved states fail validation.
#[tokio::test]
async fn chassis_admission_countdown_bounds_and_invalid_saved_states() {
    for source in [MECH, VEHICLE] {
        let (_dir, _, mut world, id, _) = fixture(source, MECH).await;
        let before = world.btech.clone();
        assert!(set_battle_hull_down(&mut world, id, ObjectId(1), "").is_err());
        assert_eq!(world.btech, before);
    }
    let (_dir, config, mut world, id, _) = fixture(QUAD, MECH).await;
    for (speed, delay) in [(0.0, 30), (10.75, 30), (64.5, 5), (96.75, 3), (400.0, 1)] {
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["definition"]["max_speed"] = serde_json::json!(speed);
            })
            .unwrap();
        set_battle_hull_down(&mut world, id, ObjectId(1), "").unwrap();
        assert_eq!(
            world.btech.constructed_units()[&id].hull_down().remaining,
            delay
        );
        set_battle_hull_down(&mut world, id, ObjectId(1), "stop").unwrap();
    }
    set_battle_hull_down(&mut world, id, ObjectId(1), "").unwrap();
    for state in [
        serde_json::json!({"active":false,"pending":true,"remaining":0}),
        serde_json::json!({"active":false,"pending":true,"remaining":31}),
        serde_json::json!({"active":true,"pending":true,"remaining":3}),
    ] {
        let mut saved = serde_json::to_value(&world.btech).unwrap();
        saved["constructed"][id.0.to_string()]["hull_down"] = state;
        let invalid: BtechState = serde_json::from_value(saved).unwrap();
        let mut candidate = world.clone();
        candidate.btech = invalid;
        assert!(candidate.validate(&config).is_err());
    }
}

/// Hull-down cover adds two to partial cover for Mech and vehicle attackers by sensors or sight.
#[tokio::test]
async fn hull_down_cover_is_shared_between_attackers_by_sensors_and_sight() {
    for shooter_source in [MECH, VEHICLE] {
        let (_dir, config, mut world, id, shooter) = fixture(QUAD, shooter_source).await;
        // Shallow water supplies cover visible from both tall Mechs and low vehicles.
        let map = world.btech.constructed_units()[&id].position().unwrap().map;
        world
            .btech
            .rewrite_map_record(map, |record| {
                for index in 0..9 {
                    crate::support::set_hex_terrain(
                        &mut record["terrain"][index],
                        if index == 3 {
                            stompymux_rs::Terrain::Grassland
                        } else {
                            stompymux_rs::Terrain::Water
                        },
                    );
                    crate::support::set_hex_elevation(
                        &mut record["terrain"][index],
                        if index == 3 { 0 } else { 1 },
                    );
                }
            })
            .unwrap();
        let terrain = battle_unit_terrain_los(&world, shooter, id).unwrap();
        assert!(terrain.partial_cover);
        let cover = i16::from(terrain.woods) + i16::from(terrain.target_woods) + 3;
        // The sensor band reaches first; with it switched off, sight carries the same cover.
        for (sensors, channel) in [
            (true, BattleDetectionChannel::Sensors),
            (false, BattleDetectionChannel::Sight),
        ] {
            set_battle_map_perception(&mut world, map, BattleMapPerceptionFlag::Sensors, sensors)
                .unwrap();
            let before = battle_perceive(&world, shooter, id).unwrap().unwrap();
            assert_eq!((before.channel, before.aim_modifier), (channel, cover));
            lower(&mut world, id);
            let after = battle_perceive(&world, shooter, id).unwrap().unwrap();
            assert_eq!(after.aim_modifier - before.aim_modifier, 2);
            assert_eq!(
                (after.channel, after.identified, after.probed),
                (before.channel, before.identified, before.probed)
            );
            set_battle_hull_down(&mut world, id, ObjectId(1), "-").unwrap();
            for _ in 0..3 {
                advance_battle_units(&mut world, 0);
            }
        }
        set_battle_map_perception(&mut world, map, BattleMapPerceptionFlag::Sensors, true).unwrap();
        // Open ground removes the bonus, even though the unit remains lowered.
        world
            .btech
            .rewrite_map_record(map, |record| {
                for index in 0..9 {
                    crate::support::set_hex_terrain(
                        &mut record["terrain"][index],
                        stompymux_rs::Terrain::Grassland,
                    );
                    crate::support::set_hex_elevation(&mut record["terrain"][index], 0);
                }
            })
            .unwrap();
        assert!(
            !battle_unit_terrain_los(&world, shooter, id)
                .unwrap()
                .partial_cover
        );
        let before = battle_perceive(&world, shooter, id).unwrap();
        lower(&mut world, id);
        assert_eq!(before, battle_perceive(&world, shooter, id).unwrap());
        world.validate(&config).unwrap();
    }
}

/// Pickup preparation uses the same posture cleanup regardless of the carrier's chassis.
#[tokio::test]
async fn pickup_clears_completed_quad_cover() {
    let (_dir, config, mut world, id, carrier) =
        fixture(QUAD, include_str!("fixtures/btech/mechs/AS7-D.toml")).await;
    lower(&mut world, id);
    stop_battle_unit(
        &mut world,
        id,
        ObjectId(1),
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    let position = world.btech.constructed_units()[&id].position().unwrap();
    place_battle_unit(
        &mut world,
        carrier,
        position.map,
        i64::from(position.x),
        i64::from(position.y),
    )
    .unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(carrier);
    assign_battle_pilot(&mut world, carrier, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, carrier, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    set_battle_towable(&mut world, id, true).unwrap();
    world
        .btech
        .rewrite_unit_record(carrier, |record| {
            record["contacts"][id.0.to_string()] = serde_json::json!({"identified":true});
        })
        .unwrap();
    let before = world.btech.clone();
    assert!(set_battle_tow(&mut world, carrier, Some(id)).is_err());
    assert_eq!(world.btech, before);
    let _ = prepare_battle_pickup(
        &mut world,
        carrier,
        ObjectId(1),
        id,
        BattleMovementRules::STANDARD.fall,
    )
    .unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id].hull_down(),
        BattleHullDownState::default()
    );
    world.validate(&config).unwrap();
}
