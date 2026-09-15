//! Swarm supplies and flights reuse controls, launch transactions and damage across chassis.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
mod support;

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
            "1 5\n.0\n.9\n.0\n.0\n.0\n"
        } else {
            "1 5\n.0\n.0\n.0\n.0\n.0\n"
        })
        .unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for (index, source) in [observer, target].into_iter().enumerate() {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        (if index == 0 {
            launcher(source, "Swarm")
        } else {
            BattleUnitTemplate::parse(source).unwrap()
        })
        .create(&mut world, id)
        .unwrap();
        place_battle_unit(&mut world, id, map, 0, if index == 0 { 3 } else { 0 }).unwrap();
        edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            state["dice"] = serde_json::to_value(BattleDice::seeded([19; 32])).unwrap();
            state["sensor_signal"] =
                serde_json::to_value(BattleSensorSignal::seeded(100, [27; 32]).unwrap()).unwrap();
        });
        ids.push(id);
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ids[0]);
    assign_battle_pilot(&mut world, ids[0], ObjectId(1)).unwrap();
    world.validate(&config).unwrap();
    (dir, config, world, ids[0], ids[1])
}

/// Deterministic conventional firing policy for shared damage checks.
fn rules() -> BattleAimRules {
    BattleAimRules {
        woods_damage: false,
        dig_bonus: 3,
        dig_only_front: false,
        hit_arc_mode: 0,
        fasa_turning: false,
        extended_movement: false,
        extended_ranges: false,
        hotload_half_minimum: false,
        override_weapon_arcs: false,
    }
}

/// Tactical conventional shot configuration shared by admission cases.
fn shot_rules() -> BattleShotRules {
    BattleShotRules {
        range_damage: false,
        tsm_tow_bonus: true,
        tsm_sprint_bonus: true,
        vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
        stacking: BattleStackingRules::STANDARD,
        stagger: BattleStaggerMode::Retain,
        glancing: BattleGlancingMode::Disabled,
        aim: rules(),
        hit: BattleHitRules {
            fasa_criticals: false,
            inferno_penalty: false,
            exile_stun_mode: 0,
        },
        hit_arc_mode: 0,
        extended_gunnery: false,
        extended_piloting: false,
        target_toughness: false,
    }
}

/// Install the same Clan LRM-20 and full Swarm bin in either construction anatomy.
fn launcher(source: &str, mode: &str) -> BattleUnitTemplate {
    let mut definition = BattleUnitTemplate::parse(source).unwrap();
    let part = BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D"))
        .unwrap()
        .sections[&BattleSection::LeftArm]
        .criticals[&2]
        .clone();
    let install = |section: &mut SectionDefinition, mech: bool| {
        let mut mount = part.clone();
        mount.equipment = BattleWeapon::ClanLrm20.name().into();
        mount.modes = vec![mode.into()];
        mount.data = "-".into();
        for slot in 0..if mech { 4 } else { 1 } {
            section.criticals.insert(slot, mount.clone());
        }
        let mut bin = mount;
        bin.equipment = format!("Ammo_{}", BattleWeapon::ClanLrm20.name());
        bin.data = "6".into();
        section.criticals.insert(5, bin.clone());
        bin.modes = vec![if mode == "Swarm" { "Swarm1" } else { "Swarm" }.into()];
        section.criticals.insert(6, bin);
    };
    match &mut definition {
        BattleUnitTemplate::Mech(unit) => {
            for section in unit.sections.values_mut() {
                section.criticals.retain(|_, part| {
                    !part.equipment.starts_with("Ammo_")
                        && !BattleWeapon::ALL.iter().any(|w| w.name() == part.equipment)
                });
            }
            install(
                unit.sections.get_mut(&BattleSection::RightTorso).unwrap(),
                true,
            );
        }
        BattleUnitTemplate::Vehicle(unit) => {
            for section in unit.sections.values_mut() {
                section.criticals.retain(|_, part| {
                    !part.equipment.starts_with("Ammo_")
                        && !BattleWeapon::ALL.iter().any(|w| w.name() == part.equipment)
                });
            }
            install(
                unit.sections.get_mut(&BattleVehicleSection::Front).unwrap(),
                false,
            );
        }
    }
    definition
}

/// Seed an attack independently from cluster and target damage streams.
fn dice(world: &mut World, id: ObjectId, total: u8) {
    let seed = (0..=255)
        .find(|v| BattleDice::seeded([*v; 32]).two_d6() == total)
        .unwrap();
    edit(world, id, |unit| {
        unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
    });
}

