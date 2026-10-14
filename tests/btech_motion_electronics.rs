//! Anti-missile systems, NARC and iNARC pods, ECM and other electronics, stealth and null
//! signature systems, active probes, TAG and semi-guided fire, spotters, indirect and
//! hex fire, building fire, inferno ammunition, quads, Stingers and A-pods, and private feedback
//! routing.

use crate::btech_motion_common::{
    RULES, balance_hit, balance_skill, fall_rules, fixture, fixture_assets, kick_fixture,
    kick_rules, optical_aim_rules, overheat_due, overheat_rules, prepare_test_charge, shot_fixture,
    shot_rules, shot_seed, shot_skill, water_fixture, without_xp_timestamps,
};
use crate::support;
use crate::support::{restore_database, snapshot_database};
use stompymux_rs::{
    MechTemplate, MovementRules, ObjectId, advance_battle_motion, persistence, set_battle_heading,
    set_battle_speed,
};

/// TAG attack rules matching the conventional shot rules.
fn tag_rules() -> stompymux_rs::TagRules {
    stompymux_rs::TagRules {
        aim: optical_aim_rules(),
        extended_gunnery: true,
    }
}

/// Seed a unit's dice so its next TAG attack rolls a twelve, and give the connected pilot a
/// gunnery skill, since a player without a character fires at 18 and could never designate.
fn tag_seed(world: &mut stompymux_rs::World, id: ObjectId) {
    shot_skill(world, 7);
    shot_seed(world, id, support::roll_twelve_seed());
}

/// Add a defense and, for ballistic AMS, a matching bin without replacing the fixture's
/// offensive mounts.
fn install_test_ams(
    world: &mut stompymux_rs::World,
    id: ObjectId,
    weapon: stompymux_rs::Weapon,
) -> (usize, Option<usize>) {
    install_test_ams_at(world, id, weapon, stompymux_rs::MechSection::LeftArm)
}

/// Turn the shot fixture's target (one hex north of the shooter) to face south, so its arm-mounted
/// defenses cover the attack direction (Total Warfare p. 129).
fn face_attacker(world: &mut stompymux_rs::World, target: ObjectId) {
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["motion"]["heading"] = 180.0.into();
        })
        .unwrap();
}

/// Supply one arm-mounted defense for equipment selection scenarios.
fn install_test_ams_at(
    world: &mut stompymux_rs::World,
    id: ObjectId,
    weapon: stompymux_rs::Weapon,
    section: stompymux_rs::MechSection,
) -> (usize, Option<usize>) {
    use stompymux_rs::*;
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    let arm = definition.sections.get_mut(&section).unwrap();
    let mut part = arm.criticals[&2].clone();
    part.equipment = weapon.name().into();
    let slots = weapon.profile().critical_slots;
    for slot in 0..slots {
        arm.criticals.insert(4 + slot, part.clone());
    }
    if weapon.profile().ammunition_per_ton > 0 {
        part.equipment = format!("Ammo_{}", weapon.name());
        part.data = weapon.profile().ammunition_per_ton.to_string();
        arm.criticals.insert(4 + slots, part);
    }
    let constructed = Mech::from_template(definition.clone()).unwrap();
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["definition"] = serde_json::to_value(definition).unwrap();
            record["ammunition"] = serde_json::to_value(constructed.ammunition()).unwrap();
        })
        .unwrap();
    let loadout = world.btech.constructed_units()[&id].loadout().unwrap();
    (
        loadout
            .weapons
            .iter()
            .position(|m| m.weapon == weapon)
            .unwrap(),
        loadout.ammunition.iter().position(|b| b.weapon == weapon),
    )
}

/// Automatic defense uses one ready mount even on misses; a shortage limits cost, not capacity.
async fn ams_interception_matrix(entries: &[(stompymux_rs::Weapon, f64, u16)]) {
    use stompymux_rs::*;
    let (_dir, config, pristine, _id, _target) = shot_fixture().await;
    let pristine_db = snapshot_database(&config);
    let mut probed_restart = false;
    let mut probed_save = false;
    for (weapon, heat, recycle) in entries.iter().copied() {
        for case in [
            "hit",
            "miss",
            "short",
            "empty",
            "no_bin",
            "disabled",
            "shutdown",
            "recycling",
            "destroyed",
            "laser",
            "flooded",
        ] {
            let fed = weapon.profile().ammunition_per_ton > 0;
            if !fed && matches!(case, "short" | "empty" | "no_bin" | "flooded") {
                continue;
            }
            restore_database(&config, &pristine_db);
            let (mut world, id, target) = (pristine.clone(), _id, _target);
            let (ams_index, bin) = install_test_ams(&mut world, target, weapon);
            world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
            assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
            support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
            start_battle_unit(&mut world, target, ObjectId(2), true).unwrap();
            for _ in 0..5 {
                advance_battle_units(&mut world, 0, true);
            }
            set_battle_ams(&mut world, target, ObjectId(2), true).unwrap();
            face_attacker(&mut world, target);
            shot_skill(&mut world, if case == "miss" { 0 } else { 30 });
            set_battle_character_value(
                &mut world,
                ObjectId(1),
                "Gunnery-Missile",
                CharacterValue {
                    value: if case == "miss" { 0 } else { 30 },
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
            let index = world.btech.constructed_units()[&id]
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|m| {
                    m.weapon
                        == if case == "laser" {
                            Weapon::MediumLaser
                        } else {
                            Weapon::Srm4
                        }
                })
                .unwrap();
            let seed = (0..=255)
                .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
                .unwrap();
            shot_seed(&mut world, id, seed);
            let cluster_seed = (0..=255)
                .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
                .unwrap();
            shot_seed(&mut world, target, cluster_seed);
            world
                .btech
                .rewrite_unit_record(target, |record| {
                    if matches!(case, "short" | "empty") {
                        record["ammunition"][bin.unwrap()] =
                            if case == "short" { 1 } else { 0 }.into();
                    }
                    if case == "recycling" {
                        record["weapon_recycle"][ams_index.to_string()] = 1.into();
                    }
                    if case == "flooded" {
                        record["flooded_sections"] = serde_json::json!(["LeftArm"]);
                        record["ammunition"][bin.unwrap()] = 0.into();
                        // A dry reserve bin isolates the disabled mount from the lack of supply.
                        let part =
                            &mut record["definition"]["sections"]["RightTorso"]["criticals"]["0"];
                        part["equipment"] = format!("Ammo_{}", weapon.name()).into();
                        part["data"] = weapon.profile().ammunition_per_ton.to_string().into();
                        record["ammunition"][1] = weapon.profile().ammunition_per_ton.into();
                    }
                    if case == "no_bin" {
                        record["definition"]["sections"]["LeftArm"]["criticals"]
                            .as_object_mut()
                            .unwrap()
                            .remove("5");
                        record["ammunition"] = serde_json::json!([25]);
                    }
                })
                .unwrap();
            if case == "disabled" {
                set_battle_ams(&mut world, target, ObjectId(2), false).unwrap();
            }
            if case == "shutdown" {
                stop_battle_unit(&mut world, target, ObjectId(2), fall_rules()).unwrap();
            }
            if case == "destroyed" {
                destroy_battle_critical(
                    &mut world,
                    target,
                    CriticalLocation {
                        section: MechSection::LeftArm,
                        slot: 4,
                    },
                )
                .unwrap();
                assert!(!world.btech.constructed_units()[&target].ams_enabled());
                assert!(set_battle_ams(&mut world, target, ObjectId(2), true).is_err());
            }
            if case == "flooded" {
                assert!(world.btech.constructed_units()[&target].ams_enabled());
                set_battle_ams(&mut world, target, ObjectId(2), true).unwrap();
            }
            let before = world.clone();
            // Restart probe runs once per shard; every case keeps the resolve assertions.
            let restored = if probed_restart {
                None
            } else {
                probed_restart = true;
                persistence::save(&config.database(), &before)
                    .await
                    .unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                restored.objects.get_mut(&ObjectId(1)).unwrap().flags =
                    before.objects[&ObjectId(1)].flags.clone();
                Some(restored)
            };
            let report =
                resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules())
                    .unwrap();
            if let Some(mut restored) = restored {
                assert_eq!(
                    resolve_battle_shot(
                        &mut restored,
                        id,
                        ObjectId(1),
                        target,
                        index,
                        shot_rules()
                    )
                    .unwrap(),
                    report
                );
                assert_eq!(restored.btech, world.btech);
            }
            let active = matches!(case, "hit" | "short");
            assert_eq!(report.ams.is_some(), active, "{weapon:?} {case}");
            if case == "miss" {
                assert_eq!(
                    world.btech.constructed_units()[&target],
                    before.btech.constructed_units()[&target]
                );
            }
            let mut expected_dice = Dice::seeded([seed; 32]);
            expected_dice.two_d6();
            if let Some(ams) = &report.ams {
                // A cluster engagement rolls no die of its own (Total Warfare p. 129): it shifts
                // the target's cluster roll of 12 down to 8, which lands three of the four
                // missiles, and spends one round whatever the system's tech base.
                assert_eq!(ams.roll, None);
                assert_eq!(ams.shot_down, 1);
                assert_eq!(ams.ammunition_spent, u16::from(fed));
                assert_eq!(
                    world.btech.constructed_units()[&target].weapon_recycle()[&ams_index],
                    recycle
                );
                assert_eq!(
                    world.btech.constructed_units()[&target].heat().stored,
                    before.btech.constructed_units()[&target].heat().stored + heat
                );
                if let Some(bin) = bin {
                    assert_eq!(
                        world.btech.constructed_units()[&target].ammunition()[bin],
                        before.btech.constructed_units()[&target].ammunition()[bin]
                            - ams.ammunition_spent
                    );
                }
                let salvo = report.salvo.as_ref().unwrap().as_mech().unwrap();
                assert_eq!(salvo.missiles_before_defense, Some(4));
                assert_eq!(salvo.missiles_intercepted, Some(1));
                assert_eq!(salvo.groups.iter().map(|g| g.damage).sum::<u16>(), 3 * 2);
            }
            let dice = &serde_json::to_value(&world.btech).unwrap()["constructed"]
                [id.0.to_string()]["dice"];
            assert_eq!(*dice, serde_json::to_value(expected_dice).unwrap());
            if !probed_save {
                probed_save = true;
                persistence::save(&config.database(), &world).await.unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    world.btech
                );
            }
        }
    }
}

#[tokio::test]
async fn ams_interception_expenditure_eligibility_and_restart_ams() {
    use stompymux_rs::Weapon;
    ams_interception_matrix(&[(Weapon::AntiMissileSystem, 1.0, 10)]).await;
}

#[tokio::test]
async fn ams_interception_expenditure_eligibility_and_restart_clan_ams() {
    use stompymux_rs::Weapon;
    ams_interception_matrix(&[(Weapon::ClanAntiMissileSystem, 1.0, 10)]).await;
}

#[tokio::test]
async fn ams_interception_expenditure_eligibility_and_restart_laser_ams() {
    use stompymux_rs::Weapon;
    ams_interception_matrix(&[(Weapon::LaserAms, 7.0, 25)]).await;
}

#[tokio::test]
async fn ams_interception_expenditure_eligibility_and_restart_clan_laser_ams() {
    use stompymux_rs::Weapon;
    ams_interception_matrix(&[(Weapon::ClanLaserAms, 5.0, 25)]).await;
}

/// Native and Lua controls/firing share defense effects and rollback all expenditure on callback abort.
#[tokio::test]
async fn ams_native_lua_controls_fire_and_rollback() {
    use stompymux_rs::*;
    for weapon in [
        Weapon::AntiMissileSystem,
        Weapon::ClanAntiMissileSystem,
        Weapon::LaserAms,
        Weapon::ClanLaserAms,
    ] {
        let (_dir, config, mut world, id, target) = shot_fixture().await;
        let (index, _) = install_test_ams(&mut world, id, weapon);
        install_test_ams(&mut world, target, weapon);
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
        assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, target, ObjectId(2), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0, true);
        }
        set_battle_ams(&mut world, target, ObjectId(2), true).unwrap();
        face_attacker(&mut world, target);
        shot_skill(&mut world, 30);
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            "Gunnery-Missile",
            CharacterValue {
                value: 30,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        let before = world.clone();
        assert!(
            resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules())
                .unwrap_err()
                .to_string()
                .contains("defensive only")
        );
        assert!(spend_battle_weapon(&mut world, id, ObjectId(1), index).is_err());
        assert_eq!(world.btech, before.btech);
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
            lua.eval_callback::<()>(&format!("btech.unit.ams({},1); error('abort')", id.0))
                .is_err()
        );
        assert_eq!(lua.world().btech, before.btech);
        assert!(lua.drain_outbox().is_empty());
        let text = support::run_text(&native, &config, ObjectId(1), 1, "ams");
        assert!(text.contains("Anti-Missile System turned ON"));
        assert!(
            lua.eval_callback::<bool>(&format!("return btech.unit.ams({},1)", id.0))
                .unwrap()
        );
        assert_eq!(native.world().btech, lua.world().btech);
        lua.drain_outbox();
        let index = before.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|m| m.weapon == Weapon::Srm4)
            .unwrap();
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.fire({},1,{index},{}); error('abort')",
                id.0, target.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(lua.drain_outbox().is_empty());
        let text = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire {index} #{}", target.0),
        );
        assert!(
            text.contains("shot down") || text.contains("shoots down"),
            "{text}"
        );
        let report = lua
            .eval_callback::<mlua::Table>(&format!(
                "return btech.unit.fire({},1,{index},{})",
                id.0, target.0
            ))
            .unwrap();
        assert!(report.get::<mlua::Table>("ams").is_ok());
        assert_eq!(native.world().btech, lua.world().btech);
        let candidate = native.world().clone();
        persistence::save(&config.database(), &candidate)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            candidate.btech
        );
    }
}

/// Defense reduces clustered groups before damage and leaves failed Streak locks unspent.
#[tokio::test]
async fn ams_cluster_packets_and_streak_lock() {
    use stompymux_rs::*;
    for (weapon, locked) in [
        (Weapon::Lrm20, true),
        (Weapon::StreakSrm6, true),
        (Weapon::StreakSrm6, false),
    ] {
        let (_dir, config, mut world, id, target) = shot_fixture().await;
        install_test_ams(&mut world, target, Weapon::AntiMissileSystem);
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
        assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, target, ObjectId(2), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0, true);
        }
        set_battle_ams(&mut world, target, ObjectId(2), true).unwrap();
        face_attacker(&mut world, target);
        let mut definition = world.btech.constructed_units()[&id].definition().clone();
        let arm = definition.sections.get_mut(&MechSection::LeftArm).unwrap();
        let mut part = arm.criticals[&2].clone();
        part.equipment = weapon.name().into();
        arm.criticals.retain(|slot, _| *slot < 2);
        for slot in 2..2 + weapon.profile().critical_slots {
            arm.criticals.insert(slot, part.clone());
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
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["definition"] = serde_json::to_value(definition).unwrap();
                record["ammunition"][0] = weapon.profile().ammunition_per_ton.into();
            })
            .unwrap();
        let index = world.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|m| m.weapon == weapon)
            .unwrap();
        shot_skill(&mut world, 0);
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            "Gunnery-Missile",
            CharacterValue {
                value: if locked { 30 } else { 0 },
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = Dice::seeded([*seed; 32]);
                dice.two_d6() == 2 && dice.d6() == 4
            })
            .unwrap();
        shot_seed(&mut world, id, seed);
        let cluster_seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 7)
            .unwrap();
        shot_seed(&mut world, target, cluster_seed);
        let before = world.clone();
        let report =
            resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules()).unwrap();
        if !locked {
            assert!(!report.launched);
            assert!(report.ams.is_none());
            assert_eq!(
                world.btech.constructed_units()[&target],
                before.btech.constructed_units()[&target]
            );
        } else {
            // The engagement shifts the cluster roll of 7 down to 3 (twelve to six LRMs), and
            // treats the locked Streak as a roll of 11 shifted to 7 (six to four SRMs).
            let (hits, shot_down, packets) = if weapon == Weapon::Lrm20 {
                (12, 6, vec![5, 1])
            } else {
                (6, 2, vec![2, 2, 2, 2])
            };
            assert_eq!(report.ams.as_ref().unwrap().shot_down, shot_down);
            let salvo = report.salvo.as_ref().unwrap().as_mech().unwrap();
            assert_eq!(salvo.missiles_before_defense, Some(hits));
            assert_eq!(salvo.missiles_intercepted, Some(shot_down));
            assert_eq!(
                salvo.groups.iter().map(|g| g.damage).collect::<Vec<_>>(),
                packets
            );
        }
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

/// Selection skips unavailable or recycling mounts, but an empty first ready type ends the search.
#[tokio::test]
async fn ams_multiple_mount_selection_and_capability_loss() {
    use stompymux_rs::*;
    for case in [
        "first",
        "first_empty",
        "first_recycling",
        "first_flooded",
        "first_destroyed",
    ] {
        let (_dir, config, mut world, id, target) = shot_fixture().await;
        let (first, first_bin) = install_test_ams_at(
            &mut world,
            target,
            Weapon::AntiMissileSystem,
            MechSection::LeftArm,
        );
        let (second, second_bin) = install_test_ams_at(
            &mut world,
            target,
            Weapon::ClanAntiMissileSystem,
            MechSection::RightArm,
        );
        let (first_bin, second_bin) = (first_bin.unwrap(), second_bin.unwrap());
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
        assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, target, ObjectId(2), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0, true);
        }
        set_battle_ams(&mut world, target, ObjectId(2), true).unwrap();
        face_attacker(&mut world, target);
        shot_skill(&mut world, 30);
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            "Gunnery-Missile",
            CharacterValue {
                value: 30,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        let index = world.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|m| m.weapon == Weapon::Srm4)
            .unwrap();
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = Dice::seeded([*seed; 32]);
                dice.two_d6();
                dice.d6() == 6 && dice.d6() == 6
            })
            .unwrap();
        shot_seed(&mut world, id, seed);
        let cluster_seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        shot_seed(&mut world, target, cluster_seed);
        world
            .btech
            .rewrite_unit_record(target, |record| {
                if matches!(case, "first_empty" | "first_flooded") {
                    record["ammunition"][first_bin] = 0.into();
                }
                if case == "first_recycling" {
                    record["weapon_recycle"][first.to_string()] = 1.into();
                }
                if case == "first_flooded" {
                    record["flooded_sections"] = serde_json::json!(["LeftArm"]);
                }
            })
            .unwrap();
        if case == "first_destroyed" {
            destroy_battle_critical(
                &mut world,
                target,
                CriticalLocation {
                    section: MechSection::LeftArm,
                    slot: 4,
                },
            )
            .unwrap();
            assert!(!world.btech.constructed_units()[&target].ams_enabled());
            assert!(set_battle_ams(&mut world, target, ObjectId(2), true).is_err());
        }
        let before = world.clone();
        let report =
            resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules()).unwrap();
        let target_state = &world.btech.constructed_units()[&target];
        if matches!(case, "first_empty" | "first_destroyed") {
            assert!(report.ams.is_none());
            assert_eq!(
                target_state.ammunition()[second_bin],
                before.btech.constructed_units()[&target].ammunition()[second_bin]
            );
            assert!(!target_state.weapon_recycle().contains_key(&second));
        } else {
            let defense = report.ams.as_ref().unwrap();
            let use_first = case == "first";
            assert_eq!(defense.weapon_index, if use_first { first } else { second });
            assert_eq!(
                defense.ammunition_bin,
                Some(if use_first { first_bin } else { second_bin })
            );
            // One engagement: no die of its own, one round, one heat; the cluster roll of 12
            // becomes 8 and three of the four missiles still land.
            assert_eq!(defense.roll, None);
            assert_eq!(defense.shot_down, 1);
            assert_eq!(defense.ammunition_spent, 1);
            assert_eq!(
                target_state.heat().stored,
                before.btech.constructed_units()[&target].heat().stored + 1.0
            );
            assert_eq!(
                report
                    .salvo
                    .as_ref()
                    .unwrap()
                    .as_mech()
                    .unwrap()
                    .groups
                    .len(),
                3
            );
            let unused_bin = if use_first { second_bin } else { first_bin };
            assert_eq!(
                target_state.ammunition()[unused_bin],
                before.btech.constructed_units()[&target].ammunition()[unused_bin]
            );
        }
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

/// Add separate normal and explosive pod supplies without displacing the Jenner's SRM.
fn install_test_narc(
    world: &mut stompymux_rs::World,
    id: ObjectId,
    weapon: stompymux_rs::Weapon,
) -> (usize, usize) {
    use stompymux_rs::*;
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    let torso = definition
        .sections
        .get_mut(&MechSection::LeftTorso)
        .unwrap();
    for slot in 2..2 + weapon.profile().critical_slots {
        torso.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: weapon.name().into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    }
    for (slot, modes) in [(4, vec![]), (5, vec!["Narc/Smoke".into()])] {
        torso.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: format!("Ammo_{}", weapon.name()),
                data: "6".into(),
                modes,
            },
        );
    }
    let constructed = Mech::from_template(definition.clone()).unwrap();
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["definition"] = serde_json::to_value(definition).unwrap();
            record["ammunition"] = serde_json::to_value(constructed.ammunition()).unwrap();
        })
        .unwrap();
    let loadout = world.btech.constructed_units()[&id].loadout().unwrap();
    (
        loadout
            .weapons
            .iter()
            .position(|m| m.weapon == weapon)
            .unwrap(),
        loadout
            .ammunition
            .iter()
            .position(|b| b.weapon == weapon)
            .unwrap(),
    )
}

/// Pod hits preserve armor, explosive rounds deal damage, and intercepted or missed pods do not attach.
#[tokio::test]
async fn narc_pod_outcomes_and_restart() {
    use stompymux_rs::*;
    for weapon in [Weapon::NarcBeacon, Weapon::ClanNarcBeacon] {
        for case in ["hit", "miss", "intercepted", "explosive"] {
            let (_dir, config, mut world, id, target) = shot_fixture().await;
            let (index, normal_bin) = install_test_narc(&mut world, id, weapon);
            shot_skill(&mut world, 30);
            set_battle_character_value(
                &mut world,
                ObjectId(1),
                "Gunnery-Missile",
                CharacterValue {
                    value: if case == "miss" { 0 } else { 30 },
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
            if case == "intercepted" {
                install_test_ams(&mut world, target, Weapon::AntiMissileSystem);
                world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
                assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
                support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
                start_battle_unit(&mut world, target, ObjectId(2), true).unwrap();
                for _ in 0..5 {
                    advance_battle_units(&mut world, 0, true);
                }
                set_battle_ams(&mut world, target, ObjectId(2), true).unwrap();
                face_attacker(&mut world, target);
            }
            if case == "explosive" {
                assert_eq!(
                    toggle_battle_explosive(&mut world, id, ObjectId(1), index).unwrap(),
                    AmmunitionMode::Narc
                );
            }
            // A single-missile engagement rolls one die on the attacker's stream: 1-3
            // destroys the pod (Total Warfare p. 129).
            let seed = (0u32..65536)
                .find_map(|value| {
                    let mut bytes = [0; 32];
                    bytes[..4].copy_from_slice(&value.to_le_bytes());
                    let mut dice = Dice::seeded(bytes);
                    (dice.two_d6() == 2 && (case != "intercepted" || dice.d6() <= 3))
                        .then_some(bytes)
                })
                .unwrap();
            world.btech.set_unit_dice(id, Dice::seeded(seed)).unwrap();
            let target_seed = (0..=255)
                .find(|seed| {
                    let mut dice = Dice::seeded([*seed; 32]);
                    dice.two_d6() == 12 && dice.d6() < 6
                })
                .unwrap();
            shot_seed(&mut world, target, target_seed);
            let before = world.clone();
            let report =
                resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules())
                    .unwrap();
            if case == "explosive" {
                assert!(report.narc.is_none());
                assert!(report.salvo.is_some());
                assert!(
                    world.btech.constructed_units()[&target]
                        .narc_sections()
                        .is_empty()
                );
            } else {
                let pod = report.narc.as_ref().unwrap();
                assert_eq!(pod.hit, case != "miss");
                assert_eq!(pod.intercepted, case == "intercepted", "{:?}", report.ams);
                assert_eq!(pod.section.is_some(), case == "hit");
                assert!(report.salvo.is_none());
                assert_eq!(
                    world.btech.constructed_units()[&target].sections(),
                    before.btech.constructed_units()[&target].sections()
                );
                assert_eq!(
                    world.btech.constructed_units()[&id].ammunition()[normal_bin],
                    5
                );
                assert_eq!(world.btech.constructed_units()[&target].stun_remaining(), 0);
                assert!(pod.notices.is_empty());
                if matches!(case, "miss" | "intercepted") {
                    assert_eq!(
                        serde_json::to_value(&world.btech).unwrap()["constructed"]
                            [target.0.to_string()]["dice"],
                        serde_json::to_value(&before.btech).unwrap()["constructed"]
                            [target.0.to_string()]["dice"]
                    );
                }
            }
            assert_eq!(world.btech.constructed_units()[&id].heat().stored, 0.0);
            assert_eq!(
                world.btech.constructed_units()[&id].weapon_recycle()[&index],
                30
            );
            persistence::save(&config.database(), &world).await.unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(restored.btech, world.btech);
        }
    }
}

/// Compatible ammunition gains guidance only while a surviving target section carries a beacon.
#[tokio::test]
async fn narc_guidance_and_section_destruction() {
    use stompymux_rs::*;
    let (_dir, _config, mut world, id, target) = shot_fixture().await;
    let (index, _) = install_test_narc(&mut world, id, Weapon::NarcBeacon);
    shot_skill(&mut world, 30);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Gunnery-Missile",
        CharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    // A seven attaches to the center torso, away from ammunition and head effects.
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 7)
        .unwrap();
    shot_seed(&mut world, target, seed);
    let pod =
        resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules()).unwrap();
    let UnitSection::Mech(section) = pod.narc.unwrap().section.unwrap() else {
        panic!("Expected Mech attachment")
    };
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["definition"]["sections"]["RightTorso"]["criticals"]["0"]["modes"] =
                serde_json::json!(["Narc/Smoke"]);
        })
        .unwrap();
    let srm = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == Weapon::Srm4)
        .unwrap();
    toggle_battle_narc(&mut world, id, ObjectId(1), srm).unwrap();
    let cluster_seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 6)
        .unwrap();
    for marked in [false, true] {
        let mut candidate = world.clone();
        if !marked {
            candidate
                .btech
                .rewrite_unit_record(target, |record| {
                    record["beacons"] = serde_json::json!({});
                })
                .unwrap();
        }
        shot_seed(&mut candidate, target, cluster_seed);
        let report =
            resolve_battle_shot(&mut candidate, id, ObjectId(1), target, srm, shot_rules())
                .unwrap();
        assert_eq!(
            report.salvo.unwrap().into_mech().unwrap().groups.len(),
            if marked { 3 } else { 2 }
        );
    }
    apply_damage_phase(&mut world, target, section, u16::MAX, DamagePhase::Internal).unwrap();
    assert!(
        world.btech.constructed_units()[&target]
            .narc_sections()
            .is_empty()
    );
}

/// Native and Lua controls and firing share transactional mode, pod and damage state.
#[tokio::test]
async fn narc_native_lua_controls_and_rollback() {
    use stompymux_rs::*;
    for explosive in [false, true] {
        let (_dir, config, mut world, id, target) = shot_fixture().await;
        let (index, _) = install_test_narc(&mut world, id, Weapon::NarcBeacon);
        shot_skill(&mut world, 30);
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            "Gunnery-Missile",
            CharacterValue {
                value: 30,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
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
        for command in ["narc", "explosive"] {
            let before = lua.world().btech.clone();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.{command}({},1,{index}); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
        }
        if explosive {
            let text = support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("explosive {index}"),
            );
            assert!(text.contains("explosive rounds"), "{text}");
            assert_eq!(
                lua.eval_callback::<String>(&format!(
                    "return btech.unit.explosive({},1,{index})",
                    id.0
                ))
                .unwrap(),
                "narc"
            );
            assert_eq!(native.world().btech, lua.world().btech);
            lua.drain_outbox();
        }
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.fire({},1,{index},{}); error('abort')",
                id.0, target.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(lua.drain_outbox().is_empty());
        support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("fire {index} #{}", target.0),
        );
        let report = lua
            .eval_callback::<mlua::Table>(&format!(
                "return btech.unit.fire({},1,{index},{})",
                id.0, target.0
            ))
            .unwrap();
        assert!(
            report
                .get::<mlua::Table>(if explosive { "salvo" } else { "narc" })
                .is_ok()
        );
        assert_eq!(native.world().btech, lua.world().btech);
    }
}

