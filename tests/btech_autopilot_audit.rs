//! Focused gameplay audit scenarios for the ground autopilot.
//!
//! These tests use disposable worlds and the trusted in-game Lua facade where
//! possible.  They intentionally stay short: long patrol and encounter traces
//! live in the order/runtime suites, while this file checks boundary behavior
//! that is easy to regress during integration work.

use crate::{
    support,
    support::autopilot::{heartbeat_snapshots, heartbeat_snapshots_until},
};
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};
use stompymux_rs::btech::autopilot::observations;
use stompymux_rs::btech::{AutopilotOrderState, AutopilotReason, AutopilotState, LastSighting};
use stompymux_rs::{
    BattleMapAsset, BattlePosition, BattlePower, BattleUnitSignature, BattleUnitTemplate,
    BattleVehicleTemplate, Config, HeartbeatHarness, Kind, ObjectId, Scripts, World,
    assign_battle_pilot, create_battle_map, create_battle_vehicle, persistence, place_battle_unit,
    refresh_battle_contacts, set_battle_speed, set_battle_unit_signature,
};

use crate::support::btech_firing as firing;

/// Build an uncrewed, running JR7-D on an isolated map.  The fixture is kept
/// unpiloted so these scenarios exercise the ordinary no-pilot skill path.
async fn mech_fixture(
    asset: &str,
    start: (i64, i64),
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (directory, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Autopilot audit map".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "autopilot.audit",
        BattleMapAsset::parse(asset).unwrap(),
    )
    .unwrap();
    let unit = world.create(&config, "Autopilot audit mech".into(), Kind::Thing);
    world.objects.get_mut(&unit).unwrap().home = Some(ObjectId(config.home()));
    BattleUnitTemplate::parse(include_str!("../game/mechs/JR7-D"))
        .unwrap()
        .create(&mut world, unit)
        .unwrap();
    place_battle_unit(&mut world, unit, map, start.0, start.1).unwrap();

    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][unit.0.to_string()]["power"] =
        serde_json::to_value(BattlePower::Running).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    world.validate(&config).unwrap();
    (directory, config, world, map, unit)
}

