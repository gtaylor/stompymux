//! Wizard map membership preserves removal/re-entry state and shares native/Lua rollback.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing;
use crate::support;

/// Removing a live unit persists its pose, supports immediate re-entry, then shuts down on update.
#[tokio::test]
async fn removal_reentry_and_update_are_durable_for_all_chassis() {
    for source in firing::templates() {
        let (_dir, config, world, unit, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        let map = world.objects[&unit].location.unwrap();
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let text = support::run_text(&native, &config, ObjectId(1), 1, "setmapindx -1");
        assert!(text.contains("Mech removed from map."), "{text}");
        lua.eval_callback::<mlua::Table>(&format!(
            "return btech.unit.setmapindex(1, {}, -1)",
            unit.0
        ))
        .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        let removed = native.world().clone();
        let key = if removed.btech.vehicles().contains_key(&unit) {
            "vehicles"
        } else {
            "constructed"
        };
        let before = serde_json::to_value(&world.btech).unwrap();
        let after = serde_json::to_value(&removed.btech).unwrap();
        for field in ["power", "pilot", "motion", "position", "dice", "sections"] {
            assert_eq!(
                before[key][unit.0.to_string()][field],
                after[key][unit.0.to_string()][field],
                "{field}"
            );
        }
        assert!(removed.btech.units()[&unit].map.is_none());
        assert_eq!(removed.objects[&unit].location, Some(map));
        removed.validate(&config).unwrap();
        persistence::save(&config.database(), &removed)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(restored.btech, removed.btech);
        let report = reassign_battle_map(&mut restored, unit, map, Some("RX")).unwrap();
        assert_eq!(report.position.y, 11);
        assert_eq!(
            serde_json::to_value(&restored.btech).unwrap()[key][unit.0.to_string()]["power"],
            before[key][unit.0.to_string()]["power"]
        );
        let mut replay = removed.clone();
        restored = removed.clone();
        let notices = advance_battle_units(&mut restored, 0);
        assert!(
            notices
                .iter()
                .any(|notice| notice.unit == unit && notice.text.contains("invalid map"))
        );
        assert_eq!(notices, advance_battle_units(&mut replay, 0));
        assert_eq!(restored.btech, replay.btech);
        advance_battle_motion(&mut restored, BattleMovementRules::STANDARD).unwrap();
        restored.validate(&config).unwrap();
        let after = serde_json::to_value(&restored.btech).unwrap();
        assert_eq!(after[key][unit.0.to_string()]["power"]["state"], "off");
        assert!(after[key][unit.0.to_string()]["pilot"].is_null());
        reassign_battle_map(&mut restored, unit, map, Some("RX")).unwrap();
        restored.validate(&config).unwrap();
        assert_eq!(battle_unit_elevation(&restored, unit).unwrap(), Some(0));
    }
}

/// Running flight can be removed and saved before the next update cancels its off-map motion.
#[tokio::test]
async fn airborne_removal_replays_and_can_resume_before_update() {
    for index in [0, 6] {
        let source = &firing::templates()[index];
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(source, None, source).await;
        let map = world.objects[&unit].location.unwrap();
        if index == 0 {
            launch_battle_jump(&mut world, unit, ObjectId(1), 0, 3.0).unwrap();
            advance_battle_jumps(&mut world, BattleMovementRules::STANDARD).unwrap();
        } else {
            let _ = begin_battle_vtol_takeoff(&mut world, unit, ObjectId(1), 0, false).unwrap();
            advance_battle_motion(&mut world, BattleMovementRules::STANDARD).unwrap();
        }
        let before = world.btech.clone();
        remove_battle_map_membership(&mut world, unit).unwrap();
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let suspended = restored.btech.clone();
        advance_battle_jumps(&mut restored, BattleMovementRules::STANDARD).unwrap();
        assert_eq!(restored.btech, suspended);
        let mut immediate = restored.clone();
        reassign_battle_map(&mut immediate, unit, map, Some("RX")).unwrap();
        if index == 0 {
            assert_eq!(
                immediate.btech.constructed_units()[&unit].flight(),
                before.constructed_units()[&unit].flight()
            );
        } else {
            assert_eq!(
                immediate.btech.vehicles()[&unit].vtol_flight(),
                before.vehicles()[&unit].vtol_flight()
            );
        }
        advance_battle_units(&mut world, 0);
        advance_battle_units(&mut restored, 0);
        assert_eq!(restored.btech, world.btech);
        advance_battle_motion(&mut restored, BattleMovementRules::STANDARD).unwrap();
        advance_battle_jumps(&mut restored, BattleMovementRules::STANDARD).unwrap();
        restored.validate(&config).unwrap();
        reassign_battle_map(&mut restored, unit, map, Some("RX")).unwrap();
        restored.validate(&config).unwrap();
    }
}

/// Both interfaces publish assignment identically and restore world/dice on a rejected callback.
#[tokio::test]
async fn assignment_interfaces_permissions_and_callback_rollback() {
    let source = &firing::templates()[0];
    let (_dir, config, world, unit, _, _) = firing::fixture_with_target(source, None, source).await;
    let map = world.objects[&unit].location.unwrap();
    let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
    let text = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("setmapindx {} zx", map.0),
    );
    assert!(text.contains("Your ID: ZX"), "{text}");
    lua.eval_callback::<mlua::Table>(&format!(
        "return btech.unit.setmapindex(1, {}, {}, 'zx')",
        unit.0, map.0
    ))
    .unwrap();
    assert_eq!(native.world().btech, lua.world().btech);
    let before = lua.world().clone();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.setmapindex(1, {}, -1); error('reject')",
            unit.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, before.btech);
    for input in [
        "setmapindx",
        "setmapindx -2",
        "setmapindx banana",
        "setmapindx 999999",
        "setmapindx #1",
    ] {
        let before = native.world().clone();
        let _ = support::run_text(&native, &config, ObjectId(1), 1, input);
        assert_eq!(native.world().btech, before.btech, "{input}");
    }
    let visitor = native
        .world_mut()
        .create(&config, "Visitor".into(), Kind::Thing);
    assert!(
        set_battle_map_index_action(&native, &config, visitor, unit, ObjectId(-1), None).is_err()
    );
}