/// A pod transfers from a missing arm to the torso, whose destruction removes the beacon.
#[tokio::test]
async fn narc_attachment_transfers_off_destroyed_sections() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    let (index, _) = install_test_narc(&mut world, id, Weapon::ClanNarcBeacon);
    shot_skill(&mut world, 30);
    set_battle_character_value(
        &mut world,
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
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 4)
        .unwrap();
    shot_seed(&mut world, target, seed);
    apply_damage_phase(
        &mut world,
        target,
        MechSection::RightArm,
        u16::MAX,
        DamagePhase::Internal,
    )
    .unwrap();
    let report =
        resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules()).unwrap();
    assert_eq!(
        report.narc.unwrap().section,
        Some(UnitSection::Mech(MechSection::RightTorso))
    );
    apply_damage_phase(
        &mut world,
        target,
        MechSection::RightTorso,
        u16::MAX,
        DamagePhase::Internal,
    )
    .unwrap();
    assert!(
        world.btech.constructed_units()[&target]
            .narc_sections()
            .is_empty()
    );
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

/// Install a suite in a free torso block, preserving all existing weapon and ammunition indices.
fn install_test_electronics(
    world: &mut stompymux_rs::World,
    id: ObjectId,
    suite: stompymux_rs::ElectronicSuite,
) {
    use stompymux_rs::*;
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    let (name, first) = if suite == ElectronicSuite::Guardian {
        ("Ecm", 3)
    } else {
        ("AngelEcm", 5)
    };
    for slot in first..first + 2 {
        definition
            .sections
            .get_mut(&MechSection::RightTorso)
            .unwrap()
            .criticals
            .insert(
                slot,
                CriticalDefinition {
                    equipment: name.into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
    }
    Mech::from_template(definition.clone()).unwrap();
    world.btech.set_unit_definition(id, definition).unwrap();
}

/// Operating modes, team-sensitive fields and damage/shutdown loss survive database round trips.
#[tokio::test]
async fn electronics_modes_fields_damage_and_restart() {
    use stompymux_rs::*;
    for suite in [ElectronicSuite::Guardian, ElectronicSuite::Angel] {
        for cause in ["toggle", "critical", "section", "shutdown"] {
            let (_dir, config, mut world, id, target) = shot_fixture().await;
            install_test_electronics(&mut world, id, suite);
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["constructed"][id.0.to_string()]["signature"]["team"] = 1.into();
            state["constructed"][target.0.to_string()]["signature"]["team"] = 2.into();
            world.btech = serde_json::from_value(state).unwrap();
            assert_eq!(
                toggle_battle_electronics(&mut world, id, ObjectId(1), suite, ElectronicMode::Ecm)
                    .unwrap(),
                ElectronicMode::Ecm
            );
            assert!(
                battle_electronic_field(&world, id)
                    .unwrap()
                    .blocks_incoming_guidance()
            );
            assert!(
                battle_electronic_field(&world, target)
                    .unwrap()
                    .blocks_outgoing_guidance()
            );
            let notices = refresh_battle_electronic_fields(&mut world).unwrap();
            assert!(
                notices
                    .iter()
                    .any(|notice| notice.unit == target && notice.text.contains("static"))
            );
            assert!(
                refresh_battle_electronic_fields(&mut world)
                    .unwrap()
                    .is_empty()
            );
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
            match cause {
                "toggle" => {
                    toggle_battle_electronics(
                        &mut world,
                        id,
                        ObjectId(1),
                        suite,
                        ElectronicMode::Eccm,
                    )
                    .unwrap();
                }
                "critical" => {
                    let mut unit = world.btech.constructed_units()[&id].clone();
                    unit.destroy_critical(CriticalLocation {
                        section: MechSection::RightTorso,
                        slot: if suite == ElectronicSuite::Guardian {
                            3
                        } else {
                            5
                        },
                    })
                    .unwrap();
                    world
                        .btech
                        .rewrite_unit_record(id, |record| {
                            *record = serde_json::to_value(unit).unwrap();
                        })
                        .unwrap();
                    assert!(
                        toggle_battle_electronics(
                            &mut world,
                            id,
                            ObjectId(1),
                            suite,
                            ElectronicMode::Ecm
                        )
                        .is_err()
                    );
                }
                "section" => {
                    apply_damage_phase(
                        &mut world,
                        id,
                        MechSection::RightTorso,
                        u16::MAX,
                        DamagePhase::Internal,
                    )
                    .unwrap();
                }
                _ => {
                    stop_battle_unit(&mut world, id, ObjectId(1), fall_rules()).unwrap();
                }
            }
            assert!(
                !battle_electronic_field(&world, target)
                    .unwrap()
                    .blocks_outgoing_guidance()
            );
            assert!(
                refresh_battle_electronic_fields(&mut world)
                    .unwrap()
                    .iter()
                    .any(|notice| notice.unit == target && notice.text.contains("back to normal"))
            );
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
        }
    }
}

/// All four native/Lua suite controls share exclusivity, feedback, inspection and callback rollback.
#[tokio::test]
async fn electronics_native_lua_controls_and_rollback() {
    use stompymux_rs::*;
    for (suite, ecm, eccm) in [
        (ElectronicSuite::Guardian, "ecm", "eccm"),
        (ElectronicSuite::Angel, "angelecm", "angeleccm"),
    ] {
        let (_dir, config, mut world, id, _) = shot_fixture().await;
        install_test_electronics(&mut world, id, suite);
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
        for (command, mode) in [(ecm, "ecm"), (eccm, "eccm"), (eccm, "off")] {
            let before = lua.world().btech.clone();
            assert!(
                lua.eval_callback::<()>(&format!(
                    "btech.unit.{command}({},1); error('abort')",
                    id.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, before);
            assert!(lua.drain_outbox().is_empty());
            let text = support::run_text(&native, &config, ObjectId(1), 1, command);
            assert!(
                text.contains("suite online") || text.contains("suite offline"),
                "{text}"
            );
            assert_eq!(
                lua.eval_callback::<String>(&format!("return btech.unit.{command}({},1)", id.0))
                    .unwrap(),
                mode
            );
            assert_eq!(native.world().btech, lua.world().btech);
            lua.drain_outbox();
        }
    }
}

/// Current fields suppress both guidance modes; equal ECCM restores guidance without changing ammunition selection.
#[tokio::test]
async fn electronics_suppress_narc_and_artemis_guidance() {
    use stompymux_rs::*;
    for mode in [AmmunitionMode::Narc, AmmunitionMode::Artemis] {
        for suite in [ElectronicSuite::Guardian, ElectronicSuite::Angel] {
            for case in ["clear", "blocked", "countered", "short_counter"] {
                let (_dir, config, mut world, id, target) = shot_fixture().await;
                install_test_electronics(&mut world, target, suite);
                install_test_electronics(
                    &mut world,
                    id,
                    if case == "short_counter" {
                        ElectronicSuite::Guardian
                    } else {
                        suite
                    },
                );
                world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
                assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
                support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
                start_battle_unit(&mut world, target, ObjectId(2), true).unwrap();
                for _ in 0..5 {
                    advance_battle_units(&mut world, 0, true);
                }
                shot_skill(&mut world, 30);
                set_battle_character_value(
                    &mut world,
                    ObjectId(1),
                    "Gunnery-Missile",
                    CharacterValue {
                        value: 30,
                        experience: 0,
                        last_used: 0,
                    },
                )
                .unwrap();
                let mut definition = world.btech.constructed_units()[&id].definition().clone();
                let jump = definition
                    .sections
                    .get_mut(&MechSection::CenterTorso)
                    .unwrap()
                    .criticals
                    .remove(&11)
                    .unwrap();
                definition
                    .sections
                    .get_mut(&MechSection::LeftTorso)
                    .unwrap()
                    .criticals
                    .insert(11, jump);
                definition
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
                    );
                definition
                    .sections
                    .get_mut(&MechSection::RightTorso)
                    .unwrap()
                    .criticals
                    .get_mut(&0)
                    .unwrap()
                    .modes = vec![
                    if mode == AmmunitionMode::Narc {
                        "Narc/Smoke"
                    } else {
                        "Artemis/Mine"
                    }
                    .into(),
                ];
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["constructed"][id.0.to_string()]["definition"] =
                    serde_json::to_value(definition).unwrap();
                state["constructed"][id.0.to_string()]["signature"]["team"] = 1.into();
                state["constructed"][target.0.to_string()]["signature"]["team"] = 2.into();
                state["constructed"][target.0.to_string()]["beacons"] =
                    serde_json::json!({"RightArm":["narc"]});
                world.btech = serde_json::from_value(state).unwrap();
                let index = world.btech.constructed_units()[&id]
                    .loadout()
                    .unwrap()
                    .weapons
                    .iter()
                    .position(|m| m.weapon == Weapon::Srm4)
                    .unwrap();
                if mode == AmmunitionMode::Narc {
                    toggle_battle_narc(&mut world, id, ObjectId(1), index).unwrap();
                } else {
                    toggle_battle_artemis(&mut world, id, ObjectId(1), index).unwrap();
                }
                if case != "clear" {
                    toggle_battle_electronics(
                        &mut world,
                        target,
                        ObjectId(2),
                        suite,
                        ElectronicMode::Ecm,
                    )
                    .unwrap();
                }
                if matches!(case, "countered" | "short_counter") {
                    toggle_battle_electronics(
                        &mut world,
                        id,
                        ObjectId(1),
                        if case == "short_counter" {
                            ElectronicSuite::Guardian
                        } else {
                            suite
                        },
                        ElectronicMode::Eccm,
                    )
                    .unwrap();
                }
                let seed = (0..=255)
                    .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 6)
                    .unwrap();
                shot_seed(&mut world, target, seed);
                // Firing queries current emissions even before the next observation heartbeat.
                assert_eq!(
                    world.btech.constructed_units()[&target].electronics().field,
                    ElectronicField::default()
                );
                let before = world.clone();
                let report =
                    resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules())
                        .unwrap();
                let blocked = case == "blocked"
                    || (case == "short_counter" && suite == ElectronicSuite::Angel);
                assert_eq!(
                    report.salvo.unwrap().into_mech().unwrap().groups.len(),
                    if blocked { 2 } else { 3 },
                    "{mode:?} {suite:?} {case}"
                );
                assert_eq!(report.expenditure.ammunition_mode, mode);
                assert_eq!(
                    world.btech.constructed_units()[&id].ammunition()[0],
                    before.btech.constructed_units()[&id].ammunition()[0] - 1
                );
                persistence::save(&config.database(), &world).await.unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    world.btech
                );
            }
        }
    }
}

/// Angel interference removes Streak lock protection and guaranteed rack hits; Guardian interference does not.
#[tokio::test]
async fn electronics_angel_disables_streak_homing() {
    use stompymux_rs::*;
    for weapon in [Weapon::StreakSrm4, Weapon::ClanStreakSrm4] {
        for suite in [ElectronicSuite::Guardian, ElectronicSuite::Angel] {
            for (hit, glance) in [(false, false), (true, false), (true, true)] {
                let (_dir, config, mut world, id, target) = shot_fixture().await;
                install_test_electronics(&mut world, target, suite);
                world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
                assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
                support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
                start_battle_unit(&mut world, target, ObjectId(2), true).unwrap();
                for _ in 0..5 {
                    advance_battle_units(&mut world, 0, true);
                }
                toggle_battle_electronics(
                    &mut world,
                    target,
                    ObjectId(2),
                    suite,
                    ElectronicMode::Ecm,
                )
                .unwrap();
                shot_skill(&mut world, 30);
                set_battle_character_value(
                    &mut world,
                    ObjectId(1),
                    "Gunnery-Missile",
                    CharacterValue {
                        value: if hit { 30 } else { 0 },
                        experience: 0,
                        last_used: 0,
                    },
                )
                .unwrap();
                let mut definition = world.btech.constructed_units()[&id].definition().clone();
                let jump = definition
                    .sections
                    .get_mut(&MechSection::CenterTorso)
                    .unwrap()
                    .criticals
                    .remove(&11)
                    .unwrap();
                definition
                    .sections
                    .get_mut(&MechSection::LeftTorso)
                    .unwrap()
                    .criticals
                    .insert(11, jump);
                for slot in 10..10 + weapon.profile().critical_slots {
                    definition
                        .sections
                        .get_mut(&MechSection::CenterTorso)
                        .unwrap()
                        .criticals
                        .insert(
                            slot,
                            CriticalDefinition {
                                equipment: weapon.name().into(),
                                data: "-".into(),
                                modes: vec![],
                            },
                        );
                }
                let bin = definition
                    .sections
                    .get_mut(&MechSection::RightTorso)
                    .unwrap()
                    .criticals
                    .get_mut(&0)
                    .unwrap();
                bin.equipment = format!("Ammo_{}", weapon.name());
                let mut state = serde_json::to_value(&world.btech).unwrap();
                state["constructed"][id.0.to_string()]["definition"] =
                    serde_json::to_value(definition).unwrap();
                state["constructed"][id.0.to_string()]["signature"]["team"] = 1.into();
                state["constructed"][target.0.to_string()]["signature"]["team"] = 2.into();
                world.btech = serde_json::from_value(state).unwrap();
                let index = world.btech.constructed_units()[&id]
                    .loadout()
                    .unwrap()
                    .weapons
                    .iter()
                    .position(|m| m.weapon == weapon)
                    .unwrap();
                let attack_seed = (0..=255)
                    .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
                    .unwrap();
                let cluster_seed = (0..=255)
                    .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 6)
                    .unwrap();
                shot_seed(&mut world, id, attack_seed);
                shot_seed(&mut world, target, cluster_seed);
                let rules = shot_rules();
                if glance {
                    // A roll of two against a target number of two glances (Tactical Operations).
                    let mut preview = world.clone();
                    set_battle_character_value(
                        &mut preview,
                        ObjectId(1),
                        "Gunnery-Missile",
                        CharacterValue {
                            value: 0,
                            experience: 0,
                            last_used: 0,
                        },
                    )
                    .unwrap();
                    let baseline =
                        resolve_battle_shot(&mut preview, id, ObjectId(1), target, index, rules)
                            .unwrap()
                            .target_number
                            .unwrap();
                    set_battle_character_value(
                        &mut world,
                        ObjectId(1),
                        "Gunnery-Missile",
                        CharacterValue {
                            value: (baseline - 2).try_into().unwrap(),
                            experience: 0,
                            last_used: 0,
                        },
                    )
                    .unwrap();
                }
                let before = world.clone();
                let report =
                    resolve_battle_shot(&mut world, id, ObjectId(1), target, index, rules).unwrap();
                let angel = suite == ElectronicSuite::Angel;
                assert_eq!(report.streak_confused, angel);
                let landed = hit;
                // A confused Streak still launches nothing on a failed roll (TacOps: AUE p. 91).
                assert_eq!(report.launched, landed);
                assert_eq!(report.glancing, angel && glance);
                assert!(
                    !report
                        .notices()
                        .iter()
                        .any(|notice| notice.text.contains("nicked by a glancing"))
                );
                assert_eq!(
                    world.btech.constructed_units()[&id].ammunition()[0],
                    before.btech.constructed_units()[&id].ammunition()[0] - u16::from(landed)
                );
                if landed {
                    assert_eq!(
                        report.salvo.unwrap().into_mech().unwrap().groups.len(),
                        if angel { if glance { 1 } else { 2 } } else { 4 }
                    );
                } else {
                    assert!(report.salvo.is_none());
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

/// Flooding disables electronic selections, while moving a recipient out of range or to another map clears its field.
#[tokio::test]
async fn electronics_flooding_and_map_membership() {
    use stompymux_rs::*;
    for suite in [ElectronicSuite::Guardian, ElectronicSuite::Angel] {
        let (_dir, config, mut wet, id) = water_fixture(2).await;
        install_test_electronics(&mut wet, id, suite);
        toggle_battle_electronics(&mut wet, id, ObjectId(1), suite, ElectronicMode::Ecm).unwrap();
        apply_damage_phase(
            &mut wet,
            id,
            MechSection::RightTorso,
            8,
            DamagePhase::Armor { rear: false },
        )
        .unwrap();
        flood_battle_unit(&mut wet, id, fall_rules()).unwrap();
        assert!(
            !wet.btech.constructed_units()[&id]
                .electronic_suite_available(suite)
                .unwrap()
        );
        assert_eq!(
            wet.btech.constructed_units()[&id].electronics().guardian,
            ElectronicMode::Off
        );
        assert_eq!(
            wet.btech.constructed_units()[&id].electronics().angel,
            ElectronicMode::Off
        );
        persistence::save(&config.database(), &wet).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            wet.btech
        );
    }
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    install_test_electronics(&mut world, id, ElectronicSuite::Guardian);
    toggle_battle_electronics(
        &mut world,
        id,
        ObjectId(1),
        ElectronicSuite::Guardian,
        ElectronicMode::Ecm,
    )
    .unwrap();
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    refresh_battle_electronic_fields(&mut world).unwrap();
    assert!(
        battle_electronic_field(&world, target)
            .unwrap()
            .blocks_incoming_guidance()
    );
    place_battle_unit(&mut world, target, map, 11, 0).unwrap();
    assert!(battle_unit_range(&world, id, target).unwrap().spatial > 6.0);
    assert_eq!(
        battle_electronic_field(&world, target).unwrap(),
        ElectronicField::default()
    );
    refresh_battle_electronic_fields(&mut world).unwrap();
    let other = world.create(&config, "Other field".into(), Kind::Room);
    let row = ".0".repeat(12);
    create_battle_map(
        &mut world,
        other,
        "other.map",
        MapAsset::from_cells(&format!("12 12\n{}", format!("{row}\n").repeat(12))).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, other, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, target, other, 5, 5).unwrap();
    assert_eq!(
        battle_electronic_field(&world, target).unwrap(),
        ElectronicField::default()
    );
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

/// The actual one-second server heartbeat commits electronic observations without requiring a weapon attack.
#[tokio::test]
async fn electronics_server_heartbeat_persists_field() {
    use stompymux_rs::*;
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_dir, config, mut world, id, target) = shot_fixture().await;
            install_test_electronics(&mut world, id, ElectronicSuite::Angel);
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["constructed"][id.0.to_string()]["signature"]["team"] = 1.into();
            state["constructed"][target.0.to_string()]["signature"]["team"] = 2.into();
            world.btech = serde_json::from_value(state).unwrap();
            toggle_battle_electronics(
                &mut world,
                id,
                ObjectId(1),
                ElectronicSuite::Angel,
                ElectronicMode::Ecm,
            )
            .unwrap();
            assert_eq!(
                world.btech.constructed_units()[&target].electronics().field,
                ElectronicField::default()
            );
            persistence::save(&config.database(), &world).await.unwrap();
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let (shutdown, request) = tokio::sync::oneshot::channel();
            let server_config = config.clone();
            let (driver, trigger) = HeartbeatDriver::manual();
            let mut heartbeats = support::Heartbeats::new(trigger, scripts.progress(), &config);
            let task = tokio::task::spawn_local(async move {
                run_with_schedule_clock(
                    server_config,
                    scripts,
                    listener,
                    async { request.await.unwrap() },
                    || 1,
                    driver,
                )
                .await
            });
            heartbeats.ready().await;
            let saved = heartbeats
                .until_saved(&config, 10, |saved| {
                    saved.btech.constructed_units()[&target]
                        .electronics()
                        .field
                        .angel_disturbed
                })
                .await;
            assert!(
                saved.btech.constructed_units()[&id]
                    .electronics()
                    .field
                    .angel_protected
            );
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}

/// Supply an iNarc launcher and separate bins for every selectable pod type.
fn install_test_inarc(world: &mut stompymux_rs::World, id: ObjectId) -> usize {
    use stompymux_rs::*;
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    let torso = definition
        .sections
        .get_mut(&MechSection::LeftTorso)
        .unwrap();
    for slot in 2..5 {
        torso.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: "IS.iNarcBeacon".into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    }
    for (slot, flag) in [
        (5, None),
        (6, Some("iNarc_Explosive")),
        (7, Some("iNarc_Haywire")),
        (8, Some("iNarc_ECM")),
        (9, Some("iNarc_Nemesis")),
    ] {
        torso.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: "Ammo_IS.iNarcBeacon".into(),
                data: "4".into(),
                modes: flag.map(|flag| vec![flag.into()]).unwrap_or_default(),
            },
        );
    }
    let unit = Mech::from_template(definition.clone()).unwrap();
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["definition"] = serde_json::to_value(definition).unwrap();
            record["ammunition"] = serde_json::to_value(unit.ammunition()).unwrap();
        })
        .unwrap();
    world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::INarcBeacon)
        .unwrap()
}

/// Every pod type uses matching ammunition; misses and interception leave no marks, and explosive rounds deal damage.
#[tokio::test]
async fn inarc_outcomes_supply_and_restart() {
    use stompymux_rs::*;
    for (mode, kind) in [
        (AmmunitionMode::Normal, Some(BeaconKind::Homing)),
        (AmmunitionMode::INarcExplosive, None),
        (AmmunitionMode::INarcHaywire, Some(BeaconKind::Haywire)),
        (AmmunitionMode::INarcEcm, Some(BeaconKind::Ecm)),
        (AmmunitionMode::INarcNemesis, Some(BeaconKind::Homing)),
    ] {
        for case in ["hit", "miss", "intercepted", "empty"] {
            let (_dir, config, mut world, id, target) = shot_fixture().await;
            let index = install_test_inarc(&mut world, id);
            set_battle_inarc_ammunition(&mut world, id, ObjectId(1), index, mode).unwrap();
            shot_skill(&mut world, 30);
            set_battle_character_value(
                &mut world,
                ObjectId(1),
                "Gunnery-Missile",
                CharacterValue {
                    value: if case == "miss" { 0 } else { 30 },
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
            if case == "intercepted" {
                install_test_ams(&mut world, target, Weapon::AntiMissileSystem);
                world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
                assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
                support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
                start_battle_unit(&mut world, target, ObjectId(2), true).unwrap();
                for _ in 0..5 {
                    advance_battle_units(&mut world, 0, true);
                }
                set_battle_ams(&mut world, target, ObjectId(2), true).unwrap();
                face_attacker(&mut world, target);
            }
            let bin = world.btech.constructed_units()[&id]
                .loadout()
                .unwrap()
                .ammunition
                .iter()
                .position(|bin| bin.weapon == Weapon::INarcBeacon && bin.mode == mode)
                .unwrap();
            if case == "empty" {
                world
                    .btech
                    .rewrite_unit_record(id, |record| {
                        record["ammunition"][bin] = 0.into();
                    })
                    .unwrap();
            }
            // A single-missile engagement rolls one die on the attacker's stream: 1-3
            // destroys the pod (Total Warfare p. 129).
            let seed = (0u32..65536)
                .find_map(|value| {
                    let mut bytes = [0; 32];
                    bytes[..4].copy_from_slice(&value.to_le_bytes());
                    let mut dice = Dice::seeded(bytes);
                    (dice.two_d6() == 2 && (case != "intercepted" || dice.d6() <= 3))
                        .then_some(bytes)
                })
                .unwrap();
            world.btech.set_unit_dice(id, Dice::seeded(seed)).unwrap();
            shot_seed(&mut world, target, 0);
            let before = world.clone();
            let result =
                resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules());
            if case == "empty" {
                assert!(result.is_err());
                assert_eq!(world.btech, before.btech);
                continue;
            }
            let report = result.unwrap();
            assert_eq!(world.btech.constructed_units()[&id].ammunition()[bin], 3);
            assert_eq!(
                world.btech.constructed_units()[&id].weapon_recycle()[&index],
                30
            );
            assert_eq!(report.expenditure.ammunition_mode, mode);
            if let Some(kind) = kind {
                let pod = report.narc.unwrap();
                assert_eq!(pod.kind, kind);
                assert_eq!(pod.hit, case != "miss");
                assert_eq!(pod.intercepted, case == "intercepted", "{:?}", report.ams);
                assert_eq!(
                    world.btech.constructed_units()[&target].has_beacon(kind),
                    case == "hit"
                );
                assert_eq!(
                    world.btech.constructed_units()[&target].sections(),
                    before.btech.constructed_units()[&target].sections()
                );
                assert!(report.salvo.is_none());
                if kind == BeaconKind::Ecm && case == "hit" {
                    assert!(battle_electronic_field(&world, target).unwrap().disturbed);
                    assert!(
                        pod.notices
                            .iter()
                            .any(|notice| notice.text.contains("static"))
                    );
                }
            } else {
                assert!(report.narc.is_none());
                if case == "hit" {
                    assert_eq!(
                        report
                            .salvo
                            .unwrap()
                            .into_mech()
                            .unwrap()
                            .groups
                            .iter()
                            .map(|group| group.damage)
                            .sum::<u16>(),
                        6
                    );
                }
                assert!(
                    world.btech.constructed_units()[&target]
                        .beacons()
                        .is_empty()
                );
            }
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
        }
    }
}

/// Pod effects coexist, modify aim independently of ECM, and all disappear when their section is destroyed.
#[tokio::test]
async fn inarc_effects_coexist_and_follow_section_lifetime() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    let index = install_test_inarc(&mut world, id);
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["beacons"] = serde_json::json!({"CenterTorso":["narc"]});
        })
        .unwrap();

    shot_skill(&mut world, 30);
    set_battle_character_value(
        &mut world,
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
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 7)
        .unwrap();
    for mode in [
        AmmunitionMode::Normal,
        AmmunitionMode::INarcHaywire,
        AmmunitionMode::INarcEcm,
    ] {
        set_battle_inarc_ammunition(&mut world, id, ObjectId(1), index, mode).unwrap();
        shot_seed(&mut world, target, seed);
        let report =
            resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules()).unwrap();
        assert_eq!(
            report.narc.unwrap().section,
            Some(UnitSection::Mech(MechSection::CenterTorso))
        );
        for _ in 0..30 {
            advance_battle_recycle(&mut world);
        }
    }
    assert_eq!(
        world.btech.constructed_units()[&target].beacons()[&MechSection::CenterTorso].len(),
        4
    );
    let target_laser = world.btech.constructed_units()[&target]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == Weapon::MediumLaser)
        .unwrap();
    assert_eq!(
        battle_aim_modifiers(&world, target, id, target_laser, 4, shot_rules().aim)
            .unwrap()
            .beacon_accuracy,
        1
    );
    let srm = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == Weapon::Srm4)
        .unwrap();
    toggle_battle_narc(&mut world, id, ObjectId(1), srm).unwrap();
    assert_eq!(
        battle_aim_modifiers(&world, id, target, srm, 4, shot_rules().aim)
            .unwrap()
            .beacon_accuracy,
        -1
    );
    assert!(
        battle_electronic_field(&world, target)
            .unwrap()
            .blocks_outgoing_guidance()
    );
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["definition"]["sections"]["RightTorso"]["criticals"]["0"]["modes"] =
                serde_json::json!(["Narc/Smoke"]);
        })
        .unwrap();
    let cluster_seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 6)
        .unwrap();
    shot_seed(&mut world, target, cluster_seed);
    let salvo =
        resolve_battle_shot(&mut world, id, ObjectId(1), target, srm, shot_rules()).unwrap();
    assert_eq!(salvo.salvo.unwrap().into_mech().unwrap().groups.len(), 3); // Narc and iNarc share one bonus.
    apply_damage_phase(
        &mut world,
        target,
        MechSection::CenterTorso,
        u16::MAX,
        DamagePhase::Internal,
    )
    .unwrap();
    assert!(
        world.btech.constructed_units()[&target]
            .beacons()
            .is_empty()
    );
    assert!(
        !battle_electronic_field(&world, target)
            .unwrap()
            .blocks_outgoing_guidance()
    );
    refresh_battle_electronic_fields(&mut world).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
}

