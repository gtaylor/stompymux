//! Shared artillery cluster and missile Smoke/Mine controls retain their distinct reference behavior.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
/// Representative construction for every supported movement class.
fn templates() -> Vec<String> {
    let tracked = include_str!("../game/mechs/Demolisher.toml");
    vec![
        include_str!("../game/mechs/JR7-D.toml").into(),
        include_str!("../game/mechs/GOL-1H.toml").into(),
        tracked.into(),
        tracked.replace("movement = \"track\"", "movement = \"wheel\""),
        tracked.replace("movement = \"track\"", "movement = \"hover\""),
        tracked
            .replace("movement = \"track\"", "movement = \"none\"")
            .replace("walk_mp = 5", "walk_mp = 0"),
        include_str!("../game/mechs/Kestrel.toml").into(),
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
    artillery: bool,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId) {
    let (dir, config, mut world) = support::isolated_world().await;
    let map = world.create(&config, "Visibility field".into(), Kind::Room);
    create_battle_map(
        &mut world,
        map,
        "visibility",
        BattleMapAsset::parse("1 5\n.0\n.0\n.0\n.0\n.0\n").unwrap(),
    )
    .unwrap();
    let mut ids = Vec::new();
    for (index, source) in [observer, target].into_iter().enumerate() {
        let id = world.create(&config, format!("Unit {index}"), Kind::Thing);
        world.objects.get_mut(&id).unwrap().home = Some(ObjectId(config.home()));
        (if index == 0 {
            launcher(source, artillery)
        } else {
            BattleUnitTemplate::parse("test", source).unwrap()
        })
        .create(&mut world, id)
        .unwrap();
        place_battle_unit(&mut world, id, map, 0, if index == 0 { 3 } else { 0 }).unwrap();
        edit(&mut world, id, |state| {
            state["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            state["dice"] = serde_json::to_value(BattleDice::seeded([19; 32])).unwrap();
        });
        ids.push(id);
    }
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(ids[0]);
    assign_battle_pilot(&mut world, ids[0], ObjectId(1)).unwrap();
    world.validate(&config).unwrap();
    (dir, config, world, ids[0], ids[1])
}

/// Install one weapon and three distinct bins without copying control or feed behavior.
fn launcher(source: &str, artillery: bool) -> BattleUnitTemplate {
    let weapon = if artillery {
        BattleWeapon::ClanArrowIv
    } else {
        BattleWeapon::ClanLrm20
    };
    let mut definition = BattleUnitTemplate::parse("test", source).unwrap();
    let part = CriticalDefinition {
        equipment: weapon.name().into(),
        data: "-".into(),
        modes: Vec::new(),
        brand: None,
    };
    let bins = |section: &mut SectionDefinition| {
        for (slot, flag) in [None, Some("Smoke"), Some("Mine"), Some("Cluster")]
            .into_iter()
            .enumerate()
        {
            if flag == Some("Cluster") && !artillery {
                continue;
            }
            section.criticals.insert(
                slot as u8,
                CriticalDefinition {
                    equipment: format!("Ammo_{}", weapon.name()),
                    data: weapon.profile().ammunition_per_ton.to_string(),
                    modes: flag.into_iter().map(str::to_owned).collect(),
                    brand: None,
                },
            );
        }
    };
    match &mut definition {
        BattleUnitTemplate::Mech(unit) => {
            for section in unit.sections.values_mut() {
                section.criticals.retain(|_, p| {
                    !p.equipment.starts_with("Ammo_")
                        && !BattleWeapon::ALL.iter().any(|w| w.name() == p.equipment)
                });
            }
            let mount = unit.sections.get_mut(&BattleSection::LeftTorso).unwrap();
            mount.criticals.clear();
            for slot in 0..weapon.profile().critical_slots {
                mount.criticals.insert(slot, part.clone());
            }
            bins(unit.sections.get_mut(&BattleSection::RightTorso).unwrap());
        }
        BattleUnitTemplate::Vehicle(unit) => {
            for section in unit.sections.values_mut() {
                section.criticals.retain(|_, p| {
                    !p.equipment.starts_with("Ammo_")
                        && !BattleWeapon::ALL.iter().any(|w| w.name() == p.equipment)
                });
            }
            unit.sections
                .get_mut(&BattleVehicleSection::Front)
                .unwrap()
                .criticals
                .insert(0, part);
            bins(unit.sections.get_mut(&BattleVehicleSection::Rear).unwrap());
        }
    }
    definition
}

/// The three commands share native/Lua state, rejection and effect rollback on every supported chassis.
#[tokio::test]
async fn special_round_controls_share_chassis_transactions_and_restart() {
    for source in templates() {
        for artillery in [false, true] {
            let (_dir, config, world, shooter, _) =
                fixture(&source, &templates()[0], artillery).await;
            let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
            let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let commands: &[&str] = if artillery {
                &["firecluster", "cluster"]
            } else {
                &["firesmoke", "firesmoke", "firemine", "firemine"]
            };
            for command in commands {
                let before = lua.world().clone();
                let call = format!("btech.unit.{command}({},1,0)", shooter.0);
                assert!(
                    lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                        .is_err()
                );
                assert_eq!(before.btech, lua.world().btech);
                assert!(lua.drain_outbox().is_empty());
                let selected: String = lua.eval_callback(&format!("return {call}")).unwrap();
                let output =
                    support::run_text(&native, &config, ObjectId(1), 1, &format!("{command} 0"));
                assert!(output.contains("rounds"), "{selected}: {output}");
                assert_eq!(native.world().btech, lua.world().btech);
                let saved = lua.world().clone();
                persistence::save(&config.database(), &saved).await.unwrap();
                let loaded = persistence::load(&config.database()).await.unwrap();
                assert_eq!(loaded.btech, lua.world().btech);
                lua.drain_outbox();
            }
            let before = lua.world().clone();
            for command in if artillery {
                &["firesmoke", "firemine"][..]
            } else {
                &["firecluster"][..]
            } {
                let call = format!("btech.unit.{command}({},1,0)", shooter.0);
                assert!(lua.eval_callback::<()>(&call).is_err());
                assert_eq!(before.btech, lua.world().btech);
                assert!(lua.drain_outbox().is_empty());
            }
        }
    }
}

/// Cluster controls select matching supplies and retain the existing queued artillery/restart behavior.
#[tokio::test]
async fn artillery_cluster_controls_feed_real_launches() {
    for source in templates() {
        let (_dir, config, world, shooter, _) = fixture(&source, &templates()[0], true).await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let call = format!("btech.unit.firecluster({},1,0)", shooter.0);
        assert_eq!(
            scripts
                .eval_callback::<String>(&format!("return {call}"))
                .unwrap(),
            "cluster"
        );
        let launch = format!("btech.unit.fire({},1,0,{{x=0,y=1}})", shooter.0);
        let before = scripts.world().clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!("{launch}; error('abort')"))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before.btech);
        let mode: String = scripts
            .eval_callback(&format!("return {launch}.expenditure.ammunition_mode"))
            .unwrap();
        assert_eq!(mode, "cluster");
        let fired = scripts.world().clone();
        persistence::save(&config.database(), &fired).await.unwrap();
        let replay = Scripts::new(
            &config,
            Rc::new(RefCell::new(
                persistence::load(&config.database()).await.unwrap(),
            )),
        )
        .unwrap();
        for _ in 0..10 {
            assert_eq!(
                advance_artillery_action(&scripts, &config, BattleMovementRules::STANDARD.fall)
                    .unwrap(),
                advance_artillery_action(&replay, &config, BattleMovementRules::STANDARD.fall)
                    .unwrap()
            );
        }
        assert_eq!(scripts.world().btech, replay.world().btech);
    }
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
        vehicle_impact: stompymux_rs::BattleVehicleImpactRules::STANDARD,
        stacking: BattleStackingRules::STANDARD,
        stagger: BattleStaggerMode::Retain,
        glancing: BattleGlancingMode::Disabled,
        aim: rules(),
        hit: BattleHitRules {
            inferno_penalty: false,
            exile_stun_mode: 0,
        },
        hit_arc_mode: 0,
        extended_gunnery: false,
        extended_piloting: false,
        target_toughness: false,
    }
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
        refresh_battle_contacts(world, &[shooter]).unwrap();
        if visible_battle_contact(world, shooter, target)
            .unwrap()
            .is_some()
        {
            return;
        }
    }
    panic!("Fixture contact was not acquired");
}

