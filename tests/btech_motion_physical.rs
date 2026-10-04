//! Triple-strength myomer, handweapons, clubs, charges and death-from-above attacks, character
//! casualties from battle actions, and pilot experience awards.

use crate::btech_motion_common::{
    RULES, balance_hit, balance_skill, expected_grass_miss_rolls, fall_rules, fixture,
    fixture_source, kick_fixture, kick_rules, overheat_due, overheat_rules, prepare_test_charge,
    shot_fixture, shot_rules, shot_seed, shot_skill, single_critical_seed, stagger_fixture,
    stagger_hit, stagger_rules, stand_fixture, water_fixture, without_xp_timestamps,
};
use crate::support;
use crate::support::{install, restore_database, snapshot_database};
use stompymux_rs::ObjectId;

/// Add a complete passive installation without changing unit identity or active equipment.
fn install_test_myomer(world: &mut stompymux_rs::World, id: ObjectId) {
    use stompymux_rs::*;
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    for slot in 2..8 {
        definition
            .sections
            .get_mut(&BattleSection::LeftTorso)
            .unwrap()
            .criticals
            .insert(
                slot,
                CriticalDefinition {
                    equipment: "TripleStrengthMyomer".into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
    }
    state["constructed"][id.0.to_string()]["definition"] =
        serde_json::to_value(definition).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
}

/// Set stored and sampled heat independently, as required to test threshold transitions.
fn myomer_test_heat(world: &mut stompymux_rs::World, id: ObjectId, stored: f64, excess: f64) {
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["heat"] = serde_json::json!({"stored":stored,"excess":excess});
        })
        .unwrap();
}

/// TSM doubles base physical damage before actuator halving, and passive-slot loss does not disable it.
#[tokio::test]
async fn myomer_physical_damage_threshold_and_passive_loss() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    install_test_myomer(&mut base, id);
    for heat in [8.999, 9.0, 9.999, 10.0] {
        let mut world = base.clone();
        myomer_test_heat(&mut world, id, 40.0, heat);
        let active = heat >= 9.0;
        assert_eq!(
            world.btech.constructed_units()[&id].triple_myomer_active(),
            active
        );
        let kick = battle_kick_profile(
            &world,
            id,
            ObjectId(1),
            target,
            BattleLeg::Right,
            kick_rules(),
        )
        .unwrap();
        let punch = battle_punch_profile(
            &world,
            id,
            ObjectId(1),
            target,
            BattleArm::Right,
            kick_rules(),
        )
        .unwrap();
        let trip = battle_trip_profile(
            &world,
            id,
            ObjectId(1),
            target,
            BattleLeg::Right,
            kick_rules(),
        )
        .unwrap();
        assert_eq!(kick.damage, if active { 14 } else { 7 });
        assert_eq!(punch.damage, if active { 4 } else { 2 });
        assert_eq!(trip.damage, 0);
        for slot in 2..8 {
            destroy_battle_critical(
                &mut world,
                id,
                CriticalLocation {
                    section: BattleSection::LeftTorso,
                    slot,
                },
            )
            .unwrap();
        }
        assert_eq!(
            world.btech.constructed_units()[&id].triple_myomer_active(),
            active
        );
        assert_eq!(
            battle_kick_profile(
                &world,
                id,
                ObjectId(1),
                target,
                BattleLeg::Right,
                kick_rules()
            )
            .unwrap()
            .damage,
            kick.damage
        );
        for section in [BattleSection::LeftLeg, BattleSection::RightLeg] {
            destroy_battle_critical(&mut world, id, CriticalLocation { section, slot: 0 }).unwrap();
        }
        assert_eq!(
            world.btech.constructed_units()[&id].movement_maximum_speed(),
            0.0
        );
        world.validate(&config).unwrap();
    }
}

/// TSM has separate throttle, acceleration, turning, movement-heat and aim calculations.
#[tokio::test]
async fn myomer_movement_heat_and_native_lua_controls() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id) = fixture('.').await;
    install_test_myomer(&mut base, id);
    for (heat, maximum, steady) in [
        (8.999, 118.25, 96.75),
        (9.0, 129.0, 129.0),
        (10.0, 129.0, 129.0 * 118.25 / 150.5),
    ] {
        let mut world = base.clone();
        myomer_test_heat(&mut world, id, 30.0, heat);
        assert_eq!(
            world.btech.constructed_units()[&id].movement_maximum_speed(),
            maximum
        );
        let native = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        let lua = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        support::run_text(&native, &config, ObjectId(1), 1, "speed run");
        lua.eval_callback::<()>(&format!("btech.unit.speed({},1,{maximum})", id.0))
            .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        let mut world = native.world().clone();
        for _ in 0..20 {
            advance_battle_motion(&mut world, RULES).unwrap();
        }
        assert!(
            (world.btech.constructed_units()[&id].motion().unwrap().speed - steady).abs() < 1e-9
        );
        world.validate(&config).unwrap();
    }
    let mut world = base.clone();
    myomer_test_heat(&mut world, id, 30.0, 9.0);
    set_battle_heading(&mut world, id, ObjectId(1), 90.0).unwrap();
    advance_battle_motion(&mut world, RULES).unwrap();
    assert!(
        (world.btech.constructed_units()[&id]
            .motion()
            .unwrap()
            .heading
            - 20.25)
            .abs()
            < 1e-9
    );
    base.btech
        .edit_unit_motion(id, |motion| {
            motion.speed = 85.0;
            motion.desired_speed = 85.0;
        })
        .unwrap();
    myomer_test_heat(&mut base, id, 30.0, 9.0);
    assert_eq!(
        base.btech.constructed_units()[&id]
            .heat_rates(&base)
            .production,
        1.0
    );
    assert_eq!(
        base.btech.constructed_units()[&id].attacker_movement_modifier(false),
        1
    );
    myomer_test_heat(&mut base, id, 30.0, 8.999);
    assert_eq!(
        base.btech.constructed_units()[&id]
            .heat_rates(&base)
            .production,
        2.0
    );
}

/// Cooling preserves valid saved hot motion and then clamps the forward throttle on the next update.
#[tokio::test]
async fn myomer_cooling_restart_preserves_transition_motion() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = fixture('.').await;
    install_test_myomer(&mut world, id);
    myomer_test_heat(&mut world, id, 16.0, 9.0);
    set_battle_speed(&mut world, id, ObjectId(1), 129.0).unwrap();
    for _ in 0..20 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    advance_battle_heat(&mut world);
    assert!(!world.btech.constructed_units()[&id].triple_myomer_active());
    assert_eq!(
        world.btech.constructed_units()[&id].motion().unwrap().speed,
        129.0
    );
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    for _ in 0..5 {
        assert_eq!(
            advance_battle_motion(&mut world, RULES).unwrap(),
            advance_battle_motion(&mut loaded, RULES).unwrap()
        );
    }
    assert_eq!(world.btech, loaded.btech);
    assert_eq!(
        world.btech.constructed_units()[&id]
            .motion()
            .unwrap()
            .desired_speed,
        118.25
    );
    world.validate(&config).unwrap();
}

/// The reference clamps only forward throttle when myomer cools; reverse controls remain bounded by installed capability.
#[tokio::test]
async fn myomer_reverse_cooling_retains_throttle_without_invalidating_state() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = fixture('.').await;
    install_test_myomer(&mut world, id);
    myomer_test_heat(&mut world, id, 20.0, 9.0);
    set_battle_speed(&mut world, id, ObjectId(1), -86.0).unwrap();
    for _ in 0..20 {
        advance_battle_motion(&mut world, RULES).unwrap();
    }
    myomer_test_heat(&mut world, id, 0.0, 0.0);
    advance_battle_motion(&mut world, RULES).unwrap();
    let unit = &world.btech.constructed_units()[&id];
    assert_eq!(unit.movement_maximum_speed(), 118.25);
    assert_eq!(unit.motion().unwrap().speed, -86.0);
    assert_eq!(unit.motion().unwrap().desired_speed, -86.0);
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
}

/// Template equipment name that installs a hand weapon.
fn handweapon_template_name(kind: stompymux_rs::BattleArmAttack) -> &'static str {
    use stompymux_rs::BattleArmAttack as W;
    match kind {
        W::Axe => "Axe",
        W::Mace => "Mace",
        W::Sword => "Sword",
        W::Saw => "Dual_Saw",
        W::Claw => "Claw",
        W::RetractableBlade => "Retractable_Blade",
        W::Lance => "Lance",
        W::Flail => "Flail",
        W::WreckingBall => "Wrecking_Ball",
        W::ChainWhip => "Chain_Whip",
        W::SmallVibroblade => "Small_Vibroblade",
        W::MediumVibroblade => "Medium_Vibroblade",
        W::LargeVibroblade => "Large_Vibroblade",
        W::Punch => panic!("fixture needs installed equipment"),
    }
}

/// Equip both hands while preserving the fixture's identity and ammunition layout.
fn install_test_handweapons(
    world: &mut stompymux_rs::World,
    id: ObjectId,
    kind: stompymux_rs::BattleArmAttack,
) {
    use stompymux_rs::*;
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    let equipment = handweapon_template_name(kind);
    // A 35-ton mace fills four slots; the other hand weapons fill three.
    let last = if kind == BattleArmAttack::Mace { 7 } else { 6 };
    for section in [BattleSection::LeftArm, BattleSection::RightArm] {
        for (slot, name) in [(2, "LowerActuator"), (3, "HandOrFootActuator")]
            .into_iter()
            .chain((4..=last).map(|slot| (slot, equipment)))
        {
            definition
                .sections
                .get_mut(&section)
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
    }
    definition
        .sections
        .get_mut(&BattleSection::CenterTorso)
        .unwrap()
        .criticals
        .remove(&10);
    if kind == BattleArmAttack::Saw {
        for section in [BattleSection::LeftArm, BattleSection::RightArm] {
            let arm = definition.sections.get_mut(&section).unwrap();
            arm.criticals.retain(|slot, _| *slot < 4);
        }
        let arm = definition
            .sections
            .get_mut(&BattleSection::LeftArm)
            .unwrap();
        for slot in 4..=10 {
            arm.criticals.insert(
                slot,
                CriticalDefinition {
                    equipment: "Dual_Saw".into(),
                    data: "-".into(),
                    modes: vec![],
                },
            );
        }
    }
    world.btech.set_unit_definition(id, definition).unwrap();
}

/// Axes/swords retain damage with failed arm actuators, require hands and enough surviving parts, and have distinct mass.
#[tokio::test]
async fn handweapon_profiles_parts_mass_and_myomer() {
    use stompymux_rs::*;
    let (_dir, config, original, id, target) = kick_fixture().await;
    for (kind, damage, fixed_base, slots, slot_mass) in [
        (BattleArmAttack::Axe, 7, 4, 3, 1024),
        (BattleArmAttack::Mace, 9, 4, 4, 1024),
        (BattleArmAttack::Sword, 5, 3, 3, 682),
    ] {
        let mut base = original.clone();
        install_test_handweapons(&mut base, id, kind);
        let mass = base.btech.constructed_units()[&id]
            .mass()
            .unwrap()
            .equipment;
        let mut bare = base.clone();
        bare.btech
            .rewrite_unit_record(id, |record| {
                for section in ["LeftArm", "RightArm"] {
                    for slot in 4..4 + slots {
                        record["definition"]["sections"][section]["criticals"]
                            .as_object_mut()
                            .unwrap()
                            .remove(&slot.to_string());
                    }
                }
            })
            .unwrap();
        let before = bare.btech.clone();
        let error = resolve_battle_arm_attack(
            &mut bare,
            id,
            ObjectId(1),
            target,
            BattleArmSelection::Both,
            kind,
            kick_rules(),
        )
        .unwrap_err();
        assert!(error.to_string().contains("No usable"));
        assert_eq!(bare.btech, before);
        assert_eq!(
            mass - bare.btech.constructed_units()[&id]
                .mass()
                .unwrap()
                .equipment,
            2 * slots * slot_mass
        );
        assert!(
            base.btech.constructed_units()[&id]
                .critical_candidates(BattleSection::RightArm)
                .iter()
                .any(|location| location.slot == 4)
        );
        for hot in [false, true] {
            let mut world = base.clone();
            if hot {
                install_test_myomer(&mut world, id);
                myomer_test_heat(&mut world, id, 20.0, 9.0);
            }
            for count in 0..=2 {
                let profile = battle_arm_attack_profile(
                    &world,
                    id,
                    ObjectId(1),
                    target,
                    BattleArm::Right,
                    kind,
                    kick_rules(),
                )
                .unwrap();
                assert_eq!(profile.base, fixed_base);
                let weapon_modifier = if kind == BattleArmAttack::Mace { 2 } else { 0 };
                assert_eq!(profile.weapon_modifier, weapon_modifier);
                assert_eq!(
                    profile.target_number,
                    i32::from(fixed_base) + i32::from(count * 2) + i32::from(weapon_modifier) - 4
                );
                assert_eq!(profile.actuators, count * 2);
                assert_eq!(profile.damage, if hot { damage * 2 } else { damage });
                assert_eq!(profile.hit_table, BattleHitTable::Weapon);
                if count < 2 {
                    destroy_battle_critical(
                        &mut world,
                        id,
                        CriticalLocation {
                            section: BattleSection::RightArm,
                            slot: count + 1,
                        },
                    )
                    .unwrap();
                }
            }
            destroy_battle_critical(
                &mut world,
                id,
                CriticalLocation {
                    section: BattleSection::RightArm,
                    slot: 3,
                },
            )
            .unwrap();
            let before = world.btech.clone();
            assert!(
                resolve_battle_arm_attack(
                    &mut world,
                    id,
                    ObjectId(1),
                    target,
                    BattleArmSelection::Right,
                    kind,
                    kick_rules()
                )
                .is_err()
            );
            assert_eq!(world.btech, before);
            world.validate(&config).unwrap();
        }
        for mode in [
            BattleGlancingMode::AtTarget,
            BattleGlancingMode::BelowTarget,
        ] {
            let mut world = base.clone();
            let rules = BattlePhysicalRules {
                use_pilot_skill: true,
                glancing: mode,
                ..kick_rules()
            };
            let profile = battle_arm_attack_profile(
                &world,
                id,
                ObjectId(1),
                target,
                BattleArm::Right,
                kind,
                rules,
            )
            .unwrap();
            let threshold =
                profile.target_number - i32::from(mode == BattleGlancingMode::BelowTarget);
            let seed = (0..=255)
                .find(|seed| i32::from(BattleDice::seeded([*seed; 32]).two_d6()) == threshold)
                .unwrap();
            shot_seed(&mut world, id, seed);
            let report = resolve_battle_arm_attack(
                &mut world,
                id,
                ObjectId(1),
                target,
                BattleArmSelection::Right,
                kind,
                rules,
            )
            .unwrap();
            assert!(report.attacks[0].glancing);
            let phase = &report.attacks[0].impact.as_ref().unwrap().impact.phases[0];
            assert_eq!(phase.absorbed + phase.remaining, damage.div_ceil(2));
        }
        let mut world = base.clone();
        let location = CriticalLocation {
            section: BattleSection::RightArm,
            slot: 4,
        };
        let candidates = world.btech.constructed_units()[&id].critical_candidates(location.section);
        let selected = candidates
            .iter()
            .position(|candidate| *candidate == location)
            .unwrap();
        shot_seed(
            &mut world,
            id,
            single_critical_seed(candidates.len() as u16, (selected + 1) as u16),
        );
        let critical = resolve_battle_tactical_impact(
            &mut world,
            id,
            balance_hit(location.section, true),
            1,
            fall_rules(),
        )
        .unwrap();
        let name = match kind {
            BattleArmAttack::Axe => "axe",
            BattleArmAttack::Mace => "mace",
            BattleArmAttack::Sword => "sword",
            _ => unreachable!(),
        };
        assert!(
            critical.notices.iter().any(|notice| notice.unit == id
                && notice.text == format!("Your {name} has been destroyed!"))
        );
        // A hand weapon fills exactly the slots it needs, so one critical hit disables it.
        assert!(
            battle_arm_attack_profile(
                &world,
                id,
                ObjectId(1),
                target,
                BattleArm::Right,
                kind,
                kick_rules()
            )
            .is_err()
        );
        assert_eq!(
            world.btech.constructed_units()[&id]
                .mass()
                .unwrap()
                .equipment,
            mass
        );
        destroy_battle_critical(
            &mut world,
            id,
            CriticalLocation {
                section: BattleSection::RightArm,
                slot: 5,
            },
        )
        .unwrap();
        assert!(
            battle_arm_attack_profile(
                &world,
                id,
                ObjectId(1),
                target,
                BattleArm::Right,
                kind,
                kick_rules()
            )
            .is_err()
        );
        world.validate(&config).unwrap();
    }
}

/// A default two-arm weapon swing stops after its first accepted attack; hit, miss and callback rollback share this behavior.
#[tokio::test]
async fn handweapon_native_lua_recovery_and_restart() {
    use stompymux_rs::*;
    let (_dir, config, original, id, target) = kick_fixture().await;
    let native = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(original.clone())),
    )
    .unwrap();
    let lua = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(original.clone())),
    )
    .unwrap();
    let pristine_db = snapshot_database(&config);
    for (kind, command) in [
        (BattleArmAttack::Axe, "axe"),
        (BattleArmAttack::Mace, "mace"),
        (BattleArmAttack::Saw, "saw"),
        (BattleArmAttack::Claw, "claw"),
        (BattleArmAttack::Sword, "sword"),
    ] {
        for roll in [2, 12] {
            restore_database(&config, &pristine_db);
            let mut base = original.clone();
            install_test_handweapons(&mut base, id, kind);
            let seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
                .unwrap();
            shot_seed(&mut base, id, seed);
            shot_seed(&mut base, target, 19);
            install(&native, base.clone());
            install(&lua, base.clone());
            let call = format!("btech.unit.{command}({},1,'both',{})", id.0, target.0);
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
                &format!("melee both #{}", target.0),
            )
            .unwrap();
            let report: mlua::Table = lua.eval_callback(&format!("return {call}")).unwrap();
            let attacks: mlua::Table = report.get("attacks").unwrap();
            let count = if kind == BattleArmAttack::Claw { 2 } else { 1 };
            assert_eq!(attacks.raw_len(), count);
            let attack: mlua::Table = attacks.get(1).unwrap();
            assert_eq!(attack.get::<bool>("hit").unwrap(), roll == 12);
            assert_eq!(
                matches!(
                    attack.get::<mlua::Value>("balance").unwrap(),
                    mlua::Value::Table(_)
                ),
                kind == BattleArmAttack::Mace && roll == 2
            );
            assert_eq!(native.world().btech, lua.world().btech);
            let messages = |scripts: &Scripts| {
                scripts
                    .drain_outbox()
                    .into_iter()
                    .map(|(to, message)| (to, message.source().to_owned()))
                    .collect::<Vec<_>>()
            };
            let native_messages = messages(&native);
            assert_eq!(native_messages, messages(&lua));
            let mut world = lua.world().clone();
            assert_eq!(
                world.btech.constructed_units()[&id].limb_recycle().len(),
                count
            );
            assert_eq!(
                world.btech.constructed_units()[&id].limb_recycle()[&BattleSection::LeftArm],
                60
            );
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
    }
}

/// `melee` swings each arm's own installed weapon, pairs only claws, and rejects empty arms.
#[tokio::test]
async fn melee_swings_each_arms_installed_weapon() {
    use stompymux_rs::*;
    let (_dir, config, original, id, target) = kick_fixture().await;
    let mixed = |left: BattleArmAttack, right: &str| {
        let mut world = original.clone();
        install_test_handweapons(&mut world, id, left);
        let mut definition = world.btech.constructed_units()[&id].definition().clone();
        let arm = definition
            .sections
            .get_mut(&BattleSection::RightArm)
            .unwrap();
        for slot in 4..=6 {
            arm.criticals.get_mut(&slot).unwrap().equipment = right.into();
        }
        world.btech.set_unit_definition(id, definition).unwrap();
        world
    };
    let attack = |world: &World, arms| {
        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
        )
        .unwrap();
        resolve_battle_arm_attack_action(
            &scripts,
            &config,
            id,
            ObjectId(1),
            target,
            BattleArmAttackChoice {
                arms,
                kind: BattleArmWeapon::Installed,
            },
            kick_rules(),
        )
    };
    let swung = |report: &BattleArmAttackReport| {
        report
            .attacks
            .iter()
            .map(|attack| attack.profile.attack)
            .collect::<Vec<_>>()
    };
    let claw_axe = mixed(BattleArmAttack::Claw, "Axe");
    assert_eq!(
        swung(&attack(&claw_axe, BattleArmSelection::Right).unwrap()),
        [BattlePhysicalAttack::Weapon {
            arm: BattleArm::Right,
            weapon: BattleArmAttack::Axe
        }]
    );
    assert_eq!(
        swung(&attack(&claw_axe, BattleArmSelection::Left).unwrap()),
        [BattlePhysicalAttack::Weapon {
            arm: BattleArm::Left,
            weapon: BattleArmAttack::Claw
        }]
    );
    let both = attack(&claw_axe, BattleArmSelection::Both).unwrap();
    assert_eq!(
        swung(&both),
        [BattlePhysicalAttack::Weapon {
            arm: BattleArm::Left,
            weapon: BattleArmAttack::Claw
        }]
    );
    assert_eq!(both.rejections.len(), 1);
    assert_eq!(both.rejections[0].arm, BattleArm::Right);
    assert!(
        both.rejections[0]
            .reason
            .contains("Your limbs are still recovering")
    );
    let axe_claw = mixed(BattleArmAttack::Axe, "Claw");
    assert_eq!(
        swung(&attack(&axe_claw, BattleArmSelection::Both).unwrap()),
        [BattlePhysicalAttack::Weapon {
            arm: BattleArm::Left,
            weapon: BattleArmAttack::Axe
        }]
    );
    let claws = mixed(BattleArmAttack::Claw, "Claw");
    assert_eq!(
        swung(&attack(&claws, BattleArmSelection::Both).unwrap()),
        [
            BattlePhysicalAttack::Weapon {
                arm: BattleArm::Left,
                weapon: BattleArmAttack::Claw
            },
            BattlePhysicalAttack::Weapon {
                arm: BattleArm::Right,
                weapon: BattleArmAttack::Claw
            }
        ]
    );
    let bare = format!(
        "{:#}",
        attack(&original, BattleArmSelection::Left).unwrap_err()
    );
    assert!(
        bare.contains("No physical weapon installed in this arm"),
        "{bare}"
    );
    let bare = format!(
        "{:#}",
        attack(&original, BattleArmSelection::Both).unwrap_err()
    );
    assert!(bare.contains("No usable physical weapon"), "{bare}");
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(original.clone())),
    )
    .unwrap();
    assert!(
        support::run_text(&scripts, &config, ObjectId(1), 1, "melee/invalid")
            .contains("melee takes no switches")
    );
    assert!(
        support::run_text(
            &scripts,
            &config,
            ObjectId(1),
            1,
            &format!("melee left #{}", target.0)
        )
        .contains("No physical weapon installed in this arm")
    );
    assert_eq!(scripts.world().btech, original.btech);
}

/// Give a unit's right arm working lower and hand actuators, then `count` parts of one hand weapon.
fn install_right_arm_weapon(
    world: &mut stompymux_rs::World,
    id: ObjectId,
    kind: stompymux_rs::BattleArmAttack,
    count: u8,
) {
    use stompymux_rs::*;
    let mut definition = world.btech.constructed_units()[&id].definition().clone();
    let arm = definition
        .sections
        .get_mut(&BattleSection::RightArm)
        .unwrap();
    arm.criticals.retain(|slot, _| *slot < 2);
    let equipment = handweapon_template_name(kind);
    let parts = [(2, "LowerActuator"), (3, "HandOrFootActuator")]
        .into_iter()
        .chain((4..4 + count).map(|slot| (slot, equipment)));
    for (slot, name) in parts {
        arm.criticals.insert(
            slot,
            CriticalDefinition {
                equipment: name.into(),
                data: "-".into(),
                modes: vec![],
            },
        );
    }
    world.btech.set_unit_definition(id, definition).unwrap();
}

/// Each added 35-ton hand weapon has its own slots, aim, damage, myomer, hand and heat rules.
#[tokio::test]
async fn added_handweapon_profiles_slots_hands_and_heat() {
    use stompymux_rs::*;
    let (_dir, config, original, id, target) = kick_fixture().await;
    // (weapon, slots, damage, fixed base, weapon modifier, myomer doubles, heat)
    for (kind, slots, damage, fixed_base, modifier, doubles, heat) in [
        (BattleArmAttack::RetractableBlade, 3, 4, 3, 0, true, 0.0),
        (BattleArmAttack::Lance, 2, 7, 4, 2, true, 0.0),
        (BattleArmAttack::Flail, 4, 9, 4, 1, false, 0.0),
        (BattleArmAttack::WreckingBall, 5, 8, 4, 2, false, 0.0),
        (BattleArmAttack::ChainWhip, 2, 3, 3, 0, false, 0.0),
        (BattleArmAttack::SmallVibroblade, 1, 7, 3, 0, false, 3.0),
        (BattleArmAttack::MediumVibroblade, 2, 10, 3, 0, false, 5.0),
        (BattleArmAttack::LargeVibroblade, 4, 14, 3, 0, false, 7.0),
    ] {
        let profile = |world: &World| {
            battle_arm_attack_profile(
                world,
                id,
                ObjectId(1),
                target,
                BattleArm::Right,
                kind,
                kick_rules(),
            )
        };
        let mut short = original.clone();
        install_right_arm_weapon(&mut short, id, kind, slots - 1);
        let error = format!("{:#}", profile(&short).unwrap_err());
        assert!(error.contains("No usable"), "{kind:?}: {error}");

        let mut base = original.clone();
        install_right_arm_weapon(&mut base, id, kind, slots);
        base.validate(&config).unwrap();
        let armed = profile(&base).unwrap();
        assert_eq!(
            armed.attack,
            BattlePhysicalAttack::Weapon {
                arm: BattleArm::Right,
                weapon: kind
            }
        );
        assert_eq!(armed.base, fixed_base, "{kind:?}");
        assert_eq!(armed.weapon_modifier, modifier, "{kind:?}");
        assert_eq!(armed.damage, damage, "{kind:?}");
        assert_eq!(armed.hit_table, BattleHitTable::Weapon);
        assert_eq!(armed.fixed_location, None);

        let mut hot = base.clone();
        install_test_myomer(&mut hot, id);
        myomer_test_heat(&mut hot, id, 20.0, 9.0);
        assert_eq!(
            profile(&hot).unwrap().damage,
            if doubles { damage * 2 } else { damage },
            "{kind:?}"
        );

        let mut handless = base.clone();
        destroy_battle_critical(
            &mut handless,
            id,
            CriticalLocation {
                section: BattleSection::RightArm,
                slot: 3,
            },
        )
        .unwrap();
        assert_eq!(profile(&handless).is_ok(), !kind.needs_hand(), "{kind:?}");

        let scripts = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(base.clone())),
        )
        .unwrap();
        let before = scripts.world().btech.constructed_units()[&id].heat().stored;
        let report = resolve_battle_arm_attack_action(
            &scripts,
            &config,
            id,
            ObjectId(1),
            target,
            BattleArmAttackChoice {
                arms: BattleArmSelection::Both,
                kind: BattleArmWeapon::Installed,
            },
            kick_rules(),
        )
        .unwrap();
        assert_eq!(report.attacks.len(), 1);
        assert_eq!(report.attacks[0].profile.attack, armed.attack);
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].heat().stored - before,
            heat,
            "{kind:?}"
        );
    }
}

/// Flails and wrecking balls strike their wielder on a natural 2; wrecking-ball hits unbalance targets.
#[tokio::test]
async fn flail_and_wrecking_ball_fumbles_and_knockdown() {
    use stompymux_rs::*;
    let (_dir, config, original, id, target) = kick_fixture().await;
    let seeded = |roll| {
        (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
            .unwrap()
    };
    for (kind, slots) in [
        (BattleArmAttack::Flail, 4),
        (BattleArmAttack::WreckingBall, 5),
    ] {
        let mut world = original.clone();
        install_right_arm_weapon(&mut world, id, kind, slots);
        shot_seed(&mut world, id, seeded(2));
        let report = resolve_battle_arm_attack(
            &mut world,
            id,
            ObjectId(1),
            target,
            BattleArmSelection::Right,
            kind,
            kick_rules(),
        )
        .unwrap();
        let attack = &report.attacks[0];
        assert_eq!(attack.roll, 2);
        assert!(!attack.hit);
        assert!(attack.impact.is_none());
        let phase = &attack.fumble.as_ref().unwrap().impact.phases[0];
        let expected = if kind == BattleArmAttack::Flail { 5 } else { 4 };
        assert_eq!(phase.absorbed + phase.remaining, expected, "{kind:?}");
        assert!(attack.balance.is_some());
        assert!(
            report
                .notices
                .iter()
                .any(|notice| notice.unit == id && notice.text.contains("slams into you"))
        );
        world.validate(&config).unwrap();
    }

    let mut world = original.clone();
    install_right_arm_weapon(&mut world, id, BattleArmAttack::WreckingBall, 5);
    shot_seed(&mut world, id, seeded(12));
    let report = resolve_battle_arm_attack(
        &mut world,
        id,
        ObjectId(1),
        target,
        BattleArmSelection::Right,
        BattleArmAttack::WreckingBall,
        kick_rules(),
    )
    .unwrap();
    let attack = &report.attacks[0];
    assert!(attack.hit);
    assert!(attack.fumble.is_none());
    assert!(attack.impact.is_some());
    assert_eq!(attack.balance.as_ref().unwrap().situational, 2);

    let mut world = original.clone();
    install_right_arm_weapon(&mut world, id, BattleArmAttack::Flail, 4);
    shot_seed(&mut world, id, seeded(12));
    let report = resolve_battle_arm_attack(
        &mut world,
        id,
        ObjectId(1),
        target,
        BattleArmSelection::Right,
        BattleArmAttack::Flail,
        kick_rules(),
    )
    .unwrap();
    assert!(report.attacks[0].hit);
    assert!(report.attacks[0].balance.is_none());
}

/// A lance drives one point through remaining armor on 10+, rolling that point's critical at -2;
/// once the struck location's armor is gone, no penetration check is made.
#[tokio::test]
async fn lance_penetrates_remaining_armor_with_reduced_criticals() {
    use stompymux_rs::*;
    let (_dir, config, original, id, target) = kick_fixture().await;
    let hit_seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    let mut armed = original.clone();
    install_right_arm_weapon(&mut armed, id, BattleArmAttack::Lance, 2);
    shot_seed(&mut armed, id, hit_seed);
    // Armor the target heavily enough that the lance's seven damage never breaches it.
    armed
        .btech
        .rewrite_unit_record(target, |record| {
            let unit = record;
            for name in unit["sections"]
                .as_object()
                .unwrap()
                .keys()
                .cloned()
                .collect::<Vec<_>>()
            {
                unit["sections"][&name]["armor"] = 30.into();
                unit["definition"]["sections"][&name]["armor"] = 30.into();
            }
        })
        .unwrap();
    let attack = |world: &mut World| {
        resolve_battle_arm_attack(
            world,
            id,
            ObjectId(1),
            target,
            BattleArmSelection::Right,
            BattleArmAttack::Lance,
            kick_rules(),
        )
        .unwrap()
        .attacks
        .remove(0)
    };
    let dice_state = |world: &World| {
        serde_json::to_value(&world.btech).unwrap()["constructed"][target.0.to_string()]["dice"]
            .clone()
    };
    let (mut misses, mut pierced, mut suppressed, mut breached) = (0, 0, 0, 0);
    for seed in 0..=255 {
        let mut world = armed.clone();
        shot_seed(&mut world, target, seed);
        let report = attack(&mut world);
        assert!(report.hit);
        let Some(penetration) = &report.penetration else {
            // Thin armor the lance stripped away gets no penetration check.
            let struck = report.impact.as_ref().unwrap().impact.phases[0].section;
            let state = &world.btech.constructed_units()[&target].sections()[&struck];
            assert!(state.armor == 0 || state.rear == 0, "seed {seed}");
            breached += 1;
            continue;
        };
        let Some(impact) = &penetration.impact else {
            assert!(penetration.roll < 10);
            misses += 1;
            continue;
        };
        assert!(penetration.roll >= 10);
        pierced += 1;
        let phase = &impact.impact.phases[0];
        assert_eq!(phase.absorbed + phase.remaining, 1, "seed {seed}");
        // Replay the target's stream. Its generic rolls run: hit location, damage entry, the
        // penetration check, the penetration's damage entry, then the raw critical roll.
        let finished = dice_state(&world);
        let mut dice = BattleDice::seeded([seed; 32]);
        let mut rolls = Vec::new();
        while serde_json::to_value(&dice).unwrap() != finished {
            rolls.push(dice.d6());
            assert!(rolls.len() < 64, "seed {seed}: replay lost the stream");
        }
        let pairs: Vec<u8> = rolls.chunks(2).map(|pair| pair.iter().sum()).collect();
        assert_eq!(pairs[2], penetration.roll, "seed {seed}");
        let raw = pairs[4];
        assert_eq!(
            impact.impact.criticals.is_empty(),
            raw.saturating_sub(2) < 8,
            "seed {seed}"
        );
        // Raw 8 or 9 would crit an ordinary internal hit; the -2 penalty suppresses it.
        if (8..10).contains(&raw) {
            suppressed += 1;
        }
    }
    assert!(
        misses > 0 && pierced > 0 && suppressed > 0,
        "{misses} {pierced} {suppressed} {breached}"
    );
    assert!(breached < misses + pierced);

    let mut stripped = armed.clone();
    stripped
        .btech
        .rewrite_unit_record(target, |record| {
            let sections = record["sections"].as_object_mut().unwrap();
            for section in sections.values_mut() {
                section["armor"] = 1.into();
                if section["rear"].as_u64().unwrap() > 0 {
                    section["rear"] = 1.into();
                }
            }
        })
        .unwrap();
    for seed in 0..32 {
        let mut world = stripped.clone();
        shot_seed(&mut world, target, seed);
        assert!(attack(&mut world).penetration.is_none());
    }
    let mut world = armed.clone();
    let mut fist = world.clone();
    install_right_arm_weapon(&mut fist, id, BattleArmAttack::Axe, 3);
    shot_seed(&mut world, target, 0);
    shot_seed(&mut fist, target, 0);
    assert!(attack(&mut world).penetration.is_some());
    let axe = resolve_battle_arm_attack(
        &mut fist,
        id,
        ObjectId(1),
        target,
        BattleArmSelection::Right,
        BattleArmAttack::Axe,
        kick_rules(),
    )
    .unwrap();
    assert!(axe.attacks[0].penetration.is_none());
    world.validate(&config).unwrap();
}

/// A weapon swing that names a punch is malformed and cannot reach resolution through the host action.
#[tokio::test]
async fn weapon_attack_rejects_punch() {
    use stompymux_rs::*;
    let (_dir, config, original, id, target) = kick_fixture().await;
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(original.clone())),
    )
    .unwrap();
    let error = resolve_battle_physical_attack_action(
        &scripts,
        &config,
        id,
        ObjectId(1),
        target,
        BattlePhysicalAttack::Weapon {
            arm: BattleArm::Right,
            weapon: BattleArmAttack::Punch,
        },
        kick_rules(),
    )
    .unwrap_err();
    assert!(format!("{error:#}").contains("A punch is not a weapon swing"));
    assert_eq!(scripts.world().btech, original.btech);
    assert!(scripts.drain_outbox().is_empty());
}

