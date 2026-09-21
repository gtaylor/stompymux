//! Automatic acquisition uses the same sensor rules for all supported observer and target classes.
use crate::support;
use stompymux_rs::*;

/// Representative supported chassis with stationary movement explicitly authored.
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

/// Add probes to unoccupied slots without changing the unit's weapons or required equipment.
fn equipment(source: &str) -> BattleUnitTemplate {
    let mut template = BattleUnitTemplate::parse(source).unwrap();
    let (attributes, section, parts) = match &mut template {
        BattleUnitTemplate::Mech(definition) => {
            let section = definition
                .sections
                .iter_mut()
                .find(|(name, section)| {
                    matches!(
                        **name,
                        BattleSection::LeftTorso
                            | BattleSection::RightTorso
                            | BattleSection::CenterTorso
                    ) && (0..12)
                        .filter(|slot| !section.criticals.contains_key(slot))
                        .count()
                        >= 6
                })
                .unwrap()
                .1;
            (
                &mut definition.attributes,
                section,
                vec![
                    "BeagleProbe",
                    "BeagleProbe",
                    "Light_BAP",
                    "BloodhoundProbe",
                    "BloodhoundProbe",
                    "BloodhoundProbe",
                ],
            )
        }
        BattleUnitTemplate::Vehicle(definition) => (
            &mut definition.attributes,
            definition
                .sections
                .get_mut(&BattleVehicleSection::Front)
                .unwrap(),
            vec!["BeagleProbe", "Light_BAP", "BloodhoundProbe"],
        ),
    };
    attributes
        .entry("specials".into())
        .or_default()
        .push_str(" AntiAircraft");
    for name in parts {
        let slot = (0..12)
            .find(|slot| !section.criticals.contains_key(slot))
            .unwrap();
        section.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: name.into(),
                data: "-".into(),
                modes: vec![],
                brand: None,
            },
        );
    }
    template
}

/// Change only explicitly selected scenario state, using the same representation as restart.
fn edit(world: &mut World, id: ObjectId, change: impl FnOnce(&mut serde_json::Value)) {
    let key = if world.btech.vehicles().contains_key(&id) {
        "vehicles"
    } else {
        "constructed"
    };
    let mut saved = serde_json::to_value(&world.btech).unwrap();
    change(&mut saved[key][id.0.to_string()]);
    world.btech = serde_json::from_value(saved).unwrap();
}

/// A running, equipped observer faces an ordinary target down a clear lane.
async fn fixture(
    source: &str,
    target: &str,
    distance: u16,
    blocked: bool,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Sensor lane".into(), Kind::Room);
    let mut rows = vec![".0\n"; usize::from(distance + 1)];
    if blocked {
        rows[1] = ".9\n";
    }
    create_battle_map(
        &mut world,
        map,
        "sensors",
        BattleMapAsset::parse(&format!("1 {}\n{}", distance + 1, rows.concat())).unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for (index, template) in [
        equipment(source),
        BattleUnitTemplate::parse(target).unwrap(),
    ]
    .into_iter()
    .enumerate()
    {
        let id = world.create(&config, format!("Sensor unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        template.create(&mut world, id).unwrap();
        place_battle_unit(
            &mut world,
            id,
            map,
            0,
            if index == 0 { i64::from(distance) } else { 0 },
        )
        .unwrap();
        edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            state["sensor_signal"] =
                serde_json::to_value(BattleSensorSignal::seeded(100, [27; 32]).unwrap()).unwrap();
        });
        ids.push(id);
    }
    configure_battle_sensor_policy(&mut world, true);
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ids[0]);
    assign_battle_pilot(&mut world, ids[0], ObjectId(1)).unwrap();
    world.validate(&config).unwrap();
    (dir, config, world, ids[0], ids[1])
}