/// A stationary removed vehicle still schedules its deferred shutdown when no other work exists.
#[tokio::test]
async fn detached_vehicle_keeps_the_scheduler_awake() {
    let source = &firing::templates()[5];
    let (_dir, config, mut world, unit, other, _) =
        firing::fixture_with_target(source, None, source).await;
    remove_battle_map_membership(&mut world, other).unwrap();
    advance_battle_units(&mut world, 0);
    for _ in 0..31 {
        advance_battle_reactor_windows(&mut world);
    }
    remove_battle_map_membership(&mut world, unit).unwrap();
    let stats = battle_runtime_stats(&world, &config, ObjectId(1)).unwrap();
    assert_eq!(stats.scanner_observers, 0);
    assert!(stats.simulation_pending);
    advance_battle_units(&mut world, 0);
    assert!(
        !battle_runtime_stats(&world, &config, ObjectId(1))
            .unwrap()
            .simulation_pending
    );
    world.validate(&config).unwrap();
}

/// Failure of the second assignment confirmation restores membership, identity and random stream.
#[tokio::test]
async fn late_output_failure_restores_assignment_and_prior_output() {
    let source = &firing::templates()[0];
    let (dir, _, world, unit, _, _) = firing::fixture_with_target(source, None, source).await;
    let path = dir.path().join("stompymux.toml");
    let mut table: toml::Table = std::fs::read_to_string(&path).unwrap().parse().unwrap();
    table
        .entry("lua")
        .or_insert(toml::Value::Table(toml::Table::new()))
        .as_table_mut()
        .unwrap()
        .insert("output_entry_limit".into(), 2.into());
    std::fs::write(path, toml::to_string(&table).unwrap()).unwrap();
    let config = Config::load(dir.path()).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    let map = scripts.world().objects[&unit].location.unwrap();
    set_battle_map_index_action(&scripts, &config, ObjectId(1), unit, ObjectId(-1), None).unwrap();
    let before = scripts.world().clone();
    let error =
        set_battle_map_index_action(&scripts, &config, ObjectId(1), unit, map, None).unwrap_err();
    assert!(error.to_string().contains("output limit"), "{error:#}");
    assert_eq!(scripts.world().btech, before.btech);
    let outbox = scripts.drain_outbox();
    assert_eq!(outbox.len(), 1);
    assert!(outbox[0].1.source().contains("Mech removed"));
}

/// Startup removal aborts on update, while stopped and never-placed units remain valid and idempotent.
#[tokio::test]
async fn removal_handles_starting_stopped_and_unplaced_units() {
    for source in firing::templates() {
        let (_dir, config, original, unit, _, _) =
            firing::fixture_with_target(&source, None, &source).await;
        for power in [BattlePower::Off, BattlePower::Starting { remaining: 10 }] {
            let mut world = original.clone();
            firing::edit(&mut world, unit, |state| {
                state["power"] = serde_json::to_value(power).unwrap();
                state["target_lock"] = serde_json::Value::Null;
            });
            world.validate(&config).unwrap();
            remove_battle_map_membership(&mut world, unit).unwrap();
            let removed = world.btech.clone();
            remove_battle_map_membership(&mut world, unit).unwrap();
            assert_eq!(removed, world.btech);
            let notices = advance_battle_units(&mut world, 0);
            assert_eq!(
                notices.iter().any(|notice| notice.unit == unit
                    && notice.text.contains("startup sequence has been aborted")),
                matches!(power, BattlePower::Starting { .. })
            );
            world.validate(&config).unwrap();
        }
        let mut world = original;
        let new = world.create(&config, "Unplaced".into(), Kind::Thing);
        world.objects.get_mut(&new).unwrap().home = Some(ObjectId(config.home()));
        BattleUnitTemplate::parse(&source)
            .unwrap()
            .create(&mut world, new)
            .unwrap();
        remove_battle_map_membership(&mut world, new).unwrap();
        world.validate(&config).unwrap();
    }
}

/// Removing the former map does not erase detached coordinates or prevent entry into a new map.
#[tokio::test]
async fn detached_pose_survives_former_map_purge() {
    for index in [0, 2] {
        let source = &firing::templates()[index];
        let (_dir, config, mut world, unit, _, _) =
            firing::fixture_with_target(source, None, source).await;
        let former = world.objects[&unit].location.unwrap();
        remove_battle_map_membership(&mut world, unit).unwrap();
        advance_battle_units(&mut world, 0);
        world
            .objects
            .get_mut(&former)
            .unwrap()
            .flags
            .insert(Flag::Going);
        persistence::save(&config.database(), &world).await.unwrap();
        persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
            dbck::plan(&world, raw, &config)
        })
        .await
        .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let new = restored.create(&config, "New map".into(), Kind::Room);
        create_battle_map(
            &mut restored,
            new,
            "new",
            BattleMapAsset::parse(&format!("1 12\n{}", ".0\n".repeat(12))).unwrap(),
        )
        .unwrap();
        let report = reassign_battle_map(&mut restored, unit, new, Some("XY")).unwrap();
        assert_eq!(report.position.y, 11);
        restored.validate(&config).unwrap();
    }
}