/// Installed hand weapons reach a lower standing target, and choose tables independently of arm arcs.
#[tokio::test]
async fn handweapon_elevation_tables_and_default_arm_selection() {
    use stompymux_rs::*;
    let (_dir, config, original, id, target) = kick_fixture().await;
    for kind in [
        BattleArmAttack::Axe,
        BattleArmAttack::Sword,
        BattleArmAttack::Mace,
    ] {
        let mut base = original.clone();
        install_test_handweapons(&mut base, id, kind);
        for (height, prone, expected) in [
            (-1, false, Some(BattleHitTable::Punch)),
            (0, false, Some(BattleHitTable::Weapon)),
            (1, false, Some(BattleHitTable::Kick)),
            (1, true, Some(BattleHitTable::Weapon)),
            (0, true, None),
        ] {
            let mut world = base.clone();
            world
                .btech
                .rewrite_unit_record(target, |record| {
                    record["ground_elevation"] = height.into();
                    record["posture"] = serde_json::to_value(if prone {
                        BattlePosture::Prone
                    } else {
                        BattlePosture::Standing
                    })
                    .unwrap();
                })
                .unwrap();
            assert_eq!(
                battle_arm_attack_profile(
                    &world,
                    id,
                    ObjectId(1),
                    target,
                    BattleArm::Right,
                    kind,
                    kick_rules()
                )
                .ok()
                .map(|profile| profile.hit_table),
                expected
            );
            world.validate(&config).unwrap();
        }
        // A left arm with too few usable parts is omitted from default selection.
        for slot in [4, 5] {
            destroy_battle_critical(
                &mut base,
                id,
                CriticalLocation {
                    section: BattleSection::LeftArm,
                    slot,
                },
            )
            .unwrap();
        }
        let report = resolve_battle_arm_attack(
            &mut base,
            id,
            ObjectId(1),
            target,
            BattleArmSelection::Both,
            kind,
            kick_rules(),
        )
        .unwrap();
        assert_eq!(report.attacks.len(), 1);
        assert!(report.rejections.is_empty());
        assert_eq!(
            report.attacks[0]
                .profile
                .attack
                .section(BattleMechChassis::Biped),
            BattleSection::RightArm
        );
        base.validate(&config).unwrap();
    }
}

/// Missed maces test attacker balance at +2; successful swings do not roll extra control dice.
#[tokio::test]
async fn mace_miss_balance_success_and_failure() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    base.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
    base.objects
        .get_mut(&ObjectId(2))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    install_test_handweapons(&mut base, id, BattleArmAttack::Mace);
    set_battle_character_value(
        &mut base,
        ObjectId(1),
        "Piloting-Biped",
        BattleCharacterValue {
            value: 5,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    base.btech
        .set_unit_power(target, BattlePower::Running)
        .unwrap();
    let rules = BattlePhysicalRules {
        use_pilot_skill: true,
        ..kick_rules()
    };
    for succeeds in [false, true] {
        let mut world = base.clone();
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = BattleDice::seeded([*seed; 32]);
                dice.two_d6() == 2 && (dice.two_d6() >= 8) == succeeds
            })
            .unwrap();
        let mut expected = BattleDice::seeded([seed; 32]);
        expected.two_d6();
        let balance_roll = expected.two_d6();
        shot_seed(&mut world, id, seed);
        let before = world.clone();
        let target_before = world.btech.constructed_units()[&target].clone();
        let report = resolve_battle_arm_attack(
            &mut world,
            id,
            ObjectId(1),
            target,
            BattleArmSelection::Right,
            BattleArmAttack::Mace,
            rules,
        )
        .unwrap();
        let attack = &report.attacks[0];
        assert!(!attack.hit);
        assert!(attack.impact.is_none());
        let balance = attack.balance.unwrap();
        assert_eq!(balance.situational, 2);
        assert_eq!(balance.target, 8);
        assert_eq!(balance.roll, Some(balance_roll));
        assert_eq!(balance.success, succeeds);
        assert_eq!(attack.fall.is_some(), !succeeds);
        assert_eq!(
            world.btech.constructed_units()[&id].posture() == BattlePosture::Prone,
            !succeeds
        );
        assert_eq!(world.btech.constructed_units()[&target], target_before);
        world.validate(&config).unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(before))).unwrap();
        let published = resolve_battle_arm_attack_action(
            &scripts,
            &config,
            id,
            ObjectId(1),
            target,
            BattleArmAttackChoice {
                arms: BattleArmSelection::Right,
                kind: BattleArmWeapon::Fixed(BattleArmAttack::Mace),
            },
            rules,
        )
        .unwrap();
        assert_eq!(published.pilot_notices, report.pilot_notices);
        let output = scripts.drain_outbox();
        let pilot: Vec<_> = output
            .iter()
            .filter(|(who, _)| *who == ObjectId(1))
            .map(|(_, text)| text.source())
            .collect();
        let warning = pilot
            .iter()
            .position(|text| *text == "You miss and try to remain standing!")
            .unwrap();
        assert_eq!(pilot[warning + 1], "You make a piloting skill roll!");
        assert_eq!(
            pilot[warning + 2],
            format!("Modified Pilot Skill: BTH 8\tRoll: {balance_roll}")
        );
        assert!(!output.iter().any(|(who, text)| *who != ObjectId(1)
            && (text.source().starts_with("You make a piloting")
                || text.source().starts_with("Modified Pilot Skill:"))));
        assert!(output.iter().any(|(who, text)| *who == ObjectId(2)
            && text.source() == "You miss and try to remain standing!"));
    }
}

/// Saw mechanics retain fixed damage and location, need seven parts, and work without a hand.
#[tokio::test]
async fn saw_parts_damage_and_fixed_location() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    install_test_handweapons(&mut base, id, BattleArmAttack::Saw);
    let rules = kick_rules();
    let profile = |world: &World| {
        battle_arm_attack_profile(
            world,
            id,
            ObjectId(1),
            target,
            BattleArm::Left,
            BattleArmAttack::Saw,
            rules,
        )
    };
    let initial_mass = base.btech.constructed_units()[&id]
        .mass()
        .unwrap()
        .equipment;
    let mut bare = base.clone();
    bare.btech
        .rewrite_unit_record(id, |record| {
            for slot in 4..=10 {
                record["definition"]["sections"]["LeftArm"]["criticals"]
                    .as_object_mut()
                    .unwrap()
                    .remove(&slot.to_string());
            }
        })
        .unwrap();
    assert_eq!(
        initial_mass
            - bare.btech.constructed_units()[&id]
                .mass()
                .unwrap()
                .equipment,
        7 * 1024
    );
    assert!(profile(&bare).is_err());
    for hot in [false, true] {
        let mut world = base.clone();
        if hot {
            install_test_myomer(&mut world, id);
            myomer_test_heat(&mut world, id, 20.0, 9.0);
        }
        for slot in [1, 2, 3] {
            destroy_battle_critical(
                &mut world,
                id,
                CriticalLocation {
                    section: BattleSection::LeftArm,
                    slot,
                },
            )
            .unwrap();
        }
        let inspected = profile(&world).unwrap();
        assert_eq!(inspected.damage, 7);
        assert_eq!(inspected.actuators, 4);
        assert_eq!(inspected.weapon_modifier, 1);
        assert_eq!(inspected.target_number, 5);
        assert_eq!(inspected.fixed_location, Some(BattleSection::LeftArm));
        for mode in [
            BattleGlancingMode::Disabled,
            BattleGlancingMode::AtTarget,
            BattleGlancingMode::BelowTarget,
        ] {
            let mut attempt = world.clone();
            let roll = if mode == BattleGlancingMode::Disabled {
                12
            } else {
                5 - u8::from(mode == BattleGlancingMode::BelowTarget)
            };
            let seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
                .unwrap();
            shot_seed(&mut attempt, id, seed);
            let mut expected = attempt.clone();
            resolve_battle_tactical_impact(
                &mut expected,
                target,
                balance_hit(BattleSection::LeftArm, false),
                if mode == BattleGlancingMode::Disabled {
                    7
                } else {
                    4
                },
                rules.fall,
            )
            .unwrap();
            let target_dice = serde_json::to_value(&expected.btech.constructed_units()[&target])
                .unwrap()["dice"]
                .clone();
            let report = resolve_battle_arm_attack(
                &mut attempt,
                id,
                ObjectId(1),
                target,
                BattleArmSelection::Left,
                BattleArmAttack::Saw,
                BattlePhysicalRules {
                    glancing: mode,
                    ..rules
                },
            )
            .unwrap();
            let attack = &report.attacks[0];
            assert!(attack.hit);
            assert!(attack.balance.is_none());
            let phase = &attack.impact.as_ref().unwrap().impact.phases[0];
            assert_eq!(phase.section, BattleSection::LeftArm);
            assert_eq!(
                phase.absorbed + phase.remaining,
                if mode == BattleGlancingMode::Disabled {
                    7
                } else {
                    4
                }
            );
            assert_eq!(
                serde_json::to_value(&attempt.btech.constructed_units()[&target]).unwrap()["dice"],
                target_dice
            );
            attempt.validate(&config).unwrap();
        }
    }
    let location = CriticalLocation {
        section: BattleSection::LeftArm,
        slot: 4,
    };
    let candidates = base.btech.constructed_units()[&id].critical_candidates(location.section);
    let selected = candidates
        .iter()
        .position(|entry| *entry == location)
        .unwrap();
    shot_seed(
        &mut base,
        id,
        single_critical_seed(candidates.len() as u16, (selected + 1) as u16),
    );
    let critical = resolve_battle_tactical_impact(
        &mut base,
        id,
        balance_hit(location.section, true),
        1,
        fall_rules(),
    )
    .unwrap();
    assert!(
        critical
            .notices
            .iter()
            .any(|notice| notice.unit == id && notice.text == "Your dual saw has been destroyed!")
    );
    assert!(profile(&base).is_err());
    assert_eq!(
        base.btech.constructed_units()[&id]
            .mass()
            .unwrap()
            .equipment,
        initial_mass
    );
}

/// Claws keep tonnage damage despite actuator loss, share two-arm recovery, and apply fixed left-arm damage.
#[tokio::test]
async fn claw_parts_actuators_damage_and_dice() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    install_test_handweapons(&mut base, id, BattleArmAttack::Claw);
    let profile = |world: &World, rules| {
        battle_arm_attack_profile(
            world,
            id,
            ObjectId(1),
            target,
            BattleArm::Left,
            BattleArmAttack::Claw,
            rules,
        )
    };
    let initial_mass = base.btech.constructed_units()[&id]
        .mass()
        .unwrap()
        .equipment;
    let mut bare = base.clone();
    bare.btech
        .rewrite_unit_record(id, |record| {
            for section in ["LeftArm", "RightArm"] {
                for slot in 4..=6 {
                    record["definition"]["sections"][section]["criticals"]
                        .as_object_mut()
                        .unwrap()
                        .remove(&slot.to_string());
                }
            }
        })
        .unwrap();
    assert_eq!(
        initial_mass
            - bare.btech.constructed_units()[&id]
                .mass()
                .unwrap()
                .equipment,
        6 * 1024
    );
    assert!(profile(&bare, kick_rules()).is_err());
    assert_eq!(
        profile(
            &base,
            BattlePhysicalRules {
                use_pilot_skill: true,
                ..kick_rules()
            }
        )
        .unwrap()
        .base,
        11
    );
    for hot in [false, true] {
        let mut world = base.clone();
        if hot {
            install_test_myomer(&mut world, id);
            myomer_test_heat(&mut world, id, 20.0, 9.0);
        }
        for slot in 0..=3 {
            destroy_battle_critical(
                &mut world,
                id,
                CriticalLocation {
                    section: BattleSection::LeftArm,
                    slot,
                },
            )
            .unwrap();
        }
        let inspected = profile(&world, kick_rules()).unwrap();
        assert_eq!(inspected.base, 4);
        assert_eq!(inspected.weapon_modifier, 1);
        assert_eq!(inspected.actuators, 4);
        assert_eq!(inspected.target_number, 5);
        assert_eq!(inspected.damage, if hot { 10 } else { 5 });
        assert_eq!(inspected.fixed_location, Some(BattleSection::LeftArm));
        for mode in [
            BattleGlancingMode::Disabled,
            BattleGlancingMode::AtTarget,
            BattleGlancingMode::BelowTarget,
        ] {
            let mut attempt = world.clone();
            let roll = if mode == BattleGlancingMode::Disabled {
                12
            } else {
                5 - u8::from(mode == BattleGlancingMode::BelowTarget)
            };
            let seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
                .unwrap();
            shot_seed(&mut attempt, id, seed);
            let damage = if mode == BattleGlancingMode::Disabled {
                inspected.damage
            } else {
                inspected.damage.div_ceil(2)
            };
            let mut expected = attempt.clone();
            resolve_battle_tactical_impact(
                &mut expected,
                target,
                balance_hit(BattleSection::LeftArm, false),
                damage,
                fall_rules(),
            )
            .unwrap();
            let report = resolve_battle_arm_attack(
                &mut attempt,
                id,
                ObjectId(1),
                target,
                BattleArmSelection::Left,
                BattleArmAttack::Claw,
                BattlePhysicalRules {
                    glancing: mode,
                    ..kick_rules()
                },
            )
            .unwrap();
            let attack = &report.attacks[0];
            assert!(attack.hit);
            assert!(attack.balance.is_none());
            let phase = &attack.impact.as_ref().unwrap().impact.phases[0];
            assert_eq!(phase.section, BattleSection::LeftArm);
            assert_eq!(phase.absorbed + phase.remaining, damage);
            assert_eq!(
                attempt.btech.constructed_units()[&target],
                expected.btech.constructed_units()[&target]
            );
            attempt.validate(&config).unwrap();
        }
    }
    for slot in [4, 5] {
        destroy_battle_critical(
            &mut base,
            id,
            CriticalLocation {
                section: BattleSection::LeftArm,
                slot,
            },
        )
        .unwrap();
        // A claw needs every one of its slots: any critical hit puts it out of action.
        assert!(profile(&base, kick_rules()).is_err());
        assert_eq!(
            base.btech.constructed_units()[&id]
                .mass()
                .unwrap()
                .equipment,
            initial_mass
        );
    }
    let report = resolve_battle_arm_attack(
        &mut base,
        id,
        ObjectId(1),
        target,
        BattleArmSelection::Both,
        BattleArmAttack::Claw,
        kick_rules(),
    )
    .unwrap();
    assert_eq!(report.attacks.len(), 1);
    assert_eq!(
        report.attacks[0]
            .profile
            .attack
            .section(BattleMechChassis::Biped),
        BattleSection::RightArm
    );
    assert!(report.rejections.is_empty());
}

/// Give the fixture working hands and trees without installing a manufactured physical weapon.
fn prepare_test_club(world: &mut stompymux_rs::World, id: ObjectId) {
    use stompymux_rs::*;
    install_test_handweapons(world, id, BattleArmAttack::Axe);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    for section in ["LeftArm", "RightArm"] {
        for slot in 4..=6 {
            state["constructed"][id.0.to_string()]["definition"]["sections"][section]["criticals"]
                .as_object_mut()
                .unwrap()
                .remove(&slot.to_string());
        }
    }
    let position = world.btech.constructed_units()[&id].position().unwrap();
    let index = usize::from(position.y) * world.btech.maps()[&position.map].width as usize
        + usize::from(position.x);
    crate::support::set_hex_terrain(
        &mut state["maps"][position.map.0.to_string()]["terrain"][index],
        Terrain::LightForest,
    );
    world.btech = serde_json::from_value(state).unwrap();
}

/// Tree acquisition, punch restriction, explicit drop, critical loss and shutdown preserve owned state.
#[tokio::test]
async fn club_carry_lifecycle_and_guards() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = kick_fixture().await;
    let before = world.btech.clone();
    assert!(grab_battle_club(&mut world, id, ObjectId(1), None).is_err());
    assert_eq!(world.btech, before);
    prepare_test_club(&mut world, id);
    grab_battle_club(&mut world, id, ObjectId(1), None).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&id].carried_club(),
        Some(BattleArm::Left)
    );
    assert_eq!(
        world.btech.constructed_units()[&id].limb_recycle()[&BattleSection::LeftArm],
        60
    );
    for _ in 0..60 {
        advance_battle_recycle(&mut world);
    }
    assert!(
        battle_punch_profile(
            &world,
            id,
            ObjectId(1),
            target,
            BattleArm::Left,
            kick_rules()
        )
        .is_err()
    );
    assert!(
        battle_punch_profile(
            &world,
            id,
            ObjectId(1),
            target,
            BattleArm::Right,
            kick_rules()
        )
        .is_ok()
    );
    let base = world.clone();
    let mut outside = base.clone();
    let position = outside.btech.constructed_units()[&id].position().unwrap();
    let index = usize::from(position.y) * outside.btech.maps()[&position.map].width as usize
        + usize::from(position.x);
    let mut state = serde_json::to_value(&outside.btech).unwrap();
    crate::support::set_hex_terrain(
        &mut state["maps"][position.map.0.to_string()]["terrain"][index],
        Terrain::Grassland,
    );
    outside.btech = serde_json::from_value(state).unwrap();
    assert!(battle_club_profile(&outside, id, ObjectId(1), target, kick_rules()).is_ok());
    grab_battle_club(&mut outside, id, ObjectId(1), Some("-")).unwrap();
    let before = outside.btech.clone();
    assert!(resolve_battle_club(&mut outside, id, ObjectId(1), target, kick_rules()).is_err());
    assert_eq!(outside.btech, before);

    let notices = grab_battle_club(&mut world, id, ObjectId(1), Some("-")).unwrap();
    assert!(
        notices
            .iter()
            .any(|notice| notice.text.contains("shatters"))
    );
    assert!(
        world.btech.constructed_units()[&id]
            .carried_club()
            .is_none()
    );
    world = base.clone();
    destroy_battle_critical(
        &mut world,
        id,
        CriticalLocation {
            section: BattleSection::LeftArm,
            slot: 3,
        },
    )
    .unwrap();
    assert!(
        world.btech.constructed_units()[&id]
            .carried_club()
            .is_none()
    );
    world = base;
    let notices = stop_battle_unit(&mut world, id, ObjectId(1), fall_rules()).unwrap();
    assert!(
        notices
            .iter()
            .any(|notice| notice.text.contains("shatters"))
    );
    assert!(
        world.btech.constructed_units()[&id]
            .carried_club()
            .is_none()
    );
    world.validate(&config).unwrap();
}

/// Carried clubs shatter only on hits; both arms recycle, and failed actuators affect aim rather than damage.
#[tokio::test]
async fn club_attacks_damage_breakage_and_recovery() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    prepare_test_club(&mut base, id);
    for hot in [false, true] {
        let mut world = base.clone();
        if hot {
            install_test_myomer(&mut world, id);
            myomer_test_heat(&mut world, id, 20.0, 9.0);
        }
        for section in [BattleSection::LeftArm, BattleSection::RightArm] {
            for slot in [1, 2] {
                destroy_battle_critical(&mut world, id, CriticalLocation { section, slot })
                    .unwrap();
            }
        }
        let profile = battle_club_profile(&world, id, ObjectId(1), target, kick_rules()).unwrap();
        assert_eq!(profile.actuators, 8);
        assert_eq!(profile.damage, if hot { 14 } else { 7 });
        assert_eq!(profile.hit_table, BattleHitTable::Weapon);
    }
    grab_battle_club(&mut base, id, ObjectId(1), Some("right")).unwrap();
    for _ in 0..60 {
        advance_battle_recycle(&mut base);
    }
    for roll in [2, 12] {
        let mut world = base.clone();
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
            .unwrap();
        shot_seed(&mut world, id, seed);
        let report = resolve_battle_club(
            &mut world,
            id,
            ObjectId(1),
            target,
            BattlePhysicalRules {
                use_pilot_skill: true,
                ..kick_rules()
            },
        )
        .unwrap();
        assert_eq!(report.hit, roll == 12);
        assert!(report.balance.is_none());
        assert_eq!(
            world.btech.constructed_units()[&id]
                .carried_club()
                .is_none(),
            roll == 12
        );
        assert_eq!(world.btech.constructed_units()[&id].limb_recycle().len(), 2);
        world.validate(&config).unwrap();
    }
}

/// Native and Lua club actions share messages, complete rollback and persisted carried-tree recovery.
#[tokio::test]
async fn club_native_lua_rollback_and_saved_carry() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    prepare_test_club(&mut base, id);
    for grabbing in [true, false] {
        let mut world = base.clone();
        if !grabbing {
            grab_battle_club(&mut world, id, ObjectId(1), None).unwrap();
            for _ in 0..60 {
                advance_battle_recycle(&mut world);
            }
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
        let (command, call) = if grabbing {
            (
                "grabclub left".to_owned(),
                format!("btech.unit.grabclub({},1,'left')", id.0),
            )
        } else {
            (
                format!("club #{}", target.0),
                format!("btech.unit.club({},1,{})", id.0, target.0),
            )
        };
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, world.btech);
        assert!(lua.drain_outbox().is_empty());
        commands::run(&native, &config, ObjectId(1), 1, &command).unwrap();
        lua.eval_callback::<mlua::Table>(&format!("return {call}"))
            .unwrap();
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
}

/// Charge policy selects velocity or accumulated travel and current mass, without spending dice on rejection.
#[tokio::test]
async fn charge_collision_profiles_and_rejection() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    prepare_test_charge(&mut base, id);
    let mass = base.btech.constructed_units()[&id].mass().unwrap().total / 1024;
    let target_mass = base.btech.constructed_units()[&target]
        .mass()
        .unwrap()
        .total
        / 1024;
    for new_rules in [false, true] {
        for technology_level_three in [false, true] {
            let rules = BattleChargeRules {
                distance: 4.0,
                new_rules,
                technology_level_three,
                physical: kick_rules(),
            };
            let profile = battle_charge_profile(&base, id, target, rules).unwrap();
            let mp = if new_rules { 4 } else { 2 };
            assert_eq!(profile.inflicted_damage, (mp * (mass + 5) / 10 + 1) as u16);
            assert_eq!(
                profile.received_damage,
                if new_rules && technology_level_three {
                    (mp * (target_mass + 5) / 20) as u16
                } else {
                    ((target_mass + 5) / 10) as u16
                }
            );
            assert_eq!(profile.target_number, 2);
        }
    }
    let rules = BattleChargeRules {
        distance: 2.0,
        new_rules: false,
        technology_level_three: false,
        physical: kick_rules(),
    };
    for invalid in [f32::NAN, f32::INFINITY, -1.0] {
        let mut world = base.clone();
        assert!(
            resolve_battle_charge(
                &mut world,
                id,
                target,
                BattleChargeRules {
                    distance: invalid,
                    ..rules
                }
            )
            .is_err()
        );
        assert_eq!(world.btech, base.btech);
    }
    let mut stopped = base.clone();
    stopped
        .btech
        .edit_unit_motion(id, |motion| {
            motion.speed = 0.0;
        })
        .unwrap();
    let before = stopped.btech.clone();
    assert!(resolve_battle_charge(&mut stopped, id, target, rules).is_err());
    assert_eq!(stopped.btech, before);
    base.validate(&config).unwrap();
}

/// Hits apply ordered collision packets and two control checks; misses only consume attack dice and six recovery timers.
#[tokio::test]
async fn charge_collision_hit_miss_and_saved_recovery() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    prepare_test_charge(&mut base, id);
    // Keep target packet locations reproducible as well as the attacker's hit roll.
    shot_seed(&mut base, target, 0);
    // Use an operational target so a low roll remains a miss; immobile targets get -4 aim.
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][target.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    base.btech = serde_json::from_value(state).unwrap();
    for roll in [2, 12] {
        let mut world = base.clone();
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
            .unwrap();
        shot_seed(&mut world, id, seed);
        let before = world.btech.constructed_units()[&target].clone();
        let rules = BattleChargeRules {
            distance: 2.0,
            new_rules: false,
            technology_level_three: false,
            physical: kick_rules(),
        };
        let report = resolve_battle_charge(&mut world, id, target, rules).unwrap();
        assert_eq!(report.hit, roll == 12);
        assert_eq!(world.btech.constructed_units()[&id].limb_recycle().len(), 6);
        if roll == 2 {
            assert_eq!(report.received_damage, 0);
            assert!(report.recoil_arc.is_none());
            assert!(report.target_impacts.is_empty());
            assert!(report.attacker_impacts.is_empty());
            assert!(report.balance.is_empty());
            assert_eq!(world.btech.constructed_units()[&target], before);
            assert_eq!(
                world.btech.constructed_units()[&id].motion().unwrap().speed,
                21.5
            );
        } else {
            let sum = |impacts: &[BattleTacticalImpact]| {
                impacts
                    .iter()
                    .map(|impact| {
                        let phase = &impact.impact.phases[0];
                        assert!(phase.absorbed + phase.remaining <= 5);
                        phase.absorbed + phase.remaining
                    })
                    .sum::<u16>()
            };
            assert_eq!(sum(&report.target_impacts), report.profile.inflicted_damage);
            assert_eq!(sum(&report.attacker_impacts), report.received_damage);
            assert_eq!(report.balance.len(), 2);
            assert!(
                report
                    .balance
                    .iter()
                    .all(|balance| balance.check.situational == 2)
            );
            assert_eq!(
                world.btech.constructed_units()[&id].motion().unwrap().speed,
                0.0
            );
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        for _ in 0..60 {
            assert_eq!(
                advance_battle_recycle(&mut world),
                advance_battle_recycle(&mut loaded)
            );
        }
        assert_eq!(world.btech, loaded.btech);
        assert!(
            loaded.btech.constructed_units()[&id]
                .limb_recycle()
                .is_empty()
        );
    }
}

/// Target damage can stop its motion before level-three charge recoil is calculated.
#[tokio::test]
async fn charge_recoil_samples_motion_after_target_damage() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = kick_fixture().await;
    prepare_test_charge(&mut world, id);
    world
        .btech
        .rewrite_unit_record(target, |record| {
            let victim = record;
            victim["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            victim["motion"]["speed"] = 21.5.into();
            victim["motion"]["desired_speed"] = 21.5.into();
            victim["sections"]["RightLeg"]["armor"] = 0.into();
            victim["sections"]["RightLeg"]["internal"] = 1.into();
        })
        .unwrap();
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    shot_seed(&mut world, id, seed);
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 5)
        .unwrap();
    shot_seed(&mut world, target, seed);
    let mass = world.btech.constructed_units()[&target]
        .mass()
        .unwrap()
        .total
        / 1024;
    let rules = BattleChargeRules {
        distance: 4.0,
        new_rules: true,
        technology_level_three: true,
        physical: kick_rules(),
    };
    let forecast = battle_charge_profile(&world, id, target, rules).unwrap();
    let report = resolve_battle_charge(&mut world, id, target, rules).unwrap();
    assert!(report.hit);
    assert_eq!(report.profile, forecast);
    assert_eq!(
        report.target_impacts[0].impact.phases[0].section,
        BattleSection::RightLeg
    );
    assert_eq!(
        world.btech.constructed_units()[&target].sections()[&BattleSection::RightLeg].internal,
        0
    );
    assert_eq!(
        world.btech.constructed_units()[&target]
            .motion()
            .unwrap()
            .speed,
        0.0
    );
    assert!(report.received_damage > forecast.received_damage);
    assert_eq!(report.received_damage, (4 * (mass + 5) / 20) as u16);
    let applied = report
        .attacker_impacts
        .iter()
        .map(|impact| {
            let phase = &impact.impact.phases[0];
            phase.absorbed + phase.remaining
        })
        .sum::<u16>();
    assert_eq!(applied, report.received_damage);
    assert!(report.recoil_arc.is_some());
    world.validate(&config).unwrap();
}