/// Nine modes on all 49 chassis pairs enter the automatic scanner and retain the same state after restart.
async fn automatic_scanning_matrix(source: &str) {
    for target_source in templates() {
        let (_dir, config, base, observer, target) =
            fixture(source, &target_source, 3, false).await;
        for mode in [
            BattleSensorMode::Visual,
            BattleSensorMode::LightAmplification,
            BattleSensorMode::Infrared,
            BattleSensorMode::Electromagnetic,
            BattleSensorMode::Seismic,
            BattleSensorMode::Radar,
            BattleSensorMode::BeagleProbe,
            BattleSensorMode::LightProbe,
            BattleSensorMode::BloodhoundProbe,
        ] {
            let mut world = base.clone();
            let vtol = world
                .btech
                .vehicles()
                .get(&target)
                .is_some_and(|unit| unit.definition().is_vtol());
            if mode == BattleSensorMode::Radar && vtol {
                edit(&mut world, target, |state| {
                    state["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
                        phase: BattleVtolFlightPhase::Airborne,
                        altitude: 5.0,
                        ..Default::default()
                    })
                    .unwrap()
                });
            }
            edit(&mut world, observer, |state| {
                state["sensor_selection"]["active"] = serde_json::to_value(BattleSensorPair {
                    primary: mode,
                    secondary: mode,
                })
                .unwrap()
            });
            assert!(
                optical_scanner_observers(&world).contains(&observer),
                "{mode:?} must enter the scanner"
            );
            let expected = match mode {
                BattleSensorMode::Radar => vtol,
                BattleSensorMode::Seismic => {
                    world.btech.vehicles().get(&target).is_none_or(|unit| {
                        !matches!(
                            unit.definition().movement,
                            BattleVehicleMovement::Hover | BattleVehicleMovement::Stationary
                        )
                    })
                }
                _ => true,
            };
            assert_eq!(
                battle_map_optical_contact(&world, observer, target, mode, false, false)
                    .unwrap()
                    .eligible,
                expected,
                "{mode:?}"
            );
            if expected {
                // Serialize once per scenario; each trial only swaps the observer dice.
                let template = serde_json::to_value(&world.btech).unwrap();
                let key = if world.btech.vehicles().contains_key(&observer) {
                    "vehicles"
                } else {
                    "constructed"
                };
                let seed = (0..=255)
                    .find(|value| {
                        let mut state = template.clone();
                        state[key][observer.0.to_string()]["dice"] =
                            serde_json::to_value(BattleDice::seeded([*value; 32])).unwrap();
                        let mut trial = world.clone();
                        trial.btech = serde_json::from_value(state).unwrap();
                        scan_battle_optical_target(
                            &mut trial,
                            observer,
                            target,
                            BattleSensorScan {
                                primary: mode,
                                secondary: mode,
                                visual_disabled: false,
                                amplification_disabled: false,
                                perception: 18,
                                target: BattleScanTarget {
                                    lit: false,
                                    hostile: false,
                                    hidden: false,
                                },
                            },
                        )
                        .unwrap()
                        .detected_by
                        .is_some()
                    })
                    .unwrap();
                edit(&mut world, observer, |state| {
                    state["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
                });
            }
            persistence::save(&config.database(), &world).await.unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            configure_battle_sensor_policy(&mut replay, true);
            let events = refresh_optical_scanners(&mut world, &[observer]).unwrap();
            assert_eq!(
                events
                    .iter()
                    .any(|event| event.target == target && event.acquired),
                expected,
                "{mode:?}"
            );
            assert_eq!(
                events,
                refresh_optical_scanners(&mut replay, &[observer]).unwrap()
            );
            assert_eq!(world.btech, replay.btech);
            assert_eq!(
                visible_battle_contact(&world, observer, target)
                    .unwrap()
                    .is_some(),
                expected
            );
            world.validate(&config).unwrap();
        }
    }
}

/// One shard per observer chassis; templates are listed in `templates`.
#[tokio::test]
async fn automatic_nonvisual_scanning_covers_all_supported_chassis_pairs_01() {
    automatic_scanning_matrix(&templates()[0]).await;
}