/// Preserve known contacts without spending launch dice.
fn acquire(world: &mut World, shooter: ObjectId, target: ObjectId) {
    for seed in 0..=255 {
        edit(world, shooter, |unit| {
            unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
        });
        refresh_optical_scanners(world, &[shooter]).unwrap();
        if visible_battle_contact(world, shooter, target)
            .unwrap()
            .is_some()
        {
            return;
        }
    }
    panic!("Fixture contact was not acquired");
}

/// Normalize only the report envelope; firing still uses each public launcher API.
fn fire(world: &mut World, shooter: ObjectId, target: ObjectId) -> Option<BattleSwarmReport> {
    fire_with_rules(world, shooter, target, shot_rules())
}

/// Keep launch normalization identical while testing configured target-resolution boundaries.
fn fire_with_rules(
    world: &mut World,
    shooter: ObjectId,
    target: ObjectId,
    rules: BattleShotRules,
) -> Option<BattleSwarmReport> {
    let (salvo, ams) = if world.btech.vehicles().contains_key(&shooter) {
        let r = fire_battle_vehicle_shot(
            world,
            shooter,
            ObjectId(1),
            target,
            0,
            BattleVehicleShotRules {
                shot: rules,
                shooter_criticals: BattleVehicleImpactRules::STANDARD.criticals,
            },
        )
        .unwrap();
        (r.salvo, r.ams)
    } else {
        let r = resolve_battle_shot(world, shooter, ObjectId(1), target, 0, rules).unwrap();
        (r.salvo, r.ams)
    };
    assert!(ams.is_none());
    salvo.map(|salvo| {
        let BattleTargetSalvo::Swarm(report) = salvo else {
            panic!("Swarm report missing")
        };
        report
    })
}

/// Missiles stopped by cover are spent at that target, not returned to the next swarm hop.
#[tokio::test]
async fn swarm_woods_absorption_preserves_pre_cover_flight_accounting() {
    for source in templates() {
        for recipient in [templates()[0].clone(), templates()[2].clone()] {
            let (_dir, config, mut world, shooter, target) =
                fixture(&source, &recipient, false).await;
            let next = candidate(&mut world, &config, target, &templates()[2], 0);
            let map = world.btech.units()[&target].map.unwrap();
            let mut encoded = serde_json::to_value(&world.btech).unwrap();
            encoded["maps"][map.0.to_string()]["terrain"][0] = serde_json::to_value(BattleHex {
                terrain: Terrain::HeavyForest,
                elevation: 0,
            })
            .unwrap();
            world.btech = serde_json::from_value(encoded).unwrap();
            acquire(&mut world, shooter, target);
            set_battle_visibility(
                &mut world,
                target,
                BattleVisibility {
                    clairvoyant: true,
                    invisible: false,
                },
            )
            .unwrap();
            dice(&mut world, shooter, 12);
            dice(&mut world, target, 2);
            let mut rules = shot_rules();
            rules.aim.woods_damage = true;
            let report = fire_with_rules(&mut world, shooter, target, rules).unwrap();
            assert_eq!(report.hops.len(), 2);
            assert_eq!(report.hops[0].remaining, 14);
            assert_eq!(report.hops[1].incoming, 14);
            assert_eq!(report.hops[1].target, next);
            let salvo = serde_json::to_value(report.hops[0].salvo.as_ref().unwrap()).unwrap();
            assert_eq!(salvo["report"]["missiles_before_defense"], 6);
            assert_eq!(salvo["report"]["woods"]["damage_before"], 6);
            assert_eq!(salvo["report"]["woods"]["damage_after"], 2);
        }
    }
}

/// Construct real launchers on all seven chassis and replay the same capped missile flight.
#[tokio::test]
async fn swarm_launchers_and_targets_share_damage_and_restart() {
    for source in templates() {
        for target_source in templates() {
            let (_dir, config, mut world, shooter, target) =
                fixture(&source, &target_source, false).await;
            acquire(&mut world, shooter, target);
            dice(&mut world, shooter, 12);
            dice(&mut world, target, 2);
            let before = world.clone();
            let report = fire(&mut world, shooter, target).unwrap();
            assert_eq!(report.launched, 20);
            assert_eq!(report.hops.len(), 1);
            assert_eq!(report.hops[0].remaining, 14);
            assert_eq!(report.remaining, 14);
            assert_eq!(
                report.hops[0].salvo.as_ref().unwrap().as_mech().is_some(),
                !before.btech.vehicles().contains_key(&target)
            );
            world.validate(&config).unwrap();
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let mut replay = persistence::load(&config.database()).await.unwrap();
            assert_eq!(fire(&mut replay, shooter, target).unwrap(), report);
            assert_eq!(world.btech, replay.btech);
            let mut miss = before;
            dice(&mut miss, shooter, 2);
            assert!(fire(&mut miss, shooter, target).is_none());
        }
    }
}