/// Explicit selection is idempotent and native/Lua pod firing rolls back the complete beacon and electronic state.
#[tokio::test]
async fn inarc_native_lua_selection_fire_and_rollback() {
    use stompymux_rs::*;
    for selector in ["-", "X", "Y", "E", "Z"] {
        let (_dir, config, mut world, id, target) = shot_fixture().await;
        let index = install_test_inarc(&mut world, id);
        shot_skill(&mut world, 30);
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            "Gunnery-Missile",
            CharacterValue {
                value: 30,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
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
        for _ in 0..2 {
            support::run_text(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("inarc {index} {selector}"),
            );
            lua.eval_callback::<String>(&format!(
                "return btech.unit.inarc({},1,{index},'{selector}')",
                id.0
            ))
            .unwrap();
            assert_eq!(native.world().btech, lua.world().btech);
        }
        lua.drain_outbox();
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.fire({},1,{index},{}); error('abort')",
                id.0, target.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(lua.drain_outbox().is_empty());
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
    }
}

/// Give the test biped complete lower-arm/hand actuators and a selected attached effect.
fn prepare_test_pod_removal(
    world: &mut stompymux_rs::World,
    id: ObjectId,
    section: stompymux_rs::MechSection,
    kind: stompymux_rs::BeaconKind,
) {
    use stompymux_rs::*;
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    for arm in [MechSection::LeftArm, MechSection::RightArm] {
        for (slot, equipment) in [(2, "LowerActuator"), (3, "HandOrFootActuator")] {
            definition.sections.get_mut(&arm).unwrap().criticals.insert(
                slot,
                CriticalDefinition {
                    equipment: equipment.into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
        }
    }
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["definition"] = serde_json::to_value(definition).unwrap();
            record["beacons"] = serde_json::to_value(std::collections::BTreeMap::from([(
                section,
                std::collections::BTreeSet::from([kind]),
            )]))
            .unwrap();
        })
        .unwrap();
}

/// Swatting chooses the less impaired available arm, requires the opposite arm for arm pods, and recovers on hits or misses.
#[tokio::test]
async fn pod_removal_arm_selection_damage_and_restart() {
    use stompymux_rs::*;
    for case in [
        "success",
        "failure",
        "upper",
        "lower",
        "hand",
        "right_better",
        "left_recycle",
        "both_recycle",
        "left_pod",
        "blocked_left_pod",
    ] {
        let (_dir, config, mut world, id, _) = shot_fixture().await;
        let section = if case.contains("left_pod") {
            MechSection::LeftArm
        } else {
            MechSection::CenterTorso
        };
        prepare_test_pod_removal(&mut world, id, section, BeaconKind::Homing);
        shot_skill(&mut world, 30);
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            "Piloting-Biped",
            CharacterValue {
                value: if case == "success" { 30 } else { 0 },
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        world
            .btech
            .rewrite_unit_record(id, |record| {
                if matches!(case, "upper" | "lower" | "hand" | "right_better") {
                    let slot = match case {
                        "upper" => 1,
                        "lower" => 2,
                        _ => 3,
                    };
                    record["lost_criticals"] =
                        serde_json::json!([{"section":"LeftArm","slot":slot}]);
                    if case != "right_better" {
                        record["limb_recycle"] = serde_json::json!({"RightArm":10});
                    }
                }
                if case == "left_recycle" {
                    record["limb_recycle"] = serde_json::json!({"LeftArm":10});
                }
                if case == "both_recycle" {
                    record["limb_recycle"] = serde_json::json!({"LeftArm":10,"RightArm":10});
                }
                if case == "blocked_left_pod" {
                    record["limb_recycle"] = serde_json::json!({"RightArm":10});
                }
            })
            .unwrap();
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
            .unwrap();
        shot_seed(&mut world, id, seed);
        let before = world.clone();
        let report = remove_battle_pod(
            &mut world,
            id,
            ObjectId(1),
            section,
            BeaconKind::Homing,
            fall_rules(),
        );
        if matches!(case, "both_recycle" | "blocked_left_pod") {
            assert!(report.is_err());
            assert_eq!(world.btech, before.btech);
            continue;
        }
        let report = report.unwrap();
        let removed = case == "success";
        assert_eq!(report.removed, removed);
        assert_eq!(
            report.arm,
            if matches!(case, "right_better" | "left_recycle" | "left_pod") {
                Arm::Right
            } else {
                Arm::Left
            }
        );
        assert_eq!(
            report.self_damage,
            if removed {
                0
            } else if matches!(case, "upper" | "lower") {
                2
            } else {
                4
            }
        );
        assert_eq!(
            world.btech.constructed_units()[&id].limb_recycle()[&report.arm.section()],
            60
        );
        assert_eq!(
            world.btech.constructed_units()[&id].has_beacon(BeaconKind::Homing),
            !removed
        );
        assert_eq!(
            world.btech.character_values(),
            before.btech.character_values()
        );
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

/// Read-only inspection and removal share native/Lua results; callback abort restores injury, dice, recovery and pod state.
#[tokio::test]
async fn pod_inspection_and_removal_native_lua_rollback() {
    use stompymux_rs::*;
    for (character, success) in [(false, true), (false, false), (true, false)] {
        let (_dir, config, mut world, id, _) = shot_fixture().await;
        prepare_test_pod_removal(&mut world, id, MechSection::Head, BeaconKind::Ecm);
        if character {
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
        }
        shot_skill(&mut world, 30);
        let skill = if config.battletech.extended_piloting != 0 {
            "Piloting-Biped"
        } else {
            "Piloting-Battlemech"
        };
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            skill,
            CharacterValue {
                value: if success { 30 } else { 0 },
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
            .unwrap();
        shot_seed(&mut world, id, seed);
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
        let text = support::run_text(&native, &config, ObjectId(1), 1, "pods");
        assert!(text.contains("Head") && text.contains("iECM"), "{text}");
        assert_eq!(
            lua.eval_callback::<mlua::Table>(&format!("return btech.unit.pods({},1)", id.0))
                .unwrap()
                .raw_len(),
            8
        );
        assert_eq!(native.world().btech, world.btech);
        let before = lua.world().btech.clone();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.removepod({},1,'H','E'); error('abort')",
                id.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, before);
        assert!(lua.drain_outbox().is_empty());
        let text = support::run_text(&native, &config, ObjectId(1), 1, "removepod H E");
        assert!(text.contains("swat"), "{text}");
        let report = lua
            .eval_callback::<mlua::Table>(&format!(
                "return btech.unit.removepod({},1,'H','E')",
                id.0
            ))
            .unwrap();
        assert_eq!(report.get::<bool>("removed").unwrap(), success);
        assert_eq!(native.world().btech, lua.world().btech);
        let saved_world = native.world().clone();
        persistence::save(&config.database(), &saved_world)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            native.world().btech
        );
    }
}

/// Removing one effect preserves neighboring pods, clears self-interference, and enforces cockpit authorization atomically.
#[tokio::test]
async fn pod_removal_preserves_other_effects_and_rejects_invalid_attempts() {
    use stompymux_rs::*;
    let (_dir, _config, mut world, id, _) = shot_fixture().await;
    assert!(
        inspect_battle_pods(&world, id, ObjectId(1))
            .unwrap()
            .is_empty()
    );
    prepare_test_pod_removal(&mut world, id, MechSection::CenterTorso, BeaconKind::Ecm);
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["beacons"]["CenterTorso"] =
                serde_json::json!(["narc", "homing", "haywire", "ecm"]);
        })
        .unwrap();
    shot_skill(&mut world, 30);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Piloting-Biped",
        CharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    for (pilot, section, kind) in [
        (ObjectId(1), MechSection::CenterTorso, BeaconKind::Narc),
        (ObjectId(1), MechSection::Head, BeaconKind::Homing),
        (ObjectId(0), MechSection::CenterTorso, BeaconKind::Ecm),
    ] {
        let before = world.btech.clone();
        assert!(remove_battle_pod(&mut world, id, pilot, section, kind, fall_rules()).is_err());
        assert_eq!(world.btech, before);
    }
    assert!(battle_electronic_field(&world, id).unwrap().disturbed);
    let report = remove_battle_pod(
        &mut world,
        id,
        ObjectId(1),
        MechSection::CenterTorso,
        BeaconKind::Ecm,
        fall_rules(),
    )
    .unwrap();
    assert!(report.removed);
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(
        unit.beacons()[&MechSection::CenterTorso],
        std::collections::BTreeSet::from([
            BeaconKind::Narc,
            BeaconKind::Homing,
            BeaconKind::Haywire,
        ])
    );
    assert!(!battle_electronic_field(&world, id).unwrap().disturbed);
}

/// Install a complete stealth layout in free slots without changing weapons or ammunition order.
fn install_test_stealth(world: &mut stompymux_rs::World, id: ObjectId) {
    use stompymux_rs::*;
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    for section in MechSection::ALL {
        if matches!(section, MechSection::Head | MechSection::CenterTorso) {
            continue;
        }
        for slot in [4, 5] {
            definition
                .sections
                .get_mut(&section)
                .unwrap()
                .criticals
                .insert(
                    slot,
                    CriticalDefinition {
                        equipment: "StealthArmor".into(),
                        data: "-".into(),
                        modes: vec![],
                    },
                );
        }
    }
    for slot in [6, 7] {
        definition
            .sections
            .get_mut(&MechSection::LeftTorso)
            .unwrap()
            .criticals
            .insert(
                slot,
                CriticalDefinition {
                    equipment: "Ecm".into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
    }
    world.btech.set_unit_definition(id, definition).unwrap();
}

/// Saved switches complete at thirty ticks; damage/shutdown clear effects and invalid expiries are consumed.
#[tokio::test]
async fn stealth_switch_effects_damage_and_restart() {
    use stompymux_rs::*;
    for case in ["switch", "damage", "shutdown", "interrupted"] {
        let (_dir, config, mut world, id, _) = shot_fixture().await;
        install_test_stealth(&mut world, id);
        toggle_battle_stealth(&mut world, id, ObjectId(1)).unwrap();
        let before = world.btech.clone();
        assert!(toggle_battle_stealth(&mut world, id, ObjectId(1)).is_err());
        assert_eq!(world.btech, before);
        for _ in 0..29 {
            assert!(advance_battle_stealth(&mut world).is_empty());
        }
        assert!(!world.btech.constructed_units()[&id].stealth().enabled);
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        if case == "interrupted" {
            stop_battle_unit(&mut world, id, ObjectId(1), fall_rules()).unwrap();
            assert!(advance_battle_stealth(&mut world).is_empty());
            assert_eq!(
                world.btech.constructed_units()[&id].stealth(),
                SignatureState::default()
            );
            continue;
        }
        assert_eq!(advance_battle_stealth(&mut world).len(), 1);
        assert!(world.btech.constructed_units()[&id].stealth().enabled);
        assert_eq!(
            world.btech.constructed_units()[&id]
                .heat_rates(&world)
                .production,
            10.0
        );
        assert!(battle_electronic_field(&world, id).unwrap().disturbed);
        match case {
            "switch" => {
                toggle_battle_stealth(&mut world, id, ObjectId(1)).unwrap();
                for _ in 0..29 {
                    advance_battle_stealth(&mut world);
                }
                assert!(world.btech.constructed_units()[&id].stealth().enabled);
                advance_battle_stealth(&mut world);
            }
            "damage" => {
                destroy_battle_critical(
                    &mut world,
                    id,
                    CriticalLocation {
                        section: MechSection::LeftTorso,
                        slot: 6,
                    },
                )
                .unwrap();
            }
            _ => {
                stop_battle_unit(&mut world, id, ObjectId(1), fall_rules()).unwrap();
            }
        }
        assert!(!world.btech.constructed_units()[&id].stealth().enabled);
        assert_eq!(
            world.btech.constructed_units()[&id]
                .heat_rates(&world)
                .production,
            0.0
        );
        assert!(!battle_electronic_field(&world, id).unwrap().disturbed);
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

/// Native and Lua requests share validation, saved selection and rollback of staged notifications.
#[tokio::test]
async fn stealth_native_lua_and_rollback() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, _) = shot_fixture().await;
    install_test_stealth(&mut world, id);
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
        lua.eval_callback::<()>(&format!("btech.unit.stealth({},1); error('abort')", id.0))
            .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    assert!(
        support::run_text(&native, &config, ObjectId(1), 1, "stealth")
            .contains("begins to come online")
    );
    assert!(
        lua.eval_callback::<bool>(&format!("return btech.unit.stealth({},1)", id.0))
            .unwrap()
    );
    assert_eq!(native.world().btech, lua.world().btech);
}

/// The real heartbeat completes a persisted switch and saves its electronic consequences together.
#[tokio::test]
async fn stealth_server_switch_persists_field() {
    use stompymux_rs::*;
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_dir, config, mut world, id, _) = shot_fixture().await;
            install_test_stealth(&mut world, id);
            toggle_battle_stealth(&mut world, id, ObjectId(1)).unwrap();
            for _ in 0..29 {
                advance_battle_stealth(&mut world);
            }
            persistence::save(&config.database(), &world).await.unwrap();
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let (shutdown, request) = tokio::sync::oneshot::channel();
            let server_config = config.clone();
            let (driver, trigger) = HeartbeatDriver::manual();
            let mut heartbeats = support::Heartbeats::new(trigger, scripts.progress(), &config);
            let task = tokio::task::spawn_local(async move {
                run_with_schedule_clock(
                    server_config,
                    scripts,
                    listener,
                    async { request.await.unwrap() },
                    || 1,
                    driver,
                )
                .await
            });
            heartbeats.ready().await;
            let saved = heartbeats
                .until_saved(&config, 10, |saved| {
                    saved.btech.constructed_units()[&id].stealth().enabled
                })
                .await;
            let unit = &saved.btech.constructed_units()[&id];
            assert!(unit.stealth().pending.is_none());
            assert!(unit.electronics().field.disturbed);
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}

/// Actual weapon previews apply concealment to the target's range bracket, leaving all other aim terms intact.
#[tokio::test]
async fn stealth_weapon_preview_uses_target_state() {
    use stompymux_rs::*;
    let (_dir, _config, mut world, id, target) = shot_fixture().await;
    install_test_stealth(&mut world, target);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][target.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    state["constructed"][target.0.to_string()]["stealth"] =
        serde_json::json!({"enabled":true,"pending":null});
    world.btech = serde_json::from_value(state).unwrap();
    // Fire back at the running source so only the target's state can add concealment.
    let visible = battle_aim_modifiers(&world, target, id, 0, 4, optical_aim_rules()).unwrap();
    assert_eq!(visible.range.unwrap().modifier, 0);
    let concealed = battle_aim_modifiers(&world, id, target, 0, 4, optical_aim_rules()).unwrap();
    assert_eq!(concealed.range.unwrap().modifier, 0);
    // At medium range, the unchanged catalogue profile gains exactly one point.
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["position"]["y"] = 1.into();
            record["motion"] = serde_json::Value::Null;
        })
        .unwrap();
    let concealed = battle_aim_modifiers(&world, id, target, 0, 4, optical_aim_rules()).unwrap();
    assert_eq!(concealed.range.unwrap().modifier, 3);
}

/// Stealth requires a lock on this target; rejection must not consume attack state.
#[tokio::test]
async fn stealth_firing_requires_target_lock() {
    use stompymux_rs::*;
    for case in ["missing", "different", "selected"] {
        let (_dir, _config, mut world, id, target) = shot_fixture().await;
        install_test_stealth(&mut world, target);
        shot_skill(&mut world, 30);
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][target.0.to_string()]["power"] =
            serde_json::json!({"state":"running"});
        state["constructed"][target.0.to_string()]["stealth"] =
            serde_json::json!({"enabled":true,"pending":null});
        state["constructed"][id.0.to_string()]["target_lock"] = if case == "missing" {
            serde_json::Value::Null
        } else {
            serde_json::json!({"target": if case == "different" {id.0} else {target.0}})
        };
        world.btech = serde_json::from_value(state).unwrap();
        let before = world.btech.clone();
        let shot = resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, shot_rules());
        if case == "selected" {
            assert!(shot.is_ok(), "{shot:?}");
            assert_ne!(world.btech, before);
        } else {
            assert!(shot.unwrap_err().to_string().contains("stable lock"));
            assert_eq!(world.btech, before);
        }
    }
}

/// Install seven NSS devices in free slots, moving one torso jet to preserve the existing loadout.
fn install_test_nss(world: &mut stompymux_rs::World, id: ObjectId) {
    use stompymux_rs::*;
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    let jet = definition
        .sections
        .get_mut(&MechSection::CenterTorso)
        .unwrap()
        .criticals
        .remove(&11)
        .unwrap();
    definition
        .sections
        .get_mut(&MechSection::LeftTorso)
        .unwrap()
        .criticals
        .insert(8, jet);
    for section in MechSection::ALL
        .into_iter()
        .filter(|section| *section != MechSection::Head)
    {
        let slot = if section == MechSection::CenterTorso {
            11
        } else {
            if matches!(section, MechSection::LeftLeg | MechSection::RightLeg) {
                5
            } else {
                9
            }
        };
        definition
            .sections
            .get_mut(&section)
            .unwrap()
            .criticals
            .insert(
                slot,
                CriticalDefinition {
                    equipment: "NullSig_Device".into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
    }
    world.btech.set_unit_definition(id, definition).unwrap();
}

/// Device loss, shutdown and unavailable expiries clear NSS without supplying any ECM noise.
#[tokio::test]
async fn nss_switch_damage_accounting_and_restart() {
    use stompymux_rs::*;
    for case in ["switch", "damage", "shutdown", "interrupted"] {
        let (_dir, config, mut world, id, _) = shot_fixture().await;
        let before_mass = world.btech.constructed_units()[&id]
            .mass()
            .unwrap()
            .equipment;
        install_test_nss(&mut world, id);
        // The null signature system occupies seven slots but weighs nothing.
        assert_eq!(
            world.btech.constructed_units()[&id]
                .mass()
                .unwrap()
                .equipment,
            before_mass
        );
        toggle_battle_null_signature(&mut world, id, ObjectId(1)).unwrap();
        let before = world.btech.clone();
        assert!(toggle_battle_null_signature(&mut world, id, ObjectId(1)).is_err());
        assert_eq!(world.btech, before);
        for _ in 0..29 {
            assert!(advance_battle_null_signature(&mut world).is_empty());
        }
        assert!(
            !world.btech.constructed_units()[&id]
                .null_signature()
                .enabled
        );
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        if case == "interrupted" {
            destroy_battle_critical(
                &mut world,
                id,
                CriticalLocation {
                    section: MechSection::LeftArm,
                    slot: 9,
                },
            )
            .unwrap();
            assert!(advance_battle_null_signature(&mut world).is_empty());
            assert_eq!(
                world.btech.constructed_units()[&id].null_signature(),
                SignatureState::default()
            );
            continue;
        }
        assert_eq!(advance_battle_null_signature(&mut world).len(), 1);
        assert!(
            world.btech.constructed_units()[&id]
                .null_signature()
                .enabled
        );
        assert_eq!(
            world.btech.constructed_units()[&id]
                .heat_rates(&world)
                .production,
            10.0
        );
        assert!(!battle_electronic_field(&world, id).unwrap().disturbed);
        match case {
            "switch" => {
                toggle_battle_null_signature(&mut world, id, ObjectId(1)).unwrap();
                for _ in 0..30 {
                    advance_battle_null_signature(&mut world);
                }
            }
            "damage" => {
                destroy_battle_critical(
                    &mut world,
                    id,
                    CriticalLocation {
                        section: MechSection::LeftArm,
                        slot: 9,
                    },
                )
                .unwrap();
            }
            _ => {
                stop_battle_unit(&mut world, id, ObjectId(1), fall_rules()).unwrap();
            }
        }
        assert!(
            !world.btech.constructed_units()[&id]
                .null_signature()
                .enabled
        );
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

/// A null signature system cannot share a Mech with stealth armor; on its own it conceals the
/// unit at range without requiring a firing lock.
#[tokio::test]
async fn nss_excludes_stealth_armor_and_allows_unlocked_fire() {
    use stompymux_rs::*;
    let (_dir, _config, mut world, id, target) = shot_fixture().await;
    install_test_nss(&mut world, target);
    let mut definition = world.btech.constructed_units()[&target]
        .definition()
        .clone();
    let arm = &mut definition
        .sections
        .get_mut(&MechSection::LeftArm)
        .unwrap()
        .criticals;
    let free = (0..12).find(|slot| !arm.contains_key(slot)).unwrap();
    arm.insert(
        free,
        CriticalDefinition {
            equipment: "StealthArmor".into(),
            data: "-".into(),
            modes: vec![],
        },
    );
    assert!(
        Mech::from_template(definition)
            .unwrap_err()
            .to_string()
            .contains("cannot be combined with stealth armor")
    );
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][target.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    state["constructed"][target.0.to_string()]["null_signature"] =
        serde_json::json!({"enabled":true,"pending":null});
    world.btech = serde_json::from_value(state).unwrap();
    shot_skill(&mut world, 30);
    assert!(world.btech.constructed_units()[&target].range_concealed());
    assert!(world.btech.constructed_units()[&id].target_lock().is_none());
    let mut fired = world.clone();
    let result = resolve_battle_shot(&mut fired, id, ObjectId(1), target, 0, shot_rules());
    assert!(result.is_ok(), "{result:?}");
    let range = Weapon::MediumLaser.range_modifier(4.0).unwrap().unwrap();
    assert_eq!(
        range
            .against_stealth(world.btech.constructed_units()[&target].range_concealed())
            .modifier,
        3
    );
}

/// Native/Lua controls use identical transactions and discard requested switches on callback failure.
#[tokio::test]
async fn nss_native_lua_rollback() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, _) = shot_fixture().await;
    install_test_nss(&mut world, id);
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
        lua.eval_callback::<()>(&format!("btech.unit.nss({},1); error('abort')", id.0))
            .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    assert!(
        support::run_text(&native, &config, ObjectId(1), 1, "nss")
            .contains("begins to come online")
    );
    assert!(
        lua.eval_callback::<bool>(&format!("return btech.unit.nss({},1)", id.0))
            .unwrap()
    );
    assert_eq!(native.world().btech, lua.world().btech);
}

/// The real heartbeat completes a persisted switch and saves its electronic consequences together.
#[tokio::test]
async fn nss_server_switch_persists_state() {
    use stompymux_rs::*;
    tokio::task::LocalSet::new()
        .run_until(async {
            let (_dir, config, mut world, id, _) = shot_fixture().await;
            install_test_nss(&mut world, id);
            toggle_battle_null_signature(&mut world, id, ObjectId(1)).unwrap();
            for _ in 0..29 {
                advance_battle_null_signature(&mut world);
            }
            persistence::save(&config.database(), &world).await.unwrap();
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let (shutdown, request) = tokio::sync::oneshot::channel();
            let server_config = config.clone();
            let (driver, trigger) = HeartbeatDriver::manual();
            let mut heartbeats = support::Heartbeats::new(trigger, scripts.progress(), &config);
            let task = tokio::task::spawn_local(async move {
                run_with_schedule_clock(
                    server_config,
                    scripts,
                    listener,
                    async { request.await.unwrap() },
                    || 1,
                    driver,
                )
                .await
            });
            heartbeats.ready().await;
            let saved = heartbeats
                .until_saved(&config, 10, |saved| {
                    saved.btech.constructed_units()[&id]
                        .null_signature()
                        .enabled
                })
                .await;
            let unit = &saved.btech.constructed_units()[&id];
            assert!(unit.null_signature().pending.is_none());
            assert!(!unit.electronics().field.disturbed);
            shutdown.send(ShutdownRequest::Sigterm).unwrap();
            task.await.unwrap().unwrap();
        })
        .await;
}

/// Recent firing is set by a committed shot, survives a restart, and clears at the heartbeat.
#[tokio::test]
async fn recent_fire_is_visible_durable_and_heartbeat_scoped() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    shot_skill(&mut world, 30);
    shot_seed(&mut world, id, 7);
    assert!(!world.btech.constructed_units()[&id].fired_recently());
    let before = world.btech.clone();
    let preview =
        battle_pilot_aim_modifiers(&world, id, target, 0, true, shot_rules().aim).unwrap();
    assert_eq!(world.btech, before);
    assert_eq!(
        preview.perception,
        Some(PerceptionAim {
            channel: Some(DetectionChannel::Sight),
            direct_fire: true,
            modifier: 0,
        })
    );
    let report = resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, shot_rules()).unwrap();
    assert_eq!(report.aim, preview);
    // Perception consumes no dice, so the attack roll is the stream's first.
    assert_eq!(report.roll, Dice::seeded([7; 32]).two_d6());
    assert!(world.btech.constructed_units()[&id].fired_recently());
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    let before = world.btech.clone();
    assert!(resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, shot_rules()).is_err());
    assert_eq!(world.btech, before);
    clear_battle_recent_fire(&mut world);
    assert!(!world.btech.constructed_units()[&id].fired_recently());
}

/// Add TAG to a spare torso slot and put its target on the opposing team.
fn install_test_tag(world: &mut stompymux_rs::World, id: ObjectId, target: ObjectId) {
    use stompymux_rs::*;
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    definition
        .sections
        .get_mut(&MechSection::LeftTorso)
        .unwrap()
        .criticals
        .insert(
            3,
            CriticalDefinition {
                equipment: "TAG".into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    Mech::from_template(definition.clone()).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][id.0.to_string()]["definition"] =
        serde_json::to_value(definition).unwrap();
    state["constructed"][target.0.to_string()]["signature"]["team"] = 2.into();
    world.btech = serde_json::from_value(state).unwrap();
    refresh_battle_contacts(world, &[id]).unwrap();
}

/// TAG lock/recycle timing and unique ownership survive restart; the attack draws the unit's dice.
#[tokio::test]
async fn tag_selection_timers_and_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    let before = world.btech.clone();
    assert!(select_battle_tag(&mut world, id, ObjectId(1), Some(target), tag_rules()).is_err());
    assert_eq!(world.btech, before);
    install_test_tag(&mut world, id, target);
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["contacts"] = serde_json::json!({});
        })
        .unwrap();
    let before = world.btech.clone();
    assert!(select_battle_tag(&mut world, id, ObjectId(1), Some(target), tag_rules()).is_err());
    assert_eq!(world.btech, before);
    refresh_battle_contacts(&mut world, &[id]).unwrap();

    tag_seed(&mut world, id);
    let dice = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"].clone();
    select_battle_tag(&mut world, id, ObjectId(1), Some(target), tag_rules()).unwrap();
    assert_eq!(battle_tagged_by(&world, target), Some(id));
    let selected = world.btech.clone();
    assert!(select_battle_tag(&mut world, id, ObjectId(1), None, tag_rules()).is_err());
    assert_eq!(world.btech, selected);
    for _ in 0..29 {
        assert!(advance_battle_tags(&mut world).is_empty());
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    let notices = advance_battle_tags(&mut loaded);
    assert_eq!(notices.len(), 1);
    assert!(notices[0].text.contains("stable lock"));
    assert!(advance_battle_tags(&mut loaded).is_empty());
    select_battle_tag(&mut loaded, id, ObjectId(1), None, tag_rules()).unwrap();
    assert_eq!(battle_tagged_by(&loaded, target), None);
    for _ in 0..29 {
        assert!(advance_battle_tags(&mut loaded).is_empty());
    }
    assert!(
        advance_battle_tags(&mut loaded)[0]
            .text
            .contains("finished recycling")
    );
    assert_eq!(
        loaded.btech.constructed_units()[&id].tag(),
        TagState::default()
    );
    // The designation was a weapon attack and drew its 2d6 from the unit's dice.
    assert_ne!(
        serde_json::to_value(&loaded.btech.constructed_units()[&id]).unwrap()["dice"],
        dice
    );
}

/// A failed TAG roll designates nothing and recycles the TAG as a fired weapon would (TW p. 142).
#[tokio::test]
async fn tag_miss_designates_nothing_and_recycles() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    install_test_tag(&mut world, id, target);
    // Gunnery 4 against a running, unlocked, adjacent target needs a 4; the dice roll a 2.
    shot_skill(&mut world, 7);
    shot_seed(&mut world, id, support::roll_two_seed());
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["power"] = serde_json::json!({"state":"running"});
        })
        .unwrap();
    let aim = battle_tag_aim_modifiers(&world, id, target, 4, optical_aim_rules()).unwrap();
    assert_eq!(aim.subtotal(), Some(4), "{aim:?}");
    let notices =
        select_battle_tag(&mut world, id, ObjectId(1), Some(target), tag_rules()).unwrap();
    assert_eq!(notices.len(), 1);
    assert!(
        notices[0].text.contains("fails to designate"),
        "{}",
        notices[0].text
    );
    assert_eq!(battle_tagged_by(&world, target), None);
    assert_eq!(
        world.btech.constructed_units()[&id].tag(),
        TagState {
            target: None,
            remaining: 30
        }
    );
    let before = world.btech.clone();
    assert!(select_battle_tag(&mut world, id, ObjectId(1), Some(target), tag_rules()).is_err());
    assert_eq!(world.btech, before);
    for _ in 0..29 {
        assert!(advance_battle_tags(&mut world).is_empty());
    }
    assert!(
        advance_battle_tags(&mut world)[0]
            .text
            .contains("finished recycling")
    );
    world.validate(&config).unwrap();
}

