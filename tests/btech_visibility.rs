//! Operator visibility persists across chassis without replacing ordinary sensor acquisition.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Representative construction for every supported movement class.
fn templates() -> Vec<String> {
    let tracked = include_str!("../game/mechs/Demolisher");
    vec![
        include_str!("../game/mechs/JR7-D").into(),
        include_str!("../game/mechs/GOL-1H").into(),
        tracked.into(),
        tracked.replace("{ Track }", "{ Wheel }"),
        tracked.replace("{ Track }", "{ Hover }"),
        tracked
            .replace("{ Track }", "{ None }")
            .replace("{ 53.75 }", "{ 0 }"),
        include_str!("../game/mechs/Kestrel").into(),
    ]
}

/// Set isolated runtime facts through the persisted representation for either anatomy.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    let collection = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    change(&mut saved[collection][id.0.to_string()]);
    world.btech = serde_json::from_value(saved).unwrap();
}

/// A piloted observer and target on opposite sides of an optional obstructing hill.
async fn fixture(
    observer: &str,
    target: &str,
    blocked: bool,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Visibility field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "visibility",
        BattleMapAsset::parse(if blocked {
            "1 3\n.0\n.9\n.0\n"
        } else {
            "1 3\n.0\n.0\n.0\n"
        })
        .unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for (index, source) in [observer, target].into_iter().enumerate() {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        BattleUnitTemplate::parse(source)
            .unwrap()
            .create(&mut world, id)
            .unwrap();
        place_battle_unit(&mut world, id, map, 0, if index == 0 { 2 } else { 0 }).unwrap();
        edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
        });
        ids.push(id);
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ids[0]);
    assign_battle_pilot(&mut world, ids[0], ObjectId(1)).unwrap();
    world.validate(&config).unwrap();
    (dir, config, world, ids[0], ids[1])
}

/// Native edits and trusted Lua share validation, callback rollback and exact saved state.
#[tokio::test]
async fn visibility_controls_all_chassis_are_atomic_and_durable() {
    for source in templates() {
        let (_dir, config, mut world, id, _) = fixture(&source, &source, false).await;
        world
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let command = format!("@btech unit-visibility #{id}=both", id = id.0);
        let denied = support::run_text(&scripts, &config, ObjectId(2), 2, &command);
        assert!(denied.contains("Permission"), "{denied}");
        assert_eq!(scripts.world().btech, world.btech);
        let reply = support::run_text(&scripts, &config, ObjectId(1), 1, &command);
        assert!(
            reply.contains("invisible=true, clairvoyant=true"),
            "{reply}"
        );
        let before = scripts.world().btech.clone();
        scripts.drain_outbox();
        let call = format!(
            "btech.unit.visibility({}, {{invisible=false,clairvoyant=false}})",
            id.0
        );
        assert!(
            scripts
                .eval_callback::<()>(&(call.clone() + "; error('abort')"))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        assert!(scripts.drain_outbox().is_empty());
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.visibility({}, {{invisible=false}})",
                    id.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        let flags: (bool, bool) = scripts
            .eval_callback(&format!(
                "local v=btech.unit.state({}).visibility; return v.invisible,v.clairvoyant",
                id.0
            ))
            .unwrap();
        assert_eq!(flags, (true, true));
        let detached: bool = scripts.eval_callback(&format!("local v=btech.unit.visibility({}); v.invisible=false; return btech.unit.visibility({}).invisible",id.0,id.0)).unwrap();
        assert!(detached);
        let saved_world = scripts.world().clone();
        persistence::save(&config.database(), &saved_world)
            .await
            .unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(loaded.btech, scripts.world().btech);
        assert_eq!(
            battle_visibility(&loaded, id).unwrap(),
            BattleVisibility {
                invisible: true,
                clairvoyant: true
            }
        );
        scripts.eval_callback::<()>(&call).unwrap();
        assert_eq!(scripts.world().btech, world.btech);
        let mut going = world.clone();
        going
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::Going);
        assert!(
            set_battle_visibility(
                &mut going,
                id,
                BattleVisibility {
                    invisible: true,
                    clairvoyant: false
                }
            )
            .is_err()
        );
        assert_eq!(going.btech, world.btech);
        loaded.validate(&config).unwrap();
    }
}