#[tokio::test(flavor = "current_thread")]
async fn observation_is_safe_before_startup_and_expires_future_or_old_memory() {
    let (_directory, config, mut world, map, unit) = mech_fixture("1 2\n.0\n.0\n", (0, 1)).await;
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][unit.0.to_string()]["power"] =
        serde_json::to_value(BattlePower::Off).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    world.validate(&config).unwrap();

    let sightings = BTreeMap::from([
        (
            ObjectId(101),
            LastSighting {
                position: BattlePosition { map, x: 0, y: 0 },
                seen_at: 0,
            },
        ),
        (
            ObjectId(102),
            LastSighting {
                position: BattlePosition { map, x: 0, y: 0 },
                seen_at: 50,
            },
        ),
    ]);
    let observation = observations::observe_with_memory(&world, unit, 20, &sightings).unwrap();
    assert_eq!(observation.own.power, BattlePower::Off);
    assert!(observation.contacts.is_empty());
    assert_eq!(observation.remembered.len(), 1);
    assert_eq!(observation.remembered[0].unit, ObjectId(101));

    let expired = observations::observe_with_memory(&world, unit, 31, &sightings).unwrap();
    assert!(expired.remembered.is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn wall_barrier_blocks_without_turning_budget_exhaustion_into_unreachable() {
    let (_directory, config, world, map, unit) = mech_fixture("1 3\n.0\n=0\n.0\n", (0, 2)).await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .eval_callback::<()>(&format!(
            r#"
            local u = mux.world.object({unit})
            local a = btech.autopilot
            a.attach(u)
            a.submit(u, {{ {{ kind = a.orders.MOVE,
                destination = {{ map = {map}, x = 0, y = 0 }} }} }}, a.submission_modes.APPEND)
            a.resume(u)
            "#,
            unit = unit.0,
            map = map.0,
        ))
        .unwrap();
    let snapshots = heartbeat_snapshots_until(&config, &scripts.world().clone(), 8, |world| {
        world.btech.controllers()[&unit].state() == AutopilotState::Blocked
    })
    .await;
    let latest = snapshots.last().unwrap();
    let controller = latest.btech.controllers().get(&unit).unwrap();
    assert_eq!(controller.state(), AutopilotState::Blocked);
    assert_eq!(
        controller.blocking_reason(),
        Some(AutopilotReason::Unreachable)
    );
    assert_eq!(
        controller.active_order().map(|order| order.state),
        Some(AutopilotOrderState::Failed)
    );
    assert_eq!(
        latest.btech.constructed_units()[&unit]
            .motion()
            .map(|motion| motion.speed)
            .unwrap_or_default(),
        0.0
    );
}

#[tokio::test(flavor = "current_thread")]
async fn vehicle_ground_classes_are_admitted_on_water_and_bridge_maps() {
    let (directory, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Autopilot surfaces".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "autopilot.surfaces",
        BattleMapAsset::parse("3 4\n.0.0.0\n~0/0.0\n.0~0.0\n.0.0.0\n").unwrap(),
    )
    .unwrap();
    let templates = [
        include_str!("../game/mechs/Demolisher"),
        include_str!("../game/mechs/Flatbed_Truck"),
        include_str!("../game/mechs/Fulcrum"),
    ];
    let starts = [(0_i64, 0_i64), (1, 0), (2, 0)];
    let mut units = Vec::new();
    for (index, template) in templates.into_iter().enumerate() {
        let id = world.create(
            &config,
            format!("Autopilot surface vehicle {index}"),
            Kind::Thing,
        );
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        create_battle_vehicle(
            &mut world,
            id,
            BattleVehicleTemplate::parse(template).unwrap(),
        )
        .unwrap();
        place_battle_unit(&mut world, id, map, starts[index].0, starts[index].1).unwrap();
        units.push(id);
    }
    let mut state = serde_json::to_value(&world.btech).unwrap();
    for id in &units {
        state["vehicles"][id.0.to_string()]["power"] =
            serde_json::to_value(BattlePower::Running).unwrap();
    }
    world.btech = serde_json::from_value(state).unwrap();
    world.validate(&config).unwrap();
    assert_eq!(
        world.btech.maps()[&map].hex(0, 1).unwrap().terrain,
        stompymux_rs::Terrain::Water
    );
    assert_eq!(
        world.btech.maps()[&map].hex(1, 1).unwrap().terrain,
        stompymux_rs::Terrain::Bridge
    );

    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (index, unit) in units.iter().enumerate() {
        scripts
            .eval_callback::<()>(&format!(
                r#"
                local u = mux.world.object({unit})
                local a = btech.autopilot
                a.attach(u)
                a.submit(u, {{ {{ kind = a.orders.MOVE,
                    destination = {{ map = {map}, x = {x}, y = 3 }} }} }}, a.submission_modes.APPEND)
                a.resume(u)
                "#,
                unit = unit.0,
                map = map.0,
                x = 2 - index % 3,
            ))
            .unwrap();
    }
    let snapshots = heartbeat_snapshots_until(&config, &scripts.world().clone(), 72, |world| {
        units.iter().zip(starts).all(|(id, start)| {
            let position = world.btech.vehicles()[id].position().unwrap();
            (i64::from(position.x), i64::from(position.y)) != start
        })
    })
    .await;
    let latest = snapshots.last().unwrap();
    for (index, unit) in units.into_iter().enumerate() {
        let controller = latest.btech.controllers().get(&unit).unwrap();
        assert_ne!(
            controller.blocking_reason(),
            Some(AutopilotReason::Unsupported)
        );
        let position = latest.btech.vehicles()[&unit].position().unwrap();
        assert_ne!(
            (position.x, position.y),
            (starts[index].0 as u16, starts[index].1 as u16),
            "ground vehicle should traverse the surface route"
        );
    }
    drop(directory);
}

#[tokio::test(flavor = "current_thread")]
async fn competing_routes_make_progress_without_permanent_congestion_block() {
    // Leave a margin around both routes.  The live movement adapter probes
    // neighboring hexes while turning, so a narrow edge fixture reports
    // harmless out-of-bounds probes instead of exercising congestion.
    let map_asset = format!("9 12\n{}", (".0".repeat(9) + "\n").repeat(12));
    let (_directory, config, mut world, map, first) = mech_fixture(&map_asset, (3, 6)).await;
    let second = world.create(&config, "Autopilot congestion follower".into(), Kind::Thing);
    world.objects.get_mut(&second).unwrap().home = Some(ObjectId(config.home()));
    BattleUnitTemplate::parse(include_str!("../game/mechs/JR7-D"))
        .unwrap()
        .create(&mut world, second)
        .unwrap();
    place_battle_unit(&mut world, second, map, 5, 6).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][second.0.to_string()]["power"] =
        serde_json::to_value(BattlePower::Running).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    world.validate(&config).unwrap();

    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    for (index, unit) in [first, second].into_iter().enumerate() {
        scripts
            .eval_callback::<()>(&format!(
                r#"
                local u = mux.world.object({unit})
                local a = btech.autopilot
                a.attach(u)
                a.submit(u, {{ {{ kind = a.orders.MOVE,
                    destination = {{ map = {map}, x = {x}, y = 4 }} }} }}, a.submission_modes.APPEND)
                a.resume(u)
                "#,
                unit = unit.0,
                map = map.0,
                // Keep distinct arrival regions so a yielded route can
                // recover after the other controller reaches its goal.
                x = 4 + index,
            ))
            .unwrap();
    }
    // Finish only when both controllers succeed. A nearby goal keeps the
    // successful trace short without weakening the bounded deadlock check.
    let snapshots = heartbeat_snapshots_until(&config, &scripts.world().clone(), 72, |world| {
        [first, second].iter().all(|id| {
            world.btech.controllers()[id]
                .feedback_records()
                .iter()
                .any(|feedback| {
                    feedback.event == stompymux_rs::btech::AutopilotFeedbackEvent::OrderSucceeded
                })
        })
    })
    .await;
    let latest = snapshots.last().unwrap();
    for unit in [first, second] {
        let position = latest.btech.constructed_units()[&unit].position().unwrap();
        assert_eq!(position.y, 4, "controller {unit:?} must finish its route");
        assert!(
            latest.btech.controllers()[&unit]
                .feedback_records()
                .iter()
                .any(|feedback| {
                    feedback.event == stompymux_rs::btech::AutopilotFeedbackEvent::OrderSucceeded
                }),
            "controller {unit:?} must finish without a permanent congestion block"
        );
        assert_ne!(
            latest.btech.controllers()[&unit].blocking_reason(),
            Some(AutopilotReason::Congested),
            "congestion must yield and replan rather than become a permanent block"
        );
    }
}