/// Add a retarget candidate without changing the original launcher or its pilot.
fn candidate(
    world: &mut World,
    config: &Config,
    target: ObjectId,
    source: &str,
    team: i16,
) -> ObjectId {
    let map = if let Some(unit) = world.btech.vehicles().get(&target) {
        unit.position().unwrap().map
    } else {
        world.btech.constructed_units()[&target]
            .position()
            .unwrap()
            .map
    };
    let id = world.create(config, "Retarget candidate".into(), Kind::Thing);
    world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
    BattleUnitTemplate::parse(source)
        .unwrap()
        .create(world, id)
        .unwrap();
    place_battle_unit(world, id, map, 0, 0).unwrap();
    edit(world, id, |unit| {
        unit["sensor_signature"]["team"] = team.into();
    });
    dice(world, id, 12);
    id
}

/// Map order, acquired visibility, immunity and friend-or-foe filtering precede shared capped damage.
#[tokio::test]
async fn swarm_retarget_selection_caps_hits_and_skips_ineligible_units() {
    for source in templates() {
        for friend_or_foe in [false, true] {
            let (_dir, config, mut initial, shooter, target) =
                fixture(&source, &templates()[0], false).await;
            let friendly = candidate(&mut initial, &config, target, &templates()[2], 0);
            let safe = candidate(&mut initial, &config, target, &templates()[1], 1);
            set_battle_combat_safe(&mut initial, safe, true).unwrap();
            let hostile = candidate(&mut initial, &config, target, &templates()[6], 1);
            acquire(&mut initial, shooter, target);
            if friend_or_foe {
                assert_eq!(
                    toggle_battle_swarm(&mut initial, shooter, ObjectId(1), 0, true).unwrap(),
                    BattleAmmunitionMode::Swarm1
                );
            }
            dice(&mut initial, shooter, 12);
            dice(&mut initial, target, 2);
            let mut unseen = initial.clone();
            assert_eq!(fire(&mut unseen, shooter, target).unwrap().hops.len(), 1);
            set_battle_visibility(
                &mut initial,
                target,
                BattleVisibility {
                    clairvoyant: true,
                    invisible: false,
                },
            )
            .unwrap();
            // Secondary attack must hit so the target's maximum cluster exercises the incoming cap.
            let seed = (0u32..65536)
                .find_map(|n| {
                    let mut seed = [0; 32];
                    seed[..4].copy_from_slice(&n.to_le_bytes());
                    let mut dice = BattleDice::seeded(seed);
                    (dice.two_d6() == 12 && dice.two_d6() == 12).then_some(seed)
                })
                .unwrap();
            edit(&mut initial, shooter, |unit| {
                unit["dice"] = serde_json::to_value(BattleDice::seeded(seed)).unwrap()
            });
            let report = fire(&mut initial, shooter, target).unwrap();
            assert_eq!(report.hops.len(), 2);
            assert_eq!(
                report.hops[1].target,
                if friend_or_foe { hostile } else { friendly }
            );
            assert_eq!(report.hops[1].incoming, 14);
            assert_eq!(report.remaining, 0);
            let BattleTargetSalvo::Vehicle(salvo) = report.hops[1].salvo.as_ref().unwrap() else {
                panic!("Expected vehicle target")
            };
            assert_eq!(salvo.missiles_before_defense, Some(14));
            initial.validate(&config).unwrap();
        }
    }
}