/// Missile Smoke/Mine supplies retain ordinary impact damage; only Mine exempts the shot from AMS.
#[tokio::test]
async fn missile_special_rounds_use_shared_launch_damage_and_ams_policy() {
    for source in templates() {
        for target_source in [
            include_str!("../game/mechs/Daishi-A.toml"),
            include_str!("../game/mechs/Goblin-58.toml"),
        ] {
            let (_dir, config, mut initial, shooter, target) =
                fixture(&source, target_source, false).await;
            edit(&mut initial, target, |unit| {
                unit["ams_enabled"] = true.into()
            });
            acquire(&mut initial, shooter, target);
            for mode in [BattleAmmunitionMode::Smoke, BattleAmmunitionMode::Mine] {
                let mut world = initial.clone();
                toggle_battle_missile_rounds(&mut world, shooter, ObjectId(1), 0, mode).unwrap();
                dice(&mut world, shooter, 12);
                dice(&mut world, target, 2);
                let before = world.clone();
                let (defense, salvo) = if world.btech.vehicles().contains_key(&shooter) {
                    let report = fire_battle_vehicle_shot(
                        &mut world,
                        shooter,
                        ObjectId(1),
                        target,
                        0,
                        BattleVehicleShotRules {
                            shot: shot_rules(),
                            shooter_criticals: BattleVehicleImpactRules::STANDARD.criticals,
                        },
                    )
                    .unwrap();
                    assert_eq!(report.launch.expenditure.ammunition_mode, mode);
                    (report.ams, report.salvo)
                } else {
                    let report = resolve_battle_shot(
                        &mut world,
                        shooter,
                        ObjectId(1),
                        target,
                        0,
                        shot_rules(),
                    )
                    .unwrap();
                    assert_eq!(report.expenditure.ammunition_mode, mode);
                    (report.ams, report.salvo)
                };
                assert_eq!(defense.is_some(), mode == BattleAmmunitionMode::Smoke);
                assert!(salvo.is_some());
                if mode == BattleAmmunitionMode::Mine {
                    let hits = match salvo.unwrap() {
                        BattleTargetSalvo::Mech(report) => report.missiles_before_defense,
                        BattleTargetSalvo::Vehicle(report) => report.missiles_before_defense,
                        BattleTargetSalvo::Swarm(_) => panic!("Mine ammunition does not retarget"),
                    };
                    assert_eq!(hits, Some(6));
                }
                world.validate(&config).unwrap();
                persistence::save(&config.database(), &before)
                    .await
                    .unwrap();
                let replay = persistence::load(&config.database()).await.unwrap();
                assert_eq!(replay.btech, before.btech);
                assert_eq!(
                    world.btech.maps(),
                    before.btech.maps(),
                    "Missile supplies do not deploy artillery terrain payloads"
                );
            }
        }
    }
}