#[tokio::test(flavor = "current_thread")]
async fn successful_manual_control_takes_over_but_rejected_control_does_not() {
    let (_directory, config, mut world, _map, unit) = mech_fixture("1 2\n.0\n.0\n", (0, 1)).await;
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(unit);
    assign_battle_pilot(&mut world, unit, ObjectId(1)).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .eval_callback::<()>(&format!(
            r#"
            local u = mux.world.object({unit})
            local a = btech.autopilot
            a.attach(u)
            a.submit(u, {{ {{ kind = a.orders.HOLD }} }}, a.submission_modes.APPEND)
            a.resume(u)
            "#,
            unit = unit.0,
        ))
        .unwrap();
    assert_eq!(
        scripts.world().btech.controllers()[&unit].state(),
        AutopilotState::Executing
    );

    assert!(set_battle_speed(&mut scripts.world_mut(), unit, ObjectId(1), f64::NAN).is_err());
    assert_eq!(
        scripts.world().btech.controllers()[&unit].state(),
        AutopilotState::Executing
    );
    set_battle_speed(&mut scripts.world_mut(), unit, ObjectId(1), 10.75).unwrap();
    let current = scripts.world();
    let controller = current.btech.controllers().get(&unit).unwrap();
    assert_eq!(controller.state(), AutopilotState::Paused);
    assert!(controller.feedback_records().iter().any(|feedback| {
        feedback.event == stompymux_rs::btech::AutopilotFeedbackEvent::ManualTakeover
    }));
}