/// Secondary misses keep their incoming missiles and proceed to the next unvisited map slot.
#[tokio::test]
async fn swarm_secondary_misses_preserve_missiles() {
    let (_dir, config, mut world, shooter, target) =
        fixture(&templates()[0], &templates()[2], false).await;
    let second = candidate(&mut world, &config, target, &templates()[1], 1);
    let third = candidate(&mut world, &config, target, &templates()[2], 1);
    for id in [target, second] {
        set_battle_visibility(
            &mut world,
            id,
            BattleVisibility {
                clairvoyant: true,
                invisible: false,
            },
        )
        .unwrap();
    }
    acquire(&mut world, shooter, target);
    let seed = (0u32..65536)
        .find_map(|n| {
            let mut seed = [0; 32];
            seed[..4].copy_from_slice(&n.to_le_bytes());
            let mut dice = BattleDice::seeded(seed);
            (dice.two_d6() == 12 && dice.two_d6() == 2).then_some(seed)
        })
        .unwrap();
    edit(&mut world, shooter, |unit| {
        unit["dice"] = serde_json::to_value(BattleDice::seeded(seed)).unwrap()
    });
    dice(&mut world, target, 2);
    let report = fire(&mut world, shooter, target).unwrap();
    assert_eq!(
        report.hops.iter().map(|hop| hop.target).collect::<Vec<_>>(),
        vec![target, second, third]
    );
    assert_eq!(report.hops[1].roll, 2);
    assert_eq!(report.hops[1].remaining, 14);
    assert!(report.hops[1].salvo.is_none());
    assert_eq!(report.hops[2].incoming, 14);
}

/// Mode selection and firing publish the same shared consequences, and late callbacks undo the whole flight.
#[tokio::test]
async fn swarm_native_lua_controls_firing_and_rollback() {
    for source in templates() {
        let (_dir, config, mut world, shooter, target) =
            fixture(&source, &templates()[0], false).await;
        candidate(&mut world, &config, target, &templates()[2], 1);
        set_battle_visibility(
            &mut world,
            target,
            BattleVisibility {
                clairvoyant: true,
                invisible: false,
            },
        )
        .unwrap();
        acquire(&mut world, shooter, target);
        let seed = (0u32..65536)
            .find_map(|n| {
                let mut seed = [0; 32];
                seed[..4].copy_from_slice(&n.to_le_bytes());
                let mut dice = BattleDice::seeded(seed);
                (dice.two_d6() == 12 && dice.two_d6() == 12).then_some(seed)
            })
            .unwrap();
        edit(&mut world, shooter, |unit| {
            unit["dice"] = serde_json::to_value(BattleDice::seeded(seed)).unwrap()
        });
        dice(&mut world, target, 2);
        let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
        let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        for command in ["fireswarm", "fireswarm1", "fireswarm1", "fireswarm"] {
            let before = lua.world().clone();
            let call = format!("btech.unit.{command}({},1,0)", shooter.0);
            assert!(
                lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, before.btech);
            assert!(lua.drain_outbox().is_empty());
            lua.eval_callback::<String>(&format!("return {call}"))
                .unwrap();
            let output =
                support::run_text(&native, &config, ObjectId(1), 1, &format!("{command} 0"));
            assert!(output.contains("missiles"), "{output}");
            assert_eq!(native.world().btech, lua.world().btech);
            lua.drain_outbox();
        }
        let before = lua.world().clone();
        let call = format!("btech.unit.fire({},1,0,{})", shooter.0, target.0);
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, before.btech);
        assert!(lua.drain_outbox().is_empty());
        let (kind, hops): (String, usize) = lua
            .eval_callback(&format!(
                "local r={call}; return r.salvo.kind,#r.salvo.report.hops"
            ))
            .unwrap();
        assert_eq!(kind, "swarm");
        assert_eq!(hops, 2);
        let output = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire 0 #{}", target.0),
        );
        assert!(output.contains("Hit."), "{output}");
        assert_eq!(native.world().btech, lua.world().btech);
    }
}

/// A connected pilot with a chosen base gunnery target makes the terminal flight deterministic.
fn gunnery(world: &mut World, target: u8) {
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    set_battle_character(
        world,
        ObjectId(1),
        BattleCharacter {
            bruise: 0,
            lethal: 0,
            build: 5,
            reflexes: 4,
            intuition: 14 - target,
            learn: 4,
            charisma: 4,
        },
    )
    .unwrap();
}

/// A maximum first roll followed by ten misses preserves missiles through the visited-slot boundary.
fn long_flight_seed() -> [u8; 32] {
    (0u32..65536)
        .find_map(|n| {
            let mut seed = [0; 32];
            seed[..4].copy_from_slice(&n.to_le_bytes());
            let mut dice = BattleDice::seeded(seed);
            (dice.two_d6() == 12 && (0..10).all(|_| dice.two_d6() < 12)).then_some(seed)
        })
        .unwrap()
}