/// TAG aims like a weapon: its own 5/9/15 brackets, the attacker's and target's movement,
/// the secondary-target penalty without a lock, and a Haywire pod on the designator.
#[tokio::test]
async fn tag_aim_uses_weapon_attack_modifiers() {
    use stompymux_rs::*;
    let (_dir, _config, mut world, id, target) = long_field_fixture().await;
    install_test_tag(&mut world, id, target);
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["free_fall"] = serde_json::Value::Null;
        })
        .unwrap();
    // Administrative placement needs the unit shut down; it runs again afterwards so it is
    // not an immobile target.
    let relocate = |world: &mut World, x: i64| {
        world
            .btech
            .rewrite_unit_record(target, |record| {
                record["power"] = serde_json::to_value(Power::Off).unwrap();
            })
            .unwrap();
        place_battle_unit(world, target, map, x, 5).unwrap();
        world
            .btech
            .rewrite_unit_record(target, |record| {
                record["power"] = serde_json::json!({"state":"running"});
            })
            .unwrap();
        refresh_battle_contacts(world, &[id]).unwrap();
    };
    for (x, bracket, modifier) in [
        (10, RangeBracket::Short, 0),
        (12, RangeBracket::Medium, 2),
        (18, RangeBracket::Long, 4),
    ] {
        relocate(&mut world, x);
        let aim = battle_tag_aim_modifiers(&world, id, target, 4, optical_aim_rules()).unwrap();
        let range = aim.range.unwrap();
        assert_eq!((range.bracket, range.modifier), (bracket, modifier), "{x}");
        assert_eq!(aim.gunnery, 4);
        // With no lock at all there is no secondary-target penalty.
        assert_eq!(aim.target_lock, 0);
        assert_eq!(aim.beacon_accuracy, 0);
        assert_eq!(aim.ammunition_accuracy, 0);
        assert_eq!(aim.targeting_computer, 0);
    }
    relocate(&mut world, 27);
    assert!(
        battle_tag_aim_modifiers(&world, id, target, 4, optical_aim_rules())
            .unwrap()
            .range
            .is_none()
    );
    relocate(&mut world, 6);
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["target_lock"] = serde_json::json!({"target":target.0});
            record["beacons"] = serde_json::json!({"CenterTorso":["haywire"]});
            record["motion"]["speed"] = 43.0.into();
            record["motion"]["desired_speed"] = 43.0.into();
        })
        .unwrap();
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["motion"]["speed"] = 43.0.into();
            record["motion"]["desired_speed"] = 43.0.into();
        })
        .unwrap();
    let aim = battle_tag_aim_modifiers(&world, id, target, 4, optical_aim_rules()).unwrap();
    assert_eq!(aim.target_lock, 0);
    assert_eq!(aim.beacon_accuracy, 1);
    assert_eq!(aim.attacker_movement, 1);
    assert_eq!(aim.target_movement, 1);
}

/// Geometry, component loss and shutdown invalidate TAG immediately for consumers, then start recycle on the tick.
#[tokio::test]
async fn tag_loss_geometry_damage_shutdown_and_validation() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    install_test_tag(&mut base, id, target);
    tag_seed(&mut base, id);
    for reason in ["wall", "damage", "shutdown", "missing", "unseen"] {
        let mut world = base.clone();
        select_battle_tag(&mut world, id, ObjectId(1), Some(target), tag_rules()).unwrap();
        match reason {
            "unseen" => {
                // An adjacent target is always in sight; only invisibility takes it away.
                set_battle_visibility(
                    &mut world,
                    target,
                    Visibility {
                        invisible: true,
                        clairvoyant: false,
                    },
                )
                .unwrap();
            }
            "damage" => {
                destroy_battle_critical(
                    &mut world,
                    id,
                    CriticalLocation {
                        section: MechSection::LeftTorso,
                        slot: 3,
                    },
                )
                .unwrap();
            }
            "shutdown" => {
                let notices = stop_battle_unit(&mut world, id, ObjectId(1), fall_rules()).unwrap();
                assert!(
                    notices
                        .iter()
                        .any(|notice| notice.text.contains("TAG connection"))
                );
                assert_eq!(world.btech.constructed_units()[&id].tag().remaining, 30);
            }
            "missing" => {
                world
                    .objects
                    .get_mut(&target)
                    .unwrap()
                    .flags
                    .insert(Flag::Going);
            }
            _ => {
                let map = world.btech.constructed_units()[&id].position().unwrap().map;
                place_battle_unit(&mut world, target, map, 5, 2).unwrap();
                world
                    .btech
                    .rewrite_map_record(map, |record| {
                        record["terrain"][3 * 12 + 5] = serde_json::to_value(
                            stompymux_rs::Hex::new(stompymux_rs::Terrain::Wall, 5),
                        )
                        .unwrap();
                    })
                    .unwrap();
            }
        }
        assert_eq!(battle_tagged_by(&world, target), None, "{reason}");
        assert_eq!(
            advance_battle_tags(&mut world).len(),
            usize::from(reason != "shutdown"),
            "{reason}"
        );
        assert_eq!(
            world.btech.constructed_units()[&id].tag(),
            TagState {
                target: None,
                remaining: if reason == "shutdown" { 29 } else { 30 }
            }
        );
        world.validate(&config).unwrap();
        if reason == "damage" {
            for _ in 0..30 {
                assert!(advance_battle_tags(&mut world).is_empty());
            }
            assert_eq!(world.btech.constructed_units()[&id].tag().remaining, 0);
        }
    }
    base.btech
        .rewrite_unit_record(id, |record| {
            record["tag"] = serde_json::json!({"target":target.0,"remaining":31});
        })
        .unwrap();
    assert!(base.validate(&config).is_err());
}

/// Native TAG and Lua use the same transaction, restoring links and staged notices after callback failure.
#[tokio::test]
async fn tag_native_lua_rollback_and_inspection() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    install_test_tag(&mut world, id, target);
    tag_seed(&mut world, id);
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
    let text = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("tag #{}", target.0),
    );
    assert_eq!(
        native.world().btech.constructed_units()[&id].tag().target,
        Some(target),
        "{text}"
    );
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.tag({},1,{}); error('abort')",
            id.0, target.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    lua.eval_callback::<bool>(&format!("return btech.unit.tag({},1,{})", id.0, target.0))
        .unwrap();
    assert_eq!(lua.world().btech, native.world().btech);
}

/// An observer and an unpowered falling target on a long battlefield.
async fn long_field_fixture() -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
    ObjectId,
) {
    use stompymux_rs::*;
    let mut template =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    template
        .attributes
        .insert("specials".into(), "AntiAircraft".into());
    let source = format!("200 12\n{}", format!("{}\n", ".0".repeat(200)).repeat(12));
    let (dir, config, mut world, id) = fixture_assets(&source, template).await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let target = world.create(&config, "Falling target".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        target,
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, target, map, 5, 4).unwrap();
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["free_fall"] = serde_json::to_value(FreeFall::new(11)).unwrap();
        })
        .unwrap();
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    world.validate(&config).unwrap();
    (dir, config, world, id, target)
}

/// A new tagger takes ownership atomically; rejected range/self/team requests leave links unchanged.
#[tokio::test]
async fn tag_takeover_and_rejected_targets() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = long_field_fixture().await;
    install_test_tag(&mut world, id, target);
    tag_seed(&mut world, id);
    select_battle_tag(&mut world, id, ObjectId(1), Some(target), tag_rules()).unwrap();
    let definition = world.btech.constructed_units()[&id].definition().clone();
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let other = world.create(&config, "Other tagger".into(), Kind::Thing);
    create_battle_unit(&mut world, other, definition).unwrap();
    support::seed_object_dice(&mut world, other, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, other, map, 5, 6).unwrap();
    release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(other);
    assign_battle_pilot(&mut world, other, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, other, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    refresh_battle_contacts(&mut world, &[other]).unwrap();
    tag_seed(&mut world, other);
    for rejected in [other, id, ObjectId(999999)] {
        let before = world.btech.clone();
        assert!(
            select_battle_tag(&mut world, other, ObjectId(1), Some(rejected), tag_rules()).is_err()
        );
        assert_eq!(world.btech, before);
    }
    assert_eq!(
        select_battle_tag(&mut world, other, ObjectId(1), Some(target), tag_rules())
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        world.btech.constructed_units()[&id].tag(),
        TagState {
            target: None,
            remaining: 30
        }
    );
    assert_eq!(battle_tagged_by(&world, target), Some(other));
    let mut invalid = world.clone();
    invalid
        .btech
        .rewrite_unit_record(id, |record| {
            record["tag"] = serde_json::json!({"target":target.0,"remaining":0});
        })
        .unwrap();
    assert!(invalid.validate(&config).is_err());
    for _ in 0..30 {
        advance_battle_tags(&mut world);
    }
    place_battle_unit(&mut world, target, map, 25, 5).unwrap();
    assert!(battle_unit_range(&world, other, target).unwrap().spatial > 15.0);
    let before = world.btech.clone();
    assert!(select_battle_tag(&mut world, other, ObjectId(1), Some(target), tag_rules()).is_err());
    assert_eq!(world.btech, before);
    assert_eq!(battle_tagged_by(&world, target), None);
    assert_eq!(advance_battle_tags(&mut world).len(), 1);
}

/// A separate friendly TAG unit illuminates a moving enemy for a piloted LRM launcher with both supply types.
async fn semiguided_fixture() -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
    ObjectId,
    ObjectId,
    usize,
) {
    use stompymux_rs::*;
    let (dir, config, mut world, tagger, target) = shot_fixture().await;
    install_test_tag(&mut world, tagger, target);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][target.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    state["constructed"][target.0.to_string()]["motion"]["speed"] = 43.0.into();
    state["constructed"][target.0.to_string()]["motion"]["desired_speed"] = 43.0.into();
    world.btech = serde_json::from_value(state).unwrap();
    tag_seed(&mut world, tagger);
    select_battle_tag(&mut world, tagger, ObjectId(1), Some(target), tag_rules()).unwrap();
    let mut definition =
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap();
    definition
        .sections
        .get_mut(&MechSection::CenterTorso)
        .unwrap()
        .criticals
        .get_mut(&10)
        .unwrap()
        .equipment = "IS.LRM-5".into();
    let torso = definition
        .sections
        .get_mut(&MechSection::RightTorso)
        .unwrap();
    let bin = torso.criticals.get_mut(&0).unwrap();
    bin.equipment = "Ammo_IS.LRM-5".into();
    bin.data = "24".into();
    bin.modes = vec!["Sguided".into()];
    let mut ordinary = bin.clone();
    ordinary.modes.clear();
    torso.criticals.insert(3, ordinary);
    let shooter = world.create(&config, "Semi-guided shooter".into(), Kind::Thing);
    create_battle_unit(&mut world, shooter, definition).unwrap();
    support::seed_object_dice(&mut world, shooter, support::FIXTURE_DICE_SEED);
    let map = world.btech.constructed_units()[&tagger]
        .position()
        .unwrap()
        .map;
    place_battle_unit(&mut world, shooter, map, 5, 6).unwrap();
    release_battle_pilot(&mut world, tagger, ObjectId(1)).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
    assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, shooter, ObjectId(1), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0, true);
    }
    refresh_battle_contacts(&mut world, &[shooter]).unwrap();
    let index = world.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::Lrm5)
        .unwrap();
    world.validate(&config).unwrap();
    (dir, config, world, shooter, target, tagger, index)
}

/// TAG assistance is live, applies during settling, and comes from any friendly TAG, the
/// shooter's own included (TW p. 142); an immobile target keeps its -4.
#[tokio::test]
async fn semiguided_tag_aim_and_link_loss() {
    use stompymux_rs::*;
    let (_dir, config, mut world, shooter, target, tagger, index) = semiguided_fixture().await;
    let normal =
        battle_aim_modifiers(&world, shooter, target, index, 4, optical_aim_rules()).unwrap();
    assert!(normal.target_movement > 0);
    toggle_battle_semiguided(&mut world, shooter, ObjectId(1), index).unwrap();
    let selected = world.btech.clone();
    let aided =
        battle_aim_modifiers(&world, shooter, target, index, 4, optical_aim_rules()).unwrap();
    assert_eq!(aided.target_movement, 0);
    assert_eq!(
        normal.subtotal().unwrap() - aided.subtotal().unwrap(),
        i32::from(normal.target_movement)
    );
    assert_eq!(world.btech, selected);
    assert_eq!(world.btech.constructed_units()[&tagger].tag().remaining, 30);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(
        battle_aim_modifiers(&restored, shooter, target, index, 4, optical_aim_rules())
            .unwrap()
            .target_movement,
        0
    );
    for case in ["enemy", "lost", "self", "negative"] {
        let mut trial = world.clone();
        match case {
            "lost" => {
                destroy_battle_critical(
                    &mut trial,
                    tagger,
                    CriticalLocation {
                        section: MechSection::LeftTorso,
                        slot: 3,
                    },
                )
                .unwrap();
            }
            _ => {
                let mut state = serde_json::to_value(&trial.btech).unwrap();
                if case == "enemy" {
                    state["constructed"][tagger.0.to_string()]["signature"]["team"] = 3.into();
                } else if case == "negative" {
                    state["constructed"][target.0.to_string()]["power"] =
                        serde_json::json!({"state":"off"});
                    state["constructed"][target.0.to_string()]["motion"]["speed"] = 0.0.into();
                    state["constructed"][target.0.to_string()]["motion"]["desired_speed"] =
                        0.0.into();
                } else {
                    state["constructed"][tagger.0.to_string()]["tag"] =
                        serde_json::json!({"target":null,"remaining":0});
                }
                trial.btech = serde_json::from_value(state).unwrap();
                if case == "self" {
                    install_test_tag(&mut trial, shooter, target);
                    tag_seed(&mut trial, shooter);
                    select_battle_tag(&mut trial, shooter, ObjectId(1), Some(target), tag_rules())
                        .unwrap();
                }
            }
        }
        let movement = battle_aim_modifiers(&trial, shooter, target, index, 4, optical_aim_rules())
            .unwrap()
            .target_movement;
        match case {
            "negative" => assert_eq!(movement, -4),
            "self" => assert_eq!(movement, 0),
            _ => assert_eq!(movement, normal.target_movement, "{case}"),
        }
    }
}

/// Native/Lua controls and firing use matching supplies and restore all effects when the callback aborts.
#[tokio::test]
async fn semiguided_native_lua_fire_and_matching_supply() {
    use stompymux_rs::*;
    let (_dir, config, mut world, shooter, target, _tagger, index) = semiguided_fixture().await;
    shot_seed(&mut world, shooter, 7);
    shot_seed(&mut world, target, 7);
    let laser = world.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::MediumLaser)
        .unwrap();
    let before = world.btech.clone();
    assert!(toggle_battle_semiguided(&mut world, shooter, ObjectId(1), laser).is_err());
    assert_eq!(world.btech, before);
    let mut one_shot = world.clone();
    one_shot
        .btech
        .rewrite_unit_record(shooter, |record| {
            record["definition"]["sections"]["CenterTorso"]["criticals"]["10"]["modes"] =
                serde_json::json!(["OneShot"]);
        })
        .unwrap();
    let before = one_shot.btech.clone();
    assert!(toggle_battle_semiguided(&mut one_shot, shooter, ObjectId(1), index).is_err());
    assert_eq!(one_shot.btech, before);
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
    let text = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("sguided {index}"),
    );
    assert_eq!(
        native.world().btech.constructed_units()[&shooter]
            .ammunition_mode(index)
            .unwrap(),
        AmmunitionMode::SemiGuided,
        "{text}"
    );
    lua.eval_callback::<mlua::Value>(&format!(
        "return btech.unit.sguided({},1,{index})",
        shooter.0
    ))
    .unwrap();
    assert_eq!(native.world().btech, lua.world().btech);
    let selected = lua.world().clone();
    lua.drain_outbox();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.fire({},1,{index},{}); error('abort')",
            shooter.0, target.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, selected.btech);
    assert!(lua.drain_outbox().is_empty());
    lua.eval_callback::<mlua::Table>(&format!(
        "return btech.unit.fire({},1,{index},{})",
        shooter.0, target.0
    ))
    .unwrap();
    let mut direct = selected.clone();
    let rules = shot_rules();
    let shot =
        resolve_battle_shot(&mut direct, shooter, ObjectId(1), target, index, rules).unwrap();
    assert_eq!(shot.expenditure.ammunition_mode, AmmunitionMode::SemiGuided);
    assert_eq!(
        direct.btech.constructed_units()[&shooter].ammunition(),
        &[23, 24]
    );
    assert_eq!(lua.world().btech, direct.btech);
    let mut empty = selected;
    empty.btech.set_unit_ammunition_bin(shooter, 0, 0).unwrap();
    let before = empty.btech.clone();
    assert!(resolve_battle_shot(&mut empty, shooter, ObjectId(1), target, index, rules).is_err());
    assert_eq!(empty.btech, before);
}

/// Establish a self-declared observer with a selected enemy, then hand its link to the LRM pilot.
async fn spotter_fixture() -> (
    tempfile::TempDir,
    stompymux_rs::Config,
    stompymux_rs::World,
    ObjectId,
    ObjectId,
    ObjectId,
    usize,
) {
    use stompymux_rs::*;
    let (dir, config, mut world, shooter, target, observer, index) = semiguided_fixture().await;
    release_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(observer);
    assign_battle_pilot(&mut world, observer, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    select_battle_spotter(&mut world, observer, ObjectId(1), Some(observer)).unwrap();
    select_battle_target(&mut world, observer, ObjectId(1), Some(target)).unwrap();
    release_battle_pilot(&mut world, observer, ObjectId(1)).unwrap();
    world.objects.get_mut(&ObjectId(1)).unwrap().location = Some(shooter);
    assign_battle_pilot(&mut world, shooter, ObjectId(1)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    // The shooter moves behind a wall: an attacker that can see its target may not fire
    // indirectly (TW p. 111), so the observer's sight must be the only one.
    let map = world.btech.constructed_units()[&shooter]
        .position()
        .unwrap()
        .map;
    world
        .btech
        .rewrite_unit_record(shooter, |record| {
            record["power"] = serde_json::to_value(Power::Off).unwrap();
        })
        .unwrap();
    place_battle_unit(&mut world, shooter, map, 5, 8).unwrap();
    world
        .btech
        .rewrite_unit_record(shooter, |record| {
            record["power"] = serde_json::to_value(Power::Running).unwrap();
        })
        .unwrap();
    world
        .btech
        .rewrite_map_record(map, |record| {
            record["terrain"][SPOTTER_WALL] =
                serde_json::to_value(Hex::new(Terrain::Wall, 8)).unwrap();
        })
        .unwrap();
    refresh_battle_contacts(&mut world, &[shooter, observer]).unwrap();
    select_battle_spotter(&mut world, shooter, ObjectId(1), Some(observer)).unwrap();
    assert!(
        battle_aim_modifiers(&world, shooter, target, index, 6, optical_aim_rules())
            .unwrap()
            .indirect
            .is_some()
    );
    (dir, config, world, shooter, target, observer, index)
}

/// Terrain index of the wall hex (5,7) between the spotter fixture's shooter and target.
const SPOTTER_WALL: usize = 7 * 12 + 5;

/// Clear the spotter fixture's wall so the shooter can see its target again.
fn unblock_spotter_wall(world: &mut stompymux_rs::World, shooter: ObjectId) {
    use stompymux_rs::*;
    let map = world.btech.constructed_units()[&shooter]
        .position()
        .unwrap()
        .map;
    world
        .btech
        .rewrite_map_record(map, |record| {
            record["terrain"][SPOTTER_WALL] =
                serde_json::to_value(Hex::new(Terrain::Clear, 0)).unwrap();
        })
        .unwrap();
    refresh_battle_contacts(world, &[shooter]).unwrap();
}

/// A spotter may fire (TW p. 111): recycling weapons do not stop the declaration and the
/// declared spotter keeps shooting. A Haywire pod (TW p. 141) and a charge or death from
/// above this turn (TW p. 111) are what bar a unit from spotting.
#[tokio::test]
async fn spotter_self_declaration_and_fire_guards() {
    use stompymux_rs::*;
    let (_dir, config, world, id, target) = shot_fixture().await;
    for field in ["weapon_recycle", "limb_recycle"] {
        let mut trial = world.clone();
        trial
            .btech
            .rewrite_unit_record(id, |record| {
                record[field] = if field == "weapon_recycle" {
                    serde_json::json!({"0":1})
                } else {
                    serde_json::json!({"LeftArm":1})
                };
            })
            .unwrap();
        select_battle_spotter(&mut trial, id, ObjectId(1), Some(id)).unwrap();
        assert_eq!(trial.btech.constructed_units()[&id].spotter(), Some(id));
    }
    for prohibition in ["haywire", "charging", "dfa_landed", "charged"] {
        let mut trial = world.clone();
        trial
            .btech
            .rewrite_unit_record(id, |record| match prohibition {
                "haywire" => record["beacons"] = serde_json::json!({"CenterTorso":["haywire"]}),
                "charging" => record["charge"]["target"] = serde_json::json!(target.0),
                _ => record["physical_assault_time"] = serde_json::json!(0),
            })
            .unwrap();
        let before = trial.btech.clone();
        let error = select_battle_spotter(&mut trial, id, ObjectId(1), Some(id)).unwrap_err();
        assert!(
            error.to_string().contains("cannot spot"),
            "{prohibition}: {error}"
        );
        assert_eq!(trial.btech, before);
    }
    let mut world = world;
    select_battle_spotter(&mut world, id, ObjectId(1), Some(id)).unwrap();
    let before = world.btech.clone();
    assert!(resolve_battle_shot(&mut world, id, ObjectId(1), target, 0, shot_rules()).is_ok());
    assert_ne!(world.btech, before);
    assert_eq!(
        world.btech.constructed_units()[&id].last_weapon_fire(),
        Some(world.btech.simulation_time())
    );
    assert!(
        select_battle_spotter(&mut world, id, ObjectId(1), None).unwrap()[0]
            .text
            .contains("spot no longer")
    );
    // Weapon 0 is recycling from the shot made while spotting; the next mount fires.
    assert!(resolve_battle_shot(&mut world, id, ObjectId(1), target, 1, shot_rules()).is_ok());
    world.validate(&config).unwrap();
}

/// Selected observers and their targets survive restart; consumers reject current contact/role/team loss.
#[tokio::test]
async fn spotter_target_persistence_and_live_revalidation() {
    use stompymux_rs::*;
    let (_dir, config, mut world, shooter, target, observer, index) = spotter_fixture().await;
    let before = world.btech.clone();
    assert_eq!(
        battle_spotter_target(&world, shooter).unwrap(),
        SpotterTarget {
            spotter: observer,
            target
        }
    );
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(
        battle_spotter_target(&loaded, shooter).unwrap().target,
        target
    );
    let laser = world.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::MediumLaser)
        .unwrap();
    assert!(
        resolve_battle_shot(
            &mut world,
            shooter,
            ObjectId(1),
            target,
            laser,
            shot_rules()
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
    assert!(
        resolve_battle_shot(
            &mut world.clone(),
            shooter,
            ObjectId(1),
            target,
            index,
            shot_rules()
        )
        .is_ok()
    );
    for failure in ["role", "team", "power", "contact", "map"] {
        let mut trial = world.clone();
        let mut state = serde_json::to_value(&trial.btech).unwrap();
        match failure {
            "role" => {
                state["constructed"][observer.0.to_string()]["spotter"] = serde_json::Value::Null
            }
            "team" => state["constructed"][observer.0.to_string()]["signature"]["team"] = 9.into(),
            "power" => {
                state["constructed"][observer.0.to_string()]["power"] =
                    serde_json::json!({"state":"off"})
            }
            "contact" => {
                // A target any teammate holds is spotted for the whole side (Tactical
                // Operations p. 191), so the side as a whole must have lost it.
                for unit in [observer, shooter] {
                    state["constructed"][unit.0.to_string()]["contacts"] = serde_json::json!({});
                }
            }
            _ => state["constructed"][observer.0.to_string()]["position"]["map"] = 999.into(),
        }
        trial.btech = serde_json::from_value(state).unwrap();
        assert!(battle_spotter_target(&trial, shooter).is_err(), "{failure}");
    }
    // Once established, losing only the firer's contact with the observer does not lose the direct link.
    world
        .btech
        .rewrite_unit_record(shooter, |record| {
            record["contacts"] = serde_json::json!({});
        })
        .unwrap();
    assert_eq!(
        battle_spotter_target(&world, shooter).unwrap().target,
        target
    );
}

/// Native and Lua spotter selection use the same state/effect checkpoint, including rejected connections.
#[tokio::test]
async fn spotter_native_lua_selection_and_rollback() {
    use stompymux_rs::*;
    let (_dir, config, mut world, shooter, target, observer, _index) = spotter_fixture().await;
    select_battle_spotter(&mut world, shooter, ObjectId(1), None).unwrap();
    let before = world.btech.clone();
    assert!(select_battle_spotter(&mut world, shooter, ObjectId(1), Some(target)).is_err());
    assert_eq!(world.btech, before);
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
    let text = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("spot #{}", observer.0),
    );
    assert_eq!(
        native.world().btech.constructed_units()[&shooter].spotter(),
        Some(observer),
        "{text}"
    );
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.spot({},1,{}); error('abort')",
            shooter.0, observer.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    lua.eval_callback::<bool>(&format!(
        "return btech.unit.spot({},1,{})",
        shooter.0, observer.0
    ))
    .unwrap();
    assert_eq!(lua.world().btech, native.world().btech);
}

/// An acquired observer target permits fire without shooter contact or a bearing weapon arc.
#[tokio::test]
async fn indirect_spotter_aim_routing_and_rollback() {
    use stompymux_rs::*;
    let (_dir, config, mut world, shooter, target, observer, index) = spotter_fixture().await;
    world
        .btech
        .rewrite_unit_record(shooter, |record| {
            record["target_lock"] = serde_json::Value::Null;
            record["contacts"] = serde_json::json!({});
            record["motion"]["heading"] = 180.into();
            record["motion"]["desired_heading"] = 180.into();
        })
        .unwrap();
    shot_seed(&mut world, shooter, 7);
    let before = world.btech.clone();
    let aim = battle_aim_modifiers(&world, shooter, target, index, 6, shot_rules().aim).unwrap();
    assert_eq!(world.btech, before);
    let indirect = aim.indirect.unwrap();
    assert_eq!(
        indirect,
        IndirectAim {
            spotter: observer,
            movement: 0,
            spotter_attacked: false,
            tag_guided: false,
        }
    );
    assert_eq!(indirect.modifier(), 1);
    assert_eq!(aim.target_lock, 0);
    assert_eq!(aim.spotting, 0);
    assert!(aim.perception.is_some());
    assert!(aim.subtotal().is_some());
    assert_eq!(
        aim.distance,
        battle_unit_range(&world, shooter, target).unwrap().spatial
    );
    // A spotter that fired within the turn adds +1 to the shot it spots (TW p. 111), and its
    // own weapons take +1 while a unit fires through it.
    let mut attacked = world.clone();
    let now = attacked.btech.simulation_time();
    attacked
        .btech
        .rewrite_unit_record(observer, |record| {
            record["last_weapon_fire"] = serde_json::json!(now);
            record["motion"]["speed"] = 43.0.into();
            record["motion"]["desired_speed"] = 43.0.into();
        })
        .unwrap();
    let attacked_aim =
        battle_aim_modifiers(&attacked, shooter, target, index, 6, shot_rules().aim).unwrap();
    assert_eq!(
        attacked_aim.indirect.unwrap(),
        IndirectAim {
            spotter: observer,
            movement: 1,
            spotter_attacked: true,
            tag_guided: false,
        }
    );
    assert_eq!(
        attacked_aim.subtotal().unwrap() - aim.subtotal().unwrap(),
        2
    );
    let own = battle_aim_modifiers(&attacked, observer, target, 0, 6, shot_rules().aim).unwrap();
    assert_eq!(own.spotting, 1);
    assert_eq!(own.attacker_movement, 1);
    // A firer that can see the target itself may not fire indirectly (TW p. 111): the
    // observer still supplies the target, but the shot is direct; without any lock there
    // is no secondary-target penalty.
    let mut seen = world.clone();
    unblock_spotter_wall(&mut seen, shooter);
    // Direct fire needs the target in the weapon's arc, so the shooter faces it again.
    seen.btech
        .rewrite_unit_record(shooter, |record| {
            record["motion"]["heading"] = 0.into();
            record["motion"]["desired_heading"] = 0.into();
        })
        .unwrap();
    assert!(
        visible_battle_contact(&seen, shooter, target)
            .unwrap()
            .is_some()
    );
    let seen_aim =
        battle_aim_modifiers(&seen, shooter, target, index, 6, shot_rules().aim).unwrap();
    assert!(seen_aim.indirect.is_none());
    assert_eq!(seen_aim.target_lock, 0);
    assert_eq!(
        seen_aim.perception,
        Some(PerceptionAim {
            channel: Some(DetectionChannel::Sight),
            direct_fire: true,
            modifier: 0,
        })
    );
    let seen_report = resolve_battle_shot(
        &mut seen.clone(),
        shooter,
        ObjectId(1),
        observer,
        index,
        shot_rules(),
    )
    .unwrap();
    assert_eq!(seen_report.target, target);
    assert!(seen_report.aim.indirect.is_none());
    // Selecting a personal target drops the observer's routing: the shot becomes direct, on the
    // shooter's own sight of a target its side has spotted (Tactical Operations p. 191).
    let mut direct = world.clone();
    unblock_spotter_wall(&mut direct, shooter);
    direct
        .btech
        .rewrite_unit_record(shooter, |record| {
            record["target_lock"] = serde_json::json!({"target":target.0});
            record["contacts"] = serde_json::json!({});
        })
        .unwrap();
    let direct_aim =
        battle_aim_modifiers(&direct, shooter, target, index, 6, shot_rules().aim).unwrap();
    assert!(direct_aim.indirect.is_none());
    assert_eq!(
        direct_aim.perception,
        Some(PerceptionAim {
            channel: Some(DetectionChannel::Sight),
            direct_fire: true,
            modifier: 0,
        })
    );
    // Without anyone on the side holding the target, the direct shot has no contact at all.
    direct
        .btech
        .rewrite_unit_record(observer, |record| {
            record["contacts"] = serde_json::json!({});
        })
        .unwrap();
    assert!(
        battle_aim_modifiers(&direct, shooter, target, index, 6, shot_rules().aim)
            .unwrap()
            .perception
            .is_none()
    );
    let mut rejected = world.clone();
    rejected
        .btech
        .rewrite_unit_record(observer, |record| {
            record["contacts"] = serde_json::json!({});
        })
        .unwrap();
    let snapshot = rejected.btech.clone();
    assert!(
        resolve_battle_shot(
            &mut rejected,
            shooter,
            ObjectId(1),
            target,
            index,
            shot_rules()
        )
        .is_err()
    );
    assert_eq!(rejected.btech, snapshot);
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.fire({},1,{index}); error('abort')",
            shooter.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, before);
    assert!(lua.drain_outbox().is_empty());
    let report = lua
        .eval_callback::<(i64, i64)>(&format!(
            "local shot = btech.unit.fire({},1,{index}); return shot.target, shot.aim.indirect.spotter",
            shooter.0
        ))
        .unwrap();
    assert_eq!(report.0, target.0);
    assert_eq!(report.1, observer.0);
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let text = support::run_text(&native, &config, ObjectId(1), 1, &format!("fire {index}"));
    assert_eq!(native.world().btech, lua.world().btech, "{text}");
    native.world().validate(&config).unwrap();
    // The direct resolver also routes through the observer, even when given another explicit target.
    let report = resolve_battle_shot(
        &mut world,
        shooter,
        ObjectId(1),
        observer,
        index,
        shot_rules(),
    )
    .unwrap();
    assert_eq!(report.target, target);
    assert_eq!(report.aim.indirect, aim.indirect);
}

/// Indirect skill awards use pre-damage eligibility, survive restart, and roll back with Lua actions.
#[tokio::test]
async fn indirect_spotter_experience_eligibility_and_rollback() {
    use stompymux_rs::*;
    let (_dir, config, mut world, shooter, target, observer, index) = spotter_fixture().await;
    shot_skill(&mut world, 0);
    let observer_pilot = ObjectId(2);
    world.objects.get_mut(&observer_pilot).unwrap().location = Some(observer);
    world
        .objects
        .get_mut(&observer_pilot)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(&mut world, observer, observer_pilot).unwrap();
    support::seed_object_dice(&mut world, observer_pilot, support::FIXTURE_DICE_SEED);
    set_battle_character(
        &mut world,
        observer_pilot,
        Character {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    support::seed_object_dice(&mut world, observer_pilot, support::FIXTURE_DICE_SEED);
    for id in [shooter, observer, target] {
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
    }
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    shot_seed(&mut world, shooter, seed);
    for exclusion in [
        "none",
        "target",
        "observer",
        "firer",
        "disconnected",
        "limited",
        "level",
    ] {
        let mut trial = world.clone();
        match exclusion {
            "target" => {
                trial
                    .objects
                    .get_mut(&target)
                    .unwrap()
                    .flags
                    .remove(Flag::InCharacter);
            }
            "observer" => {
                trial
                    .objects
                    .get_mut(&observer)
                    .unwrap()
                    .flags
                    .remove(Flag::InCharacter);
            }
            "firer" => {
                trial
                    .objects
                    .get_mut(&shooter)
                    .unwrap()
                    .flags
                    .remove(Flag::InCharacter);
            }
            "disconnected" => {
                trial
                    .objects
                    .get_mut(&observer_pilot)
                    .unwrap()
                    .flags
                    .remove(Flag::Connected);
            }
            "level" => {
                set_battle_character_value(
                    &mut trial,
                    observer_pilot,
                    "Gunnery-Spotting",
                    CharacterValue {
                        experience: 1,
                        ..Default::default()
                    },
                )
                .unwrap();
            }
            "limited" => {
                for (pilot, skill) in [
                    (ObjectId(1), "Gunnery-Artillery"),
                    (observer_pilot, "Gunnery-Spotting"),
                ] {
                    set_battle_character_value(
                        &mut trial,
                        pilot,
                        skill,
                        CharacterValue {
                            last_used: i64::MAX,
                            ..Default::default()
                        },
                    )
                    .unwrap();
                }
            }
            _ => {}
        }
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(trial.clone())),
        )
        .unwrap();
        assert!(
            scripts
                .eval_callback::<()>(&format!(
                    "btech.unit.fire({},1,{index}); error('abort')",
                    shooter.0
                ))
                .is_err()
        );
        assert_eq!(scripts.world().btech, trial.btech);
        assert!(scripts.drain_outbox().is_empty());
        let (count, miss) = scripts.eval_callback::<(usize, bool)>(&format!(
            "local shot = btech.unit.fire({},1,{index}); return #shot.experience_messages, shot.salvo == nil", shooter.0
        )).unwrap();
        assert!(miss);
        let spotting = !matches!(
            exclusion,
            "target" | "observer" | "disconnected" | "limited"
        );
        let artillery = !matches!(exclusion, "target" | "firer");
        assert_eq!(
            count,
            usize::from(spotting) + usize::from(artillery),
            "{exclusion}"
        );
        for (pilot, skill, expected) in [
            (ObjectId(1), "Gunnery-Artillery", artillery),
            (observer_pilot, "Gunnery-Spotting", spotting),
        ] {
            let balance = scripts
                .world()
                .btech
                .character_values()
                .get(&pilot)
                .and_then(|values| values.get(skill))
                .map_or(0, |value| value.experience_balance());
            assert_eq!(
                balance,
                u32::from(expected)
                    + u32::from(exclusion == "level" && skill == "Gunnery-Spotting"),
                "{exclusion}: {skill}"
            );
        }
        scripts.world().validate(&config).unwrap();
        if exclusion == "none" {
            let saved = scripts.world().clone();
            persistence::save(&config.database(), &saved).await.unwrap();
            let loaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(loaded.btech, saved.btech);
        }
    }
}

/// Terrain visibility needs no acquired occupant; only sight observes empty hexes, so a sensor
/// sweep that would find a unit there sees nothing of the hex itself.
#[tokio::test]
async fn indirect_hex_visibility_uses_terrain_and_perception_rules() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, _target) = long_field_fixture().await;
    // Four hexes out: within daylight's sight and the infrared short band, beyond the three
    // hexes pitch black leaves.
    let point = HexCoordinate { x: 5, y: 1 };
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    for (sensors, light, expected) in [
        (true, Light::Day, Some(DetectionChannel::Sight)),
        (false, Light::Day, Some(DetectionChannel::Sight)),
        (true, Light::PitchBlack, None),
        (false, Light::PitchBlack, None),
    ] {
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()]["contacts"] = serde_json::json!({});
        state["constructed"][id.0.to_string()]["sensor_sweep"] = 7.into();
        state["maps"][map.0.to_string()]["light"] = light.stored().into();
        state["maps"][map.0.to_string()]["weather"] = Weather::Clear.stored().into();
        state["maps"][map.0.to_string()]["sensor_flags"] = if sensors {
            0.into()
        } else {
            MapPerceptionFlag::Sensors.bit().into()
        };
        world.btech = serde_json::from_value(state).unwrap();
        let before = world.btech.clone();
        assert_eq!(
            battle_hex_perception(&world, id, point).unwrap(),
            expected,
            "sensors {sensors}, light {light:?}"
        );
        assert_eq!(
            battle_hex_visible(&world, id, point).unwrap(),
            expected.is_some()
        );
        assert_eq!(world.btech, before);
        let mut blocked = world.clone();
        blocked
            .btech
            .rewrite_map_record(map, |record| {
                record["terrain"][3 * 200 + 5] =
                    serde_json::to_value(Hex::new(Terrain::Wall, 8)).unwrap();
            })
            .unwrap();
        assert!(!battle_hex_visible(&blocked, id, point).unwrap());
        world.validate(&config).unwrap();
    }
}

