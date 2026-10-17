//! Weapon families and their ammunition: LB-X, rockets and one-shots, targeting computers,
//! hardened and half-ton construction, Artemis, jams and hotloading, ultra/rapid/rotary/gatling
//! and special autocannons, caseless and armor-piercing rounds; then observer and
//! cockpit feedback, fueled flamers, status and warning reports, and kick, punch,
//! and trip attacks.

use crate::btech_motion_common::{
    balance_hit, balance_skill, computer_skill, configured_shot_rules, fall_rules, fixture,
    fixture_assets, fixture_source, kick_fixture, kick_rules, lock_fixture, optical_aim_rules,
    overheat_due, overheat_rules, shot_fixture, shot_rules, shot_seed, shot_skill,
    single_critical_seed, stagger_rules, stand_fixture, water_fixture,
};
use crate::support;
use crate::support::{install, restore_database, snapshot_database};
use stompymux_rs::{MechTemplate, ObjectId, persistence};

/// LB-X mode selection chooses matching bins and commits pellet damage and recycle across both adapters.
async fn lbx_modes_matrix(weapons: &[stompymux_rs::Weapon]) {
    use stompymux_rs::*;
    let (_dir, config, mut pristine, id, target) = shot_fixture().await;
    shot_skill(&mut pristine, 30);
    set_battle_character_value(
        &mut pristine,
        ObjectId(1),
        "Gunnery-Ballistic",
        CharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    shot_seed(&mut pristine, id, 1);
    shot_seed(&mut pristine, target, 1);
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let pristine_db = snapshot_database(&config);
    let mut probed = false;
    for weapon in weapons.iter().copied() {
        let mut base = pristine.clone();
        let mut definition = base.btech.constructed_units()[&id].definition().clone();
        let mut part = definition.sections[&MechSection::LeftArm].criticals[&2].clone();
        part.equipment = weapon.name().into();
        for slot in 0..weapon.profile().critical_slots {
            definition
                .sections
                .get_mut(&MechSection::LeftTorso)
                .unwrap()
                .criticals
                .insert(slot, part.clone());
        }
        let torso = definition
            .sections
            .get_mut(&MechSection::RightTorso)
            .unwrap();
        let mut bin = torso.criticals[&0].clone();
        let capacity = weapon.profile().ammunition_per_ton;
        bin.equipment = format!("Ammo_{}", weapon.name());
        bin.data = capacity.to_string();
        torso.criticals.insert(0, bin.clone());
        bin.modes.push("LBX/Cluster".into());
        torso.criticals.insert(3, bin);
        base.btech
            .rewrite_unit_record(id, |record| {
                crate::support::templates::set_record_definition(record, definition);
                record["ammunition"] = serde_json::json!([capacity, capacity]);
            })
            .unwrap();
        base.validate(&config).unwrap();
        let index = base.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == weapon)
            .unwrap();
        let normal_aim =
            battle_aim_modifiers(&base, id, target, index, 4, optical_aim_rules()).unwrap();
        for mode in [AmmunitionMode::Normal, AmmunitionMode::Cluster] {
            restore_database(&config, &pristine_db);
            install(&native, base.clone());
            install(&lua, base.clone());
            if mode == AmmunitionMode::Cluster {
                assert!(
                    lua.eval_callback::<()>(&format!(
                        "btech.unit.lbx({},1,{index}); error('abort')",
                        id.0
                    ))
                    .is_err()
                );
                assert_eq!(lua.world().btech, base.btech);
                let text =
                    support::run_text(&native, &config, ObjectId(1), 1, &format!("lbx {index}"));
                assert!(text.contains("LBX fire mode"), "{text}");
                assert_eq!(
                    lua.eval_callback::<String>(&format!(
                        "return btech.unit.lbx({},1,{index})",
                        id.0
                    ))
                    .unwrap(),
                    "cluster"
                );
            }
            assert_eq!(native.world().btech, lua.world().btech);
            let before = lua.world().clone();
            let aim =
                battle_aim_modifiers(&before, id, target, index, 4, optical_aim_rules()).unwrap();
            let accuracy = if mode == AmmunitionMode::Cluster {
                -1
            } else {
                0
            };
            assert_eq!(aim.ammunition_accuracy, accuracy);
            assert_eq!(
                aim.subtotal(),
                normal_aim
                    .subtotal()
                    .map(|value| value + i32::from(accuracy))
            );
            let selected = usize::from(mode == AmmunitionMode::Cluster);
            let mut empty = before.clone();
            empty
                .btech
                .rewrite_unit_record(id, |record| {
                    record["ammunition"][selected] = 0.into();
                })
                .unwrap();
            assert_eq!(
                empty.btech.constructed_units()[&id]
                    .weapon_readiness(index)
                    .unwrap()
                    .ammunition,
                0
            );
            let checkpoint = empty.btech.clone();
            assert!(
                resolve_battle_shot(&mut empty, id, ObjectId(1), target, index, shot_rules())
                    .is_err()
            );
            assert_eq!(empty.btech, checkpoint);
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.fire({},1,{index},{}); error('abort')",
                    id.0, target.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before.btech);
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire {index} #{}", target.0),
            );
            assert!(text.contains("You fire"), "{text}");
            let result: mlua::Table = lua
                .eval_callback(&format!(
                    "return btech.unit.fire({},1,{index},{})",
                    id.0, target.0
                ))
                .unwrap();
            let salvo: mlua::Table = result
                .get::<mlua::Table>("salvo")
                .unwrap()
                .get("report")
                .unwrap();
            let roll: Option<u8> = salvo.get("cluster_roll").unwrap();
            assert_eq!(roll.is_some(), mode == AmmunitionMode::Cluster);
            let groups: mlua::Table = salvo.get("groups").unwrap();
            let expected = weapon
                .damage_groups_for_ammunition(mode, roll, aim.distance)
                .unwrap();
            assert_eq!(groups.len().unwrap() as usize, expected.len());
            for (offset, damage) in expected.iter().enumerate() {
                assert_eq!(
                    groups
                        .get::<mlua::Table>(offset + 1)
                        .unwrap()
                        .get::<u16>("damage")
                        .unwrap(),
                    *damage
                );
            }
            assert_eq!(native.world().btech, lua.world().btech);
            let mut fired = native.world().clone();
            let unit = &fired.btech.constructed_units()[&id];
            assert_eq!(unit.ammunition()[selected], u16::from(capacity) - 1);
            assert_eq!(unit.ammunition()[1 - selected], u16::from(capacity));
            assert_eq!(unit.heat().stored, f64::from(weapon.profile().heat));
            let checkpoint = fired.btech.clone();
            assert!(toggle_battle_lbx(&mut fired, id, ObjectId(1), index).is_err());
            assert_eq!(fired.btech, checkpoint);
            // Restart probe runs once per shard, covering save/load replay and recycle parity.
            if !probed {
                probed = true;
                persistence::save(&config.database(), &fired).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                assert_eq!(restored.btech, fired.btech);
                for _ in 0..weapon.profile().recycle_seconds {
                    assert_eq!(
                        advance_battle_recycle(&mut restored),
                        advance_battle_recycle(&mut fired)
                    );
                    assert_eq!(restored.btech, fired.btech);
                }
                assert_eq!(
                    restored.btech.constructed_units()[&id]
                        .ammunition_mode(index)
                        .unwrap(),
                    mode
                );
            }
        }
    }
}

#[tokio::test]
async fn lbx_modes_ammunition_accuracy_native_lua_rollback_and_restart_clanlbx2() {
    use stompymux_rs::Weapon;
    lbx_modes_matrix(&[Weapon::ClanLbx2]).await;
}

#[tokio::test]
async fn lbx_modes_ammunition_accuracy_native_lua_rollback_and_restart_clanlbx5() {
    use stompymux_rs::Weapon;
    lbx_modes_matrix(&[Weapon::ClanLbx5]).await;
}

#[tokio::test]
async fn lbx_modes_ammunition_accuracy_native_lua_rollback_and_restart_clanlbx10() {
    use stompymux_rs::Weapon;
    lbx_modes_matrix(&[Weapon::ClanLbx10]).await;
}

#[tokio::test]
async fn lbx_modes_ammunition_accuracy_native_lua_rollback_and_restart_clanlbx20() {
    use stompymux_rs::Weapon;
    lbx_modes_matrix(&[Weapon::ClanLbx20]).await;
}

#[tokio::test]
async fn lbx_modes_ammunition_accuracy_native_lua_rollback_and_restart_lbx2() {
    use stompymux_rs::Weapon;
    lbx_modes_matrix(&[Weapon::Lbx2]).await;
}

#[tokio::test]
async fn lbx_modes_ammunition_accuracy_native_lua_rollback_and_restart_lbx5() {
    use stompymux_rs::Weapon;
    lbx_modes_matrix(&[Weapon::Lbx5]).await;
}

#[tokio::test]
async fn lbx_modes_ammunition_accuracy_native_lua_rollback_and_restart_lbx10() {
    use stompymux_rs::Weapon;
    lbx_modes_matrix(&[Weapon::Lbx10]).await;
}

#[tokio::test]
async fn lbx_modes_ammunition_accuracy_native_lua_rollback_and_restart_lbx20() {
    use stompymux_rs::Weapon;
    lbx_modes_matrix(&[Weapon::Lbx20]).await;
}

/// Self-contained launchers spend on hits and misses, retain failed Streak locks, and never reload on restart.
async fn rocket_and_one_shot_matrix(weapons: &[stompymux_rs::Weapon]) {
    use stompymux_rs::*;
    let miss_seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    let (_dir, config, mut pristine, id, target) = shot_fixture().await;
    shot_seed(&mut pristine, target, 1);
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let pristine_db = snapshot_database(&config);
    let mut probed = false;
    for weapon in weapons.iter().copied() {
        for skill in [0, 30] {
            restore_database(&config, &pristine_db);
            let mut base = pristine.clone();
            shot_skill(&mut base, skill);
            set_battle_character_value(
                &mut base,
                ObjectId(1),
                "Gunnery-Missile",
                CharacterValue {
                    value: skill,
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
            shot_seed(&mut base, id, if skill == 0 { miss_seed } else { 1 });
            let mut definition = base.btech.constructed_units()[&id].definition().clone();
            let mut part = definition.sections[&MechSection::LeftArm].criticals[&2].clone();
            part.equipment = weapon.name().into();
            part.modes = vec!["OneShot".into()];
            for slot in 0..weapon.profile().critical_slots {
                definition
                    .sections
                    .get_mut(&MechSection::LeftTorso)
                    .unwrap()
                    .criticals
                    .insert(slot, part.clone());
            }
            for slot in 6..6 + weapon.profile().critical_slots {
                definition
                    .sections
                    .get_mut(&MechSection::LeftTorso)
                    .unwrap()
                    .criticals
                    .insert(slot, part.clone());
            }
            base.btech
                .rewrite_unit_record(id, |record| {
                    crate::support::templates::set_record_definition(record, definition)
                })
                .unwrap();
            base.validate(&config).unwrap();
            let index = base.btech.constructed_units()[&id]
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|m| m.weapon == weapon)
                .unwrap();
            assert_eq!(
                base.btech.constructed_units()[&id]
                    .weapon_readiness(index)
                    .unwrap()
                    .ammunition,
                1
            );
            let mut expected = base.clone();
            let report = resolve_battle_shot(
                &mut expected,
                id,
                ObjectId(1),
                target,
                index,
                configured_shot_rules(&config),
            )
            .unwrap();
            assert_eq!(report.launched, skill == 30 || !weapon.is_streak());
            assert_eq!(report.salvo.is_some(), skill == 30);
            assert_eq!(
                report.expenditure.heat,
                if report.launched {
                    weapon.profile().heat
                } else {
                    0
                }
            );
            assert!(report.expenditure.ammunition.is_empty());
            install(&native, base.clone());
            install(&lua, base.clone());
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.fire({},1,{index},{}); error('abort')",
                    id.0, target.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, base.btech);
            support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire {index} #{}", target.0),
            );
            lua.eval_callback::<mlua::Table>(&format!(
                "return btech.unit.fire({},1,{index},{})",
                id.0, target.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            assert_eq!(native.world().btech, expected.btech);
            let mut fired = native.world().clone();
            let unit = &fired.btech.constructed_units()[&id];
            let other = unit
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .enumerate()
                .find(|(i, m)| *i != index && m.weapon == weapon && m.one_shot)
                .unwrap()
                .0;
            assert_eq!(unit.weapon_readiness(other).unwrap().ammunition, 1);
            assert!(!unit.weapon_readiness(other).unwrap().spent);
            let ready = unit.weapon_readiness(index).unwrap();
            assert_eq!(ready.spent, report.launched);
            assert_eq!(ready.ammunition, u32::from(!report.launched));
            assert_eq!(
                unit.ammunition(),
                base.btech.constructed_units()[&id].ammunition()
            );
            // Restart probe runs once per shard, covering save/load replay and recycle parity.
            if !probed {
                probed = true;
                persistence::save(&config.database(), &fired).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                restored
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
                assert_eq!(fired.btech, restored.btech);
                for _ in 0..weapon.profile().recycle_seconds {
                    assert_eq!(
                        advance_battle_recycle(&mut fired),
                        advance_battle_recycle(&mut restored)
                    );
                }
                assert_eq!(fired.btech, restored.btech);
                assert_eq!(
                    restored.btech.constructed_units()[&id]
                        .weapon_readiness(index)
                        .unwrap()
                        .ready,
                    !report.launched
                );
                if report.launched {
                    let checkpoint = restored.btech.clone();
                    assert!(
                        resolve_battle_shot(
                            &mut restored,
                            id,
                            ObjectId(1),
                            target,
                            index,
                            configured_shot_rules(&config)
                        )
                        .is_err()
                    );
                    assert_eq!(restored.btech, checkpoint);
                } else {
                    set_battle_character_value(
                        &mut restored,
                        ObjectId(1),
                        "Gunnery-Missile",
                        CharacterValue {
                            value: 30,
                            experience: 0,
                            last_used: 0,
                        },
                    )
                    .unwrap();
                    let launch = resolve_battle_shot(
                        &mut restored,
                        id,
                        ObjectId(1),
                        target,
                        index,
                        configured_shot_rules(&config),
                    )
                    .unwrap();
                    assert!(launch.launched);
                    assert!(
                        restored.btech.constructed_units()[&id]
                            .weapon_readiness(index)
                            .unwrap()
                            .spent
                    );
                }
            }
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn rocket_and_one_shot_native_lua_expenditure_rollback_restart_01() {
    use stompymux_rs::Weapon;
    rocket_and_one_shot_matrix(&[Weapon::Rocket10, Weapon::Rocket15, Weapon::Rocket20]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn rocket_and_one_shot_native_lua_expenditure_rollback_restart_02() {
    use stompymux_rs::Weapon;
    rocket_and_one_shot_matrix(&[Weapon::Srm4, Weapon::ClanLrm5, Weapon::ClanLrm10]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn rocket_and_one_shot_native_lua_expenditure_rollback_restart_03() {
    use stompymux_rs::Weapon;
    rocket_and_one_shot_matrix(&[Weapon::ClanLrm15, Weapon::ClanLrm20, Weapon::ClanSrm2]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn rocket_and_one_shot_native_lua_expenditure_rollback_restart_04() {
    use stompymux_rs::Weapon;
    rocket_and_one_shot_matrix(&[Weapon::ClanSrm4, Weapon::ClanSrm6, Weapon::ClanStreakSrm2]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn rocket_and_one_shot_native_lua_expenditure_rollback_restart_05() {
    use stompymux_rs::Weapon;
    rocket_and_one_shot_matrix(&[
        Weapon::ClanStreakSrm4,
        Weapon::ClanStreakSrm6,
        Weapon::StreakSrm2,
    ])
    .await;
}

/// Native/Lua aiming shares computer eligibility, LB-X exclusion and persisted global critical failure.
#[tokio::test(flavor = "current_thread")]
async fn targeting_computer_aim_native_lua_damage_and_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut pristine, id, target) = shot_fixture().await;
    shot_seed(&mut pristine, id, 1);
    shot_seed(&mut pristine, target, 1);
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let pristine_db = snapshot_database(&config);
    for weapon in [
        Weapon::MediumPulseLaser,
        Weapon::Lbx10,
        Weapon::Flamer,
        Weapon::HeavyFlamer,
        Weapon::VehicleFlamer,
        Weapon::HeavyMachineGun,
        Weapon::Srm4,
    ] {
        let mut base = pristine.clone();
        shot_skill(&mut base, 30);
        set_battle_character_value(
            &mut base,
            ObjectId(1),
            weapon.gunnery_skill(true),
            CharacterValue {
                value: 30,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        let mut template = base.btech.constructed_units()[&id].definition().clone();
        let mut part = template.sections[&MechSection::LeftArm].criticals[&2].clone();
        part.equipment = weapon.name().into();
        for slot in 2..2 + weapon.profile().critical_slots {
            template
                .sections
                .get_mut(&MechSection::LeftArm)
                .unwrap()
                .criticals
                .insert(slot, part.clone());
        }
        if weapon.profile().ammunition_per_ton > 0 {
            let bin = template
                .sections
                .get_mut(&MechSection::RightTorso)
                .unwrap()
                .criticals
                .get_mut(&0)
                .unwrap();
            bin.equipment = format!("Ammo_{}", weapon.name());
            bin.data = weapon.profile().ammunition_per_ton.to_string();
        }
        base.btech
            .rewrite_unit_record(id, |record| {
                crate::support::templates::set_record_definition(record, template.clone());
                if weapon.profile().ammunition_per_ton > 0 {
                    record["ammunition"][0] = weapon.profile().ammunition_per_ton.into();
                }
            })
            .unwrap();
        let index = base.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|m| m.weapon == weapon)
            .unwrap();
        let without =
            battle_aim_modifiers(&base, id, target, index, 4, optical_aim_rules()).unwrap();
        part.equipment = "TargetingComputer".into();
        for slot in 4..6 {
            template
                .sections
                .get_mut(&MechSection::RightTorso)
                .unwrap()
                .criticals
                .insert(slot, part.clone());
        }
        base.btech
            .rewrite_unit_record(id, |record| {
                crate::support::templates::set_record_definition(record, template.clone())
            })
            .unwrap();
        base.validate(&config).unwrap();
        let assisted =
            battle_aim_modifiers(&base, id, target, index, 4, optical_aim_rules()).unwrap();
        let bonus = if weapon.supports_targeting_computer() {
            -1
        } else {
            0
        };
        assert_eq!(assisted.targeting_computer, bonus);
        assert_eq!(
            assisted.subtotal(),
            without.subtotal().map(|n| n + i32::from(bonus))
        );
        if weapon.is_lbx() {
            let mut cluster = base.clone();
            toggle_battle_lbx(&mut cluster, id, ObjectId(1), index).unwrap();
            let aim =
                battle_aim_modifiers(&cluster, id, target, index, 4, optical_aim_rules()).unwrap();
            assert_eq!(aim.targeting_computer, 0);
            assert_eq!(aim.ammunition_accuracy, -1);
        }
        for damaged in [false, true] {
            let mut world = base.clone();
            if damaged {
                let mut unit = world.btech.constructed_units()[&id].clone();
                unit.destroy_critical(CriticalLocation {
                    section: MechSection::RightTorso,
                    slot: 4,
                })
                .unwrap();
                world
                    .btech
                    .rewrite_unit_record(id, |record| {
                        *record = serde_json::to_value(unit).unwrap();
                    })
                    .unwrap();
            }
            restore_database(&config, &pristine_db);
            persistence::save(&config.database(), &world).await.unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            restored
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .insert(Flag::Connected);
            assert_eq!(world.btech, restored.btech);
            install(&native, world.clone());
            install(&lua, restored);
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.fire({},1,{index},{}); error('abort')",
                    id.0, target.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, world.btech);
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire {index} #{}", target.0),
            );
            assert!(text.contains("You fire"), "{text}");
            let report: mlua::Table = lua
                .eval_callback(&format!(
                    "return btech.unit.fire({},1,{index},{})",
                    id.0, target.0
                ))
                .unwrap();
            assert_eq!(
                report
                    .get::<mlua::Table>("aim")
                    .unwrap()
                    .get::<i8>("targeting_computer")
                    .unwrap(),
                if damaged { 0 } else { bonus }
            );
            assert_eq!(native.world().btech, lua.world().btech);
        }
    }
}

/// Combat criticals announce computer failure once, and section destruction removes its remaining mass.
#[tokio::test(flavor = "current_thread")]
async fn targeting_computer_combat_critical_notice_and_section_loss() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, _target) = shot_fixture().await;
    let mut template = base.btech.constructed_units()[&id].definition().clone();
    let mut part = template.sections[&MechSection::RightTorso].criticals[&1].clone();
    part.equipment = "TargetingComputer".into();
    for slot in 4..6 {
        template
            .sections
            .get_mut(&MechSection::RightTorso)
            .unwrap()
            .criticals
            .insert(slot, part.clone());
    }
    base.btech
        .rewrite_unit_record(id, |record| {
            crate::support::templates::set_record_definition(record, template)
        })
        .unwrap();
    let hit = Hit {
        section: MechSection::RightTorso,
        rear_armor: false,
        through_armor_critical: true,
    };
    let mut found = false;
    for seed in 0..=255 {
        shot_seed(&mut base, id, seed);
        let mut world = base.clone();
        let report = resolve_battle_tactical_impact(&mut world, id, hit, 1, fall_rules()).unwrap();
        if report.impact.criticals.len() != 1
            || !report.impact.criticals.iter().any(|(_, loss)| {
                matches!(
                    loss,
                    CriticalLoss::System {
                        system: System::TargetingComputer
                    }
                )
            })
        {
            continue;
        }
        assert!(
            !world.btech.constructed_units()[&id]
                .targeting_computer_operational()
                .unwrap()
        );
        assert_eq!(
            report
                .notices
                .iter()
                .filter(|notice| notice.text == "Your targeting computer is destroyed!")
                .count(),
            1
        );
        persistence::save(&config.database(), &base).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            resolve_battle_tactical_impact(&mut restored, id, hit, 1, fall_rules()).unwrap(),
            report
        );
        assert_eq!(restored.btech, world.btech);
        found = true;
        break;
    }
    assert!(found);
    let mut ordinary = base.clone();
    ordinary
        .btech
        .rewrite_unit_record(id, |record| {
            let parts = record["definition"]["sections"]["RightTorso"]["criticals"]
                .as_object_mut()
                .unwrap();
            parts.remove("4");
            parts.remove("5");
            crate::support::templates::book_legal_record(record);
        })
        .unwrap();
    // A ton per computer slot, sized to the direct-fire weapons (TechManual p. 238).
    let computer = base.btech.constructed_units()[&id].definition().sections
        [&MechSection::RightTorso]
        .criticals
        .values()
        .filter(|part| part.equipment == "TargetingComputer")
        .count() as u32;
    assert_eq!(
        base.btech.constructed_units()[&id]
            .mass()
            .unwrap()
            .equipment
            - ordinary.btech.constructed_units()[&id]
                .mass()
                .unwrap()
                .equipment,
        computer * 1024
    );
    for world in [&mut base, &mut ordinary] {
        apply_damage_phase(world, id, MechSection::RightTorso, 8, DamagePhase::Internal).unwrap();
    }
    assert!(
        !base.btech.constructed_units()[&id]
            .targeting_computer_operational()
            .unwrap()
    );
    // Without its computer the ordinary design carries the ton as cargo space instead.
    let (with, without) = (
        base.btech.constructed_units()[&id].mass().unwrap(),
        ordinary.btech.constructed_units()[&id].mass().unwrap(),
    );
    assert_eq!(
        (with.equipment, with.armor, with.structure, with.ammunition),
        (
            without.equipment,
            without.armor,
            without.structure,
            without.ammunition
        )
    );
}

/// Section destruction and attack-induced flooding announce computer loss once and replay atomically.
#[tokio::test(flavor = "current_thread")]
async fn targeting_computer_environment_failure_feedback_replays() {
    use stompymux_rs::*;
    for flooding in [false, true] {
        let (_dir, config, mut base, id) = water_fixture(if flooding { 2 } else { 0 }).await;
        let mut template = base.btech.constructed_units()[&id].definition().clone();
        let mut part = template.sections[&MechSection::RightTorso].criticals[&1].clone();
        // A targeting computer is one contiguous run in one location (TechManual p. 238).
        part.equipment = "TargetingComputer".into();
        template
            .sections
            .get_mut(&MechSection::RightTorso)
            .unwrap()
            .criticals
            .insert(4, part.clone());
        base.btech
            .rewrite_unit_record(id, |record| {
                crate::support::templates::set_record_definition(record, template)
            })
            .unwrap();
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() < 8)
            .unwrap();
        for section in [MechSection::RightTorso, MechSection::LeftTorso] {
            shot_seed(&mut base, id, seed);
            let before = base.btech.constructed_units()[&id].sections()[&section].clone();
            let hit = Hit {
                section,
                rear_armor: flooding,
                through_armor_critical: false,
            };
            let damage = if flooding {
                before.rear
            } else {
                before.armor + before.internal
            };
            persistence::save(&config.database(), &base).await.unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            let report =
                resolve_battle_tactical_impact(&mut base, id, hit, damage, fall_rules()).unwrap();
            assert_eq!(
                resolve_battle_tactical_impact(&mut restored, id, hit, damage, fall_rules())
                    .unwrap(),
                report
            );
            assert_eq!(base.btech, restored.btech);
            assert_eq!(
                report
                    .notices
                    .iter()
                    .filter(|n| n.text == "Your Targeting Computer is Destroyed")
                    .count(),
                usize::from(section == MechSection::RightTorso)
            );
            assert!(
                !base.btech.constructed_units()[&id]
                    .targeting_computer_operational()
                    .unwrap()
            );
            if flooding {
                assert!(report.flooding.iter().any(|f| f.section == section));
                assert_eq!(
                    base.btech.constructed_units()[&id].sections()[&section].internal,
                    before.internal
                );
            } else {
                assert_eq!(
                    base.btech.constructed_units()[&id].sections()[&section].internal,
                    0
                );
            }
        }
    }
}

/// Hardened gyro criticals progress from no check to a piloting check and then a forced fall, with restart replay.
#[tokio::test(flavor = "current_thread")]
async fn hardened_gyro_critical_checks_and_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id) = fixture('.').await;
    balance_skill(&mut base);
    base.btech
        .rewrite_unit_record(id, |record| {
            record["definition"]["attributes"]["specials"] = "HDGYRO".into();
            // The gyro's three extra tons come out of the weapons, keeping the armor.
            crate::support::templates::book_legal_record_keeping(record, Some(MechSection::Head));
        })
        .unwrap();
    let slots: Vec<_> = base.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .systems
        .iter()
        .filter(|p| p.system == System::Gyro)
        .map(|p| p.location)
        .collect();
    for prior in 0..3 {
        let mut before = base.clone();
        for slot in slots.iter().take(prior) {
            destroy_battle_critical(&mut before, id, *slot).unwrap();
        }
        let mut found = false;
        for seed in 0..=255 {
            shot_seed(&mut before, id, seed);
            let mut world = before.clone();
            let report = resolve_battle_tactical_impact(
                &mut world,
                id,
                balance_hit(MechSection::CenterTorso, true),
                1,
                fall_rules(),
            )
            .unwrap();
            if report
                .impact
                .criticals
                .iter()
                .filter(|(_, loss)| {
                    matches!(
                        loss,
                        CriticalLoss::System {
                            system: System::Gyro
                        }
                    )
                })
                .count()
                != 1
            {
                continue;
            }
            let notices = [
                "Your hardened gyro takes a hit!",
                "Your Gyro has been damaged!",
                "Your Gyro has been destroyed!",
            ];
            assert!(report.notices.iter().any(|n| n.text == notices[prior]));
            let checks: Vec<_> = report
                .balance
                .iter()
                .filter(|b| {
                    matches!(
                        b.cause,
                        BalanceCause::Critical {
                            system: System::Gyro,
                            ..
                        }
                    )
                })
                .collect();
            if prior == 0 {
                assert!(checks.is_empty());
            } else if prior == 1 {
                assert_eq!(checks.len(), 1);
                assert_eq!(checks[0].check.unwrap().damage, 3);
                assert!(checks[0].check.unwrap().success);
            } else {
                assert_eq!(checks.len(), 1);
                assert!(checks[0].check.is_none());
                assert!(checks[0].fall.is_some());
            }
            if prior == 2 {
                let checkpoint = world.btech.clone();
                let error = begin_battle_stand(
                    &mut world,
                    id,
                    ObjectId(1),
                    StandMode::Normal,
                    fall_rules(),
                )
                .unwrap_err();
                assert!(error.to_string().contains("gyro"), "{error}");
                assert_eq!(world.btech, checkpoint);
            }
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            restored
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .insert(Flag::Connected);
            assert_eq!(
                resolve_battle_tactical_impact(
                    &mut restored,
                    id,
                    balance_hit(MechSection::CenterTorso, true),
                    1,
                    fall_rules()
                )
                .unwrap(),
                report
            );
            assert_eq!(restored.btech, world.btech);
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
            let inspected: mlua::Table = scripts
                .eval_callback(&format!("return btech.unit.state({})", id.0))
                .unwrap();
            assert_eq!(inspected.get::<String>("gyro").unwrap(), "hardened");
            assert_eq!(inspected.get::<u8>("gyro_damage").unwrap(), prior as u8);
            found = true;
            break;
        }
        assert!(found, "critical stage {prior}");
    }
}

/// Half-bin firing shares native/Lua expenditure, typed cluster selection and durable capacity validation.
#[tokio::test(flavor = "current_thread")]
async fn half_ton_native_lua_fire_capacity_and_restart() {
    use stompymux_rs::*;
    for weapon in [Weapon::MachineGun, Weapon::HeavyMachineGun] {
        let (_dir, config, mut base, id, target) = shot_fixture().await;
        shot_skill(&mut base, 30);
        set_battle_character_value(
            &mut base,
            ObjectId(1),
            "Gunnery-Ballistic",
            CharacterValue {
                value: 30,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        shot_seed(&mut base, id, 1);
        shot_seed(&mut base, target, 1);
        let capacity = weapon.profile().ammunition_per_ton / 2;
        let mut template = base.btech.constructed_units()[&id].definition().clone();
        let mut part = template.sections[&MechSection::LeftArm].criticals[&2].clone();
        part.equipment = weapon.name().into();
        for slot in 2..2 + weapon.profile().critical_slots {
            template
                .sections
                .get_mut(&MechSection::LeftArm)
                .unwrap()
                .criticals
                .insert(slot, part.clone());
        }
        let bin = template
            .sections
            .get_mut(&MechSection::RightTorso)
            .unwrap()
            .criticals
            .get_mut(&0)
            .unwrap();
        bin.equipment = format!("Ammo_{}", weapon.name());
        bin.data = capacity.to_string();
        bin.modes = vec!["Halfton".into()];
        base.btech
            .rewrite_unit_record(id, |record| {
                crate::support::templates::set_record_definition(record, template);
                record["ammunition"][0] = capacity.into();
            })
            .unwrap();
        let index = base.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|m| m.weapon == weapon)
            .unwrap();
        base.validate(&config).unwrap();
        let mut invalid = base.clone();
        // Overfill the bin through the raw record, which skips validation.
        invalid
            .btech
            .rewrite_unit_record(id, |record| record["ammunition"][0] = (capacity + 1).into())
            .unwrap();
        assert!(invalid.validate(&config).is_err());
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.fire({},1,{index},{}); error('abort')",
                id.0, target.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, base.btech);
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire {index} #{}", target.0),
        );
        assert!(text.contains("You fire"), "{text}");
        lua.eval_callback::<mlua::Table>(&format!(
            "return btech.unit.fire({},1,{index},{})",
            id.0, target.0
        ))
        .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        let mut fired = native.world().clone();
        assert_eq!(
            fired.btech.constructed_units()[&id].ammunition(),
            [u16::from(capacity) - 1]
        );
        persistence::save(&config.database(), &fired).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(fired.btech, restored.btech);
        for _ in 0..weapon.profile().recycle_seconds {
            assert_eq!(
                advance_battle_recycle(&mut fired),
                advance_battle_recycle(&mut restored)
            );
        }
        let loadout = restored.btech.constructed_units()[&id].loadout().unwrap();
        assert_eq!(loadout.ammunition[0].capacity, u16::from(capacity));
        assert!(loadout.ammunition[0].half_ton);
        assert_eq!(fired.btech, restored.btech);
    }
}

/// Small cockpit construction affects control targets without pretending the unit is damaged.
#[tokio::test]
async fn small_cockpit_piloting_mass_and_restart() {
    use stompymux_rs::{CriticalLocation, MechSection, roll_battle_piloting};
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    let standard = stompymux_rs::Mech::from_template(template.clone()).unwrap();
    support::templates::small_cockpit(&mut template, "SMCPIT");
    let (_dir, config, mut world, id) = fixture_assets(
        &format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12)),
        template,
    )
    .await;
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.mass().unwrap().cockpit, 2048);
    // The ton the small cockpit saves becomes cargo space, keeping the design at its tonnage.
    assert_eq!(unit.mass().unwrap().total, standard.mass().unwrap().total);
    assert_eq!(unit.mobility(), standard.mobility());
    for damaged in [false, true] {
        if damaged {
            stompymux_rs::destroy_battle_critical(
                &mut world,
                id,
                CriticalLocation {
                    section: MechSection::CenterTorso,
                    slot: 3,
                },
            )
            .unwrap();
        }
        shot_seed(&mut world, id, 12);
        persistence::save(&config.database(), &world).await.unwrap();
        let check = roll_battle_piloting(&mut world, id, -1, true).unwrap();
        let mut prone = world.clone();
        let mut state = serde_json::to_value(&prone.btech).unwrap();
        state["constructed"][id.0.to_string()]["posture"] = "prone".into();
        prone.btech = serde_json::from_value(state).unwrap();
        assert_eq!(
            stompymux_rs::battle_stand_target(&prone, id, ObjectId(1), true).unwrap(),
            check.target + 1
        );
        assert_eq!(check.cockpit, 1);
        assert_eq!(check.damage, if damaged { 3 } else { 0 });
        assert_eq!(
            check.target,
            i32::from(check.skill) + i32::from(check.damage)
        );
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            roll_battle_piloting(&mut restored, id, -1, true).unwrap(),
            check
        );
        assert_eq!(restored.btech, world.btech);
    }
}

