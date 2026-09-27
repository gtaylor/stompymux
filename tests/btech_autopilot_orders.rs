//! End-to-end order behavior on isolated, disposable BattleTech worlds.
//!
//! The runtime suite covers the basic move/hold lifecycle.  These scenarios exercise
//! the long-lived orders that continuously make decisions during heartbeat ticks.

use crate::{
    support,
    support::autopilot::{heartbeat_snapshots, heartbeat_snapshots_until},
};
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::btech::{AutopilotOrder, AutopilotOrderState, AutopilotReason, AutopilotState};
use stompymux_rs::{
    BattleMapAsset, BattlePower, BattleUnitSignature, BattleUnitTemplate, BattleVehicleTemplate,
    Config, Kind, ObjectId, Scripts, World, assign_battle_pilot, create_battle_map,
    create_battle_vehicle, place_battle_unit, refresh_battle_contacts, set_battle_unit_signature,
};

struct GroundFixture {
    _directory: tempfile::TempDir,
    config: Config,
    world: World,
    map: ObjectId,
    units: Vec<ObjectId>,
}

#[tokio::test(flavor = "current_thread")]
async fn tracked_wheeled_and_hover_vehicles_accept_and_drive_move_orders() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_directory, config, mut world) = support::isolated_world().await;
            let map = world.create(&config, "Vehicle autopilot map".into(), Kind::Room);
            create_battle_map(
                &mut world,
                map,
                "autopilot.vehicle",
                BattleMapAsset::parse(&format!("6 8\n{}", ".0.0.0.0.0.0\n".repeat(8)))
                    .unwrap(),
            )
            .unwrap();
            let templates = [
                include_str!("../game/mechs/Demolisher"),
                include_str!("../game/mechs/Flatbed_Truck"),
                include_str!("../game/mechs/Fulcrum"),
            ];
            let mut units = Vec::new();
            for (index, template) in templates.into_iter().enumerate() {
                let id = world.create(&config, format!("Autopilot vehicle {index}"), Kind::Thing);
                world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
                create_battle_vehicle(
                    &mut world,
                    id,
                    BattleVehicleTemplate::parse(template).unwrap(),
                )
                .unwrap();
                place_battle_unit(&mut world, id, map, (index * 2) as i64, 5).unwrap();
                units.push(id);
            }
            let mut state = serde_json::to_value(&world.btech).unwrap();
            for id in &units {
                state["vehicles"][id.0.to_string()]["power"] =
                    serde_json::to_value(BattlePower::Running).unwrap();
            }
            world.btech = serde_json::from_value(state).unwrap();
            world.validate(&config).unwrap();
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            scripts.inspect_lua().globals().set("map_id", map.0).unwrap();
            for (index, id) in units.iter().enumerate() {
                scripts.inspect_lua().globals().set("unit_id", id.0).unwrap();
                scripts.inspect_lua().globals().set("dest_x", (index * 2) as i64).unwrap();
                scripts.eval_callback::<()>(
                    r#"
                    local u = mux.world.object(unit_id)
                    local a = btech.autopilot
                    a.attach(u)
                    a.submit(u, {{kind=a.orders.MOVE, destination={map=map_id, x=dest_x, y=3}}}, a.submission_modes.APPEND)
                    a.resume(u)
                    "#,
                ).unwrap();
            }
            let world_snapshot = scripts.world().clone();
            let snapshots = heartbeat_snapshots_until(&config, &world_snapshot, 120, |world| {
                units.iter().all(|id| world.btech.vehicles()[id].position().unwrap().y < 5)
            }).await;
            let latest = snapshots.last().unwrap();
            for id in units {
                let position = latest.btech.vehicles()[&id].position().unwrap();
                assert!(position.y < 5, "vehicle #{id:?} should have moved");
                assert_ne!(latest.btech.controllers()[&id].state(), AutopilotState::Blocked);
            }
        })
        .await;
}

