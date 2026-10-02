//! Wreck retirement preserves objects, shared chassis behavior and transactional callbacks.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Inspect the durable shared timer without exposing a mutable production interface.
fn remaining(world: &World, id: ObjectId) -> Option<u64> {
    serde_json::to_value(&world.btech).unwrap()["wrecks"][id.0.to_string()].as_u64()
}

/// Create one independently owned native unit on a small shared battlefield.
fn unit(world: &mut World, config: &Config, map: ObjectId, chassis: &str) -> ObjectId {
    let id = world.create(config, chassis.into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    if matches!(chassis, "biped" | "quad") {
        create_battle_unit(
            world,
            id,
            BattleTemplate::parse(
                "test",
                if chassis == "quad" {
                    include_str!("../game/mechs/GOL-1H.toml")
                } else {
                    include_str!("../game/mechs/Daishi-H.toml")
                },
            )
            .unwrap(),
        )
        .unwrap();
    } else {
        let source = match chassis {
            "vtol" => include_str!("../game/mechs/Kestrel.toml").to_owned(),
            "wheeled" => include_str!("../game/mechs/Demolisher.toml")
                .replace("movement = \"track\"", "movement = \"wheel\""),
            "hover" => include_str!("../game/mechs/Demolisher.toml")
                .replace("movement = \"track\"", "movement = \"hover\""),
            "stationary" => include_str!("../game/mechs/Demolisher.toml")
                .replace("movement = \"track\"", "movement = \"none\"")
                .replace("walk_mp = 5", "walk_mp = 0"),
            _ => include_str!("../game/mechs/Demolisher.toml").to_owned(),
        };
        create_battle_vehicle(
            world,
            id,
            BattleVehicleTemplate::parse("test", &source).unwrap(),
        )
        .unwrap();
    }
    place_battle_unit(world, id, map, 0, 0).unwrap();
    id
}

/// A reactor finishes its own wreck and, for vehicles, a wreck with one remaining rear point.
async fn fixture(
    chassis: &str,
    ic: bool,
) -> (tempfile::TempDir, Config, Scripts, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Retirement field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "wreck",
        BattleMapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    let id = unit(&mut world, &config, map, chassis);
    let source = if world.btech.vehicles().contains_key(&id) {
        let mut vehicle = world.btech.vehicles()[&id].clone();
        let sections: Vec<_> = vehicle.sections().keys().copied().collect();
        for section in sections {
            let internal = vehicle.sections()[&section].internal;
            let amount = if section == BattleVehicleSection::Rear {
                internal.saturating_sub(1)
            } else {
                internal
            };
            vehicle
                .damage_phase(section, amount, BattleDamagePhase::Internal)
                .unwrap();
        }
        let mut encoded = serde_json::to_value(&vehicle).unwrap();
        encoded["sections"]["rear"]["armor"] = 0.into();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["vehicles"][id.0.to_string()] = encoded;
        world.btech = serde_json::from_value(state).unwrap();
        unit(&mut world, &config, map, "biped")
    } else {
        id
    };
    if ic {
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
    }
    set_minefield(
        &mut world,
        map,
        42,
        Some(BattleMinefield {
            coordinate: BattleHexCoordinate { x: 0, y: 0 },
            kind: BattleMineKind::Command,
            strength: 5,
            extra: 0,
            owner: id,
        }),
    )
    .unwrap();
    // Find a fixed victim stream that hits its remaining rear section in the reactor packets.
    for seed in 0..=255u8 {
        let mut candidate = world.clone();
        let mut state = serde_json::to_value(&candidate.btech).unwrap();
        if source != id {
            state["vehicles"][id.0.to_string()]["dice"] =
                serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap();
        }
        candidate.btech = serde_json::from_value(state).unwrap();
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(candidate))).unwrap();
        reactor_explosion_action(&scripts, &config, source).unwrap();
        if source == id
            || scripts.world().btech.vehicles()[&id]
                .sections()
                .values()
                .all(|section| section.internal == 0)
        {
            return (dir, config, scripts, id, map);
        }
    }
    panic!("No complete vehicle wreck");
}