/// Ten previously visited slots allow eleven total attacks, with no repeat or extra attack roll.
#[tokio::test]
async fn swarm_visited_boundary_allows_exactly_eleven_attacks() {
    for source in [templates()[0].clone(), templates()[2].clone()] {
        let (_dir, config, mut world, shooter, target) =
            fixture(&source, &templates()[0], false).await;
        let mut targets = vec![target];
        for index in 0..12 {
            targets.push(candidate(
                &mut world,
                &config,
                target,
                &templates()[index % 7],
                1,
            ));
        }
        for id in &targets {
            set_battle_visibility(
                &mut world,
                *id,
                BattleVisibility {
                    clairvoyant: true,
                    invisible: false,
                },
            )
            .unwrap();
        }
        acquire(&mut world, shooter, target);
        gunnery(&mut world, 10);
        let seed = long_flight_seed();
        edit(&mut world, shooter, |unit| {
            unit["dice"] = serde_json::to_value(BattleDice::seeded(seed)).unwrap()
        });
        dice(&mut world, target, 2);
        let report = fire(&mut world, shooter, target).unwrap();
        assert_eq!(
            report.hops.iter().map(|hop| hop.target).collect::<Vec<_>>(),
            targets[..11]
        );
        assert_eq!(report.remaining, 14);
        assert!(report.hops[1..].iter().all(|hop| hop.salvo.is_none()));
        let mut expected = BattleDice::seeded(seed);
        for _ in 0..11 {
            expected.two_d6();
        }
        let actual = if let Some(unit) = world.btech.vehicles().get(&shooter) {
            serde_json::to_value(unit).unwrap()
        } else {
            serde_json::to_value(&world.btech.constructed_units()[&shooter]).unwrap()
        };
        assert_eq!(actual["dice"], serde_json::to_value(expected).unwrap());
    }
}

/// Range is cumulative; the exact long-range endpoint is attacked and the following leg falls short.
#[tokio::test]
async fn swarm_cumulative_range_stops_before_spending_another_attack_roll() {
    let (_dir, config, mut world, shooter, target) =
        fixture(&templates()[0], &templates()[0], false).await;
    let map = world.create(&config, "Long swarm field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "long-flight",
        BattleMapAsset::parse(&format!("1 32\n{}", ".0\n".repeat(32))).unwrap(),
    )
    .unwrap();
    for (id, y) in [(shooter, 20), (target, 0)] {
        edit(&mut world, id, |unit| {
            unit["power"] = serde_json::to_value(BattlePower::Off).unwrap()
        });
        place_battle_unit(&mut world, id, map, 0, y).unwrap();
        edit(&mut world, id, |unit| {
            unit["power"] = serde_json::to_value(BattlePower::Running).unwrap()
        });
    }
    let second = candidate(&mut world, &config, target, &templates()[2], 1);
    let third = candidate(&mut world, &config, target, &templates()[1], 1);
    place_battle_unit(&mut world, second, map, 0, 1).unwrap();
    place_battle_unit(&mut world, third, map, 0, 2).unwrap();
    for id in [target, second] {
        set_battle_visibility(
            &mut world,
            id,
            BattleVisibility {
                clairvoyant: true,
                invisible: false,
            },
        )
        .unwrap();
    }
    acquire(&mut world, shooter, target);
    gunnery(&mut world, 6);
    edit(&mut world, shooter, |unit| {
        unit["dice"] = serde_json::to_value(BattleDice::seeded(long_flight_seed())).unwrap()
    });
    dice(&mut world, target, 2);
    let report = fire(&mut world, shooter, target).unwrap();
    assert_eq!(
        report.hops.iter().map(|hop| hop.target).collect::<Vec<_>>(),
        vec![target, second]
    );
    assert_eq!(report.traveled, 22.0);
    assert_eq!(report.remaining, 14);
    assert!(
        report
            .notices
            .iter()
            .any(|notice| notice.unit == third && notice.text.contains("fall short"))
    );
}