/// Build a small open battlefield and resolve every unit from a real supported template.
/// The second and later units intentionally remain uncrewed so these tests cover the
/// existing no-pilot skill path used by autonomous ground control.
async fn ground_fixture(positions: &[(u16, u16)], pilot_first: bool) -> GroundFixture {
    let (directory, config, mut world) = support::isolated_world().await;

    let map = world.create(&config, "Autopilot orders".into(), Kind::Room);
    let rows = ".0.0.0\n".repeat(16);
    create_battle_map(
        &mut world,
        map,
        "autopilot.orders",
        BattleMapAsset::parse(&format!("3 16\n{rows}")).unwrap(),
    )
    .unwrap();

    let mut units = Vec::with_capacity(positions.len());
    for (index, &(x, y)) in positions.iter().enumerate() {
        let id = world.create(&config, format!("Autopilot unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        BattleUnitTemplate::parse(include_str!("../game/mechs/JR7-D"))
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        place_battle_unit(&mut world, id, map, i64::from(x), i64::from(y)).unwrap();
        units.push(id);
    }

    if pilot_first && let Some(&id) = units.first() {
        world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(id);
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    }

    // Starting powered units keeps each test focused on the order being checked while
    // preserving the ordinary movement and combat admission path.
    let mut state = serde_json::to_value(&world.btech).unwrap();
    for id in &units {
        state["constructed"][id.0.to_string()]["power"] =
            serde_json::to_value(BattlePower::Running).unwrap();
    }
    world.btech = serde_json::from_value(state).unwrap();
    world.validate(&config).unwrap();

    GroundFixture {
        _directory: directory,
        config,
        world,
        map,
        units,
    }
}

fn hex_distance(a: stompymux_rs::BattlePosition, b: stompymux_rs::BattlePosition) -> u32 {
    stompymux_rs::btech::autopilot::navigation::Hex::new(a.x, a.y).distance(
        stompymux_rs::btech::autopilot::navigation::Hex::new(b.x, b.y),
    )
}

fn unit_armor(world: &World, id: ObjectId) -> u32 {
    world.btech.constructed_units()[&id]
        .sections()
        .values()
        .map(|section| u32::from(section.armor))
        .sum()
}

#[tokio::test(flavor = "current_thread")]
async fn follow_order_moves_an_uncrewed_unit_to_its_leader() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let fixture = ground_fixture(&[(0, 10), (0, 15)], false).await;
            let leader = fixture.units[0];
            let follower = fixture.units[1];
            assert!(fixture.world.btech.constructed_units()[&follower].pilot().is_none());

            let scripts = Scripts::new(
                &fixture.config,
                Rc::new(RefCell::new(fixture.world)),
            )
            .unwrap();
            scripts
                .eval_callback::<()>(&format!(
                    r#"
                    local u = mux.world.object({follower})
                    local a = btech.autopilot
                    a.attach(u)
                    a.submit(u, {{ {{ kind = a.orders.FOLLOW, target = {leader}, separation = 2 }} }}, a.submission_modes.APPEND)
                    a.resume(u)
                    "#,
                    follower = follower.0,
                    leader = leader.0,
                ))
                .unwrap();
            let world = scripts.world().clone();
            let snapshots = heartbeat_snapshots_until(&fixture.config, &world, 64, |world| {
                hex_distance(world.btech.constructed_units()[&leader].position().unwrap(),
                    world.btech.constructed_units()[&follower].position().unwrap()) <= 2
            }).await;
            let latest = snapshots.last().unwrap();
            let leader_position = latest.btech.constructed_units()[&leader]
                .position()
                .unwrap();
            let follower_position = latest.btech.constructed_units()[&follower]
                .position()
                .unwrap();
            assert!(follower_position.y < 15, "uncrewed follower should move");
            assert!(hex_distance(leader_position, follower_position) <= 2);
            let controller = latest.btech.controllers().get(&follower).unwrap();
            assert_eq!(controller.state(), AutopilotState::Executing);
            assert!(matches!(
                controller.active_order().map(|record| &record.order),
                Some(AutopilotOrder::Follow { target, .. }) if *target == leader
            ));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn patrol_order_cycles_through_waypoints() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let fixture = ground_fixture(&[(0, 15)], false).await;
            let unit = fixture.units[0];
            let scripts =
                Scripts::new(&fixture.config, Rc::new(RefCell::new(fixture.world))).unwrap();
            scripts
                .eval_callback::<()>(&format!(
                    r#"
                    local u = mux.world.object({unit})
                    local a = btech.autopilot
                    a.attach(u)
                    a.submit(u, {{ {{ kind = a.orders.PATROL, waypoints = {{
                        {{ map = {map}, x = 0, y = 14 }},
                        {{ map = {map}, x = 1, y = 13 }}
                    }} }} }}, a.submission_modes.APPEND)
                    a.resume(u)
                    "#,
                    unit = unit.0,
                    map = fixture.map.0,
                ))
                .unwrap();
            let world = scripts.world().clone();
            let mut passed_first = false;
            let mut completed_cycle = false;
            let snapshots = heartbeat_snapshots_until(&fixture.config, &world, 280, |world| {
                let Some(order) = world.btech.controllers()[&unit].active_order() else {
                    return false;
                };
                if order.progress.waypoint_index == 1 {
                    passed_first = true;
                }
                completed_cycle = passed_first && order.progress.waypoint_index == 0;
                completed_cycle
            })
            .await;

            let visited_first = snapshots.iter().any(|world| {
                world.btech.constructed_units()[&unit]
                    .position()
                    .is_some_and(|position| {
                        hex_distance(
                            position,
                            stompymux_rs::BattlePosition {
                                map: fixture.map,
                                x: 0,
                                y: 14,
                            },
                        ) <= 1
                    })
            });
            let visited_second = snapshots.iter().any(|world| {
                world.btech.constructed_units()[&unit]
                    .position()
                    .is_some_and(|position| {
                        hex_distance(
                            position,
                            stompymux_rs::BattlePosition {
                                map: fixture.map,
                                x: 1,
                                y: 13,
                            },
                        ) <= 1
                    })
            });
            assert!(
                completed_cycle,
                "patrol must wrap back to its first waypoint"
            );
            assert!(visited_first, "patrol should reach its first waypoint");
            assert!(visited_second, "patrol should cycle to its second waypoint");
            let controller = snapshots
                .last()
                .unwrap()
                .btech
                .controllers()
                .get(&unit)
                .unwrap();
            assert_eq!(controller.state(), AutopilotState::Executing);
            assert!(matches!(
                controller.active_order().map(|record| &record.order),
                Some(AutopilotOrder::Patrol { .. })
            ));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn attack_move_pursues_a_visible_contact_then_resumes_destination() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let mut fixture = ground_fixture(&[(0, 11), (2, 6)], true).await;
            let shooter = fixture.units[0];
            let target = fixture.units[1];
            set_battle_unit_signature(
                &mut fixture.world,
                target,
                BattleUnitSignature {
                    team: 1,
                    hidden: false,
                    illuminated: false,
                },
            )
            .unwrap();
            refresh_battle_contacts(&mut fixture.world, &[shooter]).unwrap();

            let scripts = Scripts::new(
                &fixture.config,
                Rc::new(RefCell::new(fixture.world)),
            )
            .unwrap();
            scripts
                .eval_callback::<()>(&format!(
                    r#"
                    local u = mux.world.object({shooter})
                    local a = btech.autopilot
                    a.attach(u, {{ fire_mode = a.fire_modes.OPPORTUNISTIC, heat_ceiling = 0 }})
                    a.submit(u, {{ {{ kind = a.orders.ATTACK_MOVE,
                        destination = {{ map = {map}, x = 0, y = 9 }} }} }}, a.submission_modes.APPEND)
                    a.resume(u)
                    "#,
                    shooter = shooter.0,
                    map = fixture.map.0,
                ))
                .unwrap();
            let world = scripts.world().clone();
            // Approach the weapon-derived band, then force a material leash change.
            let mut snapshots = heartbeat_snapshots_until(&fixture.config, &world, 200, |world| {
                let own=world.btech.constructed_units()[&shooter].position().unwrap();
                let enemy=world.btech.constructed_units()[&target].position().unwrap();
                (2..=3).contains(&hex_distance(own,enemy))
            }).await;
            let mut resumed_world=snapshots.last().unwrap().clone();
            stompymux_rs::transfer_battle_unit(&mut resumed_world,target,stompymux_rs::BattlePosition{map:fixture.map,x:2,y:0}).unwrap();
            refresh_battle_contacts(&mut resumed_world,&[shooter]).unwrap();
            snapshots.extend(heartbeat_snapshots_until(&fixture.config,&resumed_world,200,|world| {
                world.btech.controllers()[&shooter].feedback_records().iter().any(|feedback|feedback.event==stompymux_rs::btech::AutopilotFeedbackEvent::OrderSucceeded)
            }).await);
            let destination = stompymux_rs::BattlePosition {
                map: fixture.map,
                x: 0,
                y: 9,
            };
            let pursued_at = snapshots.iter().position(|world| {
                let shooter_position = world.btech.constructed_units()[&shooter].position();
                let target_position = world.btech.constructed_units()[&target].position();
                shooter_position
                    .zip(target_position)
                    .is_some_and(|(shooter, target)| (2..=3).contains(&hex_distance(shooter, target)))
            });
            let resumed = pursued_at.is_some_and(|index| {
                snapshots[index..].iter().any(|world| {
                    world.btech.constructed_units()[&shooter]
                        .position()
                        .is_some_and(|position| hex_distance(position, destination) == 0)
                })
            });
            assert!(pursued_at.is_some(), "attack-move should pursue the visible contact");
            assert!(resumed, "attack-move should resume its original destination");
            let controller = snapshots.last().unwrap().btech.controllers().get(&shooter).unwrap();
            assert!(controller.feedback_records().iter().any(|feedback| {
                feedback.event == stompymux_rs::btech::AutopilotFeedbackEvent::OrderSucceeded
            }));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn wrong_map_destination_blocks_the_active_order() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let mut fixture = ground_fixture(&[(0, 12)], false).await;
            let unit = fixture.units[0];
            let other_map = fixture
                .world
                .create(&fixture.config, "Other autopilot map".into(), Kind::Room);
            create_battle_map(
                &mut fixture.world,
                other_map,
                "autopilot.other",
                BattleMapAsset::parse(&format!("3 16\n{}", ".0.0.0\n".repeat(16))).unwrap(),
            )
            .unwrap();

            let scripts = Scripts::new(
                &fixture.config,
                Rc::new(RefCell::new(fixture.world)),
            )
            .unwrap();
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
                    map = fixture.map.0,
                ))
                .unwrap();
            let mut world = scripts.world().clone();
            stompymux_rs::transfer_battle_unit(
                &mut world,
                unit,
                stompymux_rs::BattlePosition {
                    map: other_map,
                    x: 0,
                    y: 0,
                },
            )
            .unwrap();
            let snapshots = heartbeat_snapshots(&fixture.config, &world, 4).await;
            let controller = snapshots.last().unwrap().btech.controllers().get(&unit).unwrap();
            assert_eq!(controller.state(), AutopilotState::Blocked);
            assert_eq!(
                controller.active_order().map(|record| record.state),
                Some(AutopilotOrderState::Failed)
            );
            assert!(controller.feedback_records().iter().any(|feedback| {
                feedback.reason == Some(AutopilotReason::MapChanged)
            }));
        })
        .await;
}