/// Every supported chassis keeps one ten-tick timer and preserves game-object-owned mines on retirement.
#[tokio::test]
async fn wreck_retirement_cross_chassis_and_restart() {
    for chassis in [
        "biped",
        "quad",
        "tracked",
        "wheeled",
        "hover",
        "stationary",
        "vtol",
    ] {
        let (_dir, config, scripts, id, map) = fixture(chassis, true).await;
        assert_eq!(remaining(&scripts.world(), id), Some(10), "{chassis}");
        for tick in 1..=9 {
            assert!(
                advance_battle_wrecks_action(&scripts, &config)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(remaining(&scripts.world(), id), Some(10 - tick));
        }
        let before = scripts.world().clone();
        let rolls = before.battle_roll_statistics().unwrap();
        assert!(rolls.total() > 0);
        let live = Scripts::new(&config, Rc::new(RefCell::new(before.clone()))).unwrap();
        assert_eq!(advance_battle_wrecks_action(&live, &config).unwrap(), [id]);
        assert_eq!(live.world().battle_roll_statistics().unwrap(), rolls);
        assert!(live.world().btech_retired_rolls.total() > 0);
        persistence::save(&config.database(), &before)
            .await
            .unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, before.btech);
        assert_eq!(loaded.battle_roll_statistics().unwrap().total(), 0);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(loaded))).unwrap();
        assert_eq!(
            advance_battle_wrecks_action(&scripts, &config).unwrap(),
            [id]
        );
        let after = scripts.world().clone();
        assert!(!after.btech.units().contains_key(&id));
        assert!(!after.btech.registrations().contains_key(&id));
        assert!(!battle_wrecks_pending(&after));
        assert_eq!(after.objects[&id].kind, Kind::Thing);
        assert_eq!(
            after.objects[&id].location,
            Some(ObjectId(config.battletech.usedmechstore))
        );
        for flag in [Flag::Going, Flag::Dark, Flag::Zombie] {
            assert!(after.objects[&id].flags.contains(flag));
        }
        let mut before_map = serde_json::to_value(&before.btech.maps()[&map]).unwrap();
        let mut after_map = serde_json::to_value(&after.btech.maps()[&map]).unwrap();
        let extent = after_map
            .as_object_mut()
            .unwrap()
            .remove("membership_extent")
            .unwrap();
        assert_eq!(
            extent,
            if before.btech.vehicles().contains_key(&id) {
                2
            } else {
                0
            }
        );
        before_map
            .as_object_mut()
            .unwrap()
            .remove("membership_extent");
        assert_eq!(after_map, before_map);
        persistence::save(&config.database(), &after).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, after.btech);
        assert_eq!(loaded.objects[&id].location, after.objects[&id].location);
        assert_eq!(loaded.objects[&id].flags, after.objects[&id].flags);
    }
}

/// Database cleanup retains both chassis journals on its candidate without changing the source.
#[tokio::test]
async fn database_purge_retains_unit_rolls_without_mutating_the_source_world() {
    for chassis in ["biped", "tracked"] {
        let (_dir, config, scripts, id, _) = fixture(chassis, true).await;
        let mut before = scripts.world().clone();
        before
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::Going);
        let rolls = before.battle_roll_statistics().unwrap();
        assert!(rolls.total() > 0);
        let links = dbck::rebuild_links(&before, &before.links);
        let (after, report) = dbck::plan(&before, &links, &config).unwrap();
        assert!(report.plan.purges.contains(&id));
        assert!(!after.btech.units().contains_key(&id));
        assert_eq!(after.battle_roll_statistics().unwrap(), rolls);
        assert!(after.btech_retired_rolls.total() > 0);
        assert_eq!(before.battle_roll_statistics().unwrap(), rolls);
        assert_eq!(before.btech_retired_rolls.total(), 0);
        before.validate(&config).unwrap();
        let live = Scripts::new(&config, Rc::new(RefCell::new(before.clone()))).unwrap();
        let error = live
            .eval_callback::<()>("mux.check_db(); error('undo cleanup')")
            .unwrap_err();
        assert!(error.to_string().contains("undo cleanup"), "{error}");
        assert_eq!(live.world().battle_roll_statistics().unwrap(), rolls);
        assert_eq!(live.world().btech_retired_rolls.total(), 0);
        assert!(live.world().btech.units().contains_key(&id));
        live.eval_callback::<()>("mux.check_db()").unwrap();
        assert!(!live.world().btech.units().contains_key(&id));
        assert_eq!(live.world().battle_roll_statistics().unwrap(), rolls);
        assert!(live.world().btech_retired_rolls.total() > 0);
    }
}