/// Ordinary Swarm can revisit the launcher, while Swarm-1 excludes it as a friendly candidate.
#[tokio::test]
async fn swarm_can_return_to_its_launcher() {
    for source in [templates()[0].clone(), templates()[2].clone()] {
        for friend_or_foe in [false, true] {
            let (_dir, _config, mut world, shooter, target) =
                fixture(&source, &templates()[0], false).await;
            let map = world.btech.constructed_units()[&target]
                .position()
                .unwrap()
                .map;
            edit(&mut world, target, |unit| {
                unit["power"] = serde_json::to_value(BattlePower::Off).unwrap()
            });
            place_battle_unit(&mut world, target, map, 0, 2).unwrap();
            set_battle_visibility(
                &mut world,
                target,
                BattleVisibility {
                    clairvoyant: true,
                    invisible: false,
                },
            )
            .unwrap();
            acquire(&mut world, shooter, target);
            if friend_or_foe {
                toggle_battle_swarm(&mut world, shooter, ObjectId(1), 0, true).unwrap();
            }
            dice(&mut world, shooter, 12);
            dice(&mut world, target, 2);
            let report = fire(&mut world, shooter, target).unwrap();
            assert_eq!(report.hops.len(), if friend_or_foe { 1 } else { 2 });
            if !friend_or_foe {
                assert_eq!(report.hops[1].target, shooter);
            }
        }
    }
}

/// Swarm's interception exemption preserves a real enabled defense's ammunition, heat and recycle.
#[tokio::test]
async fn swarm_skips_installed_ams_on_hits_and_misses() {
    for source in [templates()[0].clone(), templates()[2].clone()] {
        for target_source in [
            include_str!("../game/mechs/Daishi-A"),
            include_str!("../game/mechs/Goblin-58"),
        ] {
            let (_dir, _config, mut initial, shooter, target) =
                fixture(&source, target_source, false).await;
            edit(&mut initial, target, |unit| {
                unit["ams_enabled"] = true.into()
            });
            acquire(&mut initial, shooter, target);
            for roll in [2, 12] {
                let mut world = initial.clone();
                dice(&mut world, shooter, roll);
                dice(&mut world, target, 2);
                let before = if let Some(unit) = world.btech.vehicles().get(&target) {
                    serde_json::to_value(unit).unwrap()
                } else {
                    serde_json::to_value(&world.btech.constructed_units()[&target]).unwrap()
                };
                fire(&mut world, shooter, target);
                let after = if let Some(unit) = world.btech.vehicles().get(&target) {
                    serde_json::to_value(unit).unwrap()
                } else {
                    serde_json::to_value(&world.btech.constructed_units()[&target]).unwrap()
                };
                let heat = if world.btech.vehicles().contains_key(&target) {
                    "weapon_heat"
                } else {
                    "heat"
                };
                for field in ["ammunition", heat, "weapon_recycle"] {
                    assert!(!before[field].is_null(), "Missing defense field {field}");
                    assert_eq!(before[field], after[field], "{field}");
                }
            }
        }
    }
}

/// Swarm ammunition follows the same indirect-launcher eligibility, capacity and saved mode grammar.
#[test]
fn swarm_template_modes_cover_compatible_catalogue() {
    for &weapon in BattleWeapon::ALL {
        for (flag, mode) in [
            ("Swarm", BattleAmmunitionMode::Swarm),
            ("Swarm1", BattleAmmunitionMode::Swarm1),
        ] {
            let compatible = weapon.supports_semiguided() && !weapon.is_dead_fire();
            assert_eq!(
                weapon
                    .damage_groups_for_ammunition(mode, Some(7), 10.0)
                    .is_ok(),
                compatible,
                "{weapon:?} {mode:?}"
            );
            if !compatible {
                continue;
            }
            let mut template =
                BattleTemplate::parse(include_str!("fixtures/btech/mechs/JR7-D")).unwrap();
            let arm = template.sections.get_mut(&BattleSection::LeftArm).unwrap();
            let mut mount = arm.criticals[&2].clone();
            mount.equipment = weapon.name().into();
            mount.modes = vec![flag.into()];
            for slot in 2..2 + weapon.profile().critical_slots {
                arm.criticals.insert(slot, mount.clone());
            }
            let bin = template
                .sections
                .get_mut(&BattleSection::RightTorso)
                .unwrap()
                .criticals
                .get_mut(&0)
                .unwrap();
            bin.equipment = format!("Ammo_{}", weapon.name());
            bin.data = weapon.profile().ammunition_per_ton.to_string();
            bin.modes = vec![flag.into()];
            let unit = BattleUnit::from_template(template).unwrap();
            let loadout = unit.loadout().unwrap();
            let index = loadout
                .weapons
                .iter()
                .position(|mount| mount.weapon == weapon)
                .unwrap();
            assert_eq!(unit.ammunition_mode(index).unwrap(), mode);
            assert_eq!(loadout.ammunition[0].mode, mode);
            assert_eq!(
                loadout.ammunition[0].capacity,
                u16::from(weapon.profile().ammunition_per_ton)
            );
            let restored: BattleUnit =
                serde_json::from_value(serde_json::to_value(&unit).unwrap()).unwrap();
            assert_eq!(restored, unit);
        }
    }
}