#[tokio::test(flavor = "current_thread")]
async fn opportunistic_fire_changes_hostile_target_armor() {
    tokio::task::LocalSet::new()
        .run_until(async {
            let mut fixture = ground_fixture(&[(0, 11), (0, 10)], true).await;
            let shooter = fixture.units[0];
            let target = fixture.units[1];
            set_battle_unit_signature(
                &mut fixture.world,
                target,
                BattleUnitSignature {
                    team: 1,
                    hidden: false,
                    illuminated: false,
                },
            )
            .unwrap();
            refresh_battle_contacts(&mut fixture.world, &[shooter]).unwrap();
            let armor_before = unit_armor(&fixture.world, target);
            let heat_before = fixture.world.btech.constructed_units()[&shooter]
                .heat()
                .stored;

            let scripts =
                Scripts::new(&fixture.config, Rc::new(RefCell::new(fixture.world))).unwrap();
            scripts
                .eval_callback::<()>(&format!(
                    r#"
                    local u = mux.world.object({shooter})
                    local a = btech.autopilot
                    a.attach(u, {{ fire_mode = a.fire_modes.OPPORTUNISTIC }})
                    a.submit(u, {{ {{ kind = a.orders.HOLD }} }}, a.submission_modes.APPEND)
                    a.resume(u)
                    "#,
                    shooter = shooter.0,
                ))
                .unwrap();
            let world = scripts.world().clone();
            let snapshots = heartbeat_snapshots(&fixture.config, &world, 8).await;
            let armor_after = unit_armor(snapshots.last().unwrap(), target);
            let maximum_heat = snapshots
                .iter()
                .map(|world| world.btech.constructed_units()[&shooter].heat().stored)
                .fold(heat_before, f64::max);
            assert!(
                maximum_heat > heat_before || armor_after < armor_before,
                "an opportunistic controller should admit a shot at an identified hostile target"
            );
        })
        .await;
}