/// OOC wrecks never acquire a cleanup event; changing flags does not cancel an admitted IC event.
#[tokio::test]
async fn wreck_cleanup_admission_is_in_character_only() {
    let (_dir, config, scripts, id, _) = fixture("biped", false).await;
    assert_eq!(remaining(&scripts.world(), id), None);
    assert!(
        advance_battle_wrecks_action(&scripts, &config)
            .unwrap()
            .is_empty()
    );
    let (_dir, config, scripts, id, _) = fixture("biped", true).await;
    // Flag changes after admission preserve the already scheduled event.
    scripts
        .world_mut()
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .remove(Flag::InCharacter);
    for _ in 0..10 {
        advance_battle_wrecks_action(&scripts, &config).unwrap();
    }
    assert!(!scripts.world().btech.units().contains_key(&id));
}

/// Departure can inspect native state; move sees retirement flags, and either callback can roll everything back.
#[tokio::test]
async fn wreck_callbacks_order_and_failure_rollback() {
    use std::cell::Cell;
    let (_dir, config, scripts, id, map) = fixture("biped", true).await;
    for _ in 0..9 {
        advance_battle_wrecks_action(&scripts, &config).unwrap();
    }
    scripts.drain_outbox();
    let before = scripts.world().clone();
    let shared = Rc::new(RefCell::new(before.clone()));
    let scripts = Scripts::new(&config, shared.clone()).unwrap();
    let parents: mlua::Table = scripts
        .inspect_lua()
        .named_registry_value("mux.parents")
        .unwrap();
    let map_parent: mlua::Table = parents
        .get(before.objects[&map].lua_parent.as_str())
        .unwrap();
    let unit_parent: mlua::Table = parents
        .get(before.objects[&id].lua_parent.as_str())
        .unwrap();
    let fail = Rc::new(Cell::new(1));
    let trace = Rc::new(RefCell::new(Vec::new()));
    let events = scripts.inspect_lua().create_table().unwrap();
    let (seen, state, failure) = (trace.clone(), shared.clone(), fail.clone());
    events
        .set(
            "on_leave",
            scripts
                .inspect_lua()
                .create_function(move |_, ctx: mlua::Table| {
                    assert_eq!(ctx.get::<i64>("cause")?, id.0);
                    assert_eq!(ctx.get::<i64>("source")?, map.0);
                    assert_eq!(ctx.get::<Option<i64>>("destination")?, None);
                    assert!(state.borrow().btech.units().contains_key(&id));
                    seen.borrow_mut().push("leave");
                    state.borrow_mut().objects.get_mut(&id).unwrap().name =
                        "Callback mutation".into();
                    if failure.get() == 1 {
                        return Err(mlua::Error::external("wreck leave failed"));
                    }
                    Ok(())
                })
                .unwrap(),
        )
        .unwrap();
    // Quiet destination entry must not invoke its ordinary arrival event.
    events
        .set(
            "on_enter",
            scripts
                .inspect_lua()
                .create_function(|_, _: mlua::Value| -> mlua::Result<()> {
                    Err(mlua::Error::external(
                        "unexpected retirement entry callback",
                    ))
                })
                .unwrap(),
        )
        .unwrap();
    map_parent.set("events", events).unwrap();
    let events = scripts.inspect_lua().create_table().unwrap();
    let (seen, state, failure) = (trace.clone(), shared.clone(), fail.clone());
    events
        .set(
            "on_move",
            scripts
                .inspect_lua()
                .create_function(move |_, ctx: mlua::Table| {
                    assert_eq!(ctx.get::<i64>("cause")?, 1);
                    assert_eq!(ctx.get::<String>("operation")?, "teleport");
                    assert!(!state.borrow().btech.units().contains_key(&id));
                    assert!(state.borrow().objects[&id].flags.contains(Flag::Going));
                    seen.borrow_mut().push("move");
                    if failure.get() == 2 {
                        return Err(mlua::Error::external("wreck move failed"));
                    }
                    Ok(())
                })
                .unwrap(),
        )
        .unwrap();
    unit_parent.set("events", events).unwrap();
    for stage in [1, 2] {
        fail.set(stage);
        assert!(advance_battle_wrecks_action(&scripts, &config).is_err());
        assert_eq!(
            serde_json::to_value(&*scripts.world()).unwrap(),
            serde_json::to_value(&before).unwrap()
        );
        assert!(scripts.outbox().is_empty());
        assert_eq!(
            scripts.world().battle_roll_statistics().unwrap(),
            before.battle_roll_statistics().unwrap()
        );
        assert_eq!(
            scripts.world().btech_retired_rolls,
            before.btech_retired_rolls
        );
    }
    fail.set(0);
    trace.borrow_mut().clear();
    assert_eq!(
        advance_battle_wrecks_action(&scripts, &config).unwrap(),
        [id]
    );
    assert_eq!(*trace.borrow(), ["leave", "move"]);
    assert_eq!(scripts.world().objects[&id].name, "Callback mutation");
}