/// Invisibility hides cached contacts immediately and the next scan reports their loss without new rolls.
#[tokio::test]
async fn visibility_sensor_loss_and_clairvoyant_contacts_cross_all_chassis() {
    for observer_source in templates() {
        for target_source in templates() {
            let (_dir, config, mut world, observer, target) =
                fixture(&observer_source, &target_source, false).await;
            edit(&mut world, observer, |state| {
                state["contacts"] = serde_json::json!({target.0.to_string(): {"primary":true,"secondary":true,"identified":true}});
            });
            let geometry = battle_unit_terrain_los(&world, observer, target).unwrap();
            assert!(
                visible_battle_contact(&world, observer, target)
                    .unwrap()
                    .is_some()
            );
            for sensor in [
                BattleSensorMode::Visual,
                BattleSensorMode::LightAmplification,
            ] {
                assert!(
                    battle_map_optical_contact(&world, observer, target, sensor, false, false)
                        .unwrap()
                        .eligible
                );
            }
            set_battle_visibility(
                &mut world,
                target,
                BattleVisibility {
                    invisible: true,
                    clairvoyant: false,
                },
            )
            .unwrap();
            let hidden = world.btech.clone();
            assert!(
                visible_battle_contact(&world, observer, target)
                    .unwrap()
                    .is_none()
            );
            assert!(battle_observer_messages(&world, target, "moves").is_empty());
            for sensor in [
                BattleSensorMode::Visual,
                BattleSensorMode::LightAmplification,
            ] {
                let report =
                    battle_map_optical_contact(&world, observer, target, sensor, false, false)
                        .unwrap();
                assert!(!report.eligible);
                assert_eq!(report.acquisition_factor, 0);
            }
            assert_eq!(world.btech, hidden);
            let events = refresh_optical_scanners(&mut world, &[observer]).unwrap();
            assert!(
                events
                    .iter()
                    .any(|event| event.target == target && !event.acquired)
            );
            set_battle_visibility(
                &mut world,
                observer,
                BattleVisibility {
                    invisible: false,
                    clairvoyant: true,
                },
            )
            .unwrap();
            let unchanged = world.btech.clone();
            let contact = visible_battle_contact(&world, observer, target)
                .unwrap()
                .unwrap();
            assert!(contact.identified);
            assert!(!contact.sensors.primary && !contact.sensors.secondary);
            assert_eq!(visible_battle_contacts(&world, observer).unwrap().len(), 1);
            assert_eq!(battle_observer_messages(&world, target, "moves").len(), 1);
            assert_eq!(
                battle_unit_terrain_los(&world, observer, target).unwrap(),
                geometry
            );
            assert_eq!(world.btech, unchanged);
            world.validate(&config).unwrap();
        }
    }
}