#[tokio::test]
async fn automatic_nonvisual_scanning_covers_all_supported_chassis_pairs_02() {
    automatic_scanning_matrix(&templates()[1]).await;
}

#[tokio::test]
async fn automatic_nonvisual_scanning_covers_all_supported_chassis_pairs_03() {
    automatic_scanning_matrix(&templates()[2]).await;
}

#[tokio::test]
async fn automatic_nonvisual_scanning_covers_all_supported_chassis_pairs_04() {
    automatic_scanning_matrix(&templates()[3]).await;
}

#[tokio::test]
async fn automatic_nonvisual_scanning_covers_all_supported_chassis_pairs_05() {
    automatic_scanning_matrix(&templates()[4]).await;
}

#[tokio::test]
async fn automatic_nonvisual_scanning_covers_all_supported_chassis_pairs_06() {
    automatic_scanning_matrix(&templates()[5]).await;
}

#[tokio::test]
async fn automatic_nonvisual_scanning_covers_all_supported_chassis_pairs_07() {
    automatic_scanning_matrix(&templates()[6]).await;
}

/// Fixed probes extend their reach; the shared LOS ceiling still bounds both mobile and fixed radar.
#[tokio::test]
async fn stationary_probe_extension_and_shared_radar_ceiling() {
    let sources = templates();
    for (mode, distance, mobile, fixed) in [
        (BattleSensorMode::LightProbe, 4, false, true),
        (BattleSensorMode::BeagleProbe, 7, false, true),
        (BattleSensorMode::BloodhoundProbe, 10, false, true),
        (BattleSensorMode::Radar, 179, true, true),
        (BattleSensorMode::Radar, 200, false, false),
    ] {
        for (source, expected) in [(&sources[2], mobile), (&sources[5], fixed)] {
            let (_dir, config, mut world, observer, target) =
                fixture(source, &sources[6], distance, false).await;
            if mode == BattleSensorMode::Radar {
                edit(&mut world, target, |state| {
                    state["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
                        phase: BattleVtolFlightPhase::Airborne,
                        altitude: 11.0,
                        ..Default::default()
                    })
                    .unwrap()
                });
            }
            assert_eq!(
                battle_map_optical_contact(&world, observer, target, mode, false, false)
                    .unwrap()
                    .eligible,
                expected,
                "{mode:?}"
            );
            world.validate(&config).unwrap();
        }
    }
}