#[tokio::test(flavor = "current_thread")]
async fn weapons_hold_and_heat_ceiling_admit_no_autonomous_shot() {
    let (_directory, config, mut world, shooter, target, _weapon) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        None,
        include_str!("../game/mechs/JR7-D"),
    )
    .await;
    set_battle_unit_signature(
        &mut world,
        target,
        BattleUnitSignature {
            team: 1,
            hidden: false,
            illuminated: false,
        },
    )
    .unwrap();
    refresh_battle_contacts(&mut world, &[shooter]).unwrap();
    let heat_before = world.btech.constructed_units()[&shooter].heat().stored;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .eval_callback::<()>(&format!(
            r#"
            local u = mux.world.object({shooter})
            local a = btech.autopilot
            a.attach(u, {{ fire_mode = a.fire_modes.HOLD }})
            a.submit(u, {{ {{ kind = a.orders.ATTACK, target = {target} }} }}, a.submission_modes.APPEND)
            a.resume(u)
            "#,
            shooter = shooter.0,
            target = target.0,
        ))
        .unwrap();
    let snapshots = heartbeat_snapshots(&config, &scripts.world().clone(), 4).await;
    let maximum_heat = snapshots
        .iter()
        .map(|world| world.btech.constructed_units()[&shooter].heat().stored)
        .fold(heat_before, f64::max);
    assert_eq!(
        maximum_heat, heat_before,
        "weapons hold must suppress firing"
    );

    let world = snapshots.last().unwrap().clone();
    let heat_before = world.btech.constructed_units()[&shooter].heat().stored;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .eval_callback::<()>(&format!(
            r#"
            local u = mux.world.object({shooter})
            local a = btech.autopilot
            a.detach(u)
            a.attach(u, {{ fire_mode = a.fire_modes.OPPORTUNISTIC, heat_ceiling = 0 }})
            a.submit(u, {{ {{ kind = a.orders.HOLD }} }}, a.submission_modes.APPEND)
            a.resume(u)
            "#,
            shooter = shooter.0,
        ))
        .unwrap();
    let snapshots = heartbeat_snapshots(&config, &scripts.world().clone(), 4).await;
    let maximum_heat = snapshots
        .iter()
        .map(|world| world.btech.constructed_units()[&shooter].heat().stored)
        .fold(heat_before, f64::max);
    assert_eq!(
        maximum_heat, heat_before,
        "heat ceiling must suppress firing"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn autonomous_fire_rechecks_heat_between_multiple_mounts() {
    let (_directory, config, mut world, shooter, target, _weapon) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        None,
        include_str!("../game/mechs/JR7-D"),
    )
    .await;
    set_battle_unit_signature(
        &mut world,
        target,
        BattleUnitSignature {
            team: 1,
            hidden: false,
            illuminated: false,
        },
    )
    .unwrap();
    refresh_battle_contacts(&mut world, &[shooter]).unwrap();

    let ordinary_heats: Vec<_> = world.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .filter(|mount| !mount.weapon.is_ams() && !mount.weapon.is_artillery())
        .map(|mount| u16::from(mount.weapon.profile().heat))
        .filter(|heat| *heat > 0)
        .collect();
    assert!(
        ordinary_heats.len() >= 2,
        "fixture must provide at least two ordinary heat-producing mounts"
    );
    let one_shot_ceiling = *ordinary_heats.iter().min().unwrap();
    let heat_before = world.btech.constructed_units()[&shooter].heat().stored;

    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .eval_callback::<()>(&format!(
            r#"
            local u = mux.world.object({shooter})
            local a = btech.autopilot
            a.attach(u, {{ fire_mode = a.fire_modes.OPPORTUNISTIC,
                heat_ceiling = {ceiling} }})
            a.submit(u, {{ {{ kind = a.orders.HOLD }} }}, a.submission_modes.APPEND)
            a.resume(u)
            "#,
            shooter = shooter.0,
            ceiling = one_shot_ceiling,
        ))
        .unwrap();
    let low_ceiling_snapshots = heartbeat_snapshots(&config, &scripts.world().clone(), 4).await;
    let low_ceiling_heat = low_ceiling_snapshots
        .iter()
        .map(|snapshot| snapshot.btech.constructed_units()[&shooter].heat().stored)
        .fold(heat_before, f64::max);
    assert!(
        low_ceiling_heat > heat_before,
        "one ordinary mount should be admitted under its projected heat ceiling"
    );
    assert!(
        low_ceiling_heat <= heat_before + f64::from(one_shot_ceiling),
        "the next mount must be rejected after the first shot reaches the ceiling"
    );

    let scripts = Scripts::new(
        &config,
        Rc::new(RefCell::new(low_ceiling_snapshots.last().unwrap().clone())),
    )
    .unwrap();
    scripts
        .eval_callback::<()>(&format!(
            r#"
            local u = mux.world.object({shooter})
            local a = btech.autopilot
            a.detach(u)
            a.attach(u, {{ fire_mode = a.fire_modes.OPPORTUNISTIC,
                heat_ceiling = 1000 }})
            a.submit(u, {{ {{ kind = a.orders.HOLD }} }}, a.submission_modes.APPEND)
            a.resume(u)
            "#,
            shooter = shooter.0,
        ))
        .unwrap();
    let high_ceiling_snapshots = heartbeat_snapshots(&config, &scripts.world().clone(), 4).await;
    let high_ceiling_heat = high_ceiling_snapshots
        .iter()
        .map(|snapshot| snapshot.btech.constructed_units()[&shooter].heat().stored)
        .fold(low_ceiling_heat, f64::max);
    assert!(
        high_ceiling_heat >= low_ceiling_heat + f64::from(one_shot_ceiling),
        "raising the ceiling should admit another ready ordinary mount"
    );
}

