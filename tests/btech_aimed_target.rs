//! Anatomical controls and directed fire preserve one policy across all supported firing chassis.
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;
#[path = "support/btech_firing.rs"]
mod firing_support;
mod support;
use firing_support::{edit, fixture_with_target, templates};

/// Independent script hosts compare native and Lua actions from the same saved world.
fn scripts(config: &Config, world: &World) -> Scripts {
    Scripts::new(config, Rc::new(RefCell::new(world.clone()))).unwrap()
}

/// Anatomy selection is saved independently of locks, power, target identity and callback lifetime.
#[tokio::test]
async fn target_controls_share_anatomy_authority_and_saved_state() {
    let targets = templates();
    for source in templates() {
        for (target_source, section, expected) in [
            (
                &targets[0],
                "h",
                BattleAimSelection::Mech(BattleSection::Head),
            ),
            (
                &targets[1],
                "fll",
                BattleAimSelection::Mech(BattleSection::LeftArm),
            ),
            (
                &targets[2],
                "as",
                BattleAimSelection::GroundVehicle(BattleVehicleSection::Rear),
            ),
            (
                &targets[6],
                "rotor",
                BattleAimSelection::Vtol(BattleVehicleSection::Rotor),
            ),
        ] {
            let (_dir, config, world, shooter, target, _) =
                fixture_with_target(&source, Some(BattleWeapon::SmallLaser), target_source).await;
            let native = scripts(&config, &world);
            let before = native.world().btech.clone();
            for command in ["target", "target h extra", "target --", "target/nope h"] {
                support::run_text(&native, &config, ObjectId(1), 1, command);
                assert_eq!(native.world().btech, before, "{command}");
            }
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("target {section}"),
            );
            assert!(text.contains("targetted."), "{text}");
            assert_eq!(
                battle_aimed_section(&native.world(), shooter).unwrap(),
                Some(expected)
            );
            let lua = scripts(&config, &world);
            lua.eval_callback::<()>(&format!("btech.unit.target({},1,'{section}')", shooter.0))
                .unwrap();
            assert_eq!(lua.world().btech, native.world().btech);
            let selected: mlua::Table = lua
                .eval_callback(&format!("return btech.unit.aimed_section({})", shooter.0))
                .unwrap();
            selected.set("section", "invalid").unwrap();
            assert_eq!(
                battle_aimed_section(&lua.world(), shooter).unwrap(),
                Some(expected)
            );
            lua.drain_outbox();
            let saved = lua.world().btech.clone();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.target({},1); error('abort')",
                    shooter.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, saved);
            assert!(lua.drain_outbox().is_empty());
            let mut world = lua.world().clone();
            select_battle_target(&mut world, shooter, ObjectId(1), None).unwrap();
            assert_eq!(
                battle_aimed_section(&world, shooter).unwrap(),
                Some(expected)
            );
            assert!(
                set_battle_aimed_section(&mut world, shooter, ObjectId(1), Some(section)).is_err()
            );
            set_battle_aimed_section(&mut world, shooter, ObjectId(1), None).unwrap();
            assert_eq!(battle_aimed_section(&world, shooter).unwrap(), None);
            select_battle_target(&mut world, shooter, ObjectId(1), Some(target)).unwrap();
            set_battle_aimed_section(&mut world, shooter, ObjectId(1), Some(section)).unwrap();
            stop_battle_unit(
                &mut world,
                shooter,
                ObjectId(1),
                BattleMovementRules::STANDARD.fall,
            )
            .unwrap();
            assert_eq!(
                battle_aimed_section(&world, shooter).unwrap(),
                Some(expected)
            );
            persistence::save(&config.database(), &world).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(restored.btech, world.btech);
        }
    }
}

