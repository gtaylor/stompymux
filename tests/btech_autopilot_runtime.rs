//! End-to-end ground-autopilot lifecycle coverage on isolated fixture worlds.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::{
    BattleMapAsset, BattlePower, BattleUnitSignature, BattleUnitTemplate, Config, Kind, ObjectId,
    Scripts, World, assign_battle_pilot, create_battle_map, persistence, place_battle_unit,
    refresh_battle_contacts, set_battle_speed, set_battle_unit_signature,
};

#[allow(dead_code)]
#[path = "support/btech_firing.rs"]
mod firing;

/// Build a placed, running and piloted JR7-D on a private straight battlefield.
async fn ground_fixture() -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (directory, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Autopilot lane".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "autopilot.lane",
        BattleMapAsset::parse(&format!("1 8\n{}", ".0\n".repeat(8))).unwrap(),
    )
    .unwrap();
    let unit = world.create(&config, "Autopilot mech".into(), Kind::Thing);
    world.objects.get_mut(&unit).unwrap().home = Some(ObjectId(config.home()));
    BattleUnitTemplate::parse(include_str!("../game/mechs/JR7-D"))
        .unwrap()
        .create(&mut world, unit)
        .unwrap();
    place_battle_unit(&mut world, unit, map, 0, 7).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(unit);
    assign_battle_pilot(&mut world, unit, ObjectId(1)).unwrap();

    // Starting directly in Running keeps this fixture focused on controller behavior;
    // the ordinary startup state machine is covered by the BTech power scenarios.
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][unit.0.to_string()]["power"] =
        serde_json::to_value(BattlePower::Running).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    world.validate(&config).unwrap();
    (directory, config, world, map, unit)
}

#[tokio::test(flavor = "current_thread")]
async fn move_order_reaches_destination_and_starts_hold_order() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_directory, config, world, map, unit) = ground_fixture().await;
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            scripts
                .eval_callback::<()>(&format!(
                    r#"
                    local u = mux.world.object({unit})
                    local a = btech.autopilot
                    a.attach(u)
                    local result = a.submit(u, {{
                        {{ kind = a.orders.MOVE, destination = {{ map = {map}, x = 0, y = 6 }} }},
                        {{ kind = a.orders.HOLD }},
                    }}, a.submission_modes.APPEND)
                    assert(result.ids[1] == 1 and result.ids[2] == 2)
                    a.resume(u)
                    assert(a.status(u).state == 'executing')
                    "#,
                    unit = unit.0,
                    map = map.0,
                ))
                .unwrap();
            persistence::save(&config.database(), &scripts.world())
                .await
                .unwrap();
            drop(scripts);

            tokio::time::pause();
            let (_address, shutdown, task, _lua) =
                support::start(&config, Rc::new(std::cell::Cell::new(1))).await;
            let mut hold_started = false;
            let mut latest = None;
            for _ in 0..96 {
                tokio::time::advance(std::time::Duration::from_secs(1)).await;
                tokio::task::yield_now().await;
                let loaded = persistence::load(&config.database()).await.unwrap();
                if let Some(controller) = loaded.btech.controllers().get(&unit) {
                    hold_started = controller
                        .active_order()
                        .is_some_and(|record| record.id == 2);
                    latest = Some(loaded);
                    if hold_started {
                        break;
                    }
                }
            }
            let loaded = latest.expect("server committed at least one autopilot tick");
            let position = loaded.btech.constructed_units()[&unit]
                .position()
                .expect("autopilot mech remains placed");
            assert_eq!((position.map, position.x, position.y), (map, 0, 6));
            assert!(hold_started, "Move should complete before Hold starts");
            let controller = loaded.btech.controllers().get(&unit).unwrap();
            assert_eq!(
                controller.state(),
                stompymux_rs::btech::AutopilotState::Executing
            );
            assert_eq!(controller.active_order().unwrap().id, 2);

            shutdown
                .send(stompymux_rs::ShutdownRequest::Sigterm)
                .unwrap();
            tokio::time::resume();
            task.await.unwrap().unwrap();
        })
        .await;
}