/// Hardened armor costs one running MP and adds one to every piloting roll.
#[tokio::test]
async fn hardened_armor_slows_running_and_hampers_piloting() {
    use stompymux_rs::*;
    let (_dir, _config, mut world, id, _target) = shot_fixture().await;
    let standard = world.btech.constructed_units()[&id]
        .mobility()
        .maximum_speed;
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    let value = definition.attributes.entry("specials".into()).or_default();
    value.push_str(" HardenedArmor_Tech");
    world
        .btech
        .rewrite_unit_record(id, |record| {
            crate::support::templates::set_record_definition(record, definition)
        })
        .unwrap();
    let hardened = world.btech.constructed_units()[&id]
        .mobility()
        .maximum_speed;
    assert_eq!(hardened, standard - 10.75);
    for (speed, armor) in [(0.0, 1), (hardened * 2.0 / 3.0, 1), (hardened, 1)] {
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["motion"]["speed"] = serde_json::json!(speed);
            })
            .unwrap();
        let check = roll_battle_piloting(&mut world, id, 0, true).unwrap();
        assert_eq!(check.armor, armor, "speed {speed}");
        assert_eq!(
            check.target,
            i32::from(check.skill)
                + i32::from(check.damage)
                + i32::from(check.cockpit)
                + i32::from(armor)
        );
    }
}

/// Give the center torso SRM-4 an Artemis IV controller beside it (TechManual p. 206), moving
/// the displaced jump jet to the left torso.
fn install_center_artemis(template: &mut stompymux_rs::MechTemplate) {
    use stompymux_rs::*;
    let jet = template
        .sections
        .get_mut(&MechSection::CenterTorso)
        .unwrap()
        .criticals
        .insert(
            11,
            CriticalDefinition {
                equipment: "ArtemisIV".into(),
                data: "11".into(),
                modes: vec![],
            },
        )
        .unwrap();
    template
        .sections
        .get_mut(&MechSection::LeftTorso)
        .unwrap()
        .criticals
        .insert(2, jet);
}

/// Artemis V guidance makes Artemis rounds one easier to hit; ordinary Artemis IV does not.
#[tokio::test]
async fn artemis_v_guidance_improves_aim() {
    use stompymux_rs::*;
    for (controller, accuracy) in [("ArtemisIV", 0), ("ArtemisV", -1)] {
        let (_dir, _config, mut world, id, target) = shot_fixture().await;
        let mut template = world.btech.constructed_units()[&id].definition().clone();
        if controller == "ArtemisIV" {
            install_center_artemis(&mut template);
        } else {
            // Artemis V fills two slots beside its launcher (Tactical Operations: Advanced
            // Units & Equipment p. 217), so the SRM-4 moves to the right torso with it.
            let launcher = template
                .sections
                .get_mut(&MechSection::CenterTorso)
                .unwrap()
                .criticals
                .remove(&10)
                .unwrap();
            let torso = template.sections.get_mut(&MechSection::RightTorso).unwrap();
            torso.criticals.insert(7, launcher);
            for slot in [8, 9] {
                torso.criticals.insert(
                    slot,
                    CriticalDefinition {
                        equipment: controller.into(),
                        data: "8".into(),
                        modes: vec![],
                    },
                );
            }
        }
        template
            .sections
            .get_mut(&MechSection::RightTorso)
            .unwrap()
            .criticals
            .get_mut(&0)
            .unwrap()
            .modes = vec!["Artemis/Mine".into()];
        world
            .btech
            .rewrite_unit_record(id, |record| {
                crate::support::templates::set_record_definition(record, template)
            })
            .unwrap();
        let srm = world.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|m| m.weapon == Weapon::Srm4)
            .unwrap();
        let before = battle_aim_modifiers(&world, id, target, srm, 4, shot_rules().aim)
            .unwrap()
            .beacon_accuracy;
        assert_eq!(
            toggle_battle_artemis(&mut world, id, ObjectId(1), srm).unwrap(),
            AmmunitionMode::Artemis
        );
        assert_eq!(
            battle_aim_modifiers(&world, id, target, srm, 4, shot_rules().aim)
                .unwrap()
                .beacon_accuracy,
            before + accuracy,
            "{controller}"
        );
    }
}

/// Native and Lua Artemis controls share mode selection, rollback, firing and persisted expenditure.
#[tokio::test]
async fn artemis_native_lua_ammunition_fire_and_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    let mut template = base.btech.constructed_units()[&id].definition().clone();
    install_center_artemis(&mut template);
    template
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap()
        .modes = vec!["Artemis/Mine".into()];
    base.btech
        .rewrite_unit_record(id, |record| {
            crate::support::templates::set_record_definition(record, template)
        })
        .unwrap();
    shot_skill(&mut base, 30);
    set_battle_character_value(
        &mut base,
        ObjectId(1),
        "Gunnery-Missile",
        CharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    shot_seed(&mut base, id, 1);
    shot_seed(&mut base, target, 1);
    let index = base.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == Weapon::Srm4)
        .unwrap();
    assert_eq!(
        base.btech.constructed_units()[&id]
            .weapon_readiness(index)
            .unwrap()
            .ammunition,
        0
    );
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
    )
    .unwrap();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.artemis({},1,{index}); error('abort')",
            id.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, base.btech);
    let text = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("artemis {index}"),
    );
    assert!(text.contains("Artemis IV compatible"), "{text}");
    assert_eq!(
        lua.eval_callback::<String>(&format!("return btech.unit.artemis({},1,{index})", id.0))
            .unwrap(),
        "artemis"
    );
    assert_eq!(native.world().btech, lua.world().btech);
    let selected = lua.world().clone();
    persistence::save(&config.database(), &selected)
        .await
        .unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, selected.btech);
    let text = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("fire {index} #{}", target.0),
    );
    assert!(text.contains("You fire"), "{text}");
    let result: mlua::Table = lua
        .eval_callback(&format!(
            "return btech.unit.fire({},1,{index},{})",
            id.0, target.0
        ))
        .unwrap();
    assert_eq!(result.get::<usize>("weapon_index").unwrap(), index);
    assert_eq!(native.world().btech, lua.world().btech);
    assert_eq!(
        lua.world().btech.constructed_units()[&id].ammunition()[0],
        selected.btech.constructed_units()[&id].ammunition()[0] - 1
    );
    assert!(
        lua.eval_callback::<()>(&format!("btech.unit.artemis({},1,{index})", id.0))
            .is_err()
    );
    let fired = lua.world().clone();
    persistence::save(&config.database(), &fired).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        fired.btech
    );
}

/// Feed jams survive restart, block controls without expenditure, and clear independently of recycling.
#[tokio::test]
async fn weapon_feed_jam_readiness_controls_and_persistence() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = fixture('.').await;
    let original = world.btech.constructed_units()[&id].clone();
    let index = original
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == Weapon::Srm4)
        .unwrap();
    assert!(original.weapon_readiness(index).unwrap().ready);
    let mut unit = original.clone();
    assert!(unit.jam_weapon(0).is_err()); // The Jenner's first mount is a laser.
    assert!(unit.jam_weapon(999).is_err());
    assert!(unit.jam_weapon(index).unwrap());
    assert!(!unit.jam_weapon(index).unwrap());
    let readiness = unit.weapon_readiness(index).unwrap();
    assert!(readiness.intact && readiness.jammed && !readiness.ready);
    assert_eq!(unit.ammunition(), original.ammunition());
    assert_eq!(unit.mass().unwrap(), original.mass().unwrap());
    world
        .btech
        .rewrite_unit_record(id, |record| {
            *record = serde_json::to_value(&unit).unwrap();
        })
        .unwrap();
    world.validate(&config).unwrap();
    let before = world.btech.clone();
    assert!(spend_battle_weapon(&mut world, id, ObjectId(1), index).is_err());
    assert!(
        toggle_battle_artemis(&mut world, id, ObjectId(1), index)
            .unwrap_err()
            .to_string()
            .contains("jammed")
    );
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    for _ in 0..60 {
        advance_battle_recycle(&mut restored);
    }
    assert!(
        restored.btech.constructed_units()[&id]
            .weapon_jammed(index)
            .unwrap()
    );
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(restored.clone())),
    )
    .unwrap();
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "weapons");
    assert!(text.contains("jammed"), "{text}");
    let error = scripts
        .eval_callback::<()>(&format!("btech.unit.artemis({},1,{index})", id.0))
        .unwrap_err();
    assert!(error.to_string().contains("jammed"), "{error}");
    assert_eq!(scripts.world().btech, restored.btech);
    assert!(unit.clear_weapon_jam(index).unwrap());
    assert!(!unit.clear_weapon_jam(index).unwrap());
    assert_eq!(
        unit.weapon_readiness(index).unwrap(),
        original.weapon_readiness(index).unwrap()
    );
    for bad in [0, 999] {
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["jammed_weapons"] = serde_json::json!([bad]);
            })
            .unwrap();
        assert!(world.validate(&config).is_err());
    }
}

/// Recovery resolves once at sixty seconds and replays from its final saved second.
#[tokio::test]
async fn unjam_native_lua_countdown_outcomes_and_restart() {
    use stompymux_rs::*;
    for outcome in ["success", "failure", "empty", "shutdown"] {
        let (_dir, config, mut base, id) = fixture('.').await;
        let mut unit = base.btech.constructed_units()[&id].clone();
        let index = unit
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|m| m.weapon == Weapon::Srm4)
            .unwrap();
        unit.jam_weapon(index).unwrap();
        base.btech
            .rewrite_unit_record(id, |record| {
                *record = serde_json::to_value(unit).unwrap();
            })
            .unwrap();
        let seed = (0..=255)
            .find(|seed| {
                let roll = Dice::seeded([*seed; 32]).two_d6();
                if outcome == "failure" {
                    roll < 6
                } else {
                    roll >= 6
                }
            })
            .unwrap();
        shot_seed(&mut base, id, seed);
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.unjam({},1,{index}); error('abort')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, base.btech);
        let text = support::run_text(&native, &config, ObjectId(1), 1, &format!("unjam {index}"));
        assert!(text.contains("begin to shake"), "{text}");
        assert!(
            lua.eval_callback::<bool>(&format!("return btech.unit.unjam({},1,{index})", id.0))
                .unwrap()
        );
        assert_eq!(native.world().btech, lua.world().btech);
        let mut world = native.world().clone();
        assert_eq!(
            world.btech.constructed_units()[&id]
                .unjam()
                .unwrap()
                .remaining,
            60
        );
        let before = world.btech.clone();
        assert!(begin_battle_unjam(&mut world, id, ObjectId(1), index).is_err());
        assert!(spend_battle_weapon(&mut world, id, ObjectId(1), 0).is_err());
        assert!(set_battle_speed(&mut world, id, ObjectId(1), 100.0).is_err());
        assert!(launch_battle_jump(&mut world, id, ObjectId(1), 0, 1.0).is_err());
        assert_eq!(world.btech, before);
        for remaining in (1..60).rev() {
            assert!(
                advance_battle_unjamming(&mut world, true)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(
                world.btech.constructed_units()[&id]
                    .unjam()
                    .unwrap()
                    .remaining,
                remaining
            );
        }
        if outcome == "empty" {
            world.btech.set_unit_ammunition_bin(id, 0, 0).unwrap();
        }
        if outcome == "shutdown" {
            stop_battle_unit(&mut world, id, ObjectId(1)).unwrap();
        }
        let supply = world.btech.constructed_units()[&id].ammunition()[0];
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let messages = advance_battle_unjamming(&mut world, true).unwrap();
        assert_eq!(
            advance_battle_unjamming(&mut restored, true).unwrap(),
            messages
        );
        assert_eq!(world.btech, restored.btech);
        let unit = &world.btech.constructed_units()[&id];
        assert!(unit.unjam().is_none());
        assert_eq!(
            unit.weapon_jammed(index).unwrap(),
            matches!(outcome, "failure" | "shutdown")
        );
        assert_eq!(
            unit.ammunition()[0],
            supply - u16::from(outcome == "success")
        );
        assert_eq!(
            messages.len(),
            match outcome {
                "shutdown" => 0,
                "empty" => 1,
                _ => 3,
            }
        );
        if matches!(outcome, "success" | "failure") {
            assert_eq!(messages[0].0, MessageTarget::Player(ObjectId(1)));
            assert!(messages[1].1.starts_with("Modified Gunnery Skill: BTH "));
        }
        assert!(
            advance_battle_unjamming(&mut world, true)
                .unwrap()
                .is_empty()
        );
    }
}

/// Hotload controls, hits and jams use the same transaction and dice paths in both front ends.
#[tokio::test]
async fn hotload_native_lua_shots_jams_and_recovery() {
    use stompymux_rs::*;
    for weapon in [Weapon::Lrm5, Weapon::Elrm5] {
        for low_roll in [false, true] {
            let (_dir, config, mut base, id, target) = shot_fixture().await;
            let mut template = base.btech.constructed_units()[&id].definition().clone();
            template
                .sections
                .get_mut(&MechSection::CenterTorso)
                .unwrap()
                .criticals
                .get_mut(&10)
                .unwrap()
                .equipment = weapon.name().into();
            let bin = template
                .sections
                .get_mut(&MechSection::RightTorso)
                .unwrap()
                .criticals
                .get_mut(&0)
                .unwrap();
            bin.equipment = format!("Ammo_{}", weapon.name());
            bin.data = weapon.profile().ammunition_per_ton.to_string();
            base.btech
                .rewrite_unit_record(id, |record| {
                    crate::support::templates::set_record_definition(record, template);
                    record["ammunition"][0] = weapon.profile().ammunition_per_ton.into();
                })
                .unwrap();
            shot_skill(&mut base, 30);
            set_battle_character_value(
                &mut base,
                ObjectId(1),
                "Gunnery-Missile",
                CharacterValue {
                    value: 30,
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
            let seed = (0..=255)
                .find(|seed| {
                    let roll = Dice::seeded([*seed; 32]).two_d6();
                    (roll <= 3) == low_roll
                })
                .unwrap();
            shot_seed(&mut base, id, seed);
            shot_seed(&mut base, target, 1);
            let index = base.btech.constructed_units()[&id]
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|m| m.weapon == weapon)
                .unwrap();
            let native = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
            )
            .unwrap();
            let lua = Scripts::new(
                &config,
                std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
            )
            .unwrap();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.hotload({},1,{index}); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, base.btech);
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("hotload {index}"),
            );
            assert!(text.contains("toggled on"), "{text}");
            assert_eq!(
                lua.eval_callback::<String>(&format!(
                    "return btech.unit.hotload({},1,{index})",
                    id.0
                ))
                .unwrap(),
                "hotload"
            );
            assert_eq!(lua.world().btech, native.world().btech);
            let selected = lua.world().clone();
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire {index} #{}", target.0),
            );
            // Hot-loaded launchers never jam (Tactical Operations p. 101), even on a low roll.
            assert!(text.contains("You fire"), "{text}");
            assert!(!text.contains("mechanism jams"), "{text}");
            let report: mlua::Table = lua
                .eval_callback(&format!(
                    "return btech.unit.fire({},1,{index},{})",
                    id.0, target.0
                ))
                .unwrap();
            assert!(!report.get::<bool>("jammed").unwrap());
            assert!(report.get::<bool>("launched").unwrap());
            assert_eq!(lua.world().btech, native.world().btech);
            let fired = lua.world().clone();
            let unit = &fired.btech.constructed_units()[&id];
            assert_eq!(
                unit.ammunition()[0],
                selected.btech.constructed_units()[&id].ammunition()[0] - 1
            );
            assert!(!unit.weapon_jammed(index).unwrap());
            assert_eq!(
                unit.weapon_readiness(index).unwrap().recycle_remaining,
                u16::from(weapon.profile().recycle_seconds)
            );
            if let Ok(salvo) = report.get::<mlua::Table>("salvo") {
                let salvo: mlua::Table = salvo.get("report").unwrap();
                let mut dice = Dice::seeded([1; 32]);
                let a = dice.d6();
                let b = dice.d6();
                let c = dice.d6();
                assert_eq!(
                    salvo.get::<u8>("cluster_roll").unwrap(),
                    a + b + c - a.max(b).max(c)
                );
            } else {
                assert!(low_roll);
            }
            persistence::save(&config.database(), &fired).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(restored.btech, fired.btech);
        }
    }
}

/// A hotloaded launcher critical applies internal damage and injures the MechWarrior like any
/// ammunition explosion (Total Warfare p. 41).
#[tokio::test]
async fn hotloaded_launcher_tactical_explosion_replays() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id) = fixture('.').await;
    let mut template = base.btech.constructed_units()[&id].definition().clone();
    let launcher = template
        .sections
        .get_mut(&MechSection::CenterTorso)
        .unwrap()
        .criticals
        .get_mut(&10)
        .unwrap();
    launcher.equipment = "IS.LRM-5".into();
    launcher.modes = vec!["Hotload".into()];
    let bin = template
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap();
    bin.equipment = "Ammo_IS.LRM-5".into();
    bin.data = "24".into();
    base.btech
        .rewrite_unit_record(id, |record| {
            crate::support::templates::set_record_definition(record, template);
            record["ammunition"][0] = 24.into();
            record["fire_modes"] = serde_json::json!({"4":"hotload"});
        })
        .unwrap();
    let mut found = false;
    for seed in 0..=255 {
        shot_seed(&mut base, id, seed);
        let mut world = base.clone();
        let report = resolve_battle_tactical_impact(
            &mut world,
            id,
            balance_hit(MechSection::CenterTorso, true),
            1,
            fall_rules(),
        )
        .unwrap();
        if !report
            .notices
            .iter()
            .any(|notice| notice.text.contains("hotloaded launcher explodes"))
        {
            continue;
        }
        assert!(
            report
                .notices
                .iter()
                .any(|notice| notice.text.contains("personal injury from the weapon"))
        );
        assert!(
            world.btech.constructed_units()[&id].sections()[&MechSection::CenterTorso].internal
                <= base.btech.constructed_units()[&id].sections()[&MechSection::CenterTorso]
                    .internal
                    - 5
        );
        persistence::save(&config.database(), &base).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            resolve_battle_tactical_impact(
                &mut restored,
                id,
                balance_hit(MechSection::CenterTorso, true),
                1,
                fall_rules()
            )
            .unwrap(),
            report
        );
        assert_eq!(restored.btech, world.btech);
        found = true;
        break;
    }
    assert!(found, "No seed selected the launcher critical");
}

/// Recovery skips deletion-pending units and rejects bad live timers without partially advancing other units.
#[tokio::test]
async fn unjam_deletion_corruption_and_weapon_loss_are_atomic() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, other) = shot_fixture().await;
    assert!(id < other);
    let index = base.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == Weapon::Srm4)
        .unwrap();
    let mut state = serde_json::to_value(&base.btech).unwrap();
    for (unit, remaining) in [(id, 1), (other, 2)] {
        state["constructed"][unit.0.to_string()]["jammed_weapons"] = serde_json::json!([index]);
        state["constructed"][unit.0.to_string()]["unjam"] =
            serde_json::json!({"weapon_index": index, "remaining": remaining});
    }
    base.btech = serde_json::from_value(state).unwrap();
    base.validate(&config).unwrap();
    let mut deleting = base.clone();
    deleting
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::Going);
    let before = deleting.btech.constructed_units()[&id].clone();
    assert!(
        advance_battle_unjamming(&mut deleting, true)
            .unwrap()
            .is_empty()
    );
    assert_eq!(deleting.btech.constructed_units()[&id], before);
    assert_eq!(
        deleting.btech.constructed_units()[&other]
            .unjam()
            .unwrap()
            .remaining,
        1
    );

    for (weapon_index, remaining) in [(index, 0), (index, 61), (999, 1), (0, 1)] {
        let mut invalid = base.clone();
        invalid
            .btech
            .rewrite_unit_record(other, |record| {
                record["unjam"] =
                    serde_json::json!({"weapon_index":weapon_index,"remaining":remaining});
            })
            .unwrap();
        let before = invalid.btech.clone();
        assert!(invalid.validate(&config).is_err());
        assert!(advance_battle_unjamming(&mut invalid, true).is_err());
        assert_eq!(invalid.btech, before);
    }

    let mut lost = base.clone();
    destroy_battle_critical(
        &mut lost,
        id,
        CriticalLocation {
            section: MechSection::CenterTorso,
            slot: 10,
        },
    )
    .unwrap();
    let supply = lost.btech.constructed_units()[&id].ammunition().to_vec();
    let dice = serde_json::to_value(&lost.btech.constructed_units()[&id]).unwrap()["dice"].clone();
    persistence::save(&config.database(), &lost).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert!(
        advance_battle_unjamming(&mut lost, true)
            .unwrap()
            .is_empty()
    );
    assert!(
        advance_battle_unjamming(&mut restored, true)
            .unwrap()
            .is_empty()
    );
    assert_eq!(lost.btech, restored.btech);
    let unit = &lost.btech.constructed_units()[&id];
    assert!(unit.unjam().is_none());
    assert_eq!(unit.ammunition(), supply);
    assert_eq!(serde_json::to_value(unit).unwrap()["dice"], dice);
}