#[path = "support/btech_defense.rs"]
mod defense_support;

/// Aimed Swarm flights prepare only their first target and bypass active defenses on every hop.
#[tokio::test]
async fn aimed_swarm_prepares_once_across_defended_immobile_targets() {
    let seed_for = |predicate: &dyn Fn(&mut BattleDice) -> bool| {
        (0u32..65536)
            .find_map(|n| {
                let mut seed = [0; 32];
                seed[..4].copy_from_slice(&n.to_le_bytes());
                predicate(&mut BattleDice::seeded(seed)).then_some(seed)
            })
            .unwrap()
    };
    let attack = seed_for(&|dice| dice.two_d6() == 12 && dice.two_d6() >= 8 && dice.two_d6() >= 8);
    let near_attack =
        seed_for(&|dice| dice.two_d6() == 12 && dice.two_d6() == 2 && dice.two_d6() >= 8);
    let preparation = seed_for(&|dice| (6..=8).contains(&dice.two_d6()) && dice.two_d6() == 2);
    let defenders = defense_support::templates();
    for source in templates() {
        for (index, recipient) in defenders.iter().enumerate() {
            let (_dir, config, mut base, shooter, target) =
                fixture(&source, recipient, false).await;
            let second = candidate(
                &mut base,
                &config,
                target,
                &defenders[(index + 1) % defenders.len()],
                1,
            );
            let third = candidate(
                &mut base,
                &config,
                target,
                &defenders[(index + 2) % defenders.len()],
                1,
            );
            for id in [target, second, third] {
                edit(&mut base, id, |state| {
                    state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
                    state["fortified"] = true.into();
                    state["ams_enabled"] = true.into();
                    state["motion"]["heading"] = 180.0.into();
                    state["motion"]["desired_heading"] = 180.0.into();
                });
                set_battle_visibility(
                    &mut base,
                    id,
                    BattleVisibility {
                        clairvoyant: true,
                        invisible: false,
                    },
                )
                .unwrap();
            }
            acquire(&mut base, shooter, target);
            select_battle_target(&mut base, shooter, ObjectId(1), Some(target)).unwrap();
            for (smart, near_miss) in [(false, false), (true, false), (false, true), (true, true)] {
                let mut world = base.clone();
                if smart {
                    toggle_battle_swarm(&mut world, shooter, ObjectId(1), 0, true).unwrap();
                }
                edit(&mut world, shooter, |state| {
                    state["dice"] = serde_json::to_value(BattleDice::seeded(if near_miss {
                        near_attack
                    } else {
                        attack
                    }))
                    .unwrap()
                });
                edit(&mut world, target, |state| {
                    state["dice"] = serde_json::to_value(BattleDice::seeded(preparation)).unwrap()
                });
                dice(&mut world, second, 2);
                dice(&mut world, third, 12);
                let mut ordinary = world.clone();
                roll_unit_dice(&mut ordinary, target, 2).unwrap();
                let section = if world.btech.vehicles().contains_key(&target) {
                    "as"
                } else {
                    "h"
                };
                set_battle_aimed_section(&mut world, shooter, ObjectId(1), Some(section)).unwrap();
                let mut rules = shot_rules();
                if near_miss {
                    rules.glancing = BattleGlancingMode::BelowTarget;
                }
                let report = fire_with_rules(&mut world, shooter, target, rules).unwrap();
                let baseline = fire_with_rules(&mut ordinary, shooter, target, rules).unwrap();
                if near_miss {
                    assert!(report.hops[1].salvo.is_none());
                    assert_eq!(report.hops[1].roll, 2);
                    assert_eq!(report.hops[1].remaining, 14);
                    assert_eq!(report.hops[2].incoming, 14);
                }
                assert_eq!(report, baseline);
                assert_eq!(
                    report.hops.iter().map(|hop| hop.target).collect::<Vec<_>>(),
                    [target, second, third]
                );
                assert_eq!(report.hops[1].incoming, 14);
                assert_eq!(report.remaining, 0);
                let mut actual = serde_json::to_value(&world.btech).unwrap();
                let key = if world.btech.vehicles().contains_key(&shooter) {
                    "vehicles"
                } else {
                    "constructed"
                };
                actual[key][shooter.0.to_string()]["aimed_section"] = serde_json::Value::Null;
                assert_eq!(actual, serde_json::to_value(&ordinary.btech).unwrap());
                for id in [target, second, third] {
                    let recycle = if let Some(unit) = world.btech.vehicles().get(&id) {
                        unit.weapon_recycle()
                    } else {
                        world.btech.constructed_units()[&id].weapon_recycle()
                    };
                    assert!(recycle.is_empty(), "Swarm must not activate AMS on any hop");
                }
                persistence::save(&config.database(), &world).await.unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    world.btech
                );
            }
        }
    }
}