/// The idle heartbeat retries native-record deletion and object relocation together after a failed commit.
#[tokio::test]
async fn wreck_idle_server_commit_retry() {
    use sqlx::Connection;
    use std::cell::Cell;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, scripts, id, _) = fixture("biped", true).await;
        for _ in 0..9 { advance_battle_wrecks_action(&scripts, &config).unwrap(); }
        let mut world = scripts.world().clone();
        for _ in 0..31 { advance_battle_reactor_windows(&mut world); }
        // No other running systems are needed to keep the retirement timer alive.
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["crew_recovery"]["remaining"] = 0.into();
        world.btech = serde_json::from_value(state).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("CREATE TRIGGER deny_wreck BEFORE DELETE ON btech_units BEGIN SELECT RAISE(ABORT,'wreck commit failure'); END").execute(&mut sql).await.unwrap();
        let (_, shutdown, task, _) = support::start(&config, Rc::new(Cell::new(1))).await;
        tokio::time::sleep(std::time::Duration::from_millis(1250)).await;
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, world.btech);
        assert_eq!(loaded.objects[&id].location, world.objects[&id].location);
        assert!(!loaded.objects[&id].flags.contains(Flag::Going));
        sqlx::query("DROP TRIGGER deny_wreck").execute(&mut sql).await.unwrap();
        let result = tokio::time::timeout(std::time::Duration::from_secs(4), async {
            loop {
                let loaded = persistence::load(&config.database()).await.unwrap();
                if !loaded.btech.units().contains_key(&id) {
                    assert_eq!(loaded.objects[&id].location, Some(ObjectId(config.battletech.usedmechstore)));
                    assert!(!battle_wrecks_pending(&loaded));
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(100)).await;
            }
        }).await;
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
        result.unwrap();
    }).await;
}

/// Timers reject invalid counters or surviving structure, and ordinary purge also removes the event.
#[tokio::test]
async fn wreck_timer_validation_and_object_purge() {
    let (_dir, config, scripts, id, _) = fixture("biped", true).await;
    let world = scripts.world().clone();
    for count in [0, 11] {
        let mut candidate = world.clone();
        let mut state = serde_json::to_value(&candidate.btech).unwrap();
        state["wrecks"][id.0.to_string()] = count.into();
        candidate.btech = serde_json::from_value(state).unwrap();
        assert!(candidate.validate(&config).is_err());
    }
    let mut candidate = world.clone();
    let mut state = serde_json::to_value(&candidate.btech).unwrap();
    state["constructed"][id.0.to_string()]["sections"]["LeftLeg"]["internal"] = 1.into();
    candidate.btech = serde_json::from_value(state).unwrap();
    assert!(candidate.validate(&config).is_err());
    persistence::save(&config.database(), &world).await.unwrap();
    let mut deleted = world.clone();
    deleted
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    persistence::save(&config.database(), &deleted)
        .await
        .unwrap();
    persistence::repair(&config.database(), config.database.busy_timeout_ms, |raw| {
        dbck::plan(&deleted, raw, &config)
    })
    .await
    .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert!(!loaded.btech.units().contains_key(&id));
    assert!(!battle_wrecks_pending(&loaded));
}

/// Ordinary blast packets skip lost sections; they neither admit old wrecks nor restart an existing timer.
#[tokio::test]
async fn wreck_repeated_ordinary_blast_preserves_cleanup_admission() {
    for chassis in [
        "biped",
        "quad",
        "tracked",
        "wheeled",
        "hover",
        "stationary",
        "vtol",
    ] {
        for admitted in [false, true] {
            let (_dir, config, scripts, id, map) = fixture(chassis, admitted).await;
            if admitted {
                advance_battle_wrecks_action(&scripts, &config).unwrap();
            }
            scripts
                .world_mut()
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            let source = unit(&mut scripts.world_mut(), &config, map, "biped");
            let before = remaining(&scripts.world(), id);
            let report = reactor_explosion_action(&scripts, &config, source).unwrap();
            assert!(report.hits.iter().any(|hit| hit.unit == id));
            assert_eq!(remaining(&scripts.world(), id), before);
        }
    }
}
