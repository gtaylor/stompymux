//! MML family selection drives shared combat, ammunition and persistence across supported chassis.
use crate::support;
use crate::support::btech_defense as defense;
use crate::support::btech_firing as firing;
use std::{cell::RefCell, rc::Rc};
use stompymux_rs::*;

/// Published MML cluster columns and both ammunition profiles are independent of chassis.
#[test]
fn mml_profiles_and_cluster_tables() {
    for (weapon, id, capacity, hits) in [
        (Weapon::Mml3, 126, 40, [1, 1, 1, 2, 2, 2, 2, 2, 3, 3, 3]),
        (Weapon::Mml5, 127, 24, [1, 2, 2, 3, 3, 3, 3, 4, 4, 5, 5]),
        (Weapon::Mml7, 128, 17, [2, 2, 3, 4, 4, 4, 4, 6, 6, 7, 7]),
        (Weapon::Mml9, 129, 13, [3, 3, 4, 5, 5, 5, 5, 7, 7, 9, 9]),
    ] {
        assert_eq!(weapon.part_id(), id);
        for (mode, conventional, damage, ammo) in [
            (
                AmmunitionMode::Normal,
                Weapon::Srm6,
                2,
                weapon.profile().ammunition_per_ton,
            ),
            (AmmunitionMode::MmlLrm, Weapon::Lrm20, 1, capacity),
        ] {
            let profile = weapon.profile_for_ammunition(mode);
            assert_eq!(profile.ammunition_per_ton, ammo);
            assert_eq!(profile.damage, damage);
            assert_eq!(
                weapon.ammunition_explosion_damage_for_mode(10, mode),
                10 * u32::from(profile.missiles) * u32::from(damage)
            );
            assert_eq!(
                weapon.supports_indirect_ammunition(mode),
                mode == AmmunitionMode::MmlLrm
            );
            for range in [
                0.0, 1.0, 3.0, 4.0, 6.0, 7.0, 9.0, 10.0, 14.0, 15.0, 21.0, 22.0, 28.0, 29.0,
            ] {
                for extended in [false, true] {
                    assert_eq!(
                        weapon
                            .range_modifier_for_ammunition(
                                range,
                                extended,
                                FireMode::Normal,
                                false,
                                mode
                            )
                            .unwrap(),
                        conventional.range_modifier(range, extended).unwrap()
                    );
                }
            }
            for (roll, count) in (2..=12).zip(hits) {
                assert_eq!(weapon.missile_hits(roll).unwrap(), count);
                let groups = weapon
                    .damage_groups_for_ammunition(mode, Some(roll), 7.0)
                    .unwrap();
                assert_eq!(
                    groups.iter().sum::<u16>(),
                    u16::from(count) * u16::from(damage)
                );
                if damage == 2 {
                    assert_eq!(groups, vec![2; usize::from(count)]);
                } else {
                    assert!(groups.iter().all(|d| (1..=5).contains(d)));
                }
            }
        }
    }
}