/// Native/Lua switching, probe-only identification and critical fallback share one acquisition pipeline.
#[tokio::test]
async fn probe_contacts_cross_obstacles_and_reconcile_after_equipment_loss() {
    use std::{cell::RefCell, rc::Rc};
    for source in templates() {
        let (_dir, config, world, observer, target) =
            fixture(&source, include_str!("../game/mechs/Demolisher"), 3, true).await;
        assert!(
            battle_unit_terrain_los(&world, observer, target)
                .unwrap()
                .blocked
        );
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let before = lua.world().btech.clone();
        let call = format!("btech.unit.sensors({},1,'H','H')", observer.0);
        assert!(
            lua.eval_callback::<()>(&(call.clone() + "; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(lua.drain_outbox().is_empty());
        let reply = support::run_text(&native, &config, ObjectId(1), 1, "sensor H H");
        assert!(reply.contains("Wanted"), "{reply}");
        lua.eval_callback::<bool>(&call).unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        for _ in 0..10 {
            assert_eq!(
                advance_battle_sensor_selection(&mut native.world_mut()),
                advance_battle_sensor_selection(&mut lua.world_mut())
            );
        }
        assert_eq!(
            refresh_optical_scanners(&mut native.world_mut(), &[observer]).unwrap(),
            refresh_optical_scanners(&mut lua.world_mut(), &[observer]).unwrap()
        );
        let contact = visible_battle_contact(&native.world(), observer, target)
            .unwrap()
            .unwrap();
        assert!(!contact.identified);
        assert!(contact.sensors.primary && contact.sensors.secondary);
        let shown: (usize, bool) = lua
            .eval_callback(&format!(
                "local c=btech.unit.contacts({}); return #c,c[1].sensors.primary",
                observer.0
            ))
            .unwrap();
        assert_eq!(shown, (1, true));
        let text = support::run_text(&native, &config, ObjectId(1), 1, "contacts");
        assert!(text.contains("something"), "{text}");
        let mut damaged = native.world().clone();
        let _ = select_battle_target(&mut damaged, observer, ObjectId(1), Some(target)).unwrap();
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
        assert_eq!(
            battle_aim_modifiers(&damaged, observer, target, 0, 4, rules)
                .unwrap()
                .optical
                .unwrap()
                .sensor,
            BattleSensorMode::BloodhoundProbe
        );
        if let Some(vehicle) = damaged.btech.vehicles().get(&observer) {
            let location = vehicle
                .loadout()
                .unwrap()
                .systems
                .into_iter()
                .find(|part| part.system == BattleSystem::BloodhoundProbe)
                .unwrap()
                .location;
            destroy_battle_vehicle_critical(&mut damaged, observer, location).unwrap();
        } else {
            let location = damaged.btech.constructed_units()[&observer]
                .loadout()
                .unwrap()
                .systems
                .into_iter()
                .find(|part| part.system == BattleSystem::BloodhoundProbe)
                .unwrap()
                .location;
            let _ = destroy_battle_critical(&mut damaged, observer, location).unwrap();
        }
        assert!(
            !battle_active_probe_contact(&damaged, observer, target, BattleActiveProbe::Bloodhound)
                .unwrap()
                .eligible
        );
        assert!(
            battle_aim_modifiers(&damaged, observer, target, 0, 4, rules)
                .unwrap()
                .optical
                .is_none()
        );
        let events = refresh_optical_scanners(&mut damaged, &[observer]).unwrap();
        assert!(
            events
                .iter()
                .any(|event| event.target == target && !event.acquired)
        );
        assert!(
            visible_battle_contact(&damaged, observer, target)
                .unwrap()
                .is_none()
        );
        persistence::save(&config.database(), &damaged)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            damaged.btech
        );
        damaged.validate(&config).unwrap();
    }
}

/// A launch countdown retains ground contact for seismic observers and targets until actual liftoff.
#[tokio::test]
async fn seismic_keeps_launching_vtols_grounded_until_liftoff() {
    let source = include_str!("../game/mechs/Kestrel");
    let (_dir, config, mut base, observer, target) = fixture(source, source, 3, false).await;
    edit(&mut base, observer, |state| {
        state["sensor_selection"]["active"] = serde_json::to_value(BattleSensorPair {
            primary: BattleSensorMode::Seismic,
            secondary: BattleSensorMode::Seismic,
        })
        .unwrap();
        state["contacts"] = serde_json::json!({target.0.to_string():{"primary":true,"secondary":true,"identified":true}});
    });
    for endpoint in [observer, target] {
        for phase in [
            BattleVtolFlightPhase::Landed,
            BattleVtolFlightPhase::Launching { remaining: 1 },
            BattleVtolFlightPhase::Airborne,
        ] {
            let mut world = base.clone();
            edit(&mut world, endpoint, |state| {
                state["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
                    phase,
                    altitude: if phase == BattleVtolFlightPhase::Airborne {
                        5.0
                    } else {
                        0.0
                    },
                    ..Default::default()
                })
                .unwrap()
            });
            let grounded = phase != BattleVtolFlightPhase::Airborne;
            assert_eq!(
                battle_map_optical_contact(
                    &world,
                    observer,
                    target,
                    BattleSensorMode::Seismic,
                    false,
                    false
                )
                .unwrap()
                .eligible,
                grounded
            );
            let events = refresh_optical_scanners(&mut world, &[observer]).unwrap();
            assert_eq!(
                events
                    .iter()
                    .any(|event| event.target == target && !event.acquired),
                !grounded
            );
            assert_eq!(
                visible_battle_contact(&world, observer, target)
                    .unwrap()
                    .is_some(),
                grounded
            );
            world.validate(&config).unwrap();
        }
    }
}

/// Add one machine gun and a normal bin to unused slots without replacing sensor hardware.
fn install_gatling(world: &mut World, id: ObjectId, supply: u16) -> usize {
    let vehicle = world.btech.vehicles().contains_key(&id);
    let mut template = if vehicle {
        BattleUnitTemplate::Vehicle(world.btech.vehicles()[&id].definition().clone())
    } else {
        BattleUnitTemplate::Mech(world.btech.constructed_units()[&id].definition().clone())
    };
    let section = match &mut template {
        BattleUnitTemplate::Mech(definition) => {
            definition
                .sections
                .iter_mut()
                .find(|(name, section)| {
                    matches!(name, BattleSection::LeftTorso | BattleSection::RightTorso)
                        && (0..12)
                            .filter(|slot| !section.criticals.contains_key(slot))
                            .count()
                            >= 2
                })
                .unwrap()
                .1
        }
        BattleUnitTemplate::Vehicle(definition) => definition
            .sections
            .get_mut(&BattleVehicleSection::Front)
            .unwrap(),
    };
    let slots: Vec<_> = (0..12)
        .filter(|slot| !section.criticals.contains_key(slot))
        .take(2)
        .collect();
    section.criticals.insert(
        slots[0],
        CriticalDefinition {
            equipment: "IS.MachineGun".into(),
            data: "-".into(),
            modes: vec!["Gattling".into()],
            brand: None,
        },
    );
    section.criticals.insert(
        slots[1],
        CriticalDefinition {
            equipment: "Ammo_IS.MachineGun".into(),
            data: supply.to_string(),
            modes: vec![],
            brand: None,
        },
    );
    let (definition, ammunition, index) = match template {
        BattleUnitTemplate::Mech(definition) => {
            let unit = BattleUnit::from_template(definition.clone()).unwrap();
            let index = unit
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|mount| {
                    mount.weapon == BattleWeapon::MachineGun
                        && mount.initial_fire_mode == BattleFireMode::Gatling
                })
                .unwrap();
            let mut ammunition = unit.ammunition().to_vec();
            let loadout = unit.loadout().unwrap();
            let bin_index = loadout
                .ammunition
                .iter()
                .rposition(|bin| bin.weapon == BattleWeapon::MachineGun && bin.capacity >= supply)
                .unwrap();
            for (i, bin) in loadout.ammunition.iter().enumerate() {
                if bin.weapon == BattleWeapon::MachineGun {
                    ammunition[i] = if i == bin_index { supply } else { 0 };
                }
            }
            (
                serde_json::to_value(unit.definition()).unwrap(),
                serde_json::to_value(ammunition).unwrap(),
                index,
            )
        }
        BattleUnitTemplate::Vehicle(definition) => {
            let unit = BattleVehicle::new(definition.clone()).unwrap();
            let index = unit
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|mount| {
                    mount.weapon == BattleWeapon::MachineGun
                        && mount.initial_fire_mode == BattleFireMode::Gatling
                })
                .unwrap();
            let mut ammunition = unit.ammunition().to_vec();
            let loadout = unit.loadout().unwrap();
            let bin_index = loadout
                .ammunition
                .iter()
                .rposition(|bin| bin.weapon == BattleWeapon::MachineGun && bin.capacity >= supply)
                .unwrap();
            for (i, bin) in loadout.ammunition.iter().enumerate() {
                if bin.weapon == BattleWeapon::MachineGun {
                    ammunition[i] = if i == bin_index { supply } else { 0 };
                }
            }
            (
                serde_json::to_value(unit.definition()).unwrap(),
                serde_json::to_value(ammunition).unwrap(),
                index,
            )
        }
    };
    edit(world, id, |state| {
        state["definition"] = definition;
        state["ammunition"] = ammunition;
        state["fire_modes"][index.to_string()] =
            serde_json::to_value(BattleFireMode::Gatling).unwrap();
    });
    index
}