/// Later Swarm targets retain private checks through hop and launcher feedback prefixes.
#[tokio::test]
async fn swarm_secondary_balance_feedback_is_private_and_atomic() {
    for source in [templates()[0].clone(), templates()[2].clone()] {
        let (_dir, config, mut base, shooter, target) =
            fixture(&source, &templates()[0], false).await;
        let secondary = candidate(&mut base, &config, target, &templates()[0], 1);
        base.objects.get_mut(&ObjectId(2)).unwrap().location = Some(secondary);
        assign_battle_pilot(&mut base, secondary, ObjectId(2)).unwrap();
        edit(&mut base, secondary, |unit| {
            unit["power"] = serde_json::to_value(BattlePower::Running).unwrap()
        });
        for section in BattleSection::ALL {
            let armor = base.btech.constructed_units()[&secondary].sections()[&section].armor;
            apply_damage_phase(
                &mut base,
                secondary,
                section,
                armor,
                BattleDamagePhase::Armor { rear: false },
            )
            .unwrap();
        }
        set_battle_visibility(
            &mut base,
            target,
            BattleVisibility {
                clairvoyant: true,
                invisible: false,
            },
        )
        .unwrap();
        acquire(&mut base, shooter, target);
        let launch_seed = (0u32..65536)
            .find_map(|n| {
                let mut seed = [0; 32];
                seed[..4].copy_from_slice(&n.to_le_bytes());
                let mut dice = BattleDice::seeded(seed);
                (dice.two_d6() == 12 && dice.two_d6() == 12).then_some(seed)
            })
            .unwrap();
        edit(&mut base, shooter, |unit| {
            unit["dice"] = serde_json::to_value(BattleDice::seeded(launch_seed)).unwrap()
        });
        dice(&mut base, target, 2);
        let mut exercised = false;
        for seed in 0..=255 {
            let mut before = base.clone();
            edit(&mut before, secondary, |unit| {
                unit["dice"] = serde_json::to_value(BattleDice::seeded([seed; 32])).unwrap()
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(before.clone()))).unwrap();
            let call = format!("btech.unit.fire({},1,0,{})", shooter.0, target.0);
            let hops: usize = scripts
                .eval_callback(&format!("local r={call}; return #r.salvo.report.hops"))
                .unwrap();
            let output = scripts.drain_outbox();
            let private: Vec<_> = output
                .iter()
                .filter(|(_, text)| text.source() == "You make a piloting skill roll!")
                .collect();
            if private.is_empty() {
                continue;
            }
            assert!(hops >= 2);
            assert!(private.iter().all(|(who, _)| *who == ObjectId(2)));
            let pilot: Vec<_> = output
                .iter()
                .filter(|(who, _)| *who == ObjectId(2))
                .map(|(_, text)| text.source())
                .collect();
            let index = pilot
                .iter()
                .position(|text| *text == "You make a piloting skill roll!")
                .unwrap();
            assert!(index > 0);
            assert!(pilot[index + 1].starts_with("Modified Pilot Skill: BTH "));
            let after = scripts.world().btech.clone();
            *scripts.world_mut() = before.clone();
            assert!(
                scripts
                    .eval_callback::<()>(&format!("{call}; error('abort swarm balance')"))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, before.btech);
            assert!(scripts.drain_outbox().is_empty());
            commands::run(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("fire 0 #{}", target.0),
            )
            .unwrap();
            assert_eq!(scripts.world().btech, after);
            let native: Vec<_> = scripts
                .drain_outbox()
                .into_iter()
                .map(|(who, text)| (who, text.source().to_owned()))
                .collect();
            let lua: Vec<_> = output
                .into_iter()
                .map(|(who, text)| (who, text.source().to_owned()))
                .collect();
            assert_eq!(native, lua);
            exercised = true;
            break;
        }
        assert!(exercised, "No secondary target rolled for balance");
    }
}