/// Indirect cockpit and observer output describes coordinates and conceals hit/target identity.
#[tokio::test]
async fn indirect_fire_hex_feedback_native_lua_and_rollback() {
    use stompymux_rs::*;
    let (_dir, config, mut world, shooter, target, observer, index) = spotter_fixture().await;
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    shot_seed(&mut world, shooter, seed);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    for unit in [observer, target] {
        state["constructed"][unit.0.to_string()]["contacts"]
            .as_object_mut()
            .unwrap()
            .remove(&shooter.0.to_string());
    }
    world.btech = serde_json::from_value(state).unwrap();
    for recipient in [observer, target] {
        let mut trial = world.clone();
        trial.objects.get_mut(&ObjectId(2)).unwrap().location = Some(recipient);
        trial
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(trial.clone())),
        )
        .unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(trial.clone())),
        )
        .unwrap();
        assert!(
            lua.eval_callback::<()>(&format!(
                "btech.unit.fire({},1,{index}); error('abort')",
                shooter.0
            ))
            .is_err()
        );
        assert_eq!(lua.world().btech, trial.btech);
        assert!(lua.drain_outbox().is_empty());
        commands::run(&native, &config, ObjectId(1), 1, &format!("fire {index}")).unwrap();
        lua.eval_callback::<mlua::Table>(&format!(
            "return btech.unit.fire({},1,{index})",
            shooter.0
        ))
        .unwrap();
        let text = |messages: Vec<(ObjectId, stompymux_rs::text::Document)>| {
            messages
                .into_iter()
                .map(|(id, doc)| (id, doc.source().to_owned()))
                .collect::<Vec<_>>()
        };
        let messages = text(native.drain_outbox());
        assert_eq!(messages, text(lua.drain_outbox()));
        let pilot_text = messages
            .iter()
            .filter(|(id, _)| *id == ObjectId(1))
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            pilot_text.contains("You fire LRM-5 at (5,4) - BTH:"),
            "{pilot_text}"
        );
        assert!(!pilot_text.contains("Miss."));
        let observer_text = messages
            .iter()
            .filter(|(id, _)| *id == ObjectId(2))
            .map(|(_, text)| text.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            observer_text.contains("Something fires a LRM-5 at hex 5 4!"),
            "{observer_text}"
        );
        assert!(!observer_text.contains("misses"));
        if recipient == target {
            assert!(
                observer_text.contains("Something has fired a LRM-5 at you from bearing"),
                "{observer_text}"
            );
        }
        native.world().validate(&config).unwrap();
    }
}

/// Coordinate locks replace unit targets, need no visibility, and survive save/load.
#[tokio::test]
async fn hex_target_selection_lifecycle_and_validation() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let point = HexCoordinate { x: 9, y: 9 };
    // Pitch black leaves three hexes of sight; the coordinate is six away.
    set_battle_map_conditions(&mut world, map, Light::PitchBlack, Weather::Clear).unwrap();
    assert!(!battle_hex_visible(&world, id, point).unwrap());
    for mode in [
        HexTargetMode::UnitAtHex,
        HexTargetMode::Hex,
        HexTargetMode::Building,
        HexTargetMode::Ignite,
        HexTargetMode::Clear,
    ] {
        select_battle_hex_target(&mut world, id, ObjectId(1), point, mode).unwrap();
        assert_eq!(
            world.btech.constructed_units()[&id].hex_lock(),
            Some(HexLock { hex: point, mode })
        );
        assert!(world.btech.constructed_units()[&id].target_lock().is_none());
        world.validate(&config).unwrap();
    }
    let before = world.btech.clone();
    assert!(
        select_battle_hex_target(
            &mut world,
            id,
            ObjectId(1),
            HexCoordinate { x: -1, y: 0 },
            HexTargetMode::Hex
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
    persistence::save(&config.database(), &world).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, world.btech);
    assert_eq!(
        loaded.btech.constructed_units()[&id]
            .hex_lock()
            .unwrap()
            .hex,
        point
    );
    loaded.validate(&config).unwrap();
    let mut ambiguous = serde_json::to_value(&world.btech).unwrap();
    ambiguous["constructed"][id.0.to_string()]["target_lock"]["target"] = target.0.into();
    assert!(serde_json::from_value::<BtechState>(ambiguous).is_err());
    let mut moved = world.clone();
    place_battle_unit(&mut moved, target, map, 7, 7).unwrap();
    assert!(moved.btech.constructed_units()[&id].hex_lock().is_some());
    stop_battle_unit(&mut moved, id, ObjectId(1), fall_rules()).unwrap();
    place_battle_unit(&mut moved, id, map, 6, 6).unwrap();
    assert!(
        moved.btech.constructed_units()[&id]
            .target_selection()
            .is_none()
    );
    stop_battle_unit(&mut world, id, ObjectId(1), fall_rules()).unwrap();
    assert!(
        world.btech.constructed_units()[&id]
            .target_selection()
            .is_none()
    );
}

/// Native/Lua coordinate modes share selection and rollback, and unimplemented hex shots spend nothing.
#[tokio::test]
async fn hex_target_native_lua_controls_and_fire_guard() {
    use stompymux_rs::*;
    let (_dir, config, world, id, target) = shot_fixture().await;
    for (suffix, mode) in [
        ("", "unit_at_hex"),
        (" H", "hex"),
        (" B", "building"),
        (" I", "ignite"),
        (" C", "clear"),
    ] {
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
            lua.eval_callback::<()>(&format!(
                "btech.unit.lock_hex({},1,9,9,'{mode}'); error('abort')",
                id.0
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
            &format!("lock 9 9{suffix}"),
        )
        .unwrap();
        lua.eval_callback::<bool>(&format!(
            "return btech.unit.lock_hex({},1,9,9,'{mode}')",
            id.0
        ))
        .unwrap();
        assert_eq!(lua.world().btech, native.world().btech);
        let text = |messages: Vec<(ObjectId, stompymux_rs::text::Document)>| {
            messages
                .into_iter()
                .map(|(id, doc)| (id, doc.source().to_owned()))
                .collect::<Vec<_>>()
        };
        assert_eq!(text(lua.drain_outbox()), text(native.drain_outbox()));
        let before = native.world().btech.clone();
        let response = support::run_text(&native, &config, ObjectId(1), 1, "fire 0");
        assert!(response.contains("outside weapon arc"), "{response}");
        assert_eq!(native.world().btech, before);
        let mut trial = native.world().clone();
        let explicit =
            resolve_battle_shot(&mut trial, id, ObjectId(1), target, 0, shot_rules()).unwrap();
        assert!(explicit.coordinate.is_none());
        select_battle_target(&mut trial, id, ObjectId(1), Some(target)).unwrap();
        assert!(trial.btech.constructed_units()[&id].hex_lock().is_none());
        assert_eq!(
            trial.btech.constructed_units()[&id]
                .target_lock()
                .unwrap()
                .target,
            target
        );
        support::run_text(&native, &config, ObjectId(1), 1, "lock -");
        assert!(
            native.world().btech.constructed_units()[&id]
                .target_selection()
                .is_none()
        );
        native.world().validate(&config).unwrap();
    }
}

/// A unit-at-hex shot chooses the current occupant, retains its coordinate lock, and shares native/Lua rollback.
#[tokio::test]
async fn hex_occupant_fire_routing_aim_and_rollback() {
    use stompymux_rs::*;
    let (_dir, config, mut world, shooter, target) = shot_fixture().await;
    let point = HexCoordinate { x: 5, y: 4 };
    let map = world.btech.constructed_units()[&shooter]
        .position()
        .unwrap()
        .map;
    select_battle_hex_target(
        &mut world,
        shooter,
        ObjectId(1),
        point,
        HexTargetMode::UnitAtHex,
    )
    .unwrap();
    assert_eq!(
        battle_hex_occupant(&world, shooter, point).unwrap(),
        Some(target)
    );
    let aim = battle_aim_modifiers(&world, shooter, target, 0, 6, shot_rules().aim).unwrap();
    assert_eq!(aim.target_lock, 1);
    let mut moved = world.clone();
    place_battle_unit(&mut moved, target, map, 7, 7).unwrap();
    assert_eq!(battle_hex_occupant(&moved, shooter, point).unwrap(), None);
    assert_eq!(
        moved.btech.constructed_units()[&shooter]
            .hex_lock()
            .unwrap()
            .hex,
        point
    );
    let mut safe = world.clone();
    set_battle_friendly_fire_safety(&mut safe, shooter, ObjectId(1), true).unwrap();
    assert_eq!(
        battle_hex_occupant(&safe, shooter, point).unwrap(),
        Some(target)
    );
    let safety = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(safe.clone())),
    )
    .unwrap();
    let response = support::run_text(&safety, &config, ObjectId(1), 1, "fire 0");
    assert!(response.contains("FFSafeties"), "{response}");
    assert_eq!(safety.world().btech, safe.btech);
    shot_seed(&mut world, shooter, 7);
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.fire({},1,0); error('abort')",
            shooter.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    let (selected, x, y, indirect) = lua.eval_callback::<(i64, i32, i32, bool)>(&format!("local shot = btech.unit.fire({},1,0); return shot.target, shot.coordinate.x, shot.coordinate.y, shot.aim.indirect ~= nil", shooter.0)).unwrap();
    assert_eq!((selected, x, y, indirect), (target.0, 5, 4, false));
    commands::run(&native, &config, ObjectId(1), 1, "fire 0").unwrap();
    assert_eq!(native.world().btech, lua.world().btech);
    let text = |messages: Vec<(ObjectId, stompymux_rs::text::Document)>| {
        messages
            .into_iter()
            .map(|(id, doc)| (id, doc.source().to_owned()))
            .collect::<Vec<_>>()
    };
    let messages = text(native.drain_outbox());
    assert_eq!(messages, text(lua.drain_outbox()));
    assert!(
        messages
            .iter()
            .any(|(_, text)| text.contains("at (5,4) - BTH:"))
    );
    assert!(
        native.world().btech.constructed_units()[&shooter]
            .target_lock()
            .is_none()
    );
    let saved = native.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, saved.btech);
    loaded.validate(&config).unwrap();
}

/// Empty terrain shares weapon modifiers without inheriting an occupant's motion, lock or sensor aim.
#[tokio::test]
async fn hex_aim_modes_visibility_and_neutral_target_terms() {
    use stompymux_rs::*;
    let (_dir, config, mut world, shooter, target) = shot_fixture().await;
    let point = HexCoordinate { x: 5, y: 4 };
    let map = world.btech.constructed_units()[&shooter]
        .position()
        .unwrap()
        .map;
    select_battle_hex_target(
        &mut world,
        shooter,
        ObjectId(1),
        point,
        HexTargetMode::UnitAtHex,
    )
    .unwrap();
    let unit = battle_aim_modifiers(&world, shooter, target, 0, 6, optical_aim_rules()).unwrap();
    let before = world.btech.clone();
    let base = battle_hex_aim_modifiers(&world, shooter, point, 0, 6).unwrap();
    assert_eq!(world.btech, before);
    assert!(base.visible);
    assert_eq!(base.modifiers.range, unit.range);
    assert_eq!(
        base.subtotal().unwrap(),
        unit.subtotal().unwrap()
            - i32::from(unit.target_movement)
            - i32::from(unit.target_lock)
            - i32::from(unit.perception.unwrap().modifier)
    );
    for mode in [
        HexTargetMode::UnitAtHex,
        HexTargetMode::Hex,
        HexTargetMode::Building,
        HexTargetMode::Ignite,
        HexTargetMode::Clear,
    ] {
        select_battle_hex_target(&mut world, shooter, ObjectId(1), point, mode).unwrap();
        let report = battle_hex_aim_modifiers(&world, shooter, point, 0, 6).unwrap();
        let bonus = if mode == HexTargetMode::UnitAtHex {
            0
        } else {
            -4
        };
        assert_eq!(report.hex_bonus, bonus);
        assert_eq!(
            report.subtotal().unwrap(),
            base.subtotal().unwrap() + i32::from(bonus)
        );
        assert_eq!(report.modifiers.target_movement, 0);
        assert_eq!(report.modifiers.target_lock, 0);
        assert!(report.modifiers.perception.is_none());
        assert!(report.modifiers.indirect.is_none());
    }
    // A hex the shooter cannot see, here one behind a wall, keeps the same arithmetic and
    // reports only that it is out of sight.
    let far = HexCoordinate { x: 5, y: 11 };
    let visible = battle_hex_aim_modifiers(&world, shooter, far, 0, 6).unwrap();
    assert!(visible.visible);
    world
        .btech
        .rewrite_map_record(map, |record| {
            record["terrain"][8 * 12 + 5] =
                serde_json::to_value(Hex::new(Terrain::Wall, 5)).unwrap();
        })
        .unwrap();
    let hidden = battle_hex_aim_modifiers(&world, shooter, far, 0, 6).unwrap();
    assert!(!hidden.visible);
    assert_eq!(hidden.subtotal(), visible.subtotal());
    assert_eq!(hidden.modifiers, visible.modifiers);
    world.validate(&config).unwrap();
}

/// Hex previews use terrain height and report range limits without consuming shooter or target randomness.
#[tokio::test]
async fn hex_aim_range_and_lua_inspection_are_read_only() {
    use stompymux_rs::*;
    let (_dir, config, world, shooter, _airborne_target) = long_field_fixture().await;
    let near = HexCoordinate { x: 5, y: 4 };
    let far = HexCoordinate { x: 100, y: 5 };
    let before = world.btech.clone();
    let report = battle_hex_aim_modifiers(&world, shooter, near, 0, 6).unwrap();
    assert_eq!(report.modifiers.distance, 1.0);
    let distant = battle_hex_aim_modifiers(&world, shooter, far, 0, 6).unwrap();
    assert!(distant.subtotal().is_none());
    assert_eq!(world.btech, before);
    assert!(
        battle_hex_aim_modifiers(&world, shooter, HexCoordinate { x: -1, y: 0 }, 0, 6).is_err()
    );
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let table = lua
        .eval_callback::<mlua::Table>(&format!("return btech.unit.aim_hex({},0,5,4)", shooter.0))
        .unwrap();
    assert_eq!(table.get::<f64>("distance").unwrap(), 1.0);
    assert_eq!(table.get::<i8>("target_movement").unwrap(), 0);
    assert!(table.get::<Option<i32>>("subtotal").unwrap().is_some());
    table.set("gunnery", -99).unwrap();
    assert_eq!(lua.world().btech, before);
    assert!(lua.drain_outbox().is_empty());
    let out_of_range = lua
        .eval_callback::<bool>(&format!(
            "return btech.unit.aim_hex({},0,100,5).subtotal == nil",
            shooter.0
        ))
        .unwrap();
    assert!(out_of_range);
    assert_eq!(lua.world().btech, before);
}

#[tokio::test]
async fn direct_hex_shots_commit_launch_terrain_and_restart_replay() {
    use stompymux_rs::*;
    for (mode, hit) in [
        (HexTargetMode::Ignite, true),
        (HexTargetMode::Ignite, false),
        (HexTargetMode::Clear, true),
    ] {
        let (_dir, config, mut world, shooter, target) = shot_fixture().await;
        let map = world.btech.constructed_units()[&shooter]
            .position()
            .unwrap()
            .map;
        let coordinate = HexCoordinate { x: 5, y: 3 };
        let index = world.btech.constructed_units()[&shooter]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == Weapon::MediumLaser)
            .unwrap();
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        let width = world.btech.maps()[&map].width as usize;
        crate::support::set_hex_terrain(
            &mut encoded["maps"][map.0.to_string()]["terrain"][3 * width + 5],
            stompymux_rs::Terrain::HeavyWoods,
        );
        // Heavy woods have a terrain factor of ninety; spend most of it so one laser hit
        // aimed at the hex thins them.
        encoded["maps"][map.0.to_string()]["terrain_damage"] =
            serde_json::json!({ (3 * width + 5).to_string(): 85 });
        world.btech = serde_json::from_value(encoded).unwrap();
        shot_skill(&mut world, if hit { 20 } else { 0 });
        select_battle_hex_target(&mut world, shooter, ObjectId(1), coordinate, mode).unwrap();
        let seed = (0..=255)
            .find(|seed| {
                let mut trial = world.clone();
                shot_seed(&mut trial, shooter, *seed);
                let report = resolve_battle_hex_shot(
                    &mut trial,
                    shooter,
                    ObjectId(1),
                    coordinate,
                    index,
                    shot_rules(),
                )
                .unwrap();
                report.hit == hit
                    && report
                        .terrain
                        .iter()
                        .any(|impact| impact.effect != WoodlandEffect::None)
            })
            .unwrap();
        shot_seed(&mut world, shooter, seed);
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        replay
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let before = world.clone();
        let report = resolve_battle_hex_shot(
            &mut world,
            shooter,
            ObjectId(1),
            coordinate,
            index,
            shot_rules(),
        )
        .unwrap();
        assert_eq!(
            resolve_battle_hex_shot(
                &mut replay,
                shooter,
                ObjectId(1),
                coordinate,
                index,
                shot_rules()
            )
            .unwrap(),
            report
        );
        assert_eq!(world.btech, replay.btech);
        assert_eq!(report.hit, hit);
        assert!(report.launched);
        assert_eq!(report.coordinate, coordinate);
        assert!(report.aim.visible);
        assert!(report.expenditure.heat > 0);
        assert!(
            !world.btech.constructed_units()[&shooter]
                .weapon_readiness(index)
                .unwrap()
                .ready
        );
        assert_eq!(
            world.btech.constructed_units()[&target],
            before.btech.constructed_units()[&target]
        );
        assert_eq!(
            before.btech.maps()[&map].hex(5, 3).unwrap().terrain(),
            Terrain::HeavyWoods
        );
        // Heavy woods take ninety points before thinning (Total Warfare p. 112); the terrain
        // factor already spent plus this shot carries them past it.
        assert_ne!(
            world.btech.maps()[&map].hex(5, 3).unwrap().terrain(),
            Terrain::HeavyWoods
        );
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
        let checkpoint = world.btech.clone();
        assert!(
            resolve_battle_hex_shot(
                &mut world,
                shooter,
                ObjectId(1),
                coordinate,
                index,
                shot_rules()
            )
            .is_err()
        );
        assert_eq!(world.btech, checkpoint);
    }
}

#[tokio::test]
async fn direct_hex_shot_rejection_preserves_inventory_and_dice() {
    use stompymux_rs::*;
    let (_dir, _config, mut world, shooter, _target) = shot_fixture().await;
    let index = 0;
    for (point, mode) in [
        (HexCoordinate { x: -1, y: 0 }, None),
        (HexCoordinate { x: 5, y: 4 }, None),
        (HexCoordinate { x: 5, y: 7 }, Some(HexTargetMode::Ignite)),
        (HexCoordinate { x: 5, y: 7 }, Some(HexTargetMode::Building)),
    ] {
        if let Some(mode) = mode {
            select_battle_hex_target(&mut world, shooter, ObjectId(1), point, mode).unwrap();
        }
        let before = world.btech.clone();
        assert!(
            resolve_battle_hex_shot(&mut world, shooter, ObjectId(1), point, index, shot_rules())
                .is_err()
        );
        assert_eq!(world.btech, before);
    }
}