#[tokio::test]
async fn successful_manual_speed_pauses_controller_but_rejected_speed_does_not() {
    let (_directory, config, world, _map, unit) = ground_fixture().await;
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
        stompymux_rs::btech::AutopilotState::Executing
    );

    assert!(set_battle_speed(&mut scripts.world_mut(), unit, ObjectId(1), f64::NAN).is_err());
    assert_eq!(
        scripts.world().btech.controllers()[&unit].state(),
        stompymux_rs::btech::AutopilotState::Executing,
        "a rejected player action must not take over the controller"
    );
    set_battle_speed(&mut scripts.world_mut(), unit, ObjectId(1), 10.75).unwrap();
    let live_world = scripts.world();
    let controller = live_world.btech.controllers().get(&unit).unwrap();
    assert_eq!(
        controller.state(),
        stompymux_rs::btech::AutopilotState::Paused
    );
    assert!(controller.feedback_records().iter().any(|feedback| {
        feedback.event == stompymux_rs::btech::AutopilotFeedbackEvent::ManualTakeover
    }));
}

#[tokio::test]
async fn controller_intent_revision_and_queue_survive_persistence_restart() {
    let (_directory, config, world, map, unit) = ground_fixture().await;
    let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
    scripts
        .eval_callback::<()>(&format!(
            r#"
            local u = mux.world.object({unit})
            local a = btech.autopilot
            a.attach(u, {{ speed_percent = 75, heat_ceiling = 12 }})
            local result = a.submit(u, {{
                {{ kind = a.orders.MOVE, destination = {{ map = {map}, x = 0, y = 6 }} }},
                {{ kind = a.orders.HOLD }},
            }}, a.submission_modes.APPEND)
            assert(result.revision == 1)
            a.resume(u)
            "#,
            unit = unit.0,
            map = map.0,
        ))
        .unwrap();
    let before = scripts.world().btech.controllers()[&unit].clone();
    persistence::save(&config.database(), &scripts.world())
        .await
        .unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    let after = restored.btech.controllers().get(&unit).unwrap();
    assert_eq!(after, &before);
    assert_eq!(after.config().speed_percent, 75);
    assert_eq!(after.config().heat_ceiling, 12);
    assert_eq!(after.revision(), 2);
    assert_eq!(after.queued_orders().len(), 2);
    assert_eq!(after.queued_orders()[0].id, 1);
    assert_eq!(after.queued_orders()[0].order.kind_code(), 0);
    assert_eq!(
        after.queued_orders()[0].order.clone(),
        stompymux_rs::btech::AutopilotOrder::Move {
            destination: stompymux_rs::BattlePosition { map, x: 0, y: 6 },
            arrival_radius: 0,
        }
    );
}

#[tokio::test]
async fn explicit_attack_uses_filtered_sensor_observation() {
    let (_directory, config, mut world, shooter, target, _weapon) = firing::fixture_with_target(
        include_str!("../game/mechs/JR7-D"),
        None,
        include_str!("../game/mechs/JR7-D"),
    )
    .await;
    // Give the explicit target an opposing durable sensor signature so the order
    // admission path exercises hostile-contact filtering rather than a friendly
    // fire refusal.
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
    let (contact_count, weapon_readiness, target_seen) = scripts
        .eval_callback::<(u32, bool, bool)>(&format!(
            r#"
            local u = mux.world.object({shooter})
            local a = btech.autopilot
            a.attach(u)
            a.configure(u, {{ fire_mode = a.fire_modes.ASSIGNED_TARGET }})
            local result = a.submit(u, {{ {{ kind = a.orders.ATTACK, target = {target} }} }}, a.submission_modes.APPEND)
            assert(result.ids[1] == 1)
            a.resume(u)
            local observation = a.observe(u)
            local seen = false
            for _, contact in ipairs(observation.contacts) do
                assert(contact.armor == nil and contact.ammunition == nil and contact.capabilities == nil)
                if contact.unit == {target} then
                    seen = contact.identified
                end
            end
            return #observation.contacts, observation.own.weapons ~= nil, seen
            "#,
            shooter = shooter.0,
            target = target.0,
        ))
        .unwrap();
    assert!(
        contact_count > 0,
        "sensor-visible target should be represented"
    );
    assert!(
        weapon_readiness,
        "own readiness is part of the tactical observation"
    );
    assert!(
        target_seen,
        "explicit attack must use an identified sensor contact"
    );
    let live_world = scripts.world();
    let controller = live_world.btech.controllers().get(&shooter).unwrap();
    assert_eq!(
        controller.state(),
        stompymux_rs::btech::AutopilotState::Executing
    );
    assert!(matches!(
        controller.active_order().or_else(|| controller.queued_orders().first()),
        Some(record)
            if matches!(record.order, stompymux_rs::btech::AutopilotOrder::Attack { target: id, .. } if id == target)
    ));
}