/// Gatling preparation precedes random sensor aim on every chassis, with one roll and atomic replay.
#[tokio::test]
async fn gatling_sensor_attack_order_replays_across_chassis() {
    for source in templates() {
        for supply in [2, 200] {
            let (_dir, config, mut world, shooter, target) =
                fixture(&source, include_str!("../game/mechs/JR7-D"), 1, false).await;
            let index = install_gatling(&mut world, shooter, supply);
            edit(&mut world, shooter, |state| {
                state["sensor_selection"]["active"] = serde_json::to_value(BattleSensorPair {
                    primary: BattleSensorMode::BeagleProbe,
                    secondary: BattleSensorMode::BeagleProbe,
                })
                .unwrap();
                state["dice"] = serde_json::to_value(BattleDice::seeded([17; 32])).unwrap();
            });
            edit(&mut world, target, |state| {
                state["dice"] = serde_json::to_value(BattleDice::seeded([42; 32])).unwrap();
            });
            refresh_optical_scanners(&mut world, &[shooter]).unwrap();
            let optical = battle_map_optical_contact(
                &world,
                shooter,
                target,
                BattleSensorMode::BeagleProbe,
                false,
                false,
            )
            .unwrap();
            assert!(optical.eligible);
            let mut expected = BattleDice::seeded([17; 32]);
            let damage = expected.d6().min((supply.min(18) / 3).max(1) as u8);
            let jitter = expected.die(3).unwrap() - 1;
            let roll = expected.two_d6();
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let before = scripts.world().clone();
            let preview = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(before.clone())),
            )
            .unwrap();
            let sight: (u8, u8, i16) = preview.eval_callback(&format!("local r=btech.unit.sight({},1,{index},{}); return r.roll,r.gatling_roll,r.aim.optical.modifier", shooter.0, target.0)).unwrap();
            assert_eq!(
                sight,
                (
                    roll,
                    BattleDice::seeded([17; 32]).d6(),
                    optical.aim_modifier + jitter as i16
                )
            );
            let mut sight_expected = before.clone();
            edit(&mut sight_expected, shooter, |state| {
                state["dice"] = serde_json::to_value(&expected).unwrap()
            });
            assert_eq!(preview.world().btech, sight_expected.btech);

            let command = format!("btech.unit.fire({},1,{index},{})", shooter.0, target.0);
            assert!(
                scripts
                    .eval_callback::<()>(&format!("{command}; error('abort')"))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, before.btech);
            assert!(scripts.drain_outbox().is_empty());
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            let native =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
            let result: (u8, u8, i16, bool) = scripts.eval_callback(&format!("local r={command}; local launch=r.launch or r; return launch.roll,launch.expenditure.gatling_damage,r.aim.optical.modifier,r.missed_terrain ~= nil")).unwrap();
            assert_eq!(
                (result.0, result.1, result.2),
                (roll, damage, optical.aim_modifier + jitter as i16)
            );
            if result.3 {
                // Incidental misses draw ignition and clearing even on grass;
                // the low ignition branch then makes another ignition check.
                let ignition = expected.two_d6();
                expected.two_d6();
                if ignition <= 3 {
                    expected.two_d6();
                }
            }
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire {index} #{}", target.0),
            );
            assert!(text.contains("You fire"), "{text}");
            assert_eq!(native.world().btech, scripts.world().btech);
            let key = if scripts.world().btech.vehicles().contains_key(&shooter) {
                "vehicles"
            } else {
                "constructed"
            };
            assert_eq!(
                serde_json::to_value(&scripts.world().btech).unwrap()[key][shooter.0.to_string()]["dice"],
                serde_json::to_value(expected).unwrap()
            );
            scripts.world().validate(&config).unwrap();
        }
    }
}