#[tokio::test]
async fn direct_hex_missiles_spend_ammunition_and_only_resolve_packets_on_hits() {
    use stompymux_rs::*;
    for hit in [false, true] {
        let (_dir, _config, mut world, shooter, target) = shot_fixture().await;
        shot_skill(&mut world, 0);
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            "Gunnery-Missile",
            CharacterValue {
                value: if hit { 20 } else { 0 },
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        let index = world.btech.constructed_units()[&shooter]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon.profile().missiles > 0)
            .unwrap();
        let coordinate = HexCoordinate { x: 5, y: 3 };
        select_battle_hex_target(
            &mut world,
            shooter,
            ObjectId(1),
            coordinate,
            HexTargetMode::Clear,
        )
        .unwrap();
        let target_number =
            battle_pilot_hex_aim_modifiers(&world, shooter, coordinate, index, true)
                .unwrap()
                .subtotal()
                .unwrap();
        let seed = (0..=255)
            .find(|seed| (i32::from(Dice::seeded([*seed; 32]).two_d6()) >= target_number) == hit)
            .unwrap();
        shot_seed(&mut world, shooter, seed);
        let before = world.clone();
        let report = resolve_battle_hex_shot(
            &mut world,
            shooter,
            ObjectId(1),
            coordinate,
            index,
            shot_rules(),
        )
        .unwrap();
        assert!(report.launched);
        assert_eq!(report.hit, hit);
        assert_eq!(report.cluster_roll.is_some(), hit);
        assert_eq!(!report.terrain.is_empty(), hit);
        assert_eq!(
            report
                .expenditure
                .ammunition
                .iter()
                .map(|draw| draw.rounds)
                .sum::<u16>(),
            1
        );
        assert_eq!(
            world.btech.constructed_units()[&target],
            before.btech.constructed_units()[&target]
        );
    }
}

#[tokio::test]
async fn hex_fire_native_lua_routing_and_character_rollback() {
    use stompymux_rs::*;
    for character in [false, true] {
        for mode in [
            HexTargetMode::Ignite,
            HexTargetMode::Clear,
            HexTargetMode::UnitAtHex,
        ] {
            let (_dir, config, mut world, shooter, target) = shot_fixture().await;
            let map = world.btech.constructed_units()[&shooter]
                .position()
                .unwrap()
                .map;
            let coordinate = HexCoordinate { x: 5, y: 3 };
            let index = world.btech.constructed_units()[&shooter]
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|mount| mount.weapon == Weapon::MediumLaser)
                .unwrap();
            let mut encoded = serde_json::to_value(&world.btech).unwrap();
            let width = world.btech.maps()[&map].width as usize;
            crate::support::set_hex_terrain(
                &mut encoded["maps"][map.0.to_string()]["terrain"][3 * width + 5],
                stompymux_rs::Terrain::HeavyWoods,
            );
            world.btech = serde_json::from_value(encoded).unwrap();
            shot_skill(&mut world, 20);
            shot_seed(&mut world, shooter, 0);
            select_battle_hex_target(&mut world, shooter, ObjectId(1), coordinate, mode).unwrap();
            if character {
                world
                    .objects
                    .get_mut(&shooter)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
            }
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
                lua.eval_callback::<()>(&format!(
                    "btech.unit.fire({},1,{index}); error('abort')",
                    shooter.0
                ))
                .is_err()
            );
            assert_eq!(lua.world().btech, world.btech);
            assert!(lua.drain_outbox().is_empty());
            commands::run(&native, &config, ObjectId(1), 1, &format!("fire {index}")).unwrap();
            let report = lua
                .eval_callback::<mlua::Table>(&format!(
                    "return btech.unit.fire({},1,{index})",
                    shooter.0
                ))
                .unwrap();
            assert!(report.get::<Option<i64>>("target").unwrap().is_none());
            assert_eq!(report.get::<i64>("map").unwrap(), map.0);
            assert!(report.get::<bool>("launched").unwrap());
            let point = report.get::<mlua::Table>("coordinate").unwrap();
            assert_eq!(point.get::<i32>("x").unwrap(), 5);
            point.set("x", 99).unwrap();
            assert_eq!(lua.world().btech, native.world().btech);
            assert_eq!(
                lua.world().btech.constructed_units()[&target],
                world.btech.constructed_units()[&target]
            );
            let text = |messages: Vec<(ObjectId, stompymux_rs::text::Document)>| {
                messages
                    .into_iter()
                    .map(|(id, doc)| (id, doc.source().to_owned()))
                    .collect::<Vec<_>>()
            };
            let messages = text(native.drain_outbox());
            assert_eq!(messages, text(lua.drain_outbox()));
            assert!(
                messages
                    .iter()
                    .any(|(_, text)| text.contains("at (5,3) - BTH:"))
            );
            let saved = lua.world().clone();
            saved.validate(&config).unwrap();
            persistence::save(&config.database(), &saved).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                saved.btech
            );
        }
    }
}

#[tokio::test]
async fn hex_fire_character_recoil_native_lua_and_abort() {
    use stompymux_rs::*;
    let (_dir, config, mut world, shooter, _target) = shot_fixture().await;
    let mut definition = world.btech.constructed_units()[&shooter]
        .definition()
        .clone();
    let torso = definition
        .sections
        .get_mut(&MechSection::LeftTorso)
        .unwrap();
    let mut part = torso.criticals[&0].clone();
    part.equipment = "IS.HeavyGaussRifle".into();
    for slot in 0..11 {
        torso.criticals.insert(slot, part.clone());
    }
    let bin = definition
        .sections
        .values_mut()
        .flat_map(|section| section.criticals.values_mut())
        .find(|part| part.equipment.starts_with("Ammo_"))
        .unwrap();
    bin.equipment = "Ammo_IS.HeavyGaussRifle".into();
    bin.data = "4".into();
    world
        .btech
        .rewrite_unit_record(shooter, |record| {
            record["definition"] = serde_json::to_value(definition).unwrap();
            record["ammunition"][0] = 4.into();
            record["motion"]["speed"] = 1.0.into();
        })
        .unwrap();
    shot_skill(&mut world, 20);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Gunnery-Ballistic",
        CharacterValue {
            value: 20,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Piloting-Battlemech",
        CharacterValue {
            value: 0,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    world
        .objects
        .get_mut(&shooter)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let index = world.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::HeavyGaussRifle)
        .unwrap();
    select_battle_hex_target(
        &mut world,
        shooter,
        ObjectId(1),
        HexCoordinate { x: 5, y: 3 },
        HexTargetMode::UnitAtHex,
    )
    .unwrap();
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = Dice::seeded([*seed; 32]);
            dice.two_d6();
            dice.two_d6() <= 4
        })
        .unwrap();
    shot_seed(&mut world, shooter, seed);
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(shooter);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Connected);
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
        lua.eval_callback::<()>(&format!(
            "btech.unit.fire({},1,{index}); error('abort')",
            shooter.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    commands::run(&native, &config, ObjectId(1), 1, &format!("fire {index}")).unwrap();
    let report = lua
        .eval_callback::<mlua::Table>(&format!("return btech.unit.fire({},1,{index})", shooter.0))
        .unwrap();
    let recoil = report.get::<mlua::Table>("recoil").unwrap();
    assert!(
        !recoil
            .get::<mlua::Table>("check")
            .unwrap()
            .get::<bool>("success")
            .unwrap()
    );
    assert!(recoil.get::<Option<mlua::Table>>("fall").unwrap().is_some());
    // Independent actions can cross a wall-clock second when recording the same XP award.
    assert_eq!(
        without_xp_timestamps(&lua.world().btech),
        without_xp_timestamps(&native.world().btech)
    );
    assert_eq!(
        lua.world().btech.constructed_units()[&shooter].posture(),
        Posture::Prone
    );
    let text = |messages: Vec<(ObjectId, stompymux_rs::text::Document)>| {
        messages
            .into_iter()
            .map(|(id, doc)| (id, doc.source().to_owned()))
            .collect::<Vec<_>>()
    };
    let messages = text(native.drain_outbox());
    assert_eq!(messages, text(lua.drain_outbox()));
    let pilot: Vec<_> = messages
        .iter()
        .filter(|(who, _)| *who == ObjectId(1))
        .map(|(_, text)| text.as_str())
        .collect();
    let index = pilot
        .iter()
        .position(|text| *text == "You make a piloting skill roll!")
        .unwrap();
    assert_eq!(
        pilot[index - 1],
        "You realize that moving while firing this weapon may not be a good idea after all."
    );
    assert!(pilot[index + 1].starts_with("Modified Pilot Skill: BTH "));
    assert_eq!(
        pilot[index + 2],
        "The weapon's recoil knocks you to the ground!"
    );
    assert!(!messages.iter().any(|(who, text)| *who == ObjectId(2)
        && (text.starts_with("You make a piloting") || text.starts_with("Modified Pilot Skill:"))));

    assert!(
        messages
            .iter()
            .any(|(_, text)| text.contains("recoil knocks you to the ground"))
    );
    let saved = lua.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}

#[tokio::test]
async fn hex_surface_weapon_probability_and_replay() {
    use stompymux_rs::*;
    for terrain in [Terrain::Ice, Terrain::Bridge] {
        for breaks in [false, true] {
            let (_dir, config, mut world, shooter, target) = shot_fixture().await;
            let coordinate = HexCoordinate { x: 5, y: 4 };
            let map = world.btech.constructed_units()[&shooter]
                .position()
                .unwrap()
                .map;
            let index = world.btech.constructed_units()[&shooter]
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|mount| mount.weapon == Weapon::MediumLaser)
                .unwrap();
            let mut encoded = serde_json::to_value(&world.btech).unwrap();
            let width = world.btech.maps()[&map].width as usize;
            // A medium laser's five points collapse a bridge with five CF left and only
            // shake one at full strength; ice breaks on a d15 roll instead.
            let mut hex = stompymux_rs::Hex::new(terrain, 1);
            if terrain == Terrain::Bridge && breaks {
                hex = hex.with_structure(Some(Structure {
                    cf: 5,
                    ..Structure::bridge(1)
                }));
            }
            encoded["maps"][map.0.to_string()]["terrain"][4 * width + 5] =
                serde_json::to_value(hex).unwrap();
            world.btech = serde_json::from_value(encoded).unwrap();
            place_battle_unit(&mut world, target, map, 5, 4).unwrap();
            shot_skill(&mut world, 20);
            select_battle_hex_target(
                &mut world,
                shooter,
                ObjectId(1),
                coordinate,
                HexTargetMode::Hex,
            )
            .unwrap();
            let seed = (0..=255)
                .find(|seed| {
                    let mut trial = world.clone();
                    shot_seed(&mut trial, shooter, *seed);
                    let report = resolve_battle_hex_shot(
                        &mut trial,
                        shooter,
                        ObjectId(1),
                        coordinate,
                        index,
                        shot_rules(),
                    )
                    .unwrap();
                    report.surfaces[0].fracture.is_some() == breaks
                })
                .unwrap();
            shot_seed(&mut world, shooter, seed);
            let before = world.clone();
            for mode in [HexTargetMode::Ignite, HexTargetMode::Clear] {
                let mut non_structural = before.clone();
                select_battle_hex_target(
                    &mut non_structural,
                    shooter,
                    ObjectId(1),
                    coordinate,
                    mode,
                )
                .unwrap();
                let report = resolve_battle_hex_shot(
                    &mut non_structural,
                    shooter,
                    ObjectId(1),
                    coordinate,
                    index,
                    shot_rules(),
                )
                .unwrap();
                assert!(report.surfaces.is_empty());
                assert_eq!(
                    non_structural.btech.maps()[&map]
                        .base_hex(5, 4)
                        .unwrap()
                        .terrain(),
                    terrain
                );
            }
            let mut miss = before.clone();
            shot_skill(&mut miss, 0);
            let target_number =
                battle_pilot_hex_aim_modifiers(&miss, shooter, coordinate, index, true)
                    .unwrap()
                    .subtotal()
                    .unwrap();
            let miss_seed = (0..=255)
                .find(|seed| i32::from(Dice::seeded([*seed; 32]).two_d6()) < target_number)
                .unwrap();
            shot_seed(&mut miss, shooter, miss_seed);
            let missed = resolve_battle_hex_shot(
                &mut miss,
                shooter,
                ObjectId(1),
                coordinate,
                index,
                shot_rules(),
            )
            .unwrap();
            assert!(!missed.hit);
            assert!(missed.surfaces.is_empty());
            assert_eq!(
                miss.btech.maps()[&map].base_hex(5, 4).unwrap().terrain(),
                terrain
            );

            let report = resolve_battle_hex_shot(
                &mut world,
                shooter,
                ObjectId(1),
                coordinate,
                index,
                shot_rules(),
            )
            .unwrap();
            assert!(report.hit);
            assert_eq!(report.surfaces.len(), 1);
            let impact = &report.surfaces[0];
            assert_eq!(impact.damage, 5);
            if terrain == Terrain::Ice {
                assert_eq!(impact.roll.unwrap() <= 5, breaks);
                assert_eq!(impact.cf, None);
            } else {
                assert_eq!(impact.roll, None);
                assert_eq!(impact.cf, Some(if breaks { 0 } else { 35 }));
                if !breaks {
                    assert_eq!(
                        world.btech.maps()[&map]
                            .base_hex(5, 4)
                            .unwrap()
                            .structure()
                            .unwrap()
                            .cf,
                        35
                    );
                }
            }
            assert_eq!(impact.fracture.is_some(), breaks);
            assert_eq!(
                world.btech.maps()[&map].base_hex(5, 4).unwrap().terrain(),
                if breaks { Terrain::Water } else { terrain }
            );
            if breaks {
                assert!(
                    impact
                        .fracture
                        .as_ref()
                        .unwrap()
                        .falls
                        .iter()
                        .any(|(id, _)| *id == target)
                );
            } else {
                assert_eq!(
                    world.btech.constructed_units()[&target],
                    before.btech.constructed_units()[&target]
                );
            }
            let mut replay = before.clone();
            assert_eq!(
                resolve_battle_hex_shot(
                    &mut replay,
                    shooter,
                    ObjectId(1),
                    coordinate,
                    index,
                    shot_rules()
                )
                .unwrap(),
                report
            );
            assert_eq!(replay.btech, world.btech);
            persistence::save(&config.database(), &world).await.unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                world.btech
            );
            if terrain == Terrain::Bridge {
                let mut protected = before;
                let mut encoded = serde_json::to_value(&protected.btech).unwrap();
                encoded["maps"][map.0.to_string()]["flags"] =
                    (protected.btech.maps()[&map].flags | 64).into();
                protected.btech = serde_json::from_value(encoded).unwrap();
                let report = resolve_battle_hex_shot(
                    &mut protected,
                    shooter,
                    ObjectId(1),
                    coordinate,
                    index,
                    shot_rules(),
                )
                .unwrap();
                assert!(report.surfaces.is_empty());
                assert_eq!(
                    protected.btech.maps()[&map]
                        .base_hex(5, 4)
                        .unwrap()
                        .terrain(),
                    Terrain::Bridge
                );
            }
        }
    }
}

/// Hex fire wears down a building's construction factor; when none is left it collapses to
/// rubble and drops the Mech standing on its roof.
#[tokio::test]
async fn hex_fire_collapses_buildings_onto_their_occupants() {
    use stompymux_rs::*;
    for (cf, collapses) in [(40, false), (5, true)] {
        let (_dir, _config, mut world, shooter, target) = shot_fixture().await;
        let coordinate = HexCoordinate { x: 5, y: 4 };
        let map = world.btech.constructed_units()[&shooter]
            .position()
            .unwrap()
            .map;
        let index = world.btech.constructed_units()[&shooter]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == Weapon::MediumLaser)
            .unwrap();
        let building = Hex::new(Terrain::Building, 2).with_structure(Some(Structure {
            cf,
            ..Structure::building(2)
        }));
        let mut encoded = serde_json::to_value(&world.btech).unwrap();
        let width = world.btech.maps()[&map].width as usize;
        encoded["maps"][map.0.to_string()]["terrain"][4 * width + 5] =
            serde_json::to_value(building).unwrap();
        world.btech = serde_json::from_value(encoded).unwrap();
        place_battle_unit(&mut world, target, map, 5, 4).unwrap();
        shot_skill(&mut world, 20);
        select_battle_hex_target(
            &mut world,
            shooter,
            ObjectId(1),
            coordinate,
            HexTargetMode::Hex,
        )
        .unwrap();
        let seed = (0..=255)
            .find(|seed| {
                let mut trial = world.clone();
                shot_seed(&mut trial, shooter, *seed);
                resolve_battle_hex_shot(
                    &mut trial,
                    shooter,
                    ObjectId(1),
                    coordinate,
                    index,
                    shot_rules(),
                )
                .unwrap()
                .hit
            })
            .unwrap();
        shot_seed(&mut world, shooter, seed);
        let report = resolve_battle_hex_shot(
            &mut world,
            shooter,
            ObjectId(1),
            coordinate,
            index,
            shot_rules(),
        )
        .unwrap();
        let impact = &report.surfaces[0];
        assert_eq!(impact.surface, Surface::Roof);
        assert_eq!(impact.cf, Some(cf - 5));
        let after = world.btech.maps()[&map].base_hex(5, 4).unwrap();
        if !collapses {
            assert!(impact.fracture.is_none());
            assert_eq!(after.structure().unwrap().cf, 35);
            continue;
        }
        assert_eq!(after, Hex::new(Terrain::Rubble, 0));
        let fracture = impact.fracture.as_ref().unwrap();
        assert_eq!(fracture.fall_levels, 3);
        assert!(fracture.falls.iter().any(|(id, _)| *id == target));
        assert!(
            impact
                .notices
                .iter()
                .any(|notice| notice.text.contains("collapses!"))
        );
    }
}

#[tokio::test]
async fn hex_surface_weapon_native_lua_character_rollback() {
    use stompymux_rs::*;
    let (_dir, config, mut world, shooter, target) = shot_fixture().await;
    let coordinate = HexCoordinate { x: 5, y: 4 };
    let map = world.btech.constructed_units()[&shooter]
        .position()
        .unwrap()
        .map;
    let index = world.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::MediumLaser)
        .unwrap();
    let mut encoded = serde_json::to_value(&world.btech).unwrap();
    let width = world.btech.maps()[&map].width as usize;
    encoded["maps"][map.0.to_string()]["terrain"][4 * width + 5] =
        serde_json::to_value(stompymux_rs::Hex::new(stompymux_rs::Terrain::Ice, 1)).unwrap();
    world.btech = serde_json::from_value(encoded).unwrap();
    place_battle_unit(&mut world, target, map, 5, 4).unwrap();
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
    world
        .objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
    set_battle_character(
        &mut world,
        ObjectId(2),
        Character {
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
    world.btech.set_unit_power(target, Power::Running).unwrap();
    let passenger = world.create(&config, "Ice passenger".into(), Kind::Player);
    world.objects.get_mut(&passenger).unwrap().location = Some(target);
    world
        .objects
        .get_mut(&passenger)
        .unwrap()
        .flags
        .insert(Flag::Connected);

    shot_skill(&mut world, 20);
    select_battle_hex_target(
        &mut world,
        shooter,
        ObjectId(1),
        coordinate,
        HexTargetMode::Hex,
    )
    .unwrap();
    let seed = (0..=255)
        .find(|seed| {
            let mut trial = world.clone();
            shot_seed(&mut trial, shooter, *seed);
            resolve_battle_hex_shot(
                &mut trial,
                shooter,
                ObjectId(1),
                coordinate,
                index,
                shot_rules(),
            )
            .unwrap()
            .surfaces[0]
                .fracture
                .is_some()
        })
        .unwrap();
    shot_seed(&mut world, shooter, seed);
    for id in [shooter, target] {
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
    }
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
        lua.eval_callback::<()>(&format!(
            "btech.unit.fire({},1,{index}); error('abort')",
            shooter.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    let started = chrono::Utc::now().timestamp();
    commands::run(&native, &config, ObjectId(1), 1, &format!("fire {index}")).unwrap();
    lua.eval_callback::<()>(&format!("btech.unit.fire({},1,{index})", shooter.0))
        .unwrap();
    let finished = chrono::Utc::now().timestamp();
    // Separate wall-clock award times from deterministic gameplay state.
    let mut states = [
        serde_json::to_value(&lua.world().btech).unwrap(),
        serde_json::to_value(&native.world().btech).unwrap(),
    ];
    for state in &mut states {
        for values in state["character_values"]
            .as_object_mut()
            .unwrap()
            .values_mut()
        {
            for value in values.as_object_mut().unwrap().values_mut() {
                let timestamp = value["last_used"].as_i64().unwrap();
                if timestamp != 0 {
                    assert!((started..=finished).contains(&timestamp));
                    value["last_used"] = 0.into();
                }
            }
        }
    }
    assert_eq!(states[1], states[0]);
    assert_eq!(
        lua.world().btech.maps()[&map]
            .base_hex(5, 4)
            .unwrap()
            .terrain(),
        Terrain::Water
    );
    let text = |messages: Vec<(ObjectId, stompymux_rs::text::Document)>| {
        messages
            .into_iter()
            .map(|(id, doc)| (id, doc.source().to_owned()))
            .collect::<Vec<_>>()
    };
    let messages = text(native.drain_outbox());
    let pilot: Vec<_> = messages
        .iter()
        .filter(|(who, _)| *who == ObjectId(2))
        .map(|(_, message)| message.as_str())
        .collect();
    let index = pilot
        .iter()
        .position(|message| *message == "You make a piloting skill roll!")
        .unwrap();
    assert!(pilot[index + 1].starts_with("Modified Pilot Skill: BTH "));
    assert!(!messages.iter().any(|(who, message)| *who != ObjectId(2)
        && (message.starts_with("Modified Pilot Skill:")
            || message == "You make a piloting skill roll!")));

    assert_eq!(messages, text(lua.drain_outbox()));
    assert!(
        messages
            .iter()
            .any(|(_, text)| text.contains("ice breaks from the blast"))
    );
    let final_world = lua.world().clone();
    persistence::save(&config.database(), &final_world)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        final_world.btech
    );
}

#[tokio::test]
async fn building_fire_damage_policies_and_committed_repair() {
    use stompymux_rs::*;
    for initial in [3, 10] {
        let (_dir, config, mut world, shooter, target) = shot_fixture().await;
        let map = world.btech.constructed_units()[&shooter]
            .position()
            .unwrap()
            .map;
        let interior = world.create(&config, "Hangar".into(), Kind::Room);
        create_battle_map(
            &mut world,
            interior,
            "inside.map",
            MapAsset::from_cells("1 1\n.0\n").unwrap(),
        )
        .unwrap();
        support::seed_object_dice(&mut world, interior, support::FIXTURE_DICE_SEED);
        let coordinate = HexCoordinate { x: 5, y: 4 };
        set_building_entrance(
            &mut world,
            map,
            0,
            Some(BuildingEntrance {
                coordinate,
                interior,
                data_char: 0,
                data_short: 0,
                data_int: 0,
            }),
        )
        .unwrap();
        set_building_state(
            &mut world,
            interior,
            BuildingState {
                integrity: initial,
                maximum_integrity: initial,
                flags: 0,
                regeneration: 2,
            },
        )
        .unwrap();
        shot_skill(&mut world, 20);
        shot_seed(&mut world, shooter, 0);
        select_battle_hex_target(
            &mut world,
            shooter,
            ObjectId(1),
            coordinate,
            HexTargetMode::Building,
        )
        .unwrap();
        let index = world.btech.constructed_units()[&shooter]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == Weapon::MediumLaser)
            .unwrap();
        let before = world.clone();

        let mut absent = before.clone();
        set_building_entrance(&mut absent, map, 0, None).unwrap();
        let absent_report = resolve_battle_hex_shot(
            &mut absent,
            shooter,
            ObjectId(1),
            coordinate,
            index,
            shot_rules(),
        )
        .unwrap();
        assert!(absent_report.launched && absent_report.buildings.is_empty());
        assert_eq!(absent.btech.maps()[&interior].building.integrity, initial);
        let mut missed = before.clone();
        shot_skill(&mut missed, 0);
        let target_number =
            battle_pilot_hex_aim_modifiers(&missed, shooter, coordinate, index, true)
                .unwrap()
                .subtotal()
                .unwrap();
        let seed = (0..=255)
            .find(|seed| i32::from(Dice::seeded([*seed; 32]).two_d6()) < target_number)
            .unwrap();
        shot_seed(&mut missed, shooter, seed);
        let miss_report = resolve_battle_hex_shot(
            &mut missed,
            shooter,
            ObjectId(1),
            coordinate,
            index,
            shot_rules(),
        )
        .unwrap();
        assert!(!miss_report.hit && miss_report.buildings.is_empty());
        assert_eq!(missed.btech.maps()[&interior].building.integrity, initial);
        let mut pending = before.clone();
        pending
            .btech
            .rewrite_map_record(interior, |record| {
                record["building"]["integrity"] = (initial - 1).into();
                record["building_repair"] = 60.into();
            })
            .unwrap();
        let _report = resolve_battle_hex_shot(
            &mut pending,
            shooter,
            ObjectId(1),
            coordinate,
            index,
            shot_rules(),
        )
        .unwrap();
        assert_eq!(pending.btech.maps()[&interior].building_repair, Some(60));
        for _ in 0..60 {
            advance_building_repairs(&mut pending);
        }
        assert_eq!(
            pending.btech.maps()[&interior].building.integrity,
            if initial == 10 { 6 } else { 0 }
        );
        assert_eq!(building_repair_pending(&pending), initial == 10);
        for (immune_map, flag) in [(map, 2), (interior, 1)] {
            let mut immune = before.clone();
            let state = immune.btech.maps()[&immune_map].building;
            set_building_state(
                &mut immune,
                immune_map,
                BuildingState {
                    flags: flag,
                    ..state
                },
            )
            .unwrap();
            let report = resolve_battle_hex_shot(
                &mut immune,
                shooter,
                ObjectId(1),
                coordinate,
                index,
                shot_rules(),
            )
            .unwrap();
            assert!(report.buildings[0].immune);
            assert_eq!(immune.btech.maps()[&interior].building.integrity, initial);
            assert!(!building_repair_pending(&immune));
        }
        let report = resolve_battle_hex_shot(
            &mut world,
            shooter,
            ObjectId(1),
            coordinate,
            index,
            shot_rules(),
        )
        .unwrap();
        assert!(report.hit);
        assert!(report.terrain.is_empty() && report.surfaces.is_empty());
        assert_eq!(report.buildings[0].damage, initial.min(5) as u16);
        assert_eq!(report.buildings[0].destroyed, initial == 3);
        assert_eq!(
            world.btech.constructed_units()[&target],
            before.btech.constructed_units()[&target]
        );
        assert_eq!(
            world.btech.maps()[&interior].building.integrity,
            (initial - 5).max(0)
        );
        assert_eq!(
            world.btech.maps()[&interior].building_repair,
            (initial == 10).then_some(120)
        );
        persistence::save(&config.database(), &world).await.unwrap();
        for _ in 0..59 {
            advance_building_repairs(&mut world);
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        for _ in 0..61 {
            advance_building_repairs(&mut world);
            advance_building_repairs(&mut replay);
        }
        assert_eq!(world.btech, replay.btech);
        assert_eq!(
            world.btech.maps()[&interior].building.integrity,
            if initial == 10 { 7 } else { 0 }
        );
        for _ in 0..240 {
            advance_building_repairs(&mut world);
        }
        assert_eq!(
            world.btech.maps()[&interior].building.integrity,
            if initial == 10 { 10 } else { 0 }
        );
        assert!(!building_repair_pending(&world));
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

#[tokio::test]
async fn building_fire_native_lua_interior_messages_and_rollback() {
    use stompymux_rs::*;
    let (_dir, config, mut world, shooter, _target) = shot_fixture().await;
    let map = world.btech.constructed_units()[&shooter]
        .position()
        .unwrap()
        .map;
    let interior = world.create(&config, "Hangar".into(), Kind::Room);
    create_battle_map(
        &mut world,
        interior,
        "inside.map",
        MapAsset::from_cells("1 1\n.0\n").unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, interior, support::FIXTURE_DICE_SEED);
    let resident = world.create(&config, "Resident".into(), Kind::Player);
    world.objects.get_mut(&resident).unwrap().location = Some(interior);
    world.objects.get_mut(&resident).unwrap().home = Some(ObjectId(config.home()));
    let coordinate = HexCoordinate { x: 5, y: 4 };
    set_building_entrance(
        &mut world,
        map,
        0,
        Some(BuildingEntrance {
            coordinate,
            interior,
            data_char: 0,
            data_short: 0,
            data_int: 0,
        }),
    )
    .unwrap();
    set_building_state(
        &mut world,
        interior,
        BuildingState {
            integrity: 10,
            maximum_integrity: 10,
            flags: 0,
            regeneration: 1,
        },
    )
    .unwrap();
    shot_skill(&mut world, 20);
    shot_seed(&mut world, shooter, 0);
    select_battle_hex_target(
        &mut world,
        shooter,
        ObjectId(1),
        coordinate,
        HexTargetMode::Building,
    )
    .unwrap();
    let index = world.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::MediumLaser)
        .unwrap();
    world
        .objects
        .get_mut(&shooter)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
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
        lua.eval_callback::<()>(&format!(
            "btech.unit.fire({},1,{index}); error('abort')",
            shooter.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    commands::run(&native, &config, ObjectId(1), 1, &format!("fire {index}")).unwrap();
    lua.eval_callback::<()>(&format!("btech.unit.fire({},1,{index})", shooter.0))
        .unwrap();
    assert_eq!(lua.world().btech, native.world().btech);
    assert_eq!(lua.world().btech.maps()[&interior].building.integrity, 5);
    let text = |messages: Vec<(ObjectId, stompymux_rs::text::Document)>| {
        messages
            .into_iter()
            .map(|(id, doc)| (id, doc.source().to_owned()))
            .collect::<Vec<_>>()
    };
    let messages = text(native.drain_outbox());
    assert_eq!(messages, text(lua.drain_outbox()));
    assert!(
        messages
            .iter()
            .any(|(id, text)| *id == resident && text.contains("Hangar is hit for 5 points"))
    );
    assert!(
        messages.iter().any(
            |(id, text)| *id == ObjectId(1) && text.contains("You hit the Hangar for 5 points")
        )
    );
    let saved = lua.world().clone();
    persistence::save(&config.database(), &saved).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        saved.btech
    );
}

/// Give the Jenner an inferno bin without altering weapon selection or live dice.
fn install_test_inferno(world: &mut stompymux_rs::World, id: ObjectId) -> usize {
    use stompymux_rs::*;
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["definition"]["sections"]["RightTorso"]["criticals"]["0"]["modes"] =
                serde_json::json!(["Inferno"]);
        })
        .unwrap();
    world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|m| m.weapon == Weapon::Srm4)
        .unwrap()
}

#[tokio::test]
async fn inferno_ammunition_native_lua_fire_restart_and_rollback() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = shot_fixture().await;
    let index = install_test_inferno(&mut world, id);
    shot_skill(&mut world, 0);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Gunnery-Missile",
        CharacterValue {
            value: 30,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    shot_seed(&mut world, id, 11);
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    world
        .objects
        .get_mut(&target)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    world.validate(&config).unwrap();
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
        lua.eval_callback::<()>(&format!(
            "btech.unit.inferno({},1,{index}); error('abort')",
            id.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    for expected in ["inferno", "normal", "inferno"] {
        let output = support::run_text(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("inferno {index}"),
        );
        assert!(output.contains("missiles"), "{output}");
        assert_eq!(
            lua.eval_callback::<String>(&format!("return btech.unit.inferno({},1,{index})", id.0))
                .unwrap(),
            expected
        );
        assert_eq!(native.world().btech, lua.world().btech);
    }
    assert!(support::run_text(&native, &config, ObjectId(1), 1, "weapons").contains("[Inferno]"));
    native.drain_outbox();
    lua.drain_outbox();
    let selected = native.world().clone();
    persistence::save(&config.database(), &selected)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        selected.btech
    );
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.fire({},1,{index},{}); error('abort')",
            id.0, target.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, selected.btech);
    assert!(lua.drain_outbox().is_empty());
    commands::run(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("fire {index} #{}", target.0),
    )
    .unwrap();
    let heat = lua
        .eval_callback::<u32>(&format!(
            "local r=btech.unit.fire({},1,{index},{}); return r.salvo.report.inferno.heat",
            id.0, target.0
        ))
        .unwrap();
    assert!(heat > 0);
    assert_eq!(lua.world().btech, native.world().btech);
    assert_eq!(
        lua.world().btech.constructed_units()[&target].sections(),
        selected.btech.constructed_units()[&target].sections()
    );
    // Each inferno missile adds two heat (Total Warfare p. 141).
    assert_eq!(
        lua.world().btech.constructed_units()[&target].heat().stored,
        selected.btech.constructed_units()[&target].heat().stored + f64::from(heat)
    );
    let messages = |out: Vec<(ObjectId, stompymux_rs::text::Document)>| {
        out.into_iter()
            .map(|(id, text)| (id, text.source().to_owned()))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        messages(native.drain_outbox()),
        messages(lua.drain_outbox())
    );
    let after = native.world().clone();
    persistence::save(&config.database(), &after).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        after.btech
    );
}