/// Opposing biped charges preserve pre-rolled attempts, asymmetric damage and shared recovery.
#[tokio::test]
async fn mutual_charge_rolls_damage_and_saved_recovery() {
    use stompymux_rs::*;
    let (_dir, config, mut base, first, second) = kick_fixture().await;
    prepare_test_charge(&mut base, first);
    base.btech
        .rewrite_unit_record(second, |record| {
            let unit = record;
            unit["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            unit["motion"]["speed"] = 21.5.into();
            unit["motion"]["desired_speed"] = 21.5.into();
            unit["motion"]["heading"] = 180.0.into();
            unit["motion"]["desired_heading"] = 180.0.into();
        })
        .unwrap();
    let rules = BattleChargeRules {
        distance: 2.0,
        new_rules: false,
        technology_level_three: false,
        physical: kick_rules(),
    };
    for roll in [2, 12] {
        let mut world = base.clone();
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
            .unwrap();
        shot_seed(&mut world, first, seed);
        shot_seed(&mut world, second, seed);
        let report = resolve_battle_mutual_charge(&mut world, first, second, rules, 2.0).unwrap();
        for (index, id) in [first, second].into_iter().enumerate() {
            let attempt = &report.attempts[index];
            assert!(attempt.rejection.is_none());
            assert_eq!(attempt.roll, Some(roll));
            let collision = attempt.collision.as_ref().unwrap();
            assert_eq!(collision.hit, roll == 12);
            assert_eq!(world.btech.constructed_units()[&id].limb_recycle().len(), 6);
            if roll == 2 {
                assert!(collision.target_impacts.is_empty());
                let mut expected = BattleDice::seeded([seed; 32]);
                expected.two_d6();
                assert_eq!(
                    serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
                    serde_json::to_value(expected).unwrap()
                );
            } else {
                assert!(!collision.target_impacts.is_empty());
                assert_eq!(
                    world.btech.constructed_units()[&id].motion().unwrap().speed,
                    0.0
                );
            }
        }
        assert_eq!(report.notices[0].unit, first);
        assert_eq!(report.notices[1].unit, second);
        assert!(
            report.notices[..2]
                .iter()
                .all(|notice| notice.text.starts_with("Charge: BTH"))
        );
        if roll == 2 {
            assert_eq!(report.notices.len(), 2);
        }
        let first_damage = report.attempts[0]
            .collision
            .as_ref()
            .unwrap()
            .profile
            .inflicted_damage;
        let second_damage = report.attempts[1]
            .collision
            .as_ref()
            .unwrap()
            .profile
            .inflicted_damage;
        assert!(first_damage > second_damage);
        assert_eq!(
            second_damage,
            ((base.btech.constructed_units()[&second]
                .mass()
                .unwrap()
                .total
                / 1024
                + 5)
                / 10) as u16
        );
        persistence::save(&config.database(), &world).await.unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for _ in 0..60 {
            assert_eq!(
                advance_battle_recycle(&mut world),
                advance_battle_recycle(&mut restored)
            );
        }
        assert_eq!(world.btech, restored.btech);
    }
}

/// A rejected participant still consumes its roll and recycles when the other can attack; neither eligible consumes neither.
#[tokio::test]
async fn mutual_charge_rejections_and_torso_merge() {
    use stompymux_rs::*;
    let (_dir, config, mut base, first, second) = kick_fixture().await;
    prepare_test_charge(&mut base, first);
    let mut state = serde_json::to_value(&base.btech).unwrap();
    state["constructed"][first.0.to_string()]["facing"]["torso"] =
        serde_json::to_value(BattleTorso::Left).unwrap();
    state["constructed"][second.0.to_string()]["facing"]["torso"] =
        serde_json::to_value(BattleTorso::Right).unwrap();
    // Keep this target operational but stationary: the first attack misses and the second cannot charge.
    state["constructed"][second.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    base.btech = serde_json::from_value(state).unwrap();
    let rules = BattleChargeRules {
        distance: 2.0,
        new_rules: false,
        technology_level_three: false,
        physical: kick_rules(),
    };
    let mut world = base.clone();
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    shot_seed(&mut world, first, seed);
    shot_seed(&mut world, second, seed);
    let report = resolve_battle_mutual_charge(&mut world, first, second, rules, 0.0).unwrap();
    assert!(report.attempts[0].collision.is_some());
    assert!(report.attempts[1].rejection.is_some());
    assert!(report.attempts[1].collision.is_none());
    assert_eq!(report.attempts[1].roll, Some(2));
    assert_eq!(
        world.btech.constructed_units()[&second]
            .limb_recycle()
            .len(),
        6
    );
    assert_eq!(
        world.btech.constructed_units()[&first].facing().torso,
        BattleTorso::Both
    );
    assert_eq!(BattleTorso::Both.offset(), 59.0);
    for direction in [BattleTorso::Left, BattleTorso::Right, BattleTorso::Both] {
        let before = world.btech.clone();
        assert!(rotate_battle_torso(&mut world, first, ObjectId(1), direction).is_err());
        assert_eq!(world.btech, before);
    }
    rotate_battle_torso(&mut world, first, ObjectId(1), BattleTorso::Center).unwrap();
    assert_eq!(
        world.btech.constructed_units()[&first].facing().torso,
        BattleTorso::Center
    );
    world = base;
    world
        .btech
        .edit_unit_motion(first, |motion| {
            motion.speed = 0.0;
        })
        .unwrap();
    let before = serde_json::to_value(&world.btech).unwrap();
    let report = resolve_battle_mutual_charge(&mut world, first, second, rules, 0.0).unwrap();
    assert!(
        report
            .attempts
            .iter()
            .all(|attempt| attempt.roll.is_none() && attempt.rejection.is_some())
    );
    for id in [first, second] {
        let after = serde_json::to_value(&world.btech).unwrap();
        assert_eq!(
            before["constructed"][id.0.to_string()]["dice"],
            after["constructed"][id.0.to_string()]["dice"]
        );
        assert!(
            world.btech.constructed_units()[&id]
                .limb_recycle()
                .is_empty()
        );
    }
    assert_eq!(
        world.btech.constructed_units()[&first].facing().torso,
        BattleTorso::Both
    );
    world.validate(&config).unwrap();
}

/// The second mutual charge keeps its pre-impact roll even if the first charge destroys its leg.
#[tokio::test]
async fn mutual_charge_second_attack_survives_first_knockdown() {
    use stompymux_rs::*;
    let (_dir, config, mut world, first, second) = kick_fixture().await;
    prepare_test_charge(&mut world, first);
    world
        .btech
        .rewrite_unit_record(second, |record| {
            let unit = record;
            unit["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            unit["motion"]["speed"] = 21.5.into();
            unit["motion"]["desired_speed"] = 21.5.into();
            unit["motion"]["heading"] = 180.0.into();
            unit["motion"]["desired_heading"] = 180.0.into();
            unit["sections"]["RightLeg"]["armor"] = 0.into();
            unit["sections"]["RightLeg"]["internal"] = 1.into();
        })
        .unwrap();
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    shot_seed(&mut world, first, seed);
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            dice.two_d6() >= 10 && dice.two_d6() == 5
        })
        .unwrap();
    let expected_roll = BattleDice::seeded([seed; 32]).two_d6();
    shot_seed(&mut world, second, seed);
    let rules = BattleChargeRules {
        distance: 2.0,
        new_rules: false,
        technology_level_three: false,
        physical: kick_rules(),
    };
    let report = resolve_battle_mutual_charge(&mut world, first, second, rules, 2.0).unwrap();
    let first_hit = report.attempts[0].collision.as_ref().unwrap();
    assert!(first_hit.target_impacts.iter().any(|impact| {
        impact
            .impact
            .phases
            .iter()
            .any(|phase| phase.destroyed_sections.contains(&BattleSection::RightLeg))
    }));
    let second_hit = report.attempts[1].collision.as_ref().unwrap();
    assert_eq!(second_hit.roll, expected_roll);
    assert!(second_hit.hit);
    assert!(!second_hit.target_impacts.is_empty());
    assert_eq!(
        second_hit
            .balance
            .iter()
            .map(|balance| balance.unit)
            .collect::<Vec<_>>(),
        vec![first, second]
    );
    world.validate(&config).unwrap();
}

/// The second mutual recoil uses inflicted damage for its remainder, including level-three rules.
#[tokio::test]
async fn mutual_charge_second_recoil_remainder() {
    use stompymux_rs::*;
    let (_dir, _config, mut world, first, second) = kick_fixture().await;
    prepare_test_charge(&mut world, first);
    world
        .btech
        .rewrite_unit_record(second, |record| {
            let unit = record;
            unit["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            unit["motion"]["speed"] = 21.5.into();
            unit["motion"]["desired_speed"] = 21.5.into();
            unit["motion"]["heading"] = 180.0.into();
            unit["motion"]["desired_heading"] = 180.0.into();
        })
        .unwrap();
    for (id, roll) in [(first, 2), (second, 12)] {
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
            .unwrap();
        shot_seed(&mut world, id, seed);
    }
    let rules = BattleChargeRules {
        distance: 2.0,
        new_rules: true,
        technology_level_three: true,
        physical: kick_rules(),
    };
    let report = resolve_battle_mutual_charge(&mut world, first, second, rules, 8.0).unwrap();
    assert!(!report.attempts[0].collision.as_ref().unwrap().hit);
    let second_hit = report.attempts[1].collision.as_ref().unwrap();
    assert!(second_hit.hit);
    assert!(second_hit.received_damage >= 5);
    assert_eq!(
        second_hit.received_damage % 5,
        second_hit.profile.inflicted_damage % 5
    );
    assert_ne!(
        second_hit.received_damage,
        second_hit.profile.received_damage
    );
}

/// Native and Lua charge intent share rollback, messages, counters and persistence.
#[tokio::test]
async fn charge_tracking_commands_and_saved_intent() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    select_battle_target(&mut base, id, ObjectId(1), Some(target)).unwrap();
    base.btech
        .rewrite_unit_record(id, |record| {
            record["charge"] = serde_json::json!({"target":null,"elapsed":17,"distance":2.5});
        })
        .unwrap();
    for (command, argument, expected) in [
        ("charge".to_owned(), "nil".to_owned(), Some(target)),
        (
            format!("charge #{}", target.0),
            target.0.to_string(),
            Some(target),
        ),
        ("charge -".to_owned(), "'-'".to_owned(), None),
    ] {
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
        let call = format!("btech.unit.charge({},1,{argument})", id.0);
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, base.btech);
        assert!(lua.drain_outbox().is_empty());
        commands::run(&native, &config, ObjectId(1), 1, &command).unwrap();
        lua.eval_callback::<mlua::Table>(&format!("return {call}"))
            .unwrap();
        assert_eq!(native.world().btech, lua.world().btech);
        let messages = |scripts: &Scripts| {
            scripts
                .drain_outbox()
                .into_iter()
                .map(|(to, message)| (to, message.source().to_owned()))
                .collect::<Vec<_>>()
        };
        assert_eq!(messages(&native), messages(&lua));
        let world = lua.world().clone();
        let charge = world.btech.constructed_units()[&id].charge();
        assert_eq!(charge.target, expected);
        assert_eq!(charge.elapsed, if expected.is_some() { 17 } else { 0 });
        assert_eq!(charge.distance, if expected.is_some() { 2.5 } else { 0.0 });
        let before = serde_json::to_value(&base.btech.constructed_units()[&id]).unwrap();
        let after = serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap();
        assert_eq!(before["dice"], after["dice"]);
        persistence::save(&config.database(), &world).await.unwrap();
        let loaded = persistence::load(&config.database()).await.unwrap();
        assert_eq!(world.btech, loaded.btech);
    }
}

/// New-rule timeout counts moving updates, expires on update 61, and pauses at rest.
#[tokio::test]
async fn charge_tracking_timeout_and_distance() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = kick_fixture().await;
    select_battle_target(&mut world, id, ObjectId(1), Some(target)).unwrap();
    select_battle_charge(&mut world, id, ObjectId(1), BattleChargeSelection::Default).unwrap();
    let position = world.btech.constructed_units()[&target].position().unwrap();
    place_battle_unit(&mut world, target, position.map, 9, 9).unwrap();
    let rules = BattleMovementRules {
        charge: BattleChargePolicy {
            new_rules: true,
            ..BattleChargePolicy::STANDARD
        },
        ..RULES
    };
    for _ in 0..65 {
        advance_battle_motion(&mut world, rules).unwrap();
    }
    assert_eq!(world.btech.constructed_units()[&id].charge().elapsed, 0);
    prepare_test_charge(&mut world, id);
    for elapsed in 1..=60 {
        advance_battle_motion(&mut world, rules).unwrap();
        assert_eq!(
            world.btech.constructed_units()[&id].charge().elapsed,
            elapsed
        );
    }
    let state = world.btech.constructed_units()[&id].charge();
    assert!(state.distance > 1.9 && state.distance < 2.1, "{state:?}");
    let mut stopped = world.clone();
    stop_battle_unit(&mut stopped, id, ObjectId(1), fall_rules()).unwrap();
    assert_eq!(
        stopped.btech.constructed_units()[&id].charge(),
        BattleChargeState {
            target: None,
            ..state
        }
    );
    let notices = advance_battle_motion(&mut world, rules).unwrap();
    assert!(
        notices
            .iter()
            .any(|notice| notice.text.contains("timed out"))
    );
    assert_eq!(
        world.btech.constructed_units()[&id].charge(),
        BattleChargeState::default()
    );
    world.validate(&config).unwrap();
}

/// Endpoint collision consumes selection on hits and misses; restored state replays identically.
#[tokio::test]
async fn charge_tracking_collision_and_replay() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    prepare_test_charge(&mut base, id);
    select_battle_target(&mut base, id, ObjectId(1), Some(target)).unwrap();
    select_battle_charge(&mut base, id, ObjectId(1), BattleChargeSelection::Default).unwrap();
    for roll in [2, 12] {
        let mut world = base.clone();
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
            .unwrap();
        shot_seed(&mut world, id, seed);
        // Keep the target ahead of the integrated endpoint, inside collision range.
        let mut state = serde_json::to_value(&world.btech).unwrap();
        let point = world.btech.constructed_units()[&id].motion().unwrap().point;
        state["constructed"][target.0.to_string()]["motion"]["point"]["y"] = (point.y - 0.2).into();
        world.btech = serde_json::from_value(state).unwrap();
        let mut replay = world.clone();
        let notices = advance_battle_motion(&mut world, RULES).unwrap();
        assert_eq!(notices, advance_battle_motion(&mut replay, RULES).unwrap());
        assert_eq!(world.btech, replay.btech);
        assert!(
            notices.iter().any(|notice| notice.text.contains("BTH")),
            "{notices:?}"
        );
        assert_eq!(
            world.btech.constructed_units()[&id].charge(),
            BattleChargeState::default()
        );
        assert_eq!(world.btech.constructed_units()[&id].limb_recycle().len(), 6);
        world.validate(&config).unwrap();
    }
}

/// Mutual movement dispatch spends each attack roll once and clears both intents.
#[tokio::test]
async fn charge_tracking_mutual_dispatch() {
    use stompymux_rs::*;
    let (_dir, config, mut world, first, second) = kick_fixture().await;
    prepare_test_charge(&mut world, first);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let point = world.btech.constructed_units()[&first]
        .motion()
        .unwrap()
        .point;
    for (id, target) in [(first, second), (second, first)] {
        let unit = &mut state["constructed"][id.0.to_string()];
        unit["charge"] = serde_json::json!({"target":target.0,"elapsed":0,"distance":0.0});
        unit["power"] = serde_json::to_value(BattlePower::Running).unwrap();
        unit["motion"]["speed"] = 21.5.into();
        unit["motion"]["desired_speed"] = 21.5.into();
    }
    state["constructed"][second.0.to_string()]["motion"]["point"]["y"] = (point.y - 0.2).into();
    state["constructed"][second.0.to_string()]["motion"]["heading"] = 180.0.into();
    state["constructed"][second.0.to_string()]["motion"]["desired_heading"] = 180.0.into();
    world.btech = serde_json::from_value(state).unwrap();
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    for id in [first, second] {
        shot_seed(&mut world, id, seed);
    }
    let notices = advance_battle_motion(&mut world, RULES).unwrap();
    assert_eq!(
        notices
            .iter()
            .filter(|notice| notice.text.contains("BTH"))
            .count(),
        2,
        "{notices:?}"
    );
    for id in [first, second] {
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(unit.charge(), BattleChargeState::default());
        assert_eq!(unit.limb_recycle().len(), 6);
        let mut dice = BattleDice::seeded([seed; 32]);
        dice.two_d6();
        assert_eq!(
            serde_json::to_value(unit).unwrap()["dice"],
            serde_json::to_value(dice).unwrap()
        );
    }
    world.validate(&config).unwrap();
}

/// Turning without travel ages new-rule intent without attempting a zero-speed collision.
#[tokio::test]
async fn charge_tracking_turning_and_old_rules() {
    use stompymux_rs::*;
    let (_dir, _config, mut base, id, target) = kick_fixture().await;
    select_battle_charge(
        &mut base,
        id,
        ObjectId(1),
        BattleChargeSelection::Target(target),
    )
    .unwrap();
    set_battle_heading(&mut base, id, ObjectId(1), 90.0).unwrap();
    for new_rules in [false, true] {
        let mut world = base.clone();
        let rules = BattleMovementRules {
            charge: BattleChargePolicy {
                new_rules,
                ..BattleChargePolicy::STANDARD
            },
            ..RULES
        };
        assert!(advance_battle_motion(&mut world, rules).unwrap().is_empty());
        let charge = world.btech.constructed_units()[&id].charge();
        assert_eq!(charge.target, Some(target));
        assert_eq!(charge.elapsed, u8::from(new_rules));
        assert_eq!(charge.distance, 0.0);
        assert!(
            world.btech.constructed_units()[&id]
                .limb_recycle()
                .is_empty()
        );
    }
}

/// DFA forecasts use airborne aim, immobility and posture, with no mutation or random draws.
#[tokio::test]
async fn dfa_profile_movement_posture_and_specialist() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = kick_fixture().await;
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    let before = world.btech.clone();
    let profile = battle_dfa_profile(&world, id, target, kick_rules()).unwrap();
    assert_eq!(world.btech, before);
    assert_eq!(profile.base, 5);
    assert_eq!(profile.attacker_movement, 3);
    assert_eq!(profile.target_movement, -4);
    assert_eq!(profile.target_number, 4);
    assert_eq!(profile.received_damage, 7);
    assert_eq!(profile.initial_hit_table, BattleHitTable::Punch);
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["posture"] = serde_json::json!("prone");
        })
        .unwrap();
    let prone = battle_dfa_profile(&world, id, target, kick_rules()).unwrap();
    assert_eq!(prone.target_number, 2);
    assert_eq!(prone.initial_hit_table, BattleHitTable::Weapon);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Melee_Specialist",
        BattleCharacterValue {
            value: 1,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    let specialist = battle_dfa_profile(&world, id, target, kick_rules()).unwrap();
    assert_eq!(specialist.attacker_movement, -1);
    assert_eq!(specialist.inflicted_damage, prone.inflicted_damage + 1);
    assert_eq!(specialist.received_damage, prone.received_damage);
    world.validate(&config).unwrap();
}

/// Landing eligibility uses the occupied hex and four physical limbs, with six weapon sections.
#[tokio::test]
async fn dfa_profile_recovery_and_moved_target() {
    use stompymux_rs::*;
    let (_dir, config, base, id, target) = kick_fixture().await;
    for section in [
        BattleSection::LeftArm,
        BattleSection::RightArm,
        BattleSection::LeftLeg,
        BattleSection::RightLeg,
        BattleSection::LeftTorso,
        BattleSection::RightTorso,
    ] {
        let mut world = base.clone();
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["limb_recycle"] =
                    serde_json::to_value(std::collections::BTreeMap::from([(section, 60_u16)]))
                        .unwrap();
            })
            .unwrap();
        let before = world.btech.clone();
        assert_eq!(
            battle_dfa_profile(&world, id, target, kick_rules()).is_ok(),
            matches!(
                section,
                BattleSection::LeftTorso | BattleSection::RightTorso
            )
        );
        assert_eq!(world.btech, before);
    }
    for (index, mount) in base.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .enumerate()
    {
        let mut world = base.clone();
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["weapon_recycle"] =
                    serde_json::to_value(std::collections::BTreeMap::from([(index, 1_u16)]))
                        .unwrap();
            })
            .unwrap();
        assert_eq!(
            battle_dfa_profile(&world, id, target, kick_rules()).is_ok(),
            mount.criticals.iter().all(|part| matches!(
                part.section,
                BattleSection::CenterTorso | BattleSection::Head
            ))
        );
    }
    let mut world = base;
    let map = world.btech.constructed_units()[&id].position().unwrap().map;
    place_battle_unit(&mut world, target, map, 5, 4).unwrap();
    let before = world.btech.clone();
    assert!(
        battle_dfa_profile(&world, id, target, kick_rules())
            .unwrap_err()
            .to_string()
            .contains("moved")
    );
    assert_eq!(world.btech, before);
    world.validate(&config).unwrap();
}

/// Pilot-based DFA aim can decline an attack that the fixed-base policy still permits.
#[tokio::test]
async fn dfa_profile_configured_piloting_and_unreachable_aim() {
    use stompymux_rs::*;
    let (_dir, _config, mut world, id, target) = kick_fixture().await;
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Piloting-Biped",
        BattleCharacterValue {
            value: 5,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    let victim = &mut state["constructed"][target.0.to_string()];
    victim["power"] = serde_json::json!({"state":"running"});
    victim["motion"]["speed"] = 100.0.into();
    world.btech = serde_json::from_value(state).unwrap();
    let fixed = battle_dfa_profile(&world, id, target, kick_rules()).unwrap();
    assert_eq!(fixed.target_number, 12);
    let before = world.btech.clone();
    let error = battle_dfa_profile(
        &world,
        id,
        target,
        BattlePhysicalRules {
            use_pilot_skill: true,
            ..kick_rules()
        },
    )
    .unwrap_err();
    assert!(error.to_string().contains("BTH 13"), "{error:#}");
    assert_eq!(world.btech, before);
}

/// Power loss lowers charge aim by four and can turn the same attack roll into a hit.
#[tokio::test]
async fn charge_immobility_changes_one_way_and_mutual_outcomes() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    prepare_test_charge(&mut base, id);
    let rules = BattleChargeRules {
        distance: 2.0,
        new_rules: false,
        technology_level_three: false,
        physical: kick_rules(),
    };
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    shot_seed(&mut base, id, seed);
    for running in [false, true] {
        let mut world = base.clone();
        world
            .btech
            .set_unit_power(
                target,
                if running {
                    BattlePower::Running
                } else {
                    BattlePower::Off
                },
            )
            .unwrap();
        let before = world.btech.clone();
        let profile = battle_charge_profile(&world, id, target, rules).unwrap();
        assert_eq!(profile.target_number, if running { 6 } else { 2 });
        assert_eq!(world.btech, before);
        let mut mutual = world.clone();
        let report = resolve_battle_charge(&mut world, id, target, rules).unwrap();
        assert_eq!(report.roll, 2);
        assert_eq!(report.hit, !running);
        let report = resolve_battle_mutual_charge(&mut mutual, id, target, rules, 0.0).unwrap();
        assert_eq!(report.attempts[0].collision.as_ref().unwrap().hit, !running);
        assert!(report.attempts[1].rejection.is_some());
        world.validate(&config).unwrap();
        mutual.validate(&config).unwrap();
    }
}

/// An unconscious target pilot receives the same immobility adjustment without shutting down the unit.
#[tokio::test]
async fn charge_immobility_includes_unconscious_pilots() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = kick_fixture().await;
    prepare_test_charge(&mut world, id);
    let pilot = world.create(&config, "Charge target pilot".into(), Kind::Player);
    world.objects.get_mut(&pilot).unwrap().location = Some(target);
    assign_battle_pilot(&mut world, target, pilot).unwrap();
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["constructed"][target.0.to_string()]["power"] = serde_json::json!({"state":"running"});
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() < 10)
        .unwrap();
    state["recoveries"][pilot.0.to_string()] = serde_json::json!({"mode":{"kind":"tactical","injuries":0},"remaining":0,"pain_resistance":false,"toughness":false,"dice":BattleDice::seeded([seed;32])});
    world.btech = serde_json::from_value(state).unwrap();
    let rules = BattleChargeRules {
        distance: 2.0,
        new_rules: false,
        technology_level_three: false,
        physical: kick_rules(),
    };
    let awake = battle_charge_profile(&world, id, target, rules).unwrap();
    injure_battle_tactical_pilot(&mut world, target, 4, false).unwrap();
    assert!(world.btech.unconscious(pilot));
    let before = world.btech.clone();
    let asleep = battle_charge_profile(&world, id, target, rules).unwrap();
    assert_eq!(asleep.target_number, awake.target_number - 4);
    assert_eq!(asleep.inflicted_damage, awake.inflicted_damage);
    assert_eq!(world.btech, before);
    world.validate(&config).unwrap();
}

/// DFA hits use target packets then leg recoil; misses apply rear damage without an extra ordinary fall.
#[tokio::test]
async fn dfa_damage_hit_miss_and_saved_recovery() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    balance_skill(&mut base);
    // Target hit locations must be reproducible and leave the target alive for its balance check.
    shot_seed(&mut base, target, 0);
    launch_battle_jump(&mut base, id, ObjectId(1), 0, 2.0).unwrap();
    for roll in [2, 12] {
        let mut world = base.clone();
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
            .unwrap();
        shot_seed(&mut world, id, seed);
        let before = world.btech.constructed_units()[&target].clone();
        let report = resolve_battle_dfa(&mut world, id, target, kick_rules()).unwrap();
        assert_eq!(report.roll, roll);
        assert_eq!(report.hit, roll == 12);
        assert!(world.btech.constructed_units()[&id].flight().is_none());
        assert_eq!(world.btech.constructed_units()[&id].limb_recycle().len(), 6);
        let sum = |impacts: &[BattleTacticalImpact]| {
            impacts
                .iter()
                .map(|impact| {
                    let phase = &impact.impact.phases[0];
                    assert!(phase.absorbed + phase.remaining <= 5);
                    phase.absorbed + phase.remaining
                })
                .sum::<u16>()
        };
        assert_eq!(sum(&report.attacker_impacts), 7);
        if report.hit {
            assert_eq!(sum(&report.target_impacts), report.profile.inflicted_damage);
            assert!(report.attacker_impacts.iter().all(|impact| matches!(
                impact.impact.phases[0].section,
                BattleSection::LeftLeg | BattleSection::RightLeg
            )));
            assert_eq!(
                report
                    .balance
                    .iter()
                    .map(|balance| (balance.unit, balance.check.situational))
                    .collect::<Vec<_>>(),
                vec![(id, 4), (target, 2)]
            );
        } else {
            assert!(report.target_impacts.is_empty());
            assert_eq!(world.btech.constructed_units()[&target], before);
            assert_eq!(
                world.btech.constructed_units()[&id].posture(),
                BattlePosture::Prone
            );
            assert_eq!(report.balance.len(), 1);
            assert!(report.balance[0].fall.is_none());
            assert!(report.pilot_injury.is_none());
        }
        persistence::save(&config.database(), &world).await.unwrap();
        let mut loaded = persistence::load(&config.database()).await.unwrap();
        for _ in 0..60 {
            assert_eq!(
                advance_battle_recycle(&mut world),
                advance_battle_recycle(&mut loaded)
            );
        }
        assert_eq!(world.btech, loaded.btech);
        assert!(
            world.btech.constructed_units()[&id]
                .limb_recycle()
                .is_empty()
        );
        world.validate(&config).unwrap();
    }
}

/// A missed landing has one explicit injury check in addition to any damage-triggered injuries.
#[tokio::test]
async fn dfa_damage_miss_pilot_injury_and_rejection() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = kick_fixture().await;
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    shot_seed(&mut world, id, seed);
    let base = world.clone();
    let report = resolve_battle_dfa(&mut world, id, target, kick_rules()).unwrap();
    assert!(!report.hit);
    assert!(!report.balance[0].check.success);
    assert!(report.pilot_injury.is_some());
    assert!(
        report
            .notices
            .iter()
            .any(|notice| notice.text.contains("personal injury"))
    );
    world.validate(&config).unwrap();
    let before = world.btech.clone();
    assert!(resolve_battle_dfa(&mut world, id, target, kick_rules()).is_err());
    assert_eq!(world.btech, before);
    let mut replay = base.clone();
    assert_eq!(
        report,
        resolve_battle_dfa(&mut replay, id, target, kick_rules()).unwrap()
    );
}

/// Damage-induced target knockdown changes later DFA packets from punch to weapon locations.
#[tokio::test]
async fn dfa_damage_resamples_target_posture() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = kick_fixture().await;
    balance_skill(&mut base);
    launch_battle_jump(&mut base, id, ObjectId(1), 0, 2.0).unwrap();
    base.btech
        .rewrite_unit_record(target, |record| {
            record["sections"]["CenterTorso"]["rear"] = 0.into();
        })
        .unwrap();
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    shot_seed(&mut base, id, seed);
    let mut found = false;
    for seed in 0..=255 {
        let mut dice = BattleDice::seeded([seed; 32]);
        if dice.d6() != 3 {
            continue;
        }
        let mut world = base.clone();
        shot_seed(&mut world, target, seed);
        let report = resolve_battle_dfa(&mut world, id, target, kick_rules()).unwrap();
        if !report.target_impacts[0]
            .balance
            .iter()
            .any(|balance| balance.fall.is_some())
        {
            continue;
        }
        if !report.target_impacts.iter().skip(1).any(|impact| {
            matches!(
                impact.impact.phases[0].section,
                BattleSection::LeftLeg | BattleSection::RightLeg
            )
        }) {
            continue;
        }
        assert_eq!(report.profile.initial_hit_table, BattleHitTable::Punch);
        world.validate(&config).unwrap();
        found = true;
        break;
    }
    assert!(
        found,
        "a first-packet knockdown followed by a leg hit from the weapon table"
    );
}

/// A late world-validation failure discards the attack roll, flight cancellation and all damage.
#[tokio::test]
#[cfg_attr(
    not(debug_assertions),
    ignore = "injects a late failure through per-operation validation, which runs only in debug builds"
)]
async fn dfa_damage_candidate_error_rolls_back() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = kick_fixture().await;
    balance_skill(&mut world);
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    let third = world.create(&config, "Invalid saved spectator".into(), Kind::Thing);
    create_battle_unit(
        &mut world,
        third,
        BattleTemplate::parse("JR7-D", include_str!("fixtures/btech/mechs/JR7-D.toml")).unwrap(),
    )
    .unwrap();
    support::seed_object_dice(&mut world, third, support::FIXTURE_DICE_SEED);
    world
        .btech
        .rewrite_unit_record(third, |record| {
            record["charge"]["elapsed"] = 61.into();
        })
        .unwrap();
    assert!(battle_dfa_profile(&world, id, target, kick_rules()).is_ok());
    let before = world.btech.clone();
    assert!(resolve_battle_dfa(&mut world, id, target, kick_rules()).is_err());
    assert_eq!(world.btech, before);
}

/// Accepted DFA and charge attacks finish their packets after an early torso kill.
#[tokio::test]
async fn landing_collision_packets_continue_after_destruction() {
    use stompymux_rs::*;
    let (_dir, config, base, id, target) = kick_fixture().await;
    for dfa in [false, true] {
        let mut world = base.clone();
        if dfa {
            balance_skill(&mut world);
            launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
        } else {
            prepare_test_charge(&mut world, id);
        }
        world
            .btech
            .rewrite_unit_record(target, |record| {
                let torso = &mut record["sections"]["CenterTorso"];
                torso["armor"] = 0.into();
                torso["rear"] = 0.into();
                torso["internal"] = 1.into();
            })
            .unwrap();
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        shot_seed(&mut world, id, seed);
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = BattleDice::seeded([*seed; 32]);
                if dfa {
                    dice.d6() == 3
                } else {
                    dice.two_d6() == 7
                }
            })
            .unwrap();
        shot_seed(&mut world, target, seed);
        let (damage, impacts) = if dfa {
            let report = resolve_battle_dfa(&mut world, id, target, kick_rules()).unwrap();
            (report.profile.inflicted_damage, report.target_impacts)
        } else {
            let report = resolve_battle_charge(
                &mut world,
                id,
                target,
                BattleChargeRules {
                    distance: 2.0,
                    new_rules: false,
                    technology_level_three: false,
                    physical: kick_rules(),
                },
            )
            .unwrap();
            (report.profile.inflicted_damage, report.target_impacts)
        };
        assert!(impacts[0].impact.destroyed);
        assert_eq!(impacts.len(), usize::from(damage.div_ceil(5)));
        assert!(world.btech.constructed_units()[&target].is_destroyed());
        let before = world.btech.clone();
        assert!(
            resolve_battle_tactical_impact(
                &mut world,
                target,
                balance_hit(BattleSection::LeftArm, false),
                1,
                fall_rules()
            )
            .is_err()
        );
        assert_eq!(world.btech, before);
        world.validate(&config).unwrap();
    }
}

/// A missed DFA settles at the terrain elevation under ice without adding ordinary fall damage.
#[tokio::test]
async fn dfa_damage_miss_ice_settles_below_surface() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = kick_fixture().await;
    balance_skill(&mut world);
    launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
    let position = world.btech.constructed_units()[&id].position().unwrap();
    let index = usize::from(position.y) * world.btech.maps()[&position.map].width as usize
        + usize::from(position.x);
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["maps"][position.map.0.to_string()]["terrain"][index] =
        serde_json::to_value(Hex::new(Terrain::Ice, 1)).unwrap();
    world.btech = serde_json::from_value(state).unwrap();
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    shot_seed(&mut world, id, seed);
    let report = resolve_battle_dfa(&mut world, id, target, kick_rules()).unwrap();
    assert!(!report.hit);
    assert_eq!(
        serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["ground_elevation"],
        -1.0
    );
    assert_eq!(
        world.btech.constructed_units()[&id].posture(),
        BattlePosture::Prone
    );
    assert!(
        !world.btech.constructed_units()[&id]
            .flooded_sections()
            .is_empty()
    );
    world.validate(&config).unwrap();
}

/// A new in-character cockpit breach evacuates crew, with flooding and movement rolled back on failure.
#[tokio::test]
async fn cockpit_flood_action_evacuates_and_replays() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = water_fixture(2).await;
    release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    let pilot = ObjectId(2);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&pilot).unwrap().location = Some(id);
    assign_battle_pilot(&mut world, id, pilot).unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    apply_damage_phase(
        &mut world,
        id,
        BattleSection::Head,
        7,
        BattleDamagePhase::Armor { rear: false },
    )
    .unwrap();
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let baseline = world.clone();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    scripts.world_mut().objects.remove(&afterlife);
    assert!(flood_battle_unit_action(&scripts, &config, id, fall_rules()).is_err());
    assert_eq!(scripts.world().btech, baseline.btech);
    assert_eq!(scripts.world().objects[&pilot].location, Some(id));
    assert!(scripts.drain_outbox().is_empty());
    *scripts.world_mut() = baseline;
    let reports = flood_battle_unit_action(&scripts, &config, id, fall_rules()).unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].section, BattleSection::Head);
    let unit = scripts.world().btech.constructed_units()[&id].clone();
    assert!(unit.is_destroyed());
    assert_eq!(unit.pilot(), None);
    assert!(unit.sections()[&BattleSection::Head].internal > 0);
    assert_eq!(scripts.world().objects[&pilot].location, Some(afterlife));
    assert_eq!(scripts.world().objects[&ObjectId(1)].location, Some(id));
    assert!(
        flood_battle_unit_action(&scripts, &config, id, fall_rules())
            .unwrap()
            .is_empty()
    );
    let candidate = scripts.world().clone();
    candidate.validate(&config).unwrap();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, candidate.btech);
    assert_eq!(loaded.objects[&pilot].location, Some(afterlife));
    assert!(loaded.btech.constructed_units()[&id].is_destroyed());
}

/// Character falls share posture/damage geometry while using RPG pilot health and saved dice.
#[tokio::test]
async fn character_ground_fall_action_replays_health_and_damage() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = fixture('.').await;
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    set_battle_character(
        &mut world,
        ObjectId(1),
        BattleCharacter {
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
    support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    shot_seed(&mut world, id, seed);
    persistence::save(&config.database(), &world).await.unwrap();
    let mut loaded = persistence::load(&config.database()).await.unwrap();
    loaded
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(world.clone())),
    )
    .unwrap();
    let replay = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(loaded))).unwrap();
    assert!(resolve_battle_fall(&mut world, id, 1, fall_rules()).is_err());
    let report = fall_battle_unit_action(&scripts, &config, id, 1, fall_rules()).unwrap();
    let repeated = fall_battle_unit_action(&replay, &config, id, 1, fall_rules()).unwrap();
    assert_eq!(report, repeated);
    assert_eq!(scripts.world().btech, replay.world().btech);
    assert!(!report.avoidance.unwrap().success);
    assert!(report.pilot_injury.is_none());
    assert!(report.character_injury.is_some());
    assert_eq!(report.damage, 4);
    assert!(!report.groups.is_empty());
    assert_eq!(
        scripts.world().btech.constructed_units()[&id].posture(),
        BattlePosture::Prone
    );
    assert_eq!(
        u16::from(scripts.world().btech.constructed_units()[&id].pilot_injuries()),
        scripts.world().btech.constructed_units()[&id]
            .character_pilot_status()
            .unwrap()
            .injuries
    );
    assert!(scripts.world().btech.characters()[&ObjectId(1)].bruise >= 10);
    scripts.world().validate(&config).unwrap();
}