/// Both native selectors and Lua calls use the same saved ammo mode and match the same supplied bin.
#[tokio::test]
async fn mml_modes_firing_and_restart_across_chassis() {
    for source in firing::templates() {
        for weapon in [Weapon::Mml3, Weapon::Mml5, Weapon::Mml7, Weapon::Mml9] {
            for (command, flag, mode) in [
                ("mml", "MML_LRM", AmmunitionMode::MmlLrm),
                ("mml", "", AmmunitionMode::Normal),
            ] {
                let (_dir, config, mut world, shooter, target, index) =
                    firing::fixture_with_supply(
                        &source,
                        Some(weapon),
                        include_str!("../game/mechs/AS7-D.toml"),
                        false,
                        Some(flag),
                    )
                    .await;
                let seed = (0..=255)
                    .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
                    .unwrap();
                firing::edit(&mut world, shooter, |state| {
                    state["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
                });
                // Enter the opposite mode first so the same control tests both transition directions.
                if mode == AmmunitionMode::Normal {
                    toggle_mml_ammunition(&mut world, shooter, ObjectId(1), index).unwrap();
                }
                // Both families can fire at seven hexes, with different range penalties and damage.
                firing::edit(&mut world, target, |state| {
                    state["position"]["y"] = 4.into();
                    state["motion"]["point"] =
                        serde_json::to_value(HexCoordinate { x: 0, y: 4 }.center()).unwrap();
                });
                let specifications = battle_weapon_specifications(&world, shooter, true).unwrap();
                let mml: Vec<_> = specifications
                    .iter()
                    .filter(|row| row.weapon == weapon)
                    .collect();
                assert_eq!(mml.len(), 2);
                assert_eq!((mml[0].damage, mml[0].long_range), (2, 9));
                assert_eq!((mml[1].damage, mml[1].long_range), (1, 21));
                assert_eq!(mml[1].ammunition, AmmunitionMode::MmlLrm);
                let display = battle_weapon_specification_text(&world, shooter, true).unwrap();
                assert!(display.contains("(SRM)"));
                assert!(display.contains("(LRM)"));
                let native = Scripts::new(&config, Rc::new(RefCell::new(world.clone()))).unwrap();
                let lua = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                let before = lua.world().btech.clone();
                let call = format!("btech.unit.{command}({},1,{index})", shooter.0);
                assert!(
                    lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                        .is_err()
                );
                assert_eq!(lua.world().btech, before);
                assert!(lua.drain_outbox().is_empty());
                let actual: String = lua.eval_callback(&format!("return {call}")).unwrap();
                assert_eq!(
                    actual,
                    serde_json::to_value(mode).unwrap().as_str().unwrap()
                );
                let output = support::run_text(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("{command} {index}"),
                );
                assert!(output.contains("set to fire"), "{output}");
                assert_eq!(native.world().btech, lua.world().btech);
                let fire = format!("btech.unit.fire({},1,{index},{})", shooter.0, target.0);
                let selected = lua.world().btech.clone();
                assert!(
                    lua.eval_callback::<()>(&format!("{fire}; error('abort')"))
                        .is_err()
                );
                assert_eq!(lua.world().btech, selected);
                let report: mlua::Table = lua.eval_callback(&format!("return {fire}")).unwrap();
                let salvo: mlua::Table = report.get("salvo").unwrap();
                let salvo = salvo
                    .get::<Option<mlua::Table>>("report")
                    .unwrap()
                    .unwrap_or(salvo);
                let groups: mlua::Table = salvo.get("groups").unwrap();
                assert!(groups.raw_len() > 0, "MML hit must deal damage");
                let mut damage = 0;
                for group in groups.sequence_values::<mlua::Table>() {
                    let value = group.unwrap().get::<u16>("damage").unwrap();
                    if mode == AmmunitionMode::Normal {
                        assert_eq!(value, 2);
                    } else {
                        assert!((1..=5).contains(&value));
                    }
                    damage += value;
                }
                let roll: u8 = salvo.get("cluster_roll").unwrap();
                assert_eq!(
                    damage,
                    u16::from(weapon.missile_hits(roll).unwrap())
                        * u16::from(weapon.profile_for_ammunition(mode).damage)
                );
                let launch = report
                    .get::<Option<mlua::Table>>("launch")
                    .unwrap()
                    .unwrap_or(report);
                let expenditure: mlua::Table = launch.get("expenditure").unwrap();
                assert_eq!(
                    expenditure.get::<u8>("heat").unwrap(),
                    weapon.profile().heat
                );
                assert_eq!(
                    expenditure.get::<String>("ammunition_mode").unwrap(),
                    actual
                );
                let draws: mlua::Table = expenditure.get("ammunition").unwrap();
                assert_eq!(draws.raw_len(), 1);
                commands::run(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("fire {index} #{}", target.0),
                )
                .unwrap();
                assert_eq!(native.world().btech, lua.world().btech);
                let saved = lua.world().clone();
                persistence::save(&config.database(), &saved).await.unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    saved.btech
                );
            }
        }
    }
}