/// Ultra fire shares native/Lua transactions, uses both bins, and replays permanent failures and fallback.
async fn ultra_double_shots_matrix(weapons: &[stompymux_rs::Weapon]) {
    use stompymux_rs::*;
    let (_dir, config, mut pristine, id, target) = shot_fixture().await;
    shot_seed(&mut pristine, target, 1);
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let pristine_db = snapshot_database(&config);
    let mut probed_restart = false;
    let mut probed_fire = false;
    for weapon in weapons.iter().copied() {
        // Supply and attack roll distinguish cross-bin firing, same-bin firing, permanent failure,
        // single-round fallback (even on a two), and an ordinary miss that still spends two rounds.
        for (supply, attack, skill) in [
            ([1, 1], 8, 30),
            ([2, 0], 8, 30),
            ([1, 1], 2, 30),
            ([1, 0], 2, 30),
            ([1, 1], 3, 0),
        ] {
            restore_database(&config, &pristine_db);
            let mut base = pristine.clone();
            shot_skill(&mut base, skill);
            set_battle_character_value(
                &mut base,
                ObjectId(1),
                "Gunnery-Ballistic",
                CharacterValue {
                    value: skill,
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
            let mut definition = base.btech.constructed_units()[&id].definition().clone();
            let mut part = definition.sections[&MechSection::LeftArm].criticals[&2].clone();
            part.equipment = weapon.name().into();
            part.modes = vec!["UltraMode".into()];
            for slot in 2..2 + weapon.profile().critical_slots {
                definition
                    .sections
                    .get_mut(&MechSection::LeftArm)
                    .unwrap()
                    .criticals
                    .insert(slot, part.clone());
            }
            let mut bin = definition.sections[&MechSection::RightTorso].criticals[&0].clone();
            bin.equipment = format!("Ammo_{}", weapon.name());
            bin.data = weapon.profile().ammunition_per_ton.to_string();
            definition
                .sections
                .get_mut(&MechSection::RightTorso)
                .unwrap()
                .criticals
                .insert(0, bin.clone());
            definition
                .sections
                .get_mut(&MechSection::RightTorso)
                .unwrap()
                .criticals
                .insert(4, bin);
            // Factory flags initialize Ultra, while live ammunition is deliberately almost depleted.
            let factory =
                Mech::from_template(crate::support::templates::legal(definition.clone())).unwrap();
            let index = factory
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|mount| mount.weapon == weapon)
                .unwrap();
            assert_eq!(factory.fire_mode(index).unwrap(), FireMode::Ultra);
            base.btech
                .rewrite_unit_record(id, |record| {
                    crate::support::templates::set_record_definition(record, definition);
                    record["ammunition"] = serde_json::json!(supply);
                })
                .unwrap();
            base.validate(&config).unwrap();
            let seed = (0..=255)
                .find(|seed| Dice::seeded([*seed; 32]).two_d6() == attack)
                .unwrap();
            shot_seed(&mut base, id, seed);
            install(&native, base.clone());
            install(&lua, base.clone());
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.ultra({},1,{index}); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, base.btech);
            let feedback =
                support::run_text(&native, &config, ObjectId(1), 1, &format!("ultra {index}"));
            assert!(feedback.contains("ultra fire mode"), "{feedback}");
            let mode: String = lua
                .eval_callback(&format!("return btech.unit.ultra({},1,{index})", id.0))
                .unwrap();
            assert_eq!(mode, "ultra");
            assert_eq!(native.world().btech, lua.world().btech);
            let before = native.world().clone();
            // Restart probe runs once per shard; every scenario keeps the resolve assertions.
            let restored = if probed_restart {
                None
            } else {
                probed_restart = true;
                persistence::save(&config.database(), &before)
                    .await
                    .unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                restored
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
                Some(restored)
            };
            let mut expected = before.clone();
            let report = resolve_battle_shot(
                &mut expected,
                id,
                ObjectId(1),
                target,
                index,
                configured_shot_rules(&config),
            )
            .unwrap();
            if let Some(mut restored) = restored {
                assert_eq!(
                    resolve_battle_shot(
                        &mut restored,
                        id,
                        ObjectId(1),
                        target,
                        index,
                        configured_shot_rules(&config)
                    )
                    .unwrap(),
                    report
                );
                assert_eq!(restored.btech, expected.btech);
            }
            let double = supply.iter().sum::<u16>() == 2;
            // A double shot on a 2 fires both shells, then the feed is useless for the rest of
            // the game (Total Warfare p. 114): the mount stays intact but cannot be cleared.
            let broken = double && attack == 2;
            assert!(!report.loader_destroyed);
            assert_eq!(report.jammed, broken);
            assert_eq!(report.feed_broken, broken);
            assert!(report.launched);
            assert_eq!(
                report.expenditure.fire_mode,
                if double {
                    FireMode::Ultra
                } else {
                    FireMode::Normal
                }
            );
            let spent = if double { 2 } else { 1 };
            assert_eq!(
                report
                    .expenditure
                    .ammunition
                    .iter()
                    .map(|draw| draw.rounds)
                    .sum::<u16>(),
                spent
            );
            assert_eq!(report.expenditure.heat, weapon.profile().heat * spent as u8);
            let unit = &expected.btech.constructed_units()[&id];
            assert_eq!(
                unit.ammunition().iter().sum::<u16>(),
                supply.iter().sum::<u16>() - spent
            );
            assert!(unit.weapon_intact(index).unwrap());
            assert_eq!(unit.weapon_jammed(index).unwrap(), broken);
            assert_eq!(unit.weapon_feed_broken(index).unwrap(), broken);
            assert_eq!(
                unit.weapon_recycle()[&index],
                u16::from(weapon.profile().recycle_seconds)
            );
            assert_eq!(unit.fire_mode(index).unwrap(), report.expenditure.fire_mode);
            if skill == 30 {
                let salvo = report.salvo.as_ref().unwrap().as_mech().unwrap();
                assert_eq!(salvo.cluster_roll.is_some(), double);
                assert!(
                    salvo
                        .groups
                        .iter()
                        .all(|group| group.damage == u16::from(weapon.profile().damage))
                );
            } else {
                assert!(report.salvo.is_none());
            }
            if broken {
                let mut later = expected.clone();
                for _ in 0..130 {
                    advance_battle_recycle(&mut later);
                }
                assert_eq!(
                    begin_battle_unjam(&mut later, id, ObjectId(1), index)
                        .unwrap_err()
                        .to_string(),
                    "The ammo feed on that weapon is broken beyond field repair."
                );
                assert!(
                    resolve_battle_shot(
                        &mut later,
                        id,
                        ObjectId(1),
                        target,
                        index,
                        configured_shot_rules(&config)
                    )
                    .is_err()
                );
            }
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.fire({},1,{index},{}); error('abort')",
                    id.0, target.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before.btech);
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire {index} #{}", target.0),
            );
            assert!(text.contains("You fire"), "{text}");
            assert_eq!(text.contains("useless until repaired"), broken, "{text}");
            lua.eval_callback::<mlua::Table>(&format!(
                "return btech.unit.fire({},1,{index},{})",
                id.0, target.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            assert_eq!(native.world().btech, expected.btech);
            if !probed_fire {
                probed_fire = true;
                persistence::save(&config.database(), &expected)
                    .await
                    .unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    expected.btech
                );
            }
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn ultra_double_shots_supply_loader_failure_and_restart_clan_ultra_ac2() {
    use stompymux_rs::Weapon;
    ultra_double_shots_matrix(&[Weapon::ClanUltraAc2]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn ultra_double_shots_supply_loader_failure_and_restart_clan_ultra_ac5() {
    use stompymux_rs::Weapon;
    ultra_double_shots_matrix(&[Weapon::ClanUltraAc5]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn ultra_double_shots_supply_loader_failure_and_restart_clan_ultra_ac10() {
    use stompymux_rs::Weapon;
    ultra_double_shots_matrix(&[Weapon::ClanUltraAc10]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn ultra_double_shots_supply_loader_failure_and_restart_clan_ultra_ac20() {
    use stompymux_rs::Weapon;
    ultra_double_shots_matrix(&[Weapon::ClanUltraAc20]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn ultra_double_shots_supply_loader_failure_and_restart_ultra_ac2() {
    use stompymux_rs::Weapon;
    ultra_double_shots_matrix(&[Weapon::UltraAc2]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn ultra_double_shots_supply_loader_failure_and_restart_ultra_ac5() {
    use stompymux_rs::Weapon;
    ultra_double_shots_matrix(&[Weapon::UltraAc5]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn ultra_double_shots_supply_loader_failure_and_restart_ultra_ac10() {
    use stompymux_rs::Weapon;
    ultra_double_shots_matrix(&[Weapon::UltraAc10]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn ultra_double_shots_supply_loader_failure_and_restart_ultra_ac20() {
    use stompymux_rs::Weapon;
    ultra_double_shots_matrix(&[Weapon::UltraAc20]).await;
}

/// Rapid fire shares native/Lua transactions, uses both bins, and replays permanent failures and fallback.
async fn rapid_double_shots_matrix(weapons: &[stompymux_rs::Weapon]) {
    use stompymux_rs::*;
    let (_dir, config, mut pristine, id, target) = shot_fixture().await;
    shot_seed(&mut pristine, target, 1);
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let pristine_db = snapshot_database(&config);
    let mut probed_restart = false;
    let mut probed_fire = false;
    let mut probed_recovery = false;
    for weapon in weapons.iter().copied() {
        // Supply and attack roll distinguish cross-bin firing, same-bin firing, permanent failure,
        // single-round fallback (even on a two), and an ordinary miss that still spends two rounds.
        for (supply, attack, skill) in [
            ([1, 1], 8, 30),
            ([2, 0], 8, 30),
            ([1, 1], 2, 30),
            ([1, 0], 2, 30),
            ([1, 1], 5, 0),
            ([1, 1], 3, 30),
            ([1, 1], 4, 30),
        ] {
            restore_database(&config, &pristine_db);
            let mut base = pristine.clone();
            shot_skill(&mut base, skill);
            set_battle_character_value(
                &mut base,
                ObjectId(1),
                "Gunnery-Ballistic",
                CharacterValue {
                    value: skill,
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
            let mut definition = base.btech.constructed_units()[&id].definition().clone();
            let mut part = definition.sections[&MechSection::LeftArm].criticals[&2].clone();
            part.equipment = weapon.name().into();
            part.modes = vec!["RapidFire".into()];
            for slot in 2..2 + weapon.profile().critical_slots {
                definition
                    .sections
                    .get_mut(&MechSection::LeftArm)
                    .unwrap()
                    .criticals
                    .insert(slot, part.clone());
            }
            let mut bin = definition.sections[&MechSection::RightTorso].criticals[&0].clone();
            bin.equipment = format!("Ammo_{}", weapon.name());
            bin.data = weapon.profile().ammunition_per_ton.to_string();
            definition
                .sections
                .get_mut(&MechSection::RightTorso)
                .unwrap()
                .criticals
                .insert(0, bin.clone());
            definition
                .sections
                .get_mut(&MechSection::RightTorso)
                .unwrap()
                .criticals
                .insert(4, bin);
            // Factory flags initialize Rapid, while live ammunition is deliberately almost depleted.
            let factory =
                Mech::from_template(crate::support::templates::legal(definition.clone())).unwrap();
            let index = factory
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|mount| mount.weapon == weapon)
                .unwrap();
            assert_eq!(factory.fire_mode(index).unwrap(), FireMode::Rapid);
            base.btech
                .rewrite_unit_record(id, |record| {
                    crate::support::templates::set_record_definition(record, definition);
                    record["ammunition"] = serde_json::json!(supply);
                })
                .unwrap();
            base.validate(&config).unwrap();
            let seed = (0..=255)
                .find(|seed| Dice::seeded([*seed; 32]).two_d6() == attack)
                .unwrap();
            shot_seed(&mut base, id, seed);
            install(&native, base.clone());
            install(&lua, base.clone());
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.rapidfire({},1,{index}); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, base.btech);
            let feedback = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("rapidfire {index}"),
            );
            assert!(feedback.contains("Rapid Fire mode"), "{feedback}");
            let mode: String = lua
                .eval_callback(&format!("return btech.unit.rapidfire({},1,{index})", id.0))
                .unwrap();
            assert_eq!(mode, "rapid");
            assert_eq!(native.world().btech, lua.world().btech);
            let before = native.world().clone();
            // Restart probe runs once per shard; every scenario keeps the resolve assertions.
            let restored = if probed_restart {
                None
            } else {
                probed_restart = true;
                persistence::save(&config.database(), &before)
                    .await
                    .unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                restored
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
                Some(restored)
            };
            let mut expected = before.clone();
            let report = resolve_battle_shot(
                &mut expected,
                id,
                ObjectId(1),
                target,
                index,
                configured_shot_rules(&config),
            )
            .unwrap();
            if let Some(mut restored) = restored {
                assert_eq!(
                    resolve_battle_shot(
                        &mut restored,
                        id,
                        ObjectId(1),
                        target,
                        index,
                        configured_shot_rules(&config)
                    )
                    .unwrap(),
                    report
                );
                assert_eq!(restored.btech, expected.btech);
            }
            let double = supply.iter().sum::<u16>() == 2;
            // The burst is always fired; a 2 then explodes the rounds in the barrel and a 3 or
            // 4 jams the feed (Tactical Operations p. 98).
            let destroyed = double && attack == 2;
            assert_eq!(report.loader_destroyed, destroyed);
            let jammed = double && (3..=4).contains(&attack);
            assert_eq!(report.jammed, jammed);
            assert!(!report.feed_broken);
            assert_eq!(report.misload.is_some(), destroyed);
            assert!(report.launched);
            assert_eq!(
                report.expenditure.fire_mode,
                if double {
                    FireMode::Rapid
                } else {
                    FireMode::Normal
                }
            );
            let spent = if double { 2 } else { 1 };
            assert_eq!(
                report
                    .expenditure
                    .ammunition
                    .iter()
                    .map(|draw| draw.rounds)
                    .sum::<u16>(),
                spent
            );
            assert_eq!(report.expenditure.heat, weapon.profile().heat * spent as u8);
            let unit = &expected.btech.constructed_units()[&id];
            assert_eq!(
                unit.ammunition().iter().sum::<u16>(),
                supply.iter().sum::<u16>() - spent
            );
            assert_eq!(unit.weapon_intact(index).unwrap(), !destroyed);
            if skill == 30 {
                let salvo = report.salvo.as_ref().unwrap().as_mech().unwrap();
                assert_eq!(salvo.cluster_roll.is_some(), double);
                assert!(
                    salvo
                        .groups
                        .iter()
                        .all(|group| group.damage == u16::from(weapon.profile().damage))
                );
            } else {
                assert!(report.salvo.is_none());
            }
            if destroyed {
                let explosion = report.misload.as_ref().unwrap();
                assert!(!explosion.impact.phases.is_empty());
                let before_unit = &before.btech.constructed_units()[&id];
                assert_eq!(
                    unit.sections()[&MechSection::LeftArm].armor,
                    if unit.sections()[&MechSection::LeftArm].internal == 0 {
                        0 // Losing the complete arm also removes its armor.
                    } else {
                        before_unit.sections()[&MechSection::LeftArm].armor
                    }
                );
                assert_eq!(
                    unit.sections()[&MechSection::LeftTorso],
                    before_unit.sections()[&MechSection::LeftTorso]
                );
                assert_eq!(
                    unit.sections()[&MechSection::CenterTorso],
                    before_unit.sections()[&MechSection::CenterTorso]
                );
                assert!(
                    unit.sections()[&MechSection::LeftArm].internal
                        < before_unit.sections()[&MechSection::LeftArm].internal
                );
                assert!(
                    unit.loadout().unwrap().weapons[index]
                        .criticals
                        .iter()
                        .all(|slot| unit.lost_criticals().contains(slot))
                );
                assert!(unit.weapon_recycle().is_empty());
                assert!(begin_battle_unjam(&mut expected, id, ObjectId(1), index).is_err());
            } else {
                assert_eq!(
                    unit.weapon_recycle()[&index],
                    u16::from(weapon.profile().recycle_seconds)
                );
                assert_eq!(unit.fire_mode(index).unwrap(), report.expenditure.fire_mode);
                assert_eq!(unit.weapon_jammed(index).unwrap(), jammed);
            }
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.fire({},1,{index},{}); error('abort')",
                    id.0, target.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before.btech);
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("fire {index} #{}", target.0),
            );
            assert!(text.contains("You fire"), "{text}");
            assert_eq!(text.contains("explode in the barrel"), destroyed, "{text}");
            assert_eq!(
                text.contains("ammo loader mechanism jams"),
                jammed,
                "{text}"
            );
            lua.eval_callback::<mlua::Table>(&format!(
                "return btech.unit.fire({},1,{index},{})",
                id.0, target.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
            assert_eq!(native.world().btech, expected.btech);
            if !probed_fire {
                probed_fire = true;
                persistence::save(&config.database(), &expected)
                    .await
                    .unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    expected.btech
                );
            }
            if jammed {
                let frozen = expected.btech.clone();
                assert!(toggle_battle_rapid(&mut expected, id, ObjectId(1), index).is_err());
                assert_eq!(expected.btech, frozen);
                // A jam survives restart and is recoverable using the ordinary feed procedure;
                // the recovery probe runs once per shard with its own saved baseline.
                if !probed_recovery {
                    probed_recovery = true;
                    persistence::save(&config.database(), &expected)
                        .await
                        .unwrap();
                    let mut recovery = persistence::load(&config.database()).await.unwrap();
                    let seed = (0..=255)
                        .find(|seed| Dice::seeded([*seed; 32]).two_d6() >= 6)
                        .unwrap();
                    shot_seed(&mut recovery, id, seed);
                    // The jammed burst still fired, so its recycle clock must run down first.
                    for _ in 0..130 {
                        advance_battle_recycle(&mut recovery);
                    }
                    begin_battle_unjam(&mut recovery, id, ObjectId(1), index).unwrap();
                    for _ in 0..60 {
                        advance_battle_unjamming(&mut recovery, true).unwrap();
                    }
                    assert!(
                        !recovery.btech.constructed_units()[&id]
                            .weapon_jammed(index)
                            .unwrap()
                    );
                    assert_eq!(
                        recovery.btech.constructed_units()[&id]
                            .ammunition()
                            .iter()
                            .sum::<u16>(),
                        0
                    );
                    assert_eq!(
                        recovery.btech.constructed_units()[&id]
                            .fire_mode(index)
                            .unwrap(),
                        FireMode::Rapid
                    );
                }
            }
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn rapid_double_shots_supply_loader_failure_and_restart_ac2() {
    use stompymux_rs::Weapon;
    rapid_double_shots_matrix(&[Weapon::Ac2]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn rapid_double_shots_supply_loader_failure_and_restart_ac5() {
    use stompymux_rs::Weapon;
    rapid_double_shots_matrix(&[Weapon::Ac5]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn rapid_double_shots_supply_loader_failure_and_restart_ac10() {
    use stompymux_rs::Weapon;
    rapid_double_shots_matrix(&[Weapon::Ac10]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn rapid_double_shots_supply_loader_failure_and_restart_ac20() {
    use stompymux_rs::Weapon;
    rapid_double_shots_matrix(&[Weapon::Ac20]).await;
}

/// Rotary feed recovery uses gunnery plus three even while prone, with exact saved dice and skill selection.
async fn rotary_unjam_matrix(weapons: &[stompymux_rs::Weapon]) {
    use stompymux_rs::*;
    let (_dir, config, pristine, _id, _unused) = shot_fixture().await;
    let pristine_db = snapshot_database(&config);
    let mut probed = false;
    for weapon in weapons.iter().copied() {
        restore_database(&config, &pristine_db);
        let (mut base, id, _) = (pristine.clone(), _id, _unused);
        shot_skill(&mut base, 0);
        for (skill, value) in [("Gunnery-Ballistic", 6), ("Gunnery-Battlemech", 2)] {
            set_battle_character_value(
                &mut base,
                ObjectId(1),
                skill,
                CharacterValue {
                    value,
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
        }
        let mut definition = base.btech.constructed_units()[&id].definition().clone();
        if weapon.name().starts_with("CL.") {
            definition
                .attributes
                .insert("specials".into(), "Clan FlipArms".into());
            definition.heat_sinks = 10;
            for section in definition.sections.values_mut() {
                section
                    .criticals
                    .retain(|_, part| part.equipment != "HeatSink");
            }
        }

        let mut part = definition.sections[&MechSection::LeftArm].criticals[&2].clone();
        part.equipment = weapon.name().into();
        for slot in 2..2 + weapon.profile().critical_slots {
            definition
                .sections
                .get_mut(&MechSection::LeftArm)
                .unwrap()
                .criticals
                .insert(slot, part.clone());
        }
        let bin = definition
            .sections
            .get_mut(&MechSection::RightTorso)
            .unwrap()
            .criticals
            .get_mut(&0)
            .unwrap();
        bin.equipment = format!("Ammo_{}", weapon.name());
        bin.data = weapon.profile().ammunition_per_ton.to_string();
        base.btech
            .rewrite_unit_record(id, |record| {
                crate::support::templates::set_record_definition(record, definition);
                record["ammunition"] = serde_json::json!([2]);
                record["posture"] = "prone".into();
            })
            .unwrap();
        let mut unit = base.btech.constructed_units()[&id].clone();
        let index = unit
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == weapon)
            .unwrap();
        unit.jam_weapon(index).unwrap();
        base.btech
            .rewrite_unit_record(id, |record| {
                *record = serde_json::to_value(unit).unwrap();
            })
            .unwrap();
        base.validate(&config).unwrap();
        for extended in [false, true] {
            for connected in [false, true] {
                for success in [false, true] {
                    let mut world = base.clone();
                    if connected {
                        world
                            .objects
                            .get_mut(&ObjectId(1))
                            .unwrap()
                            .flags
                            .insert(Flag::Connected);
                    } else {
                        world
                            .objects
                            .get_mut(&ObjectId(1))
                            .unwrap()
                            .flags
                            .remove(Flag::Connected);
                    }
                    let target = if !connected {
                        9
                    } else if extended {
                        8
                    } else {
                        12
                    };
                    let roll = target - u8::from(!success);
                    let seed = (0..=255)
                        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == roll)
                        .unwrap();
                    shot_seed(&mut world, id, seed);
                    let dice_before = serde_json::to_value(&world.btech.constructed_units()[&id])
                        .unwrap()["dice"]
                        .clone();
                    let skills_before = world.btech.character_values().clone();
                    begin_battle_unjam(&mut world, id, ObjectId(1), index).unwrap();
                    for _ in 0..59 {
                        assert!(
                            advance_battle_unjamming(&mut world, extended)
                                .unwrap()
                                .is_empty()
                        );
                    }
                    assert_eq!(
                        serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
                        dice_before
                    );
                    // Restart probe runs once per shard, covering save/load replay and recycle parity.
                    if !probed {
                        probed = true;
                        persistence::save(&config.database(), &world).await.unwrap();
                        let mut restored = persistence::load(&config.database()).await.unwrap();
                        if connected {
                            restored
                                .objects
                                .get_mut(&ObjectId(1))
                                .unwrap()
                                .flags
                                .insert(Flag::Connected);
                        } else {
                            restored
                                .objects
                                .get_mut(&ObjectId(1))
                                .unwrap()
                                .flags
                                .remove(Flag::Connected);
                        }
                        let mut expected_dice = world.clone();
                        assert_eq!(
                            roll_unit_dice(&mut expected_dice, id, 2)
                                .unwrap()
                                .iter()
                                .sum::<u8>(),
                            roll
                        );
                        let messages = advance_battle_unjamming(&mut world, extended).unwrap();
                        assert_eq!(
                            advance_battle_unjamming(&mut restored, extended).unwrap(),
                            messages
                        );
                        assert_eq!(restored.btech, world.btech);
                        assert_eq!(messages.len(), 3);
                        assert_eq!(
                            messages[0],
                            (
                                MessageTarget::Player(ObjectId(1)),
                                "You make a roll to unjam the weapon!".into()
                            )
                        );
                        assert!(messages[1].1.starts_with("Modified Gunnery Skill: BTH "));
                        assert!(messages[2].1.contains(if success {
                            "manage to clear"
                        } else {
                            "attempt to remove"
                        }));
                        let unit = &world.btech.constructed_units()[&id];
                        assert_eq!(unit.weapon_jammed(index).unwrap(), !success);
                        assert!(unit.unjam().is_none());
                        assert_eq!(unit.ammunition(), &[if success { 1 } else { 2 }]);
                        assert_eq!(
                            serde_json::to_value(unit).unwrap()["dice"],
                            serde_json::to_value(&expected_dice.btech.constructed_units()[&id])
                                .unwrap()["dice"]
                        );
                        assert_eq!(world.btech.character_values(), &skills_before);
                    }
                }
            }
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn rotary_unjam_gunnery_thresholds_and_restart_rotaryac2() {
    use stompymux_rs::Weapon;
    rotary_unjam_matrix(&[Weapon::RotaryAc2]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn rotary_unjam_gunnery_thresholds_and_restart_rotaryac5() {
    use stompymux_rs::Weapon;
    rotary_unjam_matrix(&[Weapon::RotaryAc5]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn rotary_unjam_gunnery_thresholds_and_restart_clanrotaryac2() {
    use stompymux_rs::Weapon;
    rotary_unjam_matrix(&[Weapon::ClanRotaryAc2]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn rotary_unjam_gunnery_thresholds_and_restart_clanrotaryac5() {
    use stompymux_rs::Weapon;
    rotary_unjam_matrix(&[Weapon::ClanRotaryAc5]).await;
}

/// Rotary burst controls and firing agree across native/Lua, supply fallback, jams and restart.
async fn rotary_burst_matrix(weapons: &[stompymux_rs::Weapon]) {
    use stompymux_rs::*;
    let (_dir, config, mut pristine, id, target) = shot_fixture().await;
    shot_seed(&mut pristine, target, 1);
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let pristine_db = snapshot_database(&config);
    let mut probed_restart = false;
    let mut probed_fire = false;
    for weapon in weapons.iter().copied() {
        for (rounds, mode, flag, jam_limit) in [
            (2_u8, FireMode::Rotary2, "Rotary_TwoShot", 2_u8),
            (3, FireMode::Rotary3, "Rotary_ThreeShot", 2),
            (4, FireMode::Rotary4, "Rotary_FourShot", 3),
            (5, FireMode::Rotary5, "Rotary_FiveShot", 3),
            (6, FireMode::Rotary6, "Rotary_SixShot", 4),
        ] {
            for (supply, attack, skill) in [
                (rounds, jam_limit, 30),
                (rounds, jam_limit + 1, 30),
                (rounds - 1, 2, 30),
                (rounds, jam_limit + 1, 0),
                (0, 8, 30),
            ] {
                restore_database(&config, &pristine_db);
                let mut base = pristine.clone();
                shot_skill(&mut base, skill);
                set_battle_character_value(
                    &mut base,
                    ObjectId(1),
                    "Gunnery-Ballistic",
                    CharacterValue {
                        value: skill,
                        experience: 0,
                        last_used: 0,
                    },
                )
                .unwrap();
                let mut definition = base.btech.constructed_units()[&id].definition().clone();
                if weapon.name().starts_with("CL.") {
                    definition
                        .attributes
                        .insert("specials".into(), "Clan FlipArms".into());
                    definition.heat_sinks = 10;
                    for section in definition.sections.values_mut() {
                        section
                            .criticals
                            .retain(|_, part| part.equipment != "HeatSink");
                    }
                }

                let mut part = definition.sections[&MechSection::LeftArm].criticals[&2].clone();
                part.equipment = weapon.name().into();
                part.modes = vec![flag.into()];
                for slot in 2..2 + weapon.profile().critical_slots {
                    definition
                        .sections
                        .get_mut(&MechSection::LeftArm)
                        .unwrap()
                        .criticals
                        .insert(slot, part.clone());
                }
                let mut bin = definition.sections[&MechSection::RightTorso].criticals[&0].clone();
                bin.equipment = format!("Ammo_{}", weapon.name());
                bin.data = weapon.profile().ammunition_per_ton.to_string();
                definition
                    .sections
                    .get_mut(&MechSection::RightTorso)
                    .unwrap()
                    .criticals
                    .insert(0, bin.clone());
                definition
                    .sections
                    .get_mut(&MechSection::RightTorso)
                    .unwrap()
                    .criticals
                    .insert(4, bin);
                let factory =
                    Mech::from_template(crate::support::templates::legal(definition.clone()))
                        .unwrap();
                let index = factory
                    .loadout()
                    .unwrap()
                    .weapons
                    .iter()
                    .position(|mount| mount.weapon == weapon)
                    .unwrap();
                assert_eq!(factory.fire_mode(index).unwrap(), mode);
                base.btech
                    .rewrite_unit_record(id, |record| {
                        crate::support::templates::set_record_definition(record, definition);
                        record["ammunition"] =
                            serde_json::json!([supply.min(1), supply.saturating_sub(1)]);
                    })
                    .unwrap();
                base.validate(&config).unwrap();
                let seed = (0..=255)
                    .find(|seed| Dice::seeded([*seed; 32]).two_d6() == attack)
                    .unwrap();
                shot_seed(&mut base, id, seed);
                install(&native, base.clone());
                install(&lua, base.clone());
                assert!(
                    lua.eval_callback::<()>(&format!(
                        "btech.unit.rac({},1,{index},{rounds}); error('abort')",
                        id.0
                    ))
                    .is_err()
                );
                assert_eq!(lua.world().btech, base.btech);
                for repeated in [false, true] {
                    let text = support::run_text(
                        &native,
                        &config,
                        ObjectId(1),
                        1,
                        &format!("rac {index} {rounds}"),
                    );
                    assert!(
                        text.contains(if repeated {
                            "already set"
                        } else {
                            "has been set"
                        }),
                        "{text}"
                    );
                    assert_eq!(
                        lua.eval_callback::<bool>(&format!(
                            "return btech.unit.rac({},1,{index},{rounds})",
                            id.0
                        ))
                        .unwrap(),
                        !repeated
                    );
                    assert_eq!(native.world().btech, lua.world().btech);
                }
                let before = native.world().clone();
                let mut expected = before.clone();
                if supply == 0 {
                    assert!(
                        resolve_battle_shot(
                            &mut expected,
                            id,
                            ObjectId(1),
                            target,
                            index,
                            configured_shot_rules(&config)
                        )
                        .is_err()
                    );
                    assert_eq!(expected.btech, before.btech);
                    continue;
                }
                // Restart probe runs once per shard; every scenario keeps the resolve assertions.
                let restored = if probed_restart {
                    None
                } else {
                    probed_restart = true;
                    persistence::save(&config.database(), &before)
                        .await
                        .unwrap();
                    let mut restored = persistence::load(&config.database()).await.unwrap();
                    restored
                        .objects
                        .get_mut(&ObjectId(1))
                        .unwrap()
                        .flags
                        .insert(Flag::Connected);
                    Some(restored)
                };
                let report = resolve_battle_shot(
                    &mut expected,
                    id,
                    ObjectId(1),
                    target,
                    index,
                    configured_shot_rules(&config),
                )
                .unwrap();
                if let Some(mut restored) = restored {
                    assert_eq!(
                        resolve_battle_shot(
                            &mut restored,
                            id,
                            ObjectId(1),
                            target,
                            index,
                            configured_shot_rules(&config)
                        )
                        .unwrap(),
                        report
                    );
                    assert_eq!(restored.btech, expected.btech);
                }
                let burst = supply >= rounds;
                // The burst that jams is still fired (Total Warfare p. 114).
                let jammed = burst && attack <= jam_limit;
                assert_eq!(report.jammed, jammed);
                assert!(report.launched);
                assert!(!report.loader_destroyed && !report.feed_broken);
                assert!(report.misload.is_none());
                assert_eq!(
                    report.expenditure.fire_mode,
                    if burst { mode } else { FireMode::Normal }
                );
                let spent = if burst { rounds } else { 1 };
                assert_eq!(
                    report
                        .expenditure
                        .ammunition
                        .iter()
                        .map(|draw| draw.rounds)
                        .sum::<u16>(),
                    u16::from(spent)
                );
                assert_eq!(report.expenditure.heat, spent * weapon.profile().heat);
                let unit = &expected.btech.constructed_units()[&id];
                assert!(unit.weapon_intact(index).unwrap());
                assert_eq!(unit.weapon_jammed(index).unwrap(), jammed);
                assert_eq!(
                    unit.ammunition().iter().sum::<u16>(),
                    u16::from(supply - spent)
                );
                assert_eq!(unit.fire_mode(index).unwrap(), report.expenditure.fire_mode);
                assert_eq!(
                    unit.weapon_recycle()[&index],
                    u16::from(weapon.profile().recycle_seconds)
                );
                assert_eq!(report.salvo.is_some(), skill == 30);
                if let Some(stompymux_rs::TargetSalvo::Mech(salvo)) = &report.salvo {
                    assert_eq!(salvo.cluster_roll.is_some(), burst);
                    assert!(
                        salvo
                            .groups
                            .iter()
                            .all(|group| group.damage == u16::from(weapon.profile().damage))
                    );
                }
                assert!(
                    lua.eval_callback::<()>(&format!(
                        "btech.unit.fire({},1,{index},{}); error('abort')",
                        id.0, target.0
                    ))
                    .is_err()
                );
                assert_eq!(lua.world().btech, before.btech);
                let text = support::run_text(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("fire {index} #{}", target.0),
                );
                assert!(text.contains("You fire"), "{text}");
                assert_eq!(
                    text.contains("ammo loader mechanism jams"),
                    jammed,
                    "{text}"
                );
                lua.eval_callback::<mlua::Table>(&format!(
                    "return btech.unit.fire({},1,{index},{})",
                    id.0, target.0
                ))
                .unwrap();
                assert_eq!(native.world().btech, lua.world().btech);
                assert_eq!(native.world().btech, expected.btech);
                if !probed_fire {
                    probed_fire = true;
                    persistence::save(&config.database(), &expected)
                        .await
                        .unwrap();
                    assert_eq!(
                        persistence::load(&config.database()).await.unwrap().btech,
                        expected.btech
                    );
                }
            }
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn rotary_burst_native_lua_supply_jams_and_restart_rotary_ac2() {
    use stompymux_rs::Weapon;
    rotary_burst_matrix(&[Weapon::RotaryAc2]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn rotary_burst_native_lua_supply_jams_and_restart_rotary_ac5() {
    use stompymux_rs::Weapon;
    rotary_burst_matrix(&[Weapon::RotaryAc5]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn rotary_burst_native_lua_supply_jams_and_restart_clan_rotary_ac2() {
    use stompymux_rs::Weapon;
    rotary_burst_matrix(&[Weapon::ClanRotaryAc2]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn rotary_burst_native_lua_supply_jams_and_restart_clan_rotary_ac5() {
    use stompymux_rs::Weapon;
    rotary_burst_matrix(&[Weapon::ClanRotaryAc5]).await;
}

/// Gatling fire shares a single supply-limited die across damage/heat/expenditure and replays both interfaces.
async fn gatling_matrix(weapons: &[stompymux_rs::Weapon]) {
    use stompymux_rs::*;
    let (_dir, config, mut pristine, id, target) = shot_fixture().await;
    shot_seed(&mut pristine, target, 1);
    // The first die determines firing intensity; the next pair determines accuracy.
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = Dice::seeded([*seed; 32]);
            dice.d6() == 6 && dice.two_d6() == 6
        })
        .unwrap();
    shot_seed(&mut pristine, id, seed);
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let pristine_db = snapshot_database(&config);
    let mut probed_restart = false;
    let mut probed_fire = false;
    for weapon in weapons.iter().copied() {
        for supply in [0_u16, 1, 2, 3, 5, 6, 17, 18, 21] {
            for skill in [0, 30] {
                restore_database(&config, &pristine_db);
                let mut base = pristine.clone();
                shot_skill(&mut base, skill);
                set_battle_character_value(
                    &mut base,
                    ObjectId(1),
                    "Gunnery-Ballistic",
                    CharacterValue {
                        value: skill,
                        experience: 0,
                        last_used: 0,
                    },
                )
                .unwrap();
                let mut definition = base.btech.constructed_units()[&id].definition().clone();
                let part = definition
                    .sections
                    .get_mut(&MechSection::LeftArm)
                    .unwrap()
                    .criticals
                    .get_mut(&2)
                    .unwrap();
                part.equipment = weapon.name().into();
                part.modes = vec!["Gattling".into()];
                let mut bin = definition.sections[&MechSection::RightTorso].criticals[&0].clone();
                bin.equipment = format!("Ammo_{}", weapon.name());
                bin.data = weapon.profile().ammunition_per_ton.to_string();
                definition
                    .sections
                    .get_mut(&MechSection::RightTorso)
                    .unwrap()
                    .criticals
                    .insert(0, bin.clone());
                definition
                    .sections
                    .get_mut(&MechSection::RightTorso)
                    .unwrap()
                    .criticals
                    .insert(4, bin);
                let factory =
                    Mech::from_template(crate::support::templates::legal(definition.clone()))
                        .unwrap();
                let index = factory
                    .loadout()
                    .unwrap()
                    .weapons
                    .iter()
                    .position(|mount| mount.weapon == weapon)
                    .unwrap();
                assert_eq!(factory.fire_mode(index).unwrap(), FireMode::Gatling);
                base.btech
                    .rewrite_unit_record(id, |record| {
                        crate::support::templates::set_record_definition(record, definition);
                        record["ammunition"] =
                            serde_json::json!([supply.min(1), supply.saturating_sub(1)]);
                    })
                    .unwrap();
                base.validate(&config).unwrap();
                install(&native, base.clone());
                install(&lua, base.clone());
                assert!(
                    lua.eval_callback::<()>(&format!(
                        "btech.unit.gattling({},1,{index}); error('abort')",
                        id.0
                    ))
                    .is_err()
                );
                assert_eq!(lua.world().btech, base.btech);
                let text = support::run_text(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("gattling {index}"),
                );
                assert!(text.contains("Gattling mode"), "{text}");
                assert_eq!(
                    lua.eval_callback::<String>(&format!(
                        "return btech.unit.gattling({},1,{index})",
                        id.0
                    ))
                    .unwrap(),
                    "gatling"
                );
                assert_eq!(native.world().btech, lua.world().btech);
                let before = native.world().clone();
                let mut expected = before.clone();
                if supply == 0 {
                    assert!(
                        resolve_battle_shot(
                            &mut expected,
                            id,
                            ObjectId(1),
                            target,
                            index,
                            configured_shot_rules(&config)
                        )
                        .is_err()
                    );
                    assert!(spend_battle_weapon(&mut expected, id, ObjectId(1), index).is_err());
                    assert_eq!(expected.btech, before.btech);
                    continue;
                }
                // Restart probe runs once per shard; every scenario keeps the resolve assertions.
                let restored = if probed_restart {
                    None
                } else {
                    probed_restart = true;
                    persistence::save(&config.database(), &before)
                        .await
                        .unwrap();
                    let mut restored = persistence::load(&config.database()).await.unwrap();
                    restored
                        .objects
                        .get_mut(&ObjectId(1))
                        .unwrap()
                        .flags
                        .insert(Flag::Connected);
                    Some(restored)
                };
                let report = resolve_battle_shot(
                    &mut expected,
                    id,
                    ObjectId(1),
                    target,
                    index,
                    configured_shot_rules(&config),
                )
                .unwrap();
                if let Some(mut restored) = restored {
                    assert_eq!(
                        resolve_battle_shot(
                            &mut restored,
                            id,
                            ObjectId(1),
                            target,
                            index,
                            configured_shot_rules(&config)
                        )
                        .unwrap(),
                        report
                    );
                    assert_eq!(restored.btech, expected.btech);
                }
                // A six rolls 5 for a light machine gun and 7 for a heavy one (Tactical
                // Operations p. 100), three rounds a point.
                let rolled: u16 = match weapon {
                    Weapon::LightMachineGun | Weapon::ClanLightMachineGun => 5,
                    Weapon::HeavyMachineGun | Weapon::ClanHeavyMachineGun => 7,
                    _ => 6,
                };
                let damage = rolled.min((supply / 3).max(1)) as u8;
                let spent = supply.min(u16::from(damage) * 3);
                assert_eq!(report.roll, 6);
                assert!(report.launched && !report.jammed && !report.loader_destroyed);
                assert_eq!(report.expenditure.gatling_damage, Some(damage));
                assert_eq!(report.expenditure.heat, damage);
                assert_eq!(
                    report
                        .expenditure
                        .ammunition
                        .iter()
                        .map(|draw| draw.rounds)
                        .sum::<u16>(),
                    spent
                );
                assert_eq!(report.salvo.is_some(), skill == 30);
                if let Some(stompymux_rs::TargetSalvo::Mech(salvo)) = &report.salvo {
                    assert!(salvo.cluster_roll.is_none());
                    assert_eq!(salvo.groups.len(), 1);
                    assert_eq!(salvo.groups[0].damage, u16::from(damage));
                }
                if skill == 30 {
                    let mut glancing_world = before.clone();
                    let level =
                        u8::try_from(30 + report.target_number.unwrap() - i32::from(report.roll))
                            .unwrap();
                    set_battle_character_value(
                        &mut glancing_world,
                        ObjectId(1),
                        "Gunnery-Ballistic",
                        CharacterValue {
                            value: level,
                            experience: 0,
                            last_used: 0,
                        },
                    )
                    .unwrap();
                    let glanced = resolve_battle_shot(
                        &mut glancing_world,
                        id,
                        ObjectId(1),
                        target,
                        index,
                        configured_shot_rules(&config),
                    )
                    .unwrap();
                    assert!(glanced.glancing);
                    assert_eq!(glanced.expenditure, report.expenditure);
                    // A glancing burst halves its damage rounding down (Tactical Operations
                    // p. 79); a single point glances off without a damage group.
                    let groups = glanced.salvo.unwrap().into_mech().unwrap().groups;
                    let halved = u16::from(damage) / 2;
                    if halved == 0 {
                        assert!(groups.is_empty());
                    } else {
                        assert_eq!(groups[0].damage, halved);
                    }
                }
                let unit = &expected.btech.constructed_units()[&id];
                assert_eq!(unit.ammunition().iter().sum::<u16>(), supply - spent);
                assert_eq!(unit.heat().stored, f64::from(damage));
                assert_eq!(unit.fire_mode(index).unwrap(), FireMode::Gatling);
                let mut spent_world = before.clone();
                assert_eq!(
                    spend_battle_weapon(&mut spent_world, id, ObjectId(1), index).unwrap(),
                    report.expenditure
                );
                assert_eq!(
                    roll_unit_dice(&mut spent_world, id, 2)
                        .unwrap()
                        .iter()
                        .sum::<u8>(),
                    report.roll
                );
                // Gatling consumes multiple rounds but records exactly one directed attack.
                spent_world.btech
                    .rewrite_unit_record(id, |record| {
                record["shot_counters"] = serde_json::json!({
                    "fired": 1, "hit": i32::from(skill == 30), "missed": i32::from(skill != 30)
                });
                record["damage_counters"] = serde_json::json!({
                    "taken": 0, "inflicted": if skill == 30 { i32::from(damage) } else { 0 }
                });
                })
                    .unwrap();
                assert_eq!(spent_world.btech.constructed_units()[&id], *unit);
                assert!(
                    lua.eval_callback::<()>(&format!(
                        "btech.unit.fire({},1,{index},{}); error('abort')",
                        id.0, target.0
                    ))
                    .is_err()
                );
                assert_eq!(lua.world().btech, before.btech);
                let text = support::run_text(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("fire {index} #{}", target.0),
                );
                assert!(text.contains("You fire"), "{text}");
                lua.eval_callback::<mlua::Table>(&format!(
                    "return btech.unit.fire({},1,{index},{})",
                    id.0, target.0
                ))
                .unwrap();
                assert_eq!(native.world().btech, lua.world().btech);
                assert_eq!(native.world().btech, expected.btech);
                if !probed_fire {
                    probed_fire = true;
                    persistence::save(&config.database(), &expected)
                        .await
                        .unwrap();
                    assert_eq!(
                        persistence::load(&config.database()).await.unwrap().btech,
                        expected.btech
                    );
                }
            }
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn gatling_native_lua_low_supply_dice_and_restart_machine_gun() {
    use stompymux_rs::Weapon;
    gatling_matrix(&[Weapon::MachineGun]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn gatling_native_lua_low_supply_dice_and_restart_heavy_machine_gun() {
    use stompymux_rs::Weapon;
    gatling_matrix(&[Weapon::HeavyMachineGun]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn gatling_native_lua_low_supply_dice_and_restart_clan_machine_gun() {
    use stompymux_rs::Weapon;
    gatling_matrix(&[Weapon::ClanMachineGun]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn gatling_native_lua_low_supply_dice_and_restart_clan_light_machine_gun() {
    use stompymux_rs::Weapon;
    gatling_matrix(&[Weapon::ClanLightMachineGun]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn gatling_native_lua_low_supply_dice_and_restart_clan_heavy_machine_gun() {
    use stompymux_rs::Weapon;
    gatling_matrix(&[Weapon::ClanHeavyMachineGun]).await;
}

/// Special autocannon ammunition keeps its own supply, composes with rapid fire, and replays both interfaces.
/// Sharded one weapon per test so the ammunition x rapid grid runs in parallel; restart probes
/// fire once per shard, per the pellet-family probe latching convention.
async fn special_autocannon_matrix(weapons: &[stompymux_rs::Weapon]) {
    use stompymux_rs::*;
    let (_dir, config, mut pristine, id, target) = shot_fixture().await;
    shot_skill(&mut pristine, 30);
    set_battle_character_value(
        &mut pristine,
        ObjectId(1),
        "Gunnery-Ballistic",
        CharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    // Every scenario fires at a walking, running target.
    let mut state = serde_json::to_value(&pristine.btech).unwrap();
    state["constructed"][target.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    state["constructed"][target.0.to_string()]["motion"]["speed"] = 50.into();
    pristine.btech = serde_json::from_value(state).unwrap();
    pristine.validate(&config).unwrap();
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 8)
        .unwrap();
    shot_seed(&mut pristine, id, seed);
    shot_seed(&mut pristine, target, 1);
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let pristine_db = snapshot_database(&config);
    let mut probed = false;
    let mut replayed: Vec<(bool, bool, bool)> = Vec::new();
    let mut toggled: Vec<AmmunitionMode> = Vec::new();
    for (ammunition, label, command, divisor) in [
        (AmmunitionMode::ArmorPiercing, "AP", "armorpiercing", 2),
        (AmmunitionMode::Precision, "Precision", "precision", 2),
        (AmmunitionMode::Flechette, "Flechette", "flechette", 1),
        (AmmunitionMode::Caseless, "CASELESS", "caseless", 1),
    ] {
        for weapon in weapons.iter().copied() {
            for rapid in [false, true] {
                restore_database(&config, &pristine_db);
                let mut base = pristine.clone();
                let mut definition = base.btech.constructed_units()[&id].definition().clone();
                let mut part = definition.sections[&MechSection::LeftArm].criticals[&2].clone();
                part.equipment = weapon.name().into();
                for slot in 2..2 + weapon.profile().critical_slots {
                    definition
                        .sections
                        .get_mut(&MechSection::LeftArm)
                        .unwrap()
                        .criticals
                        .insert(slot, part.clone());
                }
                let mut bin = definition.sections[&MechSection::RightTorso].criticals[&0].clone();
                bin.equipment = format!("Ammo_{}", weapon.name());
                bin.data = weapon.profile().ammunition_per_ton.to_string();
                definition
                    .sections
                    .get_mut(&MechSection::RightTorso)
                    .unwrap()
                    .criticals
                    .insert(4, bin.clone());
                let multiplier = if ammunition == AmmunitionMode::Caseless {
                    2
                } else {
                    1
                };
                bin.modes = vec![
                    if ammunition == AmmunitionMode::Caseless {
                        "Caseless"
                    } else {
                        label
                    }
                    .into(),
                ];
                bin.data = (weapon.profile().ammunition_per_ton * multiplier / divisor).to_string();
                definition
                    .sections
                    .get_mut(&MechSection::RightTorso)
                    .unwrap()
                    .criticals
                    .insert(0, bin);
                base.btech
                    .rewrite_unit_record(id, |record| {
                        crate::support::templates::set_record_definition(record, definition);
                        record["ammunition"] = serde_json::json!([
                            weapon.profile().ammunition_per_ton * multiplier / divisor,
                            weapon.profile().ammunition_per_ton
                        ]);
                    })
                    .unwrap();
                base.validate(&config).unwrap();
                let index = base.btech.constructed_units()[&id]
                    .loadout()
                    .unwrap()
                    .weapons
                    .iter()
                    .position(|m| m.weapon == weapon)
                    .unwrap();
                if rapid {
                    toggle_battle_rapid(&mut base, id, ObjectId(1), index).unwrap();
                }
                if ammunition == AmmunitionMode::ArmorPiercing && !rapid {
                    check_armor_piercing_thresholds(&base, &config, id, target, index, weapon);
                }
                if ammunition == AmmunitionMode::Caseless {
                    let spectator = base.create(&config, "Explosion witness".into(), Kind::Player);
                    let object = base.objects.get_mut(&spectator).unwrap();
                    object.location = Some(target);
                    object.flags.insert(Flag::Connected);
                    check_caseless_failures(
                        &base,
                        &config,
                        &native,
                        &lua,
                        id,
                        target,
                        index,
                        rapid,
                        &mut probed,
                        &mut replayed,
                    )
                    .await;
                }
                install(&native, base.clone());
                install(&lua, base.clone());
                let mut normal = base.clone();
                let normal_report = resolve_battle_shot(
                    &mut normal,
                    id,
                    ObjectId(1),
                    target,
                    index,
                    configured_shot_rules(&config),
                )
                .unwrap();
                assert!(
                    lua.eval_callback::<()>(&format!(
                        "btech.unit.{command}({},1,{index}); error('abort')",
                        id.0
                    ))
                    .is_err()
                );
                assert_eq!(lua.world().btech, base.btech);
                let text = support::run_text(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("{command} {index}"),
                );
                assert!(text.contains(&format!("{label} rounds")), "{text}");
                assert_eq!(
                    lua.eval_callback::<String>(&format!(
                        "return btech.unit.{command}({},1,{index})",
                        id.0
                    ))
                    .unwrap(),
                    if ammunition == AmmunitionMode::ArmorPiercing {
                        "armor_piercing"
                    } else {
                        command
                    }
                );
                assert_eq!(native.world().btech, lua.world().btech);
                // Switching ammunition does not require a matching stocked bin, spend dice,
                // or alter the independently selected rapid-fire mode; the toggle sequence
                // probes once per ammunition mode, per the pellet-family latching convention.
                let selected = native.world().btech.clone();
                let other = if command == "precision" {
                    "flechette"
                } else {
                    "precision"
                };
                let armor_piercing = if ammunition == AmmunitionMode::ArmorPiercing {
                    "armor_piercing"
                } else {
                    command
                };
                let toggle_cases: Vec<(&str, &str)> = if toggled.contains(&ammunition) {
                    Vec::new()
                } else {
                    toggled.push(ammunition);
                    vec![
                        (other, other),
                        (command, armor_piercing),
                        (command, "normal"),
                        (command, armor_piercing),
                    ]
                };
                for (selection, expected_mode) in toggle_cases {
                    support::run_text(
                        &native,
                        &config,
                        ObjectId(1),
                        1,
                        &format!("{selection} {index}"),
                    );
                    assert_eq!(
                        lua.eval_callback::<String>(&format!(
                            "return btech.unit.{selection}({},1,{index})",
                            id.0
                        ))
                        .unwrap(),
                        expected_mode
                    );
                    assert_eq!(native.world().btech, lua.world().btech);
                }
                assert_eq!(native.world().btech, selected);
                let listing = support::run_text(&native, &config, ObjectId(1), 1, "weapons");
                assert!(listing.contains(&format!("[{label}]")), "{listing}");
                if rapid {
                    assert!(listing.contains(&format!("[RAPID] [{label}]")), "{listing}");
                }
                let before = native.world().clone();
                // Restart probe runs once per shard; later scenarios keep the resolve assertions.
                let restored = if probed {
                    None
                } else {
                    probed = true;
                    persistence::save(&config.database(), &before)
                        .await
                        .unwrap();
                    let mut restored = persistence::load(&config.database()).await.unwrap();
                    restored
                        .objects
                        .get_mut(&ObjectId(1))
                        .unwrap()
                        .flags
                        .insert(Flag::Connected);
                    Some(restored)
                };
                let mut expected = before.clone();
                let report = resolve_battle_shot(
                    &mut expected,
                    id,
                    ObjectId(1),
                    target,
                    index,
                    configured_shot_rules(&config),
                )
                .unwrap();
                if let Some(mut restored) = restored {
                    assert_eq!(
                        resolve_battle_shot(
                            &mut restored,
                            id,
                            ObjectId(1),
                            target,
                            index,
                            configured_shot_rules(&config),
                        )
                        .unwrap(),
                        report
                    );
                    assert_eq!(restored.btech, expected.btech);
                }

                let shell_damage = u16::from(weapon.profile().damage)
                    / if ammunition == AmmunitionMode::Flechette {
                        2
                    } else {
                        1
                    };
                assert!(
                    report
                        .salvo
                        .as_ref()
                        .unwrap()
                        .as_mech()
                        .unwrap()
                        .groups
                        .iter()
                        .all(|group| group.damage == shell_damage)
                );

                assert_eq!(
                    report.aim.target_movement,
                    if ammunition == AmmunitionMode::Precision {
                        (normal_report.aim.target_movement - 2).max(0)
                    } else {
                        normal_report.aim.target_movement
                    }
                );
                assert_eq!(
                    report.aim.ammunition_accuracy,
                    if ammunition == AmmunitionMode::ArmorPiercing {
                        1
                    } else {
                        0
                    }
                );
                assert_eq!(report.expenditure.ammunition_mode, ammunition);
                assert!(
                    report
                        .expenditure
                        .ammunition
                        .iter()
                        .all(|draw| draw.bin_index == 0)
                );
                assert_eq!(
                    expected.btech.constructed_units()[&id].ammunition()[1],
                    u16::from(weapon.profile().ammunition_per_ton)
                );
                assert_eq!(
                    report
                        .expenditure
                        .ammunition
                        .iter()
                        .map(|draw| draw.rounds)
                        .sum::<u16>(),
                    if rapid { 2 } else { 1 }
                );
                assert!(
                    lua.eval_callback::<()>(&format!(
                        "btech.unit.fire({},1,{index},{}); error('abort')",
                        id.0, target.0
                    ))
                    .is_err()
                );
                assert_eq!(lua.world().btech, before.btech);
                support::run_text(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("fire {index} #{}", target.0),
                );
                lua.eval_callback::<mlua::Table>(&format!(
                    "return btech.unit.fire({},1,{index},{})",
                    id.0, target.0
                ))
                .unwrap();
                assert_eq!(native.world().btech, lua.world().btech);
                assert_eq!(native.world().btech, expected.btech);
            }
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn special_autocannon_native_lua_supply_aim_and_restart_ac2() {
    use stompymux_rs::Weapon;
    special_autocannon_matrix(&[Weapon::Ac2]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn special_autocannon_native_lua_supply_aim_and_restart_ac5() {
    use stompymux_rs::Weapon;
    special_autocannon_matrix(&[Weapon::Ac5]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn special_autocannon_native_lua_supply_aim_and_restart_ac10() {
    use stompymux_rs::Weapon;
    special_autocannon_matrix(&[Weapon::Ac10]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn special_autocannon_native_lua_supply_aim_and_restart_ac20() {
    use stompymux_rs::Weapon;
    special_autocannon_matrix(&[Weapon::Ac20]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn special_autocannon_native_lua_supply_aim_and_restart_light_ac2() {
    use stompymux_rs::Weapon;
    special_autocannon_matrix(&[Weapon::LightAc2]).await;
}

#[tokio::test(flavor = "current_thread")]
async fn special_autocannon_native_lua_supply_aim_and_restart_light_ac5() {
    use stompymux_rs::Weapon;
    special_autocannon_matrix(&[Weapon::LightAc5]).await;
}

/// Deterministic caseless-failure seed searches shared across shards, keyed by roll pair.
fn caseless_seed_bytes(attack: u8, propellant: u8) -> [u8; 32] {
    static SEEDS: std::sync::OnceLock<std::collections::HashMap<(u8, u8), u32>> =
        std::sync::OnceLock::new();
    let seeds = SEEDS.get_or_init(|| {
        (0u32..10000)
            .map(|value| {
                let mut bytes = [0; 32];
                bytes[..4].copy_from_slice(&value.to_le_bytes());
                let mut dice = stompymux_rs::Dice::seeded(bytes);
                let first = dice.two_d6();
                ((first, dice.two_d6()), value)
            })
            .collect()
    });
    let mut bytes = [0; 32];
    bytes[..4].copy_from_slice(&seeds[&(attack, propellant)].to_le_bytes());
    bytes
}

/// AP threshold seed searches shared across shards; the first hit matching the target's
/// armor layout is cached per facing.
fn armor_piercing_seed_bytes(rear: bool, target: &stompymux_rs::Mech) -> [u8; 32] {
    static SEEDS: std::sync::OnceLock<[u32; 2]> = std::sync::OnceLock::new();
    let slot = usize::from(rear);
    let value = SEEDS.get_or_init(|| {
        let mut found = [0u32; 2];
        for (slot, arc) in [false, true].iter().enumerate().map(|(i, r)| {
            (
                i,
                if *r {
                    stompymux_rs::HitArc::Rear
                } else {
                    stompymux_rs::HitArc::Front
                },
            )
        }) {
            found[slot] = (0u32..10000)
                .find(|&value| {
                    let mut seed = [0; 32];
                    seed[..4].copy_from_slice(&value.to_le_bytes());
                    let initial = stompymux_rs::Dice::seeded(seed);
                    let mut dice = initial.clone();
                    let roll = dice.two_d6();
                    let hit =
                        stompymux_rs::resolve_battle_hit(target, arc, roll, &mut dice).unwrap();
                    dice.two_d6(); // Material entry.
                    hit.section == stompymux_rs::MechSection::CenterTorso
                        && !hit.through_armor_critical
                        && dice.two_d6() == 12
                })
                .unwrap();
        }
        found
    })[slot];
    let mut bytes = [0; 32];
    bytes[..4].copy_from_slice(&value.to_le_bytes());
    bytes
}

/// Every armor-piercing hit that damages armor rolls once for a critical with the autocannon's
/// modifier, whatever armor remains (Total Warfare p. 140); a penetrating round still makes the
/// ordinary structure roll as well.
fn check_armor_piercing_thresholds(
    base: &stompymux_rs::World,
    config: &stompymux_rs::Config,
    id: ObjectId,
    target: ObjectId,
    index: usize,
    weapon: stompymux_rs::Weapon,
) {
    use stompymux_rs::*;
    for rear in [false, true] {
        let seed = Dice::seeded(armor_piercing_seed_bytes(
            rear,
            &base.btech.constructed_units()[&target],
        ));
        // Serialize once per facing; only the threshold armor field varies per case.
        let mut facing = serde_json::to_value(&base.btech).unwrap();
        let target_state = &mut facing["constructed"][target.0.to_string()];
        target_state["motion"]["heading"] = if rear { 0 } else { 180 }.into();
        target_state["motion"]["speed"] = 0.into();
        // The facing under test carries the center torso's whole armor allowance, twice its
        // eleven structure points (TechManual p. 54); the arm lasers make room for it.
        let (front, back) = if rear { (0, 22) } else { (22, 0) };
        target_state["definition"]["sections"]["CenterTorso"]["armor"] = front.into();
        target_state["definition"]["sections"]["CenterTorso"]["rear"] = back.into();
        crate::support::templates::book_legal_record_keeping(
            target_state,
            Some(MechSection::CenterTorso),
        );
        target_state["dice"] = serde_json::to_value(&seed).unwrap();
        let armor = if rear { "rear" } else { "armor" };
        // The seeded roll after the hit location is a 12; the AP modifier turns it into two
        // criticals for the heavy autocannons and one for the rest.
        let count = if matches!(weapon, Weapon::Ac10 | Weapon::Ac20) {
            2
        } else {
            1
        };
        for remaining in [1i32, 0, -1] {
            let mut state = facing.clone();
            state["constructed"][target.0.to_string()]["sections"]["CenterTorso"][armor] =
                (remaining + i32::from(weapon.profile().damage)).into();
            let mut normal = base.clone();
            normal.btech = serde_json::from_value(state).unwrap();
            normal.validate(config).unwrap();
            let mut ap = normal.clone();
            toggle_battle_armor_piercing(&mut ap, id, ObjectId(1), index).unwrap();
            let report =
                resolve_battle_shot(&mut ap, id, ObjectId(1), target, index, shot_rules()).unwrap();
            let normal_report =
                resolve_battle_shot(&mut normal, id, ObjectId(1), target, index, shot_rules())
                    .unwrap();
            let group = &report.salvo.as_ref().unwrap().as_mech().unwrap().groups[0];
            assert_eq!(group.hit.section, MechSection::CenterTorso);
            let internal_before = base.btech.constructed_units()[&target].sections()
                [&MechSection::CenterTorso]
                .internal;
            let internal = ap.btech.constructed_units()[&target].sections()
                [&MechSection::CenterTorso]
                .internal;
            if remaining < 0 {
                // One point penetrates: the structure roll follows the armor-piercing roll.
                assert!(
                    group.impact.criticals.len() >= count,
                    "{weapon:?}, rear={rear}, remaining={remaining}"
                );
                assert_eq!(internal, internal_before - 1);
                continue;
            }
            // Ordinary rounds stopped by armor roll nothing; AP rounds roll once.
            assert!(
                normal_report
                    .salvo
                    .as_ref()
                    .unwrap()
                    .as_mech()
                    .unwrap()
                    .groups[0]
                    .impact
                    .criticals
                    .is_empty()
            );
            assert_eq!(
                group.impact.criticals.len(),
                count,
                "{weapon:?}, rear={rear}, remaining={remaining}"
            );
            assert_eq!(internal, internal_before);
            // One critical consumes exactly the location, material, critical check and slot
            // rolls; a second center torso critical can add its own system's rolls.
            if count == 1 {
                let mut expected_dice = seed.clone();
                expected_dice.two_d6(); // Material entry in addition to location and critical checks.
                expected_dice.two_d6();
                expected_dice.two_d6();
                let candidates = normal.btech.constructed_units()[&target]
                    .critical_candidates(MechSection::CenterTorso)
                    .len();
                expected_dice.die(candidates as u16).unwrap();
                let saved = serde_json::to_value(&ap.btech.constructed_units()[&target]).unwrap();
                assert_eq!(saved["dice"], serde_json::to_value(expected_dice).unwrap());
            }
        }
    }
}

/// A caseless feed breaks down only on an attack roll of 2 (Tactical Operations: AUE p. 165):
/// the shot is spent and misses, one Determining Critical Hits roll hits the autocannon's own
/// slots, and the feed is useless for the rest of the game. Rapid-fire mode's own explosion on
/// a 2 (Tactical Operations p. 98) applies alongside it.
async fn check_caseless_failures(
    base: &stompymux_rs::World,
    config: &stompymux_rs::Config,
    native: &stompymux_rs::Scripts,
    lua: &stompymux_rs::Scripts,
    id: ObjectId,
    target: ObjectId,
    index: usize,
    rapid: bool,
    probed_restart: &mut bool,
    replayed: &mut Vec<(bool, bool, bool)>,
) {
    use stompymux_rs::*;
    // One contact refresh and observer check covers every roll/critical shape;
    // each sub-scenario clones the prepared witness state instead.
    let mut shared = base.clone();
    for _ in 0..16 {
        if shared.btech.constructed_units()[&target]
            .contacts()
            .contains_key(&id)
        {
            break;
        }
        refresh_battle_contacts(&mut shared, &[target]).unwrap();
    }
    assert!(!battle_observer_messages(&shared, id, "test").is_empty());
    for attack in [2, 3] {
        for (critical, short) in [(7, false), (8, false), (7, true), (8, true)] {
            if short && !rapid {
                continue;
            }
            let mut before = shared.clone();
            toggle_battle_caseless(&mut before, id, ObjectId(1), index).unwrap();
            let seed = stompymux_rs::Dice::seeded(caseless_seed_bytes(attack, critical));
            before
                .btech
                .rewrite_unit_record(id, |record| {
                    record["dice"] = serde_json::to_value(&seed).unwrap();
                    if short {
                        record["ammunition"][0] = 1.into();
                    }
                })
                .unwrap();
            let mut expected = before.clone();
            let report =
                resolve_battle_shot(&mut expected, id, ObjectId(1), target, index, shot_rules())
                    .unwrap();
            let breakdown = attack == 2;
            let burst = rapid && !short;
            let exploded = breakdown && burst;
            // The single-slot autocannon is wrecked by any critical hit (a roll of 8 or more).
            let wrecked = breakdown && (critical >= 8 || exploded);
            assert_eq!(report.roll, attack);
            assert_eq!(report.feed_critical_roll, breakdown.then_some(critical));
            assert_eq!(
                report.expenditure.fire_mode,
                if burst {
                    FireMode::Rapid
                } else {
                    FireMode::Normal
                }
            );
            assert_eq!(
                expected.btech.constructed_units()[&id]
                    .fire_mode(index)
                    .unwrap(),
                report.expenditure.fire_mode
            );
            assert!(report.launched);
            // Rapid-fire mode's own jam on a 3 (Tactical Operations p. 98) still applies.
            let rapid_jam = burst && attack == 3;
            assert_eq!(report.loader_destroyed, wrecked);
            assert_eq!(report.jammed, (breakdown && !wrecked) || rapid_jam);
            assert_eq!(report.feed_broken, breakdown && !wrecked);
            assert_eq!(report.misload.is_some(), exploded);
            if breakdown {
                // The shot automatically misses, but its ammunition and heat are spent.
                assert!(!report.hit && report.salvo.is_none());
                assert_eq!(
                    expected.btech.constructed_units()[&target],
                    before.btech.constructed_units()[&target]
                );
            }
            let rounds = if burst { 2 } else { 1 };
            assert_eq!(
                report.expenditure.heat,
                report.expenditure.weapon.profile().heat * rounds
            );
            assert_eq!(
                report
                    .expenditure
                    .ammunition
                    .iter()
                    .map(|draw| draw.rounds)
                    .sum::<u16>(),
                u16::from(rounds)
            );
            let unit = &expected.btech.constructed_units()[&id];
            assert_eq!(unit.weapon_intact(index).unwrap(), !wrecked);
            // An explosion's internal damage can add its own feed conditions to the wreck.
            if !exploded {
                assert_eq!(
                    unit.weapon_jammed(index).unwrap(),
                    (breakdown && !wrecked) || rapid_jam
                );
                assert_eq!(
                    unit.weapon_feed_broken(index).unwrap(),
                    breakdown && !wrecked
                );
            }
            assert_eq!(unit.weapon_recycle().contains_key(&index), !wrecked);
            assert_eq!(
                unit.ammunition()[1],
                before.btech.constructed_units()[&id].ammunition()[1]
            );
            if breakdown && !exploded {
                // The breakdown's critical check is the only extra shooter roll.
                let mut dice = seed.clone();
                dice.two_d6();
                dice.two_d6();
                assert_eq!(
                    serde_json::to_value(unit).unwrap()["dice"],
                    serde_json::to_value(dice).unwrap()
                );
            }
            if !breakdown {
                continue;
            }
            // The fire/parity triple's outcomes, texts and messages depend on the critical
            // and ammunition shape, not the attack roll; probe each shape once.
            install(native, before.clone());
            install(lua, before.clone());
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.fire({},1,{index},{}); error('abort')",
                    id.0, target.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before.btech);
            assert!(lua.drain_outbox().is_empty());
            let text = support::run_text(
                native,
                config,
                ObjectId(1),
                1,
                &format!("fire {index} #{}", target.0),
            );
            assert!(text.contains("You fire"), "{text}");
            assert!(text.contains("caseless feed on your"), "{text}");
            assert_eq!(text.contains("wrecks your"), critical >= 8, "{text}");
            assert_eq!(text.contains("explode in the barrel"), exploded, "{text}");
            assert_eq!(
                text.contains("shudders from an internal explosion!"),
                exploded,
                "{text}"
            );
            lua.eval_callback::<mlua::Table>(&format!(
                "return btech.unit.fire({},1,{index},{})",
                id.0, target.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, expected.btech);
            assert_eq!(lua.world().btech, expected.btech);
            let messages = lua.drain_outbox();
            assert_eq!(
                messages.iter().any(|(_, text)| text
                    .source()
                    .contains("shudders from an internal explosion!")),
                exploded
            );
            // Restart parity probe runs once per shard; the save+recovery replay
            // below is latched per distinct recovery shape.
            if !*probed_restart {
                *probed_restart = true;
                persistence::save(&config.database(), &before)
                    .await
                    .unwrap();
                let mut loaded = persistence::load(&config.database()).await.unwrap();
                loaded
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
                assert_eq!(
                    resolve_battle_shot(&mut loaded, id, ObjectId(1), target, index, shot_rules())
                        .unwrap(),
                    report
                );
                assert_eq!(loaded.btech, expected.btech);
            }
            let shape = (rapid, short, wrecked);
            if replayed.contains(&shape) {
                continue;
            }
            replayed.push(shape);
            persistence::save(&config.database(), &expected)
                .await
                .unwrap();
            let mut recovery = persistence::load(&config.database()).await.unwrap();
            let frozen = recovery.btech.clone();
            // Neither a wrecked mount nor a broken feed can be cleared in the field, and a
            // broken feed still refuses mode changes.
            let error = begin_battle_unjam(&mut recovery, id, ObjectId(1), index).unwrap_err();
            if wrecked {
                assert_eq!(error.to_string(), "That weapon has been destroyed");
            } else {
                assert_eq!(
                    error.to_string(),
                    "The ammo feed on that weapon is broken beyond field repair."
                );
                assert!(toggle_battle_caseless(&mut recovery, id, ObjectId(1), index).is_err());
            }
            assert_eq!(recovery.btech, frozen);
            for _ in 0..130 {
                advance_battle_units(&mut recovery, 0, true);
            }
            assert!(
                !recovery.btech.constructed_units()[&id]
                    .weapon_readiness(index)
                    .unwrap()
                    .ready
            );
            assert_eq!(
                recovery.btech.constructed_units()[&id]
                    .ammunition_mode(index)
                    .unwrap(),
                AmmunitionMode::Caseless
            );
        }
    }
}

#[tokio::test]
async fn observer_broadcasts_use_current_acquired_contacts_without_mutation() {
    use stompymux_rs::*;
    let (_dir, config, world, observer, subject) = lock_fixture().await;
    let message = "shudders from an internal explosion!";
    let name = &world.btech.constructed_units()[&subject].definition().name;
    let before = world.btech.clone();
    let contact = visible_battle_contact(&world, observer, subject)
        .unwrap()
        .unwrap();
    assert_eq!(contact.target, subject);
    assert_eq!(contact.name, *name);
    assert!(
        visible_battle_contact(&world, observer, ObjectId(-1))
            .unwrap()
            .is_none()
    );
    assert!(visible_battle_contact(&world, ObjectId(-1), subject).is_err());

    assert_eq!(
        battle_observer_messages(&world, subject, message),
        vec![(observer, format!("#{} {name} {message}", subject.0))]
    );
    assert_eq!(
        battle_observer_messages(&world, subject, "'s arm explodes!"),
        vec![(observer, format!("#{} {name}'s arm explodes!", subject.0))]
    );
    assert_eq!(world.btech, before);
    for case in [
        "unacquired",
        "shutdown",
        "observer_gone",
        "subject_gone",
        "hidden",
        "moved",
    ] {
        let mut altered = world.clone();
        let mut state = serde_json::to_value(&altered.btech).unwrap();
        match case {
            "unacquired" => {
                state["constructed"][observer.0.to_string()]["contacts"] = serde_json::json!({})
            }
            "shutdown" => {
                state["constructed"][observer.0.to_string()]["power"] =
                    serde_json::json!({"state":"off"})
            }
            "observer_gone" => {
                altered
                    .objects
                    .get_mut(&observer)
                    .unwrap()
                    .flags
                    .insert(Flag::Going);
            }
            "subject_gone" => {
                altered
                    .objects
                    .get_mut(&subject)
                    .unwrap()
                    .flags
                    .insert(Flag::Going);
            }
            "moved" | "hidden" => {}
            _ => unreachable!(),
        }
        altered.btech = serde_json::from_value(state).unwrap();
        if case == "moved" {
            let map = altered.create(&config, "Other battlefield".into(), Kind::Thing);
            create_battle_map(
                &mut altered,
                map,
                "other",
                MapAsset::from_cells("1 1\n.0\n").unwrap(),
            )
            .unwrap();
            place_battle_unit(&mut altered, subject, map, 0, 0).unwrap();
        }
        if case == "hidden" {
            // An adjacent subject is always in sight; only invisibility leaves the saved
            // contact stale.
            set_battle_visibility(
                &mut altered,
                subject,
                Visibility {
                    invisible: true,
                    clairvoyant: false,
                },
            )
            .unwrap();
        }
        if case == "shutdown" {
            assert!(visible_battle_contact(&altered, observer, subject).is_err());
        } else if case != "observer_gone" {
            assert!(
                visible_battle_contact(&altered, observer, subject)
                    .unwrap()
                    .is_none(),
                "{case}"
            );
        }
        assert!(
            battle_observer_messages(&altered, subject, message).is_empty(),
            "{case}"
        );
    }
}

#[tokio::test]
async fn ammunition_explosion_observers_survive_damage_and_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut world, observer, subject) = lock_fixture().await;
    let rules = shot_rules();
    let rules = FallRules {
        vehicle_impact: stompymux_rs::VehicleImpactRules::STANDARD,
        extended_piloting: rules.extended_piloting,
        toughness: rules.target_toughness,
    };
    let name = world.btech.constructed_units()[&subject]
        .definition()
        .name
        .clone();
    persistence::save(&config.database(), &world).await.unwrap();
    let before = world.btech.clone();
    let report = explode_battle_ammunition(&mut world, subject, 0, rules).unwrap();
    let broadcast = format!("#{} {name} has an internal ammo explosion!", subject.0);
    assert_eq!(
        report
            .notices
            .iter()
            .filter(|notice| notice.unit == observer && notice.text == broadcast)
            .count(),
        1
    );
    assert!(world.btech.constructed_units()[&subject].is_destroyed());
    let mut restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, before);
    assert_eq!(
        explode_battle_ammunition(&mut restored, subject, 0, rules).unwrap(),
        report
    );
    assert_eq!(restored.btech, world.btech);
    let before = world.btech.clone();
    assert!(explode_battle_ammunition(&mut world, subject, 0, rules).is_err());
    assert_eq!(world.btech, before);
}

#[tokio::test]
async fn unjam_feedback_separates_pilot_cockpit_and_observers() {
    use stompymux_rs::*;
    let (_dir, _config, mut base, subject, observer) = lock_fixture().await;
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][observer.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    base.btech = serde_json::from_value(state).unwrap();
    for _ in 0..16 {
        if base.btech.constructed_units()[&observer]
            .contacts()
            .contains_key(&subject)
        {
            break;
        }
        refresh_battle_contacts(&mut base, &[observer]).unwrap();
    }
    assert!(!battle_observer_messages(&base, subject, "test").is_empty());
    for outcome in ["success", "failure", "empty", "prone", "no_pilot"] {
        let mut world = base.clone();
        let mut unit = world.btech.constructed_units()[&subject].clone();
        let index = unit
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == Weapon::Srm4)
            .unwrap();
        unit.jam_weapon(index).unwrap();
        world
            .btech
            .rewrite_unit_record(subject, |record| {
                *record = serde_json::to_value(unit).unwrap();
            })
            .unwrap();
        begin_battle_unjam(&mut world, subject, ObjectId(1), index).unwrap();
        let seed = (0..=255)
            .find(|seed| (Dice::seeded([*seed; 32]).two_d6() >= 6) == (outcome != "failure"))
            .unwrap();
        shot_seed(&mut world, subject, seed);
        world
            .btech
            .rewrite_unit_record(subject, |record| {
                let unit = record;
                if outcome == "empty" {
                    unit["ammunition"][0] = 0.into();
                }
                if outcome == "prone" {
                    unit["posture"] = "prone".into();
                }
                if outcome == "no_pilot" {
                    unit["pilot"] = serde_json::Value::Null;
                }
            })
            .unwrap();
        for _ in 0..59 {
            assert!(
                advance_battle_unjamming(&mut world, true)
                    .unwrap()
                    .is_empty()
            );
        }
        let messages = advance_battle_unjamming(&mut world, true).unwrap();
        let rolls: Vec<_> = messages
            .iter()
            .filter(|(_, text)| {
                text.contains("roll to unjam") || text.contains("Modified Gunnery Skill")
            })
            .collect();
        // A prone unit makes the same Gunnery Skill Roll as a standing one.
        if outcome == "empty" {
            assert!(rolls.is_empty());
        } else {
            assert_eq!(rolls.len(), 2);
            let recipient = if outcome == "no_pilot" {
                MessageTarget::Unit(subject)
            } else {
                MessageTarget::Player(ObjectId(1))
            };
            assert!(rolls.iter().all(|(target, _)| *target == recipient));
        }
        let witnessed: Vec<_> = messages
            .iter()
            .filter(|(target, _)| *target == MessageTarget::Unit(observer))
            .collect();
        if matches!(outcome, "failure" | "empty") {
            assert!(witnessed.is_empty());
        } else {
            assert_eq!(witnessed.len(), 1);
            assert!(witnessed[0].1.ends_with("ejects a mangled shell!"));
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn firing_observers_hide_unseen_participants_and_replay_transactionally() {
    use stompymux_rs::*;
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 6)
        .unwrap();
    let target_seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 7)
        .unwrap();
    let (_dir, config, pristine, shooter, target) = shot_fixture().await;
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let restored = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let pristine_db = snapshot_database(&config);
    for visible in 0..4 {
        for powered in [false, true] {
            for hit in [false, true] {
                restore_database(&config, &pristine_db);
                let mut world = pristine.clone();
                shot_skill(&mut world, if hit { 30 } else { 0 });
                shot_seed(&mut world, shooter, seed);
                shot_seed(&mut world, target, target_seed);
                let map = world.btech.constructed_units()[&shooter]
                    .position()
                    .unwrap()
                    .map;
                let observer = world.create(&config, "Fire observer".into(), Kind::Thing);
                create_battle_unit(
                    &mut world,
                    observer,
                    MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml"))
                        .unwrap(),
                )
                .unwrap();
                support::seed_object_dice(&mut world, observer, support::FIXTURE_DICE_SEED);
                place_battle_unit(&mut world, observer, map, 6, 5).unwrap();
                let witness = world.create(&config, "Fire witness".into(), Kind::Player);
                world.objects.get_mut(&witness).unwrap().location = Some(observer);
                world
                    .objects
                    .get_mut(&witness)
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["constructed"][observer.0.to_string()]["power"] =
                    serde_json::json!({"state":"running"});
                // A hostile observer: the shooter's own contacts are not relayed to it
                // (Tactical Operations p. 191), so what it sees is only what it holds.
                state["constructed"][observer.0.to_string()]["signature"]["team"] = 1.into();
                world.btech = serde_json::from_value(state).unwrap();
                for _ in 0..16 {
                    let contacts = world.btech.constructed_units()[&observer].contacts();
                    if contacts.contains_key(&shooter) && contacts.contains_key(&target) {
                        break;
                    }
                    refresh_battle_contacts(&mut world, &[observer]).unwrap();
                }
                assert_eq!(visible_battle_contacts(&world, observer).unwrap().len(), 2);
                let mut state = serde_json::to_value(&world.btech).unwrap();
                let observer_state = &mut state["constructed"][observer.0.to_string()];
                if visible & 1 == 0 {
                    observer_state["contacts"]
                        .as_object_mut()
                        .unwrap()
                        .remove(&shooter.0.to_string());
                }
                if visible & 2 == 0 {
                    observer_state["contacts"]
                        .as_object_mut()
                        .unwrap()
                        .remove(&target.0.to_string());
                }
                if !powered {
                    observer_state["power"] = serde_json::json!({"state":"off"});
                }
                world.btech = serde_json::from_value(state).unwrap();
                let identity = |id: ObjectId| {
                    format!(
                        "#{} {}",
                        id.0,
                        world.btech.constructed_units()[&id].definition().name
                    )
                };
                let outcome = if hit { "hits" } else { "misses" };
                let expected = if !powered {
                    None
                } else {
                    match visible {
                        1 => Some(format!(
                            "{} fires a MediumLaser at something!",
                            identity(shooter)
                        )),
                        2 => Some(format!(
                            "Something {outcome} {} with a MediumLaser",
                            identity(target)
                        )),
                        3 => Some(format!(
                            "{} {outcome} {} with a MediumLaser",
                            identity(shooter),
                            identity(target)
                        )),
                        _ => None,
                    }
                };
                restore_database(&config, &pristine_db);
                persistence::save(&config.database(), &world).await.unwrap();
                install(&native, world.clone());
                install(&lua, world.clone());
                assert!(
                    lua.eval_callback::<()>(&format!(
                        "btech.unit.fire({},1,0,{}); error('abort')",
                        shooter.0, target.0
                    ))
                    .is_err()
                );
                assert_eq!(lua.world().btech, world.btech);
                assert!(lua.drain_outbox().is_empty());
                commands::run(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("fire 0 #{}", target.0),
                )
                .unwrap();
                let native_messages = native.drain_outbox();
                lua.eval_callback::<mlua::Table>(&format!(
                    "return btech.unit.fire({},1,0,{})",
                    shooter.0, target.0
                ))
                .unwrap();
                let lua_messages = lua.drain_outbox();
                let text = |messages: &[(ObjectId, stompymux_rs::text::Document)]| {
                    messages
                        .iter()
                        .map(|(id, text)| (*id, text.source().to_owned()))
                        .collect::<Vec<_>>()
                };
                assert_eq!(text(&native_messages), text(&lua_messages));
                let observed: Vec<_> = native_messages
                    .iter()
                    .filter(|(id, _)| *id == witness)
                    .map(|(_, text)| text.source().to_owned())
                    .collect();
                assert_eq!(observed, expected.into_iter().collect::<Vec<_>>());
                assert_eq!(native.world().btech, lua.world().btech);
                let mut reloaded = persistence::load(&config.database()).await.unwrap();
                for player in [ObjectId(1), witness] {
                    reloaded
                        .objects
                        .get_mut(&player)
                        .unwrap()
                        .flags
                        .insert(Flag::Connected);
                }
                install(&restored, reloaded);
                commands::run(
                    &restored,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("fire 0 #{}", target.0),
                )
                .unwrap();
                assert_eq!(text(&restored.drain_outbox()), text(&native_messages));
                assert_eq!(restored.world().btech, native.world().btech);
            }
        }
    }
}

#[tokio::test(flavor = "current_thread")]
async fn power_observers_receive_completion_and_never_a_shutdown_fall() {
    use stompymux_rs::*;
    let (_dir, config, mut base, subject, observer) = lock_fixture().await;
    let witness = base.create(&config, "Power witness".into(), Kind::Player);
    base.objects.get_mut(&witness).unwrap().location = Some(observer);
    base.objects
        .get_mut(&witness)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][observer.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    base.btech = serde_json::from_value(state).unwrap();
    for _ in 0..16 {
        if base.btech.constructed_units()[&observer]
            .contacts()
            .contains_key(&subject)
        {
            break;
        }
        refresh_battle_contacts(&mut base, &[observer]).unwrap();
    }
    assert!(!battle_observer_messages(&base, subject, "test").is_empty());
    for remaining in [1, 5, 30] {
        let mut world = base.clone();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][subject.0.to_string()]["power"] =
            serde_json::json!({"state":"starting","remaining":remaining});
        world.btech = serde_json::from_value(state).unwrap();
        let mut abort = world.clone();
        let notices = stop_battle_unit(&mut abort, subject, ObjectId(1)).unwrap();
        assert!(notices.iter().all(|notice| notice.unit == subject));
        assert!(advance_battle_units(&mut abort, 0, true).is_empty());
        for _ in 1..remaining {
            assert!(
                advance_battle_units(&mut world, 0, true)
                    .iter()
                    .all(|notice| notice.unit == subject)
            );
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        let notices = advance_battle_units(&mut world, 0, true);
        assert_eq!(advance_battle_units(&mut restored, 0, true), notices);
        assert_eq!(restored.btech, world.btech);
        let observed: Vec<_> = notices
            .iter()
            .filter(|notice| notice.unit == observer)
            .collect();
        assert_eq!(observed.len(), 1);
        assert!(observed[0].text.ends_with("powers up!"));
        assert!(advance_battle_units(&mut world, 0, true).is_empty());
    }
    for speed in [0.0, 10.75, 10.7501, -20.0] {
        let mut state = serde_json::to_value(&base.btech).unwrap();
        state["constructed"][subject.0.to_string()]["motion"]["speed"] = speed.into();
        let mut world = base.clone();
        world.btech = serde_json::from_value(state).unwrap();
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        assert!(
            lua.eval_callback::<()>(&format!("btech.unit.stop({},1); error('abort')", subject.0))
                .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        assert!(lua.drain_outbox().is_empty());
        commands::run(&native, &config, ObjectId(1), 1, "shutdown").unwrap();
        // A moving Mech refuses to shut down (Total Warfare p. 160), so it never falls.
        assert_eq!(
            lua.eval_callback::<()>(&format!("btech.unit.stop({},1)", subject.0))
                .is_ok(),
            speed == 0.0
        );
        assert_eq!(native.world().btech, lua.world().btech);
        if speed != 0.0 {
            assert_eq!(native.world().btech, world.btech);
        }
        let observed = |scripts: &Scripts| {
            scripts
                .drain_outbox()
                .into_iter()
                .filter(|(id, _)| *id == witness)
                .map(|(_, text)| text.source().to_owned())
                .collect::<Vec<_>>()
        };
        let messages = observed(&native);
        assert_eq!(messages, observed(&lua));
        assert!(
            !messages.iter().any(|text| text.contains("mid-motion")),
            "{messages:?}"
        );
    }
}

#[tokio::test]
async fn standing_completion_broadcasts_only_success_and_replays_after_shutdown() {
    use stompymux_rs::*;
    let (_dir, config, base, observer, subject) = lock_fixture().await;
    for remaining in [1, 5] {
        for running in [false, true] {
            for rising in [false, true] {
                let mut world = base.clone();
                let mut state = serde_json::to_value(&world.btech).unwrap();
                let unit = &mut state["constructed"][subject.0.to_string()];
                unit["power"] = serde_json::json!({"state":if running {"running"} else {"off"}});
                unit["posture"] = if rising { "standing" } else { "prone" }.into();
                unit["stand_timer"] = serde_json::json!({"state":if rising {"rising"} else {"recovering"},"remaining":remaining});
                world.btech = serde_json::from_value(state).unwrap();
                world.validate(&config).unwrap();
                let dice = serde_json::to_value(&world.btech.constructed_units()[&subject])
                    .unwrap()["dice"]
                    .clone();
                for _ in 1..remaining {
                    assert!(advance_battle_standing(&mut world).is_empty());
                }
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                let notices = advance_battle_standing(&mut world);
                assert_eq!(advance_battle_standing(&mut restored), notices);
                assert_eq!(world.btech, restored.btech);
                let observed: Vec<_> = notices
                    .iter()
                    .filter(|notice| notice.unit == observer)
                    .collect();
                assert_eq!(observed.len(), usize::from(rising));
                if rising {
                    assert!(observed[0].text.ends_with("stands up!"));
                }
                let own = notices
                    .iter()
                    .find(|notice| notice.unit == subject)
                    .unwrap();
                assert_eq!(
                    own.text,
                    if rising {
                        "You have finally finished standing up."
                    } else {
                        "You have finally recovered from your attempt to stand."
                    }
                );
                assert_eq!(
                    serde_json::to_value(&world.btech.constructed_units()[&subject]).unwrap()["dice"],
                    dice
                );
                assert!(advance_battle_standing(&mut world).is_empty());
            }
        }
    }
}

/// Free-fall contact reports pre-impact visibility once, including after a saved countdown.
#[tokio::test]
async fn free_fall_observers_replay_contact_without_early_or_duplicate_messages() {
    use stompymux_rs::*;
    let (_dir, config, base, observer, subject) = lock_fixture().await;
    for speed in [1, 20] {
        for visible in [false, true] {
            let mut world = base.clone();
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["constructed"][subject.0.to_string()]["free_fall"] = serde_json::json!({
                "elevation": 2, "speed": speed, "remaining": 3,
            });
            if !visible {
                state["constructed"][observer.0.to_string()]["power"] =
                    serde_json::json!({"state":"off"});
            }
            world.btech = serde_json::from_value(state).unwrap();
            world.validate(&config).unwrap();
            let expected = battle_observer_messages(&world, subject, "hits the ground!");
            assert_eq!(expected.len(), usize::from(visible));
            for _ in 0..2 {
                assert!(
                    advance_battle_jumps(
                        &mut world,
                        stompymux_rs::MovementRules {
                            fall: fall_rules(),
                            ..stompymux_rs::MovementRules::STANDARD
                        }
                    )
                    .unwrap()
                    .is_empty()
                );
            }
            // A failed landing must retain the countdown and all damage/dice state.
            let mut rejected = world.clone();
            rejected
                .objects
                .get_mut(&subject)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            let before = rejected.btech.clone();
            assert!(
                advance_battle_jumps(
                    &mut rejected,
                    stompymux_rs::MovementRules {
                        fall: fall_rules(),
                        ..stompymux_rs::MovementRules::STANDARD
                    }
                )
                .is_err()
            );
            assert_eq!(rejected.btech, before);
            persistence::save(&config.database(), &world).await.unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            let notices = advance_battle_jumps(
                &mut world,
                stompymux_rs::MovementRules {
                    fall: fall_rules(),
                    ..stompymux_rs::MovementRules::STANDARD
                },
            )
            .unwrap();
            assert_eq!(
                advance_battle_jumps(
                    &mut restored,
                    stompymux_rs::MovementRules {
                        fall: fall_rules(),
                        ..stompymux_rs::MovementRules::STANDARD
                    }
                )
                .unwrap(),
                notices
            );
            assert_eq!(world.btech, restored.btech);
            assert_eq!(notices[0].unit, subject);
            assert_eq!(notices[0].text, "You hit the ground!");
            let observed: Vec<_> = notices
                .iter()
                .filter(|notice| {
                    notice.unit == observer && notice.text.ends_with("hits the ground!")
                })
                .map(|notice| (notice.unit, notice.text.clone()))
                .collect();
            assert_eq!(observed, expected);
            if visible {
                assert_eq!((notices[1].unit, notices[1].text.clone()), expected[0]);
            }
            assert!(
                world.btech.constructed_units()[&subject]
                    .free_fall()
                    .is_none()
            );
            assert!(
                advance_battle_jumps(
                    &mut world,
                    stompymux_rs::MovementRules {
                        fall: fall_rules(),
                        ..stompymux_rs::MovementRules::STANDARD
                    }
                )
                .unwrap()
                .is_empty()
            );
        }
    }
}

/// Stand attempts report before their outcome, route to observers, and replay as one transaction.
#[tokio::test]
async fn stand_attempt_observers_share_native_lua_order_and_saved_replay() {
    use stompymux_rs::*;
    let (_dir, config, mut base, subject) = stand_fixture().await;
    let observer = base.create(&config, "Stand observer".into(), Kind::Thing);
    create_battle_unit(
        &mut base,
        observer,
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut base, observer, support::FIXTURE_DICE_SEED);
    let map = base.btech.constructed_units()[&subject]
        .position()
        .unwrap()
        .map;
    place_battle_unit(&mut base, observer, map, 6, 5).unwrap();
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][observer.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    base.btech = serde_json::from_value(state).unwrap();
    refresh_battle_contacts(&mut base, &[observer]).unwrap();
    assert!(
        visible_battle_contact(&base, observer, subject)
            .unwrap()
            .is_some()
    );
    let witness = base.create(&config, "Stand witness".into(), Kind::Player);
    base.objects.get_mut(&witness).unwrap().location = Some(observer);
    base.objects
        .get_mut(&witness)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let messages = |scripts: &Scripts| {
        scripts
            .drain_outbox()
            .into_iter()
            .map(|(id, text)| (id, text.source().to_owned()))
            .collect::<Vec<_>>()
    };
    for (mode, command, target) in [
        ("normal", "stand", 6),
        ("anyway", "stand anyway", 6),
        ("careful", "stand careful", 4),
    ] {
        for success in [false, true] {
            for visible in [false, true] {
                let mut world = base.clone();
                let seed = (0..=255)
                    .find(|seed| (Dice::seeded([*seed; 32]).two_d6() >= target) == success)
                    .unwrap();
                shot_seed(&mut world, subject, seed);
                if !visible {
                    let mut state = serde_json::to_value(&world.btech).unwrap();
                    state["constructed"][observer.0.to_string()]["power"] =
                        serde_json::json!({"state":"off"});
                    world.btech = serde_json::from_value(state).unwrap();
                }
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                for player in [ObjectId(1), witness] {
                    restored
                        .objects
                        .get_mut(&player)
                        .unwrap()
                        .flags
                        .insert(Flag::Connected);
                }
                let native = Scripts::new(
                    &config,
                    std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
                )
                .unwrap();
                let lua =
                    Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored)))
                        .unwrap();
                commands::run(&native, &config, ObjectId(1), 1, "stand check").unwrap();
                assert!(messages(&native).is_empty());
                assert_eq!(native.world().btech, world.btech);
                let call = format!("btech.unit.stand({},1,'{}')", subject.0, mode);
                assert!(
                    lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                        .is_err()
                );
                assert_eq!(lua.world().btech, world.btech);
                assert!(messages(&lua).is_empty());
                commands::run(&native, &config, ObjectId(1), 1, command).unwrap();
                let report: mlua::Table = lua.eval_callback(&format!("return {call}")).unwrap();
                let check: mlua::Table = report.get("check").unwrap();
                assert_eq!(check.get::<bool>("success").unwrap(), success);
                let result_notices: mlua::Table = report.get("notices").unwrap();
                assert!(result_notices.raw_len() > 0);
                let output = messages(&native);
                assert_eq!(messages(&lua), output);
                assert_eq!(native.world().btech, lua.world().btech);
                let pilot_output: Vec<_> = output
                    .iter()
                    .filter(|(who, _)| *who == ObjectId(1))
                    .map(|(_, text)| text.as_str())
                    .collect();
                assert_eq!(pilot_output[0], "You make a piloting skill roll!");
                assert_eq!(
                    pilot_output[1],
                    format!(
                        "Modified Pilot Skill: BTH {target}\tRoll: {}",
                        Dice::seeded([seed; 32]).two_d6()
                    )
                );
                let witnessed: Vec<_> = output
                    .iter()
                    .filter(|(id, _)| *id == witness)
                    .map(|(_, text)| text.clone())
                    .collect();
                let mut expected =
                    battle_observer_messages(&world, subject, "attempts to stand up.");
                if !success {
                    expected.extend(battle_observer_messages(&world, subject, "falls down!"));
                }
                let expected = expected
                    .into_iter()
                    .map(|(_, text)| text)
                    .collect::<Vec<_>>();
                // Fall criticals may add damage notices after the ordered stand/fall messages.
                if visible && !success {
                    assert!(witnessed.starts_with(&expected), "{witnessed:?}");
                } else {
                    assert_eq!(witnessed, expected);
                }
                if visible {
                    assert_eq!(output[0].0, witness);
                    assert!(output[0].1.ends_with("attempts to stand up."));
                }
                let before = native.world().btech.clone();
                commands::run(&native, &config, ObjectId(1), 1, command).unwrap();
                assert!(messages(&native).is_empty());
                assert_eq!(native.world().btech, before);
            }
        }
    }
}

/// Thermal stops report independently of speed; falling and successful overrides have distinct feedback.
#[tokio::test]
async fn thermal_shutdown_observers_cover_speed_overrides_visibility_and_replay() {
    use stompymux_rs::*;
    let (_dir, config, mut base, subject, observer) = lock_fixture().await;
    computer_skill(&mut base, 0);
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][observer.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    base.btech = serde_json::from_value(state).unwrap();
    refresh_battle_contacts(&mut base, &[observer]).unwrap();
    assert!(
        visible_battle_contact(&base, observer, subject)
            .unwrap()
            .is_some()
    );
    // The Shutdown Avoid Roll at 14 heat needs a 4 (Total Warfare p. 159); the involuntary
    // shutdown's balance check, due at any speed (Total Warfare p. 60), then rolls a 2 and falls.
    let seeded = |accept: fn(&mut Dice) -> bool| {
        (0u32..100_000)
            .map(|value| {
                let mut bytes = [0; 32];
                bytes[..4].copy_from_slice(&value.to_le_bytes());
                Dice::seeded(bytes)
            })
            .find(|dice| accept(&mut dice.clone()))
            .unwrap()
    };
    let failing = seeded(|dice| dice.two_d6() < 4 && dice.two_d6() == 2);
    let passing = seeded(|dice| dice.two_d6() >= 4);
    for speed in [0.0_f64, 10.75, -10.75, 10.8, -10.8] {
        for avoid in [false, true] {
            for visible in [false, true] {
                let mut world = base.clone();
                let seed = if avoid { &passing } else { &failing };
                let mut state = serde_json::to_value(&world.btech).unwrap();
                let unit = &mut state["constructed"][subject.0.to_string()];
                unit["motion"]["speed"] = speed.into();
                unit["motion"]["desired_speed"] = speed.into();
                if !visible {
                    state["constructed"][observer.0.to_string()]["power"] =
                        serde_json::json!({"state":"off"});
                }
                world.btech = serde_json::from_value(state).unwrap();
                overheat_due(&mut world, subject, 14.0, false);
                world.btech.set_unit_dice(subject, seed.clone()).unwrap();
                let mut expected = if avoid {
                    Vec::new()
                } else if speed == 0.0 {
                    battle_observer_messages(&world, subject, "shuts down!")
                } else {
                    battle_observer_messages(&world, subject, "stops in mid-motion!")
                };
                if !avoid {
                    expected.extend(battle_observer_messages(&world, subject, "falls down!"));
                }
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                if world.objects[&ObjectId(1)].flags.contains(Flag::Connected) {
                    restored
                        .objects
                        .get_mut(&ObjectId(1))
                        .unwrap()
                        .flags
                        .insert(Flag::Connected);
                }
                let reports = advance_battle_overheat(&mut world, overheat_rules()).unwrap();
                assert_eq!(
                    advance_battle_overheat(&mut restored, overheat_rules()).unwrap(),
                    reports
                );
                assert_eq!(world.btech, restored.btech);
                assert_eq!(reports.len(), 1);
                assert_eq!(reports[0].shutdown, !avoid);
                assert_eq!(reports[0].fall.is_some(), !avoid);
                let observed: Vec<_> = reports[0]
                    .messages()
                    .into_iter()
                    .filter(|(recipient, _)| *recipient == observer)
                    .collect();
                assert_eq!(observed, expected);
                assert!(
                    advance_battle_overheat(&mut world, overheat_rules())
                        .unwrap()
                        .is_empty()
                );
                if !avoid {
                    overheat_due(&mut world, subject, 14.0, false);
                    let later = advance_battle_overheat(&mut world, overheat_rules()).unwrap();
                    assert!(later.iter().all(|report| {
                        !report.shutdown
                            && report
                                .messages()
                                .iter()
                                .all(|(recipient, _)| *recipient != observer)
                    }));
                }
            }
        }
    }
}

/// Lua facing controls share native output and guards, with saved poses and callback rollback.
#[tokio::test]
async fn facing_native_lua_controls_replay_without_observer_messages() {
    use stompymux_rs::*;
    let (_dir, config, mut world, subject, observer) = lock_fixture().await;
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][observer.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    world.btech = serde_json::from_value(state).unwrap();
    refresh_battle_contacts(&mut world, &[observer]).unwrap();
    assert!(
        visible_battle_contact(&world, observer, subject)
            .unwrap()
            .is_some()
    );
    let witness = world.create(&config, "Facing witness".into(), Kind::Player);
    world.objects.get_mut(&witness).unwrap().location = Some(observer);
    world
        .objects
        .get_mut(&witness)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let messages = |scripts: &Scripts| {
        scripts
            .drain_outbox()
            .into_iter()
            .map(|(id, text)| (id, text.source().to_owned()))
            .collect::<Vec<_>>()
    };
    for (command, method, argument, torso, flipped) in [
        ("rottorso left", "rottorso", ",' LEFT '", Torso::Left, false),
        ("rottorso right", "rottorso", ",'r'", Torso::Center, false),
        (
            "rottorso right",
            "rottorso",
            ",'RIGHT'",
            Torso::Right,
            false,
        ),
        ("rottorso center", "rottorso", ",'c'", Torso::Center, false),
        ("fliparms", "fliparms", "", Torso::Center, true),
        (
            "rottorso center",
            "rottorso",
            ",'center'",
            Torso::Center,
            true,
        ),
        ("fliparms", "fliparms", "", Torso::Center, false),
    ] {
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for player in [ObjectId(1), witness] {
            restored
                .objects
                .get_mut(&player)
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        let call = format!("btech.unit.{method}({},1{argument})", subject.0);
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort facing')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        assert!(messages(&lua).is_empty());
        for bad in [
            format!("btech.unit.rottorso({},1,'back')", subject.0),
            format!("btech.unit.fliparms({},2)", subject.0),
        ] {
            lua.eval_callback::<()>(&format!("assert(not pcall(function() {bad} end))"))
                .unwrap();
            assert_eq!(lua.world().btech, world.btech);
            assert!(messages(&lua).is_empty());
        }
        commands::run(&native, &config, ObjectId(1), 1, command).unwrap();
        assert!(
            lua.eval_callback::<bool>(&format!("return {call}"))
                .unwrap()
        );
        let output = messages(&native);
        assert_eq!(messages(&lua), output);
        assert_eq!(output.len(), 1);
        assert_eq!(output[0].0, ObjectId(1));
        assert_eq!(native.world().btech, lua.world().btech);
        assert_eq!(
            lua.world().btech.constructed_units()[&subject].facing(),
            Facing {
                torso,
                arms_flipped: flipped
            }
        );
        world = native.world().clone();
        if torso != Torso::Center {
            let direction = if torso == Torso::Left {
                "left"
            } else {
                "right"
            };
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.rottorso({},1,'{direction}')",
                    subject.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, world.btech);
            assert!(messages(&lua).is_empty());
        }
    }
    for guard in ["prone", "off"] {
        let mut rejected = world.clone();
        let mut state = serde_json::to_value(&rejected.btech).unwrap();
        if guard == "prone" {
            state["constructed"][subject.0.to_string()]["posture"] = "prone".into();
        } else {
            state["constructed"][subject.0.to_string()]["power"] =
                serde_json::json!({"state":"off"});
        }
        rejected.btech = serde_json::from_value(state).unwrap();
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(rejected.clone())),
        )
        .unwrap();
        for operation in [
            format!("btech.unit.rottorso({},1,'left')", subject.0),
            format!("btech.unit.fliparms({},1)", subject.0),
        ] {
            assert!(scripts.eval_callback::<()>(&operation).is_err());
            assert_eq!(scripts.world().btech, rejected.btech);
            assert!(messages(&scripts).is_empty());
        }
    }
}

/// Speed/heading confirmations reach cockpit occupants identically through native and Lua commands.
#[tokio::test]
async fn movement_control_feedback_routes_cockpit_and_replays_transactionally() {
    use stompymux_rs::*;
    let (_dir, config, mut world, subject) = fixture('.').await;
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let passenger = world.create(&config, "Control passenger".into(), Kind::Player);
    world.objects.get_mut(&passenger).unwrap().location = Some(subject);
    world
        .objects
        .get_mut(&passenger)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let outsider = world.create(&config, "Control outsider".into(), Kind::Player);
    world.objects.get_mut(&outsider).unwrap().location = Some(
        world.btech.constructed_units()[&subject]
            .position()
            .unwrap()
            .map,
    );
    world
        .objects
        .get_mut(&outsider)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let messages = |scripts: &Scripts| {
        scripts
            .drain_outbox()
            .into_iter()
            .map(|(id, text)| (id, text.source().to_owned()))
            .collect::<Vec<_>>()
    };
    let maximum = world.btech.constructed_units()[&subject]
        .mobility()
        .maximum_speed;
    let walking = world.btech.constructed_units()[&subject]
        .mobility()
        .walking_speed;
    for (command, method, value, text) in [
        (
            "speed walk".to_owned(),
            "speed",
            walking,
            format!("Desired speed changed to {} KPH.", walking as i32),
        ),
        (
            "speed run".to_owned(),
            "speed",
            maximum,
            format!("Desired speed changed to {} KPH.", maximum as i32),
        ),
        (
            "speed back".to_owned(),
            "speed",
            -walking,
            format!("Desired speed changed to {} KPH.", (-walking) as i32),
        ),
        (
            "speed 10.125".to_owned(),
            "speed",
            10.125,
            "Desired speed changed to 10 KPH.".to_owned(),
        ),
        (
            "speed -10.125".to_owned(),
            "speed",
            -10.125,
            "Desired speed changed to -10 KPH.".to_owned(),
        ),
        (
            "speed stop".to_owned(),
            "speed",
            0.0,
            "Desired speed changed to 0 KPH.".to_owned(),
        ),
        (
            "heading -90".to_owned(),
            "heading",
            -90.0,
            "Desired heading: 270.0 degrees.".to_owned(),
        ),
        (
            "heading 720".to_owned(),
            "heading",
            720.0,
            "Desired heading: 0.0 degrees.".to_owned(),
        ),
        (
            "heading 0".to_owned(),
            "heading",
            0.0,
            "Desired heading: 0.0 degrees.".to_owned(),
        ),
    ] {
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for player in [ObjectId(1), passenger, outsider] {
            restored
                .objects
                .get_mut(&player)
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        let call = format!("btech.unit.{method}({},1,{value})", subject.0);
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort control')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        assert!(messages(&lua).is_empty());
        for invalid in [
            format!("btech.unit.speed({},1,math.huge)", subject.0),
            format!("btech.unit.heading({},1,0/0)", subject.0),
            format!("btech.unit.speed({},{},0)", subject.0, passenger.0),
        ] {
            lua.eval_callback::<()>(&format!("assert(not pcall(function() {invalid} end))"))
                .unwrap();
            assert_eq!(lua.world().btech, world.btech);
            assert!(messages(&lua).is_empty());
        }
        commands::run(&native, &config, ObjectId(1), 1, &command).unwrap();
        assert!(
            lua.eval_callback::<bool>(&format!("return {call}"))
                .unwrap()
        );
        let output = messages(&native);
        assert_eq!(messages(&lua), output);
        assert_eq!(output, vec![(ObjectId(1), text.clone()), (passenger, text)]);
        assert_eq!(native.world().btech, lua.world().btech);
        let before_dice = serde_json::to_value(&world.btech.constructed_units()[&subject]).unwrap()
            ["dice"]
            .clone();
        world = native.world().clone();
        assert_eq!(
            serde_json::to_value(&world.btech.constructed_units()[&subject]).unwrap()["dice"],
            before_dice
        );
    }
}

/// Stagger and falling feedback reach observers once per check and survive saved cadence replay.
#[tokio::test]
async fn stagger_observers_cover_success_failure_and_saved_replay() {
    use stompymux_rs::*;
    let (_dir, config, mut base, subject, observer) = lock_fixture().await;
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][observer.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    base.btech = serde_json::from_value(state).unwrap();
    refresh_battle_contacts(&mut base, &[observer]).unwrap();
    assert!(
        visible_battle_contact(&base, observer, subject)
            .unwrap()
            .is_some()
    );
    // Any twenty or more points in one turn force a single check (Total Warfare p. 68).
    for turn_damage in [20, 100] {
        for success in [false, true] {
            for visible in [false, true] {
                let mut world = base.clone();
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["constructed"][subject.0.to_string()]["stagger"]["turn_damage"] =
                    turn_damage.into();
                if !visible {
                    state["constructed"][observer.0.to_string()]["power"] =
                        serde_json::json!({"state":"off"});
                }
                world.btech = serde_json::from_value(state).unwrap();
                let rules = stagger_rules();
                let seed = (0..=255)
                    .find(|seed| Dice::seeded([*seed; 32]).two_d6() == if success { 12 } else { 2 })
                    .unwrap();
                shot_seed(&mut world, subject, seed);
                let mut expected = Vec::new();
                if !success {
                    expected.extend(battle_observer_messages(
                        &world,
                        subject,
                        "falls down, staggered by the damage!",
                    ));
                }
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                let reports = advance_battle_stagger(&mut world, rules).unwrap();
                assert_eq!(
                    advance_battle_stagger(&mut restored, rules).unwrap(),
                    reports
                );
                assert_eq!(world.btech, restored.btech);
                assert_eq!(reports.len(), 1);
                let report = &reports[0];
                assert_eq!(report.level, 1);
                assert_eq!(report.check.success, success);
                assert_eq!(report.fall.is_some(), !success);
                let observed: Vec<_> = report
                    .notices
                    .iter()
                    .filter(|notice| notice.unit == observer)
                    .map(|notice| (notice.unit, notice.text.clone()))
                    .collect();
                assert_eq!(observed, expected);
                assert_eq!(report.notices[0].unit, subject);
                assert_eq!(report.notices[0].text, "You stagger from the damage!");
                assert!(
                    advance_battle_stagger(&mut world, rules)
                        .unwrap()
                        .is_empty()
                );
            }
        }
    }
}

/// Immediate balance losses report their cause once, before fall damage changes visibility.
#[tokio::test]
async fn critical_balance_observers_cover_ground_causes_and_saved_replay() {
    use stompymux_rs::*;
    let (_dir, config, mut base, subject, observer) = lock_fixture().await;
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][observer.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    base.btech = serde_json::from_value(state).unwrap();
    refresh_battle_contacts(&mut base, &[observer]).unwrap();
    assert!(
        visible_battle_contact(&base, observer, subject)
            .unwrap()
            .is_some()
    );
    for cause in [
        "hip",
        "upper",
        "lower",
        "foot",
        "gyro",
        "destroyed_gyro",
        "leg",
    ] {
        for success in [false, true] {
            for visible in [false, true] {
                let mut world = base.clone();
                if cause == "destroyed_gyro" {
                    destroy_battle_critical(
                        &mut world,
                        subject,
                        CriticalLocation {
                            section: MechSection::CenterTorso,
                            slot: 3,
                        },
                    )
                    .unwrap();
                }
                if !visible {
                    let mut state = serde_json::to_value(&world.btech).unwrap();
                    state["constructed"][observer.0.to_string()]["power"] =
                        serde_json::json!({"state":"off"});
                    world.btech = serde_json::from_value(state).unwrap();
                }
                let gyro = matches!(cause, "gyro" | "destroyed_gyro");
                let forced = matches!(cause, "leg" | "destroyed_gyro");
                let section = if gyro {
                    MechSection::CenterTorso
                } else {
                    MechSection::LeftLeg
                };
                let slots = world.btech.constructed_units()[&subject].critical_candidates(section);
                let seed = (0..=255)
                    .find(|seed| {
                        let mut dice = Dice::seeded([*seed; 32]);
                        dice.two_d6(); // Material entry.
                        if cause == "leg" {
                            return dice.two_d6() < 8;
                        }
                        if !matches!(dice.two_d6(), 8 | 9) {
                            return false;
                        }
                        let selected =
                            slots[usize::from(dice.die(slots.len() as u16).unwrap() - 1)].slot;
                        let matched = if gyro {
                            (3..=6).contains(&selected)
                        } else {
                            selected
                                == match cause {
                                    "hip" => 0,
                                    "upper" => 1,
                                    "lower" => 2,
                                    _ => 3,
                                }
                        };
                        matched
                            && (forced
                                || (dice.two_d6()
                                    >= if gyro {
                                        9
                                    } else if cause == "hip" {
                                        8
                                    } else {
                                        7
                                    })
                                    == success)
                    })
                    .unwrap();
                shot_seed(&mut world, subject, seed);
                let will_fall = forced || !success;
                let expected = if will_fall {
                    battle_observer_messages(
                        &world,
                        subject,
                        match cause {
                            "leg" => "crashes to the ground!",
                            "destroyed_gyro" => "is knocked over!",
                            "gyro" => "stumbles and falls down.",
                            _ => "stumbles and falls down!",
                        },
                    )
                } else {
                    Vec::new()
                };
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                let hit = balance_hit(section, cause != "leg");
                let damage = if cause == "leg" { 14 } else { 1 };
                let report =
                    resolve_battle_tactical_impact(&mut world, subject, hit, damage, fall_rules())
                        .unwrap();
                assert_eq!(
                    resolve_battle_tactical_impact(
                        &mut restored,
                        subject,
                        hit,
                        damage,
                        fall_rules()
                    )
                    .unwrap(),
                    report
                );
                assert_eq!(world.btech, restored.btech);
                assert_eq!(report.balance.len(), 1, "{cause}");
                let check = report.balance[0].check;
                let checks: Vec<_> = check
                    .into_iter()
                    .chain(
                        report.balance[0]
                            .fall
                            .as_ref()
                            .and_then(|fall| fall.avoidance),
                    )
                    .filter(|check| check.roll.is_some())
                    .collect();
                assert_eq!(report.pilot_notices.len(), checks.len() * 2, "{cause}");
                for (pair, check) in report.pilot_notices.as_chunks::<2>().0.iter().zip(&checks) {
                    assert_eq!(pair[0].text, "You make a piloting skill roll!");
                    assert_eq!(
                        pair[1].text,
                        format!(
                            "Modified Pilot Skill: BTH {}\tRoll: {}",
                            check.target,
                            check.roll.unwrap()
                        )
                    );
                    assert_eq!(pair[0].before_notice, pair[1].before_notice);
                    assert_eq!(pair[0].pilot, pair[1].pilot);
                }
                if let Some(check) = check.filter(|check| check.roll.is_some()) {
                    let feedback = &report.pilot_notices;
                    assert_eq!(feedback[0].pilot, report.balance[0].pilot.unwrap());
                    assert_eq!(feedback[0].text, "You make a piloting skill roll!");
                    assert_eq!(
                        feedback[1].text,
                        format!(
                            "Modified Pilot Skill: BTH {}\tRoll: {}",
                            check.target,
                            check.roll.unwrap()
                        )
                    );
                    assert_eq!(feedback[0].before_notice, feedback[1].before_notice);
                    if will_fall {
                        assert_eq!(
                            report.notices[feedback[0].before_notice].text,
                            "You lose your balance and fall down!"
                        );
                    }
                }

                assert_eq!(report.balance[0].fall.is_some(), will_fall, "{cause}");
                let observed: Vec<_> = report
                    .notices
                    .iter()
                    .filter(|notice| {
                        notice.unit == observer
                            && !notice.text.contains("locks into place")
                            && !notice.text.contains("twists in an odd way")
                            && !notice.text.contains("screech")
                    })
                    .map(|notice| (notice.unit, notice.text.clone()))
                    .collect();
                assert_eq!(observed, expected, "{cause}");
                assert_eq!(
                    report.balance[0]
                        .observer_notices
                        .iter()
                        .map(|notice| (notice.unit, notice.text.clone()))
                        .collect::<Vec<_>>(),
                    expected
                );
            }
        }
    }
}

/// Engine smoke uses pre-critical running state, including the hit that destroys the engine.
#[tokio::test]
async fn engine_smoke_observers_preserve_fatal_hit_visibility_and_replay() {
    use stompymux_rs::*;
    let (_dir, config, mut base, subject, observer) = lock_fixture().await;
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][observer.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    base.btech = serde_json::from_value(state).unwrap();
    refresh_battle_contacts(&mut base, &[observer]).unwrap();
    assert!(
        visible_battle_contact(&base, observer, subject)
            .unwrap()
            .is_some()
    );
    for prior_hits in 0..3 {
        for running in [false, true] {
            for visible in [false, true] {
                let mut world = base.clone();
                for slot in 0..prior_hits {
                    destroy_battle_critical(
                        &mut world,
                        subject,
                        CriticalLocation {
                            section: MechSection::CenterTorso,
                            slot,
                        },
                    )
                    .unwrap();
                }
                let mut state = serde_json::to_value(&world.btech).unwrap();
                if !running {
                    state["constructed"][subject.0.to_string()]["power"] =
                        serde_json::json!({"state":"off"});
                }
                if !visible {
                    state["constructed"][observer.0.to_string()]["power"] =
                        serde_json::json!({"state":"off"});
                }
                world.btech = serde_json::from_value(state).unwrap();
                let slots = world.btech.constructed_units()[&subject]
                    .critical_candidates(MechSection::CenterTorso);
                let engines: Vec<_> = world.btech.constructed_units()[&subject]
                    .loadout()
                    .unwrap()
                    .systems
                    .into_iter()
                    .filter(|part| part.system == System::Engine)
                    .map(|part| part.location)
                    .collect();
                let seed = (0..=255)
                    .find(|seed| {
                        let mut dice = Dice::seeded([*seed; 32]);
                        dice.two_d6(); // Material entry.
                        matches!(dice.two_d6(), 8 | 9)
                            && engines.contains(
                                &slots[usize::from(dice.die(slots.len() as u16).unwrap() - 1)],
                            )
                    })
                    .unwrap();
                shot_seed(&mut world, subject, seed);
                let expected = if running {
                    battle_observer_messages(&world, subject, "'s Center Torso spews black smoke!")
                } else {
                    Vec::new()
                };
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                let hit = balance_hit(MechSection::CenterTorso, true);
                let report =
                    resolve_battle_tactical_impact(&mut world, subject, hit, 1, fall_rules())
                        .unwrap();
                assert_eq!(
                    resolve_battle_tactical_impact(&mut restored, subject, hit, 1, fall_rules())
                        .unwrap(),
                    report
                );
                assert_eq!(world.btech, restored.btech);
                let observed: Vec<_> = report
                    .notices
                    .iter()
                    .filter(|notice| notice.unit == observer)
                    .map(|notice| (notice.unit, notice.text.clone()))
                    .collect();
                assert_eq!(observed, expected);
                assert_eq!(
                    world.btech.constructed_units()[&subject].is_destroyed(),
                    prior_hits == 2
                );
                let lost = report.impact.criticals[0].0;
                let before = world.btech.clone();
                assert!(
                    destroy_battle_critical(&mut world, subject, lost)
                        .unwrap()
                        .is_none()
                );
                assert_eq!(world.btech, before);
            }
        }
    }
}

/// Mechanical damage broadcasts depend on power, limb type and the original hip's availability.
async fn actuator_observers_matrix(sections: &[stompymux_rs::MechSection]) {
    use stompymux_rs::*;
    let (_dir, config, mut base, subject, observer) = lock_fixture().await;
    balance_skill(&mut base);
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][observer.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    base.btech = serde_json::from_value(state).unwrap();
    refresh_battle_contacts(&mut base, &[observer]).unwrap();
    assert!(
        visible_battle_contact(&base, observer, subject)
            .unwrap()
            .is_some()
    );
    for section in sections.iter().copied() {
        for slot in 0..4 {
            let location = CriticalLocation { section, slot };
            if !base.btech.constructed_units()[&subject]
                .loadout()
                .unwrap()
                .systems
                .iter()
                .any(|part| part.location == location)
            {
                continue;
            }
            for hip_lost in [false, true] {
                if hip_lost
                    && (slot == 0
                        || matches!(section, MechSection::LeftArm | MechSection::RightArm))
                {
                    continue;
                }
                for (running, visible) in [(true, true), (false, true), (true, false)] {
                    let mut world = base.clone();
                    if hip_lost {
                        destroy_battle_critical(
                            &mut world,
                            subject,
                            CriticalLocation { section, slot: 0 },
                        )
                        .unwrap();
                    }
                    let mut state = serde_json::to_value(&world.btech).unwrap();
                    if !running {
                        state["constructed"][subject.0.to_string()]["power"] =
                            serde_json::json!({"state":"off"});
                    }
                    if !visible {
                        state["constructed"][observer.0.to_string()]["power"] =
                            serde_json::json!({"state":"off"});
                    }
                    world.btech = serde_json::from_value(state).unwrap();
                    let candidates =
                        world.btech.constructed_units()[&subject].critical_candidates(section);
                    let selected = candidates
                        .iter()
                        .position(|candidate| *candidate == location)
                        .unwrap();
                    shot_seed(
                        &mut world,
                        subject,
                        single_critical_seed(candidates.len() as u16, (selected + 1) as u16),
                    );
                    let expected = if running
                        && !matches!(section, MechSection::LeftArm | MechSection::RightArm)
                        && !hip_lost
                    {
                        battle_observer_messages(
                            &world,
                            subject,
                            &if slot == 0 {
                                "'s hip locks into place!".to_owned()
                            } else {
                                format!(
                                    "'s {} twists in an odd way!",
                                    section.name().replace('_', " ")
                                )
                            },
                        )
                    } else {
                        Vec::new()
                    };
                    persistence::save(&config.database(), &world).await.unwrap();
                    let mut restored = persistence::load(&config.database()).await.unwrap();
                    restored
                        .objects
                        .get_mut(&ObjectId(1))
                        .unwrap()
                        .flags
                        .insert(Flag::Connected);
                    let hit = balance_hit(section, true);
                    let report =
                        resolve_battle_tactical_impact(&mut world, subject, hit, 1, fall_rules())
                            .unwrap();
                    assert_eq!(
                        resolve_battle_tactical_impact(
                            &mut restored,
                            subject,
                            hit,
                            1,
                            fall_rules()
                        )
                        .unwrap(),
                        report
                    );
                    assert_eq!(world.btech, restored.btech);
                    assert_eq!(report.impact.criticals[0].0, location);
                    let leg = matches!(section, MechSection::LeftLeg | MechSection::RightLeg);
                    let cockpit = match (leg, slot) {
                        (true, 0) => "Your hip takes a direct hit and freezes up!".to_owned(),
                        (true, _) => "One of your leg actuators is destroyed!".to_owned(),
                        (false, 0) => "Your shoulder joint takes a hit and is frozen!".to_owned(),
                        (false, _) => {
                            let side = if section == MechSection::LeftArm {
                                "left"
                            } else {
                                "right"
                            };
                            let part = match slot {
                                1 => "upper arm",
                                2 => "lower arm",
                                _ => "hand",
                            };
                            format!("Your {side} {part} actuator is destroyed!")
                        }
                    };
                    assert_eq!(
                        report
                            .notices
                            .iter()
                            .filter(|notice| notice.unit == subject && notice.text == cockpit)
                            .count(),
                        1
                    );
                    if let Some((_, text)) = expected.first() {
                        let own = report
                            .notices
                            .iter()
                            .position(|notice| notice.unit == subject && notice.text == cockpit)
                            .unwrap();
                        let observer_position = report
                            .notices
                            .iter()
                            .position(|notice| notice.unit == observer && notice.text == *text)
                            .unwrap();
                        assert_eq!(own > observer_position, slot == 0);
                    }
                    let observed: Vec<_> = report
                        .notices
                        .iter()
                        .filter(|notice| {
                            notice.unit == observer
                                && (notice.text.contains("locks into place")
                                    || notice.text.contains("twists in an odd way"))
                        })
                        .map(|notice| (notice.unit, notice.text.clone()))
                        .collect();
                    assert_eq!(
                        observed, expected,
                        "{section:?}, slot {slot}, hip lost {hip_lost}, running {running}, visible {visible}"
                    );
                    let before = world.btech.clone();
                    assert!(
                        destroy_battle_critical(&mut world, subject, location)
                            .unwrap()
                            .is_none()
                    );
                    assert_eq!(world.btech, before);
                }
            }
        }
    }
}

#[tokio::test]
async fn actuator_observers_cover_limb_power_hip_guards_and_saved_replay_left_leg() {
    use stompymux_rs::MechSection;
    actuator_observers_matrix(&[MechSection::LeftLeg]).await;
}

#[tokio::test]
async fn actuator_observers_cover_limb_power_hip_guards_and_saved_replay_right_leg() {
    use stompymux_rs::MechSection;
    actuator_observers_matrix(&[MechSection::RightLeg]).await;
}

#[tokio::test]
async fn actuator_observers_cover_limb_power_hip_guards_and_saved_replay_left_arm() {
    use stompymux_rs::MechSection;
    actuator_observers_matrix(&[MechSection::LeftArm]).await;
}

#[tokio::test]
async fn actuator_observers_cover_limb_power_hip_guards_and_saved_replay_right_arm() {
    use stompymux_rs::MechSection;
    actuator_observers_matrix(&[MechSection::RightArm]).await;
}

/// Core component messages follow effective damage stages even without engine power.
#[tokio::test]
async fn core_critical_cockpit_feedback_stages_and_saved_replay() {
    use stompymux_rs::*;
    let (_dir, config, mut base, subject, _) = lock_fixture().await;
    balance_skill(&mut base);
    for (system, messages) in [
        (
            System::Engine,
            vec![
                "Your engine shielding takes a hit! It's getting hotter in here!",
                "Your engine shielding takes a hit! It's getting hotter in here!",
                "Your engine is destroyed!",
            ],
        ),
        (
            System::Gyro,
            vec![
                "Your Gyro has been damaged!",
                "Your Gyro has been destroyed!",
                "Your destroyed gyro takes another hit!",
                "Your destroyed gyro takes another hit!",
            ],
        ),
        (
            System::Sensors,
            vec![
                "Your sensors have been damaged!",
                "Your sensors have been destroyed!",
            ],
        ),
        (
            System::LifeSupport,
            vec![
                "Your life support has been destroyed!",
                "Your life support has been destroyed!",
            ],
        ),
    ] {
        let locations: Vec<_> = base.btech.constructed_units()[&subject]
            .loadout()
            .unwrap()
            .systems
            .iter()
            .filter(|part| part.system == system)
            .map(|part| part.location)
            .collect();
        for (prior, expected) in messages.into_iter().enumerate() {
            for running in [false, true] {
                let mut world = base.clone();
                for location in &locations[..prior] {
                    destroy_battle_critical(&mut world, subject, *location).unwrap();
                }
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["constructed"][subject.0.to_string()]["posture"] = "prone".into();
                if !running {
                    state["constructed"][subject.0.to_string()]["power"] =
                        serde_json::json!({"state":"off"});
                }
                world.btech = serde_json::from_value(state).unwrap();
                let location = locations[prior];
                let candidates =
                    world.btech.constructed_units()[&subject].critical_candidates(location.section);
                let selected = candidates
                    .iter()
                    .position(|candidate| *candidate == location)
                    .unwrap();
                shot_seed(
                    &mut world,
                    subject,
                    single_critical_seed(candidates.len() as u16, (selected + 1) as u16),
                );
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                restored
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
                let hit = balance_hit(location.section, true);
                let report =
                    resolve_battle_tactical_impact(&mut world, subject, hit, 1, fall_rules())
                        .unwrap();
                assert_eq!(
                    resolve_battle_tactical_impact(&mut restored, subject, hit, 1, fall_rules())
                        .unwrap(),
                    report
                );
                assert_eq!(world.btech, restored.btech);
                assert_eq!(report.impact.criticals[0].0, location);
                assert_eq!(
                    report
                        .notices
                        .iter()
                        .filter(|notice| notice.unit == subject && notice.text == expected)
                        .count(),
                    1,
                    "{system:?}, prior {prior}, running {running}"
                );
                let before = world.btech.clone();
                assert!(
                    destroy_battle_critical(&mut world, subject, location)
                        .unwrap()
                        .is_none()
                );
                assert_eq!(world.btech, before);
            }
        }
    }
}

/// Gyro observer cues use pre-loss visibility and distinguish hardened protection from damage.
#[tokio::test]
async fn gyro_observers_cover_protection_power_and_saved_replay() {
    use stompymux_rs::*;
    let (_dir, config, mut base, subject, observer) = lock_fixture().await;
    balance_skill(&mut base);
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][observer.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    state["constructed"][subject.0.to_string()]["posture"] = "prone".into();
    base.btech = serde_json::from_value(state).unwrap();
    refresh_battle_contacts(&mut base, &[observer]).unwrap();
    assert!(
        visible_battle_contact(&base, observer, subject)
            .unwrap()
            .is_some()
    );
    let slots: Vec<_> = base.btech.constructed_units()[&subject]
        .loadout()
        .unwrap()
        .systems
        .iter()
        .filter(|part| part.system == System::Gyro)
        .map(|part| part.location)
        .collect();
    for hardened in [false, true] {
        for prior in 0..4 {
            for running in [false, true] {
                for visible in [false, true] {
                    let mut world = base.clone();
                    let mut state = serde_json::to_value(&world.btech).unwrap();
                    if hardened {
                        let record = &mut state["constructed"][subject.0.to_string()];
                        record["definition"]["attributes"]["specials"] = "HDGYRO".into();
                        // The gyro's three extra tons come out of the weapons.
                        crate::support::templates::book_legal_record_keeping(
                            record,
                            Some(MechSection::Head),
                        );
                    }
                    if !running {
                        state["constructed"][subject.0.to_string()]["power"] =
                            serde_json::json!({"state":"off"});
                    }
                    if !visible {
                        state["constructed"][observer.0.to_string()]["power"] =
                            serde_json::json!({"state":"off"});
                    }
                    world.btech = serde_json::from_value(state).unwrap();
                    for slot in &slots[..prior] {
                        destroy_battle_critical(&mut world, subject, *slot).unwrap();
                    }
                    let location = slots[prior];
                    let candidates = world.btech.constructed_units()[&subject]
                        .critical_candidates(location.section);
                    let selected = candidates
                        .iter()
                        .position(|candidate| *candidate == location)
                        .unwrap();
                    shot_seed(
                        &mut world,
                        subject,
                        single_critical_seed(candidates.len() as u16, (selected + 1) as u16),
                    );
                    let text = if hardened && prior == 0 {
                        Some("emits a screech as its hardened gyro buckles slightly!")
                    } else if running && prior == usize::from(hardened) {
                        Some("emits a loud screech as its gyro buckles under the impact!")
                    } else {
                        None
                    };
                    let expected = text
                        .map(|text| battle_observer_messages(&world, subject, text))
                        .unwrap_or_default();
                    persistence::save(&config.database(), &world).await.unwrap();
                    let mut restored = persistence::load(&config.database()).await.unwrap();
                    restored
                        .objects
                        .get_mut(&ObjectId(1))
                        .unwrap()
                        .flags
                        .insert(Flag::Connected);
                    let hit = balance_hit(location.section, true);
                    let report =
                        resolve_battle_tactical_impact(&mut world, subject, hit, 1, fall_rules())
                            .unwrap();
                    assert_eq!(
                        resolve_battle_tactical_impact(
                            &mut restored,
                            subject,
                            hit,
                            1,
                            fall_rules()
                        )
                        .unwrap(),
                        report
                    );
                    assert_eq!(world.btech, restored.btech);
                    assert_eq!(report.impact.criticals[0].0, location);
                    let observed: Vec<_> = report
                        .notices
                        .iter()
                        .filter(|notice| notice.unit == observer)
                        .map(|notice| (notice.unit, notice.text.clone()))
                        .collect();
                    assert_eq!(
                        observed, expected,
                        "hardened {hardened}, prior {prior}, running {running}, visible {visible}"
                    );
                    if !expected.is_empty() {
                        assert_eq!(report.notices[0].unit, observer);
                    }
                    let before = world.btech.clone();
                    assert!(
                        destroy_battle_critical(&mut world, subject, location)
                            .unwrap()
                            .is_none()
                    );
                    assert_eq!(world.btech, before);
                }
            }
        }
    }
}

/// Fatal cockpit criticals publish occupant and observer cues with the same durable unit loss.
#[tokio::test]
async fn cockpit_destruction_feedback_power_visibility_and_saved_replay() {
    use stompymux_rs::*;
    let (_dir, config, mut base, subject, observer) = lock_fixture().await;
    balance_skill(&mut base);
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][observer.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    base.btech = serde_json::from_value(state).unwrap();
    refresh_battle_contacts(&mut base, &[observer]).unwrap();
    assert!(
        visible_battle_contact(&base, observer, subject)
            .unwrap()
            .is_some()
    );
    let location = base.btech.constructed_units()[&subject]
        .loadout()
        .unwrap()
        .systems
        .iter()
        .find(|part| part.system == System::Cockpit)
        .unwrap()
        .location;
    for running in [false, true] {
        for visible in [false, true] {
            let mut world = base.clone();
            let mut state = serde_json::to_value(&world.btech).unwrap();
            if !running {
                state["constructed"][subject.0.to_string()]["power"] =
                    serde_json::json!({"state":"off"});
            }
            if !visible {
                state["constructed"][observer.0.to_string()]["power"] =
                    serde_json::json!({"state":"off"});
            }
            world.btech = serde_json::from_value(state).unwrap();
            let candidates =
                world.btech.constructed_units()[&subject].critical_candidates(location.section);
            let selected = candidates
                .iter()
                .position(|candidate| *candidate == location)
                .unwrap();
            shot_seed(
                &mut world,
                subject,
                single_critical_seed(candidates.len() as u16, (selected + 1) as u16),
            );
            let expected = battle_observer_messages(
                &world,
                subject,
                "spasms for a second then remains oddly still.",
            );
            persistence::save(&config.database(), &world).await.unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            restored
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .insert(Flag::Connected);
            let hit = balance_hit(location.section, true);
            let mut rejected = world.clone();
            rejected
                .objects
                .get_mut(&subject)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            let before = rejected.btech.clone();
            assert!(
                resolve_battle_tactical_impact(&mut rejected, subject, hit, 1, fall_rules())
                    .is_err()
            );
            assert_eq!(rejected.btech, before);
            let report =
                resolve_battle_tactical_impact(&mut world, subject, hit, 1, fall_rules()).unwrap();
            assert_eq!(
                resolve_battle_tactical_impact(&mut restored, subject, hit, 1, fall_rules())
                    .unwrap(),
                report
            );
            assert_eq!(world.btech, restored.btech);
            assert_eq!(report.impact.criticals[0].0, location);
            let own = report.notices.iter().position(|notice| notice.unit == subject && notice.text == "Your cockpit is destroyed, your blood boils, and your body is fried! [fg=yellow]You're dead![reset]").unwrap();
            let observed: Vec<_> = report
                .notices
                .iter()
                .filter(|notice| notice.unit == observer)
                .map(|notice| (notice.unit, notice.text.clone()))
                .collect();
            assert_eq!(observed, expected);
            if visible {
                assert!(
                    own < report
                        .notices
                        .iter()
                        .position(|notice| notice.unit == observer)
                        .unwrap()
                );
            }
            let unit = &world.btech.constructed_units()[&subject];
            assert!(unit.is_destroyed());
            assert_eq!(unit.power(), Power::Off);
            assert_eq!(unit.pilot(), None);
            assert!(world.objects.contains_key(&ObjectId(1)));
            let before = world.btech.clone();
            assert!(
                destroy_battle_critical(&mut world, subject, location)
                    .unwrap()
                    .is_none()
            );
            assert_eq!(world.btech, before);
        }
    }
}

/// Equipment feedback follows newly lost slots, including broken mounts and grouped sinks.
#[tokio::test]
async fn equipment_loss_feedback_and_saved_replay() {
    use stompymux_rs::*;
    let (_dir, config, mut base, subject, observer) = lock_fixture().await;
    balance_skill(&mut base);
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][observer.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    base.btech = serde_json::from_value(state).unwrap();
    refresh_battle_contacts(&mut base, &[observer]).unwrap();
    for case in ["weapon", "broken_weapon", "sink", "double_sink", "jet"] {
        for running in [false, true] {
            for visible in [false, true] {
                let mut world = base.clone();
                let mut definition = world.btech.constructed_units()[&subject]
                    .definition()
                    .clone();
                if case.contains("weapon") {
                    let arm = definition.sections.get_mut(&MechSection::LeftArm).unwrap();
                    for slot in [2, 3] {
                        arm.criticals.get_mut(&slot).unwrap().equipment = "IS.LargeLaser".into();
                    }
                }
                if case == "double_sink" {
                    let sink = definition
                        .sections
                        .get_mut(&MechSection::Head)
                        .unwrap()
                        .criticals
                        .remove(&3)
                        .unwrap();
                    definition
                        .attributes
                        .insert("specials".into(), "DoubleHS FlipArms".into());
                    definition.heat_sinks = 12;
                    for slot in 2..8 {
                        definition
                            .sections
                            .get_mut(&MechSection::LeftTorso)
                            .unwrap()
                            .criticals
                            .insert(slot, sink.clone());
                    }
                }
                let mut state = serde_json::to_value(&world.btech).unwrap();
                crate::support::templates::set_record_definition(
                    &mut state["constructed"][subject.0.to_string()],
                    definition,
                );
                if !running {
                    state["constructed"][subject.0.to_string()]["power"] =
                        serde_json::json!({"state":"off"});
                }
                if !visible {
                    state["constructed"][observer.0.to_string()]["power"] =
                        serde_json::json!({"state":"off"});
                }
                world.btech = serde_json::from_value(state).unwrap();
                let loadout = world.btech.constructed_units()[&subject].loadout().unwrap();
                let (location, text, group) = if case.contains("weapon") {
                    let mount = loadout
                        .weapons
                        .iter()
                        .find(|mount| mount.criticals.len() > 1)
                        .unwrap();
                    let name = mount.weapon.name().split_once('.').unwrap().1;
                    if case == "broken_weapon" {
                        destroy_battle_critical(&mut world, subject, mount.criticals[0]).unwrap();
                        (
                            mount.criticals[1],
                            format!("Part of your non-working {name} has been hit!"),
                            Vec::new(),
                        )
                    } else {
                        (
                            mount.criticals[0],
                            format!("Your {name} has been destroyed!!"),
                            Vec::new(),
                        )
                    }
                } else {
                    let location = loadout
                        .systems
                        .iter()
                        .find(|part| {
                            part.system
                                == if case == "jet" {
                                    System::JumpJet
                                } else {
                                    System::HeatSink
                                }
                        })
                        .unwrap()
                        .location;
                    let group = if case == "double_sink" {
                        (2..5)
                            .map(|slot| CriticalLocation {
                                section: MechSection::LeftTorso,
                                slot,
                            })
                            .collect()
                    } else {
                        vec![location]
                    };
                    (
                        location,
                        if case == "jet" {
                            "One of your jump jet engines has shut down!"
                        } else {
                            "You lost a heat sink!"
                        }
                        .to_owned(),
                        group,
                    )
                };
                let candidates =
                    world.btech.constructed_units()[&subject].critical_candidates(location.section);
                let selected = candidates
                    .iter()
                    .position(|candidate| *candidate == location)
                    .unwrap();
                if case == "weapon" {
                    // This scenario checks destruction feedback, so select the destructive table outcome too.
                    let seed = (0u32..100_000)
                        .find_map(|value| {
                            let mut bytes = [0; 32];
                            bytes[..4].copy_from_slice(&value.to_le_bytes());
                            let mut dice = stompymux_rs::Dice::seeded(bytes);
                            dice.two_d6(); // Material entry.
                            (matches!(dice.two_d6(), 8 | 9)
                                && dice.die(candidates.len() as u16).unwrap()
                                    == (selected + 1) as u16
                                && dice.two_d6() >= 9)
                                .then_some(bytes)
                        })
                        .expect("one selected critical with a destructive weapon result");
                    world
                        .btech
                        .set_unit_dice(subject, Dice::seeded(seed))
                        .unwrap();
                } else {
                    shot_seed(
                        &mut world,
                        subject,
                        single_critical_seed(candidates.len() as u16, (selected + 1) as u16),
                    );
                }
                let expected = if case == "jet" && running {
                    battle_observer_messages(
                        &world,
                        subject,
                        &format!(
                            "'s {} flares as superheated plasma spews out!",
                            location.section.name().replace('_', " ")
                        ),
                    )
                } else if case.contains("sink") {
                    battle_observer_messages(
                        &world,
                        subject,
                        &format!(
                            "'s {} is covered in a green mist!",
                            location.section.name().replace('_', " ")
                        ),
                    )
                } else {
                    Vec::new()
                };
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                restored
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
                let hit = balance_hit(location.section, true);
                let report =
                    resolve_battle_tactical_impact(&mut world, subject, hit, 1, fall_rules())
                        .unwrap();
                assert_eq!(
                    resolve_battle_tactical_impact(&mut restored, subject, hit, 1, fall_rules())
                        .unwrap(),
                    report
                );
                assert_eq!(world.btech, restored.btech);
                assert_eq!(report.impact.criticals[0].0, location);
                assert_eq!(
                    report
                        .notices
                        .iter()
                        .filter(|notice| notice.unit == subject && notice.text == text)
                        .count(),
                    1,
                    "{case}"
                );
                let observed: Vec<_> = report
                    .notices
                    .iter()
                    .filter(|notice| notice.unit == observer)
                    .map(|notice| (notice.unit, notice.text.clone()))
                    .collect();
                assert_eq!(observed, expected);
                let before = world.btech.clone();
                for lost in group.iter().chain(std::iter::once(&location)) {
                    assert!(
                        destroy_battle_critical(&mut world, subject, *lost)
                            .unwrap()
                            .is_none()
                    );
                }
                assert_eq!(world.btech, before);
            }
        }
    }
}

/// Fueled flamers share heat mode while retaining ballistic skill, fuel expenditure and replay.
#[tokio::test]
async fn fueled_flamer_modes_native_lua_and_saved_replay() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    shot_skill(&mut base, 0);
    set_battle_character_value(
        &mut base,
        ObjectId(1),
        "Gunnery-Ballistic",
        CharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
    )
    .unwrap();
    let pristine_db = snapshot_database(&config);
    for (weapon, mass, ranges) in [
        (Weapon::HeavyFlamer, 1536, (2, 3, 4)),
        (Weapon::VehicleFlamer, 512, (1, 2, 3)),
    ] {
        let profile = weapon.profile();
        let mut definition = base.btech.constructed_units()[&id].definition().clone();
        definition
            .sections
            .get_mut(&MechSection::LeftArm)
            .unwrap()
            .criticals
            .get_mut(&2)
            .unwrap()
            .equipment = weapon.name().into();
        let bin = definition
            .sections
            .get_mut(&MechSection::RightTorso)
            .unwrap()
            .criticals
            .get_mut(&0)
            .unwrap();
        bin.equipment = format!("Ammo_{}", weapon.name());
        bin.data = profile.ammunition_per_ton.to_string();
        assert_eq!(weapon.gunnery_skill(true), "Gunnery-Ballistic");
        assert_eq!(weapon.mass(), mass);
        // Flamers are not direct-fire weapons (TechManual p. 238).
        assert!(!weapon.supports_targeting_computer());
        assert_eq!(
            weapon.ammunition_explosion_damage(10),
            u32::from(profile.damage) * 10
        );
        assert_eq!(
            (
                weapon.profile().short_range,
                weapon.profile().medium_range,
                weapon.profile().long_range
            ),
            ranges
        );
        for heat in [false, true] {
            for rounds in [0, 1, profile.ammunition_per_ton] {
                let mut world = base.clone();
                let mut definition = definition.clone();
                if heat {
                    definition
                        .sections
                        .get_mut(&MechSection::LeftArm)
                        .unwrap()
                        .criticals
                        .get_mut(&2)
                        .unwrap()
                        .modes
                        .push("Heat".into());
                }
                world
                    .btech
                    .rewrite_unit_record(id, |record| {
                        crate::support::templates::set_record_definition(
                            record,
                            definition.clone(),
                        );
                        record["ammunition"][0] = rounds.into();
                        // The constructed template initializes fire modes; the live scenario uses the same mode.
                        let fresh =
                            Mech::from_template(crate::support::templates::legal(definition))
                                .unwrap();
                        record["fire_modes"] =
                            serde_json::to_value(fresh).unwrap()["fire_modes"].clone();
                    })
                    .unwrap();
                let mode = if heat {
                    FireMode::Heat
                } else {
                    FireMode::Normal
                };
                assert_eq!(
                    world.btech.constructed_units()[&id].fire_mode(0).unwrap(),
                    mode
                );
                let mut toggled = world.clone();
                toggle_battle_flamer_heat(&mut toggled, id, ObjectId(1), 0).unwrap();
                toggle_battle_flamer_heat(&mut toggled, id, ObjectId(1), 0).unwrap();
                assert_eq!(toggled.btech, world.btech);
                restore_database(&config, &pristine_db);
                persistence::save(&config.database(), &world).await.unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                restored
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
                install(&native, world.clone());
                install(&lua, restored);
                assert!(
                    lua.eval_callback::<()>(&format!(
                        "btech.unit.fire({},1,0,{}); error('abort')",
                        id.0, target.0
                    ))
                    .is_err()
                );
                assert_eq!(lua.world().btech, world.btech);
                assert!(lua.drain_outbox().is_empty());
                commands::run(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("fire 0 #{}", target.0),
                )
                .unwrap();
                let result = lua.eval_callback::<mlua::Table>(&format!(
                    "return btech.unit.fire({},1,0,{})",
                    id.0, target.0
                ));
                if rounds == 0 {
                    assert!(result.is_err());
                    assert_eq!(native.world().btech, world.btech);
                    assert_eq!(lua.world().btech, world.btech);
                    continue;
                }
                let result = result.unwrap();
                assert_eq!(native.world().btech, lua.world().btech);
                assert_eq!(native.drain_outbox(), lua.drain_outbox());
                let after = native.world();
                let unit = &after.btech.constructed_units()[&id];
                assert_eq!(unit.ammunition()[0], rounds as u16 - 1);
                assert_eq!(unit.heat().stored, f64::from(profile.heat));
                assert_eq!(
                    serde_json::to_value(unit).unwrap()["weapon_recycle"]["0"],
                    profile.recycle_seconds
                );
                assert_eq!(
                    result.get::<Option<u8>>("heat_transfer").unwrap(),
                    heat.then_some(profile.damage)
                );
                if heat {
                    assert_eq!(
                        after.btech.constructed_units()[&target].sections(),
                        world.btech.constructed_units()[&target].sections()
                    );
                    assert_eq!(
                        after.btech.constructed_units()[&target].heat().stored,
                        f64::from(profile.damage)
                    );
                } else {
                    assert_ne!(
                        after.btech.constructed_units()[&target].sections(),
                        world.btech.constructed_units()[&target].sections()
                    );
                }
            }
        }
    }
}

/// Plasma heat accompanies material hits, remains full on glances, and consumes no target dice on misses.
#[tokio::test]
async fn plasma_hit_boundaries_keep_heat_separate_from_material_damage() {
    use stompymux_rs::*;
    let (_dir, _config, mut base, id, target) = shot_fixture().await;
    shot_skill(&mut base, 4);
    set_battle_character_value(
        &mut base,
        ObjectId(1),
        "Gunnery-Ballistic",
        CharacterValue {
            value: 4,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    let mut definition = base.btech.constructed_units()[&id].definition().clone();
    for slot in [2, 3] {
        definition
            .sections
            .get_mut(&MechSection::LeftArm)
            .unwrap()
            .criticals
            .get_mut(&slot)
            .unwrap()
            .equipment = "IS.PlasmaRifle".into();
    }
    let bin = definition
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap()
        .criticals
        .get_mut(&0)
        .unwrap();
    bin.equipment = "Ammo_IS.PlasmaRifle".into();
    bin.data = "10".into();
    base.btech
        .rewrite_unit_record(id, |record| {
            crate::support::templates::set_record_definition(record, definition);
            record["ammunition"][0] = 10.into();
        })
        .unwrap();
    shot_seed(&mut base, target, 1);
    let rules = ShotRules {
        tsm_tow_bonus: true,
        ..shot_rules()
    };
    let mut probe = base.clone();
    let number = resolve_battle_shot(&mut probe, id, ObjectId(1), target, 0, rules)
        .unwrap()
        .target_number
        .unwrap();
    assert!((3..=11).contains(&number));
    for roll in [number - 1, number, number + 1] {
        let mut world = base.clone();
        let seed = (0..=u8::MAX)
            .find(|seed| i32::from(Dice::seeded([*seed; 32]).two_d6()) == roll)
            .unwrap();
        shot_seed(&mut world, id, seed);
        let report = resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, rules).unwrap();
        assert_eq!(report.heat_transfer, None);
        assert_eq!(report.glancing, roll == number);
        assert_eq!(world.btech.constructed_units()[&id].ammunition()[0], 9);
        assert_eq!(world.btech.constructed_units()[&id].heat().stored, 10.0);
        if roll < number {
            assert!(report.salvo.is_none());
            assert_eq!(
                serde_json::to_value(&world.btech.constructed_units()[&target]).unwrap(),
                serde_json::to_value(&base.btech.constructed_units()[&target]).unwrap()
            );
            continue;
        }
        let salvo = report.salvo.unwrap().into_mech().unwrap();
        assert_eq!(salvo.groups.len(), 1);
        let group = &salvo.groups[0];
        assert_eq!(group.damage, if roll == number { 5 } else { 10 });
        let heat = group.impact.plasma_heat.expect("plasma heat roll");
        assert!((1..=6).contains(&heat));
        assert_eq!(
            world.btech.constructed_units()[&target].heat().stored,
            f64::from(heat)
        );
    }
}

/// Combat warnings are enabled by default, can be muted, and do not alter damage or dice.
#[tokio::test]
async fn armor_warning_thresholds_preferences_and_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id) = fixture('.').await;
    assert!(base.btech.constructed_units()[&id].armor_warning());
    for (section, rear) in [
        (MechSection::LeftArm, false),
        (MechSection::CenterTorso, true),
    ] {
        let original = &base.btech.constructed_units()[&id].definition().sections[&section];
        let total = if rear { original.rear } else { original.armor };
        for remaining in [total / 2, total / 2 - 1, 0] {
            let mut enabled = base.clone();
            let mut muted = base.clone();
            set_battle_armor_warning(&mut muted, id, ObjectId(1), false).unwrap();
            let hit = Hit {
                section,
                rear_armor: rear,
                through_armor_critical: false,
            };
            let report = resolve_battle_tactical_impact(
                &mut enabled,
                id,
                hit,
                total - remaining,
                fall_rules(),
            )
            .unwrap();
            let quiet = resolve_battle_tactical_impact(
                &mut muted,
                id,
                hit,
                total - remaining,
                fall_rules(),
            )
            .unwrap();
            assert_eq!(report.impact, quiet.impact);
            let warnings: Vec<_> = report
                .notices
                .iter()
                .filter(|n| n.text.contains("WARNING:"))
                .collect();
            assert_eq!(warnings.len(), usize::from(remaining < total / 2));
            assert!(!quiet.notices.iter().any(|n| n.text.contains("WARNING:")));
            if let Some(notice) = warnings.first() {
                assert_eq!(notice.text.contains("(Rear)"), rear);
            }
        }
    }
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
    )
    .unwrap();
    assert!(
        scripts
            .eval_callback::<()>(&format!(
                "btech.unit.armor_warning({},1,false); error('abort')",
                id.0
            ))
            .is_err()
    );
    assert_eq!(scripts.world().btech, base.btech);
    support::run_text(&scripts, &config, ObjectId(1), 1, "unitprefs ArmorWarn OFF");
    set_battle_armor_warning(&mut base, id, ObjectId(1), false).unwrap();
    assert_eq!(scripts.world().btech, base.btech);
    persistence::save(&config.database(), &base).await.unwrap();
    assert!(
        !persistence::load(&config.database())
            .await
            .unwrap()
            .btech
            .constructed_units()[&id]
            .armor_warning()
    );
}

/// Low-ammunition messages accompany misses too, while aborted callbacks spend and emit nothing.
#[tokio::test]
async fn ammunition_warning_native_lua_thresholds_and_rollback() {
    use stompymux_rs::*;
    for supply in [2, 3, 4, 6, 7, 12, 13] {
        let (_dir, config, mut base, id, target) = shot_fixture().await;
        shot_skill(&mut base, 30);
        let index = base.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|m| m.weapon == Weapon::Srm4)
            .unwrap();
        base.btech
            .rewrite_unit_record(id, |record| {
                record["ammunition"] = serde_json::json!([supply]);
            })
            .unwrap();
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        let call = format!("btech.unit.fire({},1,{index},{})", id.0, target.0);
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, base.btech);
        assert!(lua.drain_outbox().is_empty());
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire {index} #{}", target.0),
        );
        lua.eval_callback::<()>(&call).unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        assert_eq!(text.contains("Ammo for"), matches!(supply, 6 | 12));
        if supply == 6 {
            assert!(text.contains("running low"));
        }
        let mut quiet = base.clone();
        set_battle_ammunition_warning(&mut quiet, id, ObjectId(1), false).unwrap();
        assert!(
            resolve_battle_shot(&mut quiet, id, ObjectId(1), target, index, shot_rules())
                .unwrap()
                .ammunition_warning
                .is_none()
        );
    }
}

/// Both safety controls reject teammates before expenditure, while enemy fire still resolves.
#[tokio::test]
async fn friendly_fire_safety_native_lua_map_rules_and_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut baseline, id, target) = shot_fixture().await;
    shot_skill(&mut baseline, 0);
    let map = baseline.btech.constructed_units()[&id]
        .position()
        .unwrap()
        .map;
    for same_team in [false, true] {
        for preference in [false, true] {
            for map_restriction in [false, true] {
                let mut base = baseline.clone();
                set_battle_friendly_fire_safety(&mut base, id, ObjectId(1), preference).unwrap();
                let mut state = serde_json::to_value(&base.btech).unwrap();
                state["constructed"][id.0.to_string()]["signature"]["team"] = 7.into();
                state["constructed"][target.0.to_string()]["signature"]["team"] =
                    if same_team { 7 } else { 8 }.into();
                state["maps"][map.0.to_string()]["flags"] =
                    if map_restriction { 256 } else { 0 }.into();
                base.btech = serde_json::from_value(state).unwrap();
                let denied = same_team && (preference || map_restriction);
                let mut direct = base.clone();
                let result =
                    resolve_battle_shot(&mut direct, id, ObjectId(1), target, 0, shot_rules());
                assert_eq!(result.is_err(), denied);
                if denied {
                    assert_eq!(direct.btech, base.btech);
                    let native = Scripts::new(
                        &config,
                        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
                    )
                    .unwrap();
                    let lua = Scripts::new(
                        &config,
                        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
                    )
                    .unwrap();
                    let text = support::run_text(
                        &native,
                        &config,
                        ObjectId(1),
                        1,
                        &format!("fire 0 #{}", target.0),
                    );
                    assert!(text.contains(if preference {
                        "FFSafeties"
                    } else {
                        "Friendly Fire?"
                    }));
                    assert!(
                        lua.eval_callback::<()>(&format!(
                            "btech.unit.fire({},1,0,{})",
                            id.0, target.0
                        ))
                        .is_err()
                    );
                    assert_eq!(native.world().btech, base.btech);
                    assert_eq!(lua.world().btech, base.btech);
                    assert!(lua.drain_outbox().is_empty());
                }
                persistence::save(&config.database(), &base).await.unwrap();
                let loaded = persistence::load(&config.database()).await.unwrap();
                assert_eq!(
                    loaded.btech.constructed_units()[&id].friendly_fire_safety(),
                    preference
                );
                assert_eq!(
                    loaded.btech.maps()[&map].blocks_friendly_fire(),
                    map_restriction
                );
            }
        }
    }
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(baseline.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(baseline.clone())),
    )
    .unwrap();
    let text = support::run_text(&native, &config, ObjectId(1), 1, "unitprefs FFSafety ON");
    assert!(text.contains("Friendly Fire Safeties flipped ON"));
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.friendly_fire_safety({},1,true); error('abort')",
            id.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, baseline.btech);
    lua.eval_callback::<()>(&format!("btech.unit.friendly_fire_safety({},1,true)", id.0))
        .unwrap();
    assert_eq!(native.world().btech, lua.world().btech);
    support::run_text(&native, &config, ObjectId(1), 1, "unitprefs ffsafety");
    assert!(!native.world().btech.constructed_units()[&id].friendly_fire_safety());
}

/// Cockpit status is a shared read-only projection that remains useful after damage and shutdown.
#[tokio::test]
async fn cockpit_status_native_lua_sections_damage_and_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = fixture('.').await;
    let before = world.btech.clone();
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    for options in [
        "", "armor", "info", "weapons", "heat", "short", "AIWH", "saiw",
    ] {
        let expected = battle_unit_status(&world, id, options).unwrap();
        let actual: String = scripts
            .eval_callback(&format!("return btech.unit.status({},'{}')", id.0, options))
            .unwrap();
        assert_eq!(actual, expected);
        let native = support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("status {options}"),
        );
        assert!(native.contains(expected.lines().next().unwrap().trim_end()));
        assert_eq!(scripts.world().btech, before);
    }
    let full = battle_unit_status(&world, id, "").unwrap();
    assert!(full.contains("Mech Name:") && !full.contains("SHUTDOWN"));
    assert!(full.contains("INTERNAL"));
    assert!(full.contains("Heat:"));
    assert!(full.contains("WEAPON SYSTEMS"));
    assert!(
        !battle_unit_status(&world, id, "armor")
            .unwrap()
            .contains("WEAPON SYSTEMS")
    );
    assert!(
        !battle_unit_status(&world, id, "heat")
            .unwrap()
            .contains("INTERNAL")
    );
    assert!(
        !battle_unit_status(&world, id, "short")
            .unwrap()
            .contains('\n')
    );
    // Unrecognized selectors never redirect the report to another object.
    let header = battle_unit_status(&world, id, "#1").unwrap();
    assert_eq!(header, battle_unit_status(&world, id, "?Z!").unwrap());
    assert!(header.contains("Mech Reference: JR7-D"));
    assert!(battle_unit_status(&world, ObjectId(-1), "").is_err());
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "status/quiet")
            .contains("no switches")
    );
    let section = MechSection::LeftArm;
    let old = world.btech.constructed_units()[&id].sections()[&section].armor;
    resolve_battle_impact(
        &mut world,
        id,
        Hit {
            section,
            rear_armor: false,
            through_armor_critical: false,
        },
        1,
    )
    .unwrap();
    assert!(
        battle_unit_status(&world, id, "armor")
            .unwrap()
            .contains(&format!("{:2}[reset]", old - 1))
    );
    stop_battle_unit(&mut world, id, ObjectId(1)).unwrap();
    let after = battle_unit_status(&world, id, "").unwrap();
    assert!(after.contains("SHUTDOWN"));
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(battle_unit_status(&loaded, id, "").unwrap(), after);
    remove_battle_unit(&mut world, id, ObjectId(config.start())).unwrap();
    assert!(
        battle_unit_status(&world, id, "info")
            .unwrap()
            .contains("X, Y, Z:  0,  0,  0")
    );
}

/// The compact export's walking figure is the damaged walking MP, not two thirds of the
/// running MP: a Jenner that has lost a leg walks at 1 MP and cannot run (Total Warfare
/// pp. 60, 122), so it exports 1/1 where two thirds of its 1 MP would have read 0.
#[tokio::test]
async fn compact_status_export_reports_the_damaged_walking_mp() {
    use stompymux_rs::*;
    let (_dir, _config, mut world, id) = fixture('.').await;
    let mut unit = world.btech.constructed_units()[&id].clone();
    let _ = unit.damage_phase(MechSection::LeftLeg, u16::MAX, DamagePhase::Internal);
    assert_eq!(
        unit.mobility().envelope(),
        SpeedEnvelope {
            walking: 10.75,
            running: 10.75
        }
    );
    let unit = serde_json::to_value(unit).unwrap();
    world
        .btech
        .rewrite_unit_record(id, |record| *record = unit)
        .unwrap();
    let header = battle_unit_status(&world, id, "N").unwrap();
    assert!(header.starts_with("JR7-D Jenner 35 1/1/"), "{header}");
}

/// Compact status preserves row-order ammunition pairing, empty pruning and option precedence.
#[tokio::test]
async fn compact_status_export_ammunition_damage_and_option_precedence() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = fixture('.').await;
    let header = "JR7-D Jenner 35 7/11/5 10 ";
    let full = format!(
        "{header}MediumLaser|Left_Arm|SRM-4|25 MediumLaser|Left_Arm MediumLaser|Right_Arm MediumLaser|Right_Arm SRM-4|Center_Torso "
    );
    let before = world.btech.clone();
    assert_eq!(battle_unit_status(&world, id, "N").unwrap(), header);
    assert_eq!(battle_unit_status(&world, id, "NAI").unwrap(), header);
    assert_eq!(battle_unit_status(&world, id, "NW").unwrap(), full);
    assert_eq!(
        battle_unit_status(&world, id, "SNW").unwrap(),
        battle_unit_status(&world, id, "S").unwrap()
    );
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let lua: String = scripts
        .eval_callback(&format!("return btech.unit.status({},'NW')", id.0))
        .unwrap();
    assert_eq!(lua, full);
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "status NW").contains(full.trim_end())
    );
    assert_eq!(scripts.world().btech, before);
    world.btech.set_unit_ammunition_bin(id, 0, 0).unwrap();
    assert!(
        !battle_unit_status(&world, id, "NW")
            .unwrap()
            .contains("|SRM-4|")
    );
    let mut state = serde_json::to_value(&world.btech).unwrap();
    // Surplus ammunition is a separate display row, before the compact record.
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    for section in definition.sections.values_mut() {
        section
            .criticals
            .retain(|_, part| !part.equipment.starts_with("IS."));
    }
    crate::support::templates::set_record_definition(
        &mut state["constructed"][id.0.to_string()],
        definition,
    );
    state["constructed"][id.0.to_string()]["ammunition"][0] = 25.into();
    world.btech = serde_json::from_value(state).unwrap();
    world.validate(&config).unwrap();
    let surplus = battle_unit_status(&world, id, "NW").unwrap();
    assert!(surplus.starts_with("                                                  || SRM-4"));
    assert!(surplus.contains("[fg=green bold] 25[reset]"));
    assert!(surplus.ends_with(header));
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        battle_unit_status(
            &persistence::load(&config.database()).await.unwrap(),
            id,
            "NW"
        )
        .unwrap(),
        surplus
    );
}

/// Status reports live airborne and pending state without advancing any owned cursor.
#[tokio::test]
async fn cockpit_status_reports_airborne_and_pending_state_without_advancing() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    select_battle_target(&mut world, id, ObjectId(1), Some(target)).unwrap();
    let status = battle_unit_status(&world, id, "info").unwrap();
    assert!(status.contains("Target:"));
    assert!(status.contains("Aimed Shot Location:"));
    assert!(status.contains("Heading:"));
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 4.0).unwrap();
    advance_battle_jumps(
        &mut world,
        stompymux_rs::MovementRules {
            fall: fall_rules(),
            ..stompymux_rs::MovementRules::STANDARD
        },
    )
    .unwrap();
    let before = world.btech.clone();
    let status = battle_unit_status(&world, id, "info").unwrap();
    assert!(status.contains("JUMPING -->"));
    assert!(status.contains("X, Y, Z:"));
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(battle_unit_status(&loaded, id, "info").unwrap(), status);
    power_down_battle_unit(&mut world, id).unwrap();
    assert!(world.btech.constructed_units()[&id].free_fall().is_some());
    assert!(
        battle_unit_status(&world, id, "info")
            .unwrap()
            .contains("SHUTDOWN")
    );
}

/// Mechanical kick damage and aim respond independently to upper/lower/foot loss and specialization.
#[tokio::test]
async fn kick_mechanics_actuators_profiles_and_rejection_preserve_state() {
    use stompymux_rs::*;
    let (_dir, config, base, id, target) = kick_fixture().await;
    for leg in [Leg::Left, Leg::Right] {
        for slots in [vec![], vec![1], vec![2], vec![3], vec![1, 2], vec![1, 2, 3]] {
            let mut world = base.clone();
            for &slot in &slots {
                destroy_battle_critical(
                    &mut world,
                    id,
                    CriticalLocation {
                        section: leg.section(MechChassis::Biped),
                        slot,
                    },
                )
                .unwrap();
            }
            let before = world.btech.clone();
            let profile =
                battle_kick_profile(&world, id, ObjectId(1), target, leg, kick_rules()).unwrap();
            let halves = slots.iter().filter(|&&slot| slot == 1 || slot == 2).count();
            assert_eq!(profile.damage, 7 >> halves);
            assert_eq!(
                profile.actuators,
                slots
                    .iter()
                    .map(|&slot| if slot == 3 { 1 } else { 2 })
                    .sum::<u8>()
            );
            // The unskilled pilot kicks from eleven less two, against an immobile target.
            assert_eq!(profile.base, 9);
            assert_eq!(profile.target_number, 9 + i32::from(profile.actuators) - 4);
            assert_eq!(profile.hit_table, HitTable::Kick);
            assert_eq!(world.btech, before);
        }
    }
    for (key, value) in [
        ("posture", serde_json::json!("Prone")),
        ("stun_remaining", serde_json::json!(5)),
        ("limb_recycle", serde_json::json!({"LeftLeg":30})),
    ] {
        let mut world = base.clone();
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record[key] = if key == "posture" {
                    serde_json::json!("prone")
                } else {
                    value
                };
            })
            .unwrap();
        let before = world.btech.clone();
        assert!(
            resolve_battle_kick(
                &mut world,
                id,
                ObjectId(1),
                target,
                Leg::Right,
                kick_rules()
            )
            .is_err()
        );
        assert_eq!(world.btech, before);
    }
    let mut world = base.clone();
    set_battle_friendly_fire_safety(&mut world, id, ObjectId(1), true).unwrap();
    assert!(
        battle_kick_profile(&world, id, ObjectId(1), target, Leg::Right, kick_rules()).is_err()
    );
    world.validate(&config).unwrap();
}

/// Native/Lua kicks replay identically and roll back both impact/fall cascades and recovery timers.
#[tokio::test]
async fn kick_native_lua_hit_miss_rollback_and_saved_recovery() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    for seed in [1, 7, 19] {
        shot_seed(&mut base, id, seed);
        shot_seed(&mut base, target, seed);
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        let call = format!("btech.unit.kick({},1,'right',{})", id.0, target.0);
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, base.btech);
        assert!(lua.drain_outbox().is_empty());
        support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("kick right #{}", target.0),
        );
        let report: mlua::Table = lua.eval_callback(&format!("return {call}")).unwrap();
        assert_eq!(report.get::<i64>("attacker").unwrap(), id.0);
        assert_eq!(native.world().btech, lua.world().btech);
        let mut world = lua.world().clone();
        assert_eq!(
            world.btech.constructed_units()[&id].limb_recycle()[&MechSection::RightLeg],
            30
        );
        let before = world.btech.clone();
        assert!(
            resolve_battle_kick(&mut world, id, ObjectId(1), target, Leg::Left, kick_rules())
                .is_err()
        );
        assert_eq!(world.btech, before);
        for _ in 0..17 {
            advance_battle_recycle(&mut world);
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(
            loaded.btech.constructed_units()[&id].limb_recycle()[&MechSection::RightLeg],
            13
        );
        for _ in 0..12 {
            advance_battle_recycle(&mut loaded);
        }
        assert!(
            advance_battle_recycle(&mut loaded)
                .iter()
                .any(|n| n.text.contains("Right Leg has finished"))
        );
        assert!(
            loaded.btech.constructed_units()[&id]
                .limb_recycle()
                .is_empty()
        );
    }
}

/// Explicit attack-roll boundaries distinguish misses, glancing hits and normal hits.
async fn kick_roll_matrix(target_seeds: std::ops::Range<u8>) {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    // A standing running target has no immobility discount.
    base.btech.set_unit_power(target, Power::Running).unwrap();
    base.validate(&config).unwrap();
    let target_number =
        battle_kick_profile(&base, id, ObjectId(1), target, Leg::Right, kick_rules())
            .unwrap()
            .target_number;
    let mut saw_critical_fall = false;
    for target_seed in target_seeds {
        for (offset, hit, glancing) in [(-1, false, false), (0, true, true), (1, true, false)] {
            let roll = u8::try_from(target_number + offset).unwrap();
            let seed = (0..=255)
                .find(|seed| Dice::seeded([*seed; 32]).two_d6() == roll)
                .unwrap();
            let mut world = base.clone();
            shot_seed(&mut world, id, seed);
            shot_seed(&mut world, target, target_seed);
            let rules = kick_rules();
            let mut replay = world.clone();
            let report =
                resolve_battle_kick(&mut world, id, ObjectId(1), target, Leg::Right, rules)
                    .unwrap();
            let repeated =
                resolve_battle_kick(&mut replay, id, ObjectId(1), target, Leg::Right, rules)
                    .unwrap();
            assert_eq!(report, repeated);
            assert_eq!(world.btech, replay.btech);
            assert_eq!(report.profile.target_number, target_number);
            assert_eq!(
                (report.roll, report.hit, report.glancing),
                (roll, hit, glancing)
            );
            assert_eq!(report.impact.is_some(), hit);
            // Only the admitted kick belongs to the attacker. A resulting balance
            // fall is self-attributed, including a miss that drops the attacker.
            let direct_damage = u32::from(if !hit {
                0
            } else if glancing {
                report.profile.damage / 2
            } else {
                report.profile.damage
            });
            let fall_damage = report.fall.as_ref().map_or(0, |fall| fall.damage);
            // Criticals can force a fall during impact, before the final kick balance check.
            let critical_fall_damage: u32 = report.impact.as_ref().map_or(0, |impact| {
                impact
                    .balance
                    .iter()
                    .filter_map(|balance| balance.fall.as_ref())
                    .map(|fall| fall.damage)
                    .sum()
            });
            saw_critical_fall |= critical_fall_damage > 0;
            let counters = serde_json::to_value(&world.btech).unwrap();
            assert_eq!(
                counters["constructed"][id.0.to_string()]["damage_counters"],
                serde_json::json!({
                    "taken": if hit { 0 } else { fall_damage },
                    "inflicted": direct_damage,
                })
            );
            assert_eq!(
                counters["constructed"][target.0.to_string()]["damage_counters"],
                serde_json::json!({
                    "taken": direct_damage + critical_fall_damage + if hit { fall_damage } else { 0 },
                    "inflicted": 0,
                })
            );
            world.validate(&config).unwrap();
            if hit {
                stop_battle_unit(&mut world, id, ObjectId(1)).unwrap();
                for _ in 0..61 {
                    advance_battle_recycle(&mut world);
                }
                assert_eq!(
                    world.btech.constructed_units()[&id].limb_recycle()[&MechSection::RightLeg],
                    30
                );
                assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
                support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
                start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
                assert!(
                    world.btech.constructed_units()[&id]
                        .limb_recycle()
                        .is_empty()
                );
            }
        }
    }
    assert!(
        saw_critical_fall,
        "seed matrix must exercise a critical-induced fall"
    );
}

#[tokio::test]
async fn kick_roll_boundaries_replay_and_shutdown_recovery_seeds_00_08() {
    kick_roll_matrix(0u8..8u8).await;
}

#[tokio::test]
async fn kick_roll_boundaries_replay_and_shutdown_recovery_seeds_08_16() {
    kick_roll_matrix(8u8..16u8).await;
}

#[tokio::test]
async fn kick_roll_boundaries_replay_and_shutdown_recovery_seeds_16_24() {
    kick_roll_matrix(16u8..24u8).await;
}

#[tokio::test]
async fn kick_roll_boundaries_replay_and_shutdown_recovery_seeds_24_32() {
    kick_roll_matrix(24u8..32u8).await;
}

/// Leg-mounted guns cannot fire during physical recovery and regain readiness on its last tick.
#[tokio::test]
async fn kick_recovery_blocks_leg_weapons_until_expiry() {
    use stompymux_rs::*;
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    let weapon = template
        .sections
        .get_mut(&MechSection::LeftArm)
        .unwrap()
        .criticals
        .remove(&2)
        .unwrap();
    template
        .sections
        .get_mut(&MechSection::RightLeg)
        .unwrap()
        .criticals
        .insert(4, weapon);
    let source = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
    let (_dir, config, mut world, id) = fixture_assets(&source, template).await;
    let index = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.criticals[0].section == MechSection::RightLeg)
        .unwrap();
    assert!(
        world.btech.constructed_units()[&id]
            .weapon_readiness(index)
            .unwrap()
            .ready
    );
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["limb_recycle"] = serde_json::json!({"RightLeg": 2});
        })
        .unwrap();
    world.validate(&config).unwrap();
    let before = world.btech.clone();
    assert!(spend_battle_weapon(&mut world, id, ObjectId(1), index).is_err());
    assert_eq!(world.btech, before);
    advance_battle_recycle(&mut world);
    assert!(
        !world.btech.constructed_units()[&id]
            .weapon_readiness(index)
            .unwrap()
            .ready
    );
    advance_battle_recycle(&mut world);
    assert!(
        world.btech.constructed_units()[&id]
            .weapon_readiness(index)
            .unwrap()
            .ready
    );
    spend_battle_weapon(&mut world, id, ObjectId(1), index).unwrap();
    world.validate(&config).unwrap();
}

/// Punches share targeting and damage but consume no kick balance roll, even on a miss.
#[tokio::test]
async fn punch_profiles_boundaries_and_independent_arms() {
    use stompymux_rs::*;
    let (_dir, config, base, id, target) = kick_fixture().await;
    let rules = kick_rules();
    for arm in [Arm::Left, Arm::Right] {
        let profile = battle_punch_profile(&base, id, ObjectId(1), target, arm, rules).unwrap();
        // Jenner arm weapons replace lower-arm and hand actuators.
        assert_eq!(
            (
                profile.base,
                profile.actuators,
                profile.damage,
                profile.target_number
            ),
            (11, 3, 2, 10)
        );
        assert_eq!(profile.hit_table, HitTable::Punch);
    }
    let target_number = battle_punch_profile(&base, id, ObjectId(1), target, Arm::Left, rules)
        .unwrap()
        .target_number;
    for (offset, hit, glance) in [(-1, false, false), (0, true, true), (1, true, false)] {
        let roll = u8::try_from(target_number + offset).unwrap();
        let mut world = base.clone();
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == roll)
            .unwrap();
        shot_seed(&mut world, id, seed);
        let mut expected_dice = Dice::seeded([seed; 32]);
        assert_eq!(expected_dice.two_d6(), roll);
        let result = resolve_battle_punch(
            &mut world,
            id,
            ObjectId(1),
            target,
            ArmSelection::Left,
            rules,
        )
        .unwrap();
        let attack = &result.attacks[0];
        assert_eq!(
            (attack.roll, attack.hit, attack.glancing),
            (roll, hit, glance)
        );
        assert!(attack.balance.is_none() && attack.fall.is_none());
        assert_eq!(
            serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
            serde_json::to_value(&expected_dice).unwrap()
        );
        assert_eq!(
            world.btech.constructed_units()[&target].posture(),
            Posture::Standing
        );
        world.validate(&config).unwrap();
    }
    let mut world = base.clone();
    let report = resolve_battle_punch(
        &mut world,
        id,
        ObjectId(1),
        target,
        ArmSelection::Both,
        rules,
    )
    .unwrap();
    assert_eq!(report.attacks.len(), 2);
    assert!(report.rejections.is_empty());
    assert_eq!(
        report.attacks[0].profile.attack.section(MechChassis::Biped),
        MechSection::LeftArm
    );
    assert_eq!(
        report.attacks[1].profile.attack.section(MechChassis::Biped),
        MechSection::RightArm
    );
    assert_eq!(world.btech.constructed_units()[&id].limb_recycle().len(), 2);
    let before = world.btech.clone();
    assert!(
        resolve_battle_punch(
            &mut world,
            id,
            ObjectId(1),
            target,
            ArmSelection::Both,
            rules
        )
        .is_err()
    );
    assert_eq!(before, world.btech);
    // A lethal first punch makes the second arm's target invalid without spending its roll.
    let mut world = base.clone();
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["sections"]["Head"]["armor"] = 0.into();
            record["sections"]["Head"]["internal"] = 1.into();
        })
        .unwrap();
    let attack_seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    let location_seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).d6() == 6)
        .unwrap();
    shot_seed(&mut world, id, attack_seed);
    shot_seed(&mut world, target, location_seed);
    let result = resolve_battle_punch(
        &mut world,
        id,
        ObjectId(1),
        target,
        ArmSelection::Both,
        rules,
    )
    .unwrap();
    assert!(world.btech.constructed_units()[&target].is_destroyed());
    assert_eq!((result.attacks.len(), result.rejections.len()), (1, 1));
    assert_eq!(result.rejections[0].arm, Arm::Right);
    assert!(
        !world.btech.constructed_units()[&id]
            .limb_recycle()
            .contains_key(&MechSection::RightArm)
    );
    world.validate(&config).unwrap();
    for busy in [false, true] {
        let mut world = base.clone();
        if busy {
            spend_battle_weapon(&mut world, id, ObjectId(1), 0).unwrap();
        } else {
            destroy_battle_critical(
                &mut world,
                id,
                CriticalLocation {
                    section: MechSection::LeftArm,
                    slot: 0,
                },
            )
            .unwrap();
        }
        let report = resolve_battle_punch(
            &mut world,
            id,
            ObjectId(1),
            target,
            ArmSelection::Both,
            rules,
        )
        .unwrap();
        assert_eq!(report.attacks.len(), 1);
        assert_eq!(report.rejections.len(), 1);
        assert_eq!(report.rejections[0].arm, Arm::Left);
        assert_eq!(
            report.attacks[0].profile.attack.section(MechChassis::Biped),
            MechSection::RightArm
        );
        assert!(
            !world.btech.constructed_units()[&id]
                .limb_recycle()
                .contains_key(&MechSection::LeftArm)
        );
        world.validate(&config).unwrap();
    }
}

/// Native and Lua two-arm actions stage identical messages and restart with identical dice and timers.
#[tokio::test]
async fn punch_native_lua_rollback_messages_and_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    shot_seed(&mut base, id, 9);
    shot_seed(&mut base, target, 17);
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
    )
    .unwrap();
    let call = format!("btech.unit.punch({},1,'both',{})", id.0, target.0);
    assert!(
        lua.eval_callback::<()>(&format!("{call}; error('abort')"))
            .is_err()
    );
    assert_eq!(lua.world().btech, base.btech);
    assert!(lua.drain_outbox().is_empty());
    commands::run(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("punch both #{}", target.0),
    )
    .unwrap();
    let _: mlua::Table = lua.eval_callback(&format!("return {call}")).unwrap();
    assert_eq!(native.world().btech, lua.world().btech);
    let messages = |scripts: &Scripts| {
        scripts
            .drain_outbox()
            .into_iter()
            .map(|(to, message)| (to, message.source().to_owned()))
            .collect::<Vec<_>>()
    };
    assert_eq!(messages(&native), messages(&lua));
    let mut expected = lua.world().clone();
    persistence::save(&config.database(), &expected)
        .await
        .unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    for _ in 0..60 {
        assert_eq!(
            advance_battle_recycle(&mut loaded),
            advance_battle_recycle(&mut expected)
        );
    }
    assert_eq!(loaded.btech, expected.btech);
    loaded
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let a = resolve_battle_punch(
        &mut expected,
        id,
        ObjectId(1),
        target,
        ArmSelection::Both,
        kick_rules(),
    )
    .unwrap();
    let b = resolve_battle_punch(
        &mut loaded,
        id,
        ObjectId(1),
        target,
        ArmSelection::Both,
        kick_rules(),
    )
    .unwrap();
    assert_eq!(a, b);
    assert_eq!(loaded.btech, expected.btech);
}

/// Punch reach follows torso and arm side arcs, with elevation selecting the physical location table.
#[tokio::test]
async fn punch_arm_arcs_and_elevation_tables() {
    use stompymux_rs::*;
    let (_dir, config, base, id, target) = kick_fixture().await;
    for (twist, left, right) in [
        (Torso::Center, false, true),
        (Torso::Right, true, true),
        (Torso::Left, false, false),
    ] {
        for flipped in [false, true] {
            let mut world = base.clone();
            let mut state = serde_json::to_value(&world.btech).unwrap();
            let x = world.btech.constructed_units()[&target]
                .motion()
                .unwrap()
                .point
                .x;
            state["constructed"][target.0.to_string()]["motion"]["point"]["x"] = (x + 0.25).into();
            state["constructed"][id.0.to_string()]["facing"] = serde_json::to_value(Facing {
                torso: twist,
                arms_flipped: flipped,
            })
            .unwrap();
            world.btech = serde_json::from_value(state).unwrap();
            world.validate(&config).unwrap();
            for (arm, allowed) in [(Arm::Left, left), (Arm::Right, right)] {
                assert_eq!(
                    battle_punch_profile(&world, id, ObjectId(1), target, arm, kick_rules())
                        .is_ok(),
                    allowed
                );
            }
        }
    }
    for (height, posture, expected) in [
        (0, Posture::Standing, Some(HitTable::Punch)),
        (1, Posture::Standing, Some(HitTable::Kick)),
        (1, Posture::Prone, Some(HitTable::Weapon)),
        (0, Posture::Prone, None),
        (-1, Posture::Standing, None),
    ] {
        let mut world = base.clone();
        world
            .btech
            .rewrite_unit_record(target, |record| {
                record["ground_elevation"] = height.into();
                record["posture"] = serde_json::to_value(posture).unwrap();
            })
            .unwrap();
        world.validate(&config).unwrap();
        let before = world.btech.clone();
        let profile =
            battle_punch_profile(&world, id, ObjectId(1), target, Arm::Right, kick_rules());
        assert_eq!(profile.ok().map(|p| p.hit_table), expected);
        assert_eq!(world.btech, before);
    }
}

/// Trips ignore limb actuator aim penalties, require intact hips and reject already-down targets.
#[tokio::test]
async fn trip_profile_actuators_and_target_guards() {
    use stompymux_rs::*;
    let (_dir, config, base, id, target) = kick_fixture().await;
    for leg in [Leg::Left, Leg::Right] {
        let mut world = base.clone();
        for slot in [1, 2, 3] {
            destroy_battle_critical(
                &mut world,
                id,
                CriticalLocation {
                    section: leg.section(MechChassis::Biped),
                    slot,
                },
            )
            .unwrap();
            let profile =
                battle_trip_profile(&world, id, ObjectId(1), target, leg, kick_rules()).unwrap();
            // A trip takes Piloting -1 (Tactical Operations: Advanced Rules p. 89), against
            // the kick's -2.
            assert_eq!(
                (
                    profile.base,
                    profile.actuators,
                    profile.damage,
                    profile.target_number
                ),
                (10, 0, 0, 6)
            );
            assert!(matches!(profile.attack, PhysicalAttack::Trip { .. }));
        }
        destroy_battle_critical(
            &mut world,
            id,
            CriticalLocation {
                section: leg.section(MechChassis::Biped),
                slot: 0,
            },
        )
        .unwrap();
        let before = world.btech.clone();
        assert!(
            resolve_battle_trip(&mut world, id, ObjectId(1), target, leg, kick_rules()).is_err()
        );
        assert_eq!(world.btech, before);
        world.validate(&config).unwrap();
    }
    for rising in [false, true] {
        let mut world = base.clone();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        let victim = &mut state["constructed"][target.0.to_string()];
        if rising {
            victim["power"] = serde_json::to_value(Power::Running).unwrap();
            victim["stand_timer"] = serde_json::json!({"state":"rising","remaining":5});
        } else {
            victim["posture"] = serde_json::to_value(Posture::Prone).unwrap();
        }
        world.btech = serde_json::from_value(state).unwrap();
        world.validate(&config).unwrap();
        let before = world.btech.clone();
        assert!(
            resolve_battle_trip(
                &mut world,
                id,
                ObjectId(1),
                target,
                Leg::Right,
                kick_rules()
            )
            .is_err()
        );
        assert_eq!(before, world.btech);
    }
}

/// Successful trips consume a balance roll without a hit-location roll; misses consume only the attack roll.
#[tokio::test]
async fn trip_rolls_balance_and_glancing_have_no_direct_impact() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    base.btech.set_unit_power(target, Power::Running).unwrap();
    let target_number =
        battle_trip_profile(&base, id, ObjectId(1), target, Leg::Left, kick_rules())
            .unwrap()
            .target_number;
    for (offset, hit, glancing, target_roll) in [
        (-1, false, false, 12),
        (0, true, true, 12),
        (1, true, false, 12),
        (1, true, false, 2),
    ] {
        let roll = u8::try_from(target_number + offset).unwrap();
        let attack_seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == roll)
            .unwrap();
        let target_seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == target_roll)
            .unwrap();
        let mut world = base.clone();
        shot_seed(&mut world, id, attack_seed);
        shot_seed(&mut world, target, target_seed);
        let source_sections = world.btech.constructed_units()[&id].sections().clone();
        let target_sections = world.btech.constructed_units()[&target].sections().clone();
        let report =
            resolve_battle_trip(&mut world, id, ObjectId(1), target, Leg::Left, kick_rules())
                .unwrap();
        assert_eq!(
            (report.roll, report.hit, report.glancing),
            (roll, hit, glancing)
        );
        assert_eq!(report.profile.target_number, target_number);
        assert!(report.impact.is_none());
        assert_eq!(report.balance.is_some(), hit);
        let falls = hit && target_roll == 2;
        assert_eq!(report.fall.is_some(), falls);
        assert_eq!(
            world.btech.constructed_units()[&id].sections(),
            &source_sections
        );
        let mut expected_source = Dice::seeded([attack_seed; 32]);
        expected_source.two_d6();
        assert_eq!(
            serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
            serde_json::to_value(expected_source).unwrap()
        );
        if !falls {
            assert_eq!(
                world.btech.constructed_units()[&target].sections(),
                &target_sections
            );
            let mut expected_target = Dice::seeded([target_seed; 32]);
            if hit {
                assert_eq!(expected_target.two_d6(), target_roll);
            }
            assert_eq!(
                serde_json::to_value(&world.btech.constructed_units()[&target]).unwrap()["dice"],
                serde_json::to_value(expected_target).unwrap()
            );
        } else {
            assert_eq!(
                world.btech.constructed_units()[&target].posture(),
                Posture::Prone
            );
            assert!(
                report
                    .notices
                    .iter()
                    .any(|notice| notice.text.contains("You trip #"))
            );
        }
        assert_eq!(
            world.btech.constructed_units()[&id].limb_recycle()[&MechSection::LeftLeg],
            30
        );
        world.validate(&config).unwrap();
    }
}

/// Native/Lua trip falls and notices are transactional and recovery survives restart.
#[tokio::test]
async fn trip_native_lua_rollback_and_saved_fall() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    shot_seed(&mut base, id, seed);
    shot_seed(&mut base, target, 19);
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
    )
    .unwrap();
    let call = format!("btech.unit.trip({},1,'right',{})", id.0, target.0);
    assert!(
        lua.eval_callback::<()>(&format!("{call}; error('abort')"))
            .is_err()
    );
    assert_eq!(lua.world().btech, base.btech);
    assert!(lua.drain_outbox().is_empty());
    commands::run(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("trip right #{}", target.0),
    )
    .unwrap();
    let _: mlua::Table = lua.eval_callback(&format!("return {call}")).unwrap();
    assert_eq!(native.world().btech, lua.world().btech);
    let messages = |scripts: &Scripts| {
        scripts
            .drain_outbox()
            .into_iter()
            .map(|(to, message)| (to, message.source().to_owned()))
            .collect::<Vec<_>>()
    };
    assert_eq!(messages(&native), messages(&lua));
    let mut world = lua.world().clone();
    assert_eq!(
        world.btech.constructed_units()[&target].posture(),
        Posture::Prone
    );
    assert!(messages(&lua).is_empty());
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    for _ in 0..60 {
        assert_eq!(
            advance_battle_recycle(&mut world),
            advance_battle_recycle(&mut loaded)
        );
    }
    assert_eq!(world.btech, loaded.btech);
}

/// Cockpit status renders ammunition colors while keeping literal names and weapon annotations visible.
#[tokio::test]
async fn status_rendering_preserves_literal_fields_and_ammunition_colors() {
    use stompymux_rs::text::{ColorDepth, Document, Palette, RenderOptions};
    use stompymux_rs::*;
    let label = "[fg=red]Jenner[/]";
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    template.name = label.into();
    template
        .sections
        .get_mut(&MechSection::CenterTorso)
        .unwrap()
        .criticals
        .get_mut(&10)
        .unwrap()
        .modes = vec!["OneShot".into()];
    template
        .sections
        .get_mut(&MechSection::LeftArm)
        .unwrap()
        .criticals
        .remove(&2);
    let map = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
    let (_dir, config, mut world, id) = fixture_assets(&map, template).await;
    world.validate(&config).unwrap();
    let palette = Palette::default();
    for options in ["", "W", "A", "N", "NW"] {
        let text = battle_unit_status(&world, id, options).unwrap();
        let document = Document::Styled(text);
        let plain =
            String::from_utf8(document.telnet(&palette, &RenderOptions::default(), 65536)).unwrap();
        assert!(plain.contains(label));
        if matches!(options, "" | "W") {
            assert!(plain.contains("OS SRM-4"));
        }
        let spans = document.spans(&palette, &RenderOptions::default());
        let name_spans = spans
            .iter()
            .filter(|span| span.text.contains(label))
            .collect::<Vec<_>>();
        assert!(
            !name_spans.is_empty(),
            "literal name must survive styled rendering"
        );
        for span in name_spans {
            assert!(span.style.foreground.is_none());
            assert!(span.style.foreground_ansi.is_none());
        }
    }
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    // The surplus-ammunition case does not need any weapon mounts.
    for section in definition.sections.values_mut() {
        section
            .criticals
            .retain(|_, part| !part.equipment.starts_with("IS."));
    }
    crate::support::templates::set_record_definition(
        &mut state["constructed"][id.0.to_string()],
        definition,
    );
    for (rounds, expected_color) in [
        (25, "green"),
        (13, "green"),
        (12, "yellow"),
        (7, "yellow"),
        (6, "red"),
        (1, "red"),
    ] {
        state["constructed"][id.0.to_string()]["ammunition"][0] = rounds.into();
        world.btech = serde_json::from_value(state.clone()).unwrap();
        world.validate(&config).unwrap();
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let before = scripts.world().btech.clone();
        let CommandAction::Report(CommandReport::Styled(source)) =
            commands::run(&scripts, &config, ObjectId(1), 1, "status NW").unwrap()
        else {
            panic!("status must retain styled-report rendering")
        };
        let lua: String = scripts
            .eval_callback(&format!("return btech.unit.status({},'NW')", id.0))
            .unwrap();
        assert_eq!(source, lua);
        assert!(source.contains(&format!("[fg={expected_color} bold]")));
        let document = Document::Styled(source);
        let plain =
            String::from_utf8(document.telnet(&palette, &RenderOptions::default(), 65536)).unwrap();
        assert!(!plain.contains("[reset]") && !plain.contains(" bold]"));
        assert!(plain.contains(label));
        assert!(!plain.contains('\x1b'));
        let colored = String::from_utf8(document.telnet(
            &palette,
            &RenderOptions {
                color: ColorDepth::Ansi16,
                ..Default::default()
            },
            65536,
        ))
        .unwrap();
        assert!(colored.contains('\x1b'));
        assert!(colored.contains(label));
        assert!(
            document
                .spans(&palette, &RenderOptions::default())
                .iter()
                .any(|span| span.style.bold && span.text.contains(&rounds.to_string()))
        );
        assert_eq!(scripts.world().btech, before);
    }
}

/// A leg hit on a 'Mech with partial cover from a building damages the building instead of
/// the 'Mech (Total Warfare p. 102); other hits land normally.
#[tokio::test]
async fn partial_cover_leg_hits_damage_the_covering_building() {
    use stompymux_rs::*;
    let source = format!(
        "12 12\n{}",
        (0..12)
            .map(|y| (0..12)
                .map(|x| if (x, y) == (5, 4) { "@1" } else { ".0" })
                .collect::<String>()
                + "\n")
            .collect::<String>()
    );
    let (_dir, config, mut world, id) = fixture_source(&source).await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let interior = world.create(&config, "Cover shed".into(), Kind::Room);
    create_battle_map(
        &mut world,
        interior,
        "interior",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, interior, support::FIXTURE_DICE_SEED);
    set_building_state(
        &mut world,
        interior,
        BuildingState {
            integrity: 40,
            maximum_integrity: 40,
            flags: 0,
            regeneration: 1,
        },
    )
    .unwrap();
    set_building_entrance(
        &mut world,
        map,
        0,
        Some(BuildingEntrance {
            coordinate: HexCoordinate { x: 5, y: 4 },
            interior,
            data_char: 0,
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    let target = world.create(&config, "Covered target".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        target,
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, target, map, 5, 3).unwrap();
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    shot_skill(&mut world, 20);
    let mut covered_legs = false;
    let mut struck_body = false;
    for seed in 0..=40u8 {
        let mut world = world.clone();
        let roll = Dice::seeded([seed; 32]).two_d6();
        let location = HitTable::Weapon
            .location(MechChassis::Biped, HitArc::Rear, roll)
            .unwrap();
        shot_seed(&mut world, target, seed);
        let before = world.btech.constructed_units()[&target].sections().clone();
        let report =
            resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, shot_rules()).unwrap();
        assert_eq!(report.aim.perception.unwrap().modifier, 1);
        let salvo = report.salvo.unwrap().into_mech().unwrap();
        assert_eq!(salvo.groups.len(), 1);
        assert_eq!(salvo.groups[0].hit.section, location);
        let leg = matches!(location, MechSection::LeftLeg | MechSection::RightLeg);
        let unchanged = world.btech.constructed_units()[&target].sections() == &before;
        assert_eq!(unchanged, leg, "seed {seed}: {location:?}");
        let expected = if leg {
            40 - i64::from(salvo.groups[0].damage)
        } else {
            40
        };
        assert_eq!(
            world.btech.maps()[&interior].building.integrity,
            expected,
            "seed {seed}: {location:?}"
        );
        assert_eq!(
            salvo.groups[0]
                .notices
                .iter()
                .any(|notice| notice.unit == id && notice.text.starts_with("You hit ")),
            leg,
            "seed {seed}: {location:?}"
        );
        covered_legs |= leg;
        struck_body |= !leg;
    }
    assert!(covered_legs && struck_body);
}