#[tokio::test(flavor = "current_thread")]
async fn failed_heartbeat_commit_restores_motion_intent_and_feedback() {
    let (_directory, config, world, map, unit) =
        mech_fixture("1 5\n.0\n.0\n.0\n.0\n.0\n", (0, 4)).await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .eval_callback::<()>(&format!(
            r#"
            local u = mux.world.object({unit})
            local a = btech.autopilot
            a.attach(u)
            a.submit(u, {{ {{ kind = a.orders.MOVE,
                destination = {{ map = {map}, x = 0, y = 0 }} }} }}, a.submission_modes.APPEND)
            a.resume(u)
            "#,
            unit = unit.0,
            map = map.0,
        ))
        .unwrap();
    let before = scripts.world().clone();
    persistence::save(&config.database(), &before)
        .await
        .unwrap();

    let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(config.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql(
        "CREATE TRIGGER autopilot_audit_fail BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(FAIL,'autopilot audit'); END;",
    )
    .execute(&mut db)
    .await
    .unwrap();
    let mut harness = HeartbeatHarness::new(config.clone(), before.clone()).unwrap();
    let metrics = harness.step_diagnostic(1, true, true).await;
    assert!(metrics.autopilot.notice_trace.is_empty());
    assert!(
        !metrics.committed,
        "the injected snapshot trigger must reject the candidate"
    );
    let after = harness.world();
    assert_eq!(
        after.btech, before.btech,
        "failed commit must not publish motion or feedback"
    );
    sqlx::query("DROP TRIGGER autopilot_audit_fail")
        .execute(&mut db)
        .await
        .unwrap();
    sqlx::Connection::close(db).await.unwrap();
}

/// A shot may commit inside the heartbeat candidate, but a failed database
/// publication must discard its dice, inventory, reports and controller feedback.
#[tokio::test(flavor = "current_thread")]
async fn failed_firing_heartbeat_discards_shots_and_retries_identically() {
    let (_directory, config, mut world, shooter, target, _) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        None,
        include_str!("../game/mechs/JR7-D"),
    )
    .await;
    set_battle_unit_signature(
        &mut world,
        target,
        BattleUnitSignature {
            team: 1,
            hidden: false,
            illuminated: false,
        },
    )
    .unwrap();
    refresh_battle_contacts(&mut world, &[shooter]).unwrap();
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .eval_callback::<()>(&format!(
            r#"
        local a = btech.autopilot
        local u = mux.world.object({})
        a.attach(u, {{ fire_mode = a.fire_modes.OPPORTUNISTIC, heat_ceiling = 1000 }})
        a.submit(u, {{ {{ kind = a.orders.HOLD }} }}, a.submission_modes.APPEND)
        a.resume(u)
    "#,
            shooter.0
        ))
        .unwrap();
    let mut control = HeartbeatHarness::new(config.clone(), scripts.world().clone()).unwrap();
    let mut firing_tick = None;
    for tick in 1..=4 {
        let before = control.world();
        let metrics = control.step_diagnostic(tick, true, true).await;
        assert!(metrics.committed);
        if metrics.autopilot.autonomous_shots > 0 {
            assert_eq!(metrics.autopilot.shots_by_unit.len(), 1);
            assert_eq!(
                metrics.autopilot.shots_by_unit[&shooter],
                metrics.autopilot.autonomous_shots
            );
            firing_tick = Some((
                tick,
                before,
                control.world(),
                metrics.autopilot.notice_trace,
            ));
            break;
        }
    }
    let (tick, before, expected, notices) = firing_tick.expect("fixture must fire");
    persistence::save(&config.database(), &before)
        .await
        .unwrap();
    let mut db = <sqlx::SqliteConnection as sqlx::Connection>::connect_with(
        &sqlx::sqlite::SqliteConnectOptions::new()
            .filename(config.database())
            .foreign_keys(false),
    )
    .await
    .unwrap();
    sqlx::raw_sql("CREATE TRIGGER autopilot_shot_fail BEFORE UPDATE ON snapshot BEGIN SELECT RAISE(FAIL,'shot rollback audit'); END;")
        .execute(&mut db).await.unwrap();
    let mut harness = HeartbeatHarness::new(config.clone(), before.clone()).unwrap();
    let rejected = harness.step_diagnostic(tick, true, true).await;
    assert!(!rejected.committed);
    assert!(rejected.autopilot.notice_trace.is_empty());
    assert!(rejected.autopilot.shots_by_unit.is_empty());
    assert!(rejected.autopilot.congestion_by_unit.is_empty());
    assert!(rejected.autopilot.replans_by_unit.is_empty());
    assert_eq!(rejected.autopilot.autonomous_shots, 0);
    assert_eq!(
        serde_json::to_value(harness.world()).unwrap(),
        serde_json::to_value(&before).unwrap()
    );
    assert_eq!(
        harness.world().battle_roll_statistics().unwrap(),
        before.battle_roll_statistics().unwrap()
    );
    sqlx::query("DROP TRIGGER autopilot_shot_fail")
        .execute(&mut db)
        .await
        .unwrap();
    let retry = harness.step_diagnostic(tick, true, true).await;
    assert!(retry.committed && retry.autopilot.autonomous_shots > 0);
    assert_eq!(retry.autopilot.notice_trace, notices);
    assert_eq!(
        serde_json::to_value(harness.world()).unwrap(),
        serde_json::to_value(expected).unwrap()
    );
    sqlx::Connection::close(db).await.unwrap();
}