/// Every supported defender can intercept both MML missile families through the existing AMS path.
#[tokio::test]
async fn mml_ams_and_missing_supply() {
    for source in firing::templates() {
        for (flag, mode) in [
            ("", AmmunitionMode::Normal),
            ("MML_LRM", AmmunitionMode::MmlLrm),
        ] {
            for target_source in defense::templates() {
                let (_dir, config, mut world, shooter, target, index) =
                    firing::fixture_with_supply(
                        &source,
                        Some(Weapon::Mml9),
                        &target_source,
                        false,
                        Some(flag),
                    )
                    .await;
                if mode == AmmunitionMode::MmlLrm {
                    toggle_mml_ammunition(&mut world, shooter, ObjectId(1), index).unwrap();
                }
                firing::edit(&mut world, target, |state| {
                    state["ams_enabled"] = true.into();
                    state["position"]["y"] = 4.into();
                    state["motion"]["point"] =
                        serde_json::to_value(HexCoordinate { x: 0, y: 4 }.center()).unwrap();
                });
                let seed = (0..=255)
                    .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
                    .unwrap();
                firing::edit(&mut world, shooter, |state| {
                    state["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
                });
                let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
                let report: mlua::Table = scripts
                    .eval_callback(&format!(
                        "return btech.unit.fire({},1,{index},{})",
                        shooter.0, target.0
                    ))
                    .unwrap();
                let ams: mlua::Table = report.get("ams").unwrap();
                assert!(ams.get::<u8>("shot_down").unwrap() > 0);
            }
        }
        let (_dir, config, world, shooter, target, index) = firing::fixture_with_target(
            &source,
            Some(Weapon::Mml3),
            include_str!("../game/mechs/AS7-D.toml"),
        )
        .await;
        let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
        let before = scripts.world().btech.clone();
        assert!(
            scripts
                .eval_callback::<mlua::Table>(&format!(
                    "return btech.unit.fire({},1,{index},{})",
                    shooter.0, target.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before);
    }
}

/// Controls reject unrelated weapons and wrong actors; missing selected supply never changes family.
#[tokio::test]
async fn mml_controls_require_ready_launchers_and_matching_supply() {
    for source in firing::templates() {
        for weapon in [Weapon::Mml3, Weapon::Lrm5] {
            let (_dir, config, mut world, shooter, target, index) = firing::fixture_with_supply(
                &source,
                Some(weapon),
                include_str!("../game/mechs/AS7-D.toml"),
                false,
                Some(""),
            )
            .await;
            let before = world.btech.clone();
            assert!(toggle_mml_ammunition(&mut world, shooter, ObjectId(2), index).is_err());
            assert!(toggle_mml_ammunition(&mut world, shooter, ObjectId(1), usize::MAX).is_err());
            assert_eq!(world.btech, before);
            if !weapon.is_mml() {
                assert!(toggle_mml_ammunition(&mut world, shooter, ObjectId(1), index).is_err());
                assert_eq!(world.btech, before);
                continue;
            }
            assert_eq!(
                toggle_mml_ammunition(&mut world, shooter, ObjectId(1), index).unwrap(),
                AmmunitionMode::MmlLrm
            );
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let selected = scripts.world().btech.clone();
            let output = support::run_text(
                &scripts,
                &config,
                ObjectId(1),
                1,
                &format!("mml/invalid {index}"),
            );
            assert!(output.contains("takes no switches"), "{output}");
            assert_eq!(scripts.world().btech, selected);
            assert!(
                scripts
                    .eval_callback::<mlua::Table>(&format!(
                        "return btech.unit.fire({},1,{index},{})",
                        shooter.0, target.0
                    ))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, selected);
            let mode: String = scripts
                .eval_callback(&format!("return btech.unit.mml({},1,{index})", shooter.0))
                .unwrap();
            assert_eq!(mode, "normal");
            scripts
                .eval_callback::<mlua::Table>(&format!(
                    "return btech.unit.fire({},1,{index},{})",
                    shooter.0, target.0
                ))
                .unwrap();
            let fired = scripts.world().btech.clone();
            assert!(
                scripts
                    .eval_callback::<String>(&format!(
                        "return btech.unit.mml({},1,{index})",
                        shooter.0
                    ))
                    .is_err()
            );
            assert_eq!(scripts.world().btech, fired);
        }
    }
}

/// Bin criticals and hotloaded weapons release the selected family's damage rather than a fixed SRM value.
#[tokio::test]
async fn mml_ammunition_hazards_follow_bin_contents() {
    for source in firing::templates() {
        for (flag, mode, per_missile) in [
            ("", AmmunitionMode::Normal, 2u32),
            ("MML_LRM", AmmunitionMode::MmlLrm, 1),
        ] {
            let (_dir, _config, mut world, id, _, index) = firing::fixture_with_supply(
                &source,
                Some(Weapon::Mml9),
                include_str!("../game/mechs/AS7-D.toml"),
                false,
                Some(flag),
            )
            .await;
            if mode == AmmunitionMode::MmlLrm {
                toggle_mml_ammunition(&mut world, id, ObjectId(1), index).unwrap();
            }
            toggle_battle_hotload(&mut world, id, ObjectId(1), index).unwrap();
            if let Some(unit) = world.btech.constructed_units().get(&id) {
                let loadout = unit.loadout().unwrap();
                let (bin_index, bin) = loadout
                    .ammunition
                    .iter()
                    .enumerate()
                    .find(|(_, bin)| bin.weapon == Weapon::Mml9)
                    .unwrap();
                let rounds = unit.ammunition()[bin_index];
                let mut damaged = unit.clone();
                assert_eq!(
                    damaged.destroy_critical(bin.location).unwrap(),
                    Some(CriticalLoss::Ammunition {
                        index: bin_index,
                        rounds,
                        explosion_damage: u32::from(rounds) * 9 * per_missile,
                    })
                );
                let mut damaged = unit.clone();
                assert_eq!(
                    damaged
                        .destroy_critical(loadout.weapons[index].criticals[0])
                        .unwrap(),
                    Some(CriticalLoss::Weapon {
                        index,
                        explosion_damage: (9 * per_missile) as u8,
                    })
                );
            } else {
                let unit = &world.btech.vehicles()[&id];
                let loadout = unit.loadout().unwrap();
                let report = unit.ammunition_cascade(VehicleSection::Front).unwrap();
                let other: u32 = loadout
                    .ammunition
                    .iter()
                    .enumerate()
                    .filter(|(_, bin)| bin.weapon != Weapon::Mml9)
                    .map(|(i, bin)| bin.weapon.ammunition_explosion_damage(unit.ammunition()[i]))
                    .sum();
                let bin = loadout
                    .ammunition
                    .iter()
                    .position(|bin| bin.weapon == Weapon::Mml9)
                    .unwrap();
                assert_eq!(
                    report.damage - other,
                    u32::from(unit.ammunition()[bin]) * 9 * per_missile
                );
            }
        }
    }
}

/// Long-range MML bins can carry one LRM special round. Controls select it within the LRM
/// family, firing draws the matching bin with LRM grouping, and the selection survives restart.
#[tokio::test]
async fn mml_long_range_special_rounds_select_fire_and_persist() {
    type Toggle = fn(&mut World, ObjectId, ObjectId, usize) -> anyhow::Result<AmmunitionMode>;
    let rounds: [(&str, Toggle, AmmunitionMode, AmmunitionMode); 5] = [
        (
            "Narc/Smoke",
            toggle_battle_narc,
            AmmunitionMode::Narc,
            AmmunitionMode::MmlLrmNarc,
        ),
        (
            "Swarm",
            |world, id, pilot, index| toggle_battle_swarm(world, id, pilot, index, false),
            AmmunitionMode::Swarm,
            AmmunitionMode::MmlLrmSwarm,
        ),
        (
            "Swarm1",
            |world, id, pilot, index| toggle_battle_swarm(world, id, pilot, index, true),
            AmmunitionMode::Swarm1,
            AmmunitionMode::MmlLrmSwarm1,
        ),
        (
            "Sguided",
            toggle_battle_semiguided,
            AmmunitionMode::SemiGuided,
            AmmunitionMode::MmlLrmSemiGuided,
        ),
        (
            "Stinger",
            toggle_battle_stinger,
            AmmunitionMode::Stinger,
            AmmunitionMode::MmlLrmStinger,
        ),
    ];
    for source in firing::templates() {
        for (flag, toggle, round, mode) in rounds {
            let (_dir, config, mut world, shooter, target, index) = firing::fixture_with_supply(
                &source,
                Some(Weapon::Mml9),
                include_str!("../game/mechs/AS7-D.toml"),
                false,
                Some(&format!("MML_LRM {flag}")),
            )
            .await;
            let pilot = ObjectId(1);
            // Narc rounds exist in both families; LRM-only rounds are refused while SRM is selected.
            if round == AmmunitionMode::Narc {
                assert_eq!(toggle(&mut world, shooter, pilot, index).unwrap(), round);
                assert_eq!(
                    toggle_mml_ammunition(&mut world, shooter, pilot, index).unwrap(),
                    mode
                );
            } else {
                let before = world.btech.clone();
                assert!(toggle(&mut world, shooter, pilot, index).is_err());
                assert_eq!(world.btech, before);
                assert_eq!(
                    toggle_mml_ammunition(&mut world, shooter, pilot, index).unwrap(),
                    AmmunitionMode::MmlLrm
                );
                assert_eq!(toggle(&mut world, shooter, pilot, index).unwrap(), mode);
                // Returning to SRM drops the LRM-only round rather than keeping an impossible supply.
                assert_eq!(
                    toggle_mml_ammunition(&mut world, shooter, pilot, index).unwrap(),
                    AmmunitionMode::Normal
                );
                toggle_mml_ammunition(&mut world, shooter, pilot, index).unwrap();
                assert_eq!(toggle(&mut world, shooter, pilot, index).unwrap(), mode);
            }
            // SRM-only Inferno rounds are refused while the LRM family is selected.
            let selected = world.btech.clone();
            assert!(toggle_battle_inferno(&mut world, shooter, pilot, index).is_err());
            assert_eq!(world.btech, selected);
            // Toggling the round again keeps the LRM family.
            assert_eq!(
                toggle(&mut world, shooter, pilot, index).unwrap(),
                AmmunitionMode::MmlLrm
            );
            assert_eq!(toggle(&mut world, shooter, pilot, index).unwrap(), mode);

            let seed = (0..=255)
                .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
                .unwrap();
            firing::edit(&mut world, shooter, |state| {
                state["dice"] = serde_json::to_value(Dice::seeded([seed; 32])).unwrap()
            });
            firing::edit(&mut world, target, |state| {
                state["position"]["y"] = 4.into();
                state["motion"]["point"] =
                    serde_json::to_value(HexCoordinate { x: 0, y: 4 }.center()).unwrap();
            });
            let scripts = Scripts::new(&config, Rc::new(RefCell::new(world))).unwrap();
            let fire = format!(
                "return btech.unit.fire({},1,{index},{})",
                shooter.0, target.0
            );
            if round == AmmunitionMode::Stinger {
                // Stinger rounds keep their airborne-only restriction in the LRM family.
                let before = scripts.world().btech.clone();
                let error = scripts
                    .eval_callback::<mlua::Table>(&fire)
                    .unwrap_err()
                    .to_string();
                assert!(error.contains("airborne"), "{error}");
                assert_eq!(scripts.world().btech, before);
                continue;
            }
            let report: mlua::Table = scripts.eval_callback(&fire).unwrap();
            let launch = report
                .get::<Option<mlua::Table>>("launch")
                .unwrap()
                .unwrap_or(report.clone());
            let expenditure: mlua::Table = launch.get("expenditure").unwrap();
            assert_eq!(
                expenditure.get::<String>("ammunition_mode").unwrap(),
                serde_json::to_value(mode).unwrap().as_str().unwrap()
            );
            let draws: mlua::Table = expenditure.get("ammunition").unwrap();
            assert_eq!(draws.raw_len(), 1);
            if !matches!(round, AmmunitionMode::Swarm | AmmunitionMode::Swarm1) {
                let salvo: mlua::Table = report.get("salvo").unwrap();
                let salvo = salvo
                    .get::<Option<mlua::Table>>("report")
                    .unwrap()
                    .unwrap_or(salvo);
                let groups: mlua::Table = salvo.get("groups").unwrap();
                assert!(groups.raw_len() > 0, "MML hit must deal damage");
                for group in groups.sequence_values::<mlua::Table>() {
                    let damage = group.unwrap().get::<u16>("damage").unwrap();
                    assert!((1..=5).contains(&damage), "LRM grouping: {damage}");
                }
            }
            let saved = scripts.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
    }
}
