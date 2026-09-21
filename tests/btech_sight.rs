//! Sighting preserves combat state while sharing target selection, aim and saved dice across chassis.
use crate::support;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

#[path = "support/btech_firing.rs"]
mod firing_support;
use firing_support::{edit, fixture_with_target, templates};

/// Build an equipped shooter facing a visible target, with a selected unit lock.
async fn fixture(
    source: &str,
    weapon: Option<BattleWeapon>,
) -> (tempfile::TempDir, Config, World, ObjectId, ObjectId, usize) {
    fixture_with_target(source, weapon, include_str!("../game/mechs/JR7-D")).await
}

/// Independent script hosts compare native and Lua actions from exactly the same world.
fn scripts(config: &Config, world: &World) -> Scripts {
    Scripts::new(config, Rc::new(RefCell::new(world.clone()))).unwrap()
}

/// Every target form preserves cover, hold, supply and recycle; restart resumes only the consumed dice.
#[tokio::test]
async fn sight_all_chassis_targets_preserve_state_and_replay() {
    for source in templates() {
        let (_dir, config, base, shooter, target, index) = fixture(&source, None).await;
        let lua = scripts(&config, &base);
        let native = scripts(&config, &base);
        let pristine_db = support::snapshot_database(&config);
        for condition in ["normal", "recycle", "empty", "hidden", "hiding", "hold"] {
            let mut world = base.clone();
            edit(&mut world, shooter, |state| match condition {
                "recycle" => state["weapon_recycle"][index.to_string()] = 1.into(),
                "empty" => {
                    for count in state["ammunition"].as_array_mut().unwrap() {
                        *count = 0.into();
                    }
                }
                "hidden" => state["sensor_signature"]["hidden"] = true.into(),
                "hiding" => state["hide_elapsed"] = 0.into(),
                "hold" => state["weapons_hold"] = true.into(),
                _ => (),
            });
            for (lua_target, native_target, recipient) in [
                ("nil".into(), "".into(), Some(target.0)),
                (
                    target.0.to_string(),
                    format!("#{}", target.0),
                    Some(target.0),
                ),
                ("{x=0,y=10}".into(), "0 10".into(), Some(target.0)),
                ("{x=0,y=9}".into(), "0 9".into(), None),
                ("{x=0,y=0}".into(), "0 0".into(), None),
            ] {
                support::install(&lua, world.clone());
                let before = lua.world().clone();
                let command = format!("btech.unit.sight({},1,{index},{lua_target})", shooter.0);
                assert!(
                    lua.eval_callback::<()>(&format!("{command}; error('abort')"))
                        .is_err()
                );
                assert_eq!(lua.world().btech, before.btech);
                assert!(lua.drain_outbox().is_empty());
                support::restore_database(&config, &pristine_db);
                persistence::save(&config.database(), &before)
                    .await
                    .unwrap();
                let restored = persistence::load(&config.database()).await.unwrap();
                support::install(&native, restored);
                let actual: (u8, Option<i64>) = lua
                    .eval_callback(&format!("local r={command}; return r.roll,r.target"))
                    .unwrap();
                let mut dice = BattleDice::seeded([42; 32]);
                assert_eq!(
                    actual,
                    (dice.two_d6(), recipient),
                    "{condition}: {native_target}"
                );
                let text = support::run_text(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("sight {index} {native_target}"),
                );
                assert!(text.contains("You aim"), "{condition}: {text}");
                assert_eq!(native.world().btech, lua.world().btech);
                let mut expected = before;
                edit(&mut expected, shooter, |state| {
                    state["dice"] = serde_json::to_value(dice).unwrap()
                });
                assert_eq!(lua.world().btech, expected.btech);
            }
        }
    }
}