/// Flooded support can cause a character fall and a secondary cockpit breach in one transaction.
#[tokio::test]
async fn character_leg_flood_fall_evacuates_nested_casualty() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = water_fixture(1).await;
    release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
    let pilot = ObjectId(2);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    world.objects.get_mut(&pilot).unwrap().location = Some(id);
    set_battle_character(
        &mut world,
        pilot,
        BattleCharacter {
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
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    assign_battle_pilot(&mut world, id, pilot).unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    for (section, damage) in [(BattleSection::Head, 7), (BattleSection::LeftLeg, 6)] {
        apply_damage_phase(
            &mut world,
            id,
            section,
            damage,
            BattleDamagePhase::Armor { rear: false },
        )
        .unwrap();
    }
    world
        .objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    shot_seed(&mut world, id, seed);
    world
        .objects
        .get_mut(&ObjectId(1))
        .unwrap()
        .flags
        .insert(Flag::Connected);
    let baseline = world.clone();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    scripts.world_mut().objects.remove(&afterlife);
    assert!(flood_battle_unit_action(&scripts, &config, id, fall_rules()).is_err());
    assert_eq!(scripts.world().btech, baseline.btech);
    assert!(scripts.drain_outbox().is_empty());
    *scripts.world_mut() = baseline;
    let reports = flood_battle_unit_action(&scripts, &config, id, fall_rules()).unwrap();
    let fall = reports
        .iter()
        .find(|report| report.section == BattleSection::LeftLeg)
        .unwrap()
        .fall
        .as_ref()
        .unwrap();
    assert!(fall.character_injury.is_some());
    let output = scripts.drain_outbox();
    let check = fall.avoidance.unwrap();
    let expected = format!(
        "Modified Pilot Skill: BTH {}\tRoll: {}",
        check.target,
        check.roll.unwrap()
    );
    let messages: Vec<_> = output
        .iter()
        .filter(|(who, _)| *who == pilot)
        .map(|(_, message)| message.source())
        .collect();
    let index = messages
        .iter()
        .position(|message| *message == expected)
        .unwrap();
    assert!(index > 1);
    assert_eq!(messages[index - 1], "You make a piloting skill roll!");
    assert!(!output.iter().any(|(who, message)| *who != pilot
        && (message.source().starts_with("Modified Pilot Skill:")
            || message.source() == "You make a piloting skill roll!")));

    assert!(
        fall.flooding
            .iter()
            .any(|report| report.section == BattleSection::Head)
    );
    assert!(fall.groups.is_empty());
    assert_eq!(scripts.world().objects[&pilot].location, Some(afterlife));
    assert!(scripts.world().btech.constructed_units()[&id].is_destroyed());
    let candidate = scripts.world().clone();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(candidate.btech, loaded.btech);
    assert_eq!(loaded.objects[&pilot].location, Some(afterlife));
}

/// Physical head hits apply character health and atomically publish fatal crew evacuation.
#[tokio::test]
async fn character_physical_action_replays_and_rolls_back() {
    use stompymux_rs::*;
    for fatal in [false, true] {
        let (_dir, config, mut world, id, target) = kick_fixture().await;
        let pilot = ObjectId(2);
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        world.objects.get_mut(&pilot).unwrap().location = Some(target);
        assign_battle_pilot(&mut world, target, pilot).unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        world
            .objects
            .get_mut(&target)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            pilot,
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: if fatal { 50 } else { 0 },
                lethal: if fatal { 40 } else { 0 },
            },
        )
        .unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        let attack_seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        let head_seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).d6() == 6)
            .unwrap();
        shot_seed(&mut world, id, attack_seed);
        shot_seed(&mut world, target, head_seed);
        let rules = kick_rules();
        let attack = BattlePhysicalAttack::Punch {
            arm: BattleArm::Left,
        };
        let baseline = world.clone();
        assert!(
            resolve_battle_punch(
                &mut world,
                id,
                ObjectId(1),
                target,
                BattleArmSelection::Left,
                rules
            )
            .is_err()
        );
        assert_eq!(world.btech, baseline.btech);
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        assert!(
            resolve_battle_physical_attack_action(
                &scripts, &config, id, pilot, target, attack, rules
            )
            .is_err()
        );
        assert_eq!(scripts.world().btech, baseline.btech);
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        if fatal {
            scripts.world_mut().objects.remove(&afterlife);
            assert!(
                resolve_battle_physical_attack_action(
                    &scripts,
                    &config,
                    id,
                    ObjectId(1),
                    target,
                    attack,
                    rules
                )
                .is_err()
            );
            assert_eq!(scripts.world().btech, baseline.btech);
            assert_eq!(scripts.world().objects[&pilot].location, Some(target));
            assert!(scripts.drain_outbox().is_empty());
            *scripts.world_mut() = baseline.clone();
        }
        persistence::save(&config.database(), &baseline)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        restored.objects.get_mut(&ObjectId(1)).unwrap().flags =
            baseline.objects[&ObjectId(1)].flags.clone();
        let replay =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        let report = resolve_battle_physical_attack_action(
            &scripts,
            &config,
            id,
            ObjectId(1),
            target,
            attack,
            rules,
        )
        .unwrap();
        assert_eq!(
            report,
            resolve_battle_physical_attack_action(
                &replay,
                &config,
                id,
                ObjectId(1),
                target,
                attack,
                rules
            )
            .unwrap()
        );
        assert_eq!(scripts.world().btech, replay.world().btech);
        assert!(report.hit);
        assert_eq!(
            report
                .impact
                .as_ref()
                .unwrap()
                .impact
                .character_injuries
                .len(),
            1
        );
        let candidate = scripts.world().clone();
        assert_eq!(
            candidate.btech.constructed_units()[&target]
                .character_pilot_status()
                .unwrap()
                .killed,
            fatal
        );
        assert!(
            candidate.btech.constructed_units()[&target].sections()[&BattleSection::Head].internal
                > 0
        );
        assert_eq!(
            candidate.objects[&pilot].location,
            Some(if fatal { afterlife } else { target })
        );
        assert_eq!(
            candidate.btech.constructed_units()[&id].limb_recycle()[&BattleSection::LeftArm],
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

/// A successful character trip carries its balance fall through the physical action report.
#[tokio::test]
async fn character_trip_action_resolves_balance_fall() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = kick_fixture().await;
    let pilot = ObjectId(2);
    world.objects.get_mut(&pilot).unwrap().location = Some(target);
    assign_battle_pilot(&mut world, target, pilot).unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    world
        .objects
        .get_mut(&target)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    set_battle_character(
        &mut world,
        pilot,
        BattleCharacter {
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
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    let attacker_seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    let target_seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
        .unwrap();
    shot_seed(&mut world, id, attacker_seed);
    shot_seed(&mut world, target, target_seed);
    let before = world.clone();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let report = resolve_battle_physical_attack_action(
        &scripts,
        &config,
        id,
        ObjectId(1),
        target,
        BattlePhysicalAttack::Trip {
            leg: BattleLeg::Right,
        },
        kick_rules(),
    )
    .unwrap();
    assert!(report.hit);
    assert!(report.impact.is_none());
    assert!(!report.balance.unwrap().success);
    assert!(report.fall.is_some());
    assert_eq!(
        scripts.world().btech.constructed_units()[&target].posture(),
        BattlePosture::Prone
    );
    assert_ne!(
        scripts.world().btech.constructed_units()[&target].sections(),
        before.btech.constructed_units()[&target].sections()
    );
    scripts.world().validate(&config).unwrap();
}

/// Two-arm character attacks preserve hit order, stop on a destroyed target and roll back together.
#[tokio::test]
async fn character_arm_sequence_orders_casualties_and_replays() {
    use stompymux_rs::*;
    for lethal in [0, 30, 40] {
        let (_dir, config, mut world, id, target) = kick_fixture().await;
        let pilot = ObjectId(2);
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        world.objects.get_mut(&pilot).unwrap().location = Some(target);
        assign_battle_pilot(&mut world, target, pilot).unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        world
            .objects
            .get_mut(&target)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            pilot,
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: if lethal == 0 { 0 } else { 50 },
                lethal,
            },
        )
        .unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        let attack_seed = (0..=255)
            .find(|seed| {
                let mut dice = BattleDice::seeded([*seed; 32]);
                dice.two_d6() >= 8 && dice.two_d6() >= 8
            })
            .unwrap();
        let head_seed = (0..=255)
            .find(|seed| {
                let mut dice = BattleDice::seeded([*seed; 32]);
                dice.d6() == 6 && dice.d6() == 6
            })
            .unwrap();
        shot_seed(&mut world, id, attack_seed);
        shot_seed(&mut world, target, head_seed);
        let choice = BattleArmAttackChoice {
            arms: BattleArmSelection::Both,
            kind: BattleArmWeapon::Fixed(BattleArmAttack::Punch),
        };
        let baseline = world.clone();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        if lethal > 0 {
            scripts.world_mut().objects.remove(&afterlife);
            assert!(
                resolve_battle_arm_attack_action(
                    &scripts,
                    &config,
                    id,
                    ObjectId(1),
                    target,
                    choice,
                    kick_rules()
                )
                .is_err()
            );
            assert_eq!(scripts.world().btech, baseline.btech);
            assert_eq!(scripts.world().objects[&pilot].location, Some(target));
            assert!(scripts.drain_outbox().is_empty());
            *scripts.world_mut() = baseline.clone();
        }
        persistence::save(&config.database(), &baseline)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        restored.objects.get_mut(&ObjectId(1)).unwrap().flags =
            baseline.objects[&ObjectId(1)].flags.clone();
        let replay =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        let report = resolve_battle_arm_attack_action(
            &scripts,
            &config,
            id,
            ObjectId(1),
            target,
            choice,
            kick_rules(),
        )
        .unwrap();
        assert_eq!(
            report,
            resolve_battle_arm_attack_action(
                &replay,
                &config,
                id,
                ObjectId(1),
                target,
                choice,
                kick_rules()
            )
            .unwrap()
        );
        assert_eq!(scripts.world().btech, replay.world().btech);
        assert_eq!(report.attacks.len(), if lethal == 40 { 1 } else { 2 });
        assert_eq!(report.rejections.len(), usize::from(lethal == 40));
        for (index, attack) in report.attacks.iter().enumerate() {
            assert!(attack.hit);
            assert_eq!(
                attack.profile.attack,
                BattlePhysicalAttack::Punch {
                    arm: if index == 0 {
                        BattleArm::Left
                    } else {
                        BattleArm::Right
                    }
                }
            );
            assert_eq!(
                attack
                    .impact
                    .as_ref()
                    .unwrap()
                    .impact
                    .character_injuries
                    .len(),
                1
            );
        }
        let candidate = scripts.world().clone();
        assert_eq!(
            candidate.btech.constructed_units()[&target]
                .character_pilot_status()
                .unwrap()
                .killed,
            lethal > 0
        );
        assert_eq!(
            candidate.objects[&pilot].location,
            Some(if lethal > 0 { afterlife } else { target })
        );
        assert_eq!(
            candidate.btech.constructed_units()[&id]
                .limb_recycle()
                .len(),
            report.attacks.len()
        );
        let mut expected_dice = BattleDice::seeded([attack_seed; 32]);
        for _ in &report.attacks {
            expected_dice.two_d6();
        }
        assert_eq!(
            serde_json::to_value(&candidate.btech.constructed_units()[&id]).unwrap()["dice"],
            serde_json::to_value(expected_dice).unwrap()
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

/// Native and Lua physical commands publish identical character consequences and honor outer rollback.
#[tokio::test]
async fn character_punch_native_lua_casualty_and_abort_parity() {
    use stompymux_rs::*;
    for extended in [false, true] {
        for fatal in [false, true] {
            let (_dir, _config, mut base, id, target) = kick_fixture().await;
            let path = _dir.path().join("stompymux.toml");
            let mut source: toml::Value =
                toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            source["battletech"]
                .as_table_mut()
                .unwrap()
                .insert("extended_piloting".into(), i64::from(extended).into());
            std::fs::write(path, toml::to_string(&source).unwrap()).unwrap();
            let config = Config::load(_dir.path()).unwrap();
            base.objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            let mut signature = base.btech.constructed_units()[&target].signature();
            signature.team = 1;
            set_battle_unit_signature(&mut base, target, signature).unwrap();
            let pilot = ObjectId(2);
            base.objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .remove(Flag::Wizard);
            base.objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .insert(Flag::Connected);
            base.objects.get_mut(&pilot).unwrap().location = Some(target);
            assign_battle_pilot(&mut base, target, pilot).unwrap();
            support::seed_object_dice(&mut base, pilot, support::FIXTURE_DICE_SEED);
            base.objects
                .get_mut(&target)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            set_battle_character(
                &mut base,
                pilot,
                BattleCharacter {
                    build: 5,
                    reflexes: 5,
                    intuition: 5,
                    learn: 5,
                    charisma: 5,
                    bruise: if fatal { 50 } else { 0 },
                    lethal: if fatal { 30 } else { 0 },
                },
            )
            .unwrap();
            support::seed_object_dice(&mut base, pilot, support::FIXTURE_DICE_SEED);
            let attack_seed = (0..=255)
                .find(|seed| {
                    let mut dice = BattleDice::seeded([*seed; 32]);
                    dice.two_d6() >= 10 && dice.two_d6() >= 10
                })
                .unwrap();
            let head_seed = (0..=255)
                .find(|seed| {
                    let mut dice = BattleDice::seeded([*seed; 32]);
                    dice.d6() == 6 && dice.d6() == 6
                })
                .unwrap();
            shot_seed(&mut base, id, attack_seed);
            shot_seed(&mut base, target, head_seed);
            install_xp_channels(&mut base);
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
                lua.eval_callback::<()>(&format!("{call}; error('abort character punch')"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, base.btech);
            assert_eq!(lua.world().objects[&pilot].location, Some(target));
            assert!(lua.drain_outbox().is_empty());
            assert_eq!(
                serde_json::to_value(&lua.world().channels).unwrap(),
                serde_json::to_value(&base.channels).unwrap()
            );
            lua.world_mut()
                .channels
                .get_mut("MechPilotXP")
                .unwrap()
                .messages = i64::MAX - 1;
            assert!(
                lua.eval_callback::<mlua::Table>(&format!("return {call}"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, base.btech);
            assert_eq!(lua.world().channels["MechPilotXP"].messages, i64::MAX - 1);
            assert!(lua.world().channels["MechPilotXP"].history.is_empty());
            assert!(lua.drain_outbox().is_empty());
            *lua.world_mut() = base.clone();
            commands::run(
                &native,
                &config,
                ObjectId(1),
                1,
                &format!("punch both #{}", target.0),
            )
            .unwrap();
            let _: mlua::Table = lua.eval_callback(&format!("return {call}")).unwrap();
            assert_eq!(
                without_xp_timestamps(&native.world().btech),
                without_xp_timestamps(&lua.world().btech)
            );
            let messages = |scripts: &Scripts| {
                scripts
                    .drain_outbox()
                    .into_iter()
                    .map(|(to, message)| (to, message.source().to_owned()))
                    .collect::<Vec<_>>()
            };
            let native_messages = messages(&native);
            assert_eq!(native_messages, messages(&lua));
            assert_eq!(
                native_messages
                    .iter()
                    .filter(|(to, text)| *to == ObjectId(1) && text.contains("You try to punch"))
                    .count(),
                2
            );
            assert!(
                native_messages
                    .iter()
                    .any(|(to, text)| *to == pilot && text.contains("keep consciousness"))
            );
            assert!(
                !native_messages
                    .iter()
                    .any(|(to, text)| *to == ObjectId(1) && text.contains("keep consciousness"))
            );
            let expected = ObjectId(if fatal {
                config.battletech.afterlife_dbref
            } else {
                target.0
            });
            assert_eq!(
                native.world().objects[&pilot].location,
                Some(expected),
                "{native_messages:?}"
            );
            assert_eq!(lua.world().objects[&pilot].location, Some(expected));
            let candidate = native.world().clone();
            let skill = if extended {
                "Piloting-Biped"
            } else {
                "Piloting-Battlemech"
            };
            let other = if extended {
                "Piloting-Battlemech"
            } else {
                "Piloting-Biped"
            };
            let values = &candidate.btech.character_values()[&ObjectId(1)];
            assert_eq!(values[skill].experience_balance(), 2);
            let channel = &candidate.channels["MechPilotXP"];
            assert_eq!(channel.messages, 2);
            assert_eq!(channel.history.len(), 2);
            for entry in &channel.history {
                assert_eq!(
                    text::plain_with(native.palette(), &entry.message),
                    format!("[MechPilotXP] GOD gained 1 {skill} XP")
                );
            }
            assert_eq!(
                channel
                    .history
                    .iter()
                    .map(|entry| &entry.message)
                    .collect::<Vec<_>>(),
                lua.world().channels["MechPilotXP"]
                    .history
                    .iter()
                    .map(|entry| &entry.message)
                    .collect::<Vec<_>>()
            );

            assert_eq!(
                values
                    .get(other)
                    .copied()
                    .unwrap_or_default()
                    .experience_balance(),
                0
            );

            candidate.validate(&config).unwrap();
            persistence::save(&config.database(), &candidate)
                .await
                .unwrap();
            let loaded = persistence::load(&config.database()).await.unwrap();
            assert_eq!(loaded.btech, candidate.btech);
            assert_eq!(
                serde_json::to_value(&loaded.channels).unwrap(),
                serde_json::to_value(&candidate.channels).unwrap()
            );
            assert_eq!(loaded.objects[&pilot].location, Some(expected));
        }
    }
}

/// Physical XP follows direct damage and combat eligibility, including reduced glancing damage.
#[tokio::test]
async fn physical_experience_eligibility_and_damage_awards() {
    use stompymux_rs::*;
    for extended in [false, true] {
        for mode in [
            "kick",
            "punch",
            "glancing",
            "miss",
            "trip",
            "friendly",
            "tactical_source",
            "tactical_target",
            "disconnected",
        ] {
            let (_dir, config, mut world, id, target) = kick_fixture().await;
            world
                .btech
                .set_unit_power(target, BattlePower::Running)
                .unwrap();
            let mut rules = kick_rules();
            rules.fall.extended_piloting = extended;
            let attack = match mode {
                "kick" | "glancing" => BattlePhysicalAttack::Kick {
                    leg: BattleLeg::Right,
                },
                "trip" => BattlePhysicalAttack::Trip {
                    leg: BattleLeg::Right,
                },
                _ => BattlePhysicalAttack::Punch {
                    arm: BattleArm::Left,
                },
            };
            let profile = match attack {
                BattlePhysicalAttack::Kick { leg } => {
                    battle_kick_profile(&world, id, ObjectId(1), target, leg, rules).unwrap()
                }
                BattlePhysicalAttack::Trip { leg } => {
                    battle_trip_profile(&world, id, ObjectId(1), target, leg, rules).unwrap()
                }
                _ => battle_punch_profile(&world, id, ObjectId(1), target, BattleArm::Left, rules)
                    .unwrap(),
            };
            let roll = if mode == "miss" {
                2
            } else if mode == "glancing" {
                profile.target_number as u8
            } else {
                12
            };
            if mode == "glancing" {
                rules.glancing = BattleGlancingMode::AtTarget;
            }
            let seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
                .unwrap();
            shot_seed(&mut world, id, seed);
            for (unit, pilot) in [(id, ObjectId(1)), (target, ObjectId(2))] {
                world
                    .objects
                    .get_mut(&unit)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
                set_battle_character(
                    &mut world,
                    pilot,
                    BattleCharacter {
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
                support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
            }
            let mut signature = world.btech.constructed_units()[&target].signature();
            signature.team = if mode == "friendly" { 0 } else { 1 };
            set_battle_unit_signature(&mut world, target, signature).unwrap();
            if mode == "tactical_source" {
                world
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .flags
                    .remove(Flag::InCharacter);
            }
            if mode == "tactical_target" {
                world
                    .objects
                    .get_mut(&target)
                    .unwrap()
                    .flags
                    .remove(Flag::InCharacter);
            }
            if mode == "disconnected" {
                world
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .remove(Flag::Connected);
            }
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let report = resolve_battle_physical_attack_action(
                &scripts,
                &config,
                id,
                ObjectId(1),
                target,
                attack,
                rules,
            )
            .unwrap();
            let eligible = matches!(mode, "kick" | "punch" | "glancing");
            assert_eq!(report.experience.is_some(), eligible, "{mode}");
            if let Some(award) = report.experience {
                assert!(award.accepted);
                let damage = if report.glancing {
                    profile.damage.div_ceil(2)
                } else {
                    profile.damage
                };
                assert_eq!(
                    award.after.experience_balance(),
                    u32::from(damage / 3).max(1),
                    "{mode}"
                );
                assert!(award.after.last_used > 0);
                let skill = if extended {
                    "Piloting-Biped"
                } else {
                    "Piloting-Battlemech"
                };
                let other = if extended {
                    "Piloting-Battlemech"
                } else {
                    "Piloting-Biped"
                };
                assert_eq!(
                    scripts.world().btech.character_values()[&ObjectId(1)][skill],
                    award.after
                );
                assert_eq!(
                    scripts.world().btech.character_values()[&ObjectId(1)]
                        .get(other)
                        .copied()
                        .unwrap_or_default()
                        .experience_balance(),
                    0
                );
            }
            scripts.world().validate(&config).unwrap();
        }
    }
}

/// A lethal hit earns XP before damage, while failed evacuation restores the award and attacker dice.
#[tokio::test]
async fn physical_experience_rolls_back_with_fatal_evacuation() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id, target) = kick_fixture().await;
    let pilot = ObjectId(2);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world.objects.get_mut(&pilot).unwrap().location = Some(target);
    assign_battle_pilot(&mut world, target, pilot).unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    for (unit, player) in [(id, ObjectId(1)), (target, pilot)] {
        world
            .objects
            .get_mut(&unit)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            player,
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: if player == pilot { 50 } else { 0 },
                lethal: if player == pilot { 40 } else { 0 },
            },
        )
        .unwrap();
        support::seed_object_dice(&mut world, player, support::FIXTURE_DICE_SEED);
    }
    let mut signature = world.btech.constructed_units()[&target].signature();
    signature.team = 1;
    set_battle_unit_signature(&mut world, target, signature).unwrap();
    let hit = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    let head = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).d6() == 6)
        .unwrap();
    shot_seed(&mut world, id, hit);
    shot_seed(&mut world, target, head);
    let baseline = world.clone();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let attack = BattlePhysicalAttack::Punch {
        arm: BattleArm::Left,
    };
    scripts
        .world_mut()
        .objects
        .remove(&ObjectId(config.battletech.afterlife_dbref));
    assert!(
        resolve_battle_physical_attack_action(
            &scripts,
            &config,
            id,
            ObjectId(1),
            target,
            attack,
            kick_rules()
        )
        .is_err()
    );
    assert_eq!(scripts.world().btech, baseline.btech);
    assert!(scripts.drain_outbox().is_empty());
    *scripts.world_mut() = baseline;
    let report = resolve_battle_physical_attack_action(
        &scripts,
        &config,
        id,
        ObjectId(1),
        target,
        attack,
        kick_rules(),
    )
    .unwrap();
    assert_eq!(report.experience.unwrap().after.experience_balance(), 1);
    assert!(scripts.world().btech.constructed_units()[&target].is_destroyed());
    let candidate = scripts.world().clone();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        candidate.btech
    );
}

/// Character charge packets continue after a fatal crew hit, with no XP from recoil or later wreck hits.
#[tokio::test]
async fn character_charge_action_packets_xp_and_casualty_rollback() {
    use stompymux_rs::*;
    for extended in [false, true] {
        for fatal in [false, true] {
            let (_dir, config, mut world, id, target) = kick_fixture().await;
            prepare_test_charge(&mut world, id);
            let pilot = ObjectId(2);
            world
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .remove(Flag::Wizard);
            world.objects.get_mut(&pilot).unwrap().location = Some(target);
            assign_battle_pilot(&mut world, target, pilot).unwrap();
            support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
            for (unit, player) in [(id, ObjectId(1)), (target, pilot)] {
                world
                    .objects
                    .get_mut(&unit)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
                set_battle_character(
                    &mut world,
                    player,
                    BattleCharacter {
                        build: 5,
                        reflexes: 5,
                        intuition: 5,
                        learn: 5,
                        charisma: 5,
                        bruise: if fatal && player == pilot { 50 } else { 0 },
                        lethal: if fatal && player == pilot { 40 } else { 0 },
                    },
                )
                .unwrap();
                support::seed_object_dice(&mut world, player, support::FIXTURE_DICE_SEED);
            }
            let mut signature = world.btech.constructed_units()[&target].signature();
            signature.team = 1;
            set_battle_unit_signature(&mut world, target, signature).unwrap();
            let attacker_seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
                .unwrap();
            let target_seed = (0..=255)
                .find(|seed| {
                    let mut dice = BattleDice::seeded([*seed; 32]);
                    dice.two_d6() == 12 && (5..=9).contains(&dice.two_d6())
                })
                .unwrap();
            shot_seed(&mut world, id, attacker_seed);
            shot_seed(&mut world, target, target_seed);
            let mut physical = kick_rules();
            physical.fall.extended_piloting = extended;
            let rules = BattleChargeRules {
                distance: 2.0,
                new_rules: false,
                technology_level_three: false,
                physical,
            };
            install_xp_channels(&mut world);
            let baseline = world.clone();
            assert!(resolve_battle_charge(&mut world, id, target, rules).is_err());
            assert_eq!(world.btech, baseline.btech);
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let afterlife = ObjectId(config.battletech.afterlife_dbref);
            if fatal {
                scripts.world_mut().objects.remove(&afterlife);
                assert!(
                    resolve_battle_charge_action(&scripts, &config, id, target, rules).is_err()
                );
                assert_eq!(scripts.world().btech, baseline.btech);
                assert_eq!(scripts.world().objects[&pilot].location, Some(target));
                assert!(scripts.drain_outbox().is_empty());
                assert_eq!(
                    serde_json::to_value(&scripts.world().channels).unwrap(),
                    serde_json::to_value(&baseline.channels).unwrap()
                );
                *scripts.world_mut() = baseline;
            }
            let report =
                resolve_battle_charge_action(&scripts, &config, id, target, rules).unwrap();
            assert!(report.hit);
            assert_eq!(
                report.target_impacts.len(),
                usize::from(report.profile.inflicted_damage.div_ceil(5))
            );
            assert_eq!(
                report.attacker_impacts.len(),
                usize::from(report.received_damage.div_ceil(5))
            );
            assert!(report.target_impacts.len() > 1);
            assert_eq!(
                report.experience.len(),
                if fatal {
                    1
                } else {
                    report.target_impacts.len()
                }
            );
            assert_eq!(report.target_impacts[0].impact.character_injuries.len(), 1);
            let candidate = scripts.world().clone();
            let channel = &candidate.channels["MechPilotXP"];
            let protection_messages: usize = report
                .balance
                .iter()
                .filter_map(|balance| balance.fall.as_ref())
                .map(|fall| fall.experience_messages.len())
                .sum();
            assert_eq!(
                channel.messages as usize,
                report.experience_messages.len() + protection_messages
            );
            assert_eq!(
                channel.history.len(),
                report
                    .experience
                    .iter()
                    .filter(|award| award.accepted)
                    .count()
                    + report
                        .balance
                        .iter()
                        .filter(|balance| balance
                            .check
                            .experience
                            .is_some_and(|award| award.accepted))
                        .count()
                    + protection_messages
            );
            for message in &report.experience_messages[..report.experience.len()] {
                assert_eq!(message.channel, BattleChannel::PilotingExperience);
                assert_eq!(
                    message.text,
                    format!(
                        "GOD gained 1 {} XP",
                        if extended {
                            "Piloting-Biped"
                        } else {
                            "Piloting-Battlemech"
                        }
                    )
                );
            }

            assert_eq!(
                candidate.btech.constructed_units()[&target].is_destroyed(),
                fatal
            );
            assert_eq!(
                candidate.objects[&pilot].location,
                Some(if fatal { afterlife } else { target })
            );
            let skill = if extended {
                "Piloting-Biped"
            } else {
                "Piloting-Battlemech"
            };
            let earned =
                candidate.btech.character_values()[&ObjectId(1)][skill].experience_balance();
            let control = report
                .balance
                .iter()
                .find(|balance| balance.unit == id)
                .and_then(|balance| balance.check.experience);
            let protection = report
                .balance
                .iter()
                .find(|balance| balance.unit == id)
                .and_then(|balance| balance.fall.as_ref())
                .and_then(|fall| fall.avoidance.unwrap().experience);
            assert_eq!(
                earned,
                protection
                    .or(control)
                    .map_or(report.experience.len() as u32, |award| award
                        .after
                        .experience_balance())
            );
            if let Some(award) = control {
                assert!(award.accepted);
                assert_eq!(
                    award.after.experience_balance() - award.before.experience_balance(),
                    if extended { 1 } else { 3 }
                );
                assert_eq!(
                    report.experience_messages.last().unwrap().text,
                    format!("GOD gained {} {skill} XP", if extended { 1 } else { 3 })
                );
            }
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
}

/// A pre-rolled mutual charge survives first-collision crew death, with one atomic evacuation boundary.
#[tokio::test]
async fn character_mutual_charge_keeps_accepted_second_attack() {
    use stompymux_rs::*;
    let (_dir, config, mut world, first, second) = kick_fixture().await;
    prepare_test_charge(&mut world, first);
    let pilot = ObjectId(2);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    world.objects.get_mut(&pilot).unwrap().location = Some(second);
    assign_battle_pilot(&mut world, second, pilot).unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    for (unit, player) in [(first, ObjectId(1)), (second, pilot)] {
        world
            .objects
            .get_mut(&unit)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            player,
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: if player == pilot { 50 } else { 0 },
                lethal: if player == pilot { 40 } else { 0 },
            },
        )
        .unwrap();
        support::seed_object_dice(&mut world, player, support::FIXTURE_DICE_SEED);
    }
    let mut signature = world.btech.constructed_units()[&second].signature();
    signature.team = 1;
    set_battle_unit_signature(&mut world, second, signature).unwrap();
    let second_dice = (0..=u16::MAX)
        .find_map(|seed| {
            let mut bytes = [0; 32];
            bytes[..2].copy_from_slice(&seed.to_le_bytes());
            let dice = BattleDice::seeded(bytes);
            let mut probe = dice.clone();
            (probe.two_d6() == 12 && probe.two_d6() == 12).then_some(dice)
        })
        .unwrap();
    let first_seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    shot_seed(&mut world, first, first_seed);
    world
        .btech
        .rewrite_unit_record(second, |record| {
            let unit = record;
            unit["power"] = serde_json::to_value(BattlePower::Running).unwrap();
            unit["motion"]["speed"] = 21.5.into();
            unit["motion"]["desired_speed"] = 21.5.into();
            unit["motion"]["heading"] = 180.0.into();
            unit["motion"]["desired_heading"] = 180.0.into();
            unit["dice"] = serde_json::to_value(second_dice).unwrap();
        })
        .unwrap();
    let rules = BattleChargeRules {
        distance: 2.0,
        new_rules: false,
        technology_level_three: false,
        physical: kick_rules(),
    };
    let baseline = world.clone();
    assert!(resolve_battle_mutual_charge(&mut world, first, second, rules, 2.0).is_err());
    assert_eq!(world.btech, baseline.btech);
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let afterlife = ObjectId(config.battletech.afterlife_dbref);
    scripts.world_mut().objects.remove(&afterlife);
    assert!(
        resolve_battle_mutual_charge_action(&scripts, &config, first, second, rules, 2.0).is_err()
    );
    assert_eq!(scripts.world().btech, baseline.btech);
    assert_eq!(scripts.world().objects[&pilot].location, Some(second));
    assert!(scripts.drain_outbox().is_empty());
    *scripts.world_mut() = baseline;
    let report =
        resolve_battle_mutual_charge_action(&scripts, &config, first, second, rules, 2.0).unwrap();
    for attempt in &report.attempts {
        assert!(attempt.rejection.is_none());
        assert_eq!(attempt.roll, Some(12));
        let collision = attempt.collision.as_ref().unwrap();
        assert!(collision.hit);
        assert!(!collision.target_impacts.is_empty());
        assert_eq!(
            collision.target_impacts.len(),
            usize::from(collision.profile.inflicted_damage.div_ceil(5))
        );
    }
    let first_collision = report.attempts[0].collision.as_ref().unwrap();
    assert!(first_collision.target_impacts[0].impact.destroyed);
    assert_eq!(first_collision.experience.len(), 1);
    assert!(
        report.attempts[1]
            .collision
            .as_ref()
            .unwrap()
            .experience
            .is_empty()
    );
    let candidate = scripts.world().clone();
    assert_eq!(candidate.objects[&pilot].location, Some(afterlife));
    for id in [first, second] {
        assert_eq!(
            candidate.btech.constructed_units()[&id]
                .limb_recycle()
                .len(),
            6
        );
    }
    candidate.validate(&config).unwrap();
    persistence::save(&config.database(), &candidate)
        .await
        .unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, candidate.btech);
    assert_eq!(loaded.objects[&pilot].location, Some(afterlife));
}