/// Existing special artillery payloads block cluster selection without changing supply or saved state.
#[tokio::test]
async fn artillery_cluster_conflicts_and_control_guards_are_atomic() {
    for source in templates() {
        let (_dir, config, world, shooter, _) = fixture(&source, &templates()[0], true).await;
        for mode in [BattleAmmunitionMode::Smoke, BattleAmmunitionMode::Mine] {
            let mut special = world.clone();
            edit(&mut special, shooter, |unit| {
                unit["ammunition_modes"]["0"] = serde_json::to_value(mode).unwrap()
            });
            let before = special.clone();
            let error = toggle_battle_cluster(&mut special, shooter, ObjectId(1), 0).unwrap_err();
            assert!(error.to_string().contains("already been set"), "{error}");
            assert_eq!(special.btech, before.btech);
            special.validate(&config).unwrap();
        }
        for case in ["pilot", "index", "recycle", "shutdown"] {
            let mut invalid = world.clone();
            if case == "recycle" {
                edit(&mut invalid, shooter, |unit| {
                    unit["weapon_recycle"]["0"] = 5.into()
                });
            }
            if case == "shutdown" {
                edit(&mut invalid, shooter, |unit| {
                    unit["power"] = serde_json::to_value(BattlePower::Off).unwrap()
                });
            }
            let before = invalid.clone();
            assert!(
                toggle_battle_cluster(
                    &mut invalid,
                    shooter,
                    if case == "pilot" {
                        ObjectId(0)
                    } else {
                        ObjectId(1)
                    },
                    if case == "index" { 99 } else { 0 }
                )
                .is_err()
            );
            assert_eq!(before.btech, invalid.btech);
        }
    }
}

/// Mechanical guards have the same precedence on every chassis and all shared launcher controls.
#[tokio::test]
async fn launcher_mode_feed_jams_and_recycle_are_shared_and_atomic() {
    for source in templates() {
        let (_dir, config, base, id, _) = fixture(&source, &templates()[0], false).await;
        for recycling in [false, true] {
            let mut world = base.clone();
            edit(&mut world, id, |state| {
                state["jammed_weapons"] = serde_json::json!([0]);
                if recycling {
                    state["weapon_recycle"] = serde_json::json!({"0":1});
                }
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let before = scripts.world().btech.clone();
            let expected = if recycling {
                "recycling"
            } else {
                "feed mechanism"
            };
            for command in [
                "hotload",
                "artemis",
                "fireswarm",
                "fireswarm1",
                "firesmoke",
                "firemine",
            ] {
                let output =
                    support::run_text(&scripts, &config, ObjectId(1), 1, &format!("{command} 0"));
                assert!(output.contains(expected), "{command}: {output}");
                scripts.drain_outbox();
                let error = scripts
                    .eval_callback::<mlua::Value>(&format!("btech.unit.{command}({},1,0)", id.0))
                    .unwrap_err();
                assert!(error.to_string().contains(expected), "{command}: {error}");
                assert_eq!(scripts.world().btech, before);
                assert!(scripts.drain_outbox().is_empty());
            }
        }
    }
}