/// Clairvoyance exposes blocked terrain and unacquired units but leaves the firing penalty and geometry intact.
#[tokio::test]
async fn clairvoyance_preserves_physical_los_and_unacquired_aim() {
    for source in templates() {
        let (_dir, config, mut world, observer, target) =
            fixture(&source, include_str!("../game/mechs/JR7-D"), true).await;
        let hex = BattleHexCoordinate { x: 0, y: 0 };
        assert!(
            battle_unit_terrain_los(&world, observer, target)
                .unwrap()
                .blocked
        );
        assert!(!battle_hex_visible(&world, observer, hex).unwrap());
        assert!(
            visible_battle_contacts(&world, observer)
                .unwrap()
                .is_empty()
        );
        set_battle_visibility(
            &mut world,
            observer,
            BattleVisibility {
                invisible: false,
                clairvoyant: true,
            },
        )
        .unwrap();
        let before = world.btech.clone();
        assert!(battle_hex_visible(&world, observer, hex).unwrap());
        assert_eq!(
            battle_hex_sensor_visibility(&world, observer, hex).unwrap(),
            BattleContactSensors::default()
        );
        assert!(battle_hex_visible(&world, observer, BattleHexCoordinate { x: 9, y: 9 }).is_err());
        let view = visible_battle_contact(&world, observer, target)
            .unwrap()
            .unwrap();
        assert!(view.identified);
        assert_eq!(view.sensors, BattleContactSensors::default());
        let rules = BattleAimRules {
            woods_damage: false,
            dig_bonus: 3,
            dig_only_front: false,
            hit_arc_mode: 0,
            fasa_turning: false,
            extended_movement: false,
            extended_ranges: false,
            hotload_half_minimum: false,
            override_weapon_arcs: true,
        };
        let aim = battle_aim_modifiers(&world, observer, target, 0, 4, rules).unwrap();
        assert_eq!(aim.optical.unwrap().modifier, 10_000);
        assert!(
            battle_unit_terrain_los(&world, observer, target)
                .unwrap()
                .blocked
        );
        assert_eq!(world.btech, before);
        world.validate(&config).unwrap();
    }
}

/// Every sensor family rejects invisible signatures, even for a clairvoyant operator with working hardware.
#[tokio::test]
async fn invisibility_suppresses_all_sensor_families_without_acquisition_rolls() {
    let source = include_str!("../game/mechs/JR7-D").replace("Left_Torso\n", "Left_Torso\n    CRIT_3-4 { BeagleProbe - - }\n    CRIT_5 { Light_BAP - - }\n    CRIT_6-8 { BloodhoundProbe - - }\n") + "\nSpecials { AntiAircraft }\n";
    let (_dir, config, base, observer, target) =
        fixture(&source, include_str!("../game/mechs/JR7-D"), false).await;
    for mode in [
        BattleSensorMode::Visual,
        BattleSensorMode::LightAmplification,
        BattleSensorMode::Infrared,
        BattleSensorMode::Seismic,
        BattleSensorMode::Electromagnetic,
        BattleSensorMode::Radar,
        BattleSensorMode::BeagleProbe,
        BattleSensorMode::LightProbe,
        BattleSensorMode::BloodhoundProbe,
    ] {
        let mut world = base.clone();
        configure_battle_sensor_policy(&mut world, true);
        if mode == BattleSensorMode::Radar {
            edit(&mut world, target, |state| {
                state["ground_elevation"] = 5.into()
            });
        }
        let ordinary =
            battle_map_optical_contact(&world, observer, target, mode, false, false).unwrap();
        assert!(
            ordinary.eligible,
            "{mode:?} must have a detectable control signature"
        );
        set_battle_visibility(
            &mut world,
            target,
            BattleVisibility {
                invisible: true,
                clairvoyant: false,
            },
        )
        .unwrap();
        set_battle_visibility(
            &mut world,
            observer,
            BattleVisibility {
                invisible: false,
                clairvoyant: true,
            },
        )
        .unwrap();
        let before = world.btech.clone();
        let report =
            battle_map_optical_contact(&world, observer, target, mode, false, false).unwrap();
        assert!(!report.eligible);
        assert_eq!(report.acquisition_factor, 0);
        let scan = scan_battle_optical_target(
            &mut world,
            observer,
            target,
            BattleSensorScan {
                primary: mode,
                secondary: mode,
                visual_disabled: false,
                amplification_disabled: false,
                perception: 0,
                target: BattleScanTarget {
                    lit: false,
                    hostile: true,
                    hidden: false,
                },
            },
        )
        .unwrap();
        assert!(scan.detected_by.is_none());
        assert_eq!(world.btech, before);
        world.validate(&config).unwrap();
    }
}