#[tokio::test]
async fn inferno_ammunition_interception_and_overflow_preserve_firing_transaction() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    let index = install_test_inferno(&mut base, id);
    toggle_battle_inferno(&mut base, id, ObjectId(1), index).unwrap();
    shot_skill(&mut base, 0);
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
    install_test_ams(&mut base, target, Weapon::AntiMissileSystem);
    base.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
    assign_battle_pilot(&mut base, target, ObjectId(2)).unwrap();
    support::seed_object_dice(&mut base, ObjectId(2), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut base, target, ObjectId(2), true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut base, 0, true);
    }
    set_battle_ams(&mut base, target, ObjectId(2), true).unwrap();
    face_attacker(&mut base, target);
    let mut all = false;
    let mut partial = false;
    for seed in 0..64 {
        let mut world = base.clone();
        shot_seed(&mut world, id, seed);
        shot_seed(&mut world, target, seed);
        let before = world.clone();
        let report =
            resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules()).unwrap();
        let Some(stompymux_rs::TargetSalvo::Mech(salvo)) = report.salvo else {
            continue;
        };
        assert!(salvo.groups.is_empty());
        assert!(salvo.experience.is_empty());
        let engaged = report.ams.is_some();
        let intercepted = report.ams.map_or(0, |ams| ams.shot_down);
        let hits = salvo
            .missiles_before_defense
            .unwrap()
            .saturating_sub(intercepted);
        assert_eq!(
            world.btech.constructed_units()[&target].sections(),
            before.btech.constructed_units()[&target].sections()
        );
        // Engaging the anti-missile system heats the defender (Total Warfare p. 130).
        let ams_heat = if engaged {
            f64::from(Weapon::AntiMissileSystem.profile().heat)
        } else {
            0.0
        };
        if hits == 0 {
            all = true;
            assert!(salvo.inferno.is_none());
            assert_eq!(
                world.btech.constructed_units()[&target].heat().stored,
                before.btech.constructed_units()[&target].heat().stored + ams_heat
            );
        } else {
            partial |= intercepted > 0;
            // Two heat points per missile (Total Warfare p. 141), plus the AMS heat.
            let heat = salvo.inferno.unwrap().heat;
            assert_eq!(heat, (hits * 2).min(15) as u8);
            assert_eq!(
                world.btech.constructed_units()[&target].heat().stored,
                before.btech.constructed_units()[&target].heat().stored
                    + f64::from(heat)
                    + ams_heat
            );
        }
        if partial {
            break;
        }
    }
    // The -4 on the cluster roll never stops a whole flight (Total Warfare p. 129).
    assert!(partial && !all);
    base.validate(&config).unwrap();
}

#[tokio::test]
async fn inferno_ammunition_heat_penalty_selects_inferno_before_larger_bins() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, _target) = shot_fixture().await;
    install_test_inferno(&mut base, id);
    base.btech
        .rewrite_unit_record(id, |record| {
            let mut bin = record["definition"]["sections"]["RightTorso"]["criticals"]["0"].clone();
            bin["modes"] = serde_json::json!([]);
            record["definition"]["sections"]["CenterTorso"]["criticals"]["11"] = bin;
            record["ammunition"] = serde_json::json!([1, 25]);
        })
        .unwrap();
    base.validate(&config).unwrap();
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    for (heat, target) in [
        (10.0, 4),
        (13.99, 4),
        (14.0, 6),
        (18.99, 6),
        (19.0, 8),
        (22.99, 8),
        (23.0, 10),
        (27.99, 10),
        (28.0, 12),
    ] {
        let mut world = base.clone();
        overheat_due(&mut world, id, heat, false);
        shot_seed(&mut world, id, seed);
        let report = advance_battle_overheat(&mut world, overheat_rules())
            .unwrap()
            .remove(0);
        assert_eq!(report.inferno_check.unwrap().target, target);
        assert!(report.inferno_explosion.is_some());
        // The ordinary Avoid Roll at 19+ is separate and may detonate the larger bin too.
        let ordinary = report.ammunition_check.is_some_and(|check| !check.success);
        assert_eq!(report.explosion.is_some(), ordinary);
        assert_eq!(report.ammunition_check.is_some(), heat >= 19.0);
        assert_eq!(
            world.btech.constructed_units()[&id].ammunition(),
            &[0, if ordinary { 0 } else { 25 }]
        );
        assert!(world.btech.constructed_units()[&id].heat().stored > heat + 10.0);
    }
}

#[tokio::test]
async fn inferno_ammunition_hex_hits_apply_one_zero_damage_terrain_exposure() {
    use stompymux_rs::*;
    let (_dir, _config, mut base, id, target) = shot_fixture().await;
    let index = install_test_inferno(&mut base, id);
    toggle_battle_inferno(&mut base, id, ObjectId(1), index).unwrap();
    shot_skill(&mut base, 0);
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
    let map = base.btech.constructed_units()[&id].position().unwrap().map;
    let coordinate = HexCoordinate { x: 5, y: 4 };
    let width = base.btech.maps()[&map].width as usize;
    base.btech
        .rewrite_map_record(map, |record| {
            record["terrain"][4 * width + 5] =
                serde_json::to_value(stompymux_rs::Hex::new(Terrain::HeavyWoods, 0)).unwrap();
        })
        .unwrap();
    for mode in [
        HexTargetMode::Ignite,
        HexTargetMode::Clear,
        HexTargetMode::Hex,
        HexTargetMode::Building,
    ] {
        let mut selected = base.clone();
        select_battle_hex_target(&mut selected, id, ObjectId(1), coordinate, mode).unwrap();
        for seed in 0..8 {
            let mut world = selected.clone();
            shot_seed(&mut world, id, seed);
            let report = resolve_battle_hex_shot(
                &mut world,
                id,
                ObjectId(1),
                coordinate,
                index,
                shot_rules(),
            )
            .unwrap();
            assert!(report.hit);
            assert!(report.cluster_roll.is_some());
            assert_eq!(
                report.terrain.len(),
                usize::from(mode != HexTargetMode::Building)
            );
            assert!(report.buildings.is_empty());
            assert!(report.surfaces.is_empty());
            assert_eq!(
                world.btech.maps()[&map].base_hex(5, 4).unwrap().terrain(),
                Terrain::HeavyWoods
            );
            assert_eq!(
                world.btech.constructed_units()[&target],
                selected.btech.constructed_units()[&target]
            );
        }
    }
}

/// Speed Demon changes acceleration and braking through the live movement path and saved replay.
#[tokio::test]
async fn speed_demon_acceleration_braking_and_restart_use_the_assigned_pilot() {
    use stompymux_rs::{CharacterValue, set_battle_character_value};
    let (_dir, config, mut world, id) = fixture('.').await;
    stompymux_rs::set_battle_character(
        &mut world,
        ObjectId(1),
        stompymux_rs::Character {
            bruise: 0,
            lethal: 0,
            build: 5,
            reflexes: 5,
            intuition: 5,
            learn: 5,
            charisma: 5,
        },
    )
    .unwrap();
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Speed_Demon",
        CharacterValue {
            value: 1,
            ..Default::default()
        },
    )
    .unwrap();
    set_battle_speed(&mut world, id, ObjectId(1), 118.25).unwrap();
    for step in 1..=8 {
        advance_battle_motion(&mut world, RULES).unwrap();
        assert!(
            (world.btech.constructed_units()[&id].motion().unwrap().speed - step as f64 * 7.390625)
                .abs()
                < 1e-10
        );
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    for _ in 0..8 {
        assert_eq!(
            advance_battle_motion(&mut restored, RULES).unwrap(),
            advance_battle_motion(&mut world, RULES).unwrap()
        );
        assert_eq!(restored.btech, world.btech);
    }
    assert_eq!(
        world.btech.constructed_units()[&id].motion().unwrap().speed,
        118.25
    );
    set_battle_speed(&mut world, id, ObjectId(1), 0.0).unwrap();
    advance_battle_motion(&mut world, RULES).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id].motion().unwrap().speed,
        110.859375
    );
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Speed_Demon",
        CharacterValue {
            value: 2,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id].ground_acceleration(&world),
        5.9125
    );
    world.validate(&config).unwrap();
}

/// Lateral travel rejects bipeds and damaged quad support without changing state.
#[tokio::test]
async fn quad_lateral_requires_intact_support() {
    use stompymux_rs::*;
    let (_dir, _config, mut world, id) = fixture('.').await;
    let before = world.btech.clone();
    assert!(set_battle_lateral(&mut world, id, ObjectId(1), LateralMode::FrontLeft).is_err());
    assert_eq!(world.btech, before);
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["definition"]["attributes"]["move_type"] = "Quad".into();
        })
        .unwrap();
    set_battle_lateral(&mut world, id, ObjectId(1), LateralMode::FrontLeft).unwrap();
    assert_eq!(world.btech.constructed_units()[&id].lateral().remaining, 6);
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["flooded_sections"] = serde_json::to_value([MechSection::LeftArm]).unwrap();
        })
        .unwrap();
    let before = world.btech.clone();
    assert!(set_battle_lateral(&mut world, id, ObjectId(1), LateralMode::FrontRight).is_err());
    assert_eq!(world.btech, before);
}

/// Constructed quads use live movement, automatic standing and durable world persistence.
#[tokio::test]
async fn quad_live_movement_standing_and_restart() {
    use stompymux_rs::*;
    let source = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
    let template =
        MechTemplate::parse("SCP-1N", include_str!("../game/units/SCP-1N.toml")).unwrap();
    let (_dir, config, mut world, id) = fixture_assets(&source, template).await;
    assert_eq!(
        world.btech.constructed_units()[&id].chassis(),
        MechChassis::Quad
    );
    set_battle_speed(&mut world, id, ObjectId(1), 96.75).unwrap();
    for _ in 0..5 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    assert!((world.btech.constructed_units()[&id].motion().unwrap().speed - 48.375).abs() < 0.001);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut restored = persistence::load(&config.database()).await.unwrap();
    for _ in 0..5 {
        assert_eq!(
            advance_battle_motion(&mut world, RULES).unwrap(),
            advance_battle_motion(&mut restored, RULES).unwrap()
        );
    }
    assert_eq!(world.btech, restored.btech);
    set_battle_speed(&mut world, id, ObjectId(1), 0.0).unwrap();
    for _ in 0..10 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    shot_seed(&mut world, id, 1);
    let _fall = resolve_battle_fall(&mut world, id, 1, fall_rules()).unwrap();
    assert!(!world.btech.constructed_units()[&id].is_destroyed());
    let before =
        serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"].clone();
    let stand = begin_battle_stand(
        &mut world,
        id,
        ObjectId(1),
        StandMode::Normal,
        true,
        fall_rules(),
    )
    .unwrap();
    assert!(stand.check.success);
    assert!(stand.check.roll.is_none());
    assert!(stand.timer.is_some());
    assert_eq!(
        before,
        serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"]
    );
    assert!(rotate_battle_torso(&mut world, id, ObjectId(1), Torso::Left).is_err());
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

/// Live quad weapon and front-leg attacks use the normal combat and persistence paths.
#[tokio::test]
async fn quad_live_weapon_and_front_leg_combat() {
    use stompymux_rs::*;
    let source = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
    let template =
        MechTemplate::parse("SCP-1N", include_str!("../game/units/SCP-1N.toml")).unwrap();
    let (_dir, config, mut world, id) = fixture_assets(&source, template.clone()).await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let target = world.create(&config, "Target quad".into(), Kind::Thing);
    world.objects.get_mut(&target).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(&mut world, target, template).unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, target, map, 5, 5).unwrap();
    set_battle_heading(&mut world, id, ObjectId(1), 180.0).unwrap();
    for _ in 0..40 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    let mut firing = world.clone();
    shot_seed(&mut firing, id, 3);
    shot_seed(&mut firing, target, 4);
    let _shot = resolve_battle_shot(&mut firing, id, ObjectId(1), target, 0, shot_rules()).unwrap();
    firing.validate(&config).unwrap();
    for leg in [Leg::Left, Leg::Right] {
        let mut candidate = world.clone();
        shot_seed(&mut candidate, id, 1);
        shot_seed(&mut candidate, target, 2);
        let report =
            resolve_battle_kick(&mut candidate, id, ObjectId(1), target, leg, kick_rules())
                .unwrap();
        assert_eq!(
            report.profile.attack.section(MechChassis::Quad),
            leg.section(MechChassis::Quad)
        );
        assert_eq!(
            candidate.btech.constructed_units()[&id].limb_recycle()
                [&leg.section(MechChassis::Quad)],
            60
        );
        candidate.validate(&config).unwrap();
        persistence::save(&config.database(), &candidate)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            candidate.btech
        );
    }
}

/// Quad club rejection is shared by direct, native and Lua actions and preserves world state.
#[tokio::test]
async fn quad_club_rejection_is_atomic_across_command_paths() {
    use stompymux_rs::*;
    let source = format!("12 12\n{}", format!("{}\n", "`0".repeat(12)).repeat(12));
    let template =
        MechTemplate::parse("SCP-1N", include_str!("../game/units/SCP-1N.toml")).unwrap();
    let (_dir, config, mut world, id) = fixture_assets(&source, template).await;
    let before = world.btech.clone();
    for selection in [None, Some("left"), Some("right")] {
        assert_eq!(
            grab_battle_club(&mut world, id, ObjectId(1), selection)
                .unwrap_err()
                .to_string(),
            "Quads can't carry a club."
        );
        assert_eq!(world.btech, before);
    }
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "grabclub left")
            .contains("Quads can't carry a club.")
    );
    assert_eq!(scripts.world().btech, before);
    let error = scripts
        .eval_callback::<()>(&format!("btech.unit.grabclub({},1,'right')", id.0))
        .unwrap_err();
    assert!(
        error.to_string().contains("Quads can't carry a club."),
        "{error}"
    );
    assert_eq!(scripts.world().btech, before);
    scripts.world().validate(&config).unwrap();
}

/// Quad dump selectors resolve actual front-leg bins and reject slots outside leg capacity atomically.
#[tokio::test]
async fn quad_dump_location_selectors_use_live_anatomy() {
    use stompymux_rs::*;
    let source = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
    let mut template =
        MechTemplate::parse("SCP-1N", include_str!("../game/units/SCP-1N.toml")).unwrap();
    let bin = template
        .sections
        .get_mut(&MechSection::LeftTorso)
        .unwrap()
        .criticals
        .remove(&0)
        .unwrap();
    template
        .sections
        .get_mut(&MechSection::LeftArm)
        .unwrap()
        .criticals
        .insert(5, bin);
    let (_dir, config, world, id) = fixture_assets(&source, template).await;
    for name in ["FLL", "FLLEG", "Front_Left_Leg"] {
        let mut candidate = world.clone();
        begin_battle_dump(&mut candidate, id, ObjectId(1), &format!("{name} 6")).unwrap();
        assert_eq!(
            candidate.btech.constructed_units()[&id]
                .dumping()
                .unwrap()
                .selection,
            DumpSelection::Slot(CriticalLocation {
                section: MechSection::LeftArm,
                slot: 5
            })
        );
        candidate.validate(&config).unwrap();
    }
    for invalid in ["FLL 7", "Left_Arm 6", "RLL 6"] {
        let mut candidate = world.clone();
        assert!(begin_battle_dump(&mut candidate, id, ObjectId(1), invalid).is_err());
        assert_eq!(candidate.btech, world.btech);
    }
}

/// Quad pod inspection is available, while all swatting paths reject without consuming state.
#[tokio::test]
async fn quad_pods_inspect_anatomy_and_reject_swatting() {
    use stompymux_rs::*;
    let source = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
    let template =
        MechTemplate::parse("SCP-1N", include_str!("../game/units/SCP-1N.toml")).unwrap();
    let (_dir, config, mut world, id) = fixture_assets(&source, template).await;
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["beacons"]["LeftArm"] = serde_json::to_value([BeaconKind::Homing]).unwrap();
        })
        .unwrap();
    world.validate(&config).unwrap();
    let before = world.btech.clone();
    let status = battle_pod_status(&world, id, ObjectId(1)).unwrap();
    assert!(status.contains("Front Left Leg"), "{status}");
    assert!(!status.contains("Left Arm"));
    assert_eq!(
        remove_battle_pod(
            &mut world,
            id,
            ObjectId(1),
            MechSection::LeftArm,
            BeaconKind::Homing,
            fall_rules()
        )
        .unwrap_err()
        .to_string(),
        "Quads can not knock of iNARC pods!"
    );
    assert_eq!(world.btech, before);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let text = support::run_text(&scripts, &config, ObjectId(1), 1, "removepod FLL H");
    assert!(
        text.contains("Quads can not knock of iNARC pods!"),
        "{text}"
    );
    assert_eq!(scripts.world().btech, before);
    let error = scripts
        .eval_callback::<()>(&format!(
            "btech.unit.removepod({},1,'Front_Left_Leg','H')",
            id.0
        ))
        .unwrap_err();
    assert!(
        error
            .to_string()
            .contains("Quads can not knock of iNARC pods!"),
        "{error}"
    );
    assert_eq!(scripts.world().btech, before);
}

/// A-Pods launch through normal combat, recycle without ammunition and preserve armor on hits.
#[tokio::test]
async fn apod_live_fire_and_restart() {
    use stompymux_rs::*;
    for weapon in [Weapon::APod, Weapon::ClanAPod] {
        let source = format!("12 12\n{}", format!("{}\n", ".0".repeat(12)).repeat(12));
        let mut template =
            MechTemplate::parse("SCP-1N", include_str!("../game/units/SCP-1N.toml")).unwrap();
        template
            .sections
            .get_mut(&MechSection::LeftArm)
            .unwrap()
            .criticals
            .insert(
                5,
                CriticalDefinition {
                    equipment: weapon.name().into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
        let (_dir, config, mut world, id) = fixture_assets(&source, template.clone()).await;
        let map = world.btech.constructed_units()[&id].position().unwrap().map;
        let target = world.create(&config, "A-Pod target".into(), Kind::Thing);
        world.objects.get_mut(&target).unwrap().home = Some(ObjectId(config.home()));
        create_battle_unit(&mut world, target, template).unwrap();
        support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
        place_battle_unit(&mut world, target, map, 5, 5).unwrap();
        set_battle_heading(&mut world, id, ObjectId(1), 180.0).unwrap();
        for _ in 0..40 {
            advance_battle_motion(&mut world, RULES).unwrap();
        }
        refresh_battle_contacts(&mut world, &[id]).unwrap();
        shot_skill(&mut world, 20);
        shot_seed(&mut world, id, 1);
        shot_seed(&mut world, target, 4);
        let index = world.btech.constructed_units()[&id]
            .loadout()
            .unwrap()
            .weapons
            .iter()
            .position(|mount| mount.weapon == weapon)
            .unwrap();
        let sections = world.btech.constructed_units()[&target].sections().clone();
        let shot =
            resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules()).unwrap();
        assert!(shot.launched);
        assert!(
            i32::from(shot.roll) >= shot.target_number.unwrap(),
            "{shot:?}"
        );
        assert_eq!(
            world.btech.constructed_units()[&target].sections(),
            &sections
        );
        assert_eq!(
            world.btech.constructed_units()[&id].weapon_recycle()[&index],
            30
        );
        let fired = world.btech.clone();
        assert!(
            resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules()).is_err()
        );
        assert_eq!(world.btech, fired);
        world.validate(&config).unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            world.btech
        );
    }
}

/// Stinger attacks reject ground and coordinate targets before consuming either unit's state.
#[tokio::test]
async fn stinger_airborne_admission_and_shot_replay() {
    use stompymux_rs::*;
    let template = MechTemplate::parse(
        "test",
        &include_str!("fixtures/btech/units/JR7-D.toml").replace("IS.SRM-4", "IS.LRM-5"),
    )
    .unwrap();
    let mut template = template;
    for section in template.sections.values_mut() {
        for part in section.criticals.values_mut() {
            if part.equipment == "IS.LRM-5" || part.equipment == "Ammo_IS.LRM-5" {
                part.modes = vec!["Stinger".into()];
                if part.equipment.starts_with("Ammo_") {
                    part.data = "24".into();
                }
            }
        }
    }
    let (_dir, config, mut world, id) = fixture_assets(
        &format!("12 12\n{}", (".0".repeat(12) + "\n").repeat(12)),
        template,
    )
    .await;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    let target = world.create(&config, "Jump target".into(), Kind::Thing);
    world.objects.get_mut(&target).unwrap().home = Some(ObjectId(config.home()));
    create_battle_unit(
        &mut world,
        target,
        MechTemplate::parse("JR7-D", include_str!("fixtures/btech/units/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, target, support::FIXTURE_DICE_SEED);
    place_battle_unit(&mut world, target, map, 5, 4).unwrap();
    let index = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::Lrm5)
        .unwrap();
    let before = world.btech.clone();
    assert!(
        resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules())
            .unwrap_err()
            .to_string()
            .contains("airborne")
    );
    assert!(
        battle_hex_aim_modifiers(&world, id, HexCoordinate { x: 5, y: 4 }, index, 6)
            .unwrap_err()
            .to_string()
            .contains("cannot shoot hexes")
    );
    assert_eq!(world.btech, before);
    world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
    assign_battle_pilot(&mut world, target, ObjectId(2)).unwrap();
    support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, target, ObjectId(2), false).unwrap();
    for _ in 0..30 {
        advance_battle_units(&mut world, 0, true);
    }
    launch_battle_jump(&mut world, target, ObjectId(2), 0, 2.0).unwrap();
    refresh_battle_contacts(&mut world, &[id]).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    let shot =
        resolve_battle_shot(&mut world, id, ObjectId(1), target, index, shot_rules()).unwrap();
    assert!(shot.launched);
    assert_eq!(shot.expenditure.ammunition_mode, AmmunitionMode::Stinger);
    assert_eq!(shot.expenditure.ammunition[0].rounds, 1);
    assert_eq!(
        resolve_battle_shot(&mut loaded, id, ObjectId(1), target, index, shot_rules()).unwrap(),
        shot
    );
    assert_eq!(loaded.btech, world.btech);
}