/// Rotor syntax belongs only to VTOL anatomy; invalid serialized combinations cannot enter saved state.
#[tokio::test]
async fn ground_target_rejects_rotor_without_changing_selection() {
    for source in templates() {
        let (_dir, config, mut world, shooter, _, _) = fixture_with_target(
            &source,
            Some(BattleWeapon::SmallLaser),
            include_str!("../game/mechs/Demolisher"),
        )
        .await;
        set_battle_aimed_section(&mut world, shooter, ObjectId(1), Some("turret")).unwrap();
        let before = world.btech.clone();
        assert!(
            set_battle_aimed_section(&mut world, shooter, ObjectId(1), Some("rotor"))
                .unwrap_err()
                .to_string()
                .contains("Invalid location")
        );
        assert_eq!(world.btech, before);
        let scripts = scripts(&config, &world);
        assert!(
            scripts
                .eval_callback::<()>(&format!("btech.unit.target({},2,'as')", shooter.0))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
        let key = if world.btech.vehicles().contains_key(&shooter) {
            "vehicles"
        } else {
            "constructed"
        };
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state[key][shooter.0.to_string()]["aimed_section"] = serde_json::to_value(
            BattleAimSelection::GroundVehicle(BattleVehicleSection::Rotor),
        )
        .unwrap();
        if let Ok(rebuilt) = serde_json::from_value::<BtechState>(state) {
            world.btech = rebuilt;
            assert!(world.validate(&config).is_err());
        }
    }
}

/// Target dice remain unchanged by SIGHT, including a saved head preference and immobility penalty.
#[tokio::test]
async fn head_aim_penalty_and_sight_feedback_share_all_chassis() {
    for source in templates() {
        let (_dir, config, base, shooter, target, index) = fixture_with_target(
            &source,
            Some(BattleWeapon::SmallLaser),
            include_str!("../game/mechs/JR7-D"),
        )
        .await;
        for (power, penalty) in [(BattlePower::Running, 25), (BattlePower::Off, 7)] {
            let mut world = base.clone();
            edit(&mut world, target, |state| {
                state["power"] = serde_json::to_value(power).unwrap()
            });
            set_battle_aimed_section(&mut world, shooter, ObjectId(1), Some("h")).unwrap();
            let lua = scripts(&config, &world);
            let target_before = lua.world().btech.constructed_units()[&target].clone();
            let report: mlua::Table = lua
                .eval_callback(&format!("return btech.unit.sight({},1,{index})", shooter.0))
                .unwrap();
            let aim: mlua::Table = report.get("aim").unwrap();
            assert_eq!(aim.get::<i8>("aimed_section").unwrap(), penalty);
            assert_eq!(aim.get::<i8>("targeting_computer").unwrap(), 0);
            assert_eq!(
                lua.world().btech.constructed_units()[&target],
                target_before
            );
            let native = scripts(&config, &world);
            let text =
                support::run_text(&native, &config, ObjectId(1), 1, &format!("sight {index}"));
            assert!(text.contains("'s Head"), "{text}");
            assert_eq!(native.world().btech, lua.world().btech);
        }
    }
}

/// Find a target or attack stream by its next roll, without depending on incidental seed values.
fn seed_for(predicate: impl Fn(u8) -> bool) -> [u8; 32] {
    for byte in 0..=255 {
        let seed = [byte; 32];
        if predicate(BattleDice::seeded(seed).two_d6()) {
            return seed;
        }
    }
    panic!("No matching test dice stream");
}

/// Immobile directed shots share damage, rear diagnostics and callback rollback across both anatomies.
#[tokio::test]
async fn directed_hits_share_material_resolution_and_exact_dice() {
    let targets = templates();
    let attack_seed = seed_for(|roll| roll == 12);
    let target_seed = seed_for(|roll| (6..=8).contains(&roll));
    for source in templates() {
        for (target_source, section, expected_section, missing) in [
            (
                &targets[0],
                "ct",
                serde_json::to_value(BattleSection::CenterTorso).unwrap(),
                false,
            ),
            (
                &targets[1],
                "ct",
                serde_json::to_value(BattleSection::CenterTorso).unwrap(),
                false,
            ),
            (
                &targets[2],
                "as",
                serde_json::to_value(BattleVehicleSection::Rear).unwrap(),
                false,
            ),
            (
                &targets[6],
                "as",
                serde_json::to_value(BattleVehicleSection::Rear).unwrap(),
                false,
            ),
            (
                &targets[6],
                "turret",
                serde_json::to_value(BattleVehicleSection::Turret).unwrap(),
                true,
            ),
        ] {
            let (_dir, config, mut world, shooter, target, index) =
                fixture_with_target(&source, Some(BattleWeapon::SmallLaser), target_source).await;
            edit(&mut world, shooter, |state| {
                state["dice"] = serde_json::to_value(BattleDice::seeded(attack_seed)).unwrap()
            });
            edit(&mut world, target, |state| {
                state["power"] = serde_json::to_value(BattlePower::Off).unwrap();
                state["dice"] = serde_json::to_value(BattleDice::seeded(target_seed)).unwrap();
            });
            set_battle_aimed_section(&mut world, shooter, ObjectId(1), Some(section)).unwrap();
            let lua = scripts(&config, &world);
            let before = lua.world().btech.clone();
            let action = format!("btech.unit.fire({},1,{index})", shooter.0);
            assert!(
                lua.eval_callback::<()>(&format!("{action}; error('abort')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            let report: mlua::Table = lua.eval_callback(&format!("return {action}")).unwrap();
            let report = serde_json::to_value(report).unwrap();
            let salvo = &report["salvo"]["report"];
            let group = &salvo["groups"][0];
            assert!(group.is_object(), "Missing damage group: {report}");
            let vehicle = world.btech.vehicles().contains_key(&target);
            let hit = if vehicle {
                &group["impact"]["hit"]
            } else {
                &group["hit"]
            };
            assert_eq!(hit["section"], expected_section, "{report}");
            let mut dice = BattleDice::seeded(target_seed);
            dice.two_d6(); // Immobile preference, once per launch.
            if vehicle {
                assert!(group["impact"]["rolls"].as_array().unwrap().is_empty());
                let expected = [dice.two_d6(), dice.two_d6()]; // Material entry and rear diagnostic.
                assert_eq!(
                    group["impact"]["damage"]["rolls"],
                    serde_json::json!(expected)
                );
                assert_eq!(
                    group["impact"]["damage"]["absorbed"],
                    if missing { 0 } else { 3 }
                );
                if missing {
                    assert_eq!(
                        lua.world().btech.vehicles()[&target].sections(),
                        world.btech.vehicles()[&target].sections()
                    );
                    assert_eq!(group["impact"]["damage"]["overflow"], 3);
                }
            }
            let native = scripts(&config, &world);
            let text =
                support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
            assert!(text.contains("'s "), "{text}");
            assert_eq!(native.world().btech, lua.world().btech);
            let checkpoint = lua.world().clone();
            persistence::save(&config.database(), &checkpoint)
                .await
                .unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(restored.btech, checkpoint.btech);
            assert_eq!(
                roll_unit_dice(&mut restored, target, 1).unwrap(),
                [dice.d6()]
            );
        }
    }
}

/// Computer targeting changes accuracy and uses target-owned per-packet dice for exposed anatomy.
#[tokio::test]
async fn computer_directed_fire_and_accuracy_share_chassis() {
    let targets = templates();
    let attack_seed = seed_for(|roll| roll == 12);
    let target_seed = (0..=255)
        .map(|byte| [byte; 32])
        .find(|seed| BattleDice::seeded(*seed).d6() >= 3)
        .unwrap();
    for source in templates() {
        for target_source in [&targets[0], &targets[1], &targets[2], &targets[6]] {
            let (_dir, config, mut world, shooter, target, index) =
                firing_support::fixture_with_computer(
                    &source,
                    Some(BattleWeapon::SmallLaser),
                    target_source,
                    true,
                )
                .await;
            let vehicle = world.btech.vehicles().contains_key(&target);
            let section = if vehicle { "as" } else { "ct" };
            for id in [shooter, target] {
                edit(&mut world, id, |state| {
                    state["dice"] = serde_json::to_value(BattleDice::seeded(if id == shooter {
                        attack_seed
                    } else {
                        target_seed
                    }))
                    .unwrap()
                });
            }
            for (selected, immobile, penalty) in
                [(false, false, -1), (true, false, 3), (true, true, -1)]
            {
                let mut candidate = world.clone();
                set_battle_aimed_section(
                    &mut candidate,
                    shooter,
                    ObjectId(1),
                    selected.then_some(section),
                )
                .unwrap();
                if immobile {
                    edit(&mut candidate, target, |state| {
                        state["power"] = serde_json::to_value(BattlePower::Off).unwrap()
                    });
                }
                let lua = scripts(&config, &candidate);
                let report: mlua::Table = lua
                    .eval_callback(&format!("return btech.unit.sight({},1,{index})", shooter.0))
                    .unwrap();
                let aim: mlua::Table = report.get("aim").unwrap();
                assert_eq!(aim.get::<i8>("targeting_computer").unwrap(), penalty);
                assert_eq!(aim.get::<i8>("aimed_section").unwrap(), 0);
            }
            set_battle_aimed_section(&mut world, shooter, ObjectId(1), Some(section)).unwrap();
            let lua = scripts(&config, &world);
            let report: mlua::Table = lua
                .eval_callback(&format!("return btech.unit.fire({},1,{index})", shooter.0))
                .unwrap();
            let report = serde_json::to_value(report).unwrap();
            let group = &report["salvo"]["report"]["groups"][0];
            let mut dice = BattleDice::seeded(target_seed);
            assert!(dice.d6() >= 3);
            if vehicle {
                assert_eq!(
                    group["impact"]["hit"]["section"],
                    serde_json::to_value(BattleVehicleSection::Rear).unwrap(),
                    "{report}"
                );
                assert!(group["impact"]["rolls"].as_array().unwrap().is_empty());
                assert_eq!(
                    group["impact"]["damage"]["rolls"],
                    serde_json::json!([dice.two_d6(), dice.two_d6()])
                );
            } else {
                assert_eq!(
                    group["hit"]["section"],
                    serde_json::to_value(BattleSection::CenterTorso).unwrap(),
                    "{report}"
                );
            }
            assert_eq!(
                roll_unit_dice(&mut lua.world_mut(), target, 1).unwrap(),
                [dice.d6()]
            );
        }
    }
}

/// Hidden vehicle faces and rotor selections consume preparation then reuse ordinary hit routing.
#[tokio::test]
async fn unavailable_directed_locations_fall_back_to_normal_hits() {
    let targets = templates();
    let target_seed = seed_for(|roll| (6..=8).contains(&roll));
    for source in templates() {
        for (target_source, section) in [(&targets[2], "fs"), (&targets[6], "rotor")] {
            let (_dir, config, mut world, shooter, target, index) =
                fixture_with_target(&source, Some(BattleWeapon::SmallLaser), target_source).await;
            edit(&mut world, shooter, |state| {
                state["dice"] =
                    serde_json::to_value(BattleDice::seeded(seed_for(|roll| roll == 12))).unwrap()
            });
            edit(&mut world, target, |state| {
                state["power"] = serde_json::to_value(BattlePower::Off).unwrap();
                state["dice"] = serde_json::to_value(BattleDice::seeded(target_seed)).unwrap();
            });
            let mut ordinary = world.clone();
            let mut dice = BattleDice::seeded(target_seed);
            dice.two_d6();
            edit(&mut ordinary, target, |state| {
                state["dice"] = serde_json::to_value(dice).unwrap()
            });
            set_battle_aimed_section(&mut world, shooter, ObjectId(1), Some(section)).unwrap();
            let directed = scripts(&config, &world);
            let ordinary = scripts(&config, &ordinary);
            let action = format!("return btech.unit.fire({},1,{index})", shooter.0);
            let a: mlua::Table = directed.eval_callback(&action).unwrap();
            let b: mlua::Table = ordinary.eval_callback(&action).unwrap();
            assert_eq!(
                serde_json::to_value(a).unwrap()["salvo"],
                serde_json::to_value(b).unwrap()["salvo"]
            );
            assert_eq!(
                directed.world().btech.vehicles()[&target],
                ordinary.world().btech.vehicles()[&target]
            );
        }
    }
}

/// Unused immobile aim preparation precedes burst/clustering/thermal paths, including missile misses.
#[tokio::test]
async fn special_weapons_preserve_aim_preparation_and_normal_resolution() {
    let targets = templates();
    let target_seed = seed_for(|roll| (6..=8).contains(&roll));
    let hit_seed = seed_for(|roll| roll == 12);
    let miss_seed = seed_for(|roll| roll == 2);
    let (_dir, config, base) = support::isolated_world().await;
    let directed = scripts(&config, &base);
    let ordinary = scripts(&config, &base);
    for source in templates() {
        for target_source in [&targets[0], &targets[2], &targets[6]] {
            for (weapon, mode, ammunition, flag, miss) in [
                (
                    BattleWeapon::UltraAc2,
                    BattleFireMode::Ultra,
                    BattleAmmunitionMode::Normal,
                    "",
                    false,
                ),
                (
                    BattleWeapon::Lbx2,
                    BattleFireMode::Normal,
                    BattleAmmunitionMode::Cluster,
                    "LBX/Cluster",
                    false,
                ),
                (
                    BattleWeapon::Srm2,
                    BattleFireMode::Normal,
                    BattleAmmunitionMode::Normal,
                    "",
                    false,
                ),
                (
                    BattleWeapon::Srm2,
                    BattleFireMode::Normal,
                    BattleAmmunitionMode::Normal,
                    "",
                    true,
                ),
                (
                    BattleWeapon::Flamer,
                    BattleFireMode::Heat,
                    BattleAmmunitionMode::Normal,
                    "",
                    false,
                ),
                (
                    BattleWeapon::CoolantGun,
                    BattleFireMode::Normal,
                    BattleAmmunitionMode::Normal,
                    "",
                    false,
                ),
            ] {
                let (mut world, shooter, target, index) = firing_support::supply_fixture_on(
                    base.clone(),
                    &config,
                    &source,
                    Some(weapon),
                    target_source,
                    false,
                    Some(flag),
                );
                edit(&mut world, shooter, |state| {
                    state["dice"] = serde_json::to_value(BattleDice::seeded(if miss {
                        miss_seed
                    } else {
                        hit_seed
                    }))
                    .unwrap();
                    if mode != BattleFireMode::Normal {
                        state["fire_modes"][index.to_string()] =
                            serde_json::to_value(mode).unwrap();
                    }
                    if ammunition != BattleAmmunitionMode::Normal {
                        state["ammunition_modes"][index.to_string()] =
                            serde_json::to_value(ammunition).unwrap();
                    }
                });
                edit(&mut world, target, |state| {
                    state["power"] = serde_json::to_value(BattlePower::Off).unwrap();
                    state["dice"] = serde_json::to_value(BattleDice::seeded(target_seed)).unwrap();
                });
                if miss {
                    let map = if let Some(unit) = world.btech.vehicles().get(&target) {
                        unit.position().unwrap().map
                    } else {
                        world.btech.constructed_units()[&target]
                            .position()
                            .unwrap()
                            .map
                    };
                    place_battle_unit(&mut world, target, map, 0, 5).unwrap();
                    refresh_optical_scanners(&mut world, &[shooter]).unwrap();
                    select_battle_target(&mut world, shooter, ObjectId(1), Some(target)).unwrap();
                }
                edit(&mut world, shooter, |state| {
                    state["dice"] = serde_json::to_value(BattleDice::seeded(if miss {
                        miss_seed
                    } else {
                        hit_seed
                    }))
                    .unwrap();
                });
                let mut baseline = world.clone();
                let mut dice = BattleDice::seeded(target_seed);
                dice.two_d6();
                edit(&mut baseline, target, |state| {
                    state["dice"] = serde_json::to_value(dice).unwrap()
                });
                let vehicle = world.btech.vehicles().contains_key(&target);
                set_battle_aimed_section(
                    &mut world,
                    shooter,
                    ObjectId(1),
                    Some(if vehicle { "as" } else { "ct" }),
                )
                .unwrap();
                support::install(&directed, world.clone());
                support::install(&ordinary, baseline);
                let action = format!("return btech.unit.fire({},1,{index})", shooter.0);
                let a: mlua::Table = directed.eval_callback(&action).unwrap();
                let b: mlua::Table = ordinary.eval_callback(&action).unwrap();
                let a = serde_json::to_value(a).unwrap();
                let b = serde_json::to_value(b).unwrap();
                assert_eq!(a, b, "{weapon:?} {mode:?} miss={miss}");
                if miss {
                    assert!(a["salvo"].is_null(), "{a}");
                } else if mode != BattleFireMode::Heat && weapon != BattleWeapon::CoolantGun {
                    assert!(a["salvo"].is_object(), "{a}");
                }
                if vehicle {
                    assert_eq!(
                        directed.world().btech.vehicles()[&target],
                        ordinary.world().btech.vehicles()[&target]
                    );
                } else {
                    assert_eq!(
                        directed.world().btech.constructed_units()[&target],
                        ordinary.world().btech.constructed_units()[&target]
                    );
                }
            }
        }
    }
}

/// Explicit fire at a different unit class retains the saved preference and separates computer admission.
#[tokio::test]
async fn changed_target_class_preserves_selection_and_numeric_immobile_hits() {
    for source in templates() {
        for computer in [false, true] {
            let (_dir, config, mut base, shooter, selected, index) =
                firing_support::fixture_with_computer(
                    &source,
                    Some(BattleWeapon::SmallLaser),
                    include_str!("../game/mechs/Demolisher"),
                    computer,
                )
                .await;
            let map = base.btech.vehicles()[&selected].position().unwrap().map;
            let actual = base.create(&config, "Alternate Mech".into(), Kind::Thing);
            base.objects.get_mut(&actual).unwrap().home = Some(ObjectId(config.home()));
            BattleUnitTemplate::parse(include_str!("../game/mechs/JR7-D"))
                .unwrap()
                .create(&mut base, actual)
                .unwrap();
            place_battle_unit(&mut base, actual, map, 0, 10).unwrap();
            edit(&mut base, shooter, |state| {
                state["dice"] =
                    serde_json::to_value(BattleDice::seeded(seed_for(|roll| roll == 12))).unwrap()
            });
            let preference = BattleAimSelection::GroundVehicle(BattleVehicleSection::Rear);
            for immobile in [false, true] {
                let mut world = base.clone();
                edit(&mut world, actual, |state| {
                    state["power"] = serde_json::to_value(if immobile {
                        BattlePower::Off
                    } else {
                        BattlePower::Running
                    })
                    .unwrap();
                    state["dice"] = serde_json::to_value(BattleDice::seeded(seed_for(|roll| {
                        (6..=8).contains(&roll)
                    })))
                    .unwrap();
                });
                refresh_optical_scanners(&mut world, &[shooter]).unwrap();
                edit(&mut world, shooter, |state| {
                    state["dice"] =
                        serde_json::to_value(BattleDice::seeded(seed_for(|roll| roll == 12)))
                            .unwrap();
                });
                let ordinary = scripts(&config, &world);
                set_battle_aimed_section(&mut world, shooter, ObjectId(1), Some("as")).unwrap();
                let directed = scripts(&config, &world);
                let action = format!(
                    "return btech.unit.fire({},1,{index},{})",
                    shooter.0, actual.0
                );
                let result: mlua::Table = directed.eval_callback(&action).unwrap();
                let result = serde_json::to_value(result).unwrap();
                assert!(result["salvo"].is_object(), "{result}");
                if immobile {
                    assert_eq!(
                        result["salvo"]["report"]["groups"][0]["hit"]["section"],
                        serde_json::to_value(BattleSection::RightTorso).unwrap()
                    );
                } else {
                    let baseline: mlua::Table = ordinary.eval_callback(&action).unwrap();
                    assert_eq!(
                        result["salvo"],
                        serde_json::to_value(baseline).unwrap()["salvo"]
                    );
                    assert_eq!(
                        directed.world().btech.constructed_units()[&actual],
                        ordinary.world().btech.constructed_units()[&actual]
                    );
                }
                assert_eq!(
                    battle_aimed_section(&directed.world(), shooter).unwrap(),
                    Some(preference)
                );
                let sight = scripts(&config, &world);
                let text = support::run_text(
                    &sight,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("sight {index} #{}", actual.0),
                );
                assert!(!text.contains("'s "), "{text}");
            }
        }
    }
}

#[path = "support/btech_defense.rs"]
mod defense_support;

/// Aimed missiles preserve initial target preparation, attacker-owned interception and atomic publication.
#[tokio::test]
async fn aimed_missiles_share_active_defense_dice_rollback_and_restart() {
    let target_seed = seed_for(|roll| (6..=8).contains(&roll));
    let hit_seed = seed_for(|roll| roll == 12);
    let miss_seed = seed_for(|roll| roll == 2);
    let (_dir, config, base_world) = support::isolated_world().await;
    let directed = scripts(&config, &base_world);
    let ordinary = scripts(&config, &base_world);
    let native = scripts(&config, &base_world);
    let pristine_db = support::snapshot_database(&config);
    for source in templates() {
        for recipient in defense_support::templates() {
            let (base, shooter, target, index) = firing_support::supply_fixture_on(
                base_world.clone(),
                &config,
                &source,
                Some(BattleWeapon::ClanLrm20),
                &recipient,
                false,
                Some(""),
            );
            for miss in [false, true] {
                let mut world = base.clone();
                edit(&mut world, target, |state| {
                    state["fortified"] = true.into();
                    state["ams_enabled"] = true.into();
                    state["dice"] = serde_json::to_value(BattleDice::seeded(target_seed)).unwrap();
                });
                edit(&mut world, shooter, |state| {
                    state["dice"] = serde_json::to_value(BattleDice::seeded(if miss {
                        miss_seed
                    } else {
                        hit_seed
                    }))
                    .unwrap();
                });
                let mut baseline = world.clone();
                roll_unit_dice(&mut baseline, target, 2).unwrap();
                let vehicle = world.btech.vehicles().contains_key(&target);
                set_battle_aimed_section(
                    &mut world,
                    shooter,
                    ObjectId(1),
                    Some(if vehicle { "as" } else { "h" }),
                )
                .unwrap();
                support::install(&directed, world.clone());
                let before = directed.world().btech.clone();
                let action = format!("btech.unit.fire({},1,{index})", shooter.0);
                assert!(
                    directed
                        .eval_callback::<()>(&format!("{action}; error('abort')"))
                        .is_err()
                );
                assert_eq!(directed.world().btech, before);
                assert!(directed.drain_outbox().is_empty());
                let result: mlua::Table =
                    directed.eval_callback(&format!("return {action}")).unwrap();
                let result = serde_json::to_value(result).unwrap();
                support::install(&ordinary, baseline);
                let expected: mlua::Table =
                    ordinary.eval_callback(&format!("return {action}")).unwrap();
                assert_eq!(result, serde_json::to_value(expected).unwrap());
                let ams = &result["ams"];
                if miss {
                    assert!(result["salvo"].is_null(), "{result}");
                    assert!(ams.is_null(), "A missile miss must not activate AMS");
                } else {
                    assert!(ams.is_object(), "Defense did not activate: {result}");
                    assert!(ams["ammunition_spent"].as_u64().unwrap() > 0);
                    assert!(result["salvo"].is_object());
                    assert!(ams["shot_down"].as_u64().unwrap() > 0);
                }
                let mut without_aim = serde_json::to_value(&directed.world().btech).unwrap();
                let key = if world.btech.vehicles().contains_key(&shooter) {
                    "vehicles"
                } else {
                    "constructed"
                };
                without_aim[key][shooter.0.to_string()]["aimed_section"] = serde_json::Value::Null;
                assert_eq!(
                    without_aim,
                    serde_json::to_value(&ordinary.world().btech).unwrap()
                );
                support::install(&native, world.clone());
                support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
                assert_eq!(native.world().btech, directed.world().btech);
                let saved = directed.world().clone();
                support::restore_database(&config, &pristine_db);
                persistence::save(&config.database(), &saved).await.unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    saved.btech
                );
            }
        }
    }
}

/// The near-miss option changes launch feedback, but missiles reach defenses and damage only at base BTH.
#[tokio::test]
async fn missile_base_boundary_controls_ams_swarm_and_cluster_glancing() {
    let defenders = defense_support::templates();
    let (_dir, mut config, base_world) = support::isolated_world().await;
    let path = config.root.join("stompymux.toml");
    let mut settings: toml::Value =
        toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    settings["battletech"]
        .as_table_mut()
        .unwrap()
        .insert("glancing_blows".into(), 2.into());
    std::fs::write(&path, toml::to_string(&settings).unwrap()).unwrap();
    config = Config::load(&config.root).unwrap();
    let probe = scripts(&config, &base_world);
    let lua = scripts(&config, &base_world);
    let native = scripts(&config, &base_world);
    for source in templates() {
        for recipient in [&defenders[0], &defenders[2], &defenders[6]] {
            for (ammunition, flag) in [
                (BattleAmmunitionMode::Normal, ""),
                (BattleAmmunitionMode::Swarm, "Swarm"),
                (BattleAmmunitionMode::Swarm1, "Swarm1"),
            ] {
                let (mut base, shooter, target, index) = firing_support::supply_fixture_on(
                    base_world.clone(),
                    &config,
                    &source,
                    Some(BattleWeapon::ClanLrm20),
                    recipient,
                    false,
                    Some(flag),
                );
                edit(&mut base, target, |state| {
                    state["fortified"] = true.into();
                    state["ams_enabled"] = true.into();
                    state["dice"] = serde_json::to_value(BattleDice::seeded([42; 32])).unwrap();
                });
                if ammunition != BattleAmmunitionMode::Normal {
                    edit(&mut base, shooter, |state| {
                        state["ammunition_modes"][index.to_string()] =
                            serde_json::to_value(ammunition).unwrap()
                    });
                }
                let section = if base.btech.vehicles().contains_key(&target) {
                    "as"
                } else {
                    "h"
                };
                set_battle_aimed_section(&mut base, shooter, ObjectId(1), Some(section)).unwrap();
                support::install(&probe, base.clone());
                let sight: mlua::Table = probe
                    .eval_callback(&format!("return btech.unit.sight({},1,{index})", shooter.0))
                    .unwrap();
                let threshold: u8 = sight.get("target_number").unwrap();
                assert!((3..=11).contains(&threshold));
                for roll in [threshold - 1, threshold, threshold + 1] {
                    let mut world = base.clone();
                    edit(&mut world, shooter, |state| {
                        state["dice"] =
                            serde_json::to_value(BattleDice::seeded(seed_for(|value| {
                                value == roll
                            })))
                            .unwrap()
                    });
                    support::install(&lua, world.clone());
                    let value: mlua::Table = lua
                        .eval_callback(&format!("return btech.unit.fire({},1,{index})", shooter.0))
                        .unwrap();
                    let value = serde_json::to_value(value).unwrap();
                    let glancing = if world.btech.vehicles().contains_key(&shooter) {
                        value["launch"]["glancing"].clone()
                    } else {
                        value["glancing"].clone()
                    };
                    assert_eq!(glancing, serde_json::json!(roll == threshold));
                    assert_eq!(value["salvo"].is_object(), roll >= threshold, "{value}");
                    assert_eq!(
                        value["ams"].is_object(),
                        roll >= threshold && ammunition == BattleAmmunitionMode::Normal,
                        "{value}"
                    );
                    if roll < threshold {
                        let mut expected = world.clone();
                        roll_unit_dice(&mut expected, target, 2).unwrap();
                        if let Some(unit) = expected.btech.vehicles().get(&target) {
                            assert_eq!(&lua.world().btech.vehicles()[&target], unit);
                        } else {
                            assert_eq!(
                                lua.world().btech.constructed_units()[&target],
                                expected.btech.constructed_units()[&target]
                            );
                        }
                    }
                    support::install(&native, world.clone());
                    support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
                    assert_eq!(native.world().btech, lua.world().btech);
                }
            }
        }
    }
}