/// Gatling and dead-fire sighting consume their preparation dice despite empty ammunition bins.
#[tokio::test]
async fn sight_special_rolls_do_not_launch_or_spend() {
    for source in [
        include_str!("../game/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
    ] {
        for weapon in [
            BattleWeapon::MachineGun,
            BattleWeapon::LrDfm5,
            BattleWeapon::Elrm5,
            BattleWeapon::ClanArrowIv,
            BattleWeapon::CoolantGun,
        ] {
            let (_dir, config, mut world, shooter, _, index) = fixture(source, Some(weapon)).await;
            edit(&mut world, shooter, |state| {
                for count in state["ammunition"].as_array_mut().unwrap() {
                    *count = 0.into();
                }
                if weapon == BattleWeapon::CoolantGun {
                    state["fire_modes"][index.to_string()] =
                        serde_json::to_value(BattleFireMode::Heat).unwrap();
                }
                if weapon == BattleWeapon::MachineGun {
                    state["fire_modes"][index.to_string()] =
                        serde_json::to_value(BattleFireMode::Gatling).unwrap();
                }
            });
            let scripts = scripts(&config, &world);
            let before = scripts.world().clone();
            let target = if weapon.is_artillery() {
                "{x=0,y=10}"
            } else {
                "nil"
            };
            let command = format!("btech.unit.sight({},1,{index},{target})", shooter.0);
            let actual: (u8, Option<u8>) = scripts
                .eval_callback(&format!("local r={command}; return r.roll,r.gatling_roll"))
                .unwrap();
            let mut dice = BattleDice::seeded([42; 32]);
            let gatling = (weapon == BattleWeapon::MachineGun).then(|| dice.d6());
            let roll = if matches!(weapon, BattleWeapon::LrDfm5 | BattleWeapon::Elrm5) {
                let mut rolls = [dice.d6(), dice.d6(), dice.d6()];
                rolls.sort();
                rolls[0] + rolls[1]
            } else {
                dice.two_d6()
            };
            assert_eq!(actual, (roll, gatling));
            let mut expected = before;
            edit(&mut expected, shooter, |state| {
                state["dice"] = serde_json::to_value(dice).unwrap()
            });
            assert_eq!(scripts.world().btech, expected.btech);
        }
    }
}

/// Sighting retains cockpit, target and physical-loss gates without committing partial dice or feedback.
#[tokio::test]
async fn sight_rejections_are_atomic_across_chassis() {
    for source in templates() {
        let (_dir, config, base, shooter, target, index) = fixture(&source, None).await;
        for condition in [
            "destroyed",
            "stunned",
            "unpiloted",
            "stopped",
            "index",
            "target",
            "friendly",
            "spotting",
        ] {
            let mut world = base.clone();
            let vehicle = world.btech.vehicles().contains_key(&shooter);
            if condition == "destroyed" {
                if vehicle {
                    let location = world.btech.vehicles()[&shooter].loadout().unwrap().weapons
                        [index]
                        .criticals[0];
                    destroy_battle_vehicle_critical(&mut world, shooter, location).unwrap();
                } else {
                    let location = world.btech.constructed_units()[&shooter]
                        .loadout()
                        .unwrap()
                        .weapons[index]
                        .criticals[0];
                    destroy_battle_critical(&mut world, shooter, location).unwrap();
                }
            } else {
                edit(&mut world, shooter, |state| match condition {
                    "stunned" => {
                        state[if vehicle {
                            "crew_stun_remaining"
                        } else {
                            "stun_remaining"
                        }] = 1.into()
                    }
                    "unpiloted" => state["pilot"] = serde_json::Value::Null,
                    "stopped" => {
                        state["target_lock"] = serde_json::Value::Null;
                        state["power"] = serde_json::to_value(BattlePower::Off).unwrap();
                    }
                    "friendly" => state["friendly_fire_safety"] = true.into(),
                    "spotting" => state["spotter"] = shooter.0.into(),
                    _ => (),
                });
            }
            let scripts = scripts(&config, &world);
            let before = scripts.world().btech.clone();
            let selected_index = if condition == "index" { 999 } else { index };
            let selected_target = if condition == "target" { -99 } else { target.0 };
            let result = scripts.eval_callback::<mlua::Table>(&format!(
                "return btech.unit.sight({},1,{selected_index},{selected_target})",
                shooter.0
            ));
            assert!(result.is_err(), "{condition}");
            assert_eq!(scripts.world().btech, before, "{condition}");
            assert!(scripts.drain_outbox().is_empty());
        }
    }
    for source in [
        include_str!("../game/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
    ] {
        let (_dir, config, world, shooter, _, index) =
            fixture(source, Some(BattleWeapon::LaserAms)).await;
        let scripts = scripts(&config, &world);
        let result = scripts.eval_callback::<mlua::Table>(&format!(
            "return btech.unit.sight({},1,{index})",
            shooter.0
        ));
        assert!(result.unwrap_err().to_string().contains("defensive only"));
    }
}

/// Both sighting and firing recognize airborne VTOLs before their different ammunition checks.
#[tokio::test]
async fn stinger_sight_and_fire_share_vtol_admission() {
    for source in [
        include_str!("../game/mechs/JR7-D"),
        include_str!("../game/mechs/Demolisher"),
    ] {
        let (_dir, config, base, shooter, target, index) = fixture_with_target(
            source,
            Some(BattleWeapon::Lrm5),
            include_str!("../game/mechs/Kestrel"),
        )
        .await;
        for airborne in [false, true] {
            let mut world = base.clone();
            edit(&mut world, shooter, |state| {
                state["ammunition_modes"][index.to_string()] =
                    serde_json::to_value(BattleAmmunitionMode::Stinger).unwrap()
            });
            if airborne {
                edit(&mut world, target, |state| {
                    state["vtol_flight"] = serde_json::to_value(BattleVtolFlight {
                        phase: BattleVtolFlightPhase::Airborne,
                        altitude: 1.0,
                        ..Default::default()
                    })
                    .unwrap()
                });
            }
            refresh_optical_scanners(&mut world, &[shooter]).unwrap();
            let scripts = scripts(&config, &world);
            let before = scripts.world().btech.clone();
            let result = scripts.eval_callback::<mlua::Table>(&format!(
                "return btech.unit.sight({},1,{index},{})",
                shooter.0, target.0
            ));
            if airborne {
                assert!(result.is_ok(), "{result:?}");
            } else {
                assert!(
                    result
                        .unwrap_err()
                        .to_string()
                        .contains("only engage airborne")
                );
                assert_eq!(scripts.world().btech, before);
            }
            let fired = scripts
                .eval_callback::<mlua::Table>(&format!(
                    "return btech.unit.fire({},1,{index},{})",
                    shooter.0, target.0
                ))
                .unwrap_err()
                .to_string();
            assert!(
                fired.contains(if airborne {
                    "Weapon is not ready"
                } else {
                    "only engage airborne"
                }),
                "{fired}"
            );
        }
    }
}