/// Native and Lua Stinger selection share admission, mode replacement and rollback.
#[tokio::test]
async fn stinger_native_lua_selection_and_restart() {
    use stompymux_rs::*;
    let (_dir, config, mut world, shooter, _target, _tagger, index) = semiguided_fixture().await;
    let before = world.btech.clone();
    for (pilot, selected) in [(ObjectId(2), index), (ObjectId(1), 999)] {
        assert!(toggle_battle_stinger(&mut world, shooter, pilot, selected).is_err());
        assert_eq!(world.btech, before);
    }
    let laser = world.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::MediumLaser)
        .unwrap();
    assert!(toggle_battle_stinger(&mut world, shooter, ObjectId(1), laser).is_err());
    assert_eq!(world.btech, before);
    let mut one_shot = world.clone();
    one_shot
        .btech
        .rewrite_unit_record(shooter, |record| {
            record["definition"]["sections"]["CenterTorso"]["criticals"]["10"]["modes"] =
                serde_json::json!(["OneShot"]);
        })
        .unwrap();
    let checkpoint = one_shot.btech.clone();
    assert!(toggle_battle_stinger(&mut one_shot, shooter, ObjectId(1), index).is_err());
    assert_eq!(one_shot.btech, checkpoint);
    let mut recycling = world.clone();
    let _spent = spend_battle_weapon(&mut recycling, shooter, ObjectId(1), index).unwrap();
    let checkpoint = recycling.btech.clone();
    assert!(toggle_battle_stinger(&mut recycling, shooter, ObjectId(1), index).is_err());
    assert_eq!(recycling.btech, checkpoint);
    toggle_battle_semiguided(&mut world, shooter, ObjectId(1), index).unwrap();
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let text = support::run_text(
        &native,
        &config,
        ObjectId(1),
        1,
        &format!("stinger {index}"),
    );
    assert!(
        text.contains(&format!(
            "Weapon {index} has been set to fire stinger missiles."
        )),
        "{text}"
    );
    assert_eq!(
        lua.eval_callback::<String>(&format!(
            "return btech.unit.stinger({},1,{index})",
            shooter.0
        ))
        .unwrap(),
        "stinger"
    );
    assert_eq!(native.world().btech, lua.world().btech);
    assert_eq!(
        lua.world().btech.constructed_units()[&shooter]
            .ammunition_mode(index)
            .unwrap(),
        AmmunitionMode::Stinger
    );
    // A mode may be selected without matching stock; ordinary rounds must not substitute.
    assert!(
        !lua.world().btech.constructed_units()[&shooter]
            .weapon_readiness(index)
            .unwrap()
            .ready
    );
    let selected = lua.world().clone();
    lua.drain_outbox();
    assert!(
        lua.eval_callback::<()>(&format!(
            "btech.unit.stinger({},1,{index}); error('abort')",
            shooter.0
        ))
        .is_err()
    );
    assert_eq!(lua.world().btech, selected.btech);
    assert!(lua.drain_outbox().is_empty());
    persistence::save(&config.database(), &selected)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        selected.btech
    );
    assert_eq!(
        lua.eval_callback::<String>(&format!(
            "return btech.unit.stinger({},1,{index})",
            shooter.0
        ))
        .unwrap(),
        "normal"
    );
    assert!(
        lua.world().btech.constructed_units()[&shooter]
            .weapon_readiness(index)
            .unwrap()
            .ready
    );
}

/// Remove only the additional diagnostic to compare every other report field and random outcome.
fn without_hold_warnings(mut value: serde_json::Value) -> serde_json::Value {
    match &mut value {
        serde_json::Value::Array(values) => {
            values.retain(|v| {
                v.get("text").and_then(|v| v.as_str()) != Some("You are currently in weapons hold!")
            });
            for value in values {
                *value = without_hold_warnings(value.take());
            }
        }
        serde_json::Value::Object(values) => {
            // Removing cockpit warnings also shifts any private insertion positions.
            let removed: Vec<_> = values
                .get("notices")
                .and_then(|v| v.as_array())
                .into_iter()
                .flatten()
                .enumerate()
                .filter(|(_, notice)| {
                    notice.get("text").and_then(|v| v.as_str())
                        == Some("You are currently in weapons hold!")
                })
                .map(|(index, _)| index)
                .collect();
            if let Some(private) = values
                .get_mut("pilot_notices")
                .and_then(|v| v.as_array_mut())
            {
                for notice in private {
                    let index = notice["before_notice"].as_u64().unwrap() as usize;
                    notice["before_notice"] =
                        (index - removed.iter().filter(|removed| **removed < index).count()).into();
                }
            }
            for value in values.values_mut() {
                *value = without_hold_warnings(value.take());
            }
        }
        _ => {}
    }
    value
}

/// Hold warns once per physical damage entry and transfer, while preserving all damage and dice.
#[tokio::test]
async fn weapons_hold_physical_damage_warns_without_suppressing_transfers() {
    use stompymux_rs::*;
    for (transfer, explosion) in [(false, false), (true, false), (true, true)] {
        let (_dir, config, mut base, id, target) = kick_fixture().await;
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        shot_seed(&mut base, id, seed);
        shot_seed(&mut base, target, 0);
        if transfer {
            base.btech
                .rewrite_unit_record(target, |record| {
                    for section in ["LeftLeg", "RightLeg"] {
                        record["sections"][section]["armor"] = 0.into();
                        record["sections"][section]["internal"] = 1.into();
                    }
                    if !explosion {
                        for rounds in record["ammunition"].as_array_mut().unwrap() {
                            *rounds = 0.into();
                        }
                    }
                })
                .unwrap();
        }
        let mut ordinary = base.clone();
        let mut held = base.clone();
        set_battle_weapons_hold(&mut held, id, true).unwrap();
        persistence::save(&config.database(), &held).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        // Connection state is not persisted; the kick base is the connected pilot's skill.
        replay
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let expected = resolve_battle_kick(
            &mut ordinary,
            id,
            ObjectId(1),
            target,
            Leg::Left,
            kick_rules(),
        )
        .unwrap();
        let report =
            resolve_battle_kick(&mut held, id, ObjectId(1), target, Leg::Left, kick_rules())
                .unwrap();
        assert!(report.hit && report.impact.is_some());
        let warnings: Vec<_> = report
            .notices
            .iter()
            .filter(|n| n.text == "You are currently in weapons hold!")
            .collect();
        assert_eq!(
            warnings.len(),
            if explosion {
                5
            } else if transfer {
                2
            } else {
                1
            },
            "impact={:?}",
            report.impact
        );
        assert!(warnings.iter().all(|n| n.unit == id));
        if explosion {
            assert!(report.impact.as_ref().unwrap().impact.criticals.iter().any(|(_, loss)| matches!(loss, CriticalLoss::Ammunition { explosion_damage, .. } if *explosion_damage > 0)));
        }
        assert_eq!(
            report.impact.as_ref().unwrap().notices[0].text,
            "You are currently in weapons hold!"
        );
        assert_eq!(
            without_hold_warnings(serde_json::to_value(&report).unwrap()),
            serde_json::to_value(&expected).unwrap()
        );
        assert_eq!(
            resolve_battle_kick(
                &mut replay,
                id,
                ObjectId(1),
                target,
                Leg::Left,
                kick_rules()
            )
            .unwrap(),
            report
        );
        assert_eq!(held.btech, replay.btech);
        set_battle_weapons_hold(&mut held, id, false).unwrap();
        assert_eq!(held.btech, ordinary.btech);
        held.validate(&config).unwrap();
    }
}

/// Collision packets retain their attacker even outside character mode; self recoil never warns.
#[tokio::test]
async fn weapons_hold_charge_and_dfa_warn_only_for_target_packets() {
    use stompymux_rs::*;
    for dfa in [false, true] {
        let (_dir, config, mut base, id, target) = kick_fixture().await;
        if dfa {
            dfa_pilot(&mut base);
            launch_battle_jump(&mut base, id, ObjectId(1), 0, 2.0).unwrap();
        } else {
            prepare_test_charge(&mut base, id);
        }
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        shot_seed(&mut base, id, seed);
        shot_seed(&mut base, target, 0);
        let run = |world: &mut World| {
            if dfa {
                serde_json::to_value(resolve_battle_dfa(world, id, target, kick_rules()).unwrap())
                    .unwrap()
            } else {
                serde_json::to_value(
                    resolve_battle_charge(
                        world,
                        id,
                        target,
                        ChargeRules {
                            distance: 2.0,
                            physical: kick_rules(),
                        },
                    )
                    .unwrap(),
                )
                .unwrap()
            }
        };
        let mut ordinary = base.clone();
        let expected = run(&mut ordinary);
        set_battle_weapons_hold(&mut base, id, true).unwrap();
        let report = run(&mut base);
        assert!(report["hit"].as_bool().unwrap());
        let impacts = report["target_impacts"].as_array().unwrap();
        assert!(!impacts.is_empty());
        for impact in impacts {
            assert!(
                impact["notices"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|n| n["text"] == "You are currently in weapons hold!")
            );
        }
        for impact in report["attacker_impacts"].as_array().unwrap() {
            assert!(
                !impact["notices"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|n| n["text"] == "You are currently in weapons hold!")
            );
        }
        assert_eq!(without_hold_warnings(report), expected);
        set_battle_weapons_hold(&mut base, id, false).unwrap();
        assert_eq!(base.btech, ordinary.btech);
        base.validate(&config).unwrap();
    }
}

/// Native and Lua physical attacks publish hold feedback atomically in tactical and character modes.
#[tokio::test]
async fn weapons_hold_physical_feedback_shares_host_publication_and_rollback() {
    use stompymux_rs::*;
    for character in [false, true] {
        let (_dir, config, mut base, id, target) = kick_fixture().await;
        let seed = (0..=255)
            .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        shot_seed(&mut base, id, seed);
        shot_seed(&mut base, target, 0);
        set_battle_weapons_hold(&mut base, id, true).unwrap();
        if character {
            for unit in [id, target] {
                base.objects
                    .get_mut(&unit)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
            }
        }
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
        commands::run(
            &native,
            &config,
            ObjectId(1),
            1,
            &format!("kick right #{}", target.0),
        )
        .unwrap();
        lua.eval_callback::<mlua::Table>(&format!("return {call}"))
            .unwrap();
        let messages = |scripts: &Scripts| {
            scripts
                .drain_outbox()
                .into_iter()
                .map(|(id, text)| (id, text.source().to_owned()))
                .collect::<Vec<_>>()
        };
        let output = messages(&native);
        assert!(
            output.iter().any(|(pilot, text)| *pilot == ObjectId(1)
                && text.contains("You are currently in weapons hold!")),
            "{output:?}"
        );
        assert_eq!(output, messages(&lua));
        assert_eq!(native.world().btech, lua.world().btech);
        native.world().validate(&config).unwrap();
    }
}

/// Already-admitted direct energy and missile hits warn without repeating cockpit admission.
#[tokio::test]
async fn weapons_hold_direct_mech_impacts_retain_shooter_and_weapon_effects() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    shot_skill(&mut base, 10);
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    shot_seed(&mut base, id, seed);
    shot_seed(&mut base, target, 0);
    let missile = base.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon.profile().missiles > 0)
        .unwrap();
    for index in [0, missile] {
        let mut ordinary = base.clone();
        let expected =
            resolve_battle_shot(&mut ordinary, id, ObjectId(1), target, index, shot_rules())
                .unwrap();
        let mut held = base.clone();
        set_battle_weapons_hold(&mut held, id, true).unwrap();
        persistence::save(&config.database(), &held).await.unwrap();
        let mut replay = persistence::load(&config.database()).await.unwrap();
        // A restarted player reconnects before repeating the shot with the same skill.
        replay
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let report =
            resolve_battle_shot(&mut held, id, ObjectId(1), target, index, shot_rules()).unwrap();
        let notices = report.notices();
        assert!(
            notices
                .iter()
                .any(|n| n.unit == id && n.text == "You are currently in weapons hold!")
        );
        assert_eq!(
            without_hold_warnings(serde_json::to_value(&report).unwrap()),
            serde_json::to_value(&expected).unwrap()
        );
        assert_eq!(
            resolve_battle_shot(&mut replay, id, ObjectId(1), target, index, shot_rules()).unwrap(),
            report
        );
        assert_eq!(held.btech, replay.btech);
        set_battle_weapons_hold(&mut held, id, false).unwrap();
        assert_eq!(held.btech, ordinary.btech);
        held.validate(&config).unwrap();
    }
}

/// Prediction proposals and committed movement follow the same controls across terrain and chassis.
#[tokio::test]
async fn ground_proposals_match_live_trajectories_without_mutating_state() {
    use stompymux_rs::{HexCoordinate, propose_battle_mech_ground_motion};
    for source in [
        include_str!("../game/units/JR7-D.toml"),
        include_str!("../game/units/GOL-1H.toml"),
    ] {
        for terrain in ['.', '"', '%'] {
            let row = format!("{terrain}0").repeat(12);
            let map = format!("12 12\n{}", format!("{row}\n").repeat(12));
            let (_dir, _config, baseline, id) =
                fixture_assets(&map, MechTemplate::parse("test", source).unwrap()).await;
            for slowdown in 0..=2 {
                for fasa_turning in [false, true] {
                    for speed in [-10.75, 32.25] {
                        let mut world = baseline.clone();
                        set_battle_speed(&mut world, id, ObjectId(1), speed).unwrap();
                        set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
                        let rules = MovementRules {
                            slowdown,
                            fasa_turning,
                            ..RULES
                        };
                        for _ in 0..12 {
                            let unit = &world.btech.constructed_units()[&id];
                            let position = unit.position().unwrap();
                            let motion = unit.motion().unwrap();
                            let before = world.btech.clone();
                            let proposal = propose_battle_mech_ground_motion(
                                &world,
                                id,
                                motion,
                                HexCoordinate {
                                    x: i32::from(position.x),
                                    y: i32::from(position.y),
                                },
                                rules,
                            )
                            .unwrap();
                            assert_eq!(world.btech, before);
                            assert!(!proposal.immobilized);
                            let _ = advance_battle_motion(&mut world, rules).unwrap();
                            let actual = world.btech.constructed_units()[&id].motion().unwrap();
                            assert_eq!(
                                actual,
                                stompymux_rs::Motion {
                                    point: proposal.destination,
                                    ..proposal.motion
                                }
                            );
                        }
                    }
                }
            }
        }
    }
}

/// A prediction must reject invalid inputs without touching the live world's random or motion state.
#[tokio::test]
async fn ground_proposals_reject_invalid_coordinates_and_nonfinite_motion() {
    use stompymux_rs::{HexCoordinate, propose_battle_mech_ground_motion};
    let (_dir, _config, world, id) = fixture('.').await;
    let motion = world.btech.constructed_units()[&id].motion().unwrap();
    let before = world.btech.clone();
    assert!(
        propose_battle_mech_ground_motion(&world, id, motion, HexCoordinate { x: -1, y: 5 }, RULES)
            .is_err()
    );
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert!(
            propose_battle_mech_ground_motion(
                &world,
                id,
                stompymux_rs::Motion {
                    speed: bad,
                    ..motion
                },
                HexCoordinate { x: 5, y: 5 },
                RULES
            )
            .is_err()
        );
    }
    assert_eq!(world.btech, before);
}

/// Restoring hardened protection suppresses the next hit's balance check without healing impairment.
#[tokio::test]
async fn edited_hardened_protection_controls_live_hit_messages_and_balance() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = fixture('.').await;
    balance_skill(&mut world);
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["definition"]["attributes"]["specials"] = "HDGYRO".into();
        })
        .unwrap();
    let slots: Vec<_> = world.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .systems
        .iter()
        .filter(|part| part.system == System::Gyro)
        .map(|part| part.location)
        .collect();
    destroy_battle_critical(&mut world, id, slots[0]).unwrap();
    destroy_battle_critical(&mut world, id, slots[1]).unwrap();
    assert_eq!(world.btech.constructed_units()[&id].gyro_damage(), 1);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    set_battle_unit_field_action(&scripts, &config, ObjectId(1), id, "critstatus2", "-").unwrap();
    let base = scripts.world().clone();
    let mut found = false;
    for seed in 0..=255 {
        let mut candidate = base.clone();
        shot_seed(&mut candidate, id, seed);
        let report = resolve_battle_tactical_impact(
            &mut candidate,
            id,
            balance_hit(MechSection::CenterTorso, true),
            1,
            fall_rules(),
        )
        .unwrap();
        if report.impact.criticals.len() != 1
            || !matches!(
                report.impact.criticals[0].1,
                CriticalLoss::System {
                    system: System::Gyro
                }
            )
        {
            continue;
        }
        assert_eq!(candidate.btech.constructed_units()[&id].gyro_damage(), 1);
        assert!(
            report
                .notices
                .iter()
                .any(|notice| notice.text == "Your hardened gyro takes a hit!")
        );
        assert!(report.balance.iter().all(|balance| !matches!(
            balance.cause,
            BalanceCause::Critical {
                system: System::Gyro,
                ..
            }
        )));
        persistence::save(&config.database(), &candidate)
            .await
            .unwrap();
        assert_eq!(
            persistence::load(&config.database()).await.unwrap().btech,
            candidate.btech
        );
        found = true;
        break;
    }
    assert!(
        found,
        "fixture must exercise exactly one protected gyro critical"
    );
}

/// A DFA to-hit number starts from the attacker's Piloting Skill (Total Warfare p. 144), so the
/// unskilled fixture pilot needs a skill before a landing attack is admitted.
fn dfa_pilot(world: &mut stompymux_rs::World) {
    stompymux_rs::set_battle_character_value(
        world,
        ObjectId(1),
        "Piloting-Biped",
        stompymux_rs::CharacterValue {
            value: 5,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
}

/// Melee specialist has the reference exact-one and case-insensitive semantics in DFA planning.
#[tokio::test]
async fn melee_specialist_requires_exact_one_for_dfa() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    dfa_pilot(&mut base);
    launch_battle_jump(&mut base, id, ObjectId(1), 0, 2.0).unwrap();
    let ordinary = battle_dfa_profile(&base, id, target, kick_rules()).unwrap();
    for name in ["Melee_Specialist", "melee_specialist"] {
        for raw in [0, 1, 2, 255] {
            let mut world = base.clone();
            set_battle_character_value(
                &mut world,
                ObjectId(1),
                name,
                CharacterValue {
                    value: raw,
                    ..Default::default()
                },
            )
            .unwrap();
            let before = world.btech.clone();
            let profile = battle_dfa_profile(&world, id, target, kick_rules()).unwrap();
            assert_eq!(
                profile.inflicted_damage,
                ordinary.inflicted_damage + u16::from(raw == 1)
            );
            assert_eq!(
                profile.attacker_movement,
                if raw == 1 {
                    -1
                } else {
                    ordinary.attacker_movement
                }
            );
            assert_eq!(profile.received_damage, ordinary.received_damage);
            assert_eq!(world.btech, before);
            world.validate(&config).unwrap();
        }
    }
}

/// Configured fire preserves target-only critical-balance feedback through shot formatting.
#[tokio::test]
async fn firing_keeps_critical_balance_rolls_private_to_target_pilot() {
    use stompymux_rs::*;
    let (_dir, config, mut base, shooter, target) = shot_fixture().await;
    shot_skill(&mut base, 30);
    base.objects.get_mut(&ObjectId(2)).unwrap().location = Some(target);
    assign_battle_pilot(&mut base, target, ObjectId(2)).unwrap();
    support::seed_object_dice(&mut base, ObjectId(2), support::FIXTURE_DICE_SEED);
    base.btech
        .rewrite_unit_record(target, |record| {
            record["power"] = serde_json::json!({"state":"running"});
        })
        .unwrap();
    for section in MechSection::ALL {
        let armor = base.btech.constructed_units()[&target].sections()[&section].armor;
        apply_damage_phase(
            &mut base,
            target,
            section,
            armor,
            DamagePhase::Armor { rear: false },
        )
        .unwrap();
    }
    for seed in 0..=255 {
        let mut world = base.clone();
        shot_seed(&mut world, target, seed);
        let before = world.clone();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        commands::run(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("fire 0 #{}", target.0),
        )
        .unwrap();
        let output = scripts.drain_outbox();
        let feedback: Vec<_> = output
            .iter()
            .filter(|(_, text)| text.source() == "You make a piloting skill roll!")
            .collect();
        if feedback.is_empty() {
            continue;
        }
        assert!(feedback.iter().all(|(who, _)| *who == ObjectId(2)));
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
        assert!(
            !output.iter().any(|(who, text)| *who != ObjectId(2)
                && text.source().starts_with("Modified Pilot Skill:"))
        );
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(before.clone())),
        )
        .unwrap();
        let call = format!("btech.unit.fire({},1,0,{})", shooter.0, target.0);
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort critical shot')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, before.btech);
        assert!(lua.drain_outbox().is_empty());
        lua.eval_callback::<()>(&call).unwrap();
        assert_eq!(lua.world().btech, scripts.world().btech);
        let actual: Vec<_> = lua
            .drain_outbox()
            .into_iter()
            .map(|(who, text)| (who, text.source().to_owned()))
            .collect();
        let expected: Vec<_> = output
            .into_iter()
            .map(|(who, text)| (who, text.source().to_owned()))
            .collect();
        assert_eq!(actual, expected);
        return;
    }
    panic!("No seeded shot exercised an actual critical balance roll");
}

/// A failed launch carries its internal-damage balance roll through the failure formatter.
#[tokio::test]
async fn rapid_misload_balance_feedback_stays_private() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    let mut definition = base.btech.constructed_units()[&id].definition().clone();
    let mut gun = definition.sections[&MechSection::LeftArm].criticals[&2].clone();
    gun.equipment = "IS.AC/2".into();
    gun.modes = vec!["RapidFire".into()];
    definition
        .sections
        .get_mut(&MechSection::LeftLeg)
        .unwrap()
        .criticals
        .insert(4, gun);
    let bin = definition
        .sections
        .values_mut()
        .flat_map(|section| section.criticals.values_mut())
        .find(|part| part.equipment.starts_with("Ammo_"))
        .unwrap();
    bin.equipment = "Ammo_IS.AC/2".into();
    bin.data = "45".into();
    base.btech
        .rewrite_unit_record(id, |record| {
            record["definition"] = serde_json::to_value(definition).unwrap();
            record["ammunition"][0] = 45.into();
        })
        .unwrap();
    let index = base.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == Weapon::Ac2)
        .unwrap();
    toggle_battle_rapid(&mut base, id, ObjectId(1), index).unwrap();
    base.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
    base.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    for n in 0u32..65536 {
        let mut seed = [0; 32];
        seed[..4].copy_from_slice(&n.to_le_bytes());
        if Dice::seeded(seed).two_d6() != 2 {
            continue;
        }
        let mut before = base.clone();
        before.btech.set_unit_dice(id, Dice::seeded(seed)).unwrap();
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(before.clone())),
        )
        .unwrap();
        let call = format!("btech.unit.fire({},1,{index},{})", id.0, target.0);
        scripts.eval_callback::<()>(&call).unwrap();
        let output = scripts.drain_outbox();
        let private: Vec<_> = output
            .iter()
            .filter(|(_, text)| text.source() == "You make a piloting skill roll!")
            .collect();
        if private.is_empty() {
            continue;
        }
        assert!(private.iter().all(|(who, _)| *who == ObjectId(1)));
        let pilot: Vec<_> = output
            .iter()
            .filter(|(who, _)| *who == ObjectId(1))
            .map(|(_, text)| text.source())
            .collect();
        let roll = pilot
            .iter()
            .position(|text| *text == "You make a piloting skill roll!")
            .unwrap();
        assert!(
            pilot[..roll]
                .iter()
                .any(|text| text.contains("explode in the barrel"))
        );
        assert!(pilot[roll + 1].starts_with("Modified Pilot Skill: BTH "));
        assert!(
            output.iter().any(|(who, text)| *who == ObjectId(2)
                && text.source().contains("explode in the barrel"))
        );
        assert!(
            !output.iter().any(|(who, text)| *who == ObjectId(2)
                && text.source().starts_with("Modified Pilot Skill:"))
        );
        *scripts.world_mut() = before.clone();
        assert!(
            scripts
                .eval_callback::<()>(&format!("{call}; error('abort misload')"))
                .is_err()
        );
        assert_eq!(scripts.world().btech, before.btech);
        assert!(scripts.drain_outbox().is_empty());
        return;
    }
    panic!("No misload exercised a leg balance check");
}

/// Both charge paths deliver each balance check to its own pilot, after collision warnings.
#[tokio::test]
async fn charge_control_feedback_is_private_for_single_and_mutual_collisions() {
    use stompymux_rs::*;
    for mutual in [false, true] {
        let (_dir, config, mut world, first, second) = kick_fixture().await;
        prepare_test_charge(&mut world, first);
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(second);
        assign_battle_pilot(&mut world, second, ObjectId(2)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(2), support::FIXTURE_DICE_SEED);
        for pilot in [ObjectId(1), ObjectId(2)] {
            world
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .insert(Flag::Connected);
        }
        world
            .btech
            .rewrite_unit_record(second, |record| {
                let unit = record;
                unit["power"] = serde_json::to_value(Power::Running).unwrap();
                unit["motion"]["speed"] = 21.5.into();
                unit["motion"]["desired_speed"] = 21.5.into();
                unit["motion"]["heading"] = 180.0.into();
                unit["motion"]["desired_heading"] = 180.0.into();
            })
            .unwrap();
        let seed = (0..=255)
            .find(|byte| Dice::seeded([*byte; 32]).two_d6() == 12)
            .unwrap();
        shot_seed(&mut world, first, seed);
        shot_seed(&mut world, second, seed);
        let passenger = world.create(&config, "Charge passenger".into(), Kind::Player);
        world.objects.get_mut(&passenger).unwrap().location = Some(first);
        world
            .objects
            .get_mut(&passenger)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let before = world.clone();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let rules = ChargeRules {
            distance: 2.0,
            physical: kick_rules(),
        };
        let collisions = if mutual {
            let report =
                resolve_battle_mutual_charge_action(&scripts, &config, first, second, rules, 2.0)
                    .unwrap();
            report
                .attempts
                .into_iter()
                .filter_map(|attempt| attempt.collision)
                .collect::<Vec<_>>()
        } else {
            vec![resolve_battle_charge_action(&scripts, &config, first, second, rules).unwrap()]
        };
        let output = scripts.drain_outbox();
        assert!(output.iter().any(|(who, _)| *who == passenger));
        assert!(!output.iter().any(|(who, message)| *who == passenger
            && (message.source().starts_with("Modified Pilot Skill:")
                || message.source() == "You make a piloting skill roll!")));
        let mut checked = 0;
        for balance in collisions.iter().flat_map(|collision| &collision.balance) {
            let Some(roll) = balance.check.roll else {
                continue;
            };
            let recipient = before.btech.constructed_units()[&balance.unit]
                .pilot()
                .unwrap();
            let expected = format!(
                "Modified Pilot Skill: BTH {}\tRoll: {roll}",
                balance.check.target
            );
            let messages = output
                .iter()
                .filter(|(who, _)| *who == recipient)
                .map(|(_, message)| message.source())
                .collect::<Vec<_>>();
            let index = messages
                .iter()
                .position(|message| *message == expected)
                .unwrap();
            assert!(index > 1);
            assert_eq!(messages[index - 1], "You make a piloting skill roll!");
            checked += 1;
        }
        assert!(checked > 0);
        assert!(output.iter().all(|(who, message)| {
            !message.source().starts_with("Modified Pilot Skill:")
                || [ObjectId(1), ObjectId(2)].contains(who)
        }));
    }
}

/// Failed pod swatting preserves self-damage fall feedback and the enclosing callback rollback.
#[tokio::test]
async fn pod_self_damage_preserves_private_fall_feedback() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, _) = shot_fixture().await;
    prepare_test_pod_removal(&mut world, id, MechSection::LeftLeg, BeaconKind::Ecm);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let passenger = world.create(&config, "Passenger".into(), Kind::Player);
    world.objects.get_mut(&passenger).unwrap().location = Some(id);
    world
        .objects
        .get_mut(&passenger)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["sections"]["LeftLeg"]["armor"] = 0.into();
            record["sections"]["LeftLeg"]["internal"] = 1.into();
        })
        .unwrap();
    let seed = (0..=255)
        .find(|seed| Dice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    shot_seed(&mut world, id, seed);
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let report = remove_battle_pod_action(
        &native,
        &config,
        id,
        ObjectId(1),
        MechSection::LeftLeg,
        BeaconKind::Ecm,
        FallRules::configured(&config),
    )
    .unwrap();
    assert!(!report.removed);
    assert!(
        !report.pilot_notices.is_empty(),
        "destroying the leg must cause a real protection roll"
    );
    let output = native.drain_outbox();
    let actual: Vec<_> = output
        .iter()
        .filter(|(_, message)| {
            message.source() == "You make a piloting skill roll!"
                || message.source().starts_with("Modified Pilot Skill:")
        })
        .map(|(who, message)| (*who, message.source().to_owned()))
        .collect();
    assert_eq!(
        actual,
        report
            .pilot_notices
            .iter()
            .map(|notice| (notice.pilot, notice.text.clone()))
            .collect::<Vec<_>>()
    );
    assert!(actual.iter().all(|(who, _)| *who == ObjectId(1)));
    assert!(output.iter().any(|(who, _)| *who == passenger));
    let warning = output
        .iter()
        .position(|(_, message)| message.source() == "Uh oh. You miss the pod and hit yourself!")
        .unwrap();
    let check = output
        .iter()
        .position(|(_, message)| message.source() == "You make a piloting skill roll!")
        .unwrap();
    assert!(warning < check);
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let call = format!("btech.unit.removepod({},1,'LL','E')", id.0);
    assert!(
        lua.eval_callback::<()>(&format!("{call}; error('abort')"))
            .is_err()
    );
    assert_eq!(lua.world().btech, world.btech);
    assert!(lua.drain_outbox().is_empty());
    lua.eval_callback::<()>(&call).unwrap();
    assert_eq!(lua.world().btech, native.world().btech);
    assert_eq!(lua.drain_outbox(), output);
}