/// Ground dispatch retains character charge consequences until the movement action publishes them.
#[tokio::test]
async fn character_charge_movement_dispatch_replays_and_rolls_back() {
    use stompymux_rs::*;
    for mutual in [false, true] {
        let (_dir, config, mut world, first, second) = kick_fixture().await;
        prepare_test_charge(&mut world, first);
        let pilot = ObjectId(2);
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        world.objects.get_mut(&pilot).unwrap().location = Some(second);
        assign_battle_pilot(&mut world, second, pilot).unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        world
            .objects
            .get_mut(&second)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            pilot,
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: 50,
                lethal: 40,
            },
        )
        .unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        let first_seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
            .unwrap();
        shot_seed(&mut world, first, first_seed);
        let second_dice = (0..=u16::MAX)
            .find_map(|seed| {
                let mut bytes = [0; 32];
                bytes[..2].copy_from_slice(&seed.to_le_bytes());
                let dice = BattleDice::seeded(bytes);
                let mut probe = dice.clone();
                ((!mutual || probe.two_d6() == 12) && probe.two_d6() == 12).then_some(dice)
            })
            .unwrap();
        let point = world.btech.constructed_units()[&first]
            .motion()
            .unwrap()
            .point;
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][first.0.to_string()]["charge"] =
            serde_json::json!({"target":second.0,"elapsed":0,"distance":0.0});
        let unit = &mut state["constructed"][second.0.to_string()];
        unit["power"] = serde_json::to_value(BattlePower::Running).unwrap();
        unit["motion"]["point"]["y"] = (point.y - 0.2).into();
        unit["dice"] = serde_json::to_value(second_dice).unwrap();
        if mutual {
            unit["charge"] = serde_json::json!({"target":first.0,"elapsed":0,"distance":0.0});
            unit["motion"]["speed"] = 21.5.into();
            unit["motion"]["desired_speed"] = 21.5.into();
            unit["motion"]["heading"] = 180.0.into();
            unit["motion"]["desired_heading"] = 180.0.into();
        }
        world.btech = serde_json::from_value(state).unwrap();
        world
            .objects
            .get_mut(&first)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        let mut signature = world.btech.constructed_units()[&second].signature();
        signature.team = 1;
        set_battle_unit_signature(&mut world, second, signature).unwrap();
        install_xp_channels(&mut world);
        let xp_before: Vec<_> = [ObjectId(1), pilot]
            .into_iter()
            .map(|player| {
                world
                    .btech
                    .character_values()
                    .get(&player)
                    .and_then(|values| values.get("Piloting-Biped"))
                    .map_or(0, |value| value.experience)
            })
            .collect();
        let baseline = world.clone();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        scripts.world_mut().objects.remove(&afterlife);
        assert!(advance_battle_motion_action(&scripts, &config, RULES).is_err());
        assert_eq!(scripts.world().btech, baseline.btech);
        assert_eq!(scripts.world().objects[&pilot].location, Some(second));
        assert!(scripts.drain_outbox().is_empty());
        assert_eq!(
            serde_json::to_value(&scripts.world().channels).unwrap(),
            serde_json::to_value(&baseline.channels).unwrap()
        );
        persistence::save(&config.database(), &baseline)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for player in [ObjectId(1), pilot] {
            restored.objects.get_mut(&player).unwrap().flags =
                baseline.objects[&player].flags.clone();
        }
        let replay =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        *scripts.world_mut() = baseline;
        advance_battle_motion_action(&scripts, &config, RULES).unwrap();
        advance_battle_motion_action(&replay, &config, RULES).unwrap();
        assert_eq!(
            without_xp_timestamps(&scripts.world().btech),
            without_xp_timestamps(&replay.world().btech)
        );
        let candidate = scripts.world().clone();
        let channel = &candidate.channels["MechPilotXP"];
        assert!(channel.messages >= 2);
        assert_eq!(channel.history.len() as i64, channel.messages);
        let awarded: u32 = channel
            .history
            .iter()
            .map(|entry| {
                assert!(entry.message.contains("Piloting-Biped XP"));
                entry
                    .message
                    .split(" gained ")
                    .nth(1)
                    .unwrap()
                    .split_whitespace()
                    .next()
                    .unwrap()
                    .parse::<u32>()
                    .unwrap()
            })
            .sum();
        let earned: u32 = [ObjectId(1), pilot]
            .into_iter()
            .zip(xp_before)
            .map(|(player, before)| {
                candidate
                    .btech
                    .character_values()
                    .get(&player)
                    .and_then(|values| values.get("Piloting-Biped"))
                    .map_or(0, |value| value.experience)
                    - before
            })
            .sum();
        assert_eq!(awarded, earned);
        assert_eq!(
            channel
                .history
                .iter()
                .map(|entry| &entry.message)
                .collect::<Vec<_>>(),
            replay.world().channels["MechPilotXP"]
                .history
                .iter()
                .map(|entry| &entry.message)
                .collect::<Vec<_>>()
        );

        for id in [first, second] {
            assert_eq!(
                candidate.btech.constructed_units()[&id].charge(),
                BattleChargeState::default()
            );
        }
        assert!(candidate.btech.constructed_units()[&second].is_destroyed());
        assert_eq!(candidate.objects[&pilot].location, Some(afterlife));
        assert_eq!(
            candidate.btech.constructed_units()[&first]
                .limb_recycle()
                .len(),
            6
        );
        if mutual {
            assert_eq!(
                candidate.btech.constructed_units()[&second]
                    .limb_recycle()
                    .len(),
                6
            );
        }
        let count = scripts
            .drain_outbox()
            .iter()
            .filter(|(_, message)| message.source().starts_with("Charge: BTH"))
            .count();
        assert_eq!(count, 1 + usize::from(mutual));
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

/// DFA character effects publish atomically for target head wounds and failed landing protection.
#[tokio::test]
async fn character_dfa_action_hit_miss_and_casualty_rollback() {
    use stompymux_rs::*;
    for extended in [false, true] {
        for hit in [false, true] {
            let (_dir, config, mut world, attacker, target) = kick_fixture().await;
            launch_battle_jump(&mut world, attacker, ObjectId(1), 0, 2.0).unwrap();
            let injured_unit = if hit { target } else { attacker };
            let pilot = if hit { ObjectId(2) } else { ObjectId(1) };
            world.objects.get_mut(&pilot).unwrap().location = Some(injured_unit);
            if hit {
                world
                    .objects
                    .get_mut(&pilot)
                    .unwrap()
                    .flags
                    .remove(Flag::Wizard);
                assign_battle_pilot(&mut world, target, pilot).unwrap();
                support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
            }
            world
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .insert(Flag::Connected);
            world
                .objects
                .get_mut(&injured_unit)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            set_battle_character(
                &mut world,
                pilot,
                BattleCharacter {
                    build: 5,
                    reflexes: if hit { 5 } else { 0 },
                    intuition: if hit { 5 } else { 0 },
                    learn: 5,
                    charisma: 5,
                    bruise: if hit { 50 } else { 0 },
                    lethal: if hit { 40 } else { 0 },
                },
            )
            .unwrap();
            support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
            if !hit {
                for name in ["Piloting-Biped", "Piloting-Battlemech"] {
                    set_battle_character_value(
                        &mut world,
                        pilot,
                        name,
                        BattleCharacterValue {
                            value: 0,
                            experience: 0,
                            last_used: 0,
                        },
                    )
                    .unwrap();
                }
            }
            let seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == if hit { 12 } else { 2 })
                .unwrap();
            shot_seed(&mut world, attacker, seed);
            if hit {
                let seed = (0..=255)
                    .find(|seed| BattleDice::seeded([*seed; 32]).d6() == 6)
                    .unwrap();
                shot_seed(&mut world, target, seed);
            }
            if hit {
                world
                    .objects
                    .get_mut(&attacker)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
                let mut signature = world.btech.constructed_units()[&target].signature();
                signature.team = 1;
                set_battle_unit_signature(&mut world, target, signature).unwrap();
            }
            let mut rules = kick_rules();
            rules.fall.extended_piloting = extended;
            install_xp_channels(&mut world);
            let baseline = world.clone();
            assert!(resolve_battle_dfa(&mut world, attacker, target, rules).is_err());
            assert_eq!(world.btech, baseline.btech);
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let afterlife = ObjectId(config.battletech.afterlife_dbref);
            if hit {
                scripts.world_mut().objects.remove(&afterlife);
                assert!(
                    resolve_battle_dfa_action(&scripts, &config, attacker, target, rules).is_err()
                );
                assert_eq!(scripts.world().btech, baseline.btech);
                assert_eq!(scripts.world().objects[&pilot].location, Some(injured_unit));
                assert!(scripts.drain_outbox().is_empty());
                assert_eq!(
                    serde_json::to_value(&scripts.world().channels).unwrap(),
                    serde_json::to_value(&baseline.channels).unwrap()
                );
                *scripts.world_mut() = baseline.clone();
            }
            let replay =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(baseline))).unwrap();
            let report =
                resolve_battle_dfa_action(&scripts, &config, attacker, target, rules).unwrap();
            assert_eq!(
                without_xp_timestamps(&report),
                without_xp_timestamps(
                    &resolve_battle_dfa_action(&replay, &config, attacker, target, rules).unwrap()
                )
            );
            assert_eq!(
                without_xp_timestamps(&scripts.world().btech),
                without_xp_timestamps(&replay.world().btech)
            );
            assert_eq!(report.hit, hit);
            assert!(report.pilot_injury.is_none());
            if hit {
                assert_eq!(
                    report.target_impacts.len(),
                    usize::from(report.profile.inflicted_damage.div_ceil(5))
                );
                assert!(report.target_impacts[0].impact.destroyed);
                assert_eq!(report.target_impacts[0].impact.character_injuries.len(), 1);
                assert_eq!(scripts.world().objects[&pilot].location, Some(afterlife));
            } else {
                assert!(report.character_injury.is_some());
                assert!(report.target_impacts.is_empty());
                assert_eq!(
                    scripts.world().btech.constructed_units()[&attacker].posture(),
                    BattlePosture::Prone
                );
            }
            let candidate = scripts.world().clone();
            let channel = &candidate.channels["MechPilotXP"];
            assert_eq!(channel.messages as usize, report.experience_messages.len());
            assert_eq!(
                channel.history.len(),
                report
                    .experience
                    .iter()
                    .filter(|award| award.accepted)
                    .count()
            );
            for message in &report.experience_messages {
                assert_eq!(message.channel, BattleChannel::PilotingExperience);
                assert_eq!(
                    message.text,
                    format!(
                        "GOD gained 1 {} XP",
                        if extended {
                            "Piloting-Biped"
                        } else {
                            "Piloting-Battlemech"
                        }
                    )
                );
            }

            if hit {
                assert_eq!(report.experience.len(), 1);
                let skill = if extended {
                    "Piloting-Biped"
                } else {
                    "Piloting-Battlemech"
                };
                assert_eq!(
                    candidate.btech.character_values()[&ObjectId(1)][skill],
                    report.experience[0].after
                );
            } else {
                assert!(report.experience.is_empty());
            }

            assert_eq!(
                candidate.btech.constructed_units()[&attacker]
                    .limb_recycle()
                    .len(),
                6
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
}

/// Character stand commands share ordered health, fatal evacuation, timers and callback rollback.
#[tokio::test]
async fn character_stand_commands_injuries_and_atomic_casualties() {
    use stompymux_rs::*;
    for outcome in 0..3 {
        let (_dir, config, mut world, id) = stand_fixture().await;
        let pilot = ObjectId(2);
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        assign_battle_pilot(&mut world, id, pilot).unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            pilot,
            BattleCharacter {
                build: 5,
                reflexes: 0,
                intuition: 0,
                learn: 5,
                charisma: 5,
                bruise: if outcome == 2 { 50 } else { 0 },
                lethal: if outcome == 2 { 40 } else { 0 },
            },
        )
        .unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        for name in ["Piloting-Biped", "Piloting-Battlemech"] {
            set_battle_character_value(
                &mut world,
                pilot,
                name,
                BattleCharacterValue {
                    value: if outcome == 0 { 30 } else { 0 },
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
        }
        let baseline = world.clone();
        let native =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let lua = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(baseline.clone())),
        )
        .unwrap();
        let call = format!("btech.unit.stand({},2,'anyway')", id.0);
        assert!(
            lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                .is_err()
        );
        assert_eq!(lua.world().btech, baseline.btech);
        assert_eq!(lua.world().objects[&pilot].location, Some(id));
        assert!(lua.drain_outbox().is_empty());
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        if outcome == 2 {
            lua.world_mut().objects.remove(&afterlife);
            assert!(
                lua.eval_callback::<mlua::Table>(&format!("return {call}"))
                    .is_err()
            );
            assert_eq!(lua.world().btech, baseline.btech);
            assert_eq!(lua.world().objects[&pilot].location, Some(id));
            assert!(lua.drain_outbox().is_empty());
            *lua.world_mut() = baseline;
        }
        commands::run(&native, &config, pilot, 1, "stand anyway").unwrap();
        let report: mlua::Table = lua.eval_callback(&format!("return {call}")).unwrap();
        let check: mlua::Table = report.get("check").unwrap();
        assert_eq!(check.get::<bool>("success").unwrap(), outcome == 0);
        assert_eq!(native.world().btech, lua.world().btech);
        let messages = |scripts: &Scripts| {
            scripts
                .drain_outbox()
                .into_iter()
                .map(|(id, message)| (id, message.source().to_owned()))
                .collect::<Vec<_>>()
        };
        assert_eq!(messages(&native), messages(&lua));
        let candidate = native.world().clone();
        let unit = &candidate.btech.constructed_units()[&id];
        assert_eq!(unit.is_destroyed(), outcome == 2);
        assert_eq!(unit.stand_timer().is_none(), outcome == 2);
        assert_eq!(
            candidate.objects[&pilot].location,
            Some(if outcome == 2 { afterlife } else { id })
        );
        if outcome != 0 {
            assert_eq!(unit.posture(), BattlePosture::Prone);
            let fall: mlua::Table = report.get("fall").unwrap();
            assert!(
                fall.get::<Option<mlua::Table>>("character_injury")
                    .unwrap()
                    .is_some()
            );
        }
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

/// Timed character stagger resolves every history mode with private injuries and atomic evacuation.
#[tokio::test]
async fn character_stagger_ticks_replay_and_roll_back_casualties() {
    use stompymux_rs::*;
    for mode in [
        BattleStaggerMode::Traditional,
        BattleStaggerMode::Consume,
        BattleStaggerMode::Retain,
    ] {
        for fatal in [false, true] {
            let (_dir, config, mut world, id) = stagger_fixture().await;
            stagger_hit(&mut world, id, BattleSection::LeftTorso, 20, mode);
            let pilot = ObjectId(2);
            world.objects.get_mut(&pilot).unwrap().location = Some(id);
            world
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .remove(Flag::Wizard);
            world
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .insert(Flag::Connected);
            release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
            assign_battle_pilot(&mut world, id, pilot).unwrap();
            support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            set_battle_character(
                &mut world,
                pilot,
                BattleCharacter {
                    build: 5,
                    reflexes: 0,
                    intuition: 0,
                    learn: 5,
                    charisma: 5,
                    bruise: if fatal { 50 } else { 0 },
                    lethal: if fatal { 40 } else { 0 },
                },
            )
            .unwrap();
            support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
            let rules = BattleStaggerRules {
                interval: 1,
                ..stagger_rules(mode)
            };
            let baseline = world.clone();
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let afterlife = ObjectId(config.battletech.afterlife_dbref);
            if fatal {
                scripts.world_mut().objects.remove(&afterlife);
                assert!(advance_battle_stagger_action(&scripts, &config, rules).is_err());
                assert_eq!(scripts.world().btech, baseline.btech);
                assert_eq!(scripts.world().objects[&pilot].location, Some(id));
                assert!(scripts.drain_outbox().is_empty());
            }
            persistence::save(&config.database(), &baseline)
                .await
                .unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            for player in [ObjectId(1), pilot] {
                restored.objects.get_mut(&player).unwrap().flags =
                    baseline.objects[&player].flags.clone();
            }
            let replay =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
            *scripts.world_mut() = baseline;
            let reports = advance_battle_stagger_action(&scripts, &config, rules).unwrap();
            assert_eq!(
                reports,
                advance_battle_stagger_action(&replay, &config, rules).unwrap()
            );
            assert_eq!(reports.len(), 1);
            let output = scripts.drain_outbox();
            let pilot_output: Vec<_> = output
                .iter()
                .filter(|(who, _)| *who == pilot)
                .map(|(_, text)| text.source())
                .collect();
            let check = reports[0].check;
            let roll_index = pilot_output
                .iter()
                .position(|text| *text == "You make a piloting skill roll!");
            assert_eq!(roll_index.is_some(), check.roll.is_some());
            if let Some(index) = roll_index {
                assert_eq!(pilot_output[index - 1], "You stagger from the damage!");
                assert_eq!(
                    pilot_output[index + 1],
                    format!(
                        "Modified Pilot Skill: BTH {}\tRoll: {}",
                        check.target,
                        check.roll.unwrap()
                    )
                );
                if !check.success {
                    assert_eq!(
                        pilot_output[index + 2],
                        "You fall over from all the damage!"
                    );
                }
            }
            assert!(!output.iter().any(|(who, text)| *who != pilot
                && (text.source().starts_with("You make a piloting")
                    || text.source().starts_with("Modified Pilot Skill:"))));

            assert!(!reports[0].check.success);
            assert!(reports[0].fall.as_ref().unwrap().character_injury.is_some());
            assert_eq!(scripts.world().btech, replay.world().btech);
            let messages = |scripts: &Scripts| {
                scripts
                    .drain_outbox()
                    .into_iter()
                    .map(|(id, message)| (id, message.source().to_owned()))
                    .collect::<Vec<_>>()
            };
            let notices: Vec<_> = output
                .into_iter()
                .map(|(id, message)| (id, message.source().to_owned()))
                .collect();
            assert_eq!(notices, messages(&replay));
            assert_eq!(
                notices
                    .iter()
                    .filter(
                        |(to, message)| *to == pilot && message == "You stagger from the damage!"
                    )
                    .count(),
                1
            );
            let candidate = scripts.world().clone();
            assert_eq!(
                candidate.objects[&pilot].location,
                Some(if fatal { afterlife } else { id })
            );
            assert_eq!(
                candidate.btech.constructed_units()[&id].is_destroyed(),
                fatal
            );
            assert_eq!(
                candidate.btech.constructed_units()[&id].posture(),
                BattlePosture::Prone
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
}

/// Explicit character detonation applies explosion injuries once and restores the spent bin on failure.
#[tokio::test]
async fn character_ammunition_explosion_action_replays_and_rolls_back() {
    use stompymux_rs::*;
    for resistant in [false, true] {
        for fatal in [false, true] {
            let (_dir, config, mut world, id) = fixture('.').await;
            let pilot = ObjectId(2);
            world.objects.get_mut(&pilot).unwrap().location = Some(id);
            world
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .remove(Flag::Wizard);
            world
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .insert(Flag::Connected);
            release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
            assign_battle_pilot(&mut world, id, pilot).unwrap();
            support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            set_battle_character(
                &mut world,
                pilot,
                BattleCharacter {
                    build: 5,
                    reflexes: 5,
                    intuition: 5,
                    learn: 5,
                    charisma: 5,
                    bruise: if fatal { 50 } else { 0 },
                    lethal: if fatal { 40 } else { 0 },
                },
            )
            .unwrap();
            support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
            set_battle_character_value(
                &mut world,
                pilot,
                "Pain_Resistance",
                BattleCharacterValue {
                    value: u8::from(resistant),
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
            world.btech.set_unit_ammunition_bin(id, 0, 1).unwrap();
            let baseline = world.clone();
            assert!(explode_battle_ammunition(&mut world, id, 0, fall_rules()).is_err());
            assert_eq!(world.btech, baseline.btech);
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let afterlife = ObjectId(config.battletech.afterlife_dbref);
            if fatal {
                scripts.world_mut().objects.remove(&afterlife);
                assert!(
                    explode_battle_ammunition_action(&scripts, &config, id, 0, fall_rules())
                        .is_err()
                );
                assert_eq!(scripts.world().btech, baseline.btech);
                assert_eq!(scripts.world().objects[&pilot].location, Some(id));
                assert!(scripts.drain_outbox().is_empty());
            }
            persistence::save(&config.database(), &baseline)
                .await
                .unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            for player in [ObjectId(1), pilot] {
                restored.objects.get_mut(&player).unwrap().flags =
                    baseline.objects[&player].flags.clone();
            }
            let replay =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
            *scripts.world_mut() = baseline;
            let report =
                explode_battle_ammunition_action(&scripts, &config, id, 0, fall_rules()).unwrap();
            assert_eq!(
                report,
                explode_battle_ammunition_action(&replay, &config, id, 0, fall_rules()).unwrap()
            );
            assert_eq!(scripts.world().btech, replay.world().btech);
            assert!(report.pilot_injuries.is_empty());
            assert_eq!(report.impact.character_injuries.len(), 1);
            let candidate = scripts.world().clone();
            assert_eq!(
                candidate.objects[&pilot].location,
                Some(if fatal { afterlife } else { id })
            );
            assert_eq!(candidate.btech.constructed_units()[&id].ammunition(), &[0]);
            assert_eq!(
                candidate.btech.constructed_units()[&id].is_destroyed(),
                fatal
            );
            if !fatal {
                assert_eq!(
                    candidate.btech.constructed_units()[&id]
                        .character_pilot_status()
                        .unwrap()
                        .injuries,
                    if resistant { 1 } else { 2 }
                );
            }
            scripts.drain_outbox();
            assert!(
                explode_battle_ammunition_action(&scripts, &config, id, 0, fall_rules()).is_err()
            );
            assert_eq!(scripts.world().btech, candidate.btech);
            assert!(scripts.drain_outbox().is_empty());
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
}

/// Thermal character casualties preserve phase ordering and retry clocks; shutdown falls share character resolution.
#[tokio::test]
async fn character_thermal_action_orders_casualties_and_shutdown_falls() {
    use stompymux_rs::*;
    for cause in ["heat", "ammunition", "ground", "air"] {
        let (_dir, config, mut world, id) = fixture('.').await;
        // This ordering scenario requires a surviving shutdown fall; fix its hit-location stream.
        shot_seed(&mut world, id, 7);
        if cause == "air" {
            launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
        }
        let fatal = matches!(cause, "heat" | "ammunition");
        let pilot = ObjectId(2);
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        assign_battle_pilot(&mut world, id, pilot).unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            pilot,
            BattleCharacter {
                build: 5,
                reflexes: 0,
                intuition: 0,
                learn: 0,
                charisma: 5,
                bruise: if fatal { 50 } else { 0 },
                lethal: if fatal { 40 } else { 0 },
            },
        )
        .unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        if cause == "heat" {
            destroy_battle_critical(
                &mut world,
                id,
                CriticalLocation {
                    section: BattleSection::Head,
                    slot: 0,
                },
            )
            .unwrap();
        }
        world
            .btech
            .rewrite_unit_record(id, |record| {
                record["ammunition"][0] = 1.into();
                if cause == "ground" {
                    record["motion"]["speed"] = 21.5.into();
                    record["motion"]["desired_speed"] = 21.5.into();
                }
            })
            .unwrap();
        if cause == "ammunition" {
            let seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 2)
                .unwrap();
            shot_seed(&mut world, id, seed);
        }
        overheat_due(
            &mut world,
            id,
            if fatal { 26.0 } else { 14.0 },
            cause == "heat",
        );
        let baseline = world.clone();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        if fatal {
            scripts.world_mut().objects.remove(&afterlife);
            assert!(advance_battle_overheat_action(&scripts, &config, overheat_rules()).is_err());
            assert_eq!(scripts.world().btech, baseline.btech);
            assert_eq!(scripts.world().objects[&pilot].location, Some(id));
            assert!(scripts.drain_outbox().is_empty());
        }
        let replay = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(baseline.clone())),
        )
        .unwrap();
        *scripts.world_mut() = baseline;
        let reports = advance_battle_overheat_action(&scripts, &config, overheat_rules()).unwrap();
        assert_eq!(
            reports,
            advance_battle_overheat_action(&replay, &config, overheat_rules()).unwrap()
        );
        assert_eq!(scripts.world().btech, replay.world().btech);
        assert_eq!(reports.len(), 1);
        let report = &reports[0];
        assert!(report.injury.is_none());
        if cause == "heat" {
            assert!(report.character_injury.is_some());
            assert!(report.ammunition_check.is_none());
            assert!(report.shutdown_check.is_none());
        } else if cause == "ammunition" {
            assert_eq!(
                report
                    .explosion
                    .as_ref()
                    .unwrap()
                    .impact
                    .character_injuries
                    .len(),
                1
            );
            assert!(report.shutdown_check.is_none());
        } else {
            assert!(report.shutdown);
            let fall = report.fall.as_ref().unwrap();
            if cause == "ground" {
                assert_eq!(fall.damage, 0);
            } else {
                assert!(fall.damage > 0);
            }
        }
        let candidate = scripts.world().clone();
        assert_eq!(
            candidate.objects[&pilot].location,
            Some(if fatal { afterlife } else { id })
        );
        assert_eq!(
            candidate.btech.constructed_units()[&id].is_destroyed(),
            fatal
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

/// Server thermal dispatch retries fatal character heat damage and crew movement after a failed database commit.
#[tokio::test]
async fn character_thermal_server_retries_fatal_heat_commit() {
    use sqlx::Connection;
    use stompymux_rs::*;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world, id) = fixture('.').await;
        let pilot = ObjectId(2);
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        world.objects.get_mut(&pilot).unwrap().flags.remove(Flag::Wizard);
        release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        assign_battle_pilot(&mut world, id, pilot).unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        world.objects.get_mut(&id).unwrap().flags.insert(Flag::InCharacter);
        set_battle_character(&mut world, pilot, BattleCharacter {
            build: 5, reflexes: 5, intuition: 5, learn: 5, charisma: 5, bruise: 50, lethal: 40,
        }).unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        destroy_battle_critical(&mut world, id, CriticalLocation { section: BattleSection::Head, slot: 0 }).unwrap();
        overheat_due(&mut world, id, 26.0, false);
        world.btech
            .rewrite_unit_record(id, |record| {
        record["overheat_clock"] = serde_json::json!({"elapsed":29,"phase":29,"injury_due":false});
        })
            .unwrap();
        persistence::save(&config.database(), &world).await.unwrap();
        let baseline = persistence::load(&config.database()).await.unwrap();
        let mut expected = baseline.clone();
        // The successful retry commits exactly one global turn phase; rejected ticks remain at zero.
        let mut phase_state = serde_json::to_value(&expected.btech).unwrap();
        phase_state["turn_clock"] = 1.into();
        phase_state["simulation_seconds"] = 1.into();
        expected.btech = serde_json::from_value(phase_state).unwrap();
        advance_battle_reactor_windows(&mut expected);
        advance_battle_heat(&mut expected);
        let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(expected))).unwrap();
        let reports = advance_battle_overheat_action(&scripts, &config, overheat_rules()).unwrap();
        assert_eq!(reports.len(), 1);
        assert!(reports[0].character_injury.is_some());
        let expected = scripts.world().clone();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        assert_eq!(expected.objects[&pilot].location, Some(afterlife));
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        sqlx::query("CREATE TRIGGER reject_character_heat BEFORE UPDATE ON btech_units WHEN json_extract(NEW.unit,'$.character_pilot.killed') = 1 BEGIN SELECT RAISE(ABORT,'character heat save failure'); END").execute(&mut sql).await.unwrap();
        let (_address, shutdown, task, _lua, mut heartbeats) = support::start(&config, std::rc::Rc::new(std::cell::Cell::new(1))).await;
        heartbeats.attempt().await;
        let rejected = persistence::load(&config.database()).await.unwrap();
        assert_eq!(rejected.btech, baseline.btech);
        assert_eq!(rejected.objects[&pilot].location, Some(id));
        sqlx::query("DROP TRIGGER reject_character_heat").execute(&mut sql).await.unwrap();
        let saved = heartbeats.until_saved(&config, 5, |saved| saved.objects[&pilot].location == Some(afterlife)).await;
        assert_eq!(saved.btech, expected.btech);
        assert!(saved.btech.constructed_units()[&id].is_destroyed());
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}

/// Character salvo groups stop on pilot death and restore cluster/location dice after failed evacuation.
#[tokio::test]
async fn character_salvo_action_stops_on_casualty_and_replays() {
    use stompymux_rs::*;
    for fatal in [false, true] {
        let (_dir, config, mut world, id) = fixture('.').await;
        let pilot = ObjectId(2);
        world.objects.get_mut(&pilot).unwrap().location = Some(id);
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .remove(Flag::Wizard);
        world
            .objects
            .get_mut(&pilot)
            .unwrap()
            .flags
            .insert(Flag::Connected);
        release_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        assign_battle_pilot(&mut world, id, pilot).unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            pilot,
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: if fatal { 50 } else { 0 },
                lethal: if fatal { 40 } else { 0 },
            },
        )
        .unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        let dice = (0..=u16::MAX)
            .find_map(|seed| {
                let mut bytes = [0; 32];
                bytes[..2].copy_from_slice(&seed.to_le_bytes());
                let dice = BattleDice::seeded(bytes);
                let mut probe = dice.clone();
                (probe.two_d6() == 12 && probe.two_d6() == 12).then_some(dice)
            })
            .unwrap();
        world.btech.set_unit_dice(id, dice).unwrap();
        let baseline = world.clone();
        assert!(
            resolve_battle_tactical_salvo(
                &mut world,
                id,
                BattleWeapon::Srm4,
                BattleHitArc::Front,
                fall_rules()
            )
            .is_err()
        );
        assert_eq!(world.btech, baseline.btech);
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        if fatal {
            scripts.world_mut().objects.remove(&afterlife);
            assert!(
                resolve_battle_salvo_action(
                    &scripts,
                    &config,
                    id,
                    BattleWeapon::Srm4,
                    BattleHitArc::Front,
                    fall_rules()
                )
                .is_err()
            );
            assert_eq!(scripts.world().btech, baseline.btech);
            assert_eq!(scripts.world().objects[&pilot].location, Some(id));
            assert!(scripts.drain_outbox().is_empty());
        }
        persistence::save(&config.database(), &baseline)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for player in [ObjectId(1), pilot] {
            restored.objects.get_mut(&player).unwrap().flags =
                baseline.objects[&player].flags.clone();
        }
        let replay =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        *scripts.world_mut() = baseline;
        let report = resolve_battle_salvo_action(
            &scripts,
            &config,
            id,
            BattleWeapon::Srm4,
            BattleHitArc::Front,
            fall_rules(),
        )
        .unwrap();
        assert_eq!(
            report,
            resolve_battle_salvo_action(
                &replay,
                &config,
                id,
                BattleWeapon::Srm4,
                BattleHitArc::Front,
                fall_rules()
            )
            .unwrap()
        );
        assert_eq!(scripts.world().btech, replay.world().btech);
        assert_eq!(report.cluster_roll, Some(12));
        assert_eq!(report.groups.len(), if fatal { 1 } else { 4 });
        assert_eq!(report.groups[0].hit.section, BattleSection::Head);
        assert_eq!(report.groups[0].impact.character_injuries.len(), 1);
        assert!(
            report
                .groups
                .iter()
                .all(|group| group.pilot_injuries.is_empty())
        );
        let candidate = scripts.world().clone();
        assert_eq!(
            candidate.objects[&pilot].location,
            Some(if fatal { afterlife } else { id })
        );
        assert_eq!(
            candidate.btech.constructed_units()[&id].is_destroyed(),
            fatal
        );
        assert!(
            candidate.btech.constructed_units()[&id].sections()[&BattleSection::Head].internal > 0
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

/// Complete character shots keep expenditure and all target consequences inside the casualty transaction.
#[tokio::test]
async fn character_shot_action_rolls_back_expenditure_and_replays() {
    use stompymux_rs::*;
    for classic in [false, true] {
        for missile in [false, true] {
            for fatal in [false, true] {
                let (_dir, _config, mut world, shooter, target) = shot_fixture().await;
                let config = if classic {
                    classic_shot_config(_dir.path())
                } else {
                    _config
                };
                shot_skill(&mut world, 0);
                let index = world.btech.constructed_units()[&shooter]
                    .loadout()
                    .unwrap()
                    .weapons
                    .iter()
                    .position(|mount| {
                        mount.weapon
                            == if missile {
                                BattleWeapon::Srm4
                            } else {
                                BattleWeapon::MediumLaser
                            }
                    })
                    .unwrap();
                let pilot = ObjectId(2);
                world.objects.get_mut(&pilot).unwrap().location = Some(target);
                world
                    .objects
                    .get_mut(&pilot)
                    .unwrap()
                    .flags
                    .remove(Flag::Wizard);
                world
                    .objects
                    .get_mut(&pilot)
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
                assign_battle_pilot(&mut world, target, pilot).unwrap();
                support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
                for unit in [shooter, target] {
                    world
                        .objects
                        .get_mut(&unit)
                        .unwrap()
                        .flags
                        .insert(Flag::InCharacter);
                }
                set_battle_character(
                    &mut world,
                    pilot,
                    BattleCharacter {
                        build: 5,
                        reflexes: 5,
                        intuition: 5,
                        learn: 5,
                        charisma: 5,
                        bruise: if fatal { 50 } else { 0 },
                        lethal: if fatal { 40 } else { 0 },
                    },
                )
                .unwrap();
                support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
                if !classic {
                    train_battle_value_shot_crews(&mut world, pilot);
                }
                let seed = (0..=255)
                    .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
                    .unwrap();
                shot_seed(&mut world, shooter, seed);
                let dice = (0..=u16::MAX)
                    .find_map(|seed| {
                        let mut bytes = [0; 32];
                        bytes[..2].copy_from_slice(&seed.to_le_bytes());
                        let dice = BattleDice::seeded(bytes);
                        let mut probe = dice.clone();
                        ((!missile || probe.two_d6() == 12) && probe.two_d6() == 12).then_some(dice)
                    })
                    .unwrap();
                world
                    .btech
                    .rewrite_unit_record(target, |record| {
                        record["signature"]["team"] = 1.into();
                        record["dice"] = serde_json::to_value(dice).unwrap();
                    })
                    .unwrap();
                let baseline = world.clone();
                assert!(
                    resolve_battle_shot(
                        &mut world,
                        shooter,
                        ObjectId(1),
                        target,
                        index,
                        shot_rules()
                    )
                    .is_err()
                );
                assert_eq!(world.btech, baseline.btech);
                let scripts =
                    Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world)))
                        .unwrap();
                let afterlife = ObjectId(config.battletech.afterlife_dbref);
                if fatal {
                    scripts.world_mut().objects.remove(&afterlife);
                    assert!(
                        resolve_battle_shot_action(
                            &scripts,
                            &config,
                            shooter,
                            ObjectId(1),
                            target,
                            index,
                            shot_rules()
                        )
                        .is_err()
                    );
                    assert_eq!(scripts.world().btech, baseline.btech);
                    assert_eq!(scripts.world().objects[&pilot].location, Some(target));
                    assert!(scripts.drain_outbox().is_empty());
                }
                persistence::save(&config.database(), &baseline)
                    .await
                    .unwrap();
                let mut restored = persistence::load(&config.database()).await.unwrap();
                for player in [ObjectId(1), pilot] {
                    restored.objects.get_mut(&player).unwrap().flags =
                        baseline.objects[&player].flags.clone();
                }
                let replay =
                    Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored)))
                        .unwrap();
                *scripts.world_mut() = baseline.clone();
                let report = resolve_battle_shot_action(
                    &scripts,
                    &config,
                    shooter,
                    ObjectId(1),
                    target,
                    index,
                    shot_rules(),
                )
                .unwrap();
                assert_eq!(
                    without_xp_timestamps(&report),
                    without_xp_timestamps(
                        &resolve_battle_shot_action(
                            &replay,
                            &config,
                            shooter,
                            ObjectId(1),
                            target,
                            index,
                            shot_rules()
                        )
                        .unwrap()
                    )
                );
                assert_eq!(
                    without_xp_timestamps(&scripts.world().btech),
                    without_xp_timestamps(&replay.world().btech)
                );
                assert!(report.launched);
                let salvo = report.salvo.as_ref().unwrap().as_mech().unwrap();
                assert_eq!(salvo.groups.len(), if missile && !fatal { 4 } else { 1 });
                assert_eq!(salvo.experience.len(), salvo.groups.len());
                assert!(salvo.experience.iter().all(|attempt| {
                    attempt
                        .as_ref()
                        .is_some_and(|attempt| attempt.amount().is_some())
                }));
                let mut expected_dice: BattleDice =
                    serde_json::from_value(
                        serde_json::to_value(&baseline.btech.constructed_units()[&shooter])
                            .unwrap()["dice"]
                            .clone(),
                    )
                    .unwrap();
                assert_eq!(expected_dice.two_d6(), report.roll);
                for attempt in &salvo.experience {
                    let attempt = attempt.as_ref().unwrap();
                    if classic {
                        assert!(matches!(attempt, BattleShotExperienceAward::Classic(_)));
                        assert_eq!(Some(expected_dice.die(50).unwrap() as u8), attempt.roll());
                    } else {
                        assert!(matches!(attempt, BattleShotExperienceAward::BattleValue(_)));
                        assert_eq!(attempt.roll(), None);
                    }
                }
                assert_eq!(
                    serde_json::to_value(&scripts.world().btech.constructed_units()[&shooter])
                        .unwrap()["dice"],
                    serde_json::to_value(expected_dice).unwrap()
                );
                if !classic {
                    let difficulties: Vec<_> = salvo
                        .experience
                        .iter()
                        .map(|attempt| {
                            let BattleShotExperienceAward::BattleValue(report) =
                                attempt.as_ref().unwrap()
                            else {
                                panic!("Wrong formula")
                            };
                            report.calculation.difficulty.unwrap()
                        })
                        .collect();
                    if missile && !fatal {
                        assert!(difficulties[0] > difficulties[3]);
                    }
                }
                assert_eq!(salvo.groups[0].hit.section, BattleSection::Head);
                assert_eq!(salvo.groups[0].impact.character_injuries.len(), 1);
                let candidate = scripts.world().clone();
                assert!(
                    candidate.btech.constructed_units()[&shooter]
                        .weapon_recycle()
                        .contains_key(&index)
                );
                for draw in &report.expenditure.ammunition {
                    assert_eq!(
                        candidate.btech.constructed_units()[&shooter].ammunition()[draw.bin_index],
                        baseline.btech.constructed_units()[&shooter].ammunition()[draw.bin_index]
                            - draw.rounds
                    );
                }
                assert_eq!(
                    candidate.objects[&pilot].location,
                    Some(if fatal { afterlife } else { target })
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
    }
}

/// Native and Lua character firing retain aim feedback and atomically publish casualties once.
#[tokio::test]
async fn character_firing_commands_match_and_rollback() {
    use stompymux_rs::*;
    for classic in [false, true] {
        for missile in [false, true] {
            for fatal in [false, true] {
                let (_dir, _config, mut world, shooter, target) = shot_fixture().await;
                let _selected_config = if classic {
                    classic_shot_config(_dir.path())
                } else {
                    _config
                };
                let config = noisy_shot_config(_dir.path());
                shot_skill(&mut world, 0);
                let index = world.btech.constructed_units()[&shooter]
                    .loadout()
                    .unwrap()
                    .weapons
                    .iter()
                    .position(|mount| {
                        mount.weapon
                            == if missile {
                                BattleWeapon::Srm4
                            } else {
                                BattleWeapon::MediumLaser
                            }
                    })
                    .unwrap();
                let pilot = ObjectId(2);
                world.objects.get_mut(&pilot).unwrap().location = Some(target);
                world
                    .objects
                    .get_mut(&pilot)
                    .unwrap()
                    .flags
                    .remove(Flag::Wizard);
                world
                    .objects
                    .get_mut(&pilot)
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
                assign_battle_pilot(&mut world, target, pilot).unwrap();
                support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
                for unit in [shooter, target] {
                    world
                        .objects
                        .get_mut(&unit)
                        .unwrap()
                        .flags
                        .insert(Flag::InCharacter);
                }
                set_battle_character(
                    &mut world,
                    pilot,
                    BattleCharacter {
                        build: 5,
                        reflexes: 5,
                        intuition: 5,
                        learn: 5,
                        charisma: 5,
                        bruise: if fatal { 50 } else { 0 },
                        lethal: if fatal { 40 } else { 0 },
                    },
                )
                .unwrap();
                support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
                if !classic {
                    train_battle_value_shot_crews(&mut world, pilot);
                }
                let seed = (0..=255)
                    .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
                    .unwrap();
                shot_seed(&mut world, shooter, seed);
                let dice = (0..=u16::MAX)
                    .find_map(|seed| {
                        let mut bytes = [0; 32];
                        bytes[..2].copy_from_slice(&seed.to_le_bytes());
                        let dice = BattleDice::seeded(bytes);
                        let mut probe = dice.clone();
                        let cluster = !missile || probe.two_d6() == 12;
                        let location = probe.two_d6();
                        (cluster && location == 12).then_some(dice)
                    })
                    .unwrap();
                world
                    .btech
                    .rewrite_unit_record(target, |record| {
                        record["signature"]["team"] = 1.into();
                        record["dice"] = serde_json::to_value(dice).unwrap();
                    })
                    .unwrap();

                install_xp_channels(&mut world);
                let baseline = world.clone();
                let native =
                    Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world)))
                        .unwrap();
                let lua = Scripts::new(
                    &config,
                    std::rc::Rc::new(std::cell::RefCell::new(baseline.clone())),
                )
                .unwrap();
                let call = format!("btech.unit.fire({},1,{},{})", shooter.0, index, target.0);
                assert!(
                    lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                        .is_err()
                );
                assert_eq!(lua.world().btech, baseline.btech);
                assert_eq!(lua.world().objects[&pilot].location, Some(target));
                assert!(lua.drain_outbox().is_empty());
                assert_eq!(
                    serde_json::to_value(&lua.world().channels).unwrap(),
                    serde_json::to_value(&baseline.channels).unwrap()
                );
                lua.world_mut()
                    .channels
                    .get_mut("MechAttackXP")
                    .unwrap()
                    .messages = i64::MAX;
                assert!(
                    lua.eval_callback::<mlua::Table>(&format!("return {call}"))
                        .is_err()
                );
                assert_eq!(lua.world().btech, baseline.btech);
                assert_eq!(lua.world().channels["MechAttackXP"].messages, i64::MAX);
                assert!(lua.world().channels["MechAttackXP"].history.is_empty());
                assert!(lua.drain_outbox().is_empty());
                *lua.world_mut() = baseline.clone();
                let afterlife = ObjectId(config.battletech.afterlife_dbref);
                if fatal {
                    lua.world_mut().objects.remove(&afterlife);
                    assert!(
                        lua.eval_callback::<mlua::Table>(&format!("return {call}"))
                            .is_err()
                    );
                    assert_eq!(lua.world().btech, baseline.btech);
                    assert_eq!(lua.world().objects[&pilot].location, Some(target));
                    assert!(lua.drain_outbox().is_empty());
                    assert_eq!(
                        serde_json::to_value(&lua.world().channels).unwrap(),
                        serde_json::to_value(&baseline.channels).unwrap()
                    );
                    *lua.world_mut() = baseline;
                }
                commands::run(
                    &native,
                    &config,
                    ObjectId(1),
                    1,
                    &format!("fire {index} #{}", target.0),
                )
                .unwrap();
                let report: mlua::Table = lua.eval_callback(&format!("return {call}")).unwrap();
                assert!(report.get::<bool>("launched").unwrap());
                let salvo: mlua::Table = report
                    .get::<mlua::Table>("salvo")
                    .unwrap()
                    .get("report")
                    .unwrap();
                let attempts: mlua::Table = salvo.get("experience").unwrap();
                assert_eq!(attempts.raw_len(), if missile && !fatal { 4 } else { 1 });
                let first: mlua::Table = attempts.get(1).unwrap();
                assert!(first.get::<u32>("amount").unwrap() > 0);
                assert_eq!(
                    first.get::<String>("formula").unwrap(),
                    if classic { "classic" } else { "battle_value" }
                );

                assert_eq!(
                    without_xp_timestamps(&native.world().btech),
                    without_xp_timestamps(&lua.world().btech)
                );
                let messages = |scripts: &Scripts| {
                    scripts
                        .drain_outbox()
                        .into_iter()
                        .map(|(to, message)| (to, message.source().to_owned()))
                        .collect::<Vec<_>>()
                };
                let accepted = attempts
                    .sequence_values::<mlua::Table>()
                    .map(|attempt| {
                        attempt
                            .unwrap()
                            .get::<mlua::Table>("award")
                            .unwrap()
                            .get::<bool>("accepted")
                            .unwrap()
                    })
                    .filter(|accepted| *accepted)
                    .count();
                assert!(accepted > 0);
                for (name, count) in [
                    ("MechAttackXP", accepted),
                    ("MechXP", if classic { 0 } else { accepted }),
                ] {
                    let state = native.world();
                    let channel = &state.channels[name];
                    assert_eq!(channel.messages as usize, count);
                    assert_eq!(channel.history.len(), count);
                    assert_eq!(
                        channel
                            .history
                            .iter()
                            .map(|entry| &entry.message)
                            .collect::<Vec<_>>(),
                        lua.world().channels[name]
                            .history
                            .iter()
                            .map(|entry| &entry.message)
                            .collect::<Vec<_>>()
                    );
                }
                let notices = messages(&native);
                assert_eq!(notices, messages(&lua));
                let diagnostic: Vec<_> = notices
                    .iter()
                    .filter(|(_, text)| {
                        text.starts_with("[[MechAttackXP]") || text.starts_with("[[MechXP]")
                    })
                    .collect();
                assert_eq!(diagnostic.len(), accepted * if classic { 1 } else { 2 });
                assert!(diagnostic.iter().all(|(to, _)| *to == ObjectId(1)));

                assert_eq!(
                    notices
                        .iter()
                        .filter(|(to, text)| *to == ObjectId(1) && text.starts_with("You fire "))
                        .count(),
                    1
                );
                assert!(
                !notices
                    .iter()
                    .any(|(to, text)| *to == ObjectId(1) && text.contains("keep consciousness"))
            );
                let candidate = native.world().clone();
                assert_eq!(
                    candidate.objects[&pilot].location,
                    Some(if fatal { afterlife } else { target })
                );
                candidate.validate(&config).unwrap();
                persistence::save(&config.database(), &candidate)
                    .await
                    .unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    candidate.btech
                );
                let restored = persistence::load(&config.database()).await.unwrap();
                assert_eq!(
                    serde_json::to_value(&restored.channels).unwrap(),
                    serde_json::to_value(&candidate.channels).unwrap()
                );
            }
        }
    }
}

/// Character recoil injuries belong to the shooter and roll back expenditure on fatal evacuation failure.
#[tokio::test]
async fn character_heavy_gauss_recoil_uses_shooter_health_and_toughness() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    let mut definition = base.btech.constructed_units()[&id].definition().clone();
    let torso = definition
        .sections
        .get_mut(&BattleSection::LeftTorso)
        .unwrap();
    let mut part = torso.criticals[&0].clone();
    part.equipment = "IS.HeavyGaussRifle".into();
    for slot in 0..11 {
        torso.criticals.insert(slot, part.clone());
    }
    let bin = definition
        .sections
        .values_mut()
        .flat_map(|s| s.criticals.values_mut())
        .find(|part| part.equipment.starts_with("Ammo_"))
        .unwrap();
    bin.equipment = "Ammo_IS.HeavyGaussRifle".into();
    bin.data = "4".into();
    base.btech
        .rewrite_unit_record(id, |record| {
            record["definition"] = serde_json::to_value(definition).unwrap();
            record["ammunition"][0] = 4.into();
        })
        .unwrap();
    let index = base.btech.constructed_units()[&id]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == BattleWeapon::HeavyGaussRifle)
        .unwrap();

    let pilot = ObjectId(2);
    base.objects.get_mut(&pilot).unwrap().location = Some(id);
    base.objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    base.objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    release_battle_pilot(&mut base, id, ObjectId(1)).unwrap();
    assign_battle_pilot(&mut base, id, pilot).unwrap();
    support::seed_object_dice(&mut base, pilot, support::FIXTURE_DICE_SEED);
    base.objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            dice.two_d6();
            dice.two_d6() == 2
        })
        .unwrap();
    shot_seed(&mut base, id, seed);
    base.btech
        .edit_unit_motion(id, |motion| {
            motion.speed = 1.0;
        })
        .unwrap();
    for fatal in [false, true] {
        let mut world = base.clone();
        set_battle_character(
            &mut world,
            pilot,
            BattleCharacter {
                build: 5,
                reflexes: 0,
                intuition: 0,
                learn: 0,
                charisma: 5,
                bruise: if fatal { 50 } else { 0 },
                lethal: if fatal { 40 } else { 0 },
            },
        )
        .unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        let baseline = world.clone();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        let rules = BattleShotRules {
            range_damage: false,
            tsm_tow_bonus: true,
            target_toughness: true,
            ..shot_rules()
        };
        if fatal {
            scripts.world_mut().objects.remove(&afterlife);
            assert!(
                resolve_battle_shot_action(&scripts, &config, id, pilot, target, index, rules)
                    .is_err()
            );
            assert_eq!(scripts.world().btech, baseline.btech);
            assert_eq!(scripts.world().objects[&pilot].location, Some(id));
            assert!(scripts.drain_outbox().is_empty());
        }
        persistence::save(&config.database(), &baseline)
            .await
            .unwrap();
        let mut restored = persistence::load(&config.database()).await.unwrap();
        for player in [ObjectId(1), pilot] {
            restored.objects.get_mut(&player).unwrap().flags =
                baseline.objects[&player].flags.clone();
        }
        let replay =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(restored))).unwrap();
        *scripts.world_mut() = baseline.clone();
        let report =
            resolve_battle_shot_action(&scripts, &config, id, pilot, target, index, rules).unwrap();
        // Target Toughness must not protect the shooter from recoil.
        assert_eq!(
            report,
            resolve_battle_shot_action(&replay, &config, id, pilot, target, index, shot_rules())
                .unwrap()
        );
        assert_eq!(scripts.world().btech, replay.world().btech);
        assert!(report.salvo.is_none());
        let recoil = report.recoil.as_ref().unwrap();
        assert_eq!(recoil.check.roll, Some(2));
        assert!(!recoil.check.success);
        let fall = recoil.fall.as_ref().unwrap();
        assert!(fall.character_injury.is_some());
        assert!(fall.pilot_injury.is_none());
        let candidate = scripts.world().clone();
        assert_eq!(
            candidate.btech.constructed_units()[&target],
            baseline.btech.constructed_units()[&target]
        );
        assert_eq!(candidate.btech.constructed_units()[&id].ammunition()[0], 3);
        assert_eq!(
            candidate.btech.constructed_units()[&id].posture(),
            BattlePosture::Prone
        );
        assert_eq!(
            candidate.objects[&pilot].location,
            Some(if fatal { afterlife } else { id })
        );
        assert_eq!(
            candidate.btech.constructed_units()[&id].is_destroyed(),
            fatal
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

/// A head-mounted rapid-fire misload applies shooter injury and restores weapon/bin state if evacuation fails.
#[tokio::test]
async fn character_misload_rolls_back_weapon_and_shooter_casualty() {
    use stompymux_rs::*;
    let (_dir, config, mut base, id, target) = shot_fixture().await;
    let mut definition = base.btech.constructed_units()[&id].definition().clone();
    let mut gun = definition.sections[&BattleSection::LeftArm].criticals[&2].clone();
    gun.equipment = "IS.AC/2".into();
    gun.modes = vec!["RapidFire".into()];
    definition
        .sections
        .get_mut(&BattleSection::Head)
        .unwrap()
        .criticals
        .insert(3, gun);
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
        .position(|mount| mount.weapon == BattleWeapon::Ac2)
        .unwrap();
    let pilot = ObjectId(2);
    base.objects.get_mut(&pilot).unwrap().location = Some(id);
    base.objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .remove(Flag::Wizard);
    base.objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    release_battle_pilot(&mut base, id, ObjectId(1)).unwrap();
    assign_battle_pilot(&mut base, id, pilot).unwrap();
    support::seed_object_dice(&mut base, pilot, support::FIXTURE_DICE_SEED);
    base.objects
        .get_mut(&id)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    assert_eq!(
        toggle_battle_rapid(&mut base, id, pilot, index).unwrap(),
        BattleFireMode::Rapid
    );
    let seed = (0..=255)
        .find(|seed| {
            let mut dice = BattleDice::seeded([*seed; 32]);
            dice.two_d6() == 2 && dice.two_d6() <= 7
        })
        .unwrap();
    shot_seed(&mut base, id, seed);
    for fatal in [false, true] {
        let mut world = base.clone();
        set_battle_character(
            &mut world,
            pilot,
            BattleCharacter {
                build: 5,
                reflexes: 5,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: if fatal { 50 } else { 0 },
                lethal: if fatal { 40 } else { 0 },
            },
        )
        .unwrap();
        support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
        let baseline = world.clone();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let afterlife = ObjectId(config.battletech.afterlife_dbref);
        if fatal {
            scripts.world_mut().objects.remove(&afterlife);
            assert!(
                resolve_battle_shot_action(
                    &scripts,
                    &config,
                    id,
                    pilot,
                    target,
                    index,
                    shot_rules()
                )
                .is_err()
            );
            assert_eq!(scripts.world().btech, baseline.btech);
            assert_eq!(scripts.world().objects[&pilot].location, Some(id));
            assert!(scripts.drain_outbox().is_empty());
        }
        *scripts.world_mut() = baseline.clone();
        let replay = Scripts::new(
            &config,
            std::rc::Rc::new(std::cell::RefCell::new(baseline.clone())),
        )
        .unwrap();
        let report =
            resolve_battle_shot_action(&scripts, &config, id, pilot, target, index, shot_rules())
                .unwrap();
        assert_eq!(
            report,
            resolve_battle_shot_action(&replay, &config, id, pilot, target, index, shot_rules())
                .unwrap()
        );
        assert!(!report.launched);
        assert!(report.loader_destroyed);
        let impact = report.misload.as_ref().unwrap();
        assert_eq!(impact.impact.character_injuries.len(), 1);
        assert!(impact.pilot_injuries.is_empty());
        let candidate = scripts.world().clone();
        assert_eq!(candidate.btech, replay.world().btech);
        assert_eq!(candidate.btech.constructed_units()[&id].ammunition()[0], 43);
        assert_eq!(
            candidate.btech.constructed_units()[&id].is_destroyed(),
            fatal
        );
        assert_eq!(
            candidate.btech.constructed_units()[&target],
            baseline.btech.constructed_units()[&target]
        );
        assert_eq!(
            candidate.objects[&pilot].location,
            Some(if fatal { afterlife } else { id })
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

/// XP policy persists independently of balances, and classic/battle-value eligibility retain their distinct guards.
#[tokio::test]
async fn gunnery_experience_policy_persists_and_checks_eligibility() {
    use stompymux_rs::*;
    let (_dir, config, mut base, attacker, target) = shot_fixture().await;
    for unit in [attacker, target] {
        base.objects
            .get_mut(&unit)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
    }
    base.btech
        .rewrite_unit_record(target, |record| {
            record["signature"]["team"] = 1.into();
        })
        .unwrap();
    let pilot = ObjectId(1);
    assert_eq!(
        base.btech.constructed_units()[&attacker].experience_settings(),
        BattleUnitExperience::default()
    );
    for case in [
        "eligible",
        "friendly",
        "tactical",
        "absent",
        "disconnected",
        "wrong_pilot",
        "dead",
        "suppressed",
        "sure",
        "impossible",
        "self",
    ] {
        let mut world = base.clone();
        let mut aim = 7;
        let mut selected_pilot = pilot;
        let mut selected_target = target;
        match case {
            "friendly" => {
                world
                    .btech
                    .rewrite_unit_record(target, |record| {
                        record["signature"]["team"] = 0.into();
                    })
                    .unwrap();
            }
            "tactical" => {
                world
                    .objects
                    .get_mut(&target)
                    .unwrap()
                    .flags
                    .remove(Flag::InCharacter);
            }
            "absent" => {
                world.objects.get_mut(&pilot).unwrap().location = Some(target);
            }
            "disconnected" => {
                world
                    .objects
                    .get_mut(&pilot)
                    .unwrap()
                    .flags
                    .remove(Flag::Connected);
            }
            "wrong_pilot" => {
                selected_pilot = ObjectId(2);
            }
            "dead" => {
                world
                    .btech
                    .rewrite_unit_record(target, |record| {
                        record["sections"]["Head"]["internal"] = 0.into();
                    })
                    .unwrap();
            }
            "suppressed" => {
                set_battle_unit_experience(
                    &mut world,
                    target,
                    BattleUnitExperience {
                        suppress_gunnery: true,
                        ..Default::default()
                    },
                )
                .unwrap();
            }
            "sure" => {
                aim = 2;
            }
            "impossible" => {
                aim = 13;
            }
            "self" => {
                selected_target = attacker;
            }
            _ => {}
        }
        let before = world.btech.clone();
        for (mode, expected) in [
            (
                BattleGunneryExperienceMode::Classic,
                matches!(case, "eligible" | "suppressed"),
            ),
            (
                BattleGunneryExperienceMode::BattleValue {
                    difficulty_modifier: false,
                },
                matches!(case, "eligible" | "sure"),
            ),
            (
                BattleGunneryExperienceMode::BattleValue {
                    difficulty_modifier: true,
                },
                case == "eligible",
            ),
        ] {
            assert_eq!(
                battle_gunnery_experience_eligible(
                    &world,
                    attacker,
                    selected_pilot,
                    selected_target,
                    aim,
                    mode
                ),
                expected,
                "{case} {mode:?}"
            );
        }
        assert_eq!(world.btech, before);
    }
    let settings = BattleUnitExperience {
        multiplier: 2.5,
        suppress_gunnery: true,
    };
    set_battle_unit_experience(&mut base, attacker, settings).unwrap();
    let before = base.btech.clone();
    for multiplier in [-1.0, f64::NAN, f64::INFINITY] {
        assert!(
            set_battle_unit_experience(
                &mut base,
                attacker,
                BattleUnitExperience {
                    multiplier,
                    ..settings
                }
            )
            .is_err()
        );
        assert_eq!(base.btech, before);
    }
    base.validate(&config).unwrap();
    persistence::save(&config.database(), &base).await.unwrap();
    let loaded = persistence::load(&config.database()).await.unwrap();
    assert_eq!(loaded.btech, base.btech);
    assert_eq!(
        loaded.btech.constructed_units()[&attacker].experience_settings(),
        settings
    );
    base.btech
        .rewrite_unit_record(attacker, |record| {
            record["experience"]["multiplier"] = (-1.0).into();
        })
        .unwrap();
    assert!(base.validate(&config).is_err());
}

/// Enable diagnostic XP traffic without changing the selected award formula.
fn noisy_shot_config(dir: &std::path::Path) -> stompymux_rs::Config {
    let path = dir.join("stompymux.toml");
    let mut source: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    source["battletech"]["xp"]
        .as_table_mut()
        .unwrap()
        .insert("noisy_xpgain".into(), 1.into());
    std::fs::write(path, toml::to_string(&source).unwrap()).unwrap();
    stompymux_rs::Config::load(dir).unwrap()
}

/// Only the connected wizard subscribes; the target crew is not a diagnostic recipient.
fn install_xp_channels(world: &mut stompymux_rs::World) {
    for name in ["MechAttackXP", "MechXP", "MechPilotXP"] {
        let mut channel = stompymux_rs::Channel::new(name.into());
        channel.users.push(stompymux_rs::communication::Membership {
            who: ObjectId(1),
            listening: true,
        });
        world.channels.insert(name.into(), channel);
    }
}

/// Train both fixture crews to finite BV modifiers while keeping the shot difficult enough for XP.
fn train_battle_value_shot_crews(world: &mut stompymux_rs::World, target_pilot: ObjectId) {
    for (pilot, level) in [(ObjectId(1), 6), (target_pilot, 3)] {
        for skill in ["Gunnery-Laser", "Gunnery-Missile", "Piloting-Biped"] {
            stompymux_rs::set_battle_character_value(
                world,
                pilot,
                skill,
                stompymux_rs::BattleCharacterValue {
                    value: level,
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
        }
    }
}

/// Select the classic formula explicitly; the shared game fixture selects battle-value XP.
fn classic_shot_config(dir: &std::path::Path) -> stompymux_rs::Config {
    let path = dir.join("stompymux.toml");
    let mut source: toml::Value = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
    source["battletech"]["xp"]
        .as_table_mut()
        .unwrap()
        .insert("oldxpsystem".into(), 1.into());
    std::fs::write(path, toml::to_string(&source).unwrap()).unwrap();
    stompymux_rs::Config::load(dir).unwrap()
}

/// Classic gunnery awards persist skill and RNG together, and reject errors without consuming the roll.
#[tokio::test]
async fn classic_gunnery_awards_are_atomic_and_replayable() {
    use stompymux_rs::*;
    let (_dir, config, mut world, attacker, target) = shot_fixture().await;
    shot_skill(&mut world, 0);
    for unit in [attacker, target] {
        world
            .objects
            .get_mut(&unit)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
    }
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["signature"]["team"] = 1.into();
        })
        .unwrap();
    let request = BattleGunneryAwardRequest {
        tsm_tow_bonus: true,

        attacker,
        pilot: ObjectId(1),
        target,
        weapon: BattleWeapon::MediumLaser,
        damage: 6,
        base_to_hit: 7,
        extended_gunnery: true,
        extended_piloting: true,
        use_unit_modifier: true,
        now: 100,
    };
    // A slower target changes the derived difficulty without changing request facts.
    let mut slower = world.clone();
    slower
        .btech
        .rewrite_unit_record(target, |record| {
            record["definition"]["max_speed"] = 21.5.into();
        })
        .unwrap();
    let attempt = award_battle_classic_gunnery_experience(&mut slower, request)
        .unwrap()
        .unwrap();
    assert!((attempt.chance.difficulty - (100.0 / 9.0 * 30.0 / 36.0)).abs() < 1e-10);
    let before = world.btech.clone();
    for damage in [0, 6] {
        let ineligible = BattleGunneryAwardRequest {
            tsm_tow_bonus: true,

            damage,
            base_to_hit: 2,
            ..request
        };
        assert!(
            award_battle_classic_gunnery_experience(&mut world, ineligible)
                .unwrap()
                .is_none()
        );
        assert_eq!(world.btech, before);
    }
    persistence::save(&config.database(), &world).await.unwrap();
    let mut replay = persistence::load(&config.database()).await.unwrap();
    replay.objects.get_mut(&ObjectId(1)).unwrap().flags = world.objects[&ObjectId(1)].flags.clone();
    let mut expected_dice: BattleDice = serde_json::from_value(
        serde_json::to_value(&world.btech.constructed_units()[&attacker]).unwrap()["dice"].clone(),
    )
    .unwrap();
    let expected_roll = expected_dice.die(50).unwrap() as u8;
    let report = award_battle_classic_gunnery_experience(&mut world, request)
        .unwrap()
        .unwrap();
    assert_eq!(
        report,
        award_battle_classic_gunnery_experience(&mut replay, request)
            .unwrap()
            .unwrap()
    );
    assert_eq!(world.btech, replay.btech);
    assert_eq!(report.roll, expected_roll);
    assert_eq!(report.amount, Some(5));
    assert_eq!(report.skill, "Gunnery-Laser");
    let award = report.award.unwrap();
    assert!(award.accepted);
    assert_eq!(
        award.after.experience_balance(),
        award.before.experience_balance() + 5
    );
    assert_eq!(award.after.last_used, 100);
    assert_eq!(
        serde_json::to_value(&world.btech.constructed_units()[&attacker]).unwrap()["dice"],
        serde_json::to_value(expected_dice).unwrap()
    );
    set_battle_unit_experience(
        &mut world,
        attacker,
        BattleUnitExperience {
            multiplier: 0.0,
            suppress_gunnery: false,
        },
    )
    .unwrap();
    let report = award_battle_classic_gunnery_experience(&mut world, request)
        .unwrap()
        .unwrap();
    assert!(report.amount.is_none());
    assert!(report.award.is_none());
    let report = award_battle_classic_gunnery_experience(
        &mut world,
        BattleGunneryAwardRequest {
            tsm_tow_bonus: true,

            use_unit_modifier: false,
            extended_gunnery: false,
            ..request
        },
    )
    .unwrap()
    .unwrap();
    assert_eq!(report.skill, "Gunnery-Battlemech");
    assert_eq!(report.amount, Some(5));
    world.validate(&config).unwrap();
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    // Missing character attributes are an award error, not a reason to commit an RNG-only change.
    let mut state = serde_json::to_value(&world.btech).unwrap();
    state["characters"].as_object_mut().unwrap().remove("1");
    world.btech = serde_json::from_value(state).unwrap();
    let before = world.btech.clone();
    assert!(
        award_battle_classic_gunnery_experience(
            &mut world,
            BattleGunneryAwardRequest {
                tsm_tow_bonus: true,

                use_unit_modifier: false,
                ..request
            }
        )
        .is_err()
    );
    assert_eq!(world.btech, before);
}

/// Effective XP speed derives live mass, sampled myomer heat and enabled map gravity without mutation.
#[tokio::test]
async fn effective_speed_tracks_mass_myomer_and_map_conditions() {
    use stompymux_rs::*;
    let (_dir, config, mut world, id) = fixture('.').await;
    install_test_myomer(&mut world, id);
    let map_id = world.btech.constructed_units()[&id].position().unwrap().map;
    for (heat, flags, gravity, expected) in [
        (8.999, 0, 50, 118.25),
        (9.0, 0, 50, 129.0),
        (9.0, 2, 50, 258.0),
        (9.0, 2, 0, 258.0),
        (9.0, 2, 200, 64.5),
        (8.999, 2, 200, 59.125),
    ] {
        myomer_test_heat(&mut world, id, 30.0, heat);
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["maps"][map_id.0.to_string()]["flags"] = flags.into();
        state["maps"][map_id.0.to_string()]["gravity"] = gravity.into();
        world.btech = serde_json::from_value(state).unwrap();
        let before = world.btech.clone();
        let unit = &world.btech.constructed_units()[&id];
        assert_eq!(
            unit.effective_maximum_speed(world.btech.maps().get(&map_id))
                .unwrap(),
            expected
        );
        let value = unit.battle_value(world.btech.maps().get(&map_id)).unwrap();
        let defensive = if expected == 258.0 {
            399.2
        } else if flags == 2 && gravity == 200 {
            324.35
        } else {
            349.3
        };
        assert!((value.defensive - defensive).abs() < 0.0001, "{value:?}");
        assert_eq!(world.btech, before);
    }
    // Underweight damage and ammunition loss do not raise the template speed ceiling.
    world
        .btech
        .rewrite_unit_record(id, |record| {
            record["ammunition"][0] = 0.into();
            record["sections"]["LeftLeg"]["internal"] = 0.into();
            record["sections"]["LeftLeg"]["armor"] = 0.into();
        })
        .unwrap();
    let unit = &world.btech.constructed_units()[&id];
    assert!(unit.mass().unwrap().total < 35 * 1024);
    assert_eq!(unit.mobility().maximum_speed, 10.75);
    assert_eq!(unit.effective_maximum_speed(None).unwrap(), 118.25);
    persistence::save(&config.database(), &world).await.unwrap();
    let restored = persistence::load(&config.database()).await.unwrap();
    assert_eq!(restored.btech, world.btech);
    assert_eq!(
        restored.btech.constructed_units()[&id]
            .effective_maximum_speed(None)
            .unwrap(),
        118.25
    );
}

/// Configured per-unit scaling controls awards; misses and friendly targets consume no award dice.
#[tokio::test]
async fn character_shot_xp_scaling_and_ineligible_hits() {
    use stompymux_rs::*;
    let (dir, _config, mut base, shooter, target) = shot_fixture().await;
    shot_skill(&mut base, 0);
    for id in [shooter, target] {
        base.objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
    }
    set_battle_unit_experience(
        &mut base,
        shooter,
        BattleUnitExperience {
            multiplier: 0.0,
            suppress_gunnery: false,
        },
    )
    .unwrap();
    let index = base.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == BattleWeapon::MediumLaser)
        .unwrap();
    for (per_unit, friendly, miss) in [
        (true, false, false),
        (false, false, false),
        (false, true, false),
        (false, false, true),
    ] {
        let _ = classic_shot_config(dir.path());
        let path = dir.path().join("stompymux.toml");
        let mut source: toml::Value =
            toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        source["battletech"]["xp"]
            .as_table_mut()
            .unwrap()
            .insert("perunit_xpmod".into(), i64::from(per_unit).into());
        std::fs::write(path, toml::to_string(&source).unwrap()).unwrap();
        let config = Config::load(dir.path()).unwrap();
        let mut world = base.clone();
        let seed = (0..=255)
            .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == if miss { 2 } else { 12 })
            .unwrap();
        shot_seed(&mut world, shooter, seed);
        world
            .btech
            .rewrite_unit_record(target, |record| {
                record["signature"]["team"] = i64::from(!friendly).into();
            })
            .unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let report = resolve_battle_shot_action(
            &scripts,
            &config,
            shooter,
            ObjectId(1),
            target,
            index,
            shot_rules(),
        )
        .unwrap();
        let mut expected = BattleDice::seeded([seed; 32]);
        assert_eq!(expected.two_d6(), report.roll);
        if miss {
            assert!(report.salvo.is_none());
            expected_grass_miss_rolls(|| expected.two_d6());
        } else {
            let salvo = report.salvo.unwrap().into_mech().unwrap();
            assert_eq!(salvo.experience.len(), 1);
            if friendly {
                assert!(salvo.experience[0].is_none());
            } else {
                let attempt = salvo.experience[0].as_ref().unwrap();
                assert_eq!(attempt.roll(), Some(expected.die(50).unwrap() as u8));
                assert_eq!(attempt.amount().is_none(), per_unit);
                if let Some(award) = attempt.award() {
                    assert!(award.accepted);
                    assert!(award.after.last_used > 0);
                }
            }
        }
        assert_eq!(
            serde_json::to_value(&scripts.world().btech.constructed_units()[&shooter]).unwrap()["dice"],
            serde_json::to_value(expected).unwrap()
        );
    }
}

/// Battle-value awards use both crews' skills and preserve RNG through success and rejected mutations.
#[tokio::test]
async fn battle_value_gunnery_awards_are_atomic_without_dice() {
    use stompymux_rs::*;
    let (_dir, config, mut world, attacker, target) = shot_fixture().await;
    shot_skill(&mut world, 7);
    set_battle_character_value(
        &mut world,
        ObjectId(1),
        "Piloting-Biped",
        BattleCharacterValue {
            value: 6,
            experience: 0,
            last_used: 0,
        },
    )
    .unwrap();
    for id in [attacker, target] {
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
    }
    world
        .btech
        .rewrite_unit_record(target, |record| {
            record["signature"]["team"] = 1.into();
        })
        .unwrap();
    let request = BattleGunneryAwardRequest {
        tsm_tow_bonus: true,

        attacker,
        target,
        pilot: ObjectId(1),
        weapon: BattleWeapon::MediumLaser,
        damage: 5,
        base_to_hit: 7,
        extended_gunnery: true,
        extended_piloting: true,
        use_unit_modifier: true,
        now: 100,
    };
    let xp = config::XpConfig {
        oldxpsystem: 0,
        ..Default::default()
    };
    let baseline = world.clone();
    let report = award_battle_gunnery_experience(&mut world, request, &xp)
        .unwrap()
        .unwrap();
    let BattleShotExperienceAward::BattleValue(report) = report else {
        panic!("Wrong formula")
    };
    assert_eq!(report.amount, 6);
    assert!(report.calculation.difficulty.unwrap() > 100.0);
    assert!(report.award.accepted);
    assert_eq!(report.award.after.last_used, 100);
    for id in [attacker, target] {
        assert_eq!(
            serde_json::to_value(&world.btech.constructed_units()[&id]).unwrap()["dice"],
            serde_json::to_value(&baseline.btech.constructed_units()[&id]).unwrap()["dice"]
        );
    }
    let mut replay = baseline.clone();
    assert_eq!(
        award_battle_value_gunnery_experience(&mut replay, request, &xp)
            .unwrap()
            .unwrap(),
        report
    );
    assert_eq!(world.btech, replay.btech);
    persistence::save(&config.database(), &world).await.unwrap();
    assert_eq!(
        persistence::load(&config.database()).await.unwrap().btech,
        world.btech
    );
    set_battle_unit_experience(
        &mut replay,
        target,
        BattleUnitExperience {
            suppress_gunnery: true,
            multiplier: 1.0,
        },
    )
    .unwrap();
    let suppressed = replay.btech.clone();
    assert!(
        award_battle_gunnery_experience(&mut replay, request, &xp)
            .unwrap()
            .is_none()
    );
    assert_eq!(replay.btech, suppressed);
    let mut invalid = baseline;
    let before = invalid.btech.clone();
    assert!(
        award_battle_gunnery_experience(
            &mut invalid,
            request,
            &config::XpConfig {
                defaultweapdam: 0,
                ..xp
            }
        )
        .is_err()
    );
    assert_eq!(invalid.btech, before);
}

/// Noisy battle-value XP diagnoses trivial hits only after the other eligibility gates pass.
#[tokio::test]
async fn gunnery_xp_trivial_hit_diagnostics_obey_suppression() {
    use stompymux_rs::*;
    let (dir, _config, mut base, shooter, target) = shot_fixture().await;
    let config = noisy_shot_config(dir.path());
    shot_skill(&mut base, 30);
    for id in [shooter, target] {
        base.objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
    }
    let index = base.btech.constructed_units()[&shooter]
        .loadout()
        .unwrap()
        .weapons
        .iter()
        .position(|mount| mount.weapon == BattleWeapon::MediumLaser)
        .unwrap();
    base.btech
        .rewrite_unit_record(target, |record| {
            record["signature"]["team"] = 1.into();
        })
        .unwrap();
    install_xp_channels(&mut base);
    for suppressed in [false, true] {
        let mut world = base.clone();
        set_battle_unit_experience(
            &mut world,
            target,
            BattleUnitExperience {
                multiplier: 1.0,
                suppress_gunnery: suppressed,
            },
        )
        .unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let report = resolve_battle_shot_action(
            &scripts,
            &config,
            shooter,
            ObjectId(1),
            target,
            index,
            shot_rules(),
        )
        .unwrap();
        assert!(report.target_number.unwrap() < 3);
        let salvo = report.salvo.unwrap().into_mech().unwrap();
        assert!(salvo.experience.iter().all(Option::is_none));
        assert_eq!(salvo.experience_messages.len(), usize::from(!suppressed));
        assert_eq!(scripts.world().channels["MechAttackXP"].messages, 0);
        assert_eq!(
            scripts.world().channels["MechXP"].messages,
            i64::from(!suppressed)
        );
        if !suppressed {
            let expected = format!("#1 in #{} 1 noxp #{}", shooter.0, target.0);
            assert_eq!(salvo.experience_messages[0].text, expected);
            let message = &scripts.world().channels["MechXP"].history[0].message;
            assert_eq!(
                text::plain_with(scripts.palette(), message),
                format!("[MechXP] {expected}")
            );
        }
    }
}

/// Movement XP counts committed crossings, survives restart, and rolls back failed channel delivery.
#[tokio::test]
async fn movement_experience_cadence_restart_and_channel_rollback() {
    use stompymux_rs::*;
    for extended in [false, true] {
        let source = format!("12 60\n{}", format!("{}\n", ".0".repeat(12)).repeat(60));
        let (_dir, config, mut world, id) = fixture_source(&source).await;
        let map = world.btech.constructed_units()[&id].position().unwrap().map;
        let _ = stop_battle_unit(&mut world, id, ObjectId(1), RULES.fall).unwrap();
        place_battle_unit(&mut world, id, map, 5, 45).unwrap();
        assign_battle_pilot(&mut world, id, ObjectId(1)).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        start_battle_unit(&mut world, id, ObjectId(1), true).unwrap();
        for _ in 0..5 {
            advance_battle_units(&mut world, 0);
        }
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        install_xp_channels(&mut world);
        world
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        set_battle_character(
            &mut world,
            ObjectId(1),
            BattleCharacter {
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
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        set_battle_speed(&mut world, id, ObjectId(1), 118.25).unwrap();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let rules = BattleMovementRules {
            fall: BattleFallRules {
                extended_piloting: extended,
                ..RULES.fall
            },
            ..RULES
        };
        let skill = if extended {
            "Piloting-Biped"
        } else {
            "Piloting-Battlemech"
        };
        let mut restarted = false;
        let mut crossed = 0;
        for _ in 0..150 {
            let before = scripts.world().clone();
            let old_position = before.btech.constructed_units()[&id].position();
            // The next crossing after nine is the first award. Overflow must restore that entire tick.
            if crossed == 9 {
                scripts
                    .world_mut()
                    .channels
                    .get_mut("MechPilotXP")
                    .unwrap()
                    .messages = i64::MAX;
            }
            let attempted = advance_battle_motion_action(&scripts, &config, rules);
            if attempted.is_err() {
                assert_eq!(crossed, 9);
                assert_eq!(scripts.world().btech, before.btech);
                assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
                assert!(scripts.drain_outbox().is_empty());
                scripts
                    .world_mut()
                    .channels
                    .get_mut("MechPilotXP")
                    .unwrap()
                    .messages = 0;
                advance_battle_motion_action(&scripts, &config, rules).unwrap();
            }
            let changed = scripts.world().btech.constructed_units()[&id].position() != old_position;
            crossed += u64::from(changed);
            let progress = scripts.world().btech.constructed_units()[&id].movement_experience();
            assert_eq!(progress.hexes_walked, crossed);
            if crossed < 10 {
                assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
            }
            if crossed == 9 && !restarted {
                let saved = scripts.world().clone();
                persistence::save(&config.database(), &saved).await.unwrap();
                let mut loaded = persistence::load(&config.database()).await.unwrap();
                assert_eq!(loaded.btech, saved.btech);
                loaded
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .insert(Flag::Connected);
                *scripts.world_mut() = loaded;
                restarted = true;
            }
            if crossed == 10 {
                assert!(restarted);
                assert_eq!(
                    scripts.world().btech.character_values()[&ObjectId(1)][skill]
                        .experience_balance(),
                    1
                );
                let state = scripts.world();
                let channel = &state.channels["MechPilotXP"];
                assert_eq!(channel.messages, 1);
                assert_eq!(channel.history.len(), 1);
                assert_eq!(
                    text::plain_with(scripts.palette(), &channel.history[0].message),
                    format!("[MechPilotXP] GOD gained 1 {skill} XP")
                );
                let position = state.btech.constructed_units()[&id].position().unwrap();
                assert_eq!(progress.last_award_position, (position.x, position.y));
                break;
            }
            scripts.drain_outbox();
        }
        assert_eq!(crossed, 10);
    }
}

/// Cadence, connectivity and coordinate suppression apply equally to ordinary ground and jump entries.
#[tokio::test]
async fn movement_experience_eligibility_and_airborne_entries() {
    use stompymux_rs::*;
    for airborne in [false, true] {
        for case in [
            "eligible",
            "tactical",
            "disconnected",
            "repeated",
            "interval",
        ] {
            let (_dir, config, mut world, id) = fixture('.').await;
            if case != "tactical" {
                world
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
            }
            install_xp_channels(&mut world);
            world
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .insert(Flag::Connected);
            set_battle_character(
                &mut world,
                ObjectId(1),
                BattleCharacter {
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
            support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
            if airborne {
                launch_battle_jump(&mut world, id, ObjectId(1), 0, 2.0).unwrap();
            } else {
                set_battle_speed(&mut world, id, ObjectId(1), 118.25).unwrap();
            }
            if case == "disconnected" {
                world
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .remove(Flag::Connected);
            }
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    record["movement_experience"] = serde_json::json!({
                        "hexes_walked": if case == "interval" { 10 } else { 9 },
                        "last_award_position": if case == "repeated" { [5,4] } else { [0,0] },
                    });
                })
                .unwrap();
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let mut changed = false;
            for _ in 0..40 {
                let previous = scripts.world().btech.constructed_units()[&id].position();
                if airborne {
                    advance_battle_jumps_action(&scripts, &config, RULES).unwrap();
                } else {
                    advance_battle_motion_action(&scripts, &config, RULES).unwrap();
                }
                if scripts.world().btech.constructed_units()[&id].position() != previous {
                    changed = true;
                    break;
                }
            }
            assert!(changed, "{case} airborne={airborne}");
            let world = scripts.world();
            let unit = &world.btech.constructed_units()[&id];
            let progress = unit.movement_experience();
            assert_eq!(
                progress.hexes_walked,
                match case {
                    "tactical" => 9,
                    "interval" => 11,
                    _ => 10,
                }
            );
            assert_eq!(
                world.channels["MechPilotXP"].history.len(),
                usize::from(case == "eligible")
            );
            assert_eq!(
                progress.last_award_position,
                if matches!(case, "eligible" | "repeated") {
                    (5, 4)
                } else {
                    (0, 0)
                }
            );
        }
    }
}

/// Trip balance awards use real successful rolls, preserve movement marks, and roll back with delivery.
#[tokio::test]
async fn physical_control_experience_policy_and_atomic_delivery() {
    use stompymux_rs::*;
    for extended in [false, true] {
        for case in ["success", "failed", "trivial", "disconnected", "tactical"] {
            let (dir, _config, mut world, source, target) = kick_fixture().await;
            let path = dir.path().join("stompymux.toml");
            let mut settings: toml::Value =
                toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            settings["battletech"]
                .as_table_mut()
                .unwrap()
                .insert("extended_piloting".into(), i64::from(extended).into());
            std::fs::write(path, toml::to_string(&settings).unwrap()).unwrap();
            let config = Config::load(dir.path()).unwrap();
            let pilot = ObjectId(2);
            world.objects.get_mut(&pilot).unwrap().location = Some(target);
            world
                .objects
                .get_mut(&pilot)
                .unwrap()
                .flags
                .insert(Flag::Connected);
            assign_battle_pilot(&mut world, target, pilot).unwrap();
            support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
            start_battle_unit(&mut world, target, pilot, true).unwrap();
            for _ in 0..5 {
                advance_battle_units(&mut world, 0);
            }
            if case != "tactical" {
                world
                    .objects
                    .get_mut(&target)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
            }
            set_battle_character(
                &mut world,
                pilot,
                BattleCharacter {
                    build: 5,
                    reflexes: 4,
                    intuition: 5,
                    learn: 5,
                    charisma: 5,
                    bruise: 0,
                    lethal: 0,
                },
            )
            .unwrap();
            support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
            let skill = if extended {
                "Piloting-Biped"
            } else {
                "Piloting-Battlemech"
            };
            set_battle_character_value(
                &mut world,
                pilot,
                skill,
                BattleCharacterValue {
                    value: if case == "trivial" { 7 } else { 0 },
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
            if case == "disconnected" {
                world
                    .objects
                    .get_mut(&pilot)
                    .unwrap()
                    .flags
                    .remove(Flag::Connected);
            }
            let seed = |roll| {
                (0..=255)
                    .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == roll)
                    .unwrap()
            };
            shot_seed(&mut world, source, seed(12));
            shot_seed(
                &mut world,
                target,
                seed(if case == "failed" { 2 } else { 12 }),
            );
            install_xp_channels(&mut world);
            world
                .btech
                .rewrite_unit_record(target, |record| {
                    record["movement_experience"] =
                        serde_json::json!({"hexes_walked":10,"last_award_position":[5,5]});
                })
                .unwrap();
            let before = world.clone();
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let mut rules = kick_rules();
            rules.fall.extended_piloting = extended;
            let attack = BattlePhysicalAttack::Trip {
                leg: BattleLeg::Right,
            };
            if case == "success" {
                scripts
                    .world_mut()
                    .channels
                    .get_mut("MechPilotXP")
                    .unwrap()
                    .messages = i64::MAX;
                assert!(
                    resolve_battle_physical_attack_action(
                        &scripts,
                        &config,
                        source,
                        ObjectId(1),
                        target,
                        attack,
                        rules
                    )
                    .is_err()
                );
                assert_eq!(scripts.world().btech, before.btech);
                assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
                assert!(scripts.drain_outbox().is_empty());
                *scripts.world_mut() = before.clone();
            }
            let report = resolve_battle_physical_attack_action(
                &scripts,
                &config,
                source,
                ObjectId(1),
                target,
                attack,
                rules,
            )
            .unwrap();
            assert!(report.hit);
            assert!(report.experience.is_none());
            let check = report.balance.unwrap();
            assert_eq!(check.success, case != "failed");
            assert_eq!(check.experience.is_some(), case == "success");
            let candidate = scripts.world().clone();
            assert_eq!(
                candidate.btech.constructed_units()[&target].movement_experience(),
                before.btech.constructed_units()[&target].movement_experience()
            );
            assert_eq!(
                candidate.btech.character_values()[&pilot][skill].experience_balance(),
                if matches!(case, "success" | "failed") {
                    2
                } else {
                    0
                }
            );
            assert_eq!(
                candidate.channels["MechPilotXP"].history.len(),
                usize::from(matches!(case, "success" | "failed"))
            );
            if case == "failed" {
                let fall = report.fall.as_ref().unwrap();
                assert!(fall.avoidance.unwrap().experience.unwrap().accepted);
                assert_eq!(fall.experience_messages.len(), 1);
            }
            if case == "success" {
                let native = Scripts::new(
                    &config,
                    std::rc::Rc::new(std::cell::RefCell::new(before.clone())),
                )
                .unwrap();
                let lua = Scripts::new(
                    &config,
                    std::rc::Rc::new(std::cell::RefCell::new(before.clone())),
                )
                .unwrap();
                let call = format!("btech.unit.trip({},1,'right',{})", source.0, target.0);
                assert!(
                    lua.eval_callback::<()>(&format!("{call}; error('abort')"))
                        .is_err()
                );
                assert_eq!(lua.world().btech, before.btech);
                assert!(lua.world().channels["MechPilotXP"].history.is_empty());
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
                assert_eq!(
                    without_xp_timestamps(&native.world().btech),
                    without_xp_timestamps(&lua.world().btech)
                );
                assert_eq!(
                    native.world().btech.character_values()[&pilot][skill].experience_balance(),
                    2
                );
                assert_eq!(native.world().channels["MechPilotXP"].history.len(), 1);
                assert_eq!(
                    native.world().channels["MechPilotXP"].history[0].message,
                    lua.world().channels["MechPilotXP"].history[0].message
                );
                assert_eq!(check.target, 9);
                assert_eq!(check.roll, Some(12));
                assert!(check.experience.unwrap().accepted);
                assert_eq!(
                    report.experience_messages[0].text,
                    format!("{} gained 2 {skill} XP", candidate.objects[&pilot].name)
                );
                persistence::save(&config.database(), &candidate)
                    .await
                    .unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    candidate.btech
                );
            }
        }
    }
}

/// Both a DFA hit and a missed-DFA protection roll can earn control XP without damage XP.
#[tokio::test]
async fn dfa_control_experience_hit_and_miss() {
    use stompymux_rs::*;
    for hit in [false, true] {
        let (_dir, config, mut world, attacker, target) = kick_fixture().await;
        world
            .objects
            .get_mut(&attacker)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        set_battle_character(
            &mut world,
            ObjectId(1),
            BattleCharacter {
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
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            "Piloting-Biped",
            BattleCharacterValue {
                value: 5,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        launch_battle_jump(&mut world, attacker, ObjectId(1), 0, 2.0).unwrap();
        install_xp_channels(&mut world);
        world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(attacker);
        world
            .objects
            .get_mut(&ObjectId(2))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        let before = world.clone();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let mut rules = kick_rules();
        rules.fall.extended_piloting = true;
        let mut verified = false;
        for seed in 0..=255 {
            if BattleDice::seeded([seed; 32]).two_d6() != if hit { 12 } else { 2 } {
                continue;
            }
            *scripts.world_mut() = before.clone();
            shot_seed(&mut scripts.world_mut(), attacker, seed);
            let seeded = scripts.world().clone();
            let report =
                resolve_battle_dfa_action(&scripts, &config, attacker, target, rules).unwrap();
            assert_eq!(report.hit, hit);
            assert!(report.experience.is_empty()); // The target is tactical.
            let Some(check) = report
                .balance
                .iter()
                .find(|balance| balance.unit == attacker)
                .map(|balance| balance.check)
            else {
                continue;
            };
            let Some(award) = check.experience else {
                scripts.drain_outbox();
                continue;
            };
            assert!(award.accepted);
            assert_eq!(award.after.experience_balance(), 1);
            assert_eq!(report.experience_messages.len(), 1);
            assert_eq!(
                report.experience_messages[0].text,
                "GOD gained 1 Piloting-Biped XP"
            );
            assert_eq!(scripts.world().channels["MechPilotXP"].history.len(), 1);
            let candidate = scripts.world().clone();
            let output = scripts.drain_outbox();
            let expected = format!(
                "Modified Pilot Skill: BTH {}\tRoll: {}",
                check.target,
                check.roll.unwrap()
            );
            let pilot = output
                .iter()
                .filter(|(who, _)| *who == ObjectId(1))
                .map(|(_, message)| message.source())
                .collect::<Vec<_>>();
            let index = pilot
                .iter()
                .position(|message| *message == expected)
                .unwrap();
            assert!(index > 1);
            assert_eq!(pilot[index - 1], "You make a piloting skill roll!");
            assert!(!output.iter().any(|(who, message)| *who == ObjectId(2)
                && (message.source().starts_with("Modified Pilot Skill:")
                    || message.source() == "You make a piloting skill roll!")));
            *scripts.world_mut() = seeded;
            scripts
                .world_mut()
                .channels
                .get_mut("MechPilotXP")
                .unwrap()
                .messages = i64::MAX;
            let failed = scripts.world().clone();
            assert!(resolve_battle_dfa_action(&scripts, &config, attacker, target, rules).is_err());
            assert_eq!(scripts.world().btech, failed.btech);
            assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
            assert!(scripts.drain_outbox().is_empty());
            persistence::save(&config.database(), &candidate)
                .await
                .unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                candidate.btech
            );
            verified = true;
            break;
        }
        assert!(
            verified,
            "No successful control roll exercised for hit={hit}"
        );
    }
}

/// Fall protection XP precedes damage and shares the complete fall's delivery/restart transaction.
#[tokio::test]
async fn fall_protection_experience_policy_and_rollback() {
    use stompymux_rs::*;
    for extended in [false, true] {
        for case in ["success", "failed", "trivial", "prone", "disconnected"] {
            let (_dir, config, mut world, id) = fixture('.').await;
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            world
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .insert(Flag::Connected);
            set_battle_character(
                &mut world,
                ObjectId(1),
                BattleCharacter {
                    build: 5,
                    reflexes: 4,
                    intuition: 5,
                    learn: 5,
                    charisma: 5,
                    bruise: 0,
                    lethal: 0,
                },
            )
            .unwrap();
            support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
            let skill = if extended {
                "Piloting-Biped"
            } else {
                "Piloting-Battlemech"
            };
            set_battle_character_value(
                &mut world,
                ObjectId(1),
                skill,
                BattleCharacterValue {
                    value: if case == "trivial" { 8 } else { 0 },
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
            if case == "disconnected" {
                world
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .remove(Flag::Connected);
            }
            if case == "prone" {
                world
                    .btech
                    .rewrite_unit_record(id, |record| {
                        record["posture"] = serde_json::to_value(BattlePosture::Prone).unwrap();
                    })
                    .unwrap();
            }
            let seed = (0..=255)
                .find(|seed| {
                    BattleDice::seeded([*seed; 32]).two_d6()
                        == if case == "failed" { 2 } else { 12 }
                })
                .unwrap();
            shot_seed(&mut world, id, seed);
            install_xp_channels(&mut world);
            world.objects.get_mut(&ObjectId(2)).unwrap().location = Some(id);
            world
                .objects
                .get_mut(&ObjectId(2))
                .unwrap()
                .flags
                .insert(Flag::Connected);
            let before = world.clone();
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let rules = BattleFallRules {
                extended_piloting: extended,
                ..fall_rules()
            };
            if case == "success" {
                scripts
                    .world_mut()
                    .channels
                    .get_mut("MechPilotXP")
                    .unwrap()
                    .messages = i64::MAX;
                assert!(fall_battle_unit_action(&scripts, &config, id, 1, rules).is_err());
                assert_eq!(scripts.world().btech, before.btech);
                assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
                assert!(scripts.drain_outbox().is_empty());
                *scripts.world_mut() = before.clone();
            }
            let report = fall_battle_unit_action(&scripts, &config, id, 1, rules).unwrap();
            let output = scripts.drain_outbox();
            assert_eq!(report.pilot, Some(ObjectId(1)));
            assert!(!output.iter().any(|(who, message)| *who == ObjectId(2)
                && (message.source().starts_with("Modified Pilot Skill:")
                    || message.source() == "You make a piloting skill roll!")));
            if case != "disconnected" {
                let pilot = output
                    .iter()
                    .filter(|(who, _)| *who == ObjectId(1))
                    .map(|(_, message)| message.source())
                    .collect::<Vec<_>>();
                if let Some(messages) = report.avoidance.unwrap().messages() {
                    assert_eq!(pilot[0], messages[0]);
                    assert_eq!(pilot[1], messages[1]);
                } else {
                    assert!(
                        !pilot
                            .iter()
                            .any(|message| message.starts_with("Modified Pilot Skill:"))
                    );
                }
            }
            assert_eq!(report.avoidance.unwrap().success, case != "failed");
            assert_eq!(
                report.avoidance.unwrap().experience.is_some(),
                case == "success"
            );
            assert_eq!(
                report.experience_messages.len(),
                usize::from(case == "success")
            );
            let candidate = scripts.world().clone();
            assert_eq!(
                candidate.btech.character_values()[&ObjectId(1)][skill].experience_balance(),
                if case == "success" { 2 } else { 0 }
            );
            assert_eq!(
                candidate.channels["MechPilotXP"].history.len(),
                usize::from(case == "success")
            );
            if case == "success" {
                assert_eq!(report.avoidance.unwrap().target, 10);
                assert!(report.character_injury.is_none());
                assert_eq!(
                    report.experience_messages[0].text,
                    format!("GOD gained 2 {skill} XP")
                );
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
                *scripts.world_mut() = loaded;
                let replay = fall_battle_unit_action(&scripts, &config, id, 1, rules).unwrap();
                assert_eq!(
                    without_xp_timestamps(&report),
                    without_xp_timestamps(&replay)
                );
                assert_eq!(
                    without_xp_timestamps(&candidate.btech),
                    without_xp_timestamps(&scripts.world().btech)
                );
                persistence::save(&config.database(), &candidate)
                    .await
                    .unwrap();
                assert_eq!(
                    persistence::load(&config.database()).await.unwrap().btech,
                    candidate.btech
                );
            }
        }
    }
}

/// A failed trip balance roll can still earn protection XP in its nested fall, published exactly once.
#[tokio::test]
async fn nested_trip_fall_protection_experience_is_transactional() {
    use stompymux_rs::*;
    let (_dir, config, mut world, source, target) = kick_fixture().await;
    let pilot = ObjectId(2);
    world.objects.get_mut(&pilot).unwrap().location = Some(target);
    world
        .objects
        .get_mut(&pilot)
        .unwrap()
        .flags
        .insert(Flag::Connected);
    assign_battle_pilot(&mut world, target, pilot).unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    start_battle_unit(&mut world, target, pilot, true).unwrap();
    for _ in 0..5 {
        advance_battle_units(&mut world, 0);
    }
    world
        .objects
        .get_mut(&target)
        .unwrap()
        .flags
        .insert(Flag::InCharacter);
    set_battle_character(
        &mut world,
        pilot,
        BattleCharacter {
            build: 5,
            reflexes: 4,
            intuition: 5,
            learn: 5,
            charisma: 5,
            bruise: 0,
            lethal: 0,
        },
    )
    .unwrap();
    support::seed_object_dice(&mut world, pilot, support::FIXTURE_DICE_SEED);
    let source_seed = (0..=255)
        .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
        .unwrap();
    shot_seed(&mut world, source, source_seed);
    let dice = (0..=u16::MAX)
        .find_map(|seed| {
            let mut bytes = [0; 32];
            bytes[..2].copy_from_slice(&seed.to_le_bytes());
            let dice = BattleDice::seeded(bytes);
            let mut probe = dice.clone();
            (probe.two_d6() == 2 && probe.two_d6() == 12).then_some(dice)
        })
        .unwrap();
    world.btech.set_unit_dice(target, dice).unwrap();
    install_xp_channels(&mut world);
    let before = world.clone();
    let scripts = Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
    let mut rules = kick_rules();
    rules.fall.extended_piloting = true;
    let attack = BattlePhysicalAttack::Trip {
        leg: BattleLeg::Right,
    };
    scripts
        .world_mut()
        .channels
        .get_mut("MechPilotXP")
        .unwrap()
        .messages = i64::MAX;
    assert!(
        resolve_battle_physical_attack_action(
            &scripts,
            &config,
            source,
            ObjectId(1),
            target,
            attack,
            rules
        )
        .is_err()
    );
    assert_eq!(scripts.world().btech, before.btech);
    assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
    assert!(scripts.drain_outbox().is_empty());
    *scripts.world_mut() = before;
    let report = resolve_battle_physical_attack_action(
        &scripts,
        &config,
        source,
        ObjectId(1),
        target,
        attack,
        rules,
    )
    .unwrap();
    assert!(!report.balance.unwrap().success);
    assert!(report.experience_messages.is_empty());
    let fall = report.fall.unwrap();
    assert_eq!(fall.avoidance.unwrap().roll, Some(12));
    assert_eq!(fall.avoidance.unwrap().target, 10);
    assert!(fall.avoidance.unwrap().experience.unwrap().accepted);
    assert_eq!(fall.experience_messages.len(), 1);
    assert_eq!(scripts.world().channels["MechPilotXP"].history.len(), 1);
    assert_eq!(
        scripts.world().btech.character_values()[&pilot]["Piloting-Biped"].experience_balance(),
        2
    );
}

/// A penetrating hit's successful damage balance check earns XP through the salvo host transaction.
#[tokio::test]
async fn damage_balance_experience_publication_and_rollback() {
    use stompymux_rs::*;
    for extended in [false, true] {
        let (_dir, config, mut world, id) = fixture('.').await;
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        world
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        set_battle_character(
            &mut world,
            ObjectId(1),
            BattleCharacter {
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
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        let skill = if extended {
            "Piloting-Biped"
        } else {
            "Piloting-Battlemech"
        };
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            skill,
            BattleCharacterValue {
                value: 4,
                experience: 0,
                last_used: 0,
            },
        )
        .unwrap();
        world
            .btech
            .rewrite_unit_record(id, |record| {
                for section in record["sections"].as_object_mut().unwrap().values_mut() {
                    section["armor"] = 0.into();
                    section["rear"] = 0.into();
                }
            })
            .unwrap();
        install_xp_channels(&mut world);
        let base = world.clone();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let rules = BattleFallRules {
            extended_piloting: extended,
            ..fall_rules()
        };
        let mut verified = false;
        for seed in 0..=255 {
            *scripts.world_mut() = base.clone();
            shot_seed(&mut scripts.world_mut(), id, seed);
            let before = scripts.world().clone();
            let report = resolve_battle_salvo_action(
                &scripts,
                &config,
                id,
                BattleWeapon::SmallLaser,
                BattleHitArc::Front,
                rules,
            )
            .unwrap();
            let balances: Vec<_> = report
                .groups
                .iter()
                .flat_map(|group| &group.balance)
                .collect();
            if balances.len() != 1
                || balances[0].fall.is_some()
                || balances[0].experience_messages.is_empty()
            {
                scripts.drain_outbox();
                continue;
            }
            let balance = balances[0];
            let check = balance.check.unwrap();
            assert!(check.success);
            assert!(check.target > 2);
            let award = check.experience.unwrap();
            assert!(award.accepted);
            assert_eq!(award.before.experience_balance(), 0);
            assert_eq!(award.after.experience_balance(), 1);
            assert_eq!(
                balance.experience_messages[0].text,
                format!("GOD gained 1 {skill} XP")
            );
            assert_eq!(scripts.world().channels["MechPilotXP"].history.len(), 1);
            let candidate = scripts.world().clone();
            scripts.drain_outbox();
            *scripts.world_mut() = before.clone();
            scripts
                .world_mut()
                .channels
                .get_mut("MechPilotXP")
                .unwrap()
                .messages = i64::MAX;
            assert!(
                resolve_battle_salvo_action(
                    &scripts,
                    &config,
                    id,
                    BattleWeapon::SmallLaser,
                    BattleHitArc::Front,
                    rules
                )
                .is_err()
            );
            assert_eq!(scripts.world().btech, before.btech);
            assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
            assert!(scripts.drain_outbox().is_empty());
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
            *scripts.world_mut() = loaded;
            let replay = resolve_battle_salvo_action(
                &scripts,
                &config,
                id,
                BattleWeapon::SmallLaser,
                BattleHitArc::Front,
                rules,
            )
            .unwrap();
            assert_eq!(
                without_xp_timestamps(&report),
                without_xp_timestamps(&replay)
            );
            assert_eq!(
                without_xp_timestamps(&candidate.btech),
                without_xp_timestamps(&scripts.world().btech)
            );
            *scripts.world_mut() = before;
            scripts
                .world_mut()
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .remove(Flag::Connected);
            let _ = resolve_battle_salvo_action(
                &scripts,
                &config,
                id,
                BattleWeapon::SmallLaser,
                BattleHitArc::Front,
                rules,
            )
            .unwrap();
            assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
            assert_eq!(
                scripts.world().btech.character_values()[&ObjectId(1)][skill].experience_balance(),
                0
            );
            verified = true;
            break;
        }
        assert!(verified, "No successful damage balance check exercised");
    }
}

/// Each stagger history mode awards successful-control XP once and restores its consumed history on failure.
#[tokio::test]
async fn stagger_control_experience_modes_and_rollback() {
    use stompymux_rs::*;
    for extended in [false, true] {
        for mode in [
            BattleStaggerMode::Traditional,
            BattleStaggerMode::Consume,
            BattleStaggerMode::Retain,
        ] {
            let (_dir, config, mut world, id) = stagger_fixture().await;
            stagger_hit(&mut world, id, BattleSection::LeftTorso, 20, mode);
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            world
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .insert(Flag::Connected);
            set_battle_character(
                &mut world,
                ObjectId(1),
                BattleCharacter {
                    build: 5,
                    reflexes: 4,
                    intuition: 5,
                    learn: 5,
                    charisma: 5,
                    bruise: 0,
                    lethal: 0,
                },
            )
            .unwrap();
            support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
            let skill = if extended {
                "Piloting-Biped"
            } else {
                "Piloting-Battlemech"
            };
            set_battle_character_value(
                &mut world,
                ObjectId(1),
                skill,
                BattleCharacterValue::default(),
            )
            .unwrap();
            let seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
                .unwrap();
            shot_seed(&mut world, id, seed);
            install_xp_channels(&mut world);
            let before = world.clone();
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let rules = BattleStaggerRules {
                extended_piloting: extended,
                interval: 1,
                ..stagger_rules(mode)
            };
            scripts
                .world_mut()
                .channels
                .get_mut("MechPilotXP")
                .unwrap()
                .messages = i64::MAX;
            assert!(advance_battle_stagger_action(&scripts, &config, rules).is_err());
            assert_eq!(scripts.world().btech, before.btech);
            assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
            assert!(scripts.drain_outbox().is_empty());
            *scripts.world_mut() = before.clone();
            let reports = advance_battle_stagger_action(&scripts, &config, rules).unwrap();
            assert_eq!(reports.len(), 1);
            let report = &reports[0];
            let output = scripts.drain_outbox();
            let pilot_output: Vec<_> = output
                .iter()
                .filter(|(who, _)| *who == ObjectId(1))
                .map(|(_, text)| text.source())
                .collect();
            let check = reports[0].check;
            let roll_index = pilot_output
                .iter()
                .position(|text| *text == "You make a piloting skill roll!");
            assert_eq!(roll_index.is_some(), check.roll.is_some());
            if let Some(index) = roll_index {
                assert_eq!(pilot_output[index - 1], "You stagger from the damage!");
                assert_eq!(
                    pilot_output[index + 1],
                    format!(
                        "Modified Pilot Skill: BTH {}\tRoll: {}",
                        check.target,
                        check.roll.unwrap()
                    )
                );
                if !check.success {
                    assert_eq!(
                        pilot_output[index + 2],
                        "You fall over from all the damage!"
                    );
                }
            }
            assert!(!output.iter().any(|(who, text)| *who != ObjectId(1)
                && (text.source().starts_with("You make a piloting")
                    || text.source().starts_with("Modified Pilot Skill:"))));

            assert!(report.check.success);
            assert!(report.fall.is_none());
            let amount = if mode == BattleStaggerMode::Traditional {
                2
            } else {
                1
            };
            assert_eq!(
                report.check.experience.unwrap().after.experience_balance(),
                amount
            );
            assert_eq!(report.experience_messages.len(), 1);
            assert_eq!(
                report.experience_messages[0].text,
                format!("GOD gained {amount} {skill} XP")
            );
            assert_eq!(scripts.world().channels["MechPilotXP"].history.len(), 1);
            let candidate = scripts.world().clone();
            assert!(
                advance_battle_stagger_action(&scripts, &config, rules)
                    .unwrap()
                    .is_empty()
            );
            assert_eq!(scripts.world().channels["MechPilotXP"].history.len(), 1);
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
            *scripts.world_mut() = restored;
            let replay = advance_battle_stagger_action(&scripts, &config, rules).unwrap();
            assert_eq!(
                without_xp_timestamps(&reports),
                without_xp_timestamps(&replay)
            );
            assert_eq!(
                without_xp_timestamps(&candidate.btech),
                without_xp_timestamps(&scripts.world().btech)
            );
            *scripts.world_mut() = before;
            scripts
                .world_mut()
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .remove(Flag::Connected);
            let reports = advance_battle_stagger_action(&scripts, &config, rules).unwrap();
            assert!(reports[0].check.experience.is_none());
            assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
        }
    }
}

/// Successful thermal-shutdown balance awards XP before power-down and restores the whole thermal tick on failure.
#[tokio::test]
async fn shutdown_balance_experience_precedes_power_down() {
    use stompymux_rs::*;
    for extended in [false, true] {
        let (_dir, config, mut world, id) = fixture('.').await;
        world
            .objects
            .get_mut(&id)
            .unwrap()
            .flags
            .insert(Flag::InCharacter);
        world
            .objects
            .get_mut(&ObjectId(1))
            .unwrap()
            .flags
            .insert(Flag::Connected);
        set_battle_character(
            &mut world,
            ObjectId(1),
            BattleCharacter {
                build: 5,
                reflexes: 4,
                intuition: 5,
                learn: 5,
                charisma: 5,
                bruise: 0,
                lethal: 0,
            },
        )
        .unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        overheat_due(&mut world, id, 30.0, false);
        let skill = if extended {
            "Piloting-Biped"
        } else {
            "Piloting-Battlemech"
        };
        set_battle_character_value(
            &mut world,
            ObjectId(1),
            skill,
            BattleCharacterValue::default(),
        )
        .unwrap();
        let seed = (0..=255)
            .find(|seed| {
                let mut dice = BattleDice::seeded([*seed; 32]);
                let ammunition = dice.two_d6();
                // An untrained Computer attempt uses the lowest two of three dice.
                for _ in 0..3 {
                    dice.d6();
                }
                ammunition >= 8 && dice.two_d6() == 12
            })
            .unwrap();
        shot_seed(&mut world, id, seed);
        world
            .btech
            .edit_unit_motion(id, |motion| {
                motion.speed = 21.5;
                motion.desired_speed = 21.5;
            })
            .unwrap();
        install_xp_channels(&mut world);
        let before = world.clone();
        let scripts =
            Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
        let rules = BattleOverheatRules {
            extended_piloting: extended,
            ..overheat_rules()
        };
        scripts
            .world_mut()
            .channels
            .get_mut("MechPilotXP")
            .unwrap()
            .messages = i64::MAX;
        assert!(advance_battle_overheat_action(&scripts, &config, rules).is_err());
        assert_eq!(scripts.world().btech, before.btech);
        assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
        assert!(scripts.drain_outbox().is_empty());
        *scripts.world_mut() = before.clone();
        let reports = advance_battle_overheat_action(&scripts, &config, rules).unwrap();
        assert_eq!(reports.len(), 1);
        let report = &reports[0];
        assert!(report.shutdown);
        assert!(report.fall.is_none());
        let check = report.balance.unwrap();
        let output = scripts.drain_outbox();
        let pilot: Vec<_> = output
            .iter()
            .filter(|(who, _)| *who == ObjectId(1))
            .map(|(_, text)| text.source())
            .collect();
        let roll = pilot
            .iter()
            .position(|text| *text == "You make a piloting skill roll!")
            .unwrap();
        assert_eq!(pilot[roll + 1], "Modified Pilot Skill: BTH 12\tRoll: 12");
        assert!(
            pilot[..roll]
                .iter()
                .any(|text| text.starts_with("You make a computer skill roll!"))
        );
        assert!(
            !output.iter().any(|(who, text)| *who != ObjectId(1)
                && text.source().starts_with("Modified Pilot Skill:"))
        );
        assert_eq!(report.pilot_notices.len(), 2);
        assert_eq!(report.pilot_notices[0].pilot, ObjectId(1));

        assert_eq!(check.target, 12);
        assert_eq!(check.roll, Some(12));
        assert_eq!(check.experience.unwrap().after.experience_balance(), 4);
        assert_eq!(
            report.experience_messages[0].text,
            format!("GOD gained 4 {skill} XP")
        );
        assert_eq!(
            scripts.world().btech.constructed_units()[&id].power(),
            BattlePower::Off
        );
        assert_eq!(scripts.world().channels["MechPilotXP"].history.len(), 1);
        let candidate = scripts.world().clone();
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
        *scripts.world_mut() = loaded;
        let replay = advance_battle_overheat_action(&scripts, &config, rules).unwrap();
        assert_eq!(
            without_xp_timestamps(&reports),
            without_xp_timestamps(&replay)
        );
        assert_eq!(
            without_xp_timestamps(&candidate.btech),
            without_xp_timestamps(&scripts.world().btech)
        );
    }
}

/// Terrain control awards follow accepted movement and avoid a duplicate water check after reverse slopes.
#[tokio::test]
async fn terrain_control_experience_and_movement_rollback() {
    use stompymux_rs::*;
    for extended in [false, true] {
        for case in ["water", "reverse", "reverse_water"] {
            let mut source = String::from("12 12\n");
            for y in 0..12 {
                for x in 0..12 {
                    source.push_str(if (x, y) == (5, 4) {
                        if case == "reverse" { ".2" } else { "~2" }
                    } else {
                        ".0"
                    });
                }
                source.push('\n');
            }
            let (_dir, config, mut world, id) = fixture_source(&source).await;
            world
                .objects
                .get_mut(&id)
                .unwrap()
                .flags
                .insert(Flag::InCharacter);
            world
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .insert(Flag::Connected);
            set_battle_character(
                &mut world,
                ObjectId(1),
                BattleCharacter {
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
            support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
            let skill = if extended {
                "Piloting-Biped"
            } else {
                "Piloting-Battlemech"
            };
            set_battle_character_value(
                &mut world,
                ObjectId(1),
                skill,
                BattleCharacterValue::default(),
            )
            .unwrap();
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    let unit = record;
                    let speed = if case == "water" { 21.5 } else { -21.5 };
                    let heading = if case == "water" { 0 } else { 180 };
                    unit["motion"]["speed"] = speed.into();
                    unit["motion"]["desired_speed"] = speed.into();
                    unit["motion"]["heading"] = heading.into();
                    unit["motion"]["desired_heading"] = heading.into();
                })
                .unwrap();
            let seed = (0..=255)
                .find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12)
                .unwrap();
            shot_seed(&mut world, id, seed);
            install_xp_channels(&mut world);
            world.channels.get_mut("MechPilotXP").unwrap().messages = i64::MAX;
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let rules = BattleMovementRules {
                fall: BattleFallRules {
                    extended_piloting: extended,
                    ..RULES.fall
                },
                ..RULES
            };
            let mut rejected = false;
            for _ in 0..40 {
                let before = scripts.world().clone();
                if advance_battle_motion_action(&scripts, &config, rules).is_err() {
                    assert!(!rejected);
                    rejected = true;
                    assert_eq!(scripts.world().btech, before.btech);
                    assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
                    assert!(scripts.drain_outbox().is_empty());
                    scripts
                        .world_mut()
                        .channels
                        .get_mut("MechPilotXP")
                        .unwrap()
                        .messages = 0;
                    advance_battle_motion_action(&scripts, &config, rules).unwrap();
                }
                if scripts.world().btech.constructed_units()[&id]
                    .position()
                    .unwrap()
                    .y
                    == 4
                {
                    break;
                }
                scripts.drain_outbox();
            }
            assert!(rejected, "{case}");
            let output = scripts.drain_outbox();
            let pilot_output: Vec<_> = output
                .iter()
                .filter(|(who, _)| *who == ObjectId(1))
                .map(|(_, text)| text.source())
                .collect();
            let roll = pilot_output
                .iter()
                .position(|text| *text == "You make a piloting skill roll!")
                .unwrap();
            assert!(roll > 0);
            assert_eq!(
                pilot_output
                    .iter()
                    .filter(|text| **text == "You make a piloting skill roll!")
                    .count(),
                1
            );
            assert!(pilot_output[roll - 1].contains(if case == "water" {
                "maneuver through the water"
            } else {
                "behind you!"
            }));
            assert!(pilot_output[roll + 1].starts_with("Modified Pilot Skill: BTH "));
            assert!(pilot_output[roll + 1].ends_with("\tRoll: 12"));
            if case != "water" {
                assert_eq!(
                    pilot_output[roll + 2],
                    "You manage to overcome the obstacle."
                );
            }
            assert!(!output.iter().any(|(who, text)| *who != ObjectId(1)
                && (text.source().starts_with("You make a piloting")
                    || text.source().starts_with("Modified Pilot Skill:"))));
            let candidate = scripts.world().clone();
            let unit = &candidate.btech.constructed_units()[&id];
            assert_eq!(unit.position().unwrap().y, 4);
            assert_eq!(unit.posture(), BattlePosture::Standing);
            let amount = if case == "water" { 1 } else { 2 };
            assert_eq!(
                candidate.btech.character_values()[&ObjectId(1)][skill].experience_balance(),
                amount
            );
            assert_eq!(candidate.channels["MechPilotXP"].history.len(), 1);
            assert_eq!(
                text::plain_with(
                    scripts.palette(),
                    &candidate.channels["MechPilotXP"].history[0].message
                ),
                format!("[MechPilotXP] GOD gained {amount} {skill} XP")
            );
            let mut expected = BattleDice::seeded([seed; 32]);
            expected.two_d6();
            assert_eq!(
                serde_json::to_value(unit).unwrap()["dice"],
                serde_json::to_value(expected).unwrap()
            );
            persistence::save(&config.database(), &candidate)
                .await
                .unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                candidate.btech
            );
        }
    }
}

/// A missed moving shot can earn recoil XP; failed publication restores the entire shot.
#[tokio::test]
async fn heavy_gauss_recoil_experience_and_delivery_rollback() {
    use stompymux_rs::*;
    for extended in [false, true] {
        for case in ["moving", "stationary", "disconnected", "tactical"] {
            let (_dir, config, mut base, id, target) = shot_fixture().await;
            let mut definition = base.btech.constructed_units()[&id].definition().clone();
            let torso = definition
                .sections
                .get_mut(&BattleSection::LeftTorso)
                .unwrap();
            let mut part = torso.criticals[&0].clone();
            part.equipment = "IS.HeavyGaussRifle".into();
            for slot in 0..11 {
                torso.criticals.insert(slot, part.clone());
            }
            let bin = definition
                .sections
                .values_mut()
                .flat_map(|s| s.criticals.values_mut())
                .find(|part| part.equipment.starts_with("Ammo_"))
                .unwrap();
            bin.equipment = "Ammo_IS.HeavyGaussRifle".into();
            bin.data = "4".into();
            base.btech
                .rewrite_unit_record(id, |record| {
                    record["definition"] = serde_json::to_value(definition).unwrap();
                    record["ammunition"][0] = 4.into();
                })
                .unwrap();
            let index = base.btech.constructed_units()[&id]
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|mount| mount.weapon == BattleWeapon::HeavyGaussRifle)
                .unwrap();

            let mut world = base.clone();
            if case != "tactical" {
                world
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
            }
            if case == "disconnected" {
                world
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .remove(Flag::Connected);
            }
            set_battle_character(
                &mut world,
                ObjectId(1),
                BattleCharacter {
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
            support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
            let skill = if extended {
                "Piloting-Biped"
            } else {
                "Piloting-Battlemech"
            };
            set_battle_character_value(
                &mut world,
                ObjectId(1),
                skill,
                BattleCharacterValue {
                    value: 0,
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
            let seed = (0..=u16::MAX)
                .map(|candidate| {
                    let mut seed = [0; 32];
                    seed[..2].copy_from_slice(&candidate.to_le_bytes());
                    seed
                })
                .find(|seed| {
                    let mut dice = BattleDice::seeded(*seed);
                    let attack = dice.two_d6();
                    expected_grass_miss_rolls(|| dice.two_d6());
                    attack == 2 && dice.two_d6() == 12
                })
                .unwrap();
            world
                .btech
                .rewrite_unit_record(id, |record| {
                    record["dice"] = serde_json::to_value(BattleDice::seeded(seed)).unwrap();
                    record["motion"]["speed"] = if case == "stationary" { 0.0 } else { 1.0 }.into();
                })
                .unwrap();
            install_xp_channels(&mut world);
            let before = world.clone();
            let scripts =
                Scripts::new(&config, std::rc::Rc::new(std::cell::RefCell::new(world))).unwrap();
            let rules = BattleShotRules {
                range_damage: false,
                tsm_tow_bonus: true,
                extended_piloting: extended,
                ..shot_rules()
            };
            if case == "moving" {
                scripts
                    .world_mut()
                    .channels
                    .get_mut("MechPilotXP")
                    .unwrap()
                    .messages = i64::MAX;
                assert!(
                    resolve_battle_shot_action(
                        &scripts,
                        &config,
                        id,
                        ObjectId(1),
                        target,
                        index,
                        rules
                    )
                    .is_err()
                );
                assert_eq!(scripts.world().btech, before.btech);
                assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
                assert!(scripts.drain_outbox().is_empty());
                *scripts.world_mut() = before.clone();
            }
            let report = resolve_battle_shot_action(
                &scripts,
                &config,
                id,
                ObjectId(1),
                target,
                index,
                rules,
            )
            .unwrap();
            assert!(report.salvo.is_none());
            let expected = if case == "moving" { 3 } else { 0 };
            if case == "stationary" {
                assert!(report.recoil.is_none());
            } else {
                let recoil = report.recoil.as_ref().unwrap();
                assert_eq!(recoil.check.roll, Some(12));
                assert!(recoil.check.success);
                assert!(recoil.fall.is_none());
                assert_eq!(recoil.check.experience.is_some(), expected > 0);
                assert_eq!(recoil.experience_messages.len(), usize::from(expected > 0));
            }
            let output = scripts.drain_outbox();
            let pilot: Vec<_> = output
                .iter()
                .filter(|(who, _)| *who == ObjectId(1))
                .map(|(_, text)| text.source())
                .collect();
            let roll = pilot
                .iter()
                .position(|text| *text == "You make a piloting skill roll!");
            assert_eq!(roll.is_some(), case != "stationary");
            if let Some(index) = roll {
                assert_eq!(
                    pilot[index - 1],
                    "You realize that moving while firing this weapon may not be a good idea after all."
                );
                assert_eq!(
                    pilot[index + 1],
                    format!(
                        "Modified Pilot Skill: BTH {}\tRoll: 12",
                        report.recoil.as_ref().unwrap().check.target
                    )
                );
            }
            assert!(!output.iter().any(|(who, text)| *who != ObjectId(1)
                && text.source().starts_with("Modified Pilot Skill:")));
            let candidate = scripts.world().clone();
            assert_eq!(
                candidate.btech.character_values()[&ObjectId(1)][skill].experience_balance(),
                expected
            );
            assert_eq!(
                candidate.channels["MechPilotXP"].history.len(),
                usize::from(expected > 0)
            );
            assert_eq!(candidate.btech.constructed_units()[&id].ammunition()[0], 3);
            assert_eq!(
                candidate.btech.constructed_units()[&target],
                before.btech.constructed_units()[&target]
            );
            persistence::save(&config.database(), &candidate)
                .await
                .unwrap();
            let restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(restored.btech, candidate.btech);
        }
    }
}

/// Timed feed recovery awards only the successful non-rotary control check, within one host commit.
#[tokio::test]
async fn unjam_control_experience_and_delivery_rollback() {
    use stompymux_rs::*;
    let (_dir, config, pristine, _id) = fixture('.').await;
    let scripts = Scripts::new(
        &config,
        std::rc::Rc::new(std::cell::RefCell::new(pristine.clone())),
    )
    .unwrap();
    let pristine_db = snapshot_database(&config);
    for extended in [false, true] {
        for case in [
            "success",
            "failed",
            "empty",
            "disconnected",
            "tactical",
            "rotary",
            "prone",
        ] {
            restore_database(&config, &pristine_db);
            let (mut world, id) = (pristine.clone(), _id);
            world
                .objects
                .get_mut(&ObjectId(1))
                .unwrap()
                .flags
                .insert(Flag::Connected);
            if case != "tactical" {
                world
                    .objects
                    .get_mut(&id)
                    .unwrap()
                    .flags
                    .insert(Flag::InCharacter);
            }
            set_battle_character(
                &mut world,
                ObjectId(1),
                BattleCharacter {
                    build: 5,
                    reflexes: 4,
                    intuition: 5,
                    learn: 5,
                    charisma: 5,
                    bruise: 0,
                    lethal: 0,
                },
            )
            .unwrap();
            support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
            let skill = if extended {
                "Piloting-Biped"
            } else {
                "Piloting-Battlemech"
            };
            set_battle_character_value(
                &mut world,
                ObjectId(1),
                skill,
                BattleCharacterValue {
                    value: 0,
                    experience: 0,
                    last_used: 0,
                },
            )
            .unwrap();
            if case == "disconnected" {
                world
                    .objects
                    .get_mut(&ObjectId(1))
                    .unwrap()
                    .flags
                    .remove(Flag::Connected);
            }
            if case == "rotary" {
                let mut definition = world.btech.constructed_units()[&id].definition().clone();
                let weapon = BattleWeapon::RotaryAc2;
                let mut part = definition.sections[&BattleSection::LeftArm].criticals[&2].clone();
                part.equipment = weapon.name().into();
                for slot in 2..2 + weapon.profile().critical_slots {
                    definition
                        .sections
                        .get_mut(&BattleSection::LeftArm)
                        .unwrap()
                        .criticals
                        .insert(slot, part.clone());
                }
                let bin = definition
                    .sections
                    .values_mut()
                    .flat_map(|s| s.criticals.values_mut())
                    .find(|p| p.equipment.starts_with("Ammo_"))
                    .unwrap();
                bin.equipment = format!("Ammo_{}", weapon.name());
                bin.data = weapon.profile().ammunition_per_ton.to_string();
                world.btech.set_unit_definition(id, definition).unwrap();
            }
            let mut unit = world.btech.constructed_units()[&id].clone();
            let weapon = if case == "rotary" {
                BattleWeapon::RotaryAc2
            } else {
                BattleWeapon::Srm4
            };
            let index = unit
                .loadout()
                .unwrap()
                .weapons
                .iter()
                .position(|m| m.weapon == weapon)
                .unwrap();
            unit.jam_weapon(index).unwrap();
            let mut state = serde_json::to_value(&world.btech).unwrap();
            state["constructed"][id.0.to_string()] = serde_json::to_value(unit).unwrap();
            if case == "prone" {
                state["constructed"][id.0.to_string()]["posture"] = "prone".into();
            }
            world.btech = serde_json::from_value(state).unwrap();
            begin_battle_unjam(&mut world, id, ObjectId(1), index).unwrap();
            let seed = (0..=255)
                .find(|seed| {
                    BattleDice::seeded([*seed; 32]).two_d6()
                        == if case == "failed" { 2 } else { 12 }
                })
                .unwrap();
            shot_seed(&mut world, id, seed);
            install_xp_channels(&mut world);
            let mut debug = Channel::new("MechDebugInfo".into());
            debug.users.push(communication::Membership {
                who: ObjectId(1),
                listening: true,
            });
            world.channels.insert("MechDebugInfo".into(), debug);
            install(&scripts, world);
            if case != "tactical" {
                let before = scripts.world().btech.clone();
                assert!(
                    advance_battle_unjamming(&mut scripts.world_mut(), extended, extended).is_err()
                );
                assert_eq!(scripts.world().btech, before);
            }
            for _ in 0..59 {
                advance_battle_unjamming_action(&scripts, &config, extended, extended).unwrap();
                assert!(scripts.drain_outbox().is_empty());
            }
            assert_eq!(
                scripts.world().btech.constructed_units()[&id]
                    .unjam()
                    .unwrap()
                    .remaining,
                1
            );
            if case == "empty" {
                let mut state = serde_json::to_value(&scripts.world().btech).unwrap();
                state["constructed"][id.0.to_string()]["ammunition"][0] = 0.into();
                scripts.world_mut().btech = serde_json::from_value(state).unwrap();
            }
            let before = scripts.world().clone();
            persistence::save(&config.database(), &before)
                .await
                .unwrap();
            let mut restored = persistence::load(&config.database()).await.unwrap();
            assert_eq!(restored.btech, before.btech);
            restored.objects.get_mut(&ObjectId(1)).unwrap().flags =
                before.objects[&ObjectId(1)].flags.clone();
            *scripts.world_mut() = restored;
            if case == "success" {
                scripts
                    .world_mut()
                    .channels
                    .get_mut("MechPilotXP")
                    .unwrap()
                    .messages = i64::MAX;
                assert!(
                    advance_battle_unjamming_action(&scripts, &config, extended, extended).is_err()
                );
                assert_eq!(scripts.world().btech, before.btech);
                assert!(scripts.world().channels["MechPilotXP"].history.is_empty());
                assert_eq!(scripts.world().channels["MechDebugInfo"].messages, 0);
                assert!(scripts.world().channels["MechDebugInfo"].history.is_empty());
                assert!(scripts.drain_outbox().is_empty());
                *scripts.world_mut() = before.clone();
            }
            advance_battle_unjamming_action(&scripts, &config, extended, extended).unwrap();
            let candidate = scripts.world().clone();
            let unit = &candidate.btech.constructed_units()[&id];
            assert!(unit.unjam().is_none());
            assert_eq!(unit.weapon_jammed(index).unwrap(), case == "failed");
            assert_eq!(
                unit.ammunition()[0],
                before.btech.constructed_units()[&id].ammunition()[0]
                    - u16::from(!matches!(case, "failed" | "empty"))
            );
            assert_eq!(
                candidate.btech.character_values()[&ObjectId(1)][skill].experience_balance(),
                if case == "success" { 2 } else { 0 }
            );
            assert_eq!(
                candidate.channels["MechPilotXP"].history.len(),
                usize::from(case == "success")
            );
            persistence::save(&config.database(), &candidate)
                .await
                .unwrap();
            assert_eq!(
                persistence::load(&config.database()).await.unwrap().btech,
                candidate.btech
            );
            let output = scripts.drain_outbox();
            let rolled = !matches!(case, "empty" | "rotary" | "prone");
            assert_eq!(
                candidate.channels["MechDebugInfo"].messages,
                i64::from(rolled)
            );
            if rolled && case != "disconnected" {
                let diagnostic = output
                    .iter()
                    .position(|(_, text)| {
                        text.source()
                            .contains("Attempting to make pilot (noxp) skill roll.")
                    })
                    .unwrap();
                let cockpit = output
                    .iter()
                    .position(|(_, text)| text.source() == "You make a piloting skill roll!")
                    .unwrap();
                assert!(diagnostic < cockpit);
            }
            advance_battle_unjamming_action(&scripts, &config, extended, extended).unwrap();
            assert_eq!(scripts.world().btech, candidate.btech);
            assert!(scripts.drain_outbox().is_empty());
        }
    }
}

/// The server commits recovery and XP together, retrying the saved final second after a database failure.
#[tokio::test]
async fn unjam_character_server_tick_retries_failed_commit() {
    use sqlx::Connection;
    use stompymux_rs::*;
    tokio::task::LocalSet::new().run_until(async {
        let (_dir, config, mut world, id) = fixture('.').await;
        world.objects.get_mut(&id).unwrap().flags.insert(Flag::InCharacter);
        world.objects.get_mut(&ObjectId(1)).unwrap().flags.insert(Flag::Connected);
        set_battle_character(&mut world, ObjectId(1), BattleCharacter {
            build: 5, reflexes: 4, intuition: 5, learn: 5, charisma: 5,
            bruise: 0, lethal: 0,
        }).unwrap();
        support::seed_object_dice(&mut world, ObjectId(1), support::FIXTURE_DICE_SEED);
        let skill = if config.battletech.extended_piloting != 0 { "Piloting-Biped" } else { "Piloting-Battlemech" };
        set_battle_character_value(&mut world, ObjectId(1), skill, BattleCharacterValue {
            value: 0, experience: 0, last_used: 0,
        }).unwrap();
        let mut unit = world.btech.constructed_units()[&id].clone();
        let index = unit.loadout().unwrap().weapons.iter().position(|m| m.weapon == BattleWeapon::Srm4).unwrap();
        unit.jam_weapon(index).unwrap();
        let mut state = serde_json::to_value(&world.btech).unwrap();
        state["constructed"][id.0.to_string()] = serde_json::to_value(unit).unwrap();
        state["constructed"][id.0.to_string()]["unjam"] = serde_json::json!({"weapon_index":index,"remaining":60});
        world.btech = serde_json::from_value(state).unwrap();
        let seed = (0..=255).find(|seed| BattleDice::seeded([*seed; 32]).two_d6() == 12).unwrap();
        shot_seed(&mut world, id, seed);
        install_xp_channels(&mut world);
        world.accounts.get_mut(&ObjectId(1)).unwrap().hash = Some(accounts::hash("secret", &config).unwrap());
        persistence::save(&config.database(), &world).await.unwrap();
        let shared = std::rc::Rc::new(std::cell::RefCell::new(world));
        let scripts = Scripts::new(&config, shared.clone()).unwrap();
        let (driver, trigger) = HeartbeatDriver::manual();
        let mut heartbeats = support::Heartbeats::new(trigger, scripts.progress(), &config);
        let mut sql = sqlx::SqliteConnection::connect_with(&sqlx::sqlite::SqliteConnectOptions::new().filename(config.database())).await.unwrap();
        // Reject completion before the server starts, while allowing login and countdown saves.
        sqlx::raw_sql("CREATE TRIGGER deny_unjam BEFORE UPDATE ON btech_units WHEN json_extract(OLD.live, '$.unjam') IS NOT NULL AND json_extract(NEW.live, '$.unjam') IS NULL BEGIN SELECT RAISE(ABORT,'unjam failure'); END;").execute(&mut sql).await.unwrap();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (shutdown, request) = tokio::sync::oneshot::channel();
        let server_config = config.clone();
        let task = tokio::task::spawn_local(async move {
            run_with_schedule_clock(server_config, scripts, listener, async { request.await.unwrap() }, || 1, driver).await
        });
        let mut client = support::Client { socket: tokio::net::TcpStream::connect(address).await.unwrap(), pending: Vec::new() };
        client.until("Who are you? ").await;
        client.send("#1").await;
        client.until("Password: ").await;
        client.send("secret").await;
        client.until("Jenner").await;
        let mut state = serde_json::to_value(&shared.borrow().btech).unwrap();
        state["constructed"][id.0.to_string()]["unjam"]["remaining"] = 1.into();
        shared.borrow_mut().btech = serde_json::from_value(state).unwrap();
        let before = shared.borrow().clone();
        persistence::save(&config.database(), &before).await.unwrap();
        // The shared world holds the candidate only while the server awaits SQLite, and the
        // attempt returns after that heartbeat's rollback, never in the middle of it.
        heartbeats.attempt().await;
        let saved = persistence::load(&config.database()).await.unwrap();
        assert_eq!(saved.btech, before.btech);
        assert!(shared.borrow().btech == before.btech, "The failed recovery must restore the complete world");
        assert!(shared.borrow().channels["MechPilotXP"].history.is_empty());
        sqlx::query("DROP TRIGGER deny_unjam").execute(&mut sql).await.unwrap();
        let saved = heartbeats.until_saved(&config, 5, |saved| saved.btech.constructed_units()[&id].unjam().is_none()).await;
        assert!(!saved.btech.constructed_units()[&id].weapon_jammed(index).unwrap());
        assert_eq!(saved.btech.constructed_units()[&id].ammunition()[0], before.btech.constructed_units()[&id].ammunition()[0] - 1);
        assert_eq!(saved.btech.character_values()[&ObjectId(1)][skill].experience_balance(), 2);
        assert_eq!(saved.channels["MechPilotXP"].history.len(), 1);
        client.until(&format!("GOD gained 2 {skill} XP")).await;
        shutdown.send(ShutdownRequest::Sigterm).unwrap();
        task.await.unwrap().unwrap();
    }).await;
}
